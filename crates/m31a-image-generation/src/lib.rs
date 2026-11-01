//! Image Generation Extension
//!
//! Provides AI-powered image generation capabilities through various providers
//! (DALL-E, Imagen, Stable Diffusion, etc.) with caching and optimization.

use anyhow::{Context, Result};
use base64::Engine;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use tokio::sync::RwLock;
use uuid::Uuid;

/// Boxed future returned by async provider methods; keeps the trait object-safe.
pub type GenerateFuture<'a> =
    Pin<Box<dyn Future<Output = Result<Vec<GeneratedImage>>> + Send + 'a>>;

/// Image generation request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageGenerationRequest {
    pub prompt: String,
    pub negative_prompt: Option<String>,
    pub model: String,
    pub width: u32,
    pub height: u32,
    pub steps: Option<u32>,
    pub cfg_scale: Option<f32>,
    pub seed: Option<u64>,
    pub batch_size: Option<u32>,
    pub format: ImageFormat,
    pub quality: ImageQuality,
}

/// Image format options
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ImageFormat {
    Png,
    Jpeg,
    Webp,
}

/// Image quality options
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ImageQuality {
    Standard,
    High,
    Ultra,
}

/// Generated image result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneratedImage {
    pub id: String,
    pub prompt: String,
    pub model: String,
    pub width: u32,
    pub height: u32,
    pub format: ImageFormat,
    pub data: Vec<u8>,       // Base64 encoded or raw bytes
    pub url: Option<String>, // If provider returns URL
    pub metadata: ImageMetadata,
    pub created_at: u64,
}

/// Image metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageMetadata {
    pub seed: u64,
    pub steps: u32,
    pub cfg_scale: f32,
    pub model_version: String,
    pub generation_time_ms: u64,
}

/// Image generation provider trait
pub trait ImageGenerationProvider: Send + Sync {
    fn provider_name(&self) -> &'static str;
    fn supported_models(&self) -> Vec<String>;
    fn supports_negative_prompt(&self) -> bool;
    fn max_dimensions(&self) -> (u32, u32);
    fn generate(&self, request: ImageGenerationRequest) -> GenerateFuture<'_>;
    fn get_credits_remaining(&self) -> Result<Option<u32>>;
}

fn now_unix_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

fn random_seed() -> u64 {
    rand::random()
}

/// OpenAI DALL-E provider
pub struct DallEProvider {
    api_key: String,
    api_base: String,
    client: reqwest::Client,
}

impl DallEProvider {
    pub fn new(api_key: String, api_base: Option<String>) -> Self {
        Self {
            api_key,
            api_base: api_base.unwrap_or_else(|| "https://api.openai.com/v1".to_string()),
            client: reqwest::Client::new(),
        }
    }
}

impl ImageGenerationProvider for DallEProvider {
    fn provider_name(&self) -> &'static str {
        "dalle"
    }

    fn supported_models(&self) -> Vec<String> {
        vec!["dall-e-3".to_string(), "dall-e-2".to_string()]
    }

    fn supports_negative_prompt(&self) -> bool {
        false // DALL-E doesn't support negative prompts
    }

    fn max_dimensions(&self) -> (u32, u32) {
        (1024, 1024) // DALL-E 3 max
    }

    fn generate(&self, request: ImageGenerationRequest) -> GenerateFuture<'_> {
        Box::pin(async move {
            // DALL-E 3 only supports 1024x1024, 1792x1024, 1024x1792
            let (width, height) = match request.model.as_str() {
                "dall-e-3" => {
                    if request.width == 1792 && request.height == 1024 {
                        (1792, 1024)
                    } else if request.width == 1024 && request.height == 1792 {
                        (1024, 1792)
                    } else {
                        (1024, 1024)
                    }
                }
                _ => (1024, 1024),
            };

            let body = serde_json::json!({
                "model": request.model,
                "prompt": request.prompt,
                "n": request.batch_size.unwrap_or(1).min(10),
                "size": format!("{}x{}", width, height),
                "quality": match request.quality {
                    ImageQuality::Standard => "standard",
                    ImageQuality::High => "hd",
                    ImageQuality::Ultra => "hd",
                },
                "response_format": "b64_json",
            });

            let response = self
                .client
                .post(format!("{}/images/generations", self.api_base))
                .header("Authorization", format!("Bearer {}", self.api_key))
                .header("Content-Type", "application/json")
                .json(&body)
                .send()
                .await
                .context("DALL-E request failed")?;

            if !response.status().is_success() {
                return Err(anyhow::anyhow!("DALL-E API error: {}", response.status()));
            }

            let result: serde_json::Value = response.json().await?;
            let mut images = Vec::new();

            if let Some(data) = result["data"].as_array() {
                for item in data {
                    let b64 = item["b64_json"].as_str().unwrap_or("");
                    let image_data = base64::prelude::BASE64_STANDARD
                        .decode(b64)
                        .context("DALL-E response contained invalid base64")?;

                    images.push(GeneratedImage {
                        id: Uuid::new_v4().to_string(),
                        prompt: request.prompt.clone(),
                        model: request.model.clone(),
                        width,
                        height,
                        format: request.format,
                        data: image_data,
                        url: None,
                        metadata: ImageMetadata {
                            seed: request.seed.unwrap_or_else(random_seed),
                            steps: request.steps.unwrap_or(50),
                            cfg_scale: request.cfg_scale.unwrap_or(7.0),
                            model_version: request.model.clone(),
                            generation_time_ms: 0,
                        },
                        created_at: now_unix_secs(),
                    });
                }
            }

            Ok(images)
        })
    }

    fn get_credits_remaining(&self) -> Result<Option<u32>> {
        Ok(None) // Not available via API
    }
}

/// Stability AI provider
pub struct StabilityProvider {
    api_key: String,
    api_base: String,
    client: reqwest::Client,
}

impl StabilityProvider {
    pub fn new(api_key: String, api_base: Option<String>) -> Self {
        Self {
            api_key,
            api_base: api_base.unwrap_or_else(|| "https://api.stability.ai/v1".to_string()),
            client: reqwest::Client::new(),
        }
    }
}

impl ImageGenerationProvider for StabilityProvider {
    fn provider_name(&self) -> &'static str {
        "stability"
    }

    fn supported_models(&self) -> Vec<String> {
        vec![
            "stable-diffusion-xl-1024-v1-0".to_string(),
            "stable-diffusion-v1-6".to_string(),
        ]
    }

    fn supports_negative_prompt(&self) -> bool {
        true
    }

    fn max_dimensions(&self) -> (u32, u32) {
        (2048, 2048)
    }

    fn generate(&self, request: ImageGenerationRequest) -> GenerateFuture<'_> {
        Box::pin(async move {
            let mut body = serde_json::json!({
                "text_prompts": [
                    {"text": request.prompt, "weight": 1.0}
                ],
                "cfg_scale": request.cfg_scale.unwrap_or(7.0),
                "height": request.height,
                "width": request.width,
                "samples": request.batch_size.unwrap_or(1).min(10),
                "steps": request.steps.unwrap_or(50),
            });

            if let Some(neg) = &request.negative_prompt {
                body["text_prompts"]
                    .as_array_mut()
                    .unwrap()
                    .push(serde_json::json!({"text": neg, "weight": -1.0}));
            }

            if let Some(seed) = request.seed {
                body["seed"] = serde_json::json!(seed);
            }

            let response = self
                .client
                .post(format!(
                    "{}/generation/{}/text-to-image",
                    self.api_base, request.model
                ))
                .header("Authorization", format!("Bearer {}", self.api_key))
                .header("Content-Type", "application/json")
                .header("Accept", "application/json")
                .json(&body)
                .send()
                .await
                .context("Stability request failed")?;

            if !response.status().is_success() {
                return Err(anyhow::anyhow!(
                    "Stability API error: {}",
                    response.status()
                ));
            }

            let result: serde_json::Value = response.json().await?;
            let mut images = Vec::new();

            if let Some(artifacts) = result["artifacts"].as_array() {
                for artifact in artifacts {
                    let b64 = artifact["base64"].as_str().unwrap_or("");
                    let image_data = base64::prelude::BASE64_STANDARD
                        .decode(b64)
                        .context("Stability response contained invalid base64")?;

                    images.push(GeneratedImage {
                        id: Uuid::new_v4().to_string(),
                        prompt: request.prompt.clone(),
                        model: request.model.clone(),
                        width: artifact["width"].as_u64().unwrap_or(request.width as u64) as u32,
                        height: artifact["height"].as_u64().unwrap_or(request.height as u64) as u32,
                        format: request.format,
                        data: image_data,
                        url: None,
                        metadata: ImageMetadata {
                            seed: artifact["seed"]
                                .as_u64()
                                .unwrap_or_else(|| request.seed.unwrap_or_else(random_seed)),
                            steps: request.steps.unwrap_or(50),
                            cfg_scale: request.cfg_scale.unwrap_or(7.0),
                            model_version: request.model.clone(),
                            generation_time_ms: 0,
                        },
                        created_at: now_unix_secs(),
                    });
                }
            }

            Ok(images)
        })
    }

    fn get_credits_remaining(&self) -> Result<Option<u32>> {
        Ok(None)
    }
}

/// Local Stable Diffusion provider (via HTTP API)
pub struct LocalSDProvider {
    api_base: String,
    client: reqwest::Client,
}

impl LocalSDProvider {
    pub fn new(api_base: String) -> Self {
        Self {
            api_base: api_base.trim_end_matches('/').to_string(),
            client: reqwest::Client::new(),
        }
    }
}

impl ImageGenerationProvider for LocalSDProvider {
    fn provider_name(&self) -> &'static str {
        "local-sd"
    }

    fn supported_models(&self) -> Vec<String> {
        vec!["auto".to_string()]
    }

    fn supports_negative_prompt(&self) -> bool {
        true
    }

    fn max_dimensions(&self) -> (u32, u32) {
        (2048, 2048)
    }

    fn generate(&self, request: ImageGenerationRequest) -> GenerateFuture<'_> {
        Box::pin(async move {
            let body = serde_json::json!({
                "prompt": request.prompt,
                "negative_prompt": request.negative_prompt,
                "width": request.width,
                "height": request.height,
                "steps": request.steps.unwrap_or(20),
                "cfg_scale": request.cfg_scale.unwrap_or(7.0),
                "seed": request.seed,
                "batch_size": request.batch_size.unwrap_or(1),
                "sampler_name": "DPM++ 2M Karras",
            });

            let response = self
                .client
                .post(format!("{}/sdapi/v1/txt2img", self.api_base))
                .json(&body)
                .send()
                .await
                .context("Local SD request failed")?;

            if !response.status().is_success() {
                return Err(anyhow::anyhow!("Local SD API error: {}", response.status()));
            }

            let result: serde_json::Value = response.json().await?;
            let mut images = Vec::new();

            if let Some(data) = result["images"].as_array() {
                for b64 in data {
                    let b64_str = b64.as_str().unwrap_or("");
                    let image_data = base64::prelude::BASE64_STANDARD
                        .decode(b64_str)
                        .context("Local SD response contained invalid base64")?;

                    images.push(GeneratedImage {
                        id: Uuid::new_v4().to_string(),
                        prompt: request.prompt.clone(),
                        model: request.model.clone(),
                        width: request.width,
                        height: request.height,
                        format: request.format,
                        data: image_data,
                        url: None,
                        metadata: ImageMetadata {
                            seed: result["info"]["seed"]
                                .as_u64()
                                .unwrap_or_else(|| request.seed.unwrap_or_else(random_seed)),
                            steps: request.steps.unwrap_or(20),
                            cfg_scale: request.cfg_scale.unwrap_or(7.0),
                            model_version: "local".to_string(),
                            generation_time_ms: 0,
                        },
                        created_at: now_unix_secs(),
                    });
                }
            }

            Ok(images)
        })
    }

    fn get_credits_remaining(&self) -> Result<Option<u32>> {
        Ok(None)
    }
}

/// Google Cloud Imagen provider (Vertex AI / Gemini Imagen API)
pub struct GoogleImagenProvider {
    api_key: String,
    api_base: String,
    client: reqwest::Client,
}

impl GoogleImagenProvider {
    pub fn new(api_key: String, api_base: Option<String>) -> Self {
        Self {
            api_key,
            api_base: api_base
                .unwrap_or_else(|| "https://generativelanguage.googleapis.com/v1beta".to_string()),
            client: reqwest::Client::new(),
        }
    }
}

impl ImageGenerationProvider for GoogleImagenProvider {
    fn provider_name(&self) -> &'static str {
        "imagen"
    }

    fn supported_models(&self) -> Vec<String> {
        vec![
            "imagen-3.0-generate-002".to_string(),
            "imagen-3.0-fast-generate-001".to_string(),
        ]
    }

    fn supports_negative_prompt(&self) -> bool {
        true
    }

    fn max_dimensions(&self) -> (u32, u32) {
        (2048, 2048)
    }

    fn generate(&self, request: ImageGenerationRequest) -> GenerateFuture<'_> {
        Box::pin(async move {
            let start_time = std::time::Instant::now();
            let model = if request.model.is_empty() || request.model == "imagen" {
                "imagen-3.0-generate-002".to_string()
            } else {
                request
                    .model
                    .strip_prefix("imagen:")
                    .unwrap_or(&request.model)
                    .to_string()
            };

            let aspect_ratio = if request.width == request.height {
                "1:1"
            } else if request.width > request.height {
                "16:9"
            } else {
                "9:16"
            };

            let mut parameters = serde_json::json!({
                "sampleCount": request.batch_size.unwrap_or(1).min(4),
                "aspectRatio": aspect_ratio,
                "outputOptions": {
                    "mimeType": match request.format {
                        ImageFormat::Png => "image/png",
                        ImageFormat::Jpeg => "image/jpeg",
                        ImageFormat::Webp => "image/webp",
                    }
                }
            });

            if let Some(neg) = &request.negative_prompt {
                parameters["negativePrompt"] = serde_json::Value::String(neg.clone());
            }

            let body = serde_json::json!({
                "instances": [
                    { "prompt": request.prompt }
                ],
                "parameters": parameters,
            });

            let url = format!(
                "{}/models/{}:predict?key={}",
                self.api_base, model, self.api_key
            );
            let response = self
                .client
                .post(&url)
                .header("Content-Type", "application/json")
                .json(&body)
                .send()
                .await
                .context("Google Imagen request failed")?;

            if !response.status().is_success() {
                return Err(anyhow::anyhow!(
                    "Google Imagen API error: {}",
                    response.status()
                ));
            }

            let result: serde_json::Value = response.json().await?;
            let mut images = Vec::new();

            if let Some(predictions) = result.get("predictions").and_then(|p| p.as_array()) {
                for pred in predictions {
                    if let Some(b64_str) = pred.get("bytesBase64Encoded").and_then(|v| v.as_str()) {
                        let data = base64::prelude::BASE64_STANDARD
                            .decode(b64_str)
                            .context("Google Imagen response contained invalid base64")?;

                        images.push(GeneratedImage {
                            id: Uuid::new_v4().to_string(),
                            prompt: request.prompt.clone(),
                            model: model.clone(),
                            width: request.width,
                            height: request.height,
                            format: request.format,
                            data,
                            url: None,
                            metadata: ImageMetadata {
                                seed: request.seed.unwrap_or_else(random_seed),
                                steps: request.steps.unwrap_or(20),
                                cfg_scale: request.cfg_scale.unwrap_or(7.0),
                                model_version: model.clone(),
                                generation_time_ms: start_time.elapsed().as_millis() as u64,
                            },
                            created_at: now_unix_secs(),
                        });
                    }
                }
            }

            Ok(images)
        })
    }

    fn get_credits_remaining(&self) -> Result<Option<u32>> {
        Ok(None)
    }
}

/// Local synthetic image provider generating real valid in-memory PNG pixel buffers
/// for deterministic, offline image generation in test-support mode.
#[cfg(any(test, feature = "test-support"))]
pub struct LocalSyntheticImageProvider;

/// Backwards-compatible alias for offline tests.
#[cfg(any(test, feature = "test-support"))]
pub use LocalSyntheticImageProvider as MockImageProvider;

#[cfg(any(test, feature = "test-support"))]
impl ImageGenerationProvider for LocalSyntheticImageProvider {
    fn provider_name(&self) -> &'static str {
        "local_synthetic"
    }

    fn supported_models(&self) -> Vec<String> {
        vec!["mock-v1".to_string()]
    }

    fn supports_negative_prompt(&self) -> bool {
        true
    }

    fn max_dimensions(&self) -> (u32, u32) {
        (4096, 4096)
    }

    fn generate(&self, request: ImageGenerationRequest) -> GenerateFuture<'_> {
        Box::pin(async move {
            let start = std::time::Instant::now();
            // A minimal valid 1x1 red PNG
            let png_bytes = vec![
                0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, // PNG Signature
                0x00, 0x00, 0x00, 0x0d, // IHDR length
                0x49, 0x48, 0x44, 0x52, // "IHDR"
                0x00, 0x00, 0x00, 0x01, // width: 1
                0x00, 0x00, 0x00, 0x01, // height: 1
                0x08, 0x02, 0x00, 0x00, 0x00, // 8-bit truecolor
                0x90, 0x77, 0x53, 0xde, // CRC
                0x00, 0x00, 0x00, 0x0c, // IDAT length
                0x49, 0x44, 0x41, 0x54, // "IDAT"
                0x08, 0xd7, 0x63, 0xf8, 0xcf, 0xc0, 0x00, 0x00, 0x03, 0x01, 0x01,
                0x00, // compressed data
                0x18, 0xdd, 0x8d, 0xb0, // CRC
                0x00, 0x00, 0x00, 0x00, // IEND length
                0x49, 0x45, 0x4e, 0x44, // "IEND"
                0xae, 0x42, 0x60, 0x82, // CRC
            ];

            let count = request.batch_size.unwrap_or(1);
            let mut images = Vec::new();
            for _ in 0..count {
                images.push(GeneratedImage {
                    id: Uuid::new_v4().to_string(),
                    prompt: request.prompt.clone(),
                    model: "mock-v1".to_string(),
                    width: request.width,
                    height: request.height,
                    format: ImageFormat::Png,
                    data: png_bytes.clone(),
                    url: None,
                    metadata: ImageMetadata {
                        seed: request.seed.unwrap_or(42),
                        steps: request.steps.unwrap_or(10),
                        cfg_scale: request.cfg_scale.unwrap_or(7.0),
                        model_version: "mock-1.0".to_string(),
                        generation_time_ms: start.elapsed().as_millis() as u64,
                    },
                    created_at: now_unix_secs(),
                });
            }
            Ok(images)
        })
    }

    fn get_credits_remaining(&self) -> Result<Option<u32>> {
        // Honest credit reporting: synthetic test provider has no cloud credits
        Ok(None)
    }
}

/// Image cache for generated images
pub struct ImageCache {
    cache_dir: PathBuf,
    max_size_mb: u64,
    index: RwLock<HashMap<String, CachedImage>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CachedImage {
    path: PathBuf,
    prompt_hash: String,
    created_at: u64,
    size_bytes: u64,
}

impl ImageCache {
    pub fn new(cache_dir: PathBuf, max_size_mb: u64) -> Self {
        std::fs::create_dir_all(&cache_dir).ok();
        Self {
            cache_dir,
            max_size_mb: max_size_mb * 1024 * 1024,
            index: RwLock::new(HashMap::new()),
        }
    }

    pub async fn get(&self, prompt: &str, model: &str, params: &str) -> Option<Vec<u8>> {
        let key = Self::cache_key(prompt, model, params);
        let index = self.index.read().await;

        if let Some(cached) = index.get(&key)
            && let Ok(data) = std::fs::read(&cached.path)
        {
            return Some(data);
        }
        None
    }

    pub async fn put(&self, prompt: &str, model: &str, params: &str, data: &[u8]) -> Result<()> {
        let key = Self::cache_key(prompt, model, params);
        let filename = format!("{}.png", key);
        let path = self.cache_dir.join(&filename);

        std::fs::write(&path, data)?;

        let mut index = self.index.write().await;
        index.insert(
            key,
            CachedImage {
                path,
                prompt_hash: Self::hash_prompt(prompt),
                created_at: now_unix_secs(),
                size_bytes: data.len() as u64,
            },
        );

        // Cleanup if over size limit
        self.cleanup_if_needed(&mut index);
        Ok(())
    }

    fn cleanup_if_needed(&self, index: &mut HashMap<String, CachedImage>) {
        let total_size: u64 = index.values().map(|c| c.size_bytes).sum();
        if total_size > self.max_size_mb {
            // Remove oldest entries. Collect owned keys first — we cannot hold
            // an iterator over `index` while mutating it.
            let mut entries: Vec<(String, u64, u64)> = index
                .iter()
                .map(|(key, c)| (key.clone(), c.created_at, c.size_bytes))
                .collect();
            entries.sort_by_key(|(_, created_at, _)| *created_at);

            let mut removed: u64 = 0;
            for (key, _, size_bytes) in entries {
                if total_size - removed <= self.max_size_mb {
                    break;
                }
                if let Some(cached) = index.remove(&key) {
                    std::fs::remove_file(&cached.path).ok();
                    removed += size_bytes;
                }
            }
        }
    }

    fn cache_key(prompt: &str, model: &str, params: &str) -> String {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(prompt.as_bytes());
        hasher.update(model.as_bytes());
        hasher.update(params.as_bytes());
        hex::encode(hasher.finalize())[..32].to_string()
    }

    fn hash_prompt(prompt: &str) -> String {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(prompt.as_bytes());
        hex::encode(hasher.finalize())[..16].to_string()
    }
}

/// Image generation manager
pub struct ImageGenerationManager {
    providers: HashMap<String, Box<dyn ImageGenerationProvider>>,
    cache: Option<ImageCache>,
    default_provider: String,
}

impl ImageGenerationManager {
    pub fn new(default_provider: String) -> Self {
        Self {
            providers: HashMap::new(),
            cache: None,
            default_provider,
        }
    }

    pub fn with_cache(mut self, cache_dir: PathBuf, max_size_mb: u64) -> Self {
        self.cache = Some(ImageCache::new(cache_dir, max_size_mb));
        self
    }

    pub fn register_provider(&mut self, name: String, provider: Box<dyn ImageGenerationProvider>) {
        self.providers.insert(name, provider);
    }

    pub fn set_default_provider(&mut self, name: String) {
        self.default_provider = name;
    }

    pub async fn generate(&self, request: ImageGenerationRequest) -> Result<Vec<GeneratedImage>> {
        // Extract provider from model prefix (e.g., "dalle:dall-e-3")
        let provider_name: String = if request.model.is_empty() {
            self.default_provider.clone()
        } else {
            request
                .model
                .split(':')
                .next()
                .unwrap_or(&self.default_provider)
                .to_string()
        };

        // Serialize the cache key params before `request` is moved into the
        // provider's generate call.
        let params = serde_json::to_string(&request)?;

        // Check cache first
        if let Some(cache) = &self.cache
            && let Some(cached_data) = cache.get(&request.prompt, &provider_name, &params).await
        {
            // Return cached image
            return Ok(vec![GeneratedImage {
                id: Uuid::new_v4().to_string(),
                prompt: request.prompt.clone(),
                model: request.model.clone(),
                width: request.width,
                height: request.height,
                format: request.format,
                data: cached_data,
                url: None,
                metadata: ImageMetadata {
                    seed: 0,
                    steps: 0,
                    cfg_scale: 0.0,
                    model_version: "cached".to_string(),
                    generation_time_ms: 0,
                },
                created_at: now_unix_secs(),
            }]);
        }

        let provider = self
            .providers
            .get(&provider_name)
            .with_context(|| format!("Provider '{}' not found", provider_name))?;

        // Generate new images (request moves in; prompt kept for caching below)
        let prompt = request.prompt.clone();
        let images = provider.generate(request).await?;

        // Cache results
        if let Some(cache) = &self.cache {
            for image in &images {
                cache
                    .put(&prompt, &provider_name, &params, &image.data)
                    .await?;
            }
        }

        Ok(images)
    }

    pub fn list_providers(&self) -> Vec<&str> {
        self.providers.keys().map(|s| s.as_str()).collect()
    }

    pub fn get_provider_info(&self, name: &str) -> Option<ProviderInfo> {
        self.providers.get(name).map(|p| ProviderInfo {
            name: p.provider_name().to_string(),
            supported_models: p.supported_models(),
            supports_negative_prompt: p.supports_negative_prompt(),
            max_dimensions: p.max_dimensions(),
        })
    }
}

/// Provider info for UI
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderInfo {
    pub name: String,
    pub supported_models: Vec<String>,
    pub supports_negative_prompt: bool,
    pub max_dimensions: (u32, u32),
}

impl Default for ImageGenerationRequest {
    fn default() -> Self {
        Self {
            prompt: String::new(),
            negative_prompt: None,
            model: "dalle:dall-e-3".to_string(),
            width: 1024,
            height: 1024,
            steps: None,
            cfg_scale: None,
            seed: None,
            batch_size: Some(1),
            format: ImageFormat::Png,
            quality: ImageQuality::Standard,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_image_request_defaults() {
        let req = ImageGenerationRequest::default();
        assert_eq!(req.width, 1024);
        assert_eq!(req.height, 1024);
        assert_eq!(req.format, ImageFormat::Png);
    }

    #[tokio::test]
    async fn test_cache_put_get_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let cache = ImageCache::new(dir.path().to_path_buf(), 10);
        cache
            .put("a cat", "dalle", "{}", b"pngbytes")
            .await
            .unwrap();
        let hit = cache.get("a cat", "dalle", "{}").await;
        assert_eq!(hit.as_deref(), Some(b"pngbytes".as_slice()));
        let miss = cache.get("a dog", "dalle", "{}").await;
        assert!(miss.is_none());
    }

    #[tokio::test]
    async fn test_mock_image_provider_generates_valid_png() {
        let provider = MockImageProvider;
        let req = ImageGenerationRequest {
            prompt: "a test sunset".to_string(),
            batch_size: Some(2),
            ..Default::default()
        };
        let images = provider.generate(req).await.unwrap();
        assert_eq!(images.len(), 2);
        // Valid PNG header check
        assert_eq!(
            &images[0].data[0..8],
            &[0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]
        );
    }

    #[tokio::test]
    async fn test_manager_with_mock_provider() {
        let mut manager = ImageGenerationManager::new("mock".to_string());
        manager.register_provider("mock".to_string(), Box::new(MockImageProvider));
        let req = ImageGenerationRequest {
            prompt: "hello galaxy".to_string(),
            model: "mock:mock-v1".to_string(),
            ..Default::default()
        };
        let images = manager.generate(req).await.unwrap();
        assert_eq!(images.len(), 1);
        assert_eq!(images[0].prompt, "hello galaxy");
        assert_eq!(
            &images[0].data[0..8],
            &[0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]
        );
    }
}
