//! local lsp stdio process provider.

use std::path::Path;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt};
use tokio::process::Command;

/// local lsp process client for executing stdio json-rpc queries.
pub struct LocalLspProcessClient;

impl LocalLspProcessClient {
    /// execute stdio json-rpc query with bounded timeout.
    pub async fn query_stdio(
        workspace_root: &Path,
        cmd_bin: &str,
        cmd_args: &[&str],
        method: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        let action = async {
            let mut cmd = Command::new(cmd_bin);
            cmd.args(cmd_args)
                .current_dir(workspace_root)
                .stdin(std::process::Stdio::piped())
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::null());

            let mut child = cmd
                .spawn()
                .map_err(|e| format!("failed to spawn {cmd_bin}: {e}"))?;
            let mut stdin = child
                .stdin
                .take()
                .ok_or_else(|| "failed to capture stdin".to_string())?;
            let stdout = child
                .stdout
                .take()
                .ok_or_else(|| "failed to capture stdout".to_string())?;
            let mut reader = tokio::io::BufReader::new(stdout);

            let root_uri = format!(
                "file://{}",
                workspace_root
                    .canonicalize()
                    .unwrap_or_else(|_| workspace_root.to_path_buf())
                    .display()
            );

            // send initialize request
            let init_req = serde_json::json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "initialize",
                "params": {
                    "processId": null,
                    "rootUri": root_uri,
                    "capabilities": {}
                }
            });
            send_lsp_message(&mut stdin, &init_req).await?;

            // wait for initialize response
            let _ = read_lsp_response(&mut reader, 1).await?;

            // send initialized notification
            let initialized_notif = serde_json::json!({
                "jsonrpc": "2.0",
                "method": "initialized",
                "params": {}
            });
            send_lsp_message(&mut stdin, &initialized_notif).await?;

            // send query request
            let query_req = serde_json::json!({
                "jsonrpc": "2.0",
                "id": 2,
                "method": method,
                "params": params
            });
            send_lsp_message(&mut stdin, &query_req).await?;

            // wait for query response
            let response = read_lsp_response(&mut reader, 2).await?;

            // terminate child
            let _ = child.kill().await;

            let result = response
                .get("result")
                .cloned()
                .unwrap_or(serde_json::Value::Null);
            Ok(result)
        };

        tokio::time::timeout(Duration::from_secs(5), action)
            .await
            .map_err(|_| "lsp stdio query timed out after 5s".to_string())?
    }
}

async fn send_lsp_message<W: AsyncWriteExt + Unpin>(
    writer: &mut W,
    val: &serde_json::Value,
) -> Result<(), String> {
    let payload = serde_json::to_string(val).map_err(|e| e.to_string())?;
    let header = format!("Content-Length: {}\r\n\r\n", payload.len());
    writer
        .write_all(header.as_bytes())
        .await
        .map_err(|e| e.to_string())?;
    writer
        .write_all(payload.as_bytes())
        .await
        .map_err(|e| e.to_string())?;
    writer.flush().await.map_err(|e| e.to_string())?;
    Ok(())
}

async fn read_lsp_response<R: AsyncBufReadExt + Unpin>(
    reader: &mut R,
    expected_id: i64,
) -> Result<serde_json::Value, String> {
    loop {
        let mut content_length: Option<usize> = None;
        let mut line = String::new();
        loop {
            line.clear();
            let bytes_read = reader
                .read_line(&mut line)
                .await
                .map_err(|e| e.to_string())?;
            if bytes_read == 0 {
                return Err("unexpected eof reading lsp headers".to_string());
            }
            let trimmed = line.trim();
            if trimmed.is_empty() {
                // end of headers
                break;
            }
            if let Some(rest) = trimmed.strip_prefix("Content-Length:") {
                let len: usize = rest
                    .trim()
                    .parse()
                    .map_err(|e| format!("invalid content length: {e}"))?;
                content_length = Some(len);
            }
        }

        let len = content_length
            .ok_or_else(|| "missing Content-Length header in lsp response".to_string())?;
        let mut buf = vec![0u8; len];
        reader
            .read_exact(&mut buf)
            .await
            .map_err(|e| e.to_string())?;
        let val: serde_json::Value = serde_json::from_slice(&buf)
            .map_err(|e| format!("invalid json from lsp server: {e}"))?;

        // match expected id if present; skip notifications/unmatched ids
        if val.get("id").and_then(|v| v.as_i64()) == Some(expected_id) {
            return Ok(val);
        }
    }
}
