//! Visual Regression TDD v2 Integration and Verification Suite.

use image::{ImageBuffer, ImageFormat, Rgba, RgbaImage};
use m31a_verify::visual_tdd_v2::*;
use std::io::Cursor;
use tempfile::tempdir;

fn create_test_png(width: u32, height: u32, bg: Rgba<u8>) -> Vec<u8> {
    let img: RgbaImage = ImageBuffer::from_pixel(width, height, bg);
    let mut bytes = Vec::new();
    let mut cursor = Cursor::new(&mut bytes);
    img.write_to(&mut cursor, ImageFormat::Png).unwrap();
    bytes
}

fn create_test_png_with_diff(
    width: u32,
    height: u32,
    bg: Rgba<u8>,
    diff_box: (u32, u32, u32, u32),
    diff_color: Rgba<u8>,
) -> Vec<u8> {
    let mut img: RgbaImage = ImageBuffer::from_pixel(width, height, bg);
    let (min_x, min_y, max_x, max_y) = diff_box;
    for y in min_y..=max_y {
        for x in min_x..=max_x {
            if x < width && y < height {
                img.put_pixel(x, y, diff_color);
            }
        }
    }
    let mut bytes = Vec::new();
    let mut cursor = Cursor::new(&mut bytes);
    img.write_to(&mut cursor, ImageFormat::Png).unwrap();
    bytes
}

#[test]
fn test_multi_resolution_presets_and_matrix() {
    let matrix = ViewportPreset::standard_matrix();
    assert_eq!(matrix.len(), 3);
    assert_eq!(matrix[0].name, "mobile_standard");
    assert_eq!(matrix[0].device_scale_factor, 2.0);
    assert!(matrix[0].is_mobile);

    assert_eq!(matrix[1].name, "tablet_portrait");
    assert_eq!(matrix[1].width, 768);

    assert_eq!(matrix[2].name, "desktop_1080p");
    assert_eq!(matrix[2].width, 1920);
    assert_eq!(matrix[2].height, 1080);
}

#[test]
fn test_exact_and_perceptual_image_comparison() {
    let base_png = create_test_png(200, 200, Rgba([255, 255, 255, 255]));
    let actual_png = create_test_png(200, 200, Rgba([255, 255, 255, 255]));

    let (outcome, _) = VisualComparator::compare_bytes(
        &base_png,
        &actual_png,
        &MaskSet::new(),
        &ComparisonThreshold::strict(),
    )
    .unwrap();

    assert!(outcome.passed);
    assert_eq!(outcome.mismatched_pixels, 0);
    assert_eq!(outcome.mismatch_ratio, 0.0);
}

#[test]
fn test_visual_regression_detection_and_heatmap_generation() {
    let base_png = create_test_png(200, 200, Rgba([240, 240, 240, 255]));
    // 20x20 blue box in actual image
    let actual_png = create_test_png_with_diff(
        200,
        200,
        Rgba([240, 240, 240, 255]),
        (50, 50, 69, 69),
        Rgba([0, 100, 255, 255]),
    );

    let (outcome, diff_img) = VisualComparator::compare_bytes(
        &base_png,
        &actual_png,
        &MaskSet::new(),
        &ComparisonThreshold::strict(),
    )
    .unwrap();

    assert!(!outcome.passed);
    assert_eq!(outcome.mismatched_pixels, 400); // 20 * 20
    assert_eq!(outcome.bounding_boxes.len(), 1);
    let bbox = &outcome.bounding_boxes[0];
    assert_eq!(bbox.min_x, 50);
    assert_eq!(bbox.max_x, 69);
    assert_eq!(bbox.min_y, 50);
    assert_eq!(bbox.max_y, 69);

    // Annotate and encode heatmap
    let heatmap = VisualHeatmap::annotate_heatmap(diff_img, &outcome, &HeatmapOptions::default());
    let png_bytes = VisualHeatmap::to_png_bytes(&heatmap).unwrap();
    assert!(!png_bytes.is_empty());
    assert_eq!(&png_bytes[1..4], b"PNG");
}

#[test]
fn test_dynamic_region_masking_prevents_false_positives() {
    let base_png = create_test_png(200, 200, Rgba([255, 255, 255, 255]));
    // 20x20 diff area in timestamp section [x: 10..29, y: 10..29]
    let actual_png = create_test_png_with_diff(
        200,
        200,
        Rgba([255, 255, 255, 255]),
        (10, 10, 29, 29),
        Rgba([50, 50, 50, 255]),
    );

    // Without mask: fails
    let (outcome_unmasked, _) = VisualComparator::compare_bytes(
        &base_png,
        &actual_png,
        &MaskSet::new(),
        &ComparisonThreshold::strict(),
    )
    .unwrap();
    assert!(!outcome_unmasked.passed);
    assert_eq!(outcome_unmasked.mismatched_pixels, 400);

    // With mask covering [10, 10, 20, 20]: passes!
    let masks = MaskSet::new().with_rect(10, 10, 20, 20);
    let (outcome_masked, _) = VisualComparator::compare_bytes(
        &base_png,
        &actual_png,
        &masks,
        &ComparisonThreshold::strict(),
    )
    .unwrap();

    assert!(outcome_masked.passed);
    assert_eq!(outcome_masked.mismatched_pixels, 0);
    assert_eq!(outcome_masked.masked_pixels, 400);
}

#[test]
fn test_baseline_cryptographic_integrity_and_tampering_detection() {
    let dir = tempdir().unwrap();
    let store = BaselineStore::new(dir.path());
    let vp = ViewportConfig::new("desktop", 1280, 720, 1.0, false);

    let img1 = create_test_png(1280, 720, Rgba([255, 255, 255, 255]));
    let record = store
        .approve_baseline("dashboard_test", &vp, &img1, "security_team")
        .unwrap();

    // Verification check: valid
    let status = store.check_baseline_status("dashboard_test", &vp).unwrap();
    assert_eq!(status, BaselineStatus::Valid);

    // Corrupt baseline file directly on filesystem
    let baseline_file = dir.path().join(&record.relative_image_path);
    std::fs::write(&baseline_file, b"corrupted-invalid-bytes").unwrap();

    let tampered_status = store.check_baseline_status("dashboard_test", &vp).unwrap();
    match tampered_status {
        BaselineStatus::IntegrityMismatch { expected_sha256, actual_sha256 } => {
            assert_eq!(expected_sha256, record.sha256_hash);
            assert_ne!(actual_sha256, record.sha256_hash);
        }
        other => panic!("Expected IntegrityMismatch, got {:?}", other),
    }

    // Fail-closed retrieval
    assert!(store.get_baseline_image("dashboard_test", &vp).is_err());
}

#[test]
fn test_bounded_layout_repair_loop_enforces_limits() {
    let dir = tempdir().unwrap();
    let css_file = dir.path().join("app.css");
    std::fs::write(&css_file, "body { margin: 0; padding: 10px; }").unwrap();

    let mut coordinator = LayoutRepairCoordinator::new(RepairLoopConfig {
        max_attempts: 2,
        rollback_on_failure: true,
        require_approval: true,
    });

    coordinator.create_checkpoint(&css_file).unwrap();

    // Attempt 0 and 1 are permitted
    assert!(coordinator.assert_can_attempt(0).is_ok());
    assert!(coordinator.assert_can_attempt(1).is_ok());

    // Attempt 2 exceeds max_attempts (2) -> fails closed
    assert!(coordinator.assert_can_attempt(2).is_err());

    // Modify file
    std::fs::write(&css_file, "body { broken: true; }").unwrap();

    // Rollback restores original state
    coordinator.rollback_all().unwrap();
    assert_eq!(
        std::fs::read_to_string(&css_file).unwrap(),
        "body { margin: 0; padding: 10px; }"
    );
}

#[test]
fn test_structured_visual_regression_markdown_report() {
    let vp = ViewportPreset::DesktopStandard.to_config();
    let report = VisualRegressionReport {
        scenario_name: "Cart Checkout Page".to_string(),
        initial_url: "https://shop.example.com/checkout".to_string(),
        all_passed: false,
        total_viewports: 1,
        passed_viewports: 0,
        failed_viewports: 1,
        viewport_reports: vec![ViewportReport {
            viewport: vp,
            baseline_status: BaselineStatus::Valid,
            comparison: Some(ComparisonOutcome {
                passed: false,
                width: 1280,
                height: 800,
                total_evaluated_pixels: 1024000,
                masked_pixels: 0,
                mismatched_pixels: 5000,
                mismatch_ratio: 0.00488,
                max_color_distance: 0.95,
                mean_color_distance: 0.004,
                bounding_boxes: vec![DiffBoundingBox {
                    min_x: 100,
                    min_y: 200,
                    max_x: 200,
                    max_y: 250,
                }],
                dimension_mismatch: false,
                error_details: Some("Button shifted by 20px".to_string()),
            }),
            screenshot_path: None,
            heatmap_path: None,
            passed: false,
            failure_reason: Some("Button shifted by 20px".to_string()),
        }],
        self_healing_guidance: Some("Fix alignment on `.checkout-button`".to_string()),
    };

    let md = report.render_markdown_summary();
    assert!(md.contains("Visual Regression TDD v2 Report: `Cart Checkout Page`"));
    assert!(md.contains("REGRESSION DETECTED"));
    assert!(md.contains("0.488%"));
    assert!(md.contains("Layout Self-Healing Guidance"));
}
