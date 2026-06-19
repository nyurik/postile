mod compression;
mod pg_funcs;

pgrx::pg_module_magic!(name, version);

#[pgrx::pg_extern]
fn pt_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg(any(test, feature = "pg_test"))]
#[pgrx::pg_schema]
mod tests {
    use pgrx::prelude::*;

    #[pg_test]
    fn test_pt_version() {
        assert_eq!(env!("CARGO_PKG_VERSION"), crate::pt_version());
    }
}

#[cfg(feature = "pg_bench")]
#[pgrx::pg_schema]
mod benches {
    use pgrx::prelude::*;
    use pgrx_bench::{Bencher, black_box};

    #[pg_bench]
    fn bench_pt_version(b: &mut Bencher) {
        b.iter(|| {
            black_box(crate::pt_version());
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
