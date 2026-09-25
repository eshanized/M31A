//! Strongly typed durable evidence URI linking for completion reports (RPT-03, D-13).

use crate::ids::{ArtifactId, CheckId, CheckpointId, EventId};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use std::str::FromStr;
use thiserror::Error;

/// Errors arising during evidence URI parsing or validation.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum EvidenceError {
    #[error("Invalid evidence URI format, expected '<scheme>://<locator>': '{0}'")]
    InvalidFormat(String),

    #[error("Unknown evidence URI scheme '{0}'")]
    UnknownScheme(String),

    #[error("Invalid identifier in evidence URI: {0}")]
    InvalidId(String),
}

/// Strongly typed evidence locator URI (RPT-03).
///
/// Supported schemes:
/// - `artifact://<artifact_id>`
/// - `check://<check_id>`
/// - `commit://<git_hash>`
/// - `checkpoint://<checkpoint_id>`
/// - `policy://<grant_id>`
/// - `event://<event_id>`
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum EvidenceUri {
    Artifact(ArtifactId),
    Check(CheckId),
    Commit(String),
    Checkpoint(CheckpointId),
    PolicyGrant(String),
    Event(EventId),
}

impl EvidenceUri {
    pub const SCHEME_ARTIFACT: &'static str = "artifact";
    pub const SCHEME_CHECK: &'static str = "check";
    pub const SCHEME_COMMIT: &'static str = "commit";
    pub const SCHEME_CHECKPOINT: &'static str = "checkpoint";
    pub const SCHEME_POLICY: &'static str = "policy";
    pub const SCHEME_EVENT: &'static str = "event";

    /// Parse an evidence URI from a string.
    pub fn parse(uri: &str) -> Result<Self, EvidenceError> {
        let (scheme, locator) = uri
            .split_once("://")
            .ok_or_else(|| EvidenceError::InvalidFormat(uri.to_string()))?;

        if locator.is_empty() {
            return Err(EvidenceError::InvalidFormat(uri.to_string()));
        }

        match scheme {
            Self::SCHEME_ARTIFACT => {
                let id = locator
                    .parse::<ArtifactId>()
                    .map_err(|e| EvidenceError::InvalidId(format!("{e}")))?;
                Ok(Self::Artifact(id))
            }
            Self::SCHEME_CHECK => {
                let id = locator
                    .parse::<CheckId>()
                    .map_err(|e| EvidenceError::InvalidId(format!("{e}")))?;
                Ok(Self::Check(id))
            }
            Self::SCHEME_COMMIT => {
                // Git commit hash
                if locator.chars().all(|c| c.is_ascii_hexdigit()) && locator.len() >= 7 {
                    Ok(Self::Commit(locator.to_string()))
                } else {
                    Err(EvidenceError::InvalidFormat(format!(
                        "Invalid commit hash: '{locator}'"
                    )))
                }
            }
            Self::SCHEME_CHECKPOINT => {
                let id = locator
                    .parse::<CheckpointId>()
                    .map_err(|e| EvidenceError::InvalidId(format!("{e}")))?;
                Ok(Self::Checkpoint(id))
            }
            Self::SCHEME_POLICY => Ok(Self::PolicyGrant(locator.to_string())),
            Self::SCHEME_EVENT => {
                let id = locator
                    .parse::<EventId>()
                    .map_err(|e| EvidenceError::InvalidId(format!("{e}")))?;
                Ok(Self::Event(id))
            }
            other => Err(EvidenceError::UnknownScheme(other.to_string())),
        }
    }

    /// Return the URI scheme name.
    pub fn scheme(&self) -> &'static str {
        match self {
            Self::Artifact(_) => Self::SCHEME_ARTIFACT,
            Self::Check(_) => Self::SCHEME_CHECK,
            Self::Commit(_) => Self::SCHEME_COMMIT,
            Self::Checkpoint(_) => Self::SCHEME_CHECKPOINT,
            Self::PolicyGrant(_) => Self::SCHEME_POLICY,
            Self::Event(_) => Self::SCHEME_EVENT,
        }
    }

    /// Return the inner locator string.
    pub fn locator(&self) -> String {
        match self {
            Self::Artifact(id) => id.to_string(),
            Self::Check(id) => id.to_string(),
            Self::Commit(hash) => hash.clone(),
            Self::Checkpoint(id) => id.to_string(),
            Self::PolicyGrant(grant) => grant.clone(),
            Self::Event(id) => id.to_string(),
        }
    }

    /// Return the full URI string.
    pub fn as_uri(&self) -> String {
        format!("{}://{}", self.scheme(), self.locator())
    }
}

impl fmt::Display for EvidenceUri {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_uri())
    }
}

impl FromStr for EvidenceUri {
    type Err = EvidenceError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

impl Serialize for EvidenceUri {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.as_uri())
    }
}

impl<'de> Deserialize<'de> for EvidenceUri {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        EvidenceUri::parse(&s).map_err(serde::de::Error::custom)
    }
}

/// Helper utility for creating, validating, and normalizing evidence links (RPT-03).
pub struct EvidenceLinker;

impl EvidenceLinker {
    /// Format an artifact locator into an evidence URI.
    pub fn artifact(id: ArtifactId) -> String {
        EvidenceUri::Artifact(id).as_uri()
    }

    /// Format a verification check locator into an evidence URI.
    pub fn check(id: CheckId) -> String {
        EvidenceUri::Check(id).as_uri()
    }

    /// Format a commit hash into an evidence URI.
    pub fn commit(hash: impl Into<String>) -> String {
        EvidenceUri::Commit(hash.into()).as_uri()
    }

    /// Format a checkpoint locator into an evidence URI.
    pub fn checkpoint(id: CheckpointId) -> String {
        EvidenceUri::Checkpoint(id).as_uri()
    }

    /// Format a policy grant locator into an evidence URI.
    pub fn policy_grant(grant_id: impl Into<String>) -> String {
        EvidenceUri::PolicyGrant(grant_id.into()).as_uri()
    }

    /// Format an event locator into an evidence URI.
    pub fn event(id: EventId) -> String {
        EvidenceUri::Event(id).as_uri()
    }

    /// Validate that a given string is a valid evidence URI.
    pub fn validate(uri: &str) -> Result<EvidenceUri, EvidenceError> {
        EvidenceUri::parse(uri)
    }

    /// Check if a string is a valid evidence URI.
    pub fn is_valid(uri: &str) -> bool {
        EvidenceUri::parse(uri).is_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_evidence_uri_roundtrip_all_schemes() {
        let art_id = ArtifactId::new();
        let chk_id = CheckId::new();
        let ckp_id = CheckpointId::new();
        let evt_id = EventId::new();

        let cases = vec![
            (
                EvidenceUri::Artifact(art_id),
                format!("artifact://{art_id}"),
            ),
            (EvidenceUri::Check(chk_id), format!("check://{chk_id}")),
            (
                EvidenceUri::Commit("abcdef0123456789".to_string()),
                "commit://abcdef0123456789".to_string(),
            ),
            (
                EvidenceUri::Checkpoint(ckp_id),
                format!("checkpoint://{ckp_id}"),
            ),
            (
                EvidenceUri::PolicyGrant("grant-123".to_string()),
                "policy://grant-123".to_string(),
            ),
            (EvidenceUri::Event(evt_id), format!("event://{evt_id}")),
        ];

        for (uri, expected_str) in cases {
            assert_eq!(uri.to_string(), expected_str);
            let parsed: EvidenceUri = expected_str.parse().expect("parsed correctly");
            assert_eq!(uri, parsed);

            let json = serde_json::to_string(&uri).unwrap();
            assert_eq!(json, format!("\"{expected_str}\""));
            let from_json: EvidenceUri = serde_json::from_str(&json).unwrap();
            assert_eq!(uri, from_json);
        }
    }

    #[test]
    fn test_invalid_evidence_uri_formats() {
        assert!(EvidenceUri::parse("not-a-uri").is_err());
        assert!(EvidenceUri::parse("unknown://12345").is_err());
        assert!(EvidenceUri::parse("artifact://not-a-uuid").is_err());
        assert!(EvidenceUri::parse("commit://xyz").is_err());
    }
}
