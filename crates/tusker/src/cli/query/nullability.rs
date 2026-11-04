//! Nullability of result columns that originate from table columns.
//!
//! PostgreSQL reports the source table and attribute of a result column, but
//! the `NOT NULL` constraint of that attribute only carries over to the result
//! if every row of the table that reaches the output is a real row. Outer joins
//! and grouping sets add rows in which the columns of a table are NULL. This
//! module inspects the generic plan of the query to find out which tables are
//! scanned in positions where that can happen.

use std::collections::HashSet;

use anyhow::{bail, Context, Result};
use serde_json::Value;
use tokio_postgres::{Client, SimpleQueryMessage};

/// A table identified the way `EXPLAIN (VERBOSE)` reports it.
type RelationName = (String, String);

/// Tables scanned by a query plan, split by whether the plan can NULL-extend
/// their rows.
#[derive(Debug, Default)]
pub(super) struct PlanRelations {
    /// Tables scanned where their rows reach the output unchanged.
    preserved: HashSet<RelationName>,
    /// Tables scanned on the nullable side of an outer join, below a grouping
    /// set, or inside a CTE that is used in such a position.
    nullable: HashSet<RelationName>,
}

impl PlanRelations {
    /// Explain `sql` and collect the tables its generic plan scans.
    ///
    /// `sql` must be a single statement that PostgreSQL could prepare.
    pub(super) async fn explain(client: &Client, sql: &str) -> Result<Self> {
        // GENERIC_PLAN keeps `$n` parameters unbound, so it has to go through
        // the simple query protocol. Preparing `sql` beforehand guarantees that
        // it is a single statement, so this never runs anything else.
        let explain = format!("EXPLAIN (GENERIC_PLAN, VERBOSE, FORMAT JSON) {sql}\n");
        let messages = client
            .simple_query(&explain)
            .await
            .context("Failed to explain query")?;
        let output = messages
            .iter()
            .find_map(|message| match message {
                SimpleQueryMessage::Row(row) => row.get(0),
                _ => None,
            })
            .context("EXPLAIN returned no plan")?;
        let plan: Value = serde_json::from_str(output).context("Failed to parse query plan")?;
        Self::from_plan(&plan)
    }

    /// Collect the tables of a plan in the format of `EXPLAIN (VERBOSE,
    /// FORMAT JSON)`.
    pub(super) fn from_plan(plan: &Value) -> Result<Self> {
        let Some(root) = plan.get(0).and_then(|entry| entry.get("Plan")) else {
            bail!("Unexpected query plan format");
        };
        let mut scans = Vec::new();
        collect_scans(root, false, None, &mut scans);

        // A CTE is nullable if any scan of it is in a nullable position,
        // including scans inside other nullable CTEs.
        let mut nullable_ctes = HashSet::new();
        loop {
            let mut changed = false;
            for scan in &scans {
                if let ScanTarget::Cte(name) = &scan.target {
                    if scan.is_nullable(&nullable_ctes) && nullable_ctes.insert(name.clone()) {
                        changed = true;
                    }
                }
            }
            if !changed {
                break;
            }
        }

        let mut relations = Self::default();
        for scan in &scans {
            if let ScanTarget::Relation(name) = &scan.target {
                if scan.is_nullable(&nullable_ctes) {
                    let _ = relations.nullable.insert(name.clone());
                } else {
                    let _ = relations.preserved.insert(name.clone());
                }
            }
        }
        Ok(relations)
    }

    /// Whether a column of a table can be NULL in the output only if the table
    /// column itself can be NULL.
    ///
    /// `relations` are the table and all tables inheriting from it, including
    /// partitions, since the plan scans those instead of the parent. A table
    /// that the plan does not scan at all does not keep its constraint either:
    /// the planner drops tables that cannot contribute rows, e.g. the right
    /// side of `LEFT JOIN … ON false`.
    pub(super) fn keeps_not_null<'a>(
        &self,
        relations: impl IntoIterator<Item = &'a RelationName> + Clone,
    ) -> bool {
        relations
            .clone()
            .into_iter()
            .any(|name| self.preserved.contains(name))
            && !relations
                .into_iter()
                .any(|name| self.nullable.contains(name))
    }
}

#[derive(Debug)]
enum ScanTarget {
    Relation(RelationName),
    Cte(String),
}

#[derive(Debug)]
struct Scan {
    target: ScanTarget,
    /// Whether the scan itself is in a nullable position.
    nullable: bool,
    /// The CTE whose plan contains the scan.
    cte: Option<String>,
}

impl Scan {
    fn is_nullable(&self, nullable_ctes: &HashSet<String>) -> bool {
        self.nullable
            || self
                .cte
                .as_ref()
                .is_some_and(|cte| nullable_ctes.contains(cte))
    }
}

fn collect_scans(node: &Value, nullable: bool, cte: Option<&str>, scans: &mut Vec<Scan>) {
    let field = |name: &str| node.get(name).and_then(Value::as_str);

    if let (Some(schema), Some(relation)) = (field("Schema"), field("Relation Name")) {
        scans.push(Scan {
            target: ScanTarget::Relation((schema.to_owned(), relation.to_owned())),
            nullable,
            cte: cte.map(str::to_owned),
        });
    }
    if let Some(name) = field("CTE Name") {
        scans.push(Scan {
            target: ScanTarget::Cte(name.to_owned()),
            nullable,
            cte: cte.map(str::to_owned),
        });
    }

    // Join types as the executor sees them: the planner turns `a LEFT JOIN b`
    // into a right join when it swaps the inputs. Semi and anti joins only
    // return rows of one input and never NULL-extend.
    let (outer_nullable, inner_nullable) = match field("Join Type") {
        Some("Left") => (false, true),
        Some("Right") => (true, false),
        Some("Full") => (true, true),
        _ => (false, false),
    };
    // Grouping sets put NULL into the grouping columns that a set leaves out.
    let grouping_sets = node.get("Grouping Sets").is_some();

    let Some(children) = node.get("Plans").and_then(Value::as_array) else {
        return;
    };
    for child in children {
        let relationship = child.get("Parent Relationship").and_then(Value::as_str);
        match relationship {
            Some("InitPlan" | "SubPlan") => {
                // CTEs are planned separately and attached as init plans. Their
                // nullability comes from the positions of their CTE scans.
                // Other subplans compute expressions, whose result columns
                // carry no source table.
                if let Some(name) = child
                    .get("Subplan Name")
                    .and_then(Value::as_str)
                    .and_then(|name| name.strip_prefix("CTE "))
                {
                    collect_scans(child, false, Some(name), scans);
                }
            }
            _ => {
                let side_nullable = match relationship {
                    Some("Outer") => outer_nullable,
                    Some("Inner") => inner_nullable,
                    _ => false,
                };
                collect_scans(
                    child,
                    nullable || side_nullable || grouping_sets,
                    cte,
                    scans,
                );
            }
        }
    }
}
