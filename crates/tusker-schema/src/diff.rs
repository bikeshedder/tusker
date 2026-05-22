use std::{cmp::Reverse, collections::HashMap, fmt::Debug, hash::Hash};

use crate::models::constraint::ConstraintType;

#[derive(Debug, Eq, PartialEq)]
/// Result of comparing two keyed collections of schema objects.
pub struct Diff<'a, T: Eq> {
    /// Items only present in the left-hand side.
    pub a_only: Vec<&'a T>,
    /// Items present in both sides, paired by key.
    pub a_and_b: Vec<(&'a T, &'a T)>,
    /// Items only present in the right-hand side.
    pub b_only: Vec<&'a T>,
}

/// Matches two iterators by key and groups items into additions, removals, and pairs.
pub fn diff<'a, T: Eq, K>(
    a: impl Iterator<Item = &'a T>,
    b: impl Iterator<Item = &'a T>,
    key: fn(&'a T) -> K,
) -> Diff<'a, T>
where
    K: Hash + Eq + PartialEq,
{
    let mut a_only = a.collect::<Vec<_>>();
    let mut a_map = a_only
        .iter()
        .map(|&x| (key(x), x))
        .collect::<HashMap<K, &T>>();
    let mut a_and_b: Vec<(&T, &T)> = Vec::new();
    let mut b_only: Vec<&T> = Vec::new();
    for b_item in b {
        if let Some(a_item) = a_map.remove(&key(b_item)) {
            a_and_b.push((a_item, b_item));
        } else {
            b_only.push(b_item);
        }
    }
    a_only.retain(|x| a_map.contains_key(&key(x)));
    Diff {
        a_only,
        a_and_b,
        b_only,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
/// Ordering buckets used when rendering schema changes as SQL statements.
pub enum ChangeType {
    // DROP CONSTRAINT statements must be generated in reverse
    // order.
    /// Drops a constraint, ordered by reverse dependency priority.
    DropConstraint(Reverse<ConstraintType>),
    /// Drops a column from an existing table.
    DropColumn,
    /// Drops a trigger.
    DropTrigger,
    /// Drops a function, procedure, or aggregate.
    DropRoutine,
    /// Drops a sequence.
    DropSequence,
    /// Drops a standalone index.
    DropIndex,
    /// Drops a table.
    DropTable,
    /// Drops a type-like object such as an enum or domain.
    DropType,
    /// Drops a PostgreSQL extension.
    DropExtension,
    /// Drops a schema.
    DropSchema,
    /// Alters an extension in place.
    AlterExtension,
    /// Alters a sequence in place.
    AlterSequence,
    /// Alters a type-like object in place.
    AlterType,
    /// Alters one or more columns on an existing table.
    AlterColumn,
    /// Emits a warning or failing guard for an unsupported change.
    Unsupported,
    /// Creates a schema.
    CreateSchema,
    /// Creates an extension.
    CreateExtension,
    /// Creates a sequence.
    CreateSequence,
    /// Creates a type-like object such as an enum or domain.
    CreateType,
    /// Creates a function, procedure, or aggregate.
    CreateRoutine,
    /// Creates a table.
    CreateTable,
    /// Creates a standalone index.
    CreateIndex,
    /// Creates a column.
    CreateColumn,
    /// Creates a constraint.
    CreateConstraint(ConstraintType),
    /// Creates a trigger.
    CreateTrigger,
}

/// Converts a diff into ordered SQL fragments.
pub trait DiffSql {
    /// Produces SQL statements paired with their ordering bucket.
    fn sql(&self) -> Vec<(ChangeType, String)>;
}
