use std::{cmp::Reverse, collections::HashMap, fmt::Debug, hash::Hash};

use crate::models::constraint::ConstraintType;

#[derive(Debug, Eq, PartialEq)]
pub struct Diff<'a, T: Eq> {
    pub a_only: Vec<&'a T>,
    pub a_and_b: Vec<(&'a T, &'a T)>,
    pub b_only: Vec<&'a T>,
}

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
pub enum ChangeType {
    // DROP CONSTRAINT statements must be generated in reverse
    // order.
    DropConstraint(Reverse<ConstraintType>),
    DropColumn,
    DropTrigger,
    DropRoutine,
    DropSequence,
    DropIndex,
    DropTable,
    DropType,
    DropExtension,
    DropSchema,
    AlterExtension,
    AlterSequence,
    AlterType,
    AlterColumn,
    Unsupported,
    CreateSchema,
    CreateExtension,
    CreateSequence,
    CreateType,
    CreateRoutine,
    CreateTable,
    CreateIndex,
    CreateColumn,
    CreateConstraint(ConstraintType),
    CreateTrigger,
}

pub trait DiffSql {
    fn sql(&self) -> Vec<(ChangeType, String)>;
}
