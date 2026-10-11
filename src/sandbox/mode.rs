//! Authoritative sandbox mode vocabulary and lifecycle transitions (SND-04).

use serde::{Deserialize, Serialize};

/// Authoritative sandbox execution mode (SND-04).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Default,
)]
#[serde(rename_all = "snake_case")]
pub enum SandboxMode {
    /// Isolated execution required; network denied; fail closed if sandbox unavailable.
    #[default]
    Standard,
    /// Highest isolation; network denied; fail closed if sandbox unavailable.
    Strict,
    /// Best-effort isolation; network denied whenever sandboxed.
    Permissive,
    /// Unisolated execution (operator opt-out; autonomous tasks requiring isolation fail closed).
    Disabled,
}

impl SandboxMode {
    /// All valid sandbox modes in canonical representation.
    pub const ALL: &'static [SandboxMode] = &[
        SandboxMode::Standard,
        SandboxMode::Strict,
        SandboxMode::Permissive,
        SandboxMode::Disabled,
    ];

    /// Exact canonical string literal.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Standard => "standard",
            Self::Strict => "strict",
            Self::Permissive => "permissive",
            Self::Disabled => "disabled",
        }
    }

    /// Strength rank for monotonic security comparisons (higher is more restrictive).
    pub fn strength_rank(&self) -> u8 {
        match self {
            Self::Strict => 4,
            Self::Standard => 3,
            Self::Permissive => 2,
            Self::Disabled => 1,
        }
    }

    /// Determine if transitioning to `target` satisfies monotonic security (cannot relax).
    pub fn can_transition_monotonically_to(&self, target: SandboxMode) -> bool {
        target.strength_rank() >= self.strength_rank()
    }

    /// Check if this mode is strictly more restrictive than `other`.
    pub fn is_more_restrictive_than(&self, other: SandboxMode) -> bool {
        self.strength_rank() > other.strength_rank()
    }

    /// Authoritative wizard cycle transition.
    /// Standard -> Strict -> Permissive -> Disabled -> Standard.
    pub fn next_wizard_mode(&self) -> Self {
        match self {
            Self::Standard => Self::Strict,
            Self::Strict => Self::Permissive,
            Self::Permissive => Self::Disabled,
            Self::Disabled => Self::Standard,
        }
    }
}

impl std::str::FromStr for SandboxMode {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_lowercase().as_str() {
            "standard" => Ok(Self::Standard),
            "strict" => Ok(Self::Strict),
            "permissive" => Ok(Self::Permissive),
            "disabled" => Ok(Self::Disabled),
            other => Err(format!(
                "invalid sandbox_mode '{other}': expected one of standard, strict, permissive, disabled"
            )),
        }
    }
}

impl std::fmt::Display for SandboxMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sandbox_mode_roundtrip() {
        for mode in SandboxMode::ALL {
            let s = mode.as_str();
            let parsed: SandboxMode = s.parse().expect("must parse");
            assert_eq!(*mode, parsed);
            assert_eq!(parsed.to_string(), s);
        }
    }

    #[test]
    fn test_wizard_cycle_covers_all_modes() {
        let mut cur = SandboxMode::Standard;
        let mut visited = vec![cur];
        for _ in 0..3 {
            cur = cur.next_wizard_mode();
            visited.push(cur);
        }
        assert_eq!(visited, SandboxMode::ALL);
        assert_eq!(cur.next_wizard_mode(), SandboxMode::Standard);
    }

    #[test]
    fn test_invalid_mode_rejected() {
        assert!("invalid_mode".parse::<SandboxMode>().is_err());
    }
}
