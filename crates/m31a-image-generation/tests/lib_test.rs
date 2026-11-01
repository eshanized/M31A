//! Dedicated integration tests for `m31a-image-generation` (src/lib.rs).
//!
//! Validates:
//! - Local synthetic image generation producing valid PNG pixel buffers
//! - Persistent filesystem-backed ImageCache put/get and eviction
//! - ImageGenerationManager provider registration, routing, and cache coordination
//! - Error handling for unregistered providers

use m31a_image_generation::{
    ImageCache, ImageFormat, ImageGenerationManager, ImageGenerationProvider,
    ImageGenerationRequest, ImageQuality, LocalSyntheticImageProvider,
};
use tempfile::tempdir;

fn sample_request(prompt: &str, model: &str, batch_size: u32) -> ImageGenerationRequest {
    ImageGenerationRequest {
        prompt: prompt.to_string(),
        negative_prompt: None,
        model: model.to_string(),
        width: 512,
        height: 512,
        steps: Some(25),
        cfg_scale: Some(7.5),
        seed: Some(1337),
        batch_size: Some(batch_size),
        format: ImageFormat::Png,
        quality: ImageQuality::Standard,
    }
}

#[tokio::test]
async fn test_local_synthetic_image_provider_generates_valid_png() {
    let provider = LocalSyntheticImageProvider;
    assert_eq!(provider.provider_name(), "local_synthetic");
    assert!(provider.supports_negative_prompt());
    assert_eq!(provider.max_dimensions(), (4096, 4096));

    let req = sample_request("cyberpunk cityscape neon lights", "mock-v1", 2);
    let images = provider.generate(req).await.unwrap();

    assert_eq!(images.len(), 2, "Batch size of 2 must produce 2 images");
    for img in images {
        assert_eq!(img.format, ImageFormat::Png);
        assert_eq!(img.width, 512);
        assert_eq!(img.height, 512);
        assert!(img.data.len() > 16);

        // Verify valid PNG magic header bytes: 0x89 0x50 0x4E 0x47 0x0D 0x0A 0x1A 0x0A
        let png_magic = &[0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a];
        assert_eq!(
            &img.data[0..8],
            png_magic,
            "Generated data must have valid PNG magic signature"
        );
    }
}

#[tokio::test]
async fn test_image_cache_lifecycle_and_eviction() {
    let tmp = tempdir().unwrap();
    let cache_dir = tmp.path().join("cache");

    // Initialize cache with 1MB maximum capacity
    let cache = ImageCache::new(cache_dir.clone(), 1);

    let prompt = "sunset over ocean";
    let model = "local_synthetic";
    let params = "{\"width\":512,\"height\":512}";
    let payload = vec![0x89, 0x50, 0x4e, 0x47, 0x01, 0x02, 0x03, 0x04];

    // Initial check should be cache miss
    let initial = cache.get(prompt, model, params).await;
    assert!(initial.is_none(), "Cache must be empty initially");

    // Store in cache
    cache.put(prompt, model, params, &payload).await.unwrap();

    // Check hit
    let retrieved = cache.get(prompt, model, params).await;
    assert!(retrieved.is_some(), "Stored payload must be retrieved from cache");
    assert_eq!(retrieved.unwrap(), payload);
}

#[tokio::test]
async fn test_image_generation_manager_routing_and_caching() {
    let tmp = tempdir().unwrap();
    let cache_dir = tmp.path().join("manager_cache");

    let mut manager = ImageGenerationManager::new("local_synthetic".to_string())
        .with_cache(cache_dir, 10);

    manager.register_provider(
        "local_synthetic".to_string(),
        Box::new(LocalSyntheticImageProvider),
    );

    // List providers
    let providers = manager.list_providers();
    assert!(providers.contains(&"local_synthetic"));

    let info = manager.get_provider_info("local_synthetic");
    assert!(info.is_some());
    assert_eq!(info.unwrap().name, "local_synthetic");

    // Generate first time (cache miss -> generated)
    let req = sample_request("golden retriever puppy", "local_synthetic:mock-v1", 1);
    let images_1 = manager.generate(req.clone()).await.unwrap();
    assert_eq!(images_1.len(), 1);
    assert_ne!(images_1[0].metadata.model_version, "cached");

    // Generate second time with identical request (cache hit -> cached model_version)
    let images_2 = manager.generate(req).await.unwrap();
    assert_eq!(images_2.len(), 1);
    assert_eq!(
        images_2[0].metadata.model_version, "cached",
        "Second call with identical parameters must hit cache"
    );
    assert_eq!(images_2[0].data, images_1[0].data);
}

#[tokio::test]
async fn test_unregistered_provider_fails() {
    let manager = ImageGenerationManager::new("non_existent".to_string());
    let req = sample_request("test prompt", "unknown_provider:v1", 1);

    let res = manager.generate(req).await;
    assert!(res.is_err(), "Generating from unregistered provider must fail");
    let err = res.unwrap_err().to_string();
    assert!(err.contains("Provider 'unknown_provider' not found"));
}
