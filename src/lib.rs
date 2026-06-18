use pgrx::{pg_extern, pg_schema};

mod compression;
mod pg_funcs;

::pgrx::pg_module_magic!(name, version);

#[pg_extern]
fn pt_hello_postile() -> &'static str {
    "Hello, postile"
}

#[cfg(any(test, feature = "pg_test"))]
#[pg_schema]
mod tests {
    use pgrx::prelude::*;

    #[pg_test]
    fn test_hello_postile() {
        assert_eq!("Hello, postile", crate::pt_hello_postile());
    }
}

#[cfg(feature = "pg_bench")]
#[pg_schema]
mod benches {
    use pgrx::prelude::*;
    use pgrx_bench::{Bencher, black_box};

    #[pg_bench]
    fn bench_hello_postile(b: &mut Bencher) {
        b.iter(|| {
            black_box(crate::pt_hello_postile());
        });
    }
}

/// This module is required by `cargo pgrx test` invocations.
/// It must be visible at the root of your extension crate.
#[cfg(test)]
pub mod pg_test {
    #[allow(clippy::needless_pass_by_value)]
    pub fn setup(_options: Vec<&str>) {
        // perform one-off initialization when the pg_test framework starts
    }

    #[must_use]
    pub fn postgresql_conf_options() -> Vec<&'static str> {
        // return any postgresql.conf settings that are required for your tests
        vec![]
    }
}
