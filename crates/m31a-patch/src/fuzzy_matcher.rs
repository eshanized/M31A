//! Fuzzy hunk matching engine with indentation preservation and context drift tolerance.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchLocation {
    pub start_line: usize,
    pub matched_line_count: usize,
    pub leading_indent_override: Option<String>,
    pub drift: isize,
}

pub struct FuzzyMatcher;

impl FuzzyMatcher {
    /// Attempts to find the best match location for `search_lines` within `target_lines`.
    /// `expected_line` is the 0-indexed line number suggested by the hunk header (if any).
    pub fn find_match(
        target_lines: &[String],
        search_lines: &[String],
        expected_line: Option<usize>,
        max_drift: usize,
    ) -> Option<MatchLocation> {
        if search_lines.is_empty() {
            return Some(MatchLocation {
                start_line: expected_line.unwrap_or(0).min(target_lines.len()),
                matched_line_count: 0,
                leading_indent_override: None,
                drift: 0,
            });
        }

        if search_lines.len() > target_lines.len() {
            return None;
        }

        let hint = expected_line
            .unwrap_or(0)
            .min(target_lines.len().saturating_sub(search_lines.len()));

        // Pass 1: Exact match with spiral search starting from expected_line
        if let Some(loc) =
            Self::search_with_strategy(target_lines, search_lines, hint, max_drift, |a, b| a == b)
        {
            return Some(loc);
        }

        // Pass 2: Right-strip match (ignoring trailing whitespace / CRLF)
        if let Some(loc) =
            Self::search_with_strategy(target_lines, search_lines, hint, max_drift, |a, b| {
                a.trim_end() == b.trim_end()
            })
        {
            return Some(loc);
        }

        // Pass 3: Indentation-normalized match with indentation capture
        if let Some(loc) =
            Self::search_indentation_normalized(target_lines, search_lines, hint, max_drift)
        {
            return Some(loc);
        }

        // Pass 4: Permissive unicode normalization (curly quotes, dashes, non-breaking spaces)
        if let Some(loc) =
            Self::search_with_strategy(target_lines, search_lines, hint, max_drift, |a, b| {
                Self::normalise_unicode(a) == Self::normalise_unicode(b)
            })
        {
            return Some(loc);
        }

        // Fallback: Seek sequence with EOF newline sentinel recovery
        if let Some(idx) = Self::seek_sequence(target_lines, search_lines, hint, false) {
            let drift = (idx as isize) - (hint as isize);
            return Some(MatchLocation {
                start_line: idx,
                matched_line_count: search_lines
                    .len()
                    .min(target_lines.len().saturating_sub(idx)),
                leading_indent_override: None,
                drift,
            });
        }

        None
    }

    /// Attempt to find the sequence of `pattern` lines within `lines` beginning at or after `start`.
    /// Decreasing strictness passes: exact, rstrip, trim both, permissive Unicode normalization.
    pub fn seek_sequence(
        lines: &[String],
        pattern: &[String],
        start: usize,
        eof: bool,
    ) -> Option<usize> {
        if pattern.is_empty() {
            return Some(start);
        }
        if pattern.len() > lines.len() {
            return None;
        }

        let search_start = if eof && lines.len() >= pattern.len() {
            let eof_start = lines.len() - pattern.len();
            eof_start.max(start)
        } else {
            start
        };

        // Pass 1: Exact match
        for i in search_start..=lines.len().saturating_sub(pattern.len()) {
            if lines[i..i + pattern.len()] == *pattern {
                return Some(i);
            }
        }

        // Pass 2: Right-strip match (ignoring trailing whitespace / CRLF)
        for i in search_start..=lines.len().saturating_sub(pattern.len()) {
            let mut ok = true;
            for (p_idx, pat) in pattern.iter().enumerate() {
                if lines[i + p_idx].trim_end() != pat.trim_end() {
                    ok = false;
                    break;
                }
            }
            if ok {
                return Some(i);
            }
        }

        // Pass 3: Trim both sides
        for i in search_start..=lines.len().saturating_sub(pattern.len()) {
            let mut ok = true;
            for (p_idx, pat) in pattern.iter().enumerate() {
                if lines[i + p_idx].trim() != pat.trim() {
                    ok = false;
                    break;
                }
            }
            if ok {
                return Some(i);
            }
        }

        // Pass 4: Permissive Unicode normalization
        for i in search_start..=lines.len().saturating_sub(pattern.len()) {
            let mut ok = true;
            for (p_idx, pat) in pattern.iter().enumerate() {
                if Self::normalise_unicode(&lines[i + p_idx]) != Self::normalise_unicode(pat) {
                    ok = false;
                    break;
                }
            }
            if ok {
                return Some(i);
            }
        }

        // End-of-file trailing newline sentinel recovery
        if pattern.last().is_some_and(String::is_empty) {
            let truncated = &pattern[..pattern.len() - 1];
            return Self::seek_sequence(lines, truncated, start, eof);
        }

        None
    }

    fn search_with_strategy<F>(
        target_lines: &[String],
        search_lines: &[String],
        hint_line: usize,
        max_drift: usize,
        comparator: F,
    ) -> Option<MatchLocation>
    where
        F: Fn(&str, &str) -> bool,
    {
        let max_idx = target_lines.len().saturating_sub(search_lines.len());
        let search_order = Self::generate_spiral_indices(hint_line, max_idx, max_drift);

        for idx in search_order {
            let mut matches = true;
            for (offset, search) in search_lines.iter().enumerate() {
                if !comparator(&target_lines[idx + offset], search) {
                    matches = false;
                    break;
                }
            }

            if matches {
                let drift = (idx as isize) - (hint_line as isize);
                return Some(MatchLocation {
                    start_line: idx,
                    matched_line_count: search_lines.len(),
                    leading_indent_override: None,
                    drift,
                });
            }
        }

        None
    }

    fn search_indentation_normalized(
        target_lines: &[String],
        search_lines: &[String],
        hint_line: usize,
        max_drift: usize,
    ) -> Option<MatchLocation> {
        let max_idx = target_lines.len().saturating_sub(search_lines.len());
        let search_order = Self::generate_spiral_indices(hint_line, max_idx, max_drift);

        for idx in search_order {
            let mut matches = true;
            let mut target_indent = None;

            for (offset, search) in search_lines.iter().enumerate() {
                let target = &target_lines[idx + offset];
                if target.trim() != search.trim() {
                    matches = false;
                    break;
                }

                // If non-empty, capture target indentation
                if target_indent.is_none() && !target.trim().is_empty() {
                    let indent: String = target.chars().take_while(|c| c.is_whitespace()).collect();
                    target_indent = Some(indent);
                }
            }

            if matches {
                let drift = (idx as isize) - (hint_line as isize);
                return Some(MatchLocation {
                    start_line: idx,
                    matched_line_count: search_lines.len(),
                    leading_indent_override: target_indent,
                    drift,
                });
            }
        }

        None
    }

    fn generate_spiral_indices(hint: usize, max_idx: usize, max_drift: usize) -> Vec<usize> {
        let mut indices = Vec::with_capacity(max_drift * 2 + 1);
        if hint <= max_idx {
            indices.push(hint);
        }

        for delta in 1..=max_drift {
            if hint >= delta && (hint - delta) <= max_idx {
                indices.push(hint - delta);
            }
            if hint + delta <= max_idx {
                indices.push(hint + delta);
            }
        }

        // Finally append any remaining indices beyond max_drift up to max_idx
        for i in 0..=max_idx {
            if !indices.contains(&i) {
                indices.push(i);
            }
        }

        indices
    }

    pub fn normalise_unicode(s: &str) -> String {
        s.trim()
            .chars()
            .map(|c| match c {
                // Dashes & hyphens
                '\u{2010}' | '\u{2011}' | '\u{2012}' | '\u{2013}' | '\u{2014}' | '\u{2015}'
                | '\u{2212}' | '\u{FE58}' | '\u{FE63}' | '\u{FF0D}' => '-',
                // Single quotes, apostrophes & accents
                '\u{2018}' | '\u{2019}' | '\u{201A}' | '\u{201B}' | '\u{2032}' | '\u{2035}'
                | '\u{FF07}' | '`' | '´' => '\'',
                // Double quotes & guillemets
                '\u{201C}' | '\u{201D}' | '\u{201E}' | '\u{201F}' | '\u{00AB}' | '\u{00BB}'
                | '\u{2033}' | '\u{2036}' | '\u{FF02}' => '"',
                // Whitespace & non-breaking spaces
                '\u{00A0}' | '\u{2002}' | '\u{2003}' | '\u{2004}' | '\u{2005}' | '\u{2006}'
                | '\u{2007}' | '\u{2008}' | '\u{2009}' | '\u{200A}' | '\u{202F}' | '\u{205F}'
                | '\u{3000}' | '\u{FEFF}' => ' ',
                other => other,
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exact_and_drift_matching() {
        let target = vec![
            "// header".to_string(),
            "fn foo() {".to_string(),
            "    let x = 1;".to_string(),
            "}".to_string(),
        ];
        let search = vec!["fn foo() {".to_string(), "    let x = 1;".to_string()];

        // Should find match at line 1 even if hint was 0
        let m = FuzzyMatcher::find_match(&target, &search, Some(0), 10).unwrap();
        assert_eq!(m.start_line, 1);
        assert_eq!(m.matched_line_count, 2);
    }

    #[test]
    fn test_indentation_normalized_matching() {
        let target = vec![
            "        let x = 42;".to_string(),
            "        return x;".to_string(),
        ];
        let search = vec!["    let x = 42;".to_string(), "    return x;".to_string()];

        let m = FuzzyMatcher::find_match(&target, &search, Some(0), 5).unwrap();
        assert_eq!(m.start_line, 0);
        assert_eq!(m.leading_indent_override, Some("        ".to_string()));
    }

    #[test]
    fn test_typographic_unicode_recovery_matching() {
        let target = vec!["let description = \"Hello, world! - test\";".to_string()];
        // Search string containing curly quotes and en-dash (common LLM typographic formatting)
        let search =
            vec!["let description = \u{201C}Hello, world! \u{2013} test\u{201D};".to_string()];

        let m = FuzzyMatcher::find_match(&target, &search, Some(0), 5)
            .expect("match with unicode normalization");
        assert_eq!(m.start_line, 0);
    }
}
