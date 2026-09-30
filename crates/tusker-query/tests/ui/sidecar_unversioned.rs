use tusker_query::{FromRow, Query};

#[derive(FromRow)]
struct One {
    one: Option<i32>,
}

#[derive(Query)]
#[query(sql = "unversioned", row = One)]
struct SelectOne {}

fn main() {}
