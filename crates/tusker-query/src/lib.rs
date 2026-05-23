#![doc = include_str!("../README.md")]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![deny(
    nonstandard_style,
    rust_2018_idioms,
    rustdoc::broken_intra_doc_links,
    rustdoc::private_intra_doc_links
)]
#![forbid(non_ascii_idents, unsafe_code)]
#![warn(
    deprecated_in_future,
    missing_copy_implementations,
    missing_debug_implementations,
    missing_docs,
    unreachable_pub,
    unused_import_braces,
    unused_labels,
    unused_lifetimes,
    unused_qualifications,
    unused_results
)]
#![allow(clippy::uninlined_format_args)]

use tokio_postgres::{types::ToSql, Error, Row, Statement};

pub use tusker_query_derive::Query;

/// Marker traits and PostgreSQL type markers used by checked query validation.
pub mod types;

#[doc(hidden)]
pub mod __private {
    use super::{Error, Row, Statement, ToSql};

    pub mod sealed {
        pub trait Sealed {}

        impl Sealed for tokio_postgres::Client {}
        impl Sealed for tokio_postgres::Transaction<'_> {}

        #[cfg(feature = "deadpool")]
        impl Sealed for deadpool_postgres::Client {}
        #[cfg(feature = "deadpool")]
        impl Sealed for deadpool_postgres::Transaction<'_> {}
    }

    #[doc(hidden)]
    pub trait QueryClient: sealed::Sealed {
        fn prepare_query(
            &self,
            query: &str,
        ) -> impl core::future::Future<Output = Result<Statement, Error>> + Send;

        fn query_prepared<'a>(
            &'a self,
            statement: &'a Statement,
            params: &'a [&'a (dyn ToSql + Sync)],
        ) -> impl core::future::Future<Output = Result<Vec<Row>, Error>> + Send + 'a;

        fn query_one_prepared<'a>(
            &'a self,
            statement: &'a Statement,
            params: &'a [&'a (dyn ToSql + Sync)],
        ) -> impl core::future::Future<Output = Result<Row, Error>> + Send + 'a;
    }

    impl QueryClient for tokio_postgres::Client {
        async fn prepare_query(&self, query: &str) -> Result<Statement, Error> {
            self.prepare(query).await
        }

        async fn query_prepared<'a>(
            &'a self,
            statement: &'a Statement,
            params: &'a [&'a (dyn ToSql + Sync)],
        ) -> Result<Vec<Row>, Error> {
            self.query(statement, params).await
        }

        async fn query_one_prepared<'a>(
            &'a self,
            statement: &'a Statement,
            params: &'a [&'a (dyn ToSql + Sync)],
        ) -> Result<Row, Error> {
            self.query_one(statement, params).await
        }
    }

    impl QueryClient for tokio_postgres::Transaction<'_> {
        async fn prepare_query(&self, query: &str) -> Result<Statement, Error> {
            self.prepare(query).await
        }

        async fn query_prepared<'a>(
            &'a self,
            statement: &'a Statement,
            params: &'a [&'a (dyn ToSql + Sync)],
        ) -> Result<Vec<Row>, Error> {
            self.query(statement, params).await
        }

        async fn query_one_prepared<'a>(
            &'a self,
            statement: &'a Statement,
            params: &'a [&'a (dyn ToSql + Sync)],
        ) -> Result<Row, Error> {
            self.query_one(statement, params).await
        }
    }

    #[cfg(feature = "deadpool")]
    impl QueryClient for deadpool_postgres::Client {
        async fn prepare_query(&self, query: &str) -> Result<Statement, Error> {
            deadpool_postgres::GenericClient::prepare_cached(self, query).await
        }

        async fn query_prepared<'a>(
            &'a self,
            statement: &'a Statement,
            params: &'a [&'a (dyn ToSql + Sync)],
        ) -> Result<Vec<Row>, Error> {
            deadpool_postgres::GenericClient::query(self, statement, params).await
        }

        async fn query_one_prepared<'a>(
            &'a self,
            statement: &'a Statement,
            params: &'a [&'a (dyn ToSql + Sync)],
        ) -> Result<Row, Error> {
            deadpool_postgres::GenericClient::query_one(self, statement, params).await
        }
    }

    #[cfg(feature = "deadpool")]
    impl QueryClient for deadpool_postgres::Transaction<'_> {
        async fn prepare_query(&self, query: &str) -> Result<Statement, Error> {
            deadpool_postgres::GenericClient::prepare_cached(self, query).await
        }

        async fn query_prepared<'a>(
            &'a self,
            statement: &'a Statement,
            params: &'a [&'a (dyn ToSql + Sync)],
        ) -> Result<Vec<Row>, Error> {
            deadpool_postgres::GenericClient::query(self, statement, params).await
        }

        async fn query_one_prepared<'a>(
            &'a self,
            statement: &'a Statement,
            params: &'a [&'a (dyn ToSql + Sync)],
        ) -> Result<Row, Error> {
            deadpool_postgres::GenericClient::query_one(self, statement, params).await
        }
    }

    pub trait RowFieldCount<const N: usize> {}

    pub trait RowFieldType<const I: usize> {
        type Ty;
    }
}

/// A typed SQL query that can be executed through `tokio-postgres`.
pub trait Query: Sized {
    /// SQL text loaded from the query file.
    const SQL: &'static str;
    /// Row type returned by the query.
    type Row: FromRow;
    /// Query bind parameters in positional order.
    fn as_params(&self) -> Box<[&(dyn ToSql + Sync)]>;
}

/// Converts a `tokio-postgres` row into a strongly typed Rust value.
pub trait FromRow {
    /// Builds `Self` from a single database row.
    fn from_row(row: Row) -> Self;
}

pub use tusker_query_derive::FromRow;

impl FromRow for () {
    fn from_row(_: Row) -> Self {}
}

impl __private::RowFieldCount<0> for () {}

/// Executes a query that must return exactly one row.
pub async fn query_one<Q: Query, C>(client: &C, query: Q) -> Result<Q::Row, Error>
where
    C: __private::QueryClient + ?Sized,
{
    let stmt = client.prepare_query(Q::SQL).await?;
    Ok(Q::Row::from_row(
        client.query_one_prepared(&stmt, &query.as_params()).await?,
    ))
}

/// Executes a query and collects all returned rows.
pub async fn query<Q: Query, C>(client: &C, query: Q) -> Result<Vec<Q::Row>, Error>
where
    C: __private::QueryClient + ?Sized,
{
    let stmt = client.prepare_query(Q::SQL).await?;
    let rows = client.query_prepared(&stmt, &query.as_params()).await?;
    Ok(rows.into_iter().map(Q::Row::from_row).collect())
}

#[cfg(all(test, feature = "deadpool"))]
mod tests {
    use super::__private::QueryClient;

    fn assert_query_client<T: QueryClient>() {}

    #[test]
    fn deadpool_client_implements_query_client() {
        assert_query_client::<deadpool_postgres::Client>();
    }

    #[test]
    fn deadpool_transaction_implements_query_client() {
        fn assert_transaction<'a>()
        where
            deadpool_postgres::Transaction<'a>: QueryClient,
        {
        }

        let _ = assert_transaction;
    }
}

#[cfg(test)]
mod doc_tests {
    #[test]
    fn readme_does_not_reference_public_query_client_trait() {
        assert!(!include_str!("../README.md").contains("tusker_query::QueryClient"));
    }
}
