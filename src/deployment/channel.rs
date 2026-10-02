//! Deployment channel: compile-time artifact identity.
//!
//! `DeploymentChannel` is independent from Cargo build profiles (`dev` vs
//! `release`). Valid combinations include development+debug,
//! development+release, and production+release. The channel is stamped at
//! compile time via the `development` cargo feature (production = default,
//! no feature) and can NEVER be changed at runtime by environment variables.

// P1-05: true mutual exclusion. Requesting both channel features is a
// compile error — never a silent deterministic choice. Every gate, script,
// workflow, and doc must therefore exercise each channel separately instead
// of `--all-features` (which would include both channel features).
#[cfg(all(feature = "development", feature = "production"))]
compile_error!(
    "M31A deployment channels are mutually exclusive: features `development` and `production` \
    must never be enabled together (e.g. via `--all-features`). Build each channel separately: \
    default features for production, `--features development` for development."
);

use serde::{Deserialize, Serialize};

/// Canonical deployment channel. Exactly two variants; no third channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DeploymentChannel {
    Development,
    Production,
}

impl DeploymentChannel {
    /// Compile-time current channel. `development` feature → Development,
    /// otherwise Production. No runtime override exists by design. Both
    /// channel features together are a compile error (see above), so exactly
    /// one channel is ever active — never a silent fallback.
    pub const fn current() -> Self {
        if cfg!(feature = "development") {
            Self::Development
        } else {
            Self::Production
        }
    }

    /// Parse a channel name. Fails closed on unknown values.
    pub fn parse(s: &str) -> Result<Self, ChannelError> {
        match s.trim().to_ascii_lowercase().as_str() {
            "development" | "dev" | "nightly" | "snapshot" => Ok(Self::Development),
            "production" | "prod" | "stable" => Ok(Self::Production),
            other => Err(ChannelError::Unknown(other.to_string())),
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Development => "development",
            Self::Production => "production",
        }
    }

    pub const fn is_development(self) -> bool {
        matches!(self, Self::Development)
    }

    pub const fn is_production(self) -> bool {
        matches!(self, Self::Production)
    }

    /// Binary/install name for this channel: `m31a` (production),
    /// `m31a-dev` (development). Same core runtime, distinct installation.
    pub const fn binary_name(self) -> &'static str {
        match self {
            Self::Development => "m31a-dev",
            Self::Production => "m31a",
        }
    }

    /// Global state directory suffix: production uses bare `m31a`,
    /// development uses `m31a-dev` (isolated config/data/cache/state).
    pub const fn app_dir_name(self) -> &'static str {
        match self {
            Self::Development => "m31a-dev",
            Self::Production => "m31a",
        }
    }

    /// Update feed for this channel. Production tracks `stable`,
    /// development tracks `development/nightly`.
    pub const fn update_channel(self) -> UpdateChannel {
        match self {
            Self::Development => UpdateChannel::Development,
            Self::Production => UpdateChannel::Stable,
        }
    }
}

impl std::fmt::Display for DeploymentChannel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for DeploymentChannel {
    type Err = ChannelError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

/// Update feed identity. Mirrors the deployment channel with feed naming:
/// production consumes `stable`, development consumes `development`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UpdateChannel {
    Development,
    Stable,
}

impl UpdateChannel {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Development => "development",
            Self::Stable => "stable",
        }
    }

    /// The deployment channel that normally consumes this feed.
    pub const fn deployment_channel(self) -> DeploymentChannel {
        match self {
            Self::Development => DeploymentChannel::Development,
            Self::Stable => DeploymentChannel::Production,
        }
    }
}

impl From<DeploymentChannel> for UpdateChannel {
    fn from(c: DeploymentChannel) -> Self {
        c.update_channel()
    }
}

impl std::fmt::Display for UpdateChannel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Typed channel failures.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ChannelError {
    #[error("unknown deployment channel '{0}'")]
    Unknown(String),
    #[error("channel error: {0}")]
    Conflict(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current_matches_feature_flag() {
        let c = DeploymentChannel::current();
        if cfg!(feature = "development") {
            assert_eq!(c, DeploymentChannel::Development);
        } else {
            assert_eq!(c, DeploymentChannel::Production);
        }
    }

    #[test]
    fn parse_table() {
        assert_eq!(
            DeploymentChannel::parse("development").unwrap(),
            DeploymentChannel::Development
        );
        assert_eq!(
            DeploymentChannel::parse("DEV").unwrap(),
            DeploymentChannel::Development
        );
        assert_eq!(
            DeploymentChannel::parse("stable").unwrap(),
            DeploymentChannel::Production
        );
        assert!(DeploymentChannel::parse("canary").is_err());
        assert!(DeploymentChannel::parse("").is_err());
    }

    #[test]
    fn binary_names_distinct() {
        assert_eq!(DeploymentChannel::Production.binary_name(), "m31a");
        assert_eq!(DeploymentChannel::Development.binary_name(), "m31a-dev");
    }

    #[test]
    fn update_channel_mapping() {
        assert_eq!(
            DeploymentChannel::Production.update_channel(),
            UpdateChannel::Stable
        );
        assert_eq!(
            DeploymentChannel::Development.update_channel(),
            UpdateChannel::Development
        );
    }
}
