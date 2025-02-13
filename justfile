#!/usr/bin/env just --justfile

@_default:
    just --list

# Clean all build artifacts
clean:
    cargo clean

# Update dependencies, including breaking changes
update:
    cargo +nightly -Z unstable-options update --breaking
    cargo update

# (Re-)initializing PGRX with all available PostgreSQL versions
init: cargo-pgrx
    cargo pgrx init

# Package extension
package: cargo-pgrx
    cargo pgrx package

# Use psql to connect to a database
connect: cargo-pgrx
    cargo pgrx connect

# Run cargo clippy
clippy:
    cargo clippy -- -D warnings
    cargo clippy --workspace --all-targets -- -D warnings

# Test code formatting
test-fmt:
    cargo fmt --all -- --check

# Run cargo fmt
fmt:
    cargo +nightly fmt -- --config imports_granularity=Module,group_imports=StdExternalCrate

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
    LOCAL_VERSION="$(grep '^version =' Cargo.toml | sed -E 's/version = "([^"]*)".*/\1/')"
    echo "Detected crate version:  $LOCAL_VERSION"
    CRATE_NAME="$(grep '^name =' Cargo.toml | head -1 | sed -E 's/name = "(.*)"/\1/')"
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
