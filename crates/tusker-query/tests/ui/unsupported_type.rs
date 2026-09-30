use tusker_query::{FromRow, Query};

#[derive(FromRow)]
struct Empty {}

#[derive(Query)]
#[query(sql = "interval", row = Empty)]
struct Interval {
    duration: String,
}

fn main() {}
