//! Cryptographic hashing primitives (L0 Kernel).
//!
//! Provides canonical SHA-256 digest computations and hex formatting
//! shared across artifacts, release verification, dedup, and state hashing.

use sha2::{Digest, Sha256};

/// Compute the lowercase hexadecimal SHA-256 digest of arbitrary bytes.
#[inline]
pub fn sha256_hex(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sha256_hex_known_vector() {
        // Known test vector: SHA-256 of empty string
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        // Known test vector: SHA-256 of "hello world"
        assert_eq!(
            sha256_hex(b"hello world"),
            "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9"
        );
    }
}
