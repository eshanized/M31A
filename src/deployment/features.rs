//! Centralized channel-aware feature gating.
//!
//! Development-only capabilities are gated in exactly one place. Callers ask
//! `FeatureGate::check(feature)` and receive an explicit verdict — never a
//! string comparison on `"development"` scattered across subsystems.
//!
//! Invariant: gating controls *visibility/availability* of diagnostics and
//! experimental affordances. It NEVER bypasses policy, sandbox, capability,
//! approval, containment, or secret controls (§48).

use serde::{Deserialize, Serialize};

use super::channel::DeploymentChannel;

/// Development-gated features. Production rejects every variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DevelopmentFeature {
    /// Verbose/trace diagnostics beyond production defaults.
    VerboseDiagnostics,
    /// Experimental/unreleased UI components.
    ExperimentalUi,
    /// Developer-only commands (hidden from production help).
    DeveloperCommands,
    /// Development-only telemetry detail.
    DevelopmentTelemetry,
}

impl DevelopmentFeature {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::VerboseDiagnostics => "verbose-diagnostics",
            Self::ExperimentalUi => "experimental-ui",
            Self::DeveloperCommands => "developer-commands",
            Self::DevelopmentTelemetry => "development-telemetry",
        }
    }

    pub const fn all() -> [Self; 4] {
        [
            Self::VerboseDiagnostics,
            Self::ExperimentalUi,
            Self::DeveloperCommands,
            Self::DevelopmentTelemetry,
        ]
    }
}

/// Explicit availability verdict.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FeatureAvailability {
    pub enabled: bool,
    pub channel: DeploymentChannel,
    pub feature: DevelopmentFeature,
    pub reason: String,
}

/// Single gate for all development-only features.
#[derive(Debug, Clone, Copy)]
pub struct FeatureGate {
    channel: DeploymentChannel,
}

impl FeatureGate {
    pub fn new(channel: DeploymentChannel) -> Self {
        Self { channel }
    }

    pub fn current() -> Self {
        Self::new(DeploymentChannel::current())
    }

    /// Central availability check. Production always returns disabled with an
    /// explicit reason; development returns enabled. No security control is
    /// altered by this verdict.
    pub fn check(&self, feature: DevelopmentFeature) -> FeatureAvailability {
        match self.channel {
            DeploymentChannel::Development => FeatureAvailability {
                enabled: true,
                channel: self.channel,
                feature,
                reason: "enabled on development channel".to_string(),
            },
            DeploymentChannel::Production => FeatureAvailability {
                enabled: false,
                channel: self.channel,
                feature,
                reason: format!(
                    "'{}' is development-only and disabled on production",
                    feature.as_str()
                ),
            },
        }
    }

    pub fn is_enabled(&self, feature: DevelopmentFeature) -> bool {
        self.check(feature).enabled
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn production_rejects_all_dev_features() {
        let gate = FeatureGate::new(DeploymentChannel::Production);
        for f in DevelopmentFeature::all() {
            let v = gate.check(f);
            assert!(!v.enabled, "{f:?}");
            assert!(v.reason.contains("development-only"));
        }
    }

    #[test]
    fn development_enables_all_dev_features() {
        let gate = FeatureGate::new(DeploymentChannel::Development);
        for f in DevelopmentFeature::all() {
            assert!(gate.is_enabled(f));
        }
    }
}
