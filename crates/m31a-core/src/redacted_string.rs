//! Credential wrapper with automatic redaction in formatting.
//!
//! Wraps sensitive secrets (API keys, authorization tokens, passwords) to guarantee
//! that `fmt::Debug` and `fmt::Display` output `"<redacted>"`, preventing accidental secret
//! leaks in logs, telemetry, panic traces, and error outputs.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::ops::Deref;

/// A string wrapper whose `Debug` and `Display` output is strictly redacted.
#[derive(Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RedactedString(String);

impl RedactedString {
    pub fn new(secret: impl Into<String>) -> Self {
        Self(secret.into())
    }

    /// Expose the underlying sensitive secret slice explicitly.
    pub fn expose_secret(&self) -> &str {
        &self.0
    }

    /// Consume the wrapper and return the underlying secret `String`.
    pub fn into_inner(self) -> String {
        self.0
    }

    /// Length in bytes of the underlying secret.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Returns true if the underlying secret is empty.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl Deref for RedactedString {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<String> for RedactedString {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for RedactedString {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl fmt::Debug for RedactedString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("<redacted>")
    }
}

impl fmt::Display for RedactedString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("<redacted>")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_redacted_string_debug_and_display() {
        let key = RedactedString::new("nvapi-secret-key-12345678");

        // Debug formatting MUST redact
        assert_eq!(format!("{:?}", key), "<redacted>");

        // Display formatting MUST redact
        assert_eq!(format!("{}", key), "<redacted>");

        // Explicit unwrap exposes secret
        assert_eq!(key.expose_secret(), "nvapi-secret-key-12345678");
        assert_eq!(key.len(), 25);
    }

    #[test]
    fn test_redacted_string_serde_roundtrip() {
        let original = RedactedString::new("super-secret-token");
        let json = serde_json::to_string(&original).unwrap();
        assert_eq!(json, r#""super-secret-token""#);

        let deserialized: RedactedString = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.expose_secret(), "super-secret-token");
    }
}
