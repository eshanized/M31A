# `m31a-image-generation`

Multimodal image generation client and visual asset synthesis pipeline for **M31A**.

---

## Overview

`m31a-image-generation` enables agents to generate, edit, and iterate on visual assets (mockups, diagrams, application icons, UI banners) directly within the coding workflow.

---

## User-Facing Capabilities

- **Generate Visual Assets**: Agents can generate UI illustrations, favicons, and graphic mockups directly from natural language prompts.
- **Local Artifact Caching**: Caches generated images locally in `.m31a/images/` with metadata attribution.

---

## Developer Guide & Architecture

### Key Types

- `ImageGenerationRequest`: Configures prompt, negative prompt, target dimensions, model provider, and quality settings.
- `GeneratedImage`: Contains image format (PNG/WebP/SVG), base64/binary bytes, seed, and file path.
- `ImageProvider` trait: Extensible interface for integrating external image synthesis backends.

---

## Testing

```bash
cargo test -p m31a-image-generation
```
