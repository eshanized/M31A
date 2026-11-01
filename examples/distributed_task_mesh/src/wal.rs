//! Write-Ahead Log (WAL) engine for durable task recovery.
//!
//! Provides checksummed, append-only disk logging to ensure task persistence across node failures.

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use crate::types::{TaskId, TaskStatus};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum WalRecord {
    CreateTask {
        id: TaskId,
        name: String,
        payload: serde_json::Value,
        priority: u8,
        created_at: u64,
    },
    UpdateStatus {
        id: TaskId,
        status: TaskStatus,
        timestamp: u64,
    },
    Checkpoint {
        active_tasks_count: usize,
        timestamp: u64,
    },
}

pub struct WriteAheadLog {
    path: PathBuf,
    file: Mutex<File>,
}

impl WriteAheadLog {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&path)
            .with_context(|| format!("Failed to open WAL at {}", path.display()))?;

        Ok(Self {
            path,
            file: Mutex::new(file),
        })
    }

    /// Appends a structured record with a 4-byte payload length + 32-byte SHA256 checksum.
    pub fn append(&self, record: &WalRecord) -> Result<u64> {
        let payload = serde_json::to_vec(record)?;
        let mut hasher = Sha256::new();
        hasher.update(&payload);
        let checksum = hasher.finalize();

        let mut file = self.file.lock().unwrap();
        let offset = file.seek(SeekFrom::End(0))?;

        let len_bytes = (payload.len() as u32).to_le_bytes();
        file.write_all(&len_bytes)?;
        file.write_all(&checksum)?;
        file.write_all(&payload)?;
        file.flush()?;

        Ok(offset)
    }

    /// Replays the entire WAL, verifying the cryptographic integrity of each frame.
    pub fn replay(&self) -> Result<Vec<WalRecord>> {
        let mut file = self.file.lock().unwrap();
        file.seek(SeekFrom::Start(0))?;

        let mut records = Vec::new();
        let mut len_buf = [0u8; 4];

        loop {
            match file.read_exact(&mut len_buf) {
                Ok(()) => {
                    let len = u32::from_le_bytes(len_buf) as usize;
                    let mut checksum = [0u8; 32];
                    file.read_exact(&mut checksum)
                        .context("Truncated WAL: missing checksum")?;

                    let mut payload = vec![0u8; len];
                    file.read_exact(&mut payload)
                        .context("Truncated WAL: missing payload")?;

                    let mut hasher = Sha256::new();
                    hasher.update(&payload);
                    let computed = hasher.finalize();

                    if computed.as_slice() != checksum {
                        bail!(
                            "WAL integrity checksum mismatch at record #{}",
                            records.len()
                        );
                    }

                    let record: WalRecord = serde_json::from_slice(&payload)
                        .context("Corrupt record payload JSON in WAL")?;
                    records.push(record);
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                    break;
                }
                Err(e) => return Err(e.into()),
            }
        }

        Ok(records)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}
