#!/usr/bin/env just --justfile

CRATE_NAME := "postile"

@_default:
    just --list

# Clean all build artifacts
clean:
    cargo clean

# Update all dependencies, including breaking changes. Requires nightly toolchain (install with `rustup install nightly`)
update:
    cargo pgrx upgrade
    cargo +nightly -Z unstable-options update --breaking
    cargo update
    cargo pgrx upgrade

# (Re-)initializing PGRX with all available PostgreSQL versions
init: cargo-pgrx
    cargo pgrx init

# Package extension
package: cargo-pgrx
    cargo pgrx package

# Use psql to connect to a database
connect: cargo-pgrx
    cargo pgrx connect

# Find unused dependencies. Install it with `cargo install cargo-udeps`
udeps:
    cargo +nightly udeps --all-targets --workspace --all-features

# Check semver compatibility with prior published version. Install it with `cargo install cargo-semver-checks`
semver *ARGS:
    cargo semver-checks {{ARGS}}

# Find the minimum supported Rust version (MSRV) using cargo-msrv extension, and update Cargo.toml
msrv:
    cargo msrv find --write-msrv --ignore-lockfile

# Get the minimum supported Rust version (MSRV) for the crate
get-msrv: (get-crate-field "rust_version")

# Get any package's field from the metadata
get-crate-field field package=CRATE_NAME:
    cargo metadata --format-version 1 | jq -r '.packages | map(select(.name == "{{package}}")) | first | .{{field}}'

build:
    cargo build --workspace --all-targets

# Run cargo clippy to lint the code
clippy:
    cargo clippy --workspace --all-targets -- -D warnings

# Test code formatting
test-fmt:
    cargo fmt --all -- --check

# Reformat all code `cargo fmt`. If nightly is available, use it for better results
fmt:
    #!/usr/bin/env bash
    set -euo pipefail
    if command -v cargo +nightly &> /dev/null; then
        echo 'Reformatting Rust code using nightly Rust fmt to sort imports'
        cargo +nightly fmt --all -- --config imports_granularity=Module,group_imports=StdExternalCrate
    else
        echo 'Reformatting Rust with the stable cargo fmt.  Install nightly with `rustup install nightly` for better results'
        cargo fmt --all
    fi

# Build and open code documentation
docs:
    cargo doc --no-deps --open

# Run benchmarks
bench:
    cargo bench
    open target/criterion/report/index.html

# Quick compile without building a binary
check:
    RUSTFLAGS='-D warnings' cargo check --workspace --all-targets

# Generate code coverage report
coverage *ARGS="--no-clean --open":
    cargo llvm-cov --workspace --all-targets --include-build-script {{ARGS}}

# Generate code coverage report to upload to codecov.io
ci-coverage: && \
            (coverage '--codecov --output-path target/llvm-cov/codecov.info')
    # ATTENTION: the full file path above is used in the CI workflow
    mkdir -p target/llvm-cov

# Run all tests
test: cargo-pgrx
    cargo pgrx test

# Test documentation
test-doc:
    RUSTDOCFLAGS="-D warnings" cargo test --doc
    RUSTDOCFLAGS="-D warnings" cargo doc --no-deps

# Print Rust version information
@rust-info:
    rustc --version
    cargo --version

# Run all tests as expected by CI
ci-test: rust-info test-fmt clippy check test test-doc

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

# Verify that the current version of the crate is not the same as the one published on crates.io
check-if-published:
    #!/usr/bin/env bash
    LOCAL_VERSION="$({{just_executable()}} get-crate-field version)"
    echo "Detected crate version:  $LOCAL_VERSION"
    CRATE_NAME="$({{just_executable()}} get-crate-field name)"
    echo "Detected crate name:     $CRATE_NAME"
    PUBLISHED_VERSION="$(cargo search ${CRATE_NAME} | grep "^${CRATE_NAME} =" | sed -E 's/.* = "(.*)".*/\1/')"
    echo "Published crate version: $PUBLISHED_VERSION"
    if [ "$LOCAL_VERSION" = "$PUBLISHED_VERSION" ]; then
        echo "ERROR: The current crate version has already been published."
        exit 1
    else
        echo "The current crate version has not yet been published."
    fi

# Print current PGRX version
@print-pgrx-version: (assert "jq")
    cargo metadata --format-version 1 | jq -r '.packages | map(select(.name == "postile")) | first | .dependencies | map(select(.name == "pgrx")) | first | .req | ltrimstr("=")'

# Ensure that a certain command is available
[private]
assert $COMMAND:
    @if ! type "{{COMMAND}}" > /dev/null; then \
        echo "Command '{{COMMAND}}' could not be found. Please make sure it has been installed on your computer." ;\
        exit 1 ;\
    fi
