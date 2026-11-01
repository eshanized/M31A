//! Encrypted Peer-to-Peer Session Pairing Tunnel (`m31a pair`).
//!
//! Provides ChaCha20-Poly1305 AEAD authenticated and encrypted bidirectional communication
//! tunnels, enabling remote terminals and developers to securely join and observe active
//! agent sessions without unencrypted transport exposure.

use anyhow::{Context, Result, anyhow};
use chacha20poly1305::{
    ChaCha20Poly1305, Nonce,
    aead::{Aead, KeyInit},
};
use rand::Rng;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tracing::info;

pub const NONCE_LEN: usize = 12;
pub const KEY_LEN: usize = 32;

/// Credentials required to connect to a paired live session.
#[derive(Debug, Clone)]
pub struct PairingCredentials {
    pub host_addr: SocketAddr,
    pub secret_key: [u8; KEY_LEN],
}

impl PairingCredentials {
    /// Generates fresh random 256-bit encryption key and pairing credentials.
    pub fn generate(host_addr: SocketAddr) -> Self {
        let mut key = [0u8; KEY_LEN];
        rand::rng().fill_bytes(&mut key);
        Self {
            host_addr,
            secret_key: key,
        }
    }

    /// Formats the credentials into an `m31a-pair://` URI string.
    pub fn to_uri(&self) -> String {
        format!(
            "m31a-pair://{}#{}",
            self.host_addr,
            hex::encode(self.secret_key)
        )
    }

    /// Parses an `m31a-pair://` URI into pairing credentials.
    pub fn from_uri(uri: &str) -> Result<Self> {
        let trimmed = uri.trim();
        let stripped = trimmed
            .strip_prefix("m31a-pair://")
            .ok_or_else(|| anyhow!("Invalid URI scheme, expected 'm31a-pair://'"))?;

        let (addr_str, hex_key) = stripped
            .split_once('#')
            .ok_or_else(|| anyhow!("Malformed pairing URI: missing '#<key>' fragment"))?;

        let host_addr: SocketAddr = addr_str
            .parse()
            .with_context(|| format!("Failed to parse host socket address '{}'", addr_str))?;

        let key_vec = hex::decode(hex_key)
            .context("Failed to decode hex encryption key from URI fragment")?;

        if key_vec.len() != KEY_LEN {
            return Err(anyhow!(
                "Invalid encryption key length: expected 32 bytes, got {}",
                key_vec.len()
            ));
        }

        let mut secret_key = [0u8; KEY_LEN];
        secret_key.copy_from_slice(&key_vec);

        Ok(Self {
            host_addr,
            secret_key,
        })
    }
}

/// An authenticated and encrypted bidirectional frame channel.
pub struct EncryptedTunnel {
    stream: TcpStream,
    cipher: ChaCha20Poly1305,
    outbound_counter: Arc<AtomicU64>,
}

impl EncryptedTunnel {
    pub fn new(stream: TcpStream, secret_key: &[u8; KEY_LEN]) -> Self {
        let cipher = ChaCha20Poly1305::new(secret_key.into());
        Self {
            stream,
            cipher,
            outbound_counter: Arc::new(AtomicU64::new(1)),
        }
    }

    /// Encrypts and transmits a binary frame.
    /// Wire format: `[u32 payload_len (including nonce + tag) | 12-byte nonce | ciphertext]`.
    pub async fn send_frame(&mut self, plaintext: &[u8]) -> Result<()> {
        let counter = self.outbound_counter.fetch_add(1, Ordering::SeqCst);
        let mut nonce_bytes = [0u8; NONCE_LEN];
        rand::rng().fill_bytes(&mut nonce_bytes[..4]);
        nonce_bytes[4..].copy_from_slice(&counter.to_be_bytes());

        let nonce = Nonce::from_slice(&nonce_bytes);
        let ciphertext = self
            .cipher
            .encrypt(nonce, plaintext)
            .map_err(|e| anyhow!("Encryption error: {:?}", e))?;

        let total_frame_len = (NONCE_LEN + ciphertext.len()) as u32;

        self.stream
            .write_all(&total_frame_len.to_be_bytes())
            .await?;
        self.stream.write_all(&nonce_bytes).await?;
        self.stream.write_all(&ciphertext).await?;
        self.stream.flush().await?;

        Ok(())
    }

    /// Reads and decrypts the next incoming frame.
    pub async fn recv_frame(&mut self) -> Result<Vec<u8>> {
        let mut len_buf = [0u8; 4];
        self.stream
            .read_exact(&mut len_buf)
            .await
            .context("Failed to read frame header")?;
        let frame_len = u32::from_be_bytes(len_buf) as usize;

        if frame_len < NONCE_LEN {
            return Err(anyhow!("Corrupt frame: length smaller than nonce"));
        }
        if frame_len > 16 * 1024 * 1024 {
            return Err(anyhow!("Frame exceeds 16MB maximum payload limit"));
        }

        let mut nonce_buf = [0u8; NONCE_LEN];
        self.stream
            .read_exact(&mut nonce_buf)
            .await
            .context("Failed to read frame nonce")?;
        let nonce = Nonce::from_slice(&nonce_buf);

        let ciphertext_len = frame_len - NONCE_LEN;
        let mut ciphertext = vec![0u8; ciphertext_len];
        self.stream
            .read_exact(&mut ciphertext)
            .await
            .context("Failed to read frame ciphertext")?;

        let plaintext = self
            .cipher
            .decrypt(nonce, ciphertext.as_ref())
            .map_err(|e| anyhow!("Decryption or tag authentication failed: {:?}", e))?;

        Ok(plaintext)
    }

    /// Sends a UTF-8 text message over the encrypted tunnel.
    pub async fn send_text(&mut self, text: &str) -> Result<()> {
        self.send_frame(text.as_bytes()).await
    }

    /// Receives a UTF-8 text message over the encrypted tunnel.
    pub async fn recv_text(&mut self) -> Result<String> {
        let bytes = self.recv_frame().await?;
        String::from_utf8(bytes).context("Payload was not valid UTF-8")
    }
}

/// Host listener for live P2P pairing sessions.
pub struct PairingHost {
    listener: TcpListener,
    credentials: PairingCredentials,
}

impl PairingHost {
    /// Binds to `bind_addr` and initializes encrypted pairing host.
    pub async fn bind(bind_addr: SocketAddr) -> Result<Self> {
        let listener = TcpListener::bind(bind_addr)
            .await
            .with_context(|| format!("Failed to bind pairing host listener to {}", bind_addr))?;

        let actual_addr = listener.local_addr()?;
        let credentials = PairingCredentials::generate(actual_addr);

        info!("Encrypted P2P pairing host listening on {}", actual_addr);

        Ok(Self {
            listener,
            credentials,
        })
    }

    pub fn credentials(&self) -> &PairingCredentials {
        &self.credentials
    }

    /// Accepts an incoming peer connection and wraps it in an encrypted tunnel.
    pub async fn accept(&self) -> Result<(EncryptedTunnel, SocketAddr)> {
        let (stream, remote_addr) = self.listener.accept().await?;
        let tunnel = EncryptedTunnel::new(stream, &self.credentials.secret_key);
        Ok((tunnel, remote_addr))
    }
}

/// Client connector for joining a paired session.
pub struct PairingClient;

impl PairingClient {
    /// Connects to a host using an `m31a-pair://` URI.
    pub async fn connect(pairing_uri: &str) -> Result<EncryptedTunnel> {
        let creds = PairingCredentials::from_uri(pairing_uri)?;
        let stream = TcpStream::connect(creds.host_addr)
            .await
            .with_context(|| format!("Failed to connect to pairing host at {}", creds.host_addr))?;

        Ok(EncryptedTunnel::new(stream, &creds.secret_key))
    }
}

// Simple hex encoding/decoding helper
mod hex {
    use anyhow::{Result, anyhow};

    pub fn encode(data: impl AsRef<[u8]>) -> String {
        data.as_ref().iter().map(|b| format!("{:02x}", b)).collect()
    }

    pub fn decode(s: &str) -> Result<Vec<u8>> {
        if !s.len().is_multiple_of(2) {
            return Err(anyhow!("Hex string must have even length"));
        }
        (0..s.len())
            .step_by(2)
            .map(|i| {
                u8::from_str_radix(&s[i..i + 2], 16).map_err(|e| anyhow!("Invalid hex byte: {}", e))
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_pairing_uri_and_encrypted_tunnel() {
        let host = PairingHost::bind("127.0.0.1:0".parse().unwrap())
            .await
            .unwrap();
        let uri = host.credentials().to_uri();

        assert!(uri.starts_with("m31a-pair://127.0.0.1:"));
        assert!(uri.contains('#'));

        let parsed_creds = PairingCredentials::from_uri(&uri).unwrap();
        assert_eq!(parsed_creds.host_addr, host.credentials().host_addr);
        assert_eq!(parsed_creds.secret_key, host.credentials().secret_key);

        // Connect client to host
        let client_handle = tokio::spawn(async move {
            let mut client = PairingClient::connect(&uri).await.unwrap();
            client.send_text("Hello secure peer!").await.unwrap();
            let reply = client.recv_text().await.unwrap();
            assert_eq!(reply, "Session authenticated.");
        });

        let (mut host_tunnel, _) = host.accept().await.unwrap();
        let msg = host_tunnel.recv_text().await.unwrap();
        assert_eq!(msg, "Hello secure peer!");

        host_tunnel
            .send_text("Session authenticated.")
            .await
            .unwrap();
        client_handle.await.unwrap();
    }
}
