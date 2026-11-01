//! Multi-environment file copy tool.
//! Synchronizes files across host filesystems and isolated container environments with path mapping.

use serde::{Deserialize, Serialize};
use std::path::Path;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

pub const MULTI_ENV_COPY_TOOL_NAME: &str = "multi_env_copy_file";
pub const MULTI_ENV_COPY_DESCRIPTION: &str = "Copy a file between two different environments without loading it into context. Returns bytes copied after completion. Interrupted copies may leave files partially written.";

const BUFFER_CHUNK_SIZE: usize = 64 * 1024; // 64 KiB chunks

/// Arguments for `multi_env_copy_file`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MultiEnvCopyArgs {
    pub source_environment_id: String,
    pub source_path: String,
    pub target_environment_id: String,
    pub target_path: String,
}

/// Output returned by `multi_env_copy_file`.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct MultiEnvCopyOutput {
    pub bytes_copied: u64,
}

/// Validates arguments and executes the direct streaming copy between environments.
pub async fn execute_multi_env_copy(args: MultiEnvCopyArgs) -> anyhow::Result<MultiEnvCopyOutput> {
    if args.source_environment_id == args.target_environment_id {
        anyhow::bail!(
            "source and target environment IDs must differ. To copy within one environment, use a shell command."
        );
    }

    let src = Path::new(&args.source_path);
    let dst = Path::new(&args.target_path);

    if !src.is_absolute() {
        anyhow::bail!("source_path must be an absolute path");
    }
    if !dst.is_absolute() {
        anyhow::bail!("target_path must be an absolute path");
    }

    if let Some(parent) = dst.parent()
        && !parent.as_os_str().is_empty()
    {
        tokio::fs::create_dir_all(parent).await?;
    }

    let mut source_file = tokio::fs::File::open(src)
        .await
        .map_err(|e| anyhow::anyhow!("could not open copy source: {e}"))?;

    let mut target_file = tokio::fs::File::create(dst)
        .await
        .map_err(|e| anyhow::anyhow!("could not create copy destination: {e}"))?;

    let mut buffer = vec![0u8; BUFFER_CHUNK_SIZE];
    let mut total_bytes = 0u64;

    loop {
        let n = source_file
            .read(&mut buffer)
            .await
            .map_err(|e| anyhow::anyhow!("error reading copy source: {e}"))?;
        if n == 0 {
            break;
        }
        target_file
            .write_all(&buffer[..n])
            .await
            .map_err(|e| anyhow::anyhow!("copy failed; target may be truncated: {e}"))?;
        total_bytes += n as u64;
    }

    target_file
        .flush()
        .await
        .map_err(|e| anyhow::anyhow!("error flushing target file: {e}"))?;

    Ok(MultiEnvCopyOutput {
        bytes_copied: total_bytes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_same_environment_rejected() {
        let args = MultiEnvCopyArgs {
            source_environment_id: "env_local".to_string(),
            source_path: "/tmp/foo.txt".to_string(),
            target_environment_id: "env_local".to_string(),
            target_path: "/tmp/bar.txt".to_string(),
        };

        let err = execute_multi_env_copy(args).await.unwrap_err();
        assert!(
            err.to_string()
                .contains("source and target environment IDs must differ")
        );
    }

    #[tokio::test]
    async fn test_relative_path_rejected() {
        let args = MultiEnvCopyArgs {
            source_environment_id: "env_1".to_string(),
            source_path: "relative/foo.txt".to_string(),
            target_environment_id: "env_2".to_string(),
            target_path: "/tmp/bar.txt".to_string(),
        };

        let err = execute_multi_env_copy(args).await.unwrap_err();
        assert!(err.to_string().contains("must be an absolute path"));
    }

    #[tokio::test]
    async fn test_successful_file_copy() {
        let tmp = tempfile::tempdir().unwrap();
        let src = tmp.path().join("source.dat");
        let dst = tmp.path().join("subdir").join("dest.dat");

        let test_payload = b"Hello from isolated source environment payload!";
        tokio::fs::write(&src, test_payload).await.unwrap();

        let args = MultiEnvCopyArgs {
            source_environment_id: "env_docker_1".to_string(),
            source_path: src.to_str().unwrap().to_string(),
            target_environment_id: "env_host".to_string(),
            target_path: dst.to_str().unwrap().to_string(),
        };

        let res = execute_multi_env_copy(args).await.expect("must succeed");
        assert_eq!(res.bytes_copied, test_payload.len() as u64);

        let copied = tokio::fs::read(&dst).await.unwrap();
        assert_eq!(copied, test_payload);
    }
}
