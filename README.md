# postile

[![GitHub repo](https://img.shields.io/badge/github-postile-8da0cb?logo=github)](https://github.com/nyurik/postile)
[![CI build status](https://github.com/nyurik/postile/actions/workflows/ci.yml/badge.svg)](https://github.com/nyurik/postile/actions)
[![Codecov](https://img.shields.io/codecov/c/github/nyurik/postile)](https://app.codecov.io/gh/nyurik/postile)

A PostgreSQL extension with various map tile generation functions.  Some functions could help generating tiles for [Martin tile server](https://maplibre.org/martin/) and similar projects.

## Installation

Download the release archive that matches your operating system, CPU architecture, and PostgreSQL major version. Platform release bundles contain one directory per supported PostgreSQL major version, such as `pg18`.

```bash
curl -LO https://github.com/nyurik/postile/releases/download/vX.Y.Z/postile-vX.Y.Z-linux-x86_64.tar.gz
mkdir postile-release
tar -xzf postile-vX.Y.Z-linux-x86_64.tar.gz -C postile-release
cd postile-release/pg18
```

Install the unpacked package tree into `/`. The archive is laid out with the PostgreSQL extension files already under the paths expected by `pg_config`, such as `$(pg_config --pkglibdir)/postile.so` and `$(pg_config --sharedir)/extension/postile.control`.

```bash
sudo cp -a . /

test -f "$(pg_config --pkglibdir)/postile.so"
test -f "$(pg_config --sharedir)/extension/postile.control"
ls "$(pg_config --sharedir)"/extension/postile--*.sql
```

If you downloaded a single PostgreSQL-version archive such as `postile-pg18.tar.gz`, install it directly:

```bash
sudo tar -xzf postile-pg18.tar.gz -C /

test -f "$(pg_config --pkglibdir)/postile.so"
test -f "$(pg_config --sharedir)/extension/postile.control"
ls "$(pg_config --sharedir)"/extension/postile--*.sql
```

After the files are installed, enable the extension in each database that needs it:

```sql
CREATE EXTENSION postile;
```

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
