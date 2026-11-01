//! Dedicated test for `crates/m31a-diff/src/render.rs`.
//!
//! Validates `DiffRenderer::render_lines` gutter symbols, color styling,
//! line number formatting, span highlighting, and truncation.

use m31a_diff::engine::{DiffLine, DiffLineKind, WordSpan};
use m31a_diff::render::DiffRenderer;
use ratatui::style::{Color, Modifier};

#[test]
fn test_render_gutter_symbols_and_numbers() {
    let lines = vec![
        DiffLine {
            kind: DiffLineKind::Context,
            old_line_num: Some(10),
            new_line_num: Some(10),
            content: "context text".to_string(),
            word_spans: vec![WordSpan {
                text: "context text".to_string(),
                is_highlighted: false,
            }],
        },
        DiffLine {
            kind: DiffLineKind::Delete,
            old_line_num: Some(11),
            new_line_num: None,
            content: "deleted text".to_string(),
            word_spans: vec![WordSpan {
                text: "deleted text".to_string(),
                is_highlighted: true,
            }],
        },
        DiffLine {
            kind: DiffLineKind::Add,
            old_line_num: None,
            new_line_num: Some(11),
            content: "added text".to_string(),
            word_spans: vec![WordSpan {
                text: "added text".to_string(),
                is_highlighted: true,
            }],
        },
        DiffLine {
            kind: DiffLineKind::HunkHeader,
            old_line_num: None,
            new_line_num: None,
            content: "@@ -10,2 +10,2 @@".to_string(),
            word_spans: vec![WordSpan {
                text: "@@ -10,2 +10,2 @@".to_string(),
                is_highlighted: false,
            }],
        },
    ];

    let rendered = DiffRenderer::render_lines(&lines, 100);
    assert_eq!(rendered.len(), 4);

    // 0: Context
    let ctx_line = &rendered[0];
    assert!(ctx_line.spans[0].content.contains(" 10  10   "));
    assert_eq!(ctx_line.spans[0].style.fg, Some(Color::DarkGray));

    // 1: Delete
    let del_line = &rendered[1];
    assert!(del_line.spans[0].content.contains(" 11     - "));
    assert_eq!(del_line.spans[0].style.fg, Some(Color::Rgb(248, 81, 73)));
    assert!(
        del_line.spans[1]
            .style
            .add_modifier
            .contains(Modifier::BOLD)
    );
    assert!(
        del_line.spans[1]
            .style
            .add_modifier
            .contains(Modifier::UNDERLINED)
    );

    // 2: Add
    let add_line = &rendered[2];
    assert!(add_line.spans[0].content.contains("     11 + "));
    assert_eq!(add_line.spans[0].style.fg, Some(Color::Rgb(46, 160, 67)));
    assert!(
        add_line.spans[1]
            .style
            .add_modifier
            .contains(Modifier::BOLD)
    );
    assert!(
        add_line.spans[1]
            .style
            .add_modifier
            .contains(Modifier::UNDERLINED)
    );

    // 3: Hunk Header
    let hunk_line = &rendered[3];
    assert!(hunk_line.spans[0].content.contains("        @ "));
    assert_eq!(hunk_line.spans[0].style.fg, Some(Color::Cyan));
}

#[test]
fn test_render_max_lines_clamping() {
    let dummy_line = DiffLine {
        kind: DiffLineKind::Context,
        old_line_num: Some(1),
        new_line_num: Some(1),
        content: "test".to_string(),
        word_spans: vec![WordSpan {
            text: "test".to_string(),
            is_highlighted: false,
        }],
    };
    let lines = vec![dummy_line.clone(), dummy_line.clone(), dummy_line];

    let rendered_all = DiffRenderer::render_lines(&lines, 10);
    assert_eq!(rendered_all.len(), 3);

    let rendered_truncated = DiffRenderer::render_lines(&lines, 2);
    assert_eq!(rendered_truncated.len(), 2);

    let rendered_zero = DiffRenderer::render_lines(&lines, 0);
    assert_eq!(rendered_zero.len(), 0);
}
