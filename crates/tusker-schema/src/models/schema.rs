use std::collections::HashMap;

use itertools::Itertools;

use crate::diff::{diff, ChangeType, Diff, DiffSql};

use super::{
    constraint::Constraint, domain::Domain, extension::Extension, index::Index, r#enum::Enum,
    routine::Routine, sequence::Sequence, table::Table, trigger::Trigger, view::View,
};

#[derive(Debug, Default, Eq, PartialEq)]
pub struct Schema {
    pub name: String,
    pub enums: HashMap<String, Enum>,
    pub domains: HashMap<String, Domain>,
    pub sequences: HashMap<String, Sequence>,
    pub extensions: HashMap<String, Extension>,
    pub indexes: HashMap<String, Index>,
    pub tables: HashMap<String, Table>,
    pub views: HashMap<String, View>,
    pub routines: HashMap<(String, String), Routine>,
    pub triggers: HashMap<(String, String), Trigger>,
    pub constraints: HashMap<(String, String), Constraint>,
}

impl Schema {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_owned(),
            ..Default::default()
        }
    }
    pub fn diff_tables<'a>(&'a self, other: &'a Self) -> Diff<'a, Table> {
        diff(
            self.tables.values().sorted_by(|a, b| a.name.cmp(&b.name)),
            other.tables.values().sorted_by(|a, b| a.name.cmp(&b.name)),
            |table| &table.name,
        )
    }
    pub fn diff_enums<'a>(&'a self, other: &'a Self) -> Diff<'a, Enum> {
        diff(
            self.enums.values().sorted_by(|a, b| a.name.cmp(&b.name)),
            other.enums.values().sorted_by(|a, b| a.name.cmp(&b.name)),
            |e| &e.name,
        )
    }
    pub fn diff_domains<'a>(&'a self, other: &'a Self) -> Diff<'a, Domain> {
        diff(
            self.domains.values().sorted_by(|a, b| a.name.cmp(&b.name)),
            other.domains.values().sorted_by(|a, b| a.name.cmp(&b.name)),
            |d| &d.name,
        )
    }
    pub fn diff_sequences<'a>(&'a self, other: &'a Self) -> Diff<'a, Sequence> {
        diff(
            self.sequences
                .values()
                .sorted_by(|a, b| a.name.cmp(&b.name)),
            other
                .sequences
                .values()
                .sorted_by(|a, b| a.name.cmp(&b.name)),
            |s| &s.name,
        )
    }
    pub fn diff_extensions<'a>(&'a self, other: &'a Self) -> Diff<'a, Extension> {
        diff(
            self.extensions
                .values()
                .sorted_by(|a, b| a.name.cmp(&b.name)),
            other
                .extensions
                .values()
                .sorted_by(|a, b| a.name.cmp(&b.name)),
            |e| &e.name,
        )
    }
    pub fn diff_indexes<'a>(&'a self, other: &'a Self) -> Diff<'a, Index> {
        diff(
            self.indexes.values().sorted_by(|a, b| a.name.cmp(&b.name)),
            other.indexes.values().sorted_by(|a, b| a.name.cmp(&b.name)),
            |index| &index.name,
        )
    }
    pub fn diff_constraints<'a>(&'a self, other: &'a Self) -> Diff<'a, Constraint> {
        diff(
            self.constraints
                .values()
                .sorted_by(|a, b| (&a.table, &a.name).cmp(&(&b.table, &b.name))),
            other
                .constraints
                .values()
                .sorted_by(|a, b| (&a.table, &a.name).cmp(&(&b.table, &b.name))),
            |c| (&c.table, &c.name),
        )
    }
    pub fn diff_routines<'a>(&'a self, other: &'a Self) -> Diff<'a, Routine> {
        diff(
            self.routines.values().sorted_by(|a, b| {
                (&a.name, &a.identity_arguments).cmp(&(&b.name, &b.identity_arguments))
            }),
            other.routines.values().sorted_by(|a, b| {
                (&a.name, &a.identity_arguments).cmp(&(&b.name, &b.identity_arguments))
            }),
            |f| (&f.name, &f.identity_arguments),
        )
    }
    pub fn diff_triggers<'a>(&'a self, other: &'a Self) -> Diff<'a, Trigger> {
        diff(
            self.triggers
                .values()
                .sorted_by(|a, b| (&a.table_name, &a.name).cmp(&(&b.table_name, &b.name))),
            other
                .triggers
                .values()
                .sorted_by(|a, b| (&a.table_name, &a.name).cmp(&(&b.table_name, &b.name))),
            |t| (&t.table_name, &t.name),
        )
    }
}

impl DiffSql for Diff<'_, Schema> {
    fn sql(&self) -> Vec<(ChangeType, String)> {
        let mut v = Vec::new();
        if !self.a_only.is_empty() {
            todo!("Schema creation not supported, yet.")
        }
        for (a, b) in &self.a_and_b {
            v.extend(a.diff_triggers(b).sql());
            v.extend(a.diff_enums(b).sql());
            v.extend(a.diff_domains(b).sql());
            v.extend(a.diff_sequences(b).sql());
            v.extend(a.diff_extensions(b).sql());
            v.extend(a.diff_routines(b).sql());
            v.extend(a.diff_tables(b).sql());
            v.extend(a.diff_indexes(b).sql());
            v.extend(a.diff_constraints(b).sql());
        }
        if !self.b_only.is_empty() {
            println!("{:?}", self.b_only);
            todo!("Schema creation not supported, yet.")
        }
        v
    }
}

pub fn join_sql(v: Vec<(ChangeType, String)>) -> String {
    v.into_iter()
        .sorted_by(|a, b| a.0.cmp(&b.0))
        .map(|t| t.1)
        .join("\n")
}

#[cfg(test)]
mod tests {
    use crate::diff::ChangeType;

    use super::join_sql;

    #[test]
    fn join_sql_preserves_insertion_order_within_same_change_type() {
        let sql = join_sql(vec![
            (ChangeType::CreateRoutine, "second;\n".into()),
            (ChangeType::CreateRoutine, "first;\n".into()),
        ]);

        assert_eq!(sql, "second;\n\nfirst;\n");
    }

    #[test]
    fn join_sql_creates_routines_before_tables() {
        let sql = join_sql(vec![
            (
                ChangeType::CreateTable,
                "CREATE TABLE uses_func (id integer);\n".into(),
            ),
            (
                ChangeType::CreateRoutine,
                "CREATE FUNCTION helper() RETURNS integer LANGUAGE sql AS $$ SELECT 1 $$;\n".into(),
            ),
        ]);

        assert_eq!(
            sql,
            "CREATE FUNCTION helper() RETURNS integer LANGUAGE sql AS $$ SELECT 1 $$;\n\nCREATE TABLE uses_func (id integer);\n"
        );
    }
}
