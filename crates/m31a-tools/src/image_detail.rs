//! Image detail normalization and vision capability sanitization.
//! Computes optimal image resolution and detail modes for vision-capable models.

use serde::{Deserialize, Serialize};

/// Fidelity level for multimodal vision image inputs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum ImageDetail {
    #[default]
    Auto,
    Low,
    High,
    Original,
}

/// Vision capabilities for a target model provider or model identifier.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelVisionCapabilities {
    pub supports_vision: bool,
    pub supports_original_detail: bool,
    pub max_image_dimension: u32,
}

impl Default for ModelVisionCapabilities {
    fn default() -> Self {
        Self {
            supports_vision: true,
            supports_original_detail: false,
            max_image_dimension: 2048,
        }
    }
}

impl ModelVisionCapabilities {
    pub fn claude_3_5_sonnet() -> Self {
        Self {
            supports_vision: true,
            supports_original_detail: true,
            max_image_dimension: 4096,
        }
    }

    pub fn gpt_4o() -> Self {
        Self {
            supports_vision: true,
            supports_original_detail: false,
            max_image_dimension: 2048,
        }
    }
}

/// Normalizes the requested image detail based on the model's capabilities.
/// Downgrades `Original` to `None`/`Auto` if the model does not support lossless original detail.
pub fn normalize_output_image_detail(
    caps: &ModelVisionCapabilities,
    detail: Option<ImageDetail>,
) -> Option<ImageDetail> {
    match detail {
        Some(ImageDetail::Original) if caps.supports_original_detail => Some(ImageDetail::Original),
        Some(ImageDetail::Original) => Some(ImageDetail::Auto),
        Some(ImageDetail::Auto | ImageDetail::Low | ImageDetail::High) => detail,
        None => None,
    }
}

/// Traverses a JSON payload in place and sanitizes any occurrences of `"detail": "original"`
/// down to `"detail": "auto"` if the target model does not support original detail.
pub fn sanitize_image_detail_in_json(
    caps: &ModelVisionCapabilities,
    value: &mut serde_json::Value,
) {
    if caps.supports_original_detail {
        return;
    }

    match value {
        serde_json::Value::Object(map) => {
            if let Some(detail_val) = map.get_mut("detail")
                && detail_val.as_str() == Some("original")
            {
                *detail_val = serde_json::Value::String("auto".to_string());
            }
            for v in map.values_mut() {
                sanitize_image_detail_in_json(caps, v);
            }
        }
        serde_json::Value::Array(arr) => {
            for v in arr {
                sanitize_image_detail_in_json(caps, v);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_normalize_output_image_detail() {
        let standard_caps = ModelVisionCapabilities::default();
        assert_eq!(
            normalize_output_image_detail(&standard_caps, Some(ImageDetail::Original)),
            Some(ImageDetail::Auto)
        );
        assert_eq!(
            normalize_output_image_detail(&standard_caps, Some(ImageDetail::High)),
            Some(ImageDetail::High)
        );

        let advanced_caps = ModelVisionCapabilities::claude_3_5_sonnet();
        assert_eq!(
            normalize_output_image_detail(&advanced_caps, Some(ImageDetail::Original)),
            Some(ImageDetail::Original)
        );
    }

    #[test]
    fn test_sanitize_image_detail_in_json() {
        let standard_caps = ModelVisionCapabilities::default();
        let mut payload = json!({
            "type": "image_url",
            "image_url": {
                "url": "data:image/png;base64,iVBORw0KGgo...",
                "detail": "original"
            }
        });

        sanitize_image_detail_in_json(&standard_caps, &mut payload);
        assert_eq!(payload["image_url"]["detail"], "auto");

        // Advanced caps preserves original
        let advanced_caps = ModelVisionCapabilities::claude_3_5_sonnet();
        let mut original_payload = json!({
            "detail": "original"
        });
        sanitize_image_detail_in_json(&advanced_caps, &mut original_payload);
        assert_eq!(original_payload["detail"], "original");
    }
}
