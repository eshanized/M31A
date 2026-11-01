//! CLI utility to generate canonical JSON schemas for M31A App Server Protocol.

use anyhow::Result;
use m31a_app_server::write_protocol_schema;
use std::env;
use std::path::PathBuf;

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();
    let out_path = if args.len() > 1 {
        PathBuf::from(&args[1])
    } else {
        PathBuf::from("schemas/app_server/schema.json")
    };

    println!("Generating M31A App Server schema to {}...", out_path.display());
    write_protocol_schema(&out_path)?;
    println!("Schema successfully generated at {}", out_path.display());
    Ok(())
}
