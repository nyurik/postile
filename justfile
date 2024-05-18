#!/usr/bin/env just --justfile

@_default:
    just --list --unsorted

# Clean all build artifacts
clean:
    cargo clean

# Build the project
build:
    cargo build --workspace --all-targets --bins --tests --lib --benches --examples

# (Re-)initializing PGRX with all available PostgreSQL versions
init: cargo-pgrx
    cargo pgrx init

# Package extension
package: cargo-pgrx
    cargo pgrx package

# Use psql to connect to a database
connect: cargo-pgrx
    cargo pgrx connect

# Run cargo fmt
fmt:
    cargo +nightly fmt -- --config imports_granularity=Module,group_imports=StdExternalCrate

# Test code formatting
test-fmt:
    cargo fmt --all -- --check

# Run cargo clippy
clippy:
    cargo clippy -- -D warnings
    cargo clippy --workspace --all-targets --bins --tests --lib --benches --examples -- -D warnings

# Build and open code documentation
docs:
    cargo doc --no-deps --open

# Run benchmarks
bench:
    cargo bench
    open target/criterion/report/index.html

# Quick compile
check:
    RUSTFLAGS='-D warnings' cargo check

# Run tests
test: cargo-pgrx
    cargo pgrx test

# Test documentation
test-doc:
    RUSTDOCFLAGS="-D warnings" cargo doc --no-deps

# Rint rustc and cargo versions
rust-info:
    rustc --version
    cargo --version

# Run all tests as expected by CI
ci-test: rust-info test-fmt check test test-doc clippy

# Run minimal subset of tests to ensure compatibility with MSRV
ci-test-msrv: rust-info check test

# Run some checks before pushing to remote GIT
[private]
pre-push: rust-info test-fmt clippy

# Check if cargo-pgrx is installed, and install it if needed
[private]
cargo-pgrx: (cargo-install "cargo-pgrx")

# Check if a certain Cargo command is installed, and install it if needed
[private]
cargo-install $COMMAND $INSTALL_CMD="" *ARGS="":
    #!/usr/bin/env sh
    set -eu
    if ! command -v $COMMAND > /dev/null; then
        if ! command -v cargo-binstall > /dev/null; then
            echo "$COMMAND could not be found. Installing it with    cargo install ${INSTALL_CMD:-$COMMAND} {{ARGS}}"
            cargo install ${INSTALL_CMD:-$COMMAND} {{ARGS}}
        else
            echo "$COMMAND could not be found. Installing it with    cargo binstall ${INSTALL_CMD:-$COMMAND} {{ARGS}}"
            cargo binstall ${INSTALL_CMD:-$COMMAND} {{ARGS}}
        fi
    fi
