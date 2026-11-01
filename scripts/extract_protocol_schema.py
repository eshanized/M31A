#!/usr/bin/env python3
"""Extract and validate the canonical M31A App Server Protocol JSON schema."""

import os
import sys
import json

def extract_schema(output_path: str):
    script_dir = os.path.dirname(os.path.realpath(__file__))
    workspace_dir = os.path.abspath(os.path.join(script_dir, ".."))
    
    # Preferred source: committed or generated schemas/app_server/schema.json
    source_schema_path = os.path.join(workspace_dir, "schemas", "app_server", "schema.json")
    
    if os.path.exists(source_schema_path):
        with open(source_schema_path, "r", encoding="utf-8") as f:
            schema_data = json.load(f)
    else:
        # Fallback to Rust source definition in crates/m31a-app-server/src/protocol_schema.rs
        rust_source = os.path.join(workspace_dir, "crates", "m31a-app-server", "src", "protocol_schema.rs")
        if not os.path.exists(rust_source):
            raise FileNotFoundError(f"Neither schema file nor protocol_schema.rs found in {workspace_dir}")
        with open(rust_source, "r", encoding="utf-8") as f:
            content = f.read()
        start_idx = content.find('r##"{')
        end_idx = content.find('}"##;')
        if start_idx == -1 or end_idx == -1:
            raise ValueError("Failed to locate raw protocol schema in protocol_schema.rs")
        raw_json = content[start_idx + 4:end_idx + 1]
        schema_data = json.loads(raw_json)

    # Validate essential schema elements
    assert "$schema" in schema_data, "Missing $schema declaration"
    assert "definitions" in schema_data, "Missing definitions in protocol schema"
    assert "RpcMethods" in schema_data["definitions"], "Missing RpcMethods in schema definitions"
    assert "EngineNotifications" in schema_data["definitions"], "Missing EngineNotifications in schema definitions"
    
    os.makedirs(os.path.dirname(os.path.abspath(output_path)), exist_ok=True)
    with open(output_path, "w", encoding="utf-8") as f:
        json.dump(schema_data, f, indent=2)
    print(f"App Server protocol schema successfully written to {output_path}")

if __name__ == "__main__":
    out = sys.argv[1] if len(sys.argv) > 1 else "schemas/app_server/schema.json"
    extract_schema(out)
