"""Bazel build rules for M31A releases and packaging.

NOTE: These Bazel rules wrap the host Cargo and Rust toolchains (`cargo build`, `cargo test`).
They run in non-hermetic local execution mode (`local = 1`, `no-sandbox = 1`) delegating compilation
and testing directly to the host's installed `cargo` environment rather than hermetic rules_rust toolchains.

Defines rules for compiling the M31A standalone binary via Cargo, generating release archives (tar.gz),
extracting protocol schema bundles, executing genuine E2E benchmarks, and running workspace test suites.
"""

def _m31a_binary_impl(ctx):
    """Build the genuine M31A standalone executable using cargo."""
    out = ctx.actions.declare_file(ctx.label.name)
    version = ctx.attr.version
    package = ctx.attr.package
    bin_name = ctx.attr.bin_name or ctx.label.name

    ctx.actions.run_shell(
        outputs = [out],
        inputs = ctx.files.srcs,
        command = """
set -euo pipefail

# Locate workspace root
if [ -f "Cargo.toml" ]; then
    WORKSPACE_DIR="."
elif [ -n "${BUILD_WORKSPACE_DIRECTORY:-}" ] && [ -f "${BUILD_WORKSPACE_DIRECTORY}/Cargo.toml" ]; then
    WORKSPACE_DIR="${BUILD_WORKSPACE_DIRECTORY}"
elif [ -f "../Cargo.toml" ]; then
    WORKSPACE_DIR=".."
else
    WORKSPACE_DIR="."
fi

CARGO_CMD="${CARGO:-cargo}"

echo "Building M31A package '%s' (bin: '%s') with cargo..."
"$CARGO_CMD" build --release -p "%s" --bin "%s" --manifest-path "${WORKSPACE_DIR}/Cargo.toml"

# Locate compiled binary
CANDIDATE_PATHS=(
    "${WORKSPACE_DIR}/target/release/%s"
    "target/release/%s"
    "${WORKSPACE_DIR}/target/debug/%s"
    "target/debug/%s"
)

FOUND_BIN=""
for path in "${CANDIDATE_PATHS[@]}"; do
    if [ -f "$path" ]; then
        FOUND_BIN="$path"
        break
    fi
done

if [ -z "$FOUND_BIN" ]; then
    echo "ERROR: Compiled cargo binary not found for package %s (bin: %s)" >&2
    exit 1
fi

cp "$FOUND_BIN" "%s"
chmod +x "%s"
echo "M31A binary ready at %s"
""" % (package, bin_name, package, bin_name, bin_name, bin_name, bin_name, bin_name, package, bin_name, out.path, out.path, out.path),
        execution_requirements = {
            "no-sandbox": "1",
            "local": "1",
        },
        use_default_shell_env = True,
    )
    return DefaultInfo(files = depset([out]), executable = out)

m31a_binary = rule(
    implementation = _m31a_binary_impl,
    executable = True,
    attrs = {
        "version": attr.string(default = "0.1.13"),
        "package": attr.string(default = "m31a-cli"),
        "bin_name": attr.string(default = "m31a"),
        "srcs": attr.label_list(allow_files = True),
    },
)

def _m31a_release_archive_impl(ctx):
    """Create genuine release archives containing binary, license, default config, docs, metadata, and sha256 checksums."""
    version = ctx.attr.version
    archive_file = ctx.actions.declare_file("m31a_release_archive/m31a-" + version + ".tar.gz")

    binary_file = ctx.file.binary
    inputs = [binary_file] if binary_file else []

    ctx.actions.run_shell(
        outputs = [archive_file],
        inputs = inputs,
        command = """
set -euo pipefail

# Locate workspace root
if [ -f "Cargo.toml" ]; then
    WORKSPACE_DIR="."
elif [ -n "${BUILD_WORKSPACE_DIRECTORY:-}" ] && [ -f "${BUILD_WORKSPACE_DIRECTORY}/Cargo.toml" ]; then
    WORKSPACE_DIR="${BUILD_WORKSPACE_DIRECTORY}"
elif [ -f "../Cargo.toml" ]; then
    WORKSPACE_DIR=".."
else
    WORKSPACE_DIR="."
fi

# Verify mandatory assets - fail closed if any are missing
BIN_SRC="%s"
if [ ! -f "$BIN_SRC" ]; then
    echo "ERROR: Built binary not found at '$BIN_SRC'" >&2
    exit 1
fi

LICENSE_SRC="${WORKSPACE_DIR}/LICENSE"
if [ ! -f "$LICENSE_SRC" ]; then
    echo "ERROR: Required release asset 'LICENSE' missing at '$LICENSE_SRC'" >&2
    exit 1
fi

README_SRC="${WORKSPACE_DIR}/README.md"
if [ ! -f "$README_SRC" ]; then
    echo "ERROR: Required release asset 'README.md' missing at '$README_SRC'" >&2
    exit 1
fi

CONFIG_SRC="${WORKSPACE_DIR}/config.example.toml"
if [ ! -f "$CONFIG_SRC" ]; then
    echo "ERROR: Required release asset 'config.example.toml' missing at '$CONFIG_SRC'" >&2
    exit 1
fi

# Staging directory for package creation
STAGING_DIR="$(mktemp -d -t m31a_pkg_XXXXXX)"
trap 'rm -rf "$STAGING_DIR"' EXIT

STAGE_ROOT="$STAGING_DIR/m31a-%s"
mkdir -p "$STAGE_ROOT"

cp "$BIN_SRC" "$STAGE_ROOT/m31a"
chmod +x "$STAGE_ROOT/m31a"
cp "$LICENSE_SRC" "$STAGE_ROOT/LICENSE"
cp "$README_SRC" "$STAGE_ROOT/README.md"
cp "$CONFIG_SRC" "$STAGE_ROOT/config.example.toml"

# Generate version metadata
TIMESTAMP=$(date -u +"%%Y-%%m-%%dT%%H:%%M:%%SZ")
COMMIT=$(git -C "$WORKSPACE_DIR" rev-parse HEAD 2>/dev/null || echo "unknown")
PLATFORM=$(uname -s)-$(uname -m)

cat << EOF > "$STAGE_ROOT/version.json"
{
  "name": "m31a",
  "version": "%s",
  "build_timestamp": "$TIMESTAMP",
  "git_commit": "$COMMIT",
  "target_platform": "$PLATFORM"
}
EOF

# Compute real SHA-256 checksums of all packaged assets
(
    cd "$STAGE_ROOT"
    sha256sum m31a LICENSE README.md config.example.toml version.json > SHA256SUMS
)

mkdir -p "$(dirname "%s")"
tar -czf "%s" -C "$STAGING_DIR" "m31a-%s"

echo "Release archive successfully generated at %s"
""" % (
            binary_file.path if binary_file else "",
            version,
            version,
            archive_file.path,
            archive_file.path,
            version,
            archive_file.path,
        ),
        execution_requirements = {
            "no-sandbox": "1",
            "local": "1",
        },
        use_default_shell_env = True,
    )
    return DefaultInfo(files = depset([archive_file]))

m31a_release_archive = rule(
    implementation = _m31a_release_archive_impl,
    attrs = {
        "binary": attr.label(
            mandatory = False,
            allow_single_file = True,
            executable = True,
            cfg = "target",
        ),
        "platforms": attr.string_list(mandatory = False),
        "version": attr.string(default = "0.1.13"),
    },
)

def _m31a_schema_bundle_impl(ctx):
    """Extract canonical JSON schema bundles from protocol definitions."""
    output_dir = ctx.attr.output_dir
    schema_file = ctx.actions.declare_file(output_dir + "/schema.json")

    ctx.actions.run_shell(
        outputs = [schema_file],
        inputs = ctx.files.proto_sources,
        command = """
set -euo pipefail

# Locate workspace root
if [ -f "Cargo.toml" ]; then
    WORKSPACE_DIR="."
elif [ -n "${BUILD_WORKSPACE_DIRECTORY:-}" ] && [ -f "${BUILD_WORKSPACE_DIRECTORY}/Cargo.toml" ]; then
    WORKSPACE_DIR="${BUILD_WORKSPACE_DIRECTORY}"
elif [ -f "../Cargo.toml" ]; then
    WORKSPACE_DIR=".."
else
    WORKSPACE_DIR="."
fi

SCRIPT="${WORKSPACE_DIR}/scripts/extract_protocol_schema.py"
if [ ! -f "$SCRIPT" ]; then
    echo "ERROR: Protocol schema extraction script missing at $SCRIPT" >&2
    exit 1
fi

python3 "$SCRIPT" "%s"
""" % (schema_file.path),
        execution_requirements = {
            "no-sandbox": "1",
            "local": "1",
        },
        use_default_shell_env = True,
    )
    return DefaultInfo(files = depset([schema_file]))

m31a_schema_bundle = rule(
    implementation = _m31a_schema_bundle_impl,
    attrs = {
        "proto_sources": attr.label_list(allow_files = True),
        "output_dir": attr.string(default = "schemas/app_server"),
    },
)

def _m31a_e2e_benchmark_impl(ctx):
    """Run E2E benchmarks with genuine statistical latency and throughput analysis."""
    runner = ctx.actions.declare_file(ctx.label.name + "_runner.sh")
    iterations = ctx.attr.iterations
    bench_name = ctx.label.name

    script = """#!/bin/sh
set -eu
TARGET_DIR="${BUILD_WORKING_DIRECTORY:-.}"
WORKSPACE_DIR="${BUILD_WORKSPACE_DIRECTORY:-.}"

SCRIPT="${WORKSPACE_DIR}/scripts/benchmark_runner.py"
if [ ! -f "$SCRIPT" ]; then
    if [ -f "scripts/benchmark_runner.py" ]; then
        SCRIPT="scripts/benchmark_runner.py"
    else
        echo "ERROR: Benchmark harness script not found at $SCRIPT" >&2
        exit 1
    fi
fi

python3 "$SCRIPT" --name "%s" --iterations %d --output "${TARGET_DIR}/benchmark_results.json"
""" % (bench_name, iterations)

    ctx.actions.write(
        output = runner,
        content = script,
        is_executable = True,
    )
    return DefaultInfo(files = depset([runner]), executable = runner)

m31a_e2e_benchmark = rule(
    implementation = _m31a_e2e_benchmark_impl,
    executable = True,
    attrs = {
        "test_script": attr.label(
            mandatory = False,
            allow_single_file = True,
            executable = True,
            cfg = "exec",
        ),
        "iterations": attr.int(default = 5),
    },
)

def _m31a_test_impl(ctx):
    """Run cross-platform workspace verification tests and propagate actual test results."""
    is_windows = "windows" in ctx.var.get("TARGET_CPU", "")
    ext = ".bat" if is_windows else ".sh"
    out = ctx.actions.declare_file(ctx.label.name + ext)
    package = ctx.attr.package

    if is_windows:
        if package:
            cmd = "cargo test -p %s" % package
        else:
            cmd = "cargo test --workspace"
        content = "@echo off\r\n" + cmd + "\r\nif %%ERRORLEVEL%% NEQ 0 exit /b %%ERRORLEVEL%%\r\nexit /b 0\r\n"
    else:
        if package:
            cmd = 'cargo test -p "%s"' % package
        else:
            cmd = "cargo test --workspace"
        content = """#!/bin/sh
set -e

# Robust discovery of workspace directory containing Cargo.toml
WORKSPACE_DIR=""
if [ -n "${BUILD_WORKSPACE_DIRECTORY:-}" ] && [ -f "${BUILD_WORKSPACE_DIRECTORY}/Cargo.toml" ]; then
    WORKSPACE_DIR="${BUILD_WORKSPACE_DIRECTORY}"
elif [ -f "Cargo.toml" ]; then
    WORKSPACE_DIR="$(dirname "$(readlink -f "Cargo.toml")")"
fi

if [ -z "$WORKSPACE_DIR" ]; then
    DIR="$PWD"
    while [ "$DIR" != "/" ]; do
        if [ -f "$DIR/Cargo.toml" ]; then
            WORKSPACE_DIR="$(dirname "$(readlink -f "$DIR/Cargo.toml")")"
            break
        fi
        DIR="$(dirname "$DIR")"
    done
fi

if [ -z "$WORKSPACE_DIR" ]; then
    SCRIPT_PATH="$(realpath "$0" 2>/dev/null || readlink -f "$0" 2>/dev/null || echo "$0")"
    DIR="$(dirname "$SCRIPT_PATH")"
    while [ "$DIR" != "/" ]; do
        if [ -f "$DIR/Cargo.toml" ]; then
            WORKSPACE_DIR="$(dirname "$(readlink -f "$DIR/Cargo.toml")")"
            break
        fi
        DIR="$(dirname "$DIR")"
    done
fi

if [ -z "$WORKSPACE_DIR" ]; then
    echo "ERROR: Could not locate workspace Cargo.toml" >&2
    exit 1
fi

export HOME="${HOME:-/tmp}"
export GIT_AUTHOR_NAME="${GIT_AUTHOR_NAME:-M31A Test}"
export GIT_AUTHOR_EMAIL="${GIT_AUTHOR_EMAIL:-test@m31a.local}"
export GIT_COMMITTER_NAME="${GIT_COMMITTER_NAME:-M31A Test}"
export GIT_COMMITTER_EMAIL="${GIT_COMMITTER_EMAIL:-test@m31a.local}"
echo "Running M31A verification tests in $WORKSPACE_DIR..."
cd "$WORKSPACE_DIR"
%s
""" % cmd

    ctx.actions.write(
        output = out,
        content = content,
        is_executable = True,
    )
    return DefaultInfo(executable = out)

m31a_test = rule(
    implementation = _m31a_test_impl,
    test = True,
    attrs = {
        "package": attr.string(default = ""),
    },
)
