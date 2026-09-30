use postgres_types::{FromSql, ToSql};
use tusker_query::{FromRow, Query, QueryEnum};

#[derive(Debug, FromSql, ToSql, QueryEnum)]
#[postgres(name = "group_kind", rename_all = "snake_case")]
enum GroupKind {
    Announcement,
    Community,
    Digest,
}

#[derive(FromRow)]
struct Group {
    id: i32,
    kind: GroupKind,
}

#[derive(Query)]
#[query(sql = "group_by_kind", row = Group)]
struct GroupByKind {
    kind: GroupKind,
}

fn main() {}
