//! Multi-Resolution Viewport and Display Density Configuration.
//!
//! Provides deterministic viewport presets and custom matrix configurations
//! for responsive layout verification across mobile, tablet, desktop, and high-DPI displays.

use serde::{Deserialize, Serialize};

/// Configuration for a specific display viewport and device pixel ratio.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ViewportConfig {
    /// Human-readable label (e.g. "mobile-small", "tablet-portrait", "desktop-1080p")
    pub name: String,
    /// Physical width in logical CSS pixels
    pub width: u32,
    /// Physical height in logical CSS pixels
    pub height: u32,
    /// Device Pixel Ratio (DPR) / Scale factor (1.0 = standard, 2.0 = Retina/@2x, 3.0 = @3x)
    pub device_scale_factor: f64,
    /// Emulate touch screen and mobile user-agent quirks
    pub is_mobile: bool,
}

impl ViewportConfig {
    pub fn new(
        name: impl Into<String>,
        width: u32,
        height: u32,
        device_scale_factor: f64,
        is_mobile: bool,
    ) -> Self {
        Self {
            name: name.into(),
            width,
            height,
            device_scale_factor: if device_scale_factor <= 0.0 {
                1.0
            } else {
                device_scale_factor
            },
            is_mobile,
        }
    }

    /// Viewport identifier used in filenames and baseline keys.
    pub fn id(&self) -> String {
        format!("{}_{}x{}@{:.1}x", self.name, self.width, self.height, self.device_scale_factor)
            .replace(['/', '\\', ' ', ':'], "_")
    }
}

/// Standardized cross-device viewport presets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ViewportPreset {
    MobileSmall,
    MobileStandard,
    MobileLarge,
    TabletPortrait,
    TabletLandscape,
    DesktopStandard,
    DesktopFullHd,
    Desktop4k,
}

impl ViewportPreset {
    /// Converts preset to concrete `ViewportConfig`.
    pub fn to_config(&self) -> ViewportConfig {
        match self {
            Self::MobileSmall => ViewportConfig::new("mobile_small", 360, 640, 2.0, true),
            Self::MobileStandard => ViewportConfig::new("mobile_standard", 375, 667, 2.0, true),
            Self::MobileLarge => ViewportConfig::new("mobile_large", 414, 896, 3.0, true),
            Self::TabletPortrait => ViewportConfig::new("tablet_portrait", 768, 1024, 2.0, false),
            Self::TabletLandscape => ViewportConfig::new("tablet_landscape", 1024, 768, 2.0, false),
            Self::DesktopStandard => ViewportConfig::new("desktop_standard", 1280, 800, 1.0, false),
            Self::DesktopFullHd => ViewportConfig::new("desktop_1080p", 1920, 1080, 1.0, false),
            Self::Desktop4k => ViewportConfig::new("desktop_4k", 3840, 2160, 2.0, false),
        }
    }

    /// Standard suite covering common responsive tiers (Mobile, Tablet, Desktop).
    pub fn standard_matrix() -> Vec<ViewportConfig> {
        vec![
            Self::MobileStandard.to_config(),
            Self::TabletPortrait.to_config(),
            Self::DesktopFullHd.to_config(),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_viewport_config_id_formatting() {
        let vp = ViewportConfig::new("desktop_hd", 1920, 1080, 1.5, false);
        assert_eq!(vp.id(), "desktop_hd_1920x1080@1.5x");
    }

    #[test]
    fn test_viewport_presets_sanity() {
        let mobile = ViewportPreset::MobileStandard.to_config();
        assert!(mobile.is_mobile);
        assert_eq!(mobile.device_scale_factor, 2.0);

        let desktop = ViewportPreset::DesktopFullHd.to_config();
        assert!(!desktop.is_mobile);
        assert_eq!(desktop.width, 1920);
        assert_eq!(desktop.height, 1080);
    }
}
