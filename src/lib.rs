use pgrx::{default, pg_extern, pg_module_magic};

pg_module_magic!();

mod compression;

#[pg_extern(immutable, parallel_safe)]
fn pt_gzip(data: Option<&[u8]>, level: default!(Option<i32>, "NULL")) -> Option<Vec<u8>> {
    // Need to take and return `Option` to handle NULL input in the second param
    // Otherwise calling it with NULL will panic, at least in tests
    data.map(|v| compression::pt_gzip(v, level).unwrap())
}

#[pg_extern(immutable, parallel_safe)]
fn pt_brotli(data: &[u8]) -> Vec<u8> {
    compression::pt_brotli(data).unwrap()
}

#[cfg(any(test, feature = "pg_test"))]
#[pgrx::pg_schema]
mod tests {
    use super::*;
    use pgrx::prelude::*;
    use std::fmt::Write as _;

    fn gzip_test(data: Option<&str>, level: Option<i32>) {
        let mut query = "SELECT pt_gzip(".to_string();
        if let Some(data) = data {
            write!(query, "'{data}'::bytea").unwrap();
        } else {
            write!(query, "NULL").unwrap();
        }
        if let Some(level) = level {
            write!(query, ", {level})").unwrap();
        } else {
            write!(query, ")").unwrap();
        }
        let result = Spi::get_one::<&[u8]>(&query).unwrap();
        let expected = data.map(|v| compression::pt_gzip(v.as_bytes(), level).unwrap());
        assert_eq!(result, expected.as_deref(), "{query}");
    }

    fn brotli_test(data: Option<&str>) {
        let mut query = "SELECT pt_brotli(".to_string();
        if let Some(data) = data {
            write!(query, "'{data}'::bytea)").unwrap();
        } else {
            write!(query, "NULL)").unwrap();
        }
        let result = Spi::get_one::<&[u8]>(&query).unwrap();
        let expected = data.map(|v| compression::pt_brotli(v.as_bytes()).unwrap());
        assert_eq!(result, expected.as_deref(), "{query}");
    }

    #[pg_test]
    fn test_pt_gzip() {
        gzip_test(None, None);
        gzip_test(None, Some(5));

        let data = Some("");
        gzip_test(data, None);

        let data = Some("Hello");
        gzip_test(data, Some(0));
        gzip_test(data, Some(1));
        gzip_test(data, Some(9));
        gzip_test(data, None);
    }

    #[pg_test]
    fn test_pt_brotli() {
        brotli_test(None);
        brotli_test(Some(""));
        brotli_test(Some("Hello"));
    }
}

/// This module is required by `cargo pgrx test` invocations.
/// It must be visible at the root of your extension crate.
#[cfg(test)]
pub mod pg_test {
    pub fn setup(_options: Vec<&str>) {
        // perform one-off initialization when the pg_test framework starts
    }

    pub fn postgresql_conf_options() -> Vec<&'static str> {
        // return any postgresql.conf settings that are required for your tests
        vec![]
    }
}
