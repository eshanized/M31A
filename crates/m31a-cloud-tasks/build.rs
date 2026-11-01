//! Build script for `m31a-cloud-tasks`.
//!
//! Compiles gRPC Protocol Buffer definitions (`cloud_tasks.proto`) into Rust client stubs using tonic_build.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let proto_dir = "src/proto";
    let proto_file = "src/proto/cloud_tasks.proto";

    tonic_build::configure()
        .build_server(false) // We only need client
        .compile_protos(&[proto_file], &[proto_dir])?;

    // Recompile if proto files change
    println!("cargo:rerun-if-changed=src/proto/cloud_tasks.proto");

    Ok(())
}
