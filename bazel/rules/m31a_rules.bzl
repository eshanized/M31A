"""Bazel build rules for M31A releases and packaging.

Defines custom rules for compiling cross-platform release archives (tar.gz, zip),
runtime packages, schema bundles, and benchmark targets.
"""

load("@rules_rust//rust:defs.bzl", "rust_binary")
load("@rules_pkg//:pkg.bzl", "pkg_tar", "pkg_zip")

def _m31a_release_archive_impl(ctx):
    """Create release archives for multiple platforms."""
    binary = ctx.attr.binary
    platforms = ctx.attr.platforms

    outputs = []
    for platform in platforms:
        platform_name = platform.replace("@m31a//platforms:", "").replace(":", "_")
        archive_name = "m31a-{}-{}".format(ctx.attr.version, platform_name)

        if "windows" in platform_name:
            # Windows zip
            pkg = pkg_zip(
                name = archive_name + "_zip",
                srcs = [binary],
                strip_prefix = "",
                output_dir = archive_name,
                extension = ".zip",
            )
        else:
            # Unix tar.gz
            pkg = pkg_tar(
                name = archive_name + "_tar",
                srcs = [binary],
                strip_prefix = "",
                output_dir = archive_name,
                compression = "gzip",
            )
        outputs.append(pkg)

    return DefaultInfo(files = depset(outputs))

m31a_release_archive = rule(
    implementation = _m31a_release_archive_impl,
    attrs = {
        "binary": attr.label(
            mandatory = True,
            allow_single_file = True,
            executable = True,
            cfg = "target",
        ),
        "platforms": attr.string_list(mandatory = True),
        "version": attr.string(default = "0.1.13"),
    },
)


def _m31a_schema_bundle_impl(ctx):
    """Generate JSON schema bundles from protocol definitions."""
    proto_sources = ctx.files.proto_sources
    output_dir = ctx.attr.output_dir

    # Use protoc to generate JSON schemas
    # This is a placeholder - actual implementation would use protoson or similar
    ctx.actions.run(
        inputs = proto_sources,
        outputs = [ctx.actions.declare_directory(output_dir)],
        executable = "python3",
        arguments = [
            "-c",
            "import json, os, sys; "
            "os.makedirs(sys.argv[1], exist_ok=True); "
            "print('Schema bundle generation placeholder')",
            output_dir,
        ],
        progress_message = "Generating schema bundle for %s" % output_dir,
    )

    return DefaultInfo(files = depset(direct = [ctx.actions.declare_directory(output_dir)]))

m31a_schema_bundle = rule(
    implementation = _m31a_schema_bundle_impl,
    attrs = {
        "proto_sources": attr.label_list(allow_files = [".proto"]),
        "output_dir": attr.string(),
    },
)


def _m31a_e2e_benchmark_impl(ctx):
    """Run E2E benchmarks with statistical analysis."""
    test_script = ctx.attr.test_script
    iterations = ctx.attr.iterations

    ctx.actions.run(
        inputs = [test_script],
        outputs = [ctx.actions.declare_file("benchmark_results.json")],
        executable = "bash",
        arguments = [
            test_script.path,
            "--iterations", str(iterations),
            "--output", "benchmark_results.json",
        ],
        progress_message = "Running E2E benchmark: %s" % ctx.label.name,
    )

    return DefaultInfo(files = depset(direct = [ctx.actions.declare_file("benchmark_results.json")]))

m31a_e2e_benchmark = rule(
    implementation = _m31a_e2e_benchmark_impl,
    attrs = {
        "test_script": attr.label(
            mandatory = True,
            allow_single_file = True,
            executable = True,
        ),
        "iterations": attr.int(default = 10),
    },
)