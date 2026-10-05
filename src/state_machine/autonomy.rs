//! Five canonical runtime autonomy modes (AUT-01, AUT-02, AUT-03, D-13, SEC-06).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::fmt;

/// Five canonical autonomy modes (AUT-02).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, Default, Hash)]
#[serde(rename_all = "lowercase")]
pub enum AutonomyMode {
    #[default]
    Safe,
    Plan,
    Assisted,
    Autonomous,
    Unattended,
}

impl fmt::Display for AutonomyMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Safe => write!(f, "safe"),
            Self::Plan => write!(f, "plan"),
            Self::Assisted => write!(f, "assisted"),
            Self::Autonomous => write!(f, "autonomous"),
            Self::Unattended => write!(f, "unattended"),
        }
    }
}

impl std::str::FromStr for AutonomyMode {
    type Err = String;

    /// Parse a mode name. Fails on unknown input — callers must fail closed,
    /// never default an unparseable mode into execution latitude.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_lowercase().as_str() {
            "safe" => Ok(Self::Safe),
            "plan" => Ok(Self::Plan),
            "assisted" => Ok(Self::Assisted),
            "autonomous" => Ok(Self::Autonomous),
            "unattended" => Ok(Self::Unattended),
            other => Err(format!("unknown autonomy mode: '{other}'")),
        }
    }
}
