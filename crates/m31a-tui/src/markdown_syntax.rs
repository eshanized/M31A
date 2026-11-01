//! Lightweight heuristic syntax highlighter for markdown code blocks and inline code spans.
//!
//! Colors language keywords, strings, comments, and identifiers without heavy external parser dependencies.

use std::collections::HashMap;
use std::sync::RwLock;

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

#[allow(clippy::type_complexity)]
static LINE_HIGHLIGHT_CACHE: std::sync::LazyLock<
    RwLock<HashMap<(Option<String>, String), Vec<Span<'static>>>>,
> = std::sync::LazyLock::new(|| RwLock::new(HashMap::with_capacity(512)));

static MARKDOWN_DOC_CACHE: std::sync::LazyLock<RwLock<HashMap<String, Vec<Line<'static>>>>> =
    std::sync::LazyLock::new(|| RwLock::new(HashMap::with_capacity(256)));

/// Highlights a line of code based on simple heuristic tokenization with O(1) line cache.
pub fn highlight_code_line(line: &str, lang: Option<&str>) -> Vec<Span<'static>> {
    let key = (lang.map(|s| s.to_string()), line.to_string());
    if let Ok(cache) = LINE_HIGHLIGHT_CACHE.read()
        && let Some(cached) = cache.get(&key)
    {
        return cached.clone();
    }

    let spans = highlight_code_line_uncached(line, lang);

    if let Ok(mut cache) = LINE_HIGHLIGHT_CACHE.write() {
        if cache.len() > 4096 {
            cache.clear();
        }
        cache.insert(key, spans.clone());
    }

    spans
}

fn highlight_code_line_uncached(line: &str, lang: Option<&str>) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    let trimmed = line.trim_start();
    let indent_len = line.len() - trimmed.len();
    if indent_len > 0 {
        spans.push(Span::raw(" ".repeat(indent_len)));
    }

    // Comment detection
    let is_comment = match lang {
        Some("python") | Some("sh") | Some("bash") | Some("yaml") | Some("toml") => {
            trimmed.starts_with('#')
        }
        _ => trimmed.starts_with("//"),
    };

    let t = crate::theme::theme();
    if is_comment {
        spans.push(Span::styled(
            trimmed.to_string(),
            Style::default()
                .fg(t.color(crate::theme::ColorRole::CodeComment))
                .add_modifier(Modifier::ITALIC),
        ));
        return spans;
    }

    // Heuristic word-by-word tokenization
    let words = tokenize_code_line(trimmed);
    for token in words {
        let style = match token.kind {
            TokenKind::Keyword => Style::default()
                .fg(t.color(crate::theme::ColorRole::CodeKeyword))
                .add_modifier(Modifier::BOLD),
            TokenKind::Type => Style::default().fg(t.color(crate::theme::ColorRole::CodeType)),
            TokenKind::String => Style::default().fg(t.color(crate::theme::ColorRole::CodeString)),
            TokenKind::Number => Style::default().fg(t.color(crate::theme::ColorRole::CodeNumber)),
            TokenKind::Punctuation => {
                Style::default().fg(t.color(crate::theme::ColorRole::TextSecondary))
            }
            TokenKind::Ident => Style::default().fg(t.color(crate::theme::ColorRole::TextPrimary)),
            TokenKind::Whitespace => Style::default(),
        };
        spans.push(Span::styled(token.text, style));
    }

    spans
}

#[derive(Debug, PartialEq, Eq)]
enum TokenKind {
    Keyword,
    Type,
    String,
    Number,
    Punctuation,
    Ident,
    Whitespace,
}

struct CodeToken {
    text: String,
    kind: TokenKind,
}

fn tokenize_code_line(line: &str) -> Vec<CodeToken> {
    let mut tokens = Vec::new();
    let chars: Vec<char> = line.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        let c = chars[i];

        // Whitespace
        if c.is_whitespace() {
            let start = i;
            while i < chars.len() && chars[i].is_whitespace() {
                i += 1;
            }
            tokens.push(CodeToken {
                text: chars[start..i].iter().collect(),
                kind: TokenKind::Whitespace,
            });
            continue;
        }

        // Strings
        if c == '"' || c == '\'' || c == '`' {
            let quote = c;
            let start = i;
            i += 1;
            while i < chars.len() && chars[i] != quote {
                if chars[i] == '\\' && i + 1 < chars.len() {
                    i += 2;
                } else {
                    i += 1;
                }
            }
            if i < chars.len() {
                i += 1; // consume closing quote
            }
            tokens.push(CodeToken {
                text: chars[start..i].iter().collect(),
                kind: TokenKind::String,
            });
            continue;
        }

        // Numbers
        if c.is_ascii_digit() {
            let start = i;
            while i < chars.len()
                && (chars[i].is_ascii_digit()
                    || chars[i] == '.'
                    || chars[i] == 'x'
                    || chars[i] == '_')
            {
                i += 1;
            }
            tokens.push(CodeToken {
                text: chars[start..i].iter().collect(),
                kind: TokenKind::Number,
            });
            continue;
        }

        // Identifiers / Keywords / Types
        if c.is_alphabetic() || c == '_' {
            let start = i;
            while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
                i += 1;
            }
            let word: String = chars[start..i].iter().collect();
            let kind = if is_keyword(&word) {
                TokenKind::Keyword
            } else if is_type_name(&word) {
                TokenKind::Type
            } else {
                TokenKind::Ident
            };
            tokens.push(CodeToken { text: word, kind });
            continue;
        }

        // Punctuation / Operators
        tokens.push(CodeToken {
            text: c.to_string(),
            kind: TokenKind::Punctuation,
        });
        i += 1;
    }

    tokens
}

fn is_keyword(word: &str) -> bool {
    matches!(
        word,
        "fn" | "let"
            | "mut"
            | "pub"
            | "use"
            | "mod"
            | "struct"
            | "enum"
            | "impl"
            | "trait"
            | "type"
            | "const"
            | "static"
            | "if"
            | "else"
            | "match"
            | "for"
            | "in"
            | "while"
            | "loop"
            | "break"
            | "continue"
            | "return"
            | "async"
            | "await"
            | "def"
            | "class"
            | "import"
            | "from"
            | "as"
            | "with"
            | "self"
            | "Self"
            | "super"
            | "crate"
            | "true"
            | "false"
            | "None"
            | "Some"
            | "Ok"
            | "Err"
    )
}

fn is_type_name(word: &str) -> bool {
    if word.is_empty() {
        return false;
    }
    // PascalCase
    let first = word.chars().next().unwrap();
    first.is_uppercase()
        || matches!(
            word,
            "u8" | "u16"
                | "u32"
                | "u64"
                | "u128"
                | "usize"
                | "i8"
                | "i16"
                | "i32"
                | "i64"
                | "i128"
                | "isize"
                | "f32"
                | "f64"
                | "bool"
                | "str"
                | "char"
                | "int"
                | "float"
                | "string"
        )
}

/// Renders markdown formatted text into Ratatui `Line`s with bold, headers, lists, code fences, inline code, tables, alert callouts, and live mermaid diagrams.
pub fn render_markdown_to_lines(text: &str) -> Vec<Line<'static>> {
    if let Ok(cache) = MARKDOWN_DOC_CACHE.read()
        && let Some(cached) = cache.get(text)
    {
        return cached.clone();
    }

    let lines = render_markdown_to_lines_uncached(text);

    if let Ok(mut cache) = MARKDOWN_DOC_CACHE.write() {
        if cache.len() > 1024 {
            cache.clear();
        }
        cache.insert(text.to_string(), lines.clone());
    }

    lines
}

fn render_markdown_to_lines_uncached(text: &str) -> Vec<Line<'static>> {
    let healed_text = m31a_term::heal_streaming_markdown(text);
    let mut lines = Vec::new();
    let mut in_code_fence = false;
    let mut code_fence_lang: Option<String> = None;
    let mut code_fence_buffer: Vec<String> = Vec::new();

    let raw_lines: Vec<&str> = healed_text.lines().collect();
    let mut i = 0;

    while i < raw_lines.len() {
        let raw_line = raw_lines[i];
        let trimmed = raw_line.trim_end();

        if trimmed.starts_with("```") {
            if in_code_fence {
                // Closing code fence
                let lang_display = code_fence_lang.as_deref().unwrap_or("code");
                if lang_display.eq_ignore_ascii_case("mermaid") {
                    let mermaid_src = code_fence_buffer.join("\n");
                    let rendered = m31a_term::render_mermaid(&mermaid_src)
                        .or_else(|_| crate::mermaid::render(&mermaid_src, 120));

                    if let Ok(diagram) = rendered {
                        lines.push(Line::from(vec![Span::styled(
                            "   ╭── Mermaid Diagram ──────────────────────────",
                            Style::default().fg(Color::Cyan),
                        )]));
                        for diag_line in diagram.lines() {
                            lines.push(Line::from(vec![
                                Span::styled("   │ ", Style::default().fg(Color::Cyan)),
                                Span::styled(
                                    diag_line.to_string(),
                                    Style::default().fg(Color::LightCyan),
                                ),
                            ]));
                        }
                        lines.push(Line::from(vec![Span::styled(
                            "   ╰─────────────────────────────────────────────",
                            Style::default().fg(Color::Cyan),
                        )]));
                    } else {
                        // Render fallback lines
                        lines.push(Line::from(vec![Span::styled(
                            format!("   ╭── {} ───────────────────────────────", lang_display),
                            Style::default().fg(Color::DarkGray),
                        )]));
                        for item in &code_fence_buffer {
                            let mut highlighted =
                                highlight_code_line(item, code_fence_lang.as_deref());
                            let mut full_spans =
                                vec![Span::styled("   │ ", Style::default().fg(Color::DarkGray))];
                            full_spans.append(&mut highlighted);
                            lines.push(Line::from(full_spans));
                        }
                        lines.push(Line::from(vec![Span::styled(
                            "   ╰──────────────────────────────────────",
                            Style::default().fg(Color::DarkGray),
                        )]));
                    }
                } else if lang_display.eq_ignore_ascii_case("latex")
                    || lang_display.eq_ignore_ascii_case("math")
                {
                    let math_src = code_fence_buffer.join("\n");
                    let rendered = m31a_term::render_latex(&math_src);
                    lines.push(Line::from(vec![Span::styled(
                        "   ╭── LaTeX Math ───────────────────────────────",
                        Style::default().fg(Color::LightMagenta),
                    )]));
                    for math_line in rendered.lines() {
                        lines.push(Line::from(vec![
                            Span::styled("   │ ", Style::default().fg(Color::LightMagenta)),
                            Span::styled(
                                math_line.to_string(),
                                Style::default()
                                    .fg(Color::White)
                                    .add_modifier(Modifier::BOLD),
                            ),
                        ]));
                    }
                    lines.push(Line::from(vec![Span::styled(
                        "   ╰─────────────────────────────────────────────",
                        Style::default().fg(Color::LightMagenta),
                    )]));
                } else {
                    lines.push(Line::from(vec![Span::styled(
                        "   ╰──────────────────────────────────────",
                        Style::default().fg(Color::DarkGray),
                    )]));
                }
                in_code_fence = false;
                code_fence_lang = None;
                code_fence_buffer.clear();
            } else {
                // Opening code fence
                in_code_fence = true;
                let lang = trimmed.trim_start_matches('`').trim();
                code_fence_lang = if lang.is_empty() {
                    None
                } else {
                    Some(lang.to_string())
                };
                code_fence_buffer.clear();
                let lang_display = code_fence_lang.as_deref().unwrap_or("code");
                if !lang_display.eq_ignore_ascii_case("mermaid")
                    && !lang_display.eq_ignore_ascii_case("latex")
                    && !lang_display.eq_ignore_ascii_case("math")
                {
                    lines.push(Line::from(vec![Span::styled(
                        format!("   ╭── {} ───────────────────────────────", lang_display),
                        Style::default().fg(Color::DarkGray),
                    )]));
                }
            }
            i += 1;
            continue;
        }

        if in_code_fence {
            let is_special = code_fence_lang
                .as_deref()
                .map(|l| {
                    l.eq_ignore_ascii_case("mermaid")
                        || l.eq_ignore_ascii_case("latex")
                        || l.eq_ignore_ascii_case("math")
                })
                .unwrap_or(false);
            if is_special {
                code_fence_buffer.push(trimmed.to_string());
            } else {
                let mut highlighted = highlight_code_line(trimmed, code_fence_lang.as_deref());
                let mut full_spans =
                    vec![Span::styled("   │ ", Style::default().fg(Color::DarkGray))];
                full_spans.append(&mut highlighted);
                lines.push(Line::from(full_spans));
            }
            i += 1;
            continue;
        }

        // Horizontal Rules: ---, ***, ___
        let rule_trimmed = trimmed.trim();
        if rule_trimmed.len() >= 3
            && (rule_trimmed.chars().all(|c| c == '-')
                || rule_trimmed.chars().all(|c| c == '*')
                || rule_trimmed.chars().all(|c| c == '_'))
        {
            lines.push(Line::from(vec![Span::styled(
                "   ────────────────────────────────────────────────────────",
                Style::default().fg(Color::DarkGray),
            )]));
            i += 1;
            continue;
        }

        // Markdown Tables: detect start of table (header row followed by delimiter row)
        if is_table_header(trimmed) {
            if i + 1 < raw_lines.len() && is_table_delimiter(raw_lines[i + 1].trim()) {
                let mut table_rows = Vec::new();
                table_rows.push(parse_table_row(trimmed));
                i += 2; // skip header and delimiter

                while i < raw_lines.len() && is_table_row(raw_lines[i].trim()) {
                    table_rows.push(parse_table_row(raw_lines[i].trim()));
                    i += 1;
                }

                // If trailing row at the very tail of the stream is an incomplete table row, hold back during streaming
                if i < raw_lines.len()
                    && raw_lines[i].trim().starts_with('|')
                    && !is_table_row(raw_lines[i].trim())
                    && i + 1 == raw_lines.len()
                {
                    i += 1;
                }

                render_table(&table_rows, &mut lines);
                continue;
            } else if i + 1 >= raw_lines.len()
                || (i + 2 >= raw_lines.len() && raw_lines[i + 1].trim().starts_with('|'))
            {
                // Table header arrived, but delimiter row is still streaming or incomplete at tail
                // Hold back to prevent flickering raw pipe display
                i += 1;
                continue;
            }
        }

        // Incomplete table row at tail of streaming markdown
        if trimmed.starts_with('|') && i + 1 >= raw_lines.len() && !trimmed.ends_with('|') {
            i += 1;
            continue;
        }

        // GitHub Alert Callouts: > [!NOTE], > [!TIP], > [!IMPORTANT], > [!WARNING], > [!CAUTION]
        if let Some((alert_kind, alert_title)) = detect_alert_callout(trimmed) {
            let (border_color, icon) = alert_style(alert_kind);
            lines.push(Line::from(vec![
                Span::styled("   ╭── ", Style::default().fg(border_color)),
                Span::styled(
                    format!("{} {}", icon, alert_title),
                    Style::default()
                        .fg(border_color)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    " ──────────────────────────────────",
                    Style::default().fg(border_color),
                ),
            ]));

            i += 1;
            while i < raw_lines.len() {
                let next_line = raw_lines[i].trim();
                if let Some(content) = next_line.strip_prefix('>') {
                    let content_trimmed = content.trim();
                    let mut spans = vec![Span::styled("   │ ", Style::default().fg(border_color))];
                    spans.extend(parse_inline_markdown(content_trimmed));
                    lines.push(Line::from(spans));
                    i += 1;
                } else {
                    break;
                }
            }

            lines.push(Line::from(vec![Span::styled(
                "   ╰─────────────────────────────────────────────",
                Style::default().fg(border_color),
            )]));
            continue;
        }

        // Markdown Headers
        if let Some(rest) = trimmed.strip_prefix("### ") {
            let t = crate::theme::theme();
            lines.push(Line::from(vec![
                Span::styled(
                    "   ### ",
                    Style::default()
                        .fg(t.color(crate::theme::ColorRole::Accent))
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    rest.to_string(),
                    Style::default()
                        .fg(t.color(crate::theme::ColorRole::TextPrimary))
                        .add_modifier(Modifier::BOLD),
                ),
            ]));
            i += 1;
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("## ") {
            let t = crate::theme::theme();
            lines.push(Line::from(vec![
                Span::styled(
                    "   ## ",
                    Style::default()
                        .fg(t.color(crate::theme::ColorRole::Info))
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    rest.to_string(),
                    Style::default()
                        .fg(t.color(crate::theme::ColorRole::Accent))
                        .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
                ),
            ]));
            i += 1;
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("# ") {
            let t = crate::theme::theme();
            lines.push(Line::from(vec![
                Span::styled(
                    "   # ",
                    Style::default()
                        .fg(t.color(crate::theme::ColorRole::Warning))
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    rest.to_string(),
                    Style::default()
                        .fg(t.color(crate::theme::ColorRole::Warning))
                        .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
                ),
            ]));
            i += 1;
            continue;
        }

        // Task List Checkboxes: - [ ] or - [x] or * [ ] or * [x]
        if let Some(rest) = trimmed
            .strip_prefix("- [ ] ")
            .or_else(|| trimmed.strip_prefix("* [ ] "))
        {
            let mut spans = vec![
                Span::raw("   "),
                Span::styled("☐ ", Style::default().fg(Color::Yellow)),
            ];
            spans.extend(parse_inline_markdown(rest));
            lines.push(Line::from(spans));
            i += 1;
            continue;
        }
        if let Some(rest) = trimmed
            .strip_prefix("- [x] ")
            .or_else(|| trimmed.strip_prefix("- [X] "))
            .or_else(|| trimmed.strip_prefix("* [x] "))
            .or_else(|| trimmed.strip_prefix("* [X] "))
        {
            let mut spans = vec![
                Span::raw("   "),
                Span::styled(
                    "☑ ",
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                ),
            ];
            spans.extend(parse_inline_markdown(rest));
            lines.push(Line::from(spans));
            i += 1;
            continue;
        }

        // Bullet lists
        if let Some(rest) = trimmed
            .strip_prefix("- ")
            .or_else(|| trimmed.strip_prefix("* "))
        {
            let mut spans = vec![
                Span::raw("   "),
                Span::styled(
                    "• ",
                    Style::default()
                        .fg(Color::LightCyan)
                        .add_modifier(Modifier::BOLD),
                ),
            ];
            spans.extend(parse_inline_markdown(rest));
            lines.push(Line::from(spans));
            i += 1;
            continue;
        }

        // Numbered lists: 1. , 2. , etc.
        if let Some((num_str, rest)) = parse_numbered_list(trimmed) {
            let mut spans = vec![
                Span::raw("   "),
                Span::styled(
                    format!("{}. ", num_str),
                    Style::default()
                        .fg(Color::LightCyan)
                        .add_modifier(Modifier::BOLD),
                ),
            ];
            spans.extend(parse_inline_markdown(rest));
            lines.push(Line::from(spans));
            i += 1;
            continue;
        }

        // Blockquote (standard)
        if let Some(rest) = trimmed.strip_prefix("> ") {
            let mut spans = vec![Span::styled(
                "   │ ",
                Style::default().fg(Color::LightYellow),
            )];
            spans.extend(parse_inline_markdown(rest));
            lines.push(Line::from(spans));
            i += 1;
            continue;
        }

        // Normal paragraph line with inline markdown (code, bold)
        let mut spans = vec![Span::raw("   ")];
        spans.extend(parse_inline_markdown(trimmed));
        lines.push(Line::from(spans));
        i += 1;
    }

    lines
}

fn parse_numbered_list(line: &str) -> Option<(String, &str)> {
    let trimmed = line.trim_start();
    let dot_idx = trimmed.find(". ")?;
    let num_part = &trimmed[..dot_idx];
    if num_part.chars().all(|c| c.is_ascii_digit()) && !num_part.is_empty() {
        Some((num_part.to_string(), &trimmed[dot_idx + 2..]))
    } else {
        None
    }
}

// ===== Table Helper Functions =====

fn is_table_header(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.starts_with('|') && trimmed.ends_with('|') && trimmed.matches('|').count() >= 2
}

fn is_table_delimiter(line: &str) -> bool {
    let trimmed = line.trim();
    if !trimmed.starts_with('|') || !trimmed.ends_with('|') {
        return false;
    }
    trimmed
        .chars()
        .all(|c| c == '|' || c == '-' || c == ':' || c.is_whitespace())
}

fn is_table_row(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.starts_with('|') && trimmed.ends_with('|') && trimmed.matches('|').count() >= 2
}

fn parse_table_row(line: &str) -> Vec<String> {
    let trimmed = line.trim();
    let stripped = trimmed.trim_start_matches('|').trim_end_matches('|');
    stripped.split('|').map(|s| s.trim().to_string()).collect()
}

fn render_table(rows: &[Vec<String>], lines: &mut Vec<Line<'static>>) {
    if rows.is_empty() {
        return;
    }

    let col_count = rows.iter().map(|r| r.len()).max().unwrap_or(0);
    if col_count == 0 {
        return;
    }

    // Determine max width for each column (minimum 4 chars, max 40 chars)
    let mut col_widths = vec![4usize; col_count];
    for row in rows {
        for (col_idx, cell) in row.iter().enumerate() {
            if col_idx < col_count {
                col_widths[col_idx] = col_widths[col_idx].max(cell.chars().count()).min(40);
            }
        }
    }

    // Top border: ┌───┬───┐
    let mut top_border = String::from("   ┌");
    for (idx, w) in col_widths.iter().enumerate() {
        top_border.push_str(&"─".repeat(w + 2));
        if idx + 1 < col_widths.len() {
            top_border.push('┬');
        } else {
            top_border.push('┐');
        }
    }
    lines.push(Line::from(Span::styled(
        top_border,
        Style::default().fg(Color::DarkGray),
    )));

    // Header row
    if let Some(header) = rows.first() {
        let mut header_spans = vec![Span::styled("   │", Style::default().fg(Color::DarkGray))];
        for (col_idx, w) in col_widths.iter().enumerate() {
            let cell = header.get(col_idx).map(|s| s.as_str()).unwrap_or("");
            let pad = w.saturating_sub(cell.chars().count());
            header_spans.push(Span::styled(
                format!(" {} ", cell),
                Style::default()
                    .fg(Color::LightCyan)
                    .add_modifier(Modifier::BOLD),
            ));
            if pad > 0 {
                header_spans.push(Span::raw(" ".repeat(pad)));
            }
            header_spans.push(Span::styled("│", Style::default().fg(Color::DarkGray)));
        }
        lines.push(Line::from(header_spans));

        // Separator between header and data rows: ├───┼───┤
        let mut mid_border = String::from("   ├");
        for (idx, w) in col_widths.iter().enumerate() {
            mid_border.push_str(&"─".repeat(w + 2));
            if idx + 1 < col_widths.len() {
                mid_border.push('┼');
            } else {
                mid_border.push('┤');
            }
        }
        lines.push(Line::from(Span::styled(
            mid_border,
            Style::default().fg(Color::DarkGray),
        )));
    }

    // Data rows
    for row in rows.iter().skip(1) {
        let mut row_spans = vec![Span::styled("   │", Style::default().fg(Color::DarkGray))];
        for (col_idx, w) in col_widths.iter().enumerate() {
            let cell = row.get(col_idx).map(|s| s.as_str()).unwrap_or("");
            let pad = w.saturating_sub(cell.chars().count());
            row_spans.push(Span::styled(
                format!(" {} ", cell),
                Style::default().fg(Color::White),
            ));
            if pad > 0 {
                row_spans.push(Span::raw(" ".repeat(pad)));
            }
            row_spans.push(Span::styled("│", Style::default().fg(Color::DarkGray)));
        }
        lines.push(Line::from(row_spans));
    }

    // Bottom border: └───┴───┘
    let mut bot_border = String::from("   └");
    for (idx, w) in col_widths.iter().enumerate() {
        bot_border.push_str(&"─".repeat(w + 2));
        if idx + 1 < col_widths.len() {
            bot_border.push('┴');
        } else {
            bot_border.push('┘');
        }
    }
    lines.push(Line::from(Span::styled(
        bot_border,
        Style::default().fg(Color::DarkGray),
    )));
}

// ===== Alert Callout Helper Functions =====

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AlertKind {
    Note,
    Tip,
    Important,
    Warning,
    Caution,
}

fn detect_alert_callout(line: &str) -> Option<(AlertKind, &'static str)> {
    let trimmed = line.trim();
    if !trimmed.starts_with('>') {
        return None;
    }
    let after = trimmed[1..].trim();
    if after.eq_ignore_ascii_case("[!NOTE]") {
        Some((AlertKind::Note, "Note"))
    } else if after.eq_ignore_ascii_case("[!TIP]") {
        Some((AlertKind::Tip, "Tip"))
    } else if after.eq_ignore_ascii_case("[!IMPORTANT]") {
        Some((AlertKind::Important, "Important"))
    } else if after.eq_ignore_ascii_case("[!WARNING]") {
        Some((AlertKind::Warning, "Warning"))
    } else if after.eq_ignore_ascii_case("[!CAUTION]") {
        Some((AlertKind::Caution, "Caution"))
    } else {
        None
    }
}

fn alert_style(kind: AlertKind) -> (Color, &'static str) {
    match kind {
        AlertKind::Note => (Color::Cyan, "ℹ"),
        AlertKind::Tip => (Color::Green, "✧"),
        AlertKind::Important => (Color::LightMagenta, "!"),
        AlertKind::Warning => (Color::Yellow, "▲"),
        AlertKind::Caution => (Color::Red, "■"),
    }
}

/// Parses inline formatting: backticks (`code`), bold (`**text**`), italics (`*text*`).
fn parse_inline_markdown(text: &str) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    let mut current = String::new();

    while i < chars.len() {
        // Inline code `...`
        if chars[i] == '`' {
            if !current.is_empty() {
                spans.push(Span::styled(
                    current.clone(),
                    Style::default().fg(Color::White),
                ));
                current.clear();
            }
            let start = i + 1;
            let mut end = start;
            while end < chars.len() && chars[end] != '`' {
                end += 1;
            }
            if end < chars.len() {
                let code_text: String = chars[start..end].iter().collect();
                spans.push(Span::styled(
                    format!(" {} ", code_text),
                    Style::default()
                        .fg(Color::LightCyan)
                        .bg(Color::Rgb(40, 44, 52)),
                ));
                i = end + 1;
                continue;
            }
        }

        // Bold **...**
        if i + 1 < chars.len() && chars[i] == '*' && chars[i + 1] == '*' {
            if !current.is_empty() {
                spans.push(Span::styled(
                    current.clone(),
                    Style::default().fg(Color::White),
                ));
                current.clear();
            }
            let start = i + 2;
            let mut end = start;
            while end + 1 < chars.len() && !(chars[end] == '*' && chars[end + 1] == '*') {
                end += 1;
            }
            if end + 1 < chars.len() {
                let bold_text: String = chars[start..end].iter().collect();
                spans.push(Span::styled(
                    bold_text,
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                ));
                i = end + 2;
                continue;
            }
        }

        // Inline math $...$
        if chars[i] == '$' {
            if !current.is_empty() {
                spans.push(Span::styled(
                    current.clone(),
                    Style::default().fg(Color::White),
                ));
                current.clear();
            }
            let start = i + 1;
            let mut end = start;
            while end < chars.len() && chars[end] != '$' {
                end += 1;
            }
            if end < chars.len() {
                let math_src: String = chars[start..end].iter().collect();
                let rendered_math = m31a_term::render_latex(&math_src);
                spans.push(Span::styled(
                    rendered_math,
                    Style::default()
                        .fg(Color::LightMagenta)
                        .add_modifier(Modifier::BOLD),
                ));
                i = end + 1;
                continue;
            }
        }

        current.push(chars[i]);
        i += 1;
    }

    if !current.is_empty() {
        spans.push(Span::styled(current, Style::default().fg(Color::White)));
    }

    spans
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_highlight_code_line() {
        let spans = highlight_code_line("pub fn test_run() -> u64 {", Some("rust"));
        let kw_color = spans
            .iter()
            .find(|s| s.content == "pub")
            .and_then(|s| s.style.fg);
        assert!(kw_color.is_some(), "keyword 'pub' must have a syntax color");
        assert!(
            spans
                .iter()
                .any(|s| s.content == "fn" && s.style.fg == kw_color)
        );
        assert!(
            spans
                .iter()
                .any(|s| s.content == "u64" && s.style.fg.is_some() && s.style.fg != kw_color)
        );
    }

    #[test]
    fn test_render_markdown_headers_and_code() {
        let md = "# Title\n## Section\n```rust\nlet x = 10;\n```\n- Item 1 with `code`";
        let lines = render_markdown_to_lines(md);
        assert!(!lines.is_empty());
        // Verify code fence rendering
        assert!(
            lines
                .iter()
                .any(|l| l.spans.iter().any(|s| s.content.contains("rust")))
        );
    }

    #[test]
    fn test_render_mermaid_diagram_in_markdown() {
        let md = "```mermaid\nflowchart TD\nClient --> Server\n```";
        let lines = render_markdown_to_lines(md);
        assert!(lines.iter().any(|l| {
            l.spans
                .iter()
                .any(|s| s.content.contains("Mermaid Diagram"))
        }));
        assert!(
            lines
                .iter()
                .any(|l| l.spans.iter().any(|s| s.content.contains("Client")))
        );
        assert!(
            lines
                .iter()
                .any(|l| l.spans.iter().any(|s| s.content.contains("Server")))
        );
    }

    #[test]
    fn test_render_latex_math_in_markdown() {
        let md = "The formula is $x^2 + y_1 = z$ for calculations.";
        let lines = render_markdown_to_lines(md);
        assert!(
            lines
                .iter()
                .any(|l| l.spans.iter().any(|s| s.content.contains("x² + y₁ = z")))
        );
    }

    #[test]
    fn test_render_streaming_healed_markdown() {
        let unclosed = "Streaming with **unclosed bold text";
        let lines = render_markdown_to_lines(unclosed);
        assert!(lines.iter().any(|l| {
            l.spans
                .iter()
                .any(|s| s.content.contains("unclosed bold text"))
        }));
    }

    #[test]
    fn test_render_markdown_table() {
        let md = "| Option | Description | Status |\n| :--- | :--- | :--- |\n| A | First feature | Active |\n| B | Second feature | Beta |";
        let lines = render_markdown_to_lines(md);
        assert!(
            lines
                .iter()
                .any(|l| l.spans.iter().any(|s| s.content.contains("Option")))
        );
        assert!(
            lines
                .iter()
                .any(|l| l.spans.iter().any(|s| s.content.contains("First feature")))
        );
        assert!(
            lines
                .iter()
                .any(|l| l.spans.iter().any(|s| s.content.contains("┌")))
        );
        assert!(
            lines
                .iter()
                .any(|l| l.spans.iter().any(|s| s.content.contains("┘")))
        );
    }

    #[test]
    fn test_render_alert_callouts() {
        let md = "> [!NOTE]\n> This is an important background note.";
        let lines = render_markdown_to_lines(md);
        assert!(
            lines
                .iter()
                .any(|l| l.spans.iter().any(|s| s.content.contains("Note")))
        );
        assert!(lines.iter().any(|l| {
            l.spans
                .iter()
                .any(|s| s.content.contains("important background note"))
        }));

        let warn_md = "> [!WARNING]\n> Breaking change caution.";
        let warn_lines = render_markdown_to_lines(warn_md);
        assert!(
            warn_lines
                .iter()
                .any(|l| l.spans.iter().any(|s| s.content.contains("Warning")))
        );
    }

    #[test]
    fn test_render_task_lists_and_rules() {
        let md = "- [ ] Unfinished task\n- [x] Completed task\n---\n1. Ordered item";
        let lines = render_markdown_to_lines(md);
        assert!(
            lines
                .iter()
                .any(|l| l.spans.iter().any(|s| s.content.contains("☐")))
        );
        assert!(
            lines
                .iter()
                .any(|l| l.spans.iter().any(|s| s.content.contains("☑")))
        );
        assert!(
            lines
                .iter()
                .any(|l| l.spans.iter().any(|s| s.content.contains("───")))
        );
        assert!(
            lines
                .iter()
                .any(|l| l.spans.iter().any(|s| s.content.contains("1. ")))
        );
    }

    #[test]
    fn test_streaming_table_holdback_and_highlight_cache() {
        // 1. Streaming table header without delimiter should be held back
        let streaming_header = "Streaming content:\n| Column A | Column B |";
        let lines1 = render_markdown_to_lines(streaming_header);
        assert!(
            !lines1
                .iter()
                .any(|l| l.spans.iter().any(|s| s.content.contains("Column A")))
        );

        // 2. Once delimiter arrives, table is rendered cleanly
        let complete_table =
            "Streaming content:\n| Column A | Column B |\n| --- | --- |\n| Val 1 | Val 2 |";
        let lines2 = render_markdown_to_lines(complete_table);
        assert!(
            lines2
                .iter()
                .any(|l| l.spans.iter().any(|s| s.content.contains("Column A")))
        );
        assert!(
            lines2
                .iter()
                .any(|l| l.spans.iter().any(|s| s.content.contains("Val 1")))
        );

        // 3. Highlight cache test
        let spans_a = highlight_code_line("let mut count = 42;", Some("rust"));
        let spans_b = highlight_code_line("let mut count = 42;", Some("rust"));
        assert_eq!(spans_a.len(), spans_b.len());

        // 4. Markdown doc memoization test
        let doc = "# Fast Heading\n- Item 1\n- Item 2";
        let doc_lines_1 = render_markdown_to_lines(doc);
        let doc_lines_2 = render_markdown_to_lines(doc);
        assert_eq!(doc_lines_1.len(), doc_lines_2.len());
        assert_eq!(doc_lines_1, doc_lines_2);
    }
}
