#!/usr/bin/env just --justfile

main_crate := 'postile'
# How to call the current just executable. Note that just_executable() may have `\` in Windows paths, so we need to quote it.
just := quote(just_executable())
# cargo-binstall needs a workaround due to caching when used in CI
binstall_args := if env('CI', '') != '' {'--no-confirm --no-track --disable-telemetry'} else {''}
# location of the coverage output, used by CI
coverage_lcov := 'target/llvm-cov/lcov.info'
# Default PG version to use for commands that require a PG version
default_pg_ver := 'pg18'
# PostgreSQL versions supported by the pgrx version in Cargo.toml
supported_pg_versions := 'pg13 pg14 pg15 pg16 pg17 pg18'

# If running in CI, treat warnings as errors for cargo commands that compile code.
# Use `CI=true just ci-test` to run the same tests as in GitHub CI.
# Use `just env-info` to see the current CI command flags.
ci_mode := if env('CI', '') != '' {'1'} else {''}
cargo_deny_warnings := if ci_mode == '1' {'--config ' + quote('build.rustflags=["-Dwarnings"]')} else {''}
clippy_deny_warnings := if ci_mode == '1' {'-- -D warnings'} else {''}
export RUST_BACKTRACE := env('RUST_BACKTRACE', if ci_mode == '1' {'1'} else {'0'})

@_default:
    {{just}} --list

# Run benchmarks
bench:
    cargo bench
    open target/criterion/report/index.html

# Build the project
build:
    cargo {{cargo_deny_warnings}} build --workspace --all-targets

# Quick compile without building a binary for a PG version
check pg_ver=default_pg_ver:
    cargo {{cargo_deny_warnings}} check --workspace --all-targets --no-default-features --features {{pg_ver}}

# Generate LCOV coverage report for CI to upload to codecov.io
ci-coverage pg_ver=default_pg_ver: env-info
    rm -rf {{quote(parent_directory(coverage_lcov))}}
    mkdir -p {{quote(parent_directory(coverage_lcov))}}
    {{just}} _coverage {{pg_ver}} --lcov --output-path {{quote(coverage_lcov)}}

# Run formatting, linting, and compile checks as expected by CI
ci-lint pg_ver=default_pg_ver: env-info test-fmt (clippy pg_ver) (check pg_ver)

# Clean all build artifacts
clean:
    cargo clean

# Run cargo clippy for a PG version
clippy pg_ver=default_pg_ver:
    cargo clippy --workspace --all-targets --no-default-features --features {{pg_ver}} {{clippy_deny_warnings}}

# Use psql to connect to a database
connect: install-pgrx
    cargo pgrx connect

# Generate and open the HTML coverage report
coverage:  (_coverage default_pg_ver '--open')

# Clean, collect, and aggregate coverage using the requested report arguments
_coverage pg_ver=default_pg_ver *report_args:  (cargo-install 'cargo-llvm-cov')
    cargo {{cargo_deny_warnings}} llvm-cov clean --workspace
    cargo {{cargo_deny_warnings}} llvm-cov --no-report --workspace --all-targets --no-default-features --features {{pg_ver}}
    cargo {{cargo_deny_warnings}} llvm-cov report --include-build-script {{report_args}}

# Print environment info
env-info:
    @echo "Running for '{{main_crate}}' crate {{if ci_mode == '1' {'in CI mode'} else {'in dev mode'} }} on {{os()}} / {{arch()}}"
    @echo "PWD {{justfile_directory()}}"
    {{just}} --version
    rustc --version
    cargo --version
    rustup --version
    @echo "cargo_deny_warnings='{{cargo_deny_warnings}}'"
    @echo "clippy_deny_warnings='{{clippy_deny_warnings}}'"
    @echo "RUST_BACKTRACE='$RUST_BACKTRACE'"

# Reformat all code `cargo fmt`. If nightly is available, use it for better results
fmt:
    #!/usr/bin/env bash
    set -euo pipefail
    if (rustup toolchain list | grep nightly && rustup component list --toolchain nightly | grep rustfmt) &> /dev/null; then
        echo 'Reformatting Rust code using nightly Rust fmt to sort imports'
        cargo +nightly fmt --all -- --config imports_granularity=Module,group_imports=StdExternalCrate
    else
        echo 'Reformatting Rust with the stable cargo fmt.  Install nightly with `rustup install nightly` for better results'
        cargo fmt --all
    fi

# Reformat all Cargo.toml files using cargo-sort
fmt-toml *args:  (cargo-install 'cargo-sort')
    cargo sort --workspace --grouped {{args}}

# Get a package field from the metadata
get-crate-field field package=main_crate:  (assert-cmd 'jq')
    @cargo metadata --no-deps --format-version 1 | jq -e -r '.packages | map(select(.name == "{{package}}")) | first | .{{field}} // error("Field \"{{field}}\" is missing in Cargo.toml for package {{package}}")'

# Print current PGRX version
get-pgrx-version package=main_crate:  (assert-cmd 'jq')
    @cargo metadata --no-deps --format-version 1 | jq -e -r '.packages | map(select(.name == "{{package}}")) | first | .dependencies | map(select(.name == "pgrx")) | first // error("Value \"dependencies/pgrx\" is missing in Cargo.toml for package {{package}}") | .req | ltrimstr("=")'

# (Re-)initializing PGRX with all available PostgreSQL versions
init: install-pgrx
    cargo pgrx init

install-pgrx:
    #!/usr/bin/env bash
    set -euo pipefail
    version="$({{just}} get-pgrx-version)"
    if command -v cargo-pgrx >/dev/null && cargo pgrx --version | grep -q " ${version}$"; then
        echo "cargo-pgrx ${version} already installed, skipping"
    else
        echo "Installing cargo-pgrx ${version}..."
        set -x
        cargo install cargo-pgrx --locked --version "${version}" --force
        { set +x; } 2>/dev/null
    fi

# Initialize pgrx for a PG version, optionally using an existing pg_config path
init-pg pg_ver=default_pg_ver pg_config='': install-pgrx
    #!/usr/bin/env bash
    set -euo pipefail
    if [[ ! "{{pg_ver}}" =~ ^pg[0-9]+$ ]]; then
        >&2 echo "ERROR: Invalid PG version format '{{pg_ver}}'. Expected 'pgXX' (for example, pg18)."
        exit 1
    fi
    if cargo pgrx info pg-config {{pg_ver}} &>/dev/null; then
        echo "pgrx already initialized for {{pg_ver}}"
    elif [ -n "{{pg_config}}" ]; then
        echo "Initializing pgrx for {{pg_ver}} with {{pg_config}}"
        cargo pgrx init --{{pg_ver}} {{quote(pg_config)}}
    else
        echo "Initializing pgrx for {{pg_ver}} by downloading PostgreSQL"
        cargo pgrx init --{{pg_ver}}=download
    fi

# Package extension for a given PG version and create a tar.gz (e.g., `just package pg18`)
package pg_ver=default_pg_ver:
    #!/usr/bin/env bash
    set -euo pipefail
    if [[ ! "{{pg_ver}}" =~ ^pg[0-9]+$ ]]; then
        >&2 echo "ERROR: Invalid PG version format '{{pg_ver}}'. Expected format is 'pgXX' where XX is the major version number (e.g., pg18)."
        exit 1
    fi
    pg_config_path="$(cargo pgrx info pg-config {{pg_ver}})"
    echo "Packaging {{pg_ver}} with ${pg_config_path}"
    cargo pgrx package --features {{pg_ver}} --no-default-features --pg-config "${pg_config_path}"
    pkg_dir="target/release/postile-{{pg_ver}}"
    if [ ! -d "$pkg_dir" ]; then
        echo "ERROR: Package directory not found at $pkg_dir"
        ls -la target/release/ | grep postile || true
        exit 1
    fi
    tar -czf "target/release/postile-{{pg_ver}}.tar.gz" -C "$pkg_dir" .
    echo "Package created: target/release/postile-{{pg_ver}}.tar.gz"

# Print the default PostGIS image tag for a PG version, or nothing if unsupported
maybe-postgis-tag pg_ver=default_pg_ver:
    #!/usr/bin/env bash
    set -euo pipefail
    case "{{pg_ver}}" in
        pg14) echo "14-3.5" ;;
        pg15) echo "15-3.5" ;;
        pg16) echo "16-3.5" ;;
        pg17) echo "17-3.5" ;;
        pg18) echo "18-3.6" ;;
    esac

# Print the default PostGIS image tag for a supported PG version
get-postgis-tag pg_ver=default_pg_ver:
    #!/usr/bin/env bash
    set -euo pipefail
    tag="$({{just}} maybe-postgis-tag {{quote(pg_ver)}})"
    if [ -z "$tag" ]; then
        >&2 echo "ERROR: No default PostGIS image tag is configured for {{pg_ver}}."
        exit 1
    fi
    echo "$tag"

# Build the local PostGIS image with Postile installed
postgis-image pg_ver=default_pg_ver postgis_tag='' image_tag='':
    #!/usr/bin/env bash
    set -euo pipefail
    pg_ver="{{pg_ver}}"
    postgis_tag="{{postgis_tag}}"
    image_tag="{{image_tag}}"
    if [[ ! "$pg_ver" =~ ^pg[0-9]+$ ]]; then
        >&2 echo "ERROR: Invalid PG version format '$pg_ver'. Expected format is 'pgXX' where XX is the major version number (e.g., pg18)."
        exit 1
    fi
    pg_major="${pg_ver#pg}"
    if [ -z "$postgis_tag" ]; then
        postgis_tag="$({{just}} get-postgis-tag "$pg_ver")"
    fi
    if [ -z "$image_tag" ]; then
        image_tag="postgis-postile:${postgis_tag}"
    fi
    {{just}} package "$pg_ver"
    docker build \
        --file docker/postgis/Dockerfile \
        --build-arg "BASE_IMAGE=postgis/postgis:${postgis_tag}" \
        --build-arg "PG_MAJOR=${pg_major}" \
        --build-arg "POSTILE_VERSION=$({{just}} get-crate-field version)" \
        --build-arg "VCS_REF=$(git rev-parse --short HEAD 2>/dev/null || echo local)" \
        --tag "$image_tag" \
        .

# Build and smoke-test the local PostGIS image
test-postgis-image pg_ver=default_pg_ver postgis_tag='' image_tag='':
    #!/usr/bin/env bash
    set -euo pipefail
    pg_ver="{{pg_ver}}"
    postgis_tag="{{postgis_tag}}"
    if [ -z "$postgis_tag" ]; then
        postgis_tag="$({{just}} get-postgis-tag "$pg_ver")"
    fi
    image_tag="{{image_tag}}"
    if [ -z "$image_tag" ]; then
        image_tag="postgis-postile:${postgis_tag}"
    fi
    {{just}} postgis-image "$pg_ver" "$postgis_tag" "$image_tag"
    bash docker/postgis/smoke-test.sh "$image_tag"

# Bundle all per-PG package directories for one platform into a release archive
bundle-platform platform version='':
    #!/usr/bin/env bash
    set -euo pipefail
    version="{{version}}"
    if [ -z "$version" ]; then
        version="$({{just}} get-crate-field version)"
    fi
    bundle_name="postile-v${version}-{{platform}}"
    bundle_dir="target/release/${bundle_name}"
    rm -rf "$bundle_dir"
    mkdir -p "$bundle_dir"
    for pg_ver in {{supported_pg_versions}}; do
        pkg_dir="target/release/postile-${pg_ver}"
        if [ ! -d "$pkg_dir" ]; then
            >&2 echo "ERROR: Package directory not found at $pkg_dir"
            exit 1
        fi
        mkdir -p "$bundle_dir/$pg_ver"
        cp -a "$pkg_dir/." "$bundle_dir/$pg_ver/"
    done
    if [[ "{{platform}}" == windows-* ]]; then
        archive="target/release/${bundle_name}.zip"
        rm -f "$archive"
        python -c 'import pathlib, sys, zipfile; archive = pathlib.Path(sys.argv[1]); root = pathlib.Path(sys.argv[2]); zf = zipfile.ZipFile(archive, "w", zipfile.ZIP_DEFLATED); [zf.write(path, path.relative_to(root)) for path in sorted(root.rglob("*")) if path.is_file()]; zf.close()' "$archive" "$bundle_dir"
    else
        archive="target/release/${bundle_name}.tar.gz"
        rm -f "$archive"
        tar -czf "$archive" -C "$bundle_dir" .
    fi
    echo "Bundle created: $archive"

# Test for a specific PG version (e.g., `just test pg18`)
test pg_ver=default_pg_ver:
    cargo pgrx test {{pg_ver}}

# Test code formatting
test-fmt: && (fmt-toml '--check' '--check-format')
    cargo fmt --all -- --check

# Find unused dependencies. Uses `cargo-udeps`
udeps:  (cargo-install 'cargo-udeps')
    cargo +nightly udeps --workspace --all-targets

# Update all dependencies, including breaking changes. Requires nightly toolchain (install with `rustup install nightly`)
update:
    cargo pgrx upgrade
    cargo +nightly -Z unstable-options update --breaking
    cargo update
    cargo pgrx upgrade

# Ensure that a certain command is available
[private]
assert-cmd command:
    @if ! type {{command}} > /dev/null; then \
        echo "Command '{{command}}' could not be found. Please make sure it has been installed on your computer." ;\
        exit 1 ;\
    fi

# Make sure the git repo has no uncommitted changes
[private]
assert-git-is-clean:
    @if [ -n "$(git status --untracked-files --porcelain)" ]; then \
        >&2 echo "ERROR: git repo is no longer clean. Make sure compilation and tests artifacts are in the .gitignore, and no repo files are modified." ;\
        >&2 echo "######### git status ##########" ;\
        git status ;\
        git --no-pager diff ;\
        exit 1 ;\
    fi

# Check if a certain Cargo command is installed, and install it if needed
[private]
cargo-install $COMMAND $INSTALL_CMD='' *args='':
    #!/usr/bin/env bash
    set -euo pipefail
    if ! command -v $COMMAND > /dev/null; then
        echo "$COMMAND could not be found. Installing..."
        if ! command -v cargo-binstall > /dev/null; then
            set -x
            cargo install ${INSTALL_CMD:-$COMMAND} --locked {{args}}
            { set +x; } 2>/dev/null
        else
            set -x
            cargo binstall ${INSTALL_CMD:-$COMMAND} {{binstall_args}} --locked {{args}}
            { set +x; } 2>/dev/null
        fi
    fi
