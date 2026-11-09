# `m31a-image-generation`

> Multimodal image generation client, asset synthesis pipeline, and dimension decoding for M31A.

---

## Why Does This Crate Exist?

Autonomous full-stack coding agents frequently build user-facing web and mobile applications. Often, an application requires placeholder graphics, app icons, favicons, hero illustrations, or diagram textures.

Rather than leaving broken `<img>` tags or requiring manual developer intervention, `m31a-image-generation` enables agents to generate visual assets programmatically directly into the workspace.

---

## Core Concepts & How It Works

### 1. Multi-Provider Generation
Provides an asynchronous trait abstraction for generating images:
- Supports PNG, JPEG, and WebP encoding.
- Configurable image resolutions (`512x512`, `1024x1024`), batch sizes, and quality presets.
- Returns binary buffers or writes files directly to target asset paths.

### 2. Dimension & Format Inspection
Includes offline image inspection utilities that read header bytes to determine image dimensions, format types, and color depth without invoking heavy external image libraries.

---

## Key Types & Structs

| Struct / Enum | Description |
| :--- | :--- |
| `ImageGenerationRequest` | Parameters: prompt, negative prompt, dimensions, format, seed. |
| `GeneratedImage` | Output data buffer, dimensions, MIME type, and local file path. |
| `ImageFormat` | Supported formats: `Png`, `Jpeg`, `Webp`. |
| `ImageQuality` | Quality settings: `Standard`, `High`, `Ultra`. |

---

## Practical Usage Example

```rust
use m31a_image_generation::{ImageFormat, ImageGenerationRequest, ImageQuality};

fn main() {
    let request = ImageGenerationRequest {
        prompt: "Minimalist dark-theme logo icon for Rust CLI tool, flat vector style".into(),
        negative_prompt: Some("blurry, low quality, text watermark".into()),
        model: "flux-schnell".into(),
        width: 512,
        height: 512,
        steps: Some(25),
        cfg_scale: Some(7.5),
        seed: Some(42),
        batch_size: Some(1),
        format: ImageFormat::Png,
        quality: ImageQuality::High,
    };

    println!("Image generation request prepared for model: {}", request.model);
}
```

---

## Testing & Verification

```bash
cargo test -p m31a-image-generation
```
