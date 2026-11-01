//! Process and Socket Connection Attribution for Security Auditing.
//!
//! Maps client network connections hitting the local proxy back to originating host
//! processes (PID, UID, process name) via Linux `/proc/net/tcp` socket inode resolution
//! or explicit framing preface (`\0M31APX1`), providing zero-trust connection telemetry
//! and session-authenticated proxy tunneling for outbound agent tool actions.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io::{self, Write};
use std::path::Path;
use std::sync::{Arc, RwLock};
use std::time::Duration;
use tokio::io::AsyncReadExt;
use tokio::net::TcpStream;

/// Protocol magic bytes prefixed to proxy TCP connections from sandboxed containers.
pub const ATTRIBUTION_FRAME_MAGIC: &[u8; 8] = b"\0M31APX1";

/// Maximum permitted length for an attribution token (128 bytes).
pub const MAX_ATTRIBUTION_TOKEN_LEN: usize = 128;

/// Timeout waiting for an attribution frame.
pub const ATTRIBUTION_FRAME_TIMEOUT: Duration = Duration::from_millis(500);

/// Environment variable key holding the execution attribution token.
pub const PROXY_ATTRIBUTION_TOKEN_ENV_KEY: &str = "M31A_NETWORK_PROXY_ATTRIBUTION";

/// Detailed attribution information for an inbound client connection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConnectionAttribution {
    pub client_port: u16,
    pub pid: Option<u32>,
    pub process_name: Option<String>,
    pub uid: Option<u32>,
    pub session_id: Option<String>,
    pub agent_id: Option<String>,
}

impl ConnectionAttribution {
    pub fn unknown(client_port: u16) -> Self {
        Self {
            client_port,
            pid: None,
            process_name: None,
            uid: None,
            session_id: None,
            agent_id: None,
        }
    }
}

/// Metadata registered for an active agent execution session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttributedSession {
    pub session_id: String,
    pub agent_id: String,
    pub token: String,
    pub allowed_domains: Vec<String>,
}

/// Global registry mapping attribution tokens to active agent sessions.
#[derive(Clone, Debug, Default)]
pub struct AttributionRegistry {
    sessions: Arc<RwLock<HashMap<String, AttributedSession>>>,
}

impl AttributionRegistry {
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn register(&self, session: AttributedSession) {
        let mut map = self.sessions.write().unwrap();
        map.insert(session.token.clone(), session);
    }

    pub fn unregister(&self, token: &str) {
        let mut map = self.sessions.write().unwrap();
        map.remove(token);
    }

    pub fn lookup(&self, token: &str) -> Option<AttributedSession> {
        let map = self.sessions.read().unwrap();
        map.get(token).cloned()
    }
}

/// Writes the trusted bridge preface consumed by the shared proxy ingress.
pub fn write_attribution_frame(writer: &mut impl Write, token: &str) -> io::Result<()> {
    if token.is_empty() || token.len() > MAX_ATTRIBUTION_TOKEN_LEN {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "invalid network proxy attribution token length",
        ));
    }
    let token_len = u16::try_from(token.len()).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "network proxy attribution token is too long",
        )
    })?;
    writer.write_all(ATTRIBUTION_FRAME_MAGIC)?;
    writer.write_all(&token_len.to_be_bytes())?;
    writer.write_all(token.as_bytes())
}

/// Inspects the start of a TCP stream, checks for the `\0M31APX1` attribution preface,
/// and consumes the token if present.
pub async fn read_attribution_token(stream: &mut TcpStream) -> io::Result<Option<String>> {
    let mut marker = [0_u8; 1];
    let read = stream.peek(&mut marker).await?;
    if read == 0 {
        return Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "empty proxy connection",
        ));
    }
    if marker[0] != ATTRIBUTION_FRAME_MAGIC[0] {
        return Ok(None);
    }

    // Try reading attribution frame with timeout
    match tokio::time::timeout(ATTRIBUTION_FRAME_TIMEOUT, async {
        let mut magic = [0_u8; ATTRIBUTION_FRAME_MAGIC.len()];
        stream.read_exact(&mut magic).await?;
        if &magic != ATTRIBUTION_FRAME_MAGIC {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "invalid network proxy attribution frame magic",
            ));
        }

        let token_len = stream.read_u16().await? as usize;
        if token_len == 0 || token_len > MAX_ATTRIBUTION_TOKEN_LEN {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "invalid network proxy attribution token length",
            ));
        }
        let mut token_bytes = vec![0_u8; token_len];
        stream.read_exact(&mut token_bytes).await?;
        String::from_utf8(token_bytes).map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "network proxy attribution token is not UTF-8",
            )
        })
    })
    .await
    {
        Ok(res) => Ok(Some(res?)),
        Err(_) => Ok(None),
    }
}

/// Socket attributor resolving local client ports to process IDs.
pub struct SocketAttributor;

impl SocketAttributor {
    /// Attempt to attribute a local client connection port to a running process on Linux.
    pub fn attribute_local_port(client_port: u16) -> ConnectionAttribution {
        #[cfg(target_os = "linux")]
        {
            if let Some((inode, uid)) = Self::find_socket_inode_and_uid(client_port)
                && let Some((pid, name)) = Self::find_pid_for_socket_inode(inode)
            {
                return ConnectionAttribution {
                    client_port,
                    pid: Some(pid),
                    process_name: Some(name),
                    uid: Some(uid),
                    session_id: None,
                    agent_id: None,
                };
            }
        }

        ConnectionAttribution::unknown(client_port)
    }

    /// Parse `/proc/net/tcp` and `/proc/net/tcp6` to find the socket inode and UID for a given local port.
    pub fn find_socket_inode_and_uid(client_port: u16) -> Option<(u64, u32)> {
        for path in &["/proc/net/tcp", "/proc/net/tcp6"] {
            if let Ok(content) = fs::read_to_string(path)
                && let Some(res) = Self::parse_proc_net_tcp(&content, client_port)
            {
                return Some(res);
            }
        }
        None
    }

    /// Helper to parse /proc/net/tcp tabular format.
    pub fn parse_proc_net_tcp(content: &str, target_port: u16) -> Option<(u64, u32)> {
        // Skip header line
        for line in content.lines().skip(1) {
            let fields: Vec<&str> = line.split_whitespace().collect();
            if fields.len() < 10 {
                continue;
            }

            // Field 1: local_address (hex IP:hex Port)
            let local_addr = fields[1];
            if let Some((_, port_hex)) = local_addr.split_once(':')
                && let Ok(port) = u16::from_str_radix(port_hex, 16)
                && port == target_port
            {
                // Field 7: uid
                let uid = fields[7].parse::<u32>().unwrap_or(0);
                // Field 9: inode
                let inode = fields[9].parse::<u64>().unwrap_or(0);
                return Some((inode, uid));
            }
        }
        None
    }

    /// Scan `/proc/[pid]/fd/` to find which process owns the target socket inode.
    pub fn find_pid_for_socket_inode(target_inode: u64) -> Option<(u32, String)> {
        let proc_dir = Path::new("/proc");
        let target_socket_pattern = format!("socket:[{}]", target_inode);

        let entries = fs::read_dir(proc_dir).ok()?;
        for entry in entries.flatten() {
            let file_name = entry.file_name();
            let name_str = file_name.to_str()?;
            if let Ok(pid) = name_str.parse::<u32>() {
                let fd_dir = entry.path().join("fd");
                if let Ok(fd_entries) = fs::read_dir(fd_dir) {
                    for fd_entry in fd_entries.flatten() {
                        if let Ok(link) = fs::read_link(fd_entry.path())
                            && link.to_string_lossy() == target_socket_pattern
                        {
                            let proc_name = Self::read_process_comm(pid)
                                .unwrap_or_else(|| "unknown".to_string());
                            return Some((pid, proc_name));
                        }
                    }
                }
            }
        }
        None
    }

    fn read_process_comm(pid: u32) -> Option<String> {
        let comm_path = format!("/proc/{}/comm", pid);
        fs::read_to_string(comm_path)
            .ok()
            .map(|s| s.trim().to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unknown_attribution() {
        let attr = ConnectionAttribution::unknown(8080);
        assert_eq!(attr.client_port, 8080);
        assert_eq!(attr.pid, None);
        assert_eq!(attr.process_name, None);
        assert_eq!(attr.session_id, None);
    }

    #[test]
    fn test_attribution_frame_roundtrip() {
        let token = "test_attribution_token_12345";
        let mut buffer = Vec::new();
        write_attribution_frame(&mut buffer, token).expect("write attribution frame");

        assert_eq!(&buffer[..8], ATTRIBUTION_FRAME_MAGIC);
        let len = u16::from_be_bytes([buffer[8], buffer[9]]) as usize;
        assert_eq!(len, token.len());
        assert_eq!(&buffer[10..10 + len], token.as_bytes());
    }

    #[test]
    fn test_attribution_registry() {
        let reg = AttributionRegistry::new();
        let session = AttributedSession {
            session_id: "sess-1".to_string(),
            agent_id: "agent-architect".to_string(),
            token: "tok-abc-123".to_string(),
            allowed_domains: vec!["api.github.com".to_string()],
        };
        reg.register(session);

        let found = reg.lookup("tok-abc-123").expect("session found");
        assert_eq!(found.agent_id, "agent-architect");
        assert_eq!(found.allowed_domains, vec!["api.github.com"]);

        reg.unregister("tok-abc-123");
        assert!(reg.lookup("tok-abc-123").is_none());
    }

    #[test]
    fn test_parse_proc_net_tcp_format() {
        let sample = r#"  sl  local_address rem_address   st tx_queue rx_queue tr tm->when retrnsmt   uid  timeout inode
   0: 0100007F:1F90 00000000:0000 0A 00000000:00000000 00:00000000 00000000  1000        0 1234567 1 0000000000000000 100 0 0 10 0
   1: 00000000:0016 00000000:0000 0A 00000000:00000000 00:00000000 00000000     0        0   23456 1 0000000000000000 100 0 0 10 0
"#;
        let res = SocketAttributor::parse_proc_net_tcp(sample, 8080);
        assert_eq!(res, Some((1234567, 1000)));

        let res_ssh = SocketAttributor::parse_proc_net_tcp(sample, 22);
        assert_eq!(res_ssh, Some((23456, 0)));

        let res_none = SocketAttributor::parse_proc_net_tcp(sample, 9999);
        assert_eq!(res_none, None);
    }
}
