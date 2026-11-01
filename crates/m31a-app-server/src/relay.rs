//! Standard I/O to Unix Domain Socket bidirectional stream relay.
//!
//! Enables stdio-based clients (like IDE extensions or subagent processes) to transparently
//! communicate with a running M31A daemon socket.

use anyhow::{Context, Result};
use std::io;
use std::path::Path;
use tokio::io::AsyncWriteExt;
use tokio::net::UnixStream;

/// Connects to the Unix Domain Socket at `socket_path` and relays data between
/// standard input/output streams and the socket, providing transparent stdio-to-UDS transport bridging.
pub async fn stdio_to_uds(socket_path: &Path) -> Result<()> {
    let stream = UnixStream::connect(socket_path)
        .await
        .with_context(|| format!("Failed to connect to UDS at {}", socket_path.display()))?;

    let (mut socket_reader, mut socket_writer) = tokio::io::split(stream);

    let copy_socket_to_stdout = async {
        let mut stdout = tokio::io::stdout();
        tokio::io::copy(&mut socket_reader, &mut stdout).await?;
        stdout.flush().await?;
        Ok::<(), anyhow::Error>(())
    };

    let copy_stdin_to_socket = async {
        let mut stdin = tokio::io::stdin();
        tokio::io::copy(&mut stdin, &mut socket_writer)
            .await
            .context("Failed to copy data from stdin to socket")?;

        if let Err(err) = socket_writer.shutdown().await
            && err.kind() != io::ErrorKind::NotConnected
        {
            return Err(anyhow::anyhow!("Failed to shutdown socket writer: {}", err));
        }
        Ok::<(), anyhow::Error>(())
    };

    tokio::try_join!(copy_stdin_to_socket, copy_socket_to_stdout)
        .context("Failed to relay data between stdio and socket")?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::UnixListener;

    #[tokio::test]
    async fn test_uds_connection() {
        let dir = tempdir().unwrap();
        let sock_path = dir.path().join("test.sock");
        let listener = UnixListener::bind(&sock_path).unwrap();

        tokio::spawn(async move {
            if let Ok((mut stream, _)) = listener.accept().await {
                let mut buf = [0u8; 4];
                let _ = stream.read_exact(&mut buf).await;
                let _ = stream.write_all(b"PONG").await;
            }
        });

        let mut client = UnixStream::connect(&sock_path).await.unwrap();
        client.write_all(b"PING").await.unwrap();
        let mut resp = [0u8; 4];
        client.read_exact(&mut resp).await.unwrap();
        assert_eq!(&resp, b"PONG");
    }
}
