# postile

[![GitHub](https://img.shields.io/badge/github-postile-8da0cb?logo=github)](https://github.com/nyurik/postile)
[![CI build](https://github.com/nyurik/postile/actions/workflows/ci.yml/badge.svg)](https://github.com/nyurik/postile/actions)
[![Codecov](https://img.shields.io/codecov/c/github/nyurik/postile)](https://app.codecov.io/gh/nyurik/postile)

A PostgreSQL extension with various map tile generation functions.  Some functions could help generating tiles for [Martin tile server](https://maplibre.org/martin/) and similar projects.

## Usage

```sql
CREATE EXTENSION postile;

-- Compress with gzip, and return as bytea.
-- Optional second argument is the compression level, 1-9.
SELECT pt_gzip('Hello, world!');

-- Compress with Brotil, and return as bytea.
SELECT pt_brotli('Hello, world!');
```

## Development

* This project is easier to develop with [just](https://github.com/casey/just#readme), a modern alternative to `make`.
  Install it with `cargo install just`.
* To get a list of available commands, run `just`.
* To run tests, use `just test`.

## License

Licensed under either of

* Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or <https://www.apache.org/licenses/LICENSE-2.0>)
* MIT license ([LICENSE-MIT](LICENSE-MIT) or <https://opensource.org/licenses/MIT>)
  at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally
submitted for inclusion in the work by you, as defined in the
Apache-2.0 license, shall be dual-licensed as above, without any
additional terms or conditions.
