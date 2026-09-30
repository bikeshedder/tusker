use tusker_query::{FromRow, Query};

#[derive(FromRow)]
struct Budget {
    budget: Option<f64>,
}

#[derive(Query)]
#[query(sql = "group_budget", row = Budget)]
struct GroupBudget {}

fn main() {}
