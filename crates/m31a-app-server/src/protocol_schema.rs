//! Canonical JSON Schema generator for M31A App Server Protocol.
//!
//! Synthesizes JSON Schema (Draft-07) specifications used by editors, clients,
//! IDE extensions, and API validation harnesses to interact with the M31A App Server.

use anyhow::{Context, Result};
use serde_json::Value;
use std::fs;
use std::path::Path;

/// Generates the canonical JSON Schema (Draft-07) for the M31A App Server Protocol.
pub fn generate_protocol_schema() -> Value {
    serde_json::from_str(PROTOCOL_SCHEMA_RAW).expect("valid App Server protocol schema JSON")
}

pub const PROTOCOL_SCHEMA_RAW: &str = r##"{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "title": "M31A App Server JSON-RPC Protocol Schema",
  "description": "JSON-RPC 2.0 and SSE event streaming protocol specification for the M31A autonomous coding agent daemon.",
  "type": "object",
  "definitions": {
    "JsonRpcRequest": {
      "type": "object",
      "description": "Standard JSON-RPC 2.0 request payload.",
      "required": ["jsonrpc", "id", "method"],
      "properties": {
        "jsonrpc": { "type": "string", "enum": ["2.0"] },
        "id": {
          "oneOf": [
            { "type": "integer" },
            { "type": "string" }
          ]
        },
        "method": { "type": "string" },
        "params": {
          "description": "Method-specific parameters object."
        }
      }
    },
    "JsonRpcResponse": {
      "type": "object",
      "description": "Standard JSON-RPC 2.0 response payload.",
      "required": ["jsonrpc", "id"],
      "properties": {
        "jsonrpc": { "type": "string", "enum": ["2.0"] },
        "id": {
          "oneOf": [
            { "type": "integer" },
            { "type": "string" },
            { "type": "null" }
          ]
        },
        "result": { "description": "Success payload if no error occurred." },
        "error": { "$ref": "#/definitions/JsonRpcError" }
      }
    },
    "JsonRpcError": {
      "type": "object",
      "description": "Standard JSON-RPC 2.0 error representation.",
      "required": ["code", "message"],
      "properties": {
        "code": { "type": "integer" },
        "message": { "type": "string" },
        "data": { "description": "Optional structured diagnostic data." }
      }
    },
    "JsonRpcNotification": {
      "type": "object",
      "description": "Unsolicited notification dispatched to subscribers over SSE, stdio, or UDS.",
      "required": ["jsonrpc", "method", "params"],
      "properties": {
        "jsonrpc": { "type": "string", "enum": ["2.0"] },
        "method": { "type": "string" },
        "params": { "type": "object" }
      }
    },
    "RpcMethods": {
      "type": "object",
      "description": "Catalog of supported RPC methods with authorization requirements and parameter schemas.",
      "properties": {
        "initialize": {
          "description": "Handshake to establish protocol capabilities and client identity.",
          "authScope": "Public",
          "params": {
            "type": "object",
            "properties": {
              "client_name": { "type": "string" },
              "client_version": { "type": "string" },
              "capabilities": { "type": "array", "items": { "type": "string" } }
            }
          },
          "result": {
            "type": "object",
            "properties": {
              "server_name": { "type": "string" },
              "version": { "type": "string" },
              "protocol_version": { "type": "string" },
              "capabilities": { "type": "array", "items": { "type": "string" } },
              "auth_required": { "type": "boolean" }
            }
          }
        },
        "server/diagnostics": {
          "description": "Retrieve daemon health, active threads, memory, and uptime metrics (secrets redacted).",
          "authScope": "Read",
          "params": { "type": "object" },
          "result": {
            "type": "object",
            "properties": {
              "status": { "type": "string" },
              "version": { "type": "string" },
              "uptime_seconds": { "type": "integer" },
              "threads_active": { "type": "integer" },
              "storage_connected": { "type": "boolean" }
            }
          }
        },
        "models/list": {
          "description": "Query available LLM model catalog from configured provider backend.",
          "authScope": "Read",
          "params": { "type": "object" },
          "result": {
            "type": "object",
            "properties": {
              "models": {
                "type": "array",
                "items": {
                  "type": "object",
                  "properties": {
                    "id": { "type": "string" },
                    "name": { "type": "string" },
                    "context_length": { "type": "integer" }
                  }
                }
              }
            }
          }
        },
        "thread/list": {
          "description": "List conversation threads owned by or accessible to caller.",
          "authScope": "Read",
          "params": { "type": "object" },
          "result": {
            "type": "object",
            "properties": {
              "threads": {
                "type": "array",
                "items": {
                  "type": "object",
                  "properties": {
                    "id": { "type": "string" },
                    "title": { "type": "string" },
                    "owner_id": { "type": "string" },
                    "created_at_epoch_ms": { "type": "integer" }
                  }
                }
              }
            }
          }
        },
        "thread/read": {
          "description": "Retrieve full thread metadata and message history.",
          "authScope": "Read",
          "requiresThreadOwnership": true,
          "params": {
            "type": "object",
            "required": ["thread_id"],
            "properties": {
              "thread_id": { "type": "string" }
            }
          },
          "result": {
            "type": "object",
            "properties": {
              "thread": { "type": "object" },
              "turns": { "type": "array", "items": { "type": "object" } }
            }
          }
        },
        "thread/create": {
          "description": "Create a new conversation thread with authoritative ownership.",
          "authScope": "Write",
          "params": {
            "type": "object",
            "properties": {
              "title": { "type": "string" },
              "workspace_path": { "type": "string" }
            }
          },
          "result": {
            "type": "object",
            "properties": {
              "thread_id": { "type": "string" },
              "owner_id": { "type": "string" }
            }
          }
        },
        "thread/archive": {
          "description": "Archive an existing thread.",
          "authScope": "Write",
          "requiresThreadOwnership": true,
          "params": {
            "type": "object",
            "required": ["thread_id"],
            "properties": { "thread_id": { "type": "string" } }
          },
          "result": {
            "type": "object",
            "properties": { "status": { "type": "string" } }
          }
        },
        "thread/unarchive": {
          "description": "Restore an archived thread.",
          "authScope": "Write",
          "requiresThreadOwnership": true,
          "params": {
            "type": "object",
            "required": ["thread_id"],
            "properties": { "thread_id": { "type": "string" } }
          },
          "result": {
            "type": "object",
            "properties": { "status": { "type": "string" } }
          }
        },
        "thread/delete": {
          "description": "Permanently delete a thread.",
          "authScope": "Write",
          "requiresThreadOwnership": true,
          "params": {
            "type": "object",
            "required": ["thread_id"],
            "properties": { "thread_id": { "type": "string" } }
          },
          "result": {
            "type": "object",
            "properties": { "status": { "type": "string" } }
          }
        },
        "thread/fork": {
          "description": "Fork a thread into a new branch (requires parent ownership).",
          "authScope": "Write",
          "requiresThreadOwnership": true,
          "params": {
            "type": "object",
            "required": ["thread_id"],
            "properties": {
              "thread_id": { "type": "string" },
              "title": { "type": "string" }
            }
          },
          "result": {
            "type": "object",
            "properties": {
              "thread_id": { "type": "string" },
              "parent_thread_id": { "type": "string" }
            }
          }
        },
        "thread/resume": {
          "description": "Resume execution context for a previous thread.",
          "authScope": "Read",
          "requiresThreadOwnership": true,
          "params": {
            "type": "object",
            "required": ["thread_id"],
            "properties": { "thread_id": { "type": "string" } }
          },
          "result": {
            "type": "object",
            "properties": { "thread_id": { "type": "string" }, "status": { "type": "string" } }
          }
        },
        "turn/start": {
          "description": "Dispatch an autonomous agent execution turn.",
          "authScope": "Execute",
          "requiresThreadOwnership": true,
          "params": {
            "type": "object",
            "required": ["thread_id", "prompt"],
            "properties": {
              "thread_id": { "type": "string" },
              "prompt": { "type": "string" },
              "model": { "type": "string" },
              "plan_mode": { "type": "boolean" },
              "sandbox_mode": { "type": "string" }
            }
          },
          "result": {
            "type": "object",
            "properties": {
              "turn_id": { "type": "string" },
              "status": { "type": "string" }
            }
          }
        },
        "turn/interrupt": {
          "description": "Halt an in-flight agent turn.",
          "authScope": "Execute",
          "requiresThreadOwnership": true,
          "params": {
            "type": "object",
            "required": ["thread_id"],
            "properties": { "thread_id": { "type": "string" } }
          },
          "result": {
            "type": "object",
            "properties": { "interrupted": { "type": "boolean" } }
          }
        },
        "turn/approve": {
          "description": "Grant or reject interactive tool execution approval.",
          "authScope": "Execute",
          "params": {
            "type": "object",
            "required": ["turn_id", "decision"],
            "properties": {
              "turn_id": { "type": "string" },
              "decision": { "type": "string", "enum": ["approve", "deny"] },
              "reason": { "type": "string" }
            }
          },
          "result": {
            "type": "object",
            "properties": { "acknowledged": { "type": "boolean" } }
          }
        },
        "workspace/trust_grant": {
          "description": "Grant security authorization to execute commands in workspace directory.",
          "authScope": "Admin",
          "params": {
            "type": "object",
            "required": ["path"],
            "properties": {
              "path": { "type": "string" }
            }
          },
          "result": {
            "type": "object",
            "properties": { "granted": { "type": "boolean" } }
          }
        },
        "config/read": {
          "description": "Read sanitized configuration properties (all secrets and tokens redacted).",
          "authScope": "Read",
          "params": { "type": "object" },
          "result": {
            "type": "object",
            "properties": {
              "config": { "type": "object" },
              "redacted_secrets": { "type": "array", "items": { "type": "string" } }
            }
          }
        },
        "auth/stream_ticket": {
          "description": "Issue a short-lived single-use ticket for authenticating SSE event streams.",
          "authScope": "Read",
          "requiresThreadOwnership": true,
          "params": {
            "type": "object",
            "required": ["thread_id"],
            "properties": { "thread_id": { "type": "string" } }
          },
          "result": {
            "type": "object",
            "properties": {
              "ticket": { "type": "string" },
              "thread_id": { "type": "string" },
              "expires_in_sec": { "type": "integer" }
            }
          }
        },
        "auth/rotate_token": {
          "description": "Rotate an authentication token and invalidate previous secret.",
          "authScope": "Admin",
          "params": {
            "type": "object",
            "required": ["token_id"],
            "properties": { "token_id": { "type": "string" } }
          },
          "result": {
            "type": "object",
            "properties": {
              "token_id": { "type": "string" },
              "new_token": { "type": "string" },
              "scope": { "type": "string" }
            }
          }
        },
        "auth/revoke_token": {
          "description": "Revoke an authentication token.",
          "authScope": "Admin",
          "params": {
            "type": "object",
            "required": ["token_id"],
            "properties": { "token_id": { "type": "string" } }
          },
          "result": {
            "type": "object",
            "properties": { "revoked": { "type": "boolean" } }
          }
        }
      }
    },
    "EngineNotifications": {
      "type": "object",
      "description": "Asynchronous events emitted by the agent engine.",
      "properties": {
        "chunk": {
          "description": "Incremental streaming token delta from the model.",
          "properties": {
            "thread_id": { "type": "string" },
            "delta": { "type": "string" }
          }
        },
        "tool_start": {
          "description": "Notification that a tool call has begun execution.",
          "properties": {
            "thread_id": { "type": "string" },
            "tool_name": { "type": "string" },
            "call_id": { "type": "string" },
            "arguments": { "type": "object" }
          }
        },
        "tool_result": {
          "description": "Notification containing output of completed tool call.",
          "properties": {
            "thread_id": { "type": "string" },
            "call_id": { "type": "string" },
            "output": { "type": "string" },
            "is_error": { "type": "boolean" }
          }
        },
        "compact": {
          "description": "Context compaction event with token statistics.",
          "properties": {
            "thread_id": { "type": "string" },
            "tokens_before": { "type": "integer" },
            "tokens_after": { "type": "integer" }
          }
        },
        "done": {
          "description": "Turn execution finished.",
          "properties": {
            "thread_id": { "type": "string" },
            "turn_id": { "type": "string" },
            "usage": { "type": "object" }
          }
        },
        "interrupted": {
          "description": "Turn execution was canceled or interrupted.",
          "properties": {
            "thread_id": { "type": "string" },
            "reason": { "type": "string" }
          }
        },
        "error": {
          "description": "Turn failed with an unrecoverable error.",
          "properties": {
            "thread_id": { "type": "string" },
            "error_code": { "type": "string" },
            "error_message": { "type": "string" }
          }
        }
      }
    }
  }
}"##;

/// Writes the canonical App Server protocol schema to the specified path.
pub fn write_protocol_schema(out_path: &Path) -> Result<()> {
    if let Some(parent) = out_path.parent() {
        fs::create_dir_all(parent).with_context(|| {
            format!("Failed to create schema directory at {}", parent.display())
        })?;
    }
    let schema = generate_protocol_schema();
    let json_bytes = serde_json::to_vec_pretty(&schema)?;
    fs::write(out_path, json_bytes)
        .with_context(|| format!("Failed to write schema to {}", out_path.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_and_validate_protocol_schema() {
        let schema = generate_protocol_schema();
        assert_eq!(schema["$schema"], "http://json-schema.org/draft-07/schema#");
        assert_eq!(schema["title"], "M31A App Server JSON-RPC Protocol Schema");

        // Verify definitions exist
        let defs = schema["definitions"]
            .as_object()
            .expect("definitions object");
        assert!(defs.contains_key("JsonRpcRequest"));
        assert!(defs.contains_key("JsonRpcResponse"));
        assert!(defs.contains_key("JsonRpcError"));
        assert!(defs.contains_key("JsonRpcNotification"));
        assert!(defs.contains_key("RpcMethods"));
        assert!(defs.contains_key("EngineNotifications"));

        // Verify core RPC methods have an entry in RpcMethods
        let rpc_methods = defs["RpcMethods"]["properties"]
            .as_object()
            .expect("RpcMethods properties");
        let expected_methods = [
            "initialize",
            "server/diagnostics",
            "models/list",
            "thread/list",
            "thread/read",
            "thread/create",
            "thread/archive",
            "thread/unarchive",
            "thread/delete",
            "thread/fork",
            "thread/resume",
            "turn/start",
            "turn/interrupt",
            "turn/approve",
            "workspace/trust_grant",
            "config/read",
            "auth/stream_ticket",
            "auth/rotate_token",
            "auth/revoke_token",
        ];
        for method_name in expected_methods {
            assert!(
                rpc_methods.contains_key(method_name),
                "Missing schema definition for registered RPC method: {}",
                method_name
            );
        }
    }

    #[test]
    fn test_write_protocol_schema() {
        let temp_dir = tempfile::tempdir().unwrap();
        let schema_path = temp_dir.path().join("schemas/app_server/schema.json");
        write_protocol_schema(&schema_path).unwrap();
        assert!(schema_path.exists());

        let content = fs::read_to_string(&schema_path).unwrap();
        let parsed: Value = serde_json::from_str(&content).unwrap();
        assert_eq!(parsed["$schema"], "http://json-schema.org/draft-07/schema#");
    }
}
