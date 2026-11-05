//! Image generation and editing extension tool.
//! Tool bridge for generating and editing images via configured image generation providers.

use anyhow::Result;
use m31a_image_generation::{
    DallEProvider, ImageFormat, ImageGenerationManager, ImageGenerationRequest as ProviderRequest,
    ImageQuality, LocalSDProvider, StabilityProvider,
};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use uuid::Uuid;

/// Extension-owned artifact directory name for generated images.
pub const GENERATED_IMAGE_ARTIFACTS_DIR: &str = "generated_images";

/// Maximum length of the output path hint displayed to the model.
pub const MAX_IMAGE_GENERATION_OUTPUT_HINT_BYTES: usize = 1024;

/// Sanitize input strings for safe filesystem paths.
pub fn sanitize_identifier(value: &str) -> String {
    let mut sanitized: String = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect();
    if sanitized.is_empty() {
        sanitized = "generated_image".to_string();
    }
    sanitized
}

/// Computes the extension-owned artifact path for a generated image.
pub fn image_generation_artifact_path(
    save_root: &Path,
    session_id: &str,
    call_id: &str,
) -> PathBuf {
    save_root
        .join(GENERATED_IMAGE_ARTIFACTS_DIR)
        .join(sanitize_identifier(session_id))
        .join(format!("{}.png", sanitize_identifier(call_id)))
}

/// Generates the model-facing output path hint.
pub fn image_generation_output_hint(
    image_output_dir: &Path,
    image_output_path: &Path,
) -> Option<String> {
    let hint = format!(
        "Generated images are saved to {} as {} by default.\nIf you need to use a generated image at another path, copy it and leave the original in place unless the user explicitly asks you to delete it.\nThe generated image is already displayed to the user. There is no need to render it in the final response as a Markdown image or file link.",
        image_output_dir.display(),
        image_output_path.display()
    );

    if hint.len() <= MAX_IMAGE_GENERATION_OUTPUT_HINT_BYTES {
        Some(hint)
    } else {
        None
    }
}

/// Request parameters for image generation or editing.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ImageGenerationRequest {
    pub prompt: String,
    pub size: Option<String>,
    pub quality: Option<String>,
    pub transparent_background: Option<bool>,
    pub referenced_image_paths: Option<Vec<String>>,
    pub num_last_images_to_include: Option<usize>,
}

/// Result of an image generation operation.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ImageGenerationResult {
    pub call_id: String,
    pub image_path: String,
    pub width: u32,
    pub height: u32,
    pub format: String,
    pub size_bytes: usize,
    pub hint: String,
}

/// Decodes actual image dimensions and format from PNG or JPEG byte streams.
pub fn decode_image_dimensions(bytes: &[u8]) -> Option<(u32, u32, &'static str)> {
    // 1. PNG Header and IHDR chunk
    if bytes.len() >= 24
        && bytes[0..8] == [0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]
        && &bytes[12..16] == b"IHDR"
    {
        let w = u32::from_be_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]);
        let h = u32::from_be_bytes([bytes[20], bytes[21], bytes[22], bytes[23]]);
        return Some((w, h, "png"));
    }

    // 2. JPEG SOI and SOF markers
    if bytes.len() >= 4 && bytes[0] == 0xFF && bytes[1] == 0xD8 {
        let mut i = 2;
        while i + 8 < bytes.len() {
            if bytes[i] != 0xFF {
                i += 1;
                continue;
            }
            let marker = bytes[i + 1];
            // SOF0..SOF3, SOF5..SOF7, SOF9..SOF11, SOF13..SOF15
            if (0xC0..=0xC3).contains(&marker)
                || (0xC5..=0xC7).contains(&marker)
                || (0xC9..=0xCB).contains(&marker)
                || (0xCD..=0xCF).contains(&marker)
            {
                let h = u16::from_be_bytes([bytes[i + 5], bytes[i + 6]]) as u32;
                let w = u16::from_be_bytes([bytes[i + 7], bytes[i + 8]]) as u32;
                return Some((w, h, "jpeg"));
            }
            let len = u16::from_be_bytes([bytes[i + 2], bytes[i + 3]]) as usize;
            if len < 2 {
                break;
            }
            i += 2 + len;
        }
    }

    None
}

/// Computes standard CRC-32 (IEEE 802.3) for PNG chunks.
fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &byte in data {
        crc ^= byte as u32;
        for _ in 0..8 {
            if (crc & 1) != 0 {
                crc = (crc >> 1) ^ 0xEDB8_8320;
            } else {
                crc >>= 1;
            }
        }
    }
    !crc
}

/// Computes Adler-32 checksum (RFC 1950) for zlib data streams.
fn adler32(data: &[u8]) -> u32 {
    let mut s1 = 1u32;
    let mut s2 = 0u32;
    for &byte in data {
        s1 = (s1 + byte as u32) % 65521;
        s2 = (s2 + s1) % 65521;
    }
    (s2 << 16) | s1
}

/// Synthesizes a valid, fully standards-compliant PNG file for given dimensions.
/// Uses stored (uncompressed) RFC 1951 deflate blocks inside an RFC 1950 zlib stream.
pub fn synthesize_valid_png(width: u32, height: u32) -> Vec<u8> {
    let mut png = Vec::new();
    // 1. PNG 8-byte Signature
    png.extend_from_slice(&[0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]);

    // 2. IHDR Chunk
    let mut ihdr_data = Vec::with_capacity(13);
    ihdr_data.extend_from_slice(&width.to_be_bytes());
    ihdr_data.extend_from_slice(&height.to_be_bytes());
    ihdr_data.push(8); // 8-bit depth
    ihdr_data.push(2); // RGB (3 bytes/pixel)
    ihdr_data.push(0); // compression deflate
    ihdr_data.push(0); // filter adaptive
    ihdr_data.push(0); // no interlace

    png.extend_from_slice(&(ihdr_data.len() as u32).to_be_bytes());
    let mut ihdr_full = Vec::with_capacity(4 + ihdr_data.len());
    ihdr_full.extend_from_slice(b"IHDR");
    ihdr_full.extend_from_slice(&ihdr_data);
    png.extend_from_slice(&ihdr_full);
    png.extend_from_slice(&crc32(&ihdr_full).to_be_bytes());

    // 3. IDAT Chunk: raw filtered scanlines (1 filter byte 0x00 + 3 bytes RGB per pixel)
    let row_len = 1 + (width as usize * 3);
    let mut raw_data = Vec::with_capacity(row_len * (height as usize));
    for y in 0..height {
        raw_data.push(0x00); // Filter type None
        let r = ((y * 128 / height.max(1)) + 64) as u8;
        let g = ((y * 180 / height.max(1)) + 80) as u8;
        let b = 220u8;
        for _ in 0..width {
            raw_data.push(r);
            raw_data.push(g);
            raw_data.push(b);
        }
    }

    // Build zlib wrapper (0x78, 0x01 = zlib default)
    let mut zlib_data = Vec::new();
    zlib_data.push(0x78);
    zlib_data.push(0x01);

    // Deflate stored blocks (max 65535 bytes per block)
    let chunks: Vec<&[u8]> = raw_data.chunks(65535).collect();
    for (i, chunk) in chunks.iter().enumerate() {
        let is_final = i == chunks.len() - 1;
        zlib_data.push(if is_final { 0x01 } else { 0x00 });
        let len = chunk.len() as u16;
        let nlen = !len;
        zlib_data.extend_from_slice(&len.to_le_bytes());
        zlib_data.extend_from_slice(&nlen.to_le_bytes());
        zlib_data.extend_from_slice(chunk);
    }
    // Adler-32 of uncompressed raw_data
    zlib_data.extend_from_slice(&adler32(&raw_data).to_be_bytes());

    png.extend_from_slice(&(zlib_data.len() as u32).to_be_bytes());
    let mut idat_full = Vec::with_capacity(4 + zlib_data.len());
    idat_full.extend_from_slice(b"IDAT");
    idat_full.extend_from_slice(&zlib_data);
    png.extend_from_slice(&idat_full);
    png.extend_from_slice(&crc32(&idat_full).to_be_bytes());

    // 4. IEND Chunk
    png.extend_from_slice(&0u32.to_be_bytes());
    let iend_full = b"IEND";
    png.extend_from_slice(iend_full);
    png.extend_from_slice(&crc32(iend_full).to_be_bytes());

    png
}

/// Parses image dimensions from standard size string, defaulting to (1024, 1024).
pub fn parse_requested_dimensions(size_str: Option<&str>) -> (u32, u32) {
    match size_str {
        Some("1536x1024") => (1536, 1024),
        Some("1024x1536") => (1024, 1536),
        Some("512x512") => (512, 512),
        Some("256x256") => (256, 256),
        _ => (1024, 1024),
    }
}

/// Builds an configured `ImageGenerationManager` based on environment credentials.
pub fn build_configured_image_manager() -> Option<(ImageGenerationManager, String)> {
    let mut manager = ImageGenerationManager::new("dalle".to_string());
    let mut configured_any = false;
    let mut preferred_model = "dall-e-3".to_string();

    if let Ok(api_key) = std::env::var("OPENAI_API_KEY")
        && !api_key.trim().is_empty()
    {
        let api_base = std::env::var("OPENAI_BASE_URL").ok();
        manager.register_provider(
            "dalle".to_string(),
            Box::new(DallEProvider::new(api_key, api_base)),
        );
        configured_any = true;
        preferred_model = "dalle:dall-e-3".to_string();
    }

    if let Ok(api_key) = std::env::var("STABILITY_API_KEY")
        && !api_key.trim().is_empty()
    {
        let api_base = std::env::var("STABILITY_BASE_URL").ok();
        manager.register_provider(
            "stability".to_string(),
            Box::new(StabilityProvider::new(api_key, api_base)),
        );
        if !configured_any {
            manager.set_default_provider("stability".to_string());
            preferred_model = "stability:stable-diffusion-xl-1024-v1-0".to_string();
            configured_any = true;
        }
    }

    if let Ok(local_url) = std::env::var("LOCAL_SD_URL")
        && !local_url.trim().is_empty()
    {
        manager.register_provider(
            "local-sd".to_string(),
            Box::new(LocalSDProvider::new(local_url)),
        );
        if !configured_any {
            manager.set_default_provider("local-sd".to_string());
            preferred_model = "local-sd:auto".to_string();
            configured_any = true;
        }
    }

    if configured_any {
        Some((manager, preferred_model))
    } else {
        None
    }
}

/// Executes an image generation or editing request.
pub async fn execute_image_generation(
    save_root: &Path,
    session_id: &str,
    req: ImageGenerationRequest,
) -> Result<ImageGenerationResult> {
    // Validation: cannot specify both referenced_image_paths and num_last_images_to_include
    if req.referenced_image_paths.is_some() && req.num_last_images_to_include.is_some() {
        anyhow::bail!(
            "Never provide both 'referenced_image_paths' and 'num_last_images_to_include' at the same time"
        );
    }

    if let Some(count) = req.num_last_images_to_include
        && (count == 0 || count > 5)
    {
        anyhow::bail!("'num_last_images_to_include' must be between 1 and 5");
    }

    // Validate referenced image paths against directory traversal
    if let Some(ref paths) = req.referenced_image_paths {
        for p in paths {
            if p.contains("..") {
                anyhow::bail!(
                    "Path traversal attempt rejected in referenced_image_paths: {}",
                    p
                );
            }
        }
    }

    // Reject path traversal in session_id
    if session_id.contains("..") {
        anyhow::bail!(
            "Path traversal attempt rejected in session_id: {}",
            session_id
        );
    }

    let call_id = format!("img_{}", Uuid::new_v4().simple());
    let target_path = image_generation_artifact_path(save_root, session_id, &call_id);

    // Validate target_path against directory traversal
    for comp in target_path.components() {
        if matches!(comp, std::path::Component::ParentDir) {
            anyhow::bail!("Path traversal detected in image generation target path");
        }
    }

    if let Some(parent) = target_path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    let (width, height) = parse_requested_dimensions(req.size.as_deref());

    // Host-controlled offline preview switch (never authorized by model arguments)
    let is_offline_preview = std::env::var("M31A_OFFLINE_PREVIEW")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);

    let (bytes, is_synthetic) = if is_offline_preview {
        (synthesize_valid_png(width, height), true)
    } else {
        let (manager, model_name) = build_configured_image_manager().ok_or_else(|| {
            anyhow::anyhow!(
                "No external AI image provider configured (e.g. OPENAI_API_KEY, STABILITY_API_KEY, or LOCAL_SD_URL). Image generation failed."
            )
        })?;

        let quality = if req.quality.as_deref() == Some("hd") {
            ImageQuality::High
        } else {
            ImageQuality::Standard
        };

        let prov_req = ProviderRequest {
            prompt: req.prompt.clone(),
            negative_prompt: None,
            model: model_name,
            width,
            height,
            steps: None,
            cfg_scale: None,
            seed: None,
            batch_size: Some(1),
            format: ImageFormat::Png,
            quality,
        };

        let images = manager.generate(prov_req).await.map_err(|e| {
            anyhow::anyhow!("External AI image provider failed to generate image: {e}")
        })?;

        if images.is_empty() {
            anyhow::bail!("Configured image provider returned an empty image set");
        }

        (images[0].data.clone(), false)
    };

    tokio::fs::write(&target_path, &bytes).await?;

    let output_dir = target_path.parent().unwrap_or(save_root);
    let mut hint = image_generation_output_hint(output_dir, &target_path)
        .unwrap_or_else(|| format!("Generated image saved to {}", target_path.display()));

    if is_synthetic {
        hint = format!("[SYNTHETIC TEST IMAGE] {}", hint);
    }

    // Decode actual dimensions from generated file bytes
    let (decoded_width, decoded_height, decoded_format) =
        decode_image_dimensions(&bytes).unwrap_or((width, height, "png"));

    Ok(ImageGenerationResult {
        call_id,
        image_path: target_path.to_string_lossy().to_string(),
        width: decoded_width,
        height: decoded_height,
        format: decoded_format.to_string(),
        size_bytes: bytes.len(),
        hint,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::Mutex;

    static ENV_MUTEX: Mutex<()> = Mutex::const_new(());

    #[test]
    fn test_artifact_path_sanitization() {
        let root = PathBuf::from("/tmp/m31a");
        let path = image_generation_artifact_path(&root, "session/with!special#chars", "call-123");
        assert_eq!(
            path,
            root.join("generated_images")
                .join("session_with_special_chars")
                .join("call-123.png")
        );
    }

    #[test]
    fn test_output_hint_formatting() {
        let dir = PathBuf::from("/tmp/m31a/generated_images/sess");
        let file = dir.join("img_1.png");
        let hint = image_generation_output_hint(&dir, &file).unwrap();
        assert!(hint.contains("Generated images are saved to"));
        assert!(hint.contains("img_1.png"));
    }

    #[test]
    fn test_synthesize_valid_png_header_and_ihdr() {
        let png = synthesize_valid_png(64, 48);
        assert_eq!(
            &png[0..8],
            &[0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]
        );
        // IHDR chunk begins at offset 12 with "IHDR"
        assert_eq!(&png[12..16], b"IHDR");
        let w = u32::from_be_bytes([png[16], png[17], png[18], png[19]]);
        let h = u32::from_be_bytes([png[20], png[21], png[22], png[23]]);
        assert_eq!(w, 64);
        assert_eq!(h, 48);
    }

    #[tokio::test]
    async fn test_execute_image_generation_offline_preview_success() {
        let _guard = ENV_MUTEX.lock().await;
        let tmp = tempfile::tempdir().unwrap();
        let req = ImageGenerationRequest {
            prompt: "A beautiful nebula with purple stars".to_string(),
            size: Some("1024x1024".to_string()),
            quality: Some("hd".to_string()),
            transparent_background: Some(false),
            referenced_image_paths: None,
            num_last_images_to_include: None,
        };

        unsafe { std::env::set_var("M31A_OFFLINE_PREVIEW", "1") };
        let result = execute_image_generation(tmp.path(), "test_session", req).await;
        unsafe { std::env::remove_var("M31A_OFFLINE_PREVIEW") };
        let result = result.expect("must succeed in offline preview");

        assert_eq!(result.width, 1024);
        assert_eq!(result.height, 1024);
        assert_eq!(result.format, "png");
        assert!(result.hint.contains("[SYNTHETIC TEST IMAGE]"));
        assert!(Path::new(&result.image_path).exists());

        // Verify valid PNG signature in written file
        let bytes = tokio::fs::read(&result.image_path).await.unwrap();
        assert_eq!(
            &bytes[0..8],
            &[0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]
        );
        let w = u32::from_be_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]);
        let h = u32::from_be_bytes([bytes[20], bytes[21], bytes[22], bytes[23]]);
        assert_eq!(w, 1024);
        assert_eq!(h, 1024);
    }

    #[tokio::test]
    async fn test_execute_image_generation_fails_without_provider() {
        let _guard = ENV_MUTEX.lock().await;
        // Save and remove provider env vars for this test
        let vars = [
            "OPENAI_API_KEY",
            "STABILITY_API_KEY",
            "LOCAL_SD_URL",
            "M31A_OFFLINE_PREVIEW",
        ];
        let saved: Vec<_> = vars.iter().map(|k| (*k, std::env::var(k).ok())).collect();
        for k in &vars {
            unsafe { std::env::remove_var(k) };
        }

        let tmp = tempfile::tempdir().unwrap();
        let req = ImageGenerationRequest {
            prompt: "A futuristic skyline".to_string(),
            size: Some("512x512".to_string()),
            quality: None,
            transparent_background: None,
            referenced_image_paths: None,
            num_last_images_to_include: None,
        };

        let err = execute_image_generation(tmp.path(), "test_session", req)
            .await
            .unwrap_err();

        // Restore env vars
        for (k, val) in saved {
            if let Some(v) = val {
                unsafe { std::env::set_var(k, v) };
            }
        }

        assert!(
            err.to_string()
                .contains("No external AI image provider configured")
        );
    }

    #[tokio::test]
    async fn test_execute_image_generation_conflict_error() {
        let tmp = tempfile::tempdir().unwrap();
        let req = ImageGenerationRequest {
            prompt: "Edit nebula".to_string(),
            size: None,
            quality: None,
            transparent_background: None,
            referenced_image_paths: Some(vec!["/tmp/nebula.png".to_string()]),
            num_last_images_to_include: Some(2),
        };

        let err = execute_image_generation(tmp.path(), "test_session", req)
            .await
            .unwrap_err();
        assert!(err.to_string().contains("Never provide both"));
    }

    #[tokio::test]
    async fn test_execute_image_generation_path_traversal_rejected() {
        let tmp = tempfile::tempdir().unwrap();
        let req = ImageGenerationRequest {
            prompt: "Test traversal".to_string(),
            size: None,
            quality: None,
            transparent_background: None,
            referenced_image_paths: None,
            num_last_images_to_include: None,
        };

        // 1. Path traversal in session_id
        let err1 = execute_image_generation(tmp.path(), "../../etc", req.clone())
            .await
            .unwrap_err();
        assert!(err1.to_string().contains("Path traversal"));

        // 2. Path traversal in referenced_image_paths
        let req_ref = ImageGenerationRequest {
            prompt: "Test traversal".to_string(),
            size: None,
            quality: None,
            transparent_background: None,
            referenced_image_paths: Some(vec!["../../secret.png".to_string()]),
            num_last_images_to_include: None,
        };
        let err2 = execute_image_generation(tmp.path(), "safe_session", req_ref)
            .await
            .unwrap_err();
        assert!(err2.to_string().contains("Path traversal"));
    }

    #[test]
    fn test_decode_image_dimensions_truthful() {
        // PNG decoding test
        let png = synthesize_valid_png(800, 600);
        let decoded = decode_image_dimensions(&png);
        assert_eq!(decoded, Some((800, 600, "png")));

        // Invalid bytes
        assert_eq!(decode_image_dimensions(&[0, 1, 2, 3]), None);
    }
}
