use std::collections::{BTreeSet, HashMap};

use crate::{
    diff::{ChangeType, Diff, DiffSql},
    queries::{RoutineDependencyRow, RoutineKind, RoutineRow},
    sql::quote_ident,
};

#[derive(Debug, Clone, Eq, Hash, Ord, PartialEq, PartialOrd)]
struct RoutineKey {
    schema: String,
    name: String,
    identity_arguments: String,
}

impl RoutineKey {
    fn new(schema: &str, name: &str, identity_arguments: &str) -> Self {
        Self {
            schema: schema.to_owned(),
            name: name.to_owned(),
            identity_arguments: identity_arguments.to_owned(),
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Routine {
    pub schema: String,
    pub name: String,
    pub kind: RoutineKind,
    pub identity_arguments: String,
    pub definition: String,
    dependencies: Vec<RoutineKey>,
}

impl Routine {
    fn key(&self) -> RoutineKey {
        RoutineKey::new(&self.schema, &self.name, &self.identity_arguments)
    }

    fn create_sql(&self) -> String {
        // `pg_get_functiondef` returns the body without a guaranteed trailing
        // statement terminator/newline, while our fixture output and migration
        // application logic expect a complete standalone statement.
        format!(
            "{};\n",
            self.definition.trim_end_matches('\n').trim_end_matches(';')
        )
    }

    fn drop_sql(&self) -> String {
        let thing = match self.kind {
            RoutineKind::Function => "FUNCTION",
            RoutineKind::Procedure => "PROCEDURE",
            RoutineKind::Aggregate => "AGGREGATE",
            RoutineKind::Window => "FUNCTION",
        };
        format!(
            "DROP {} {}.{}({});\n",
            thing,
            quote_ident(&self.schema),
            quote_ident(&self.name),
            self.identity_arguments,
        )
    }

    fn create_order<'a>(routines: Vec<&'a Routine>) -> Vec<&'a Routine> {
        topological_order(routines, false)
    }

    fn drop_order<'a>(routines: Vec<&'a Routine>) -> Vec<&'a Routine> {
        topological_order(routines, true)
    }
}

impl From<RoutineRow> for Routine {
    fn from(row: RoutineRow) -> Self {
        let mut dependencies = row
            .dependencies
            .0
            .into_iter()
            .map(RoutineKey::from)
            .collect::<Vec<_>>();
        dependencies.sort();
        dependencies.dedup();

        Self {
            schema: row.schema,
            name: row.name,
            kind: row.kind,
            identity_arguments: row.identity_arguments,
            definition: row.definition,
            dependencies,
        }
    }
}

impl DiffSql for Diff<'_, Routine> {
    fn sql(&self) -> Vec<(ChangeType, String)> {
        let mut v = Vec::new();

        let mut drops = self.a_only.clone();
        let mut creates = self.b_only.clone();

        for (a, b) in &self.a_and_b {
            if a != b {
                drops.push(a);
                creates.push(b);
            }
        }

        for a in Routine::drop_order(drops) {
            v.push((ChangeType::DropRoutine, a.drop_sql()));
        }

        for b in Routine::create_order(creates) {
            v.push((ChangeType::CreateRoutine, b.create_sql()));
        }

        v
    }
}

impl From<RoutineDependencyRow> for RoutineKey {
    fn from(value: RoutineDependencyRow) -> Self {
        Self::new(&value.schema, &value.name, &value.identity_arguments)
    }
}

fn topological_order<'a>(mut routines: Vec<&'a Routine>, reverse: bool) -> Vec<&'a Routine> {
    routines.sort_by_key(|routine| routine.key());

    let routines_by_key = routines
        .iter()
        .map(|routine| (routine.key(), *routine))
        .collect::<HashMap<_, _>>();

    let mut dependents = HashMap::<RoutineKey, Vec<RoutineKey>>::new();
    let mut indegree = routines_by_key
        .keys()
        .cloned()
        .map(|key| (key, 0usize))
        .collect::<HashMap<_, _>>();

    for routine in &routines {
        let routine_key = routine.key();
        for dependency in &routine.dependencies {
            if routines_by_key.contains_key(dependency) {
                dependents
                    .entry(dependency.clone())
                    .or_default()
                    .push(routine_key.clone());
                *indegree.entry(routine_key.clone()).or_default() += 1;
            }
        }
    }

    let mut ready = indegree
        .iter()
        .filter(|(_, degree)| **degree == 0)
        .map(|(key, _)| key.clone())
        .collect::<BTreeSet<_>>();
    let mut ordered_keys = Vec::with_capacity(routines.len());

    while let Some(next) = ready.pop_first() {
        ordered_keys.push(next.clone());
        if let Some(next_dependents) = dependents.get(&next) {
            for dependent in next_dependents {
                let degree = indegree
                    .get_mut(dependent)
                    .expect("dependent routine should have indegree");
                *degree -= 1;
                if *degree == 0 {
                    ready.insert(dependent.clone());
                }
            }
        }
    }

    if ordered_keys.len() != routines.len() {
        for key in routines_by_key.keys().cloned().collect::<BTreeSet<_>>() {
            if !ordered_keys.contains(&key) {
                ordered_keys.push(key);
            }
        }
    }

    let mut ordered = ordered_keys
        .into_iter()
        .map(|key| routines_by_key[&key])
        .collect::<Vec<_>>();

    if reverse {
        ordered.reverse();
    }

    ordered
}

#[cfg(test)]
mod tests {
    use crate::{
        diff::{Diff, DiffSql},
        models::{routine::Routine, schema::join_sql},
        queries::RoutineKind,
    };

    use super::RoutineKey;

    fn routine(name: &str, body: &str, dependencies: &[(&str, &str, &str)]) -> Routine {
        Routine {
            schema: "public".into(),
            name: name.into(),
            kind: RoutineKind::Function,
            identity_arguments: "a integer".into(),
            definition: format!(
                "CREATE OR REPLACE FUNCTION public.{name}(a integer)\nRETURNS integer\nLANGUAGE sql\nAS $$\n    {body}\n$$"
            ),
            dependencies: dependencies
                .iter()
                .map(|(schema, dep_name, identity_arguments)| {
                    RoutineKey::new(schema, dep_name, identity_arguments)
                })
                .collect(),
        }
    }

    #[test]
    fn orders_created_routines_by_dependency() {
        let base = routine("base_value", "SELECT a + 1;", &[]);
        let wrapper = routine(
            "wrapper_value",
            "SELECT public.base_value(a);",
            &[("public", "base_value", "a integer")],
        );
        let diff = Diff {
            a_only: vec![],
            a_and_b: vec![],
            b_only: vec![&wrapper, &base],
        };

        assert_eq!(
            join_sql(diff.sql()),
            "CREATE OR REPLACE FUNCTION public.base_value(a integer)\nRETURNS integer\nLANGUAGE sql\nAS $$\n    SELECT a + 1;\n$$;\n\nCREATE OR REPLACE FUNCTION public.wrapper_value(a integer)\nRETURNS integer\nLANGUAGE sql\nAS $$\n    SELECT public.base_value(a);\n$$;\n"
        );
    }

    #[test]
    fn drops_dependents_before_dependencies() {
        let old_base = routine("base_value", "SELECT a + 1;", &[]);
        let old_wrapper = routine(
            "wrapper_value",
            "SELECT public.base_value(a);",
            &[("public", "base_value", "a integer")],
        );
        let new_base = routine("base_value", "SELECT a + 2;", &[]);
        let new_wrapper = routine(
            "wrapper_value",
            "SELECT public.base_value(a) + 10;",
            &[("public", "base_value", "a integer")],
        );
        let diff = Diff {
            a_only: vec![],
            a_and_b: vec![(&old_wrapper, &new_wrapper), (&old_base, &new_base)],
            b_only: vec![],
        };

        assert_eq!(
            join_sql(diff.sql()),
            "DROP FUNCTION \"public\".\"wrapper_value\"(a integer);\n\nDROP FUNCTION \"public\".\"base_value\"(a integer);\n\nCREATE OR REPLACE FUNCTION public.base_value(a integer)\nRETURNS integer\nLANGUAGE sql\nAS $$\n    SELECT a + 2;\n$$;\n\nCREATE OR REPLACE FUNCTION public.wrapper_value(a integer)\nRETURNS integer\nLANGUAGE sql\nAS $$\n    SELECT public.base_value(a) + 10;\n$$;\n"
        );
    }
}
