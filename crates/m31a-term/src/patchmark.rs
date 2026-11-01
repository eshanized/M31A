//! Streaming Markdown Auto-Healing Engine (`patchmark`).
//!
//! When LLMs stream markdown text token by token into the terminal, inline formatting
//! and block delimiters (code fences, bold, italics, math, links) are frequently in an
//! open/incomplete state at the tail of the chunk.
//!
//! Without healing, incomplete tokens cause visual glitches (e.g. `****` rendered as a
//! horizontal rule, entire paragraphs unstyled, or unclosed code blocks swallowing
//! following content).
//!
//! `patchmark` computes a minimal tail heal to temporarily close unsealed markers
//! so each streaming frame renders cleanly without flicker.

/// Describes an edit to heal the tail of a streaming Markdown fragment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TailHeal {
    /// Number of characters to strip from the end of the trimmed text.
    pub strip: usize,
    /// String to append to properly close or seal unclosed delimiters.
    pub append: String,
}

impl TailHeal {
    pub const fn empty() -> Self {
        Self {
            strip: 0,
            append: String::new(),
        }
    }
}

/// Applies auto-healing to a streaming markdown string, returning a well-formed
/// snapshot suitable for terminal rendering.
pub fn heal_streaming_markdown(input: &str) -> String {
    let heal = compute_tail_heal(input);
    let trimmed = input.trim_end();
    if heal.strip == 0 && heal.append.is_empty() {
        return input.to_string();
    }

    let end_idx = trimmed.len().saturating_sub(heal.strip);
    let mut result = String::with_capacity(end_idx + heal.append.len());
    let prefix = &trimmed[..end_idx];
    let prefix = if heal.append.is_empty() {
        prefix.trim_end()
    } else {
        prefix
    };
    result.push_str(prefix);
    result.push_str(&heal.append);
    result
}

/// Computes the tail heal for an arbitrary streaming markdown string.
pub fn compute_tail_heal(input: &str) -> TailHeal {
    let trimmed = input.trim_end();
    if trimmed.is_empty() {
        return TailHeal::empty();
    }

    // 1. Check for unclosed fenced code block (``` or ~~~)
    let mut in_code_fence: Option<(char, usize)> = None;
    for line in trimmed.lines() {
        let line_trimmed = line.trim_start();
        if line_trimmed.starts_with("```") || line_trimmed.starts_with("~~~") {
            let ch = line_trimmed.chars().next().unwrap();
            let fence_len = line_trimmed.chars().take_while(|&c| c == ch).count();
            if fence_len >= 3 {
                if let Some((curr_ch, curr_len)) = in_code_fence {
                    if curr_ch == ch && fence_len >= curr_len {
                        in_code_fence = None;
                    }
                } else {
                    in_code_fence = Some((ch, fence_len));
                }
            }
        }
    }

    if let Some((ch, fence_len)) = in_code_fence {
        let closing = format!("\n{}", ch.to_string().repeat(fence_len));
        return TailHeal {
            strip: 0,
            append: closing,
        };
    }

    // 2. Check for unfinished inline link: `[label](http...` or `![alt](http...`
    if let Some(open_paren) = trimmed.rfind("](") {
        // Find matching opening bracket '[' before ']('
        let prefix = &trimmed[..open_paren];
        if prefix.rfind('[').is_some() {
            let after_paren = &trimmed[open_paren + 2..];
            if !after_paren.contains(')') {
                // Link is still being written; close the paren
                return TailHeal {
                    strip: 0,
                    append: ")".to_string(),
                };
            }
        }
    } else if let Some(bracket_start) = trimmed.rfind('[') {
        let after_bracket = &trimmed[bracket_start + 1..];
        if !after_bracket.contains(']') {
            // Half-open link label: `[Some Text` -> close `]`
            return TailHeal {
                strip: 0,
                append: "]".to_string(),
            };
        }
    }

    // 3. Scan for inline backticks, bold (**), italics (* or _), strikethrough (~~), and math ($$)
    let chars: Vec<char> = trimmed.chars().collect();
    let n = chars.len();

    // Check inline code span
    let mut in_inline_code: Option<usize> = None;
    let mut i = 0;
    while i < n {
        if chars[i] == '`' {
            let start = i;
            while i < n && chars[i] == '`' {
                i += 1;
            }
            let run = i - start;
            if let Some(curr) = in_inline_code {
                if curr == run {
                    in_inline_code = None;
                }
            } else {
                in_inline_code = Some(run);
            }
        } else {
            i += 1;
        }
    }

    if let Some(run) = in_inline_code {
        return TailHeal {
            strip: 0,
            append: "`".repeat(run),
        };
    }

    // Check display math ($$) and inline math ($)
    let mut double_dollar_count = 0;
    let mut single_dollar_count = 0;
    let mut j = 0;
    while j < n {
        if chars[j] == '$' {
            if j + 1 < n && chars[j + 1] == '$' {
                double_dollar_count += 1;
                j += 2;
            } else {
                single_dollar_count += 1;
                j += 1;
            }
        } else {
            j += 1;
        }
    }

    if double_dollar_count % 2 != 0 {
        return TailHeal {
            strip: 0,
            append: "$$".to_string(),
        };
    }
    if single_dollar_count % 2 != 0 {
        return TailHeal {
            strip: 0,
            append: "$".to_string(),
        };
    }

    // Check bold/italics delimiters
    // Count trailing consecutive stars or tildes to avoid `****` rule creation
    let trailing_stars = chars.iter().rev().take_while(|&&c| c == '*').count();
    let trailing_tildes = chars.iter().rev().take_while(|&&c| c == '~').count();

    if trailing_stars == 2 {
        // User/model just typed `**` with nothing inside yet: hide it to prevent `****` rule
        return TailHeal {
            strip: 2,
            append: String::new(),
        };
    }

    // Check unclosed `**` (bold)
    let mut star_runs = Vec::new();
    let mut k = 0;
    while k < n {
        if chars[k] == '*' {
            let start = k;
            while k < n && chars[k] == '*' {
                k += 1;
            }
            star_runs.push(k - start);
        } else {
            k += 1;
        }
    }

    let mut open_double_stars = 0;
    let mut open_single_stars = 0;
    for run in star_runs {
        if run >= 2 {
            open_double_stars = (open_double_stars + 1) % 2;
        } else if run == 1 {
            open_single_stars = (open_single_stars + 1) % 2;
        }
    }

    if open_double_stars > 0 && open_single_stars > 0 {
        return TailHeal {
            strip: 0,
            append: "***".to_string(),
        };
    } else if open_double_stars > 0 {
        return TailHeal {
            strip: 0,
            append: "**".to_string(),
        };
    } else if open_single_stars > 0 {
        return TailHeal {
            strip: 0,
            append: "*".to_string(),
        };
    }

    // Check unclosed `~~` (strikethrough)
    if trailing_tildes == 2 {
        return TailHeal {
            strip: 2,
            append: String::new(),
        };
    }
    let tilde_pairs = chars
        .windows(2)
        .filter(|w| w[0] == '~' && w[1] == '~')
        .count();
    if tilde_pairs % 2 != 0 {
        return TailHeal {
            strip: 0,
            append: "~~".to_string(),
        };
    }

    TailHeal::empty()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unclosed_code_fence() {
        let input = "Here is the code:\n```rust\nfn main() {\n    println!(\"hello\");";
        let healed = heal_streaming_markdown(input);
        assert!(healed.ends_with("\n```"));
        assert!(healed.contains("println!(\"hello\");"));
    }

    #[test]
    fn test_closed_code_fence() {
        let input = "Here is the code:\n```rust\nfn main() {}\n```";
        let healed = heal_streaming_markdown(input);
        assert_eq!(healed, input);
    }

    #[test]
    fn test_unclosed_inline_code() {
        let input = "Use the `cargo build command";
        let healed = heal_streaming_markdown(input);
        assert_eq!(healed, "Use the `cargo build command`");
    }

    #[test]
    fn test_unclosed_bold() {
        let input = "This is **very important text";
        let healed = heal_streaming_markdown(input);
        assert_eq!(healed, "This is **very important text**");
    }

    #[test]
    fn test_half_written_bold_suppression() {
        let input = "This is **";
        let healed = heal_streaming_markdown(input);
        assert_eq!(healed, "This is");
    }

    #[test]
    fn test_unclosed_link() {
        let input = "Read the [docs](https://crates.io";
        let healed = heal_streaming_markdown(input);
        assert_eq!(healed, "Read the [docs](https://crates.io)");
    }

    #[test]
    fn test_unclosed_math() {
        let input = "Formula is $$E = mc^2";
        let healed = heal_streaming_markdown(input);
        assert_eq!(healed, "Formula is $$E = mc^2$$");
    }
}
