//! Dedicated integration tests for `m31a-analytics` (src/charts.rs).
//!
//! Validates:
//! - Horizontal Unicode bar chart rendering with padding, filled blocks, and values
//! - Zero and narrow width edge cases for `render_bar`
//! - Unicode sparkline generation across scale extremes, empty inputs, and value trends

use m31a_analytics::charts::{render_bar, render_sparkline};

#[test]
fn test_render_bar_formatting_and_scaling() {
    let bar_half = render_bar("Tokens", 500, 1000, 50);
    assert!(bar_half.contains("Tokens"));
    assert!(bar_half.contains("500"));
    assert!(bar_half.contains('█'));
    assert!(bar_half.contains('░'));

    // 100% full bar
    let bar_full = render_bar("Full", 100, 100, 40);
    assert!(bar_full.contains("Full"));
    assert!(bar_full.contains("100"));
    assert!(bar_full.contains('█'));
    assert!(!bar_full.contains('░'), "100% bar should not contain empty blocks");

    // 0% bar
    let bar_zero = render_bar("Zero", 0, 100, 40);
    assert!(bar_zero.contains("Zero"));
    assert!(bar_zero.contains('░'));
    assert!(!bar_zero.contains('█'), "0% bar should not contain filled blocks");
}

#[test]
fn test_render_bar_boundary_conditions() {
    // max_val = 0
    let bar_zero_max = render_bar("Empty", 0, 0, 40);
    assert!(bar_zero_max.contains("Empty"));
    assert!(bar_zero_max.contains("[ ]"));

    // Ultra narrow width
    let bar_narrow = render_bar("VeryLongLabelHere", 100, 200, 10);
    assert!(bar_narrow.contains("[ ]"));
}

#[test]
fn test_render_sparkline_variations() {
    // Empty series
    assert_eq!(render_sparkline(&[]), "");

    // All zeros
    let spark_zeros = render_sparkline(&[0, 0, 0]);
    assert_eq!(spark_zeros, "   ");

    // Ascending monotonic series
    let values = [10, 25, 50, 75, 100];
    let spark = render_sparkline(&values);
    assert_eq!(spark.chars().count(), 5);

    let chars: Vec<char> = spark.chars().collect();
    // Earliest should be lower than latest
    assert!(chars[0] <= chars[1]);
    assert!(chars[1] <= chars[2]);
    assert!(chars[2] <= chars[3]);
    assert!(chars[3] <= chars[4]);
    assert_eq!(chars[4], '█', "Maximum value must map to full block");
}
