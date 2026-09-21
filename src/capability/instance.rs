//! Capability instance metadata and descriptor (CTL-03).

use crate::capability::family::CapabilityFamily;
use crate::capability::health::CapabilityHealthState;
use crate::capability::permissions::CapabilityPermissions;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Full descriptor of an installed capability instance (CTL-03).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityInstance {
    /// Unique instance identifier (e.g. "fs.local", "git.cli").
    pub id: String,
    /// Human-readable capability name.
    pub name: String,
    /// Semantic version of the capability provider.
    pub version: String,
    /// Core capability family this instance satisfies.
    pub family: CapabilityFamily,
    /// Concrete provider implementation name.
    pub provider_name: String,
    /// Strongly typed permissions and resource constraints.
    pub permissions: CapabilityPermissions,
    /// Current operational health state.
    pub health: CapabilityHealthState,
    /// Arbitrary metadata attributes (e.g. backend version, capabilities).
    pub metadata: HashMap<String, String>,
}

impl CapabilityInstance {
    /// Construct a new capability instance descriptor.
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        version: impl Into<String>,
        family: CapabilityFamily,
        provider_name: impl Into<String>,
        permissions: CapabilityPermissions,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            version: version.into(),
            family,
            provider_name: provider_name.into(),
            permissions,
            health: CapabilityHealthState::Healthy,
            metadata: HashMap::new(),
        }
    }

    /// Builder to attach metadata.
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }

    /// Builder to specify initial health state.
    pub fn with_health(mut self, health: CapabilityHealthState) -> Self {
        self.health = health;
        self
    }
}
