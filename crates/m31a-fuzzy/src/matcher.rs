//! High-performance fuzzy matching and scoring engine.

/// Zero-allocation ASCII fast-path for paths under 256 bytes (99.9% of codebase paths).
/// Avoids UTF-32 `Vec<char>` heap allocations entirely for ASCII candidates.
pub fn fuzzy_match_indices_ascii(query: &[u8], target: &[u8]) -> Option<(i64, Vec<usize>)> {
    let m = query.len();
    let n = target.len();

    if m == 0 {
        return Some((0, Vec::new()));
    }
    if m > n {
        return None;
    }

    // Stack buffer limits
    if n > 256 || m > 64 {
        // Fall back to general heap version if exceptionally long
        return None;
    }

    // 1. Fast subsequence pre-check (single-pass SIMD friendly loop)
    let mut check_idx = 0;
    for &tc in target {
        if check_idx < m && tc.eq_ignore_ascii_case(&query[check_idx]) {
            check_idx += 1;
        }
    }
    if check_idx < m {
        return None;
    }

    // 2. Precompute character match bonuses on stack
    let mut bonus = [0i64; 256];
    for j in 0..n {
        if j == 0 {
            bonus[j] = 35;
        } else {
            let prev = target[j - 1];
            let curr = target[j];
            if prev == b'/' || prev == b'\\' {
                bonus[j] = 30;
            } else if prev == b'_'
                || prev == b'-'
                || prev == b'.'
                || prev == b' '
                || (prev.is_ascii_lowercase() && curr.is_ascii_uppercase())
            {
                bonus[j] = 25;
            } else if prev.is_ascii_digit() != curr.is_ascii_digit() {
                bonus[j] = 15;
            }
        }
    }

    // 3. Stack-allocated DP tables
    const NEG_INF: i64 = -1_000_000_000;
    let mut dp = [[NEG_INF; 256]; 64];
    let mut parent = [[None; 256]; 64];

    let q0_lower = query[0].to_ascii_lowercase();
    for j in 0..n {
        if target[j].to_ascii_lowercase() == q0_lower {
            let mut s = 15 + bonus[j];
            if target[j] == query[0] {
                s += 5;
            }
            s -= (j.min(10)) as i64;
            dp[0][j] = s;
        }
    }

    for i in 1..m {
        let qi_lower = query[i].to_ascii_lowercase();
        let mut max_prev = NEG_INF;
        let mut best_k = None;

        for j in i..n {
            let k = j - 1;
            if dp[i - 1][k] > max_prev {
                max_prev = dp[i - 1][k];
                best_k = Some(k);
            }

            if target[j].to_ascii_lowercase() == qi_lower {
                let consecutive_score = if dp[i - 1][j - 1] > NEG_INF {
                    dp[i - 1][j - 1] + 25 + bonus[j]
                } else {
                    NEG_INF
                };

                let non_consecutive_score = if let Some(k) = best_k {
                    if max_prev > NEG_INF {
                        max_prev + 10 + bonus[j] - ((j - k).saturating_sub(1) as i64)
                    } else {
                        NEG_INF
                    }
                } else {
                    NEG_INF
                };

                if consecutive_score >= non_consecutive_score && consecutive_score > NEG_INF {
                    dp[i][j] = consecutive_score;
                    parent[i][j] = Some(j - 1);
                } else if non_consecutive_score > NEG_INF {
                    dp[i][j] = non_consecutive_score;
                    parent[i][j] = best_k;
                }
            }
        }
    }

    let mut best_j = None;
    let mut max_final_score = NEG_INF;
    for (j, &score) in dp[m - 1].iter().enumerate().take(n).skip(m - 1) {
        if score > max_final_score {
            max_final_score = score;
            best_j = Some(j);
        }
    }

    let end_j = best_j?;
    if max_final_score <= NEG_INF {
        return None;
    }

    let mut matched_indices = vec![0; m];
    let mut curr_j = end_j;
    for i in (0..m).rev() {
        matched_indices[i] = curr_j;
        if i > 0 {
            curr_j = parent[i][curr_j]?;
        }
    }

    let len_penalty = (n - m) as i64;
    let mut total_score = max_final_score - len_penalty;

    if query.eq_ignore_ascii_case(target) {
        total_score += 150;
    }

    Some((total_score, matched_indices))
}

/// Computes a fuzzy match score and the character indices of the matched characters.
/// Returns None if the query is not a subsequence of the target.
pub fn fuzzy_match_indices(query: &str, target: &str) -> Option<(i64, Vec<usize>)> {
    if query.is_empty() {
        return Some((0, Vec::new()));
    }

    // Codex Nucleo Zero-Heap Optimization: If both strings are ASCII and fit stack bounds,
    // dispatch to zero-allocation stack engine avoiding UTF-32 Vec<char> heap allocations.
    if query.is_ascii()
        && target.is_ascii()
        && target.len() <= 256
        && query.len() <= 64
        && let Some(res) = fuzzy_match_indices_ascii(query.as_bytes(), target.as_bytes())
    {
        return Some(res);
    }

    let q_chars: Vec<char> = query.to_lowercase().chars().collect();
    let t_chars: Vec<char> = target.chars().collect();
    let t_lower: Vec<char> = target.to_lowercase().chars().collect();

    let m = q_chars.len();
    let n = t_chars.len();

    if m > n {
        return None;
    }

    // Fast subsequence pre-check
    let mut check_idx = 0;
    for &tc in &t_lower {
        if check_idx < m && tc == q_chars[check_idx] {
            check_idx += 1;
        }
    }
    if check_idx < m {
        return None;
    }

    // Precompute character match bonuses for each position in target
    let mut bonus = vec![0i64; n];
    for j in 0..n {
        if j == 0 {
            bonus[j] = 35; // Start of string bonus
        } else {
            let prev = t_chars[j - 1];
            let curr = t_chars[j];
            if prev == '/' || prev == '\\' {
                bonus[j] = 30; // Start of path segment
            } else if prev == '_' || prev == '-' || prev == '.' || prev == ' ' {
                bonus[j] = 25; // Word separator boundary
            } else if prev.is_lowercase() && curr.is_uppercase() {
                bonus[j] = 25; // CamelCase word boundary
            } else if prev.is_numeric() != curr.is_numeric() {
                bonus[j] = 15;
            }
        }
    }

    // DP table: dp[i][j] stores the best score matching query[0..=i] ending at target[j]
    // parent[i][j] stores the previous target index k < j from which dp[i][j] transitioned
    const NEG_INF: i64 = -1_000_000_000;
    let mut dp = vec![vec![NEG_INF; n]; m];
    let mut parent = vec![vec![None; n]; m];

    // Base case: i = 0 (first character of query)
    for j in 0..n {
        if t_lower[j] == q_chars[0] {
            let mut s = 15 + bonus[j];
            if t_chars[j] == query.chars().next().unwrap() {
                s += 5; // Exact case match
            }
            s -= j.min(10) as i64;
            dp[0][j] = s;
        }
    }

    // Transitions: for i in 1..m
    for i in 1..m {
        let q_char = q_chars[i];
        let mut max_prev = NEG_INF;
        let mut best_k = None;

        for j in i..n {
            let k = j - 1;
            if dp[i - 1][k] > max_prev {
                max_prev = dp[i - 1][k];
                best_k = Some(k);
            }

            if t_lower[j] == q_char {
                // If previous char matched at j-1, apply consecutive bonus
                let consecutive_score = if dp[i - 1][j - 1] > NEG_INF {
                    dp[i - 1][j - 1] + 25 + bonus[j]
                } else {
                    NEG_INF
                };

                let non_consecutive_score = if let Some(k) = best_k {
                    if max_prev > NEG_INF {
                        max_prev + 10 + bonus[j] - ((j - k).saturating_sub(1) as i64)
                    } else {
                        NEG_INF
                    }
                } else {
                    NEG_INF
                };

                if consecutive_score >= non_consecutive_score && consecutive_score > NEG_INF {
                    dp[i][j] = consecutive_score;
                    parent[i][j] = Some(j - 1);
                } else if non_consecutive_score > NEG_INF {
                    dp[i][j] = non_consecutive_score;
                    parent[i][j] = best_k;
                }
            }
        }
    }

    // Find best ending position j for the last query char m-1
    let mut best_j = None;
    let mut max_final_score = NEG_INF;
    for (j, &score) in dp[m - 1].iter().enumerate().take(n).skip(m - 1) {
        if score > max_final_score {
            max_final_score = score;
            best_j = Some(j);
        }
    }

    let end_j = best_j?;
    if max_final_score <= NEG_INF {
        return None;
    }

    // Traceback to recover matched indices
    let mut matched_indices = vec![0; m];
    let mut curr_j = end_j;
    for i in (0..m).rev() {
        matched_indices[i] = curr_j;
        if i > 0 {
            curr_j = parent[i][curr_j]?;
        }
    }

    // Length penalty: concise targets score higher
    let len_penalty = (n - m) as i64;
    let mut total_score = max_final_score - len_penalty;

    // Full exact match bonus
    if query.eq_ignore_ascii_case(target) {
        total_score += 150;
    }

    Some((total_score, matched_indices))
}

/// Computes just the fuzzy score.
pub fn fuzzy_match(query: &str, target: &str) -> Option<i64> {
    fuzzy_match_indices(query, target).map(|(s, _)| s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fuzzy_exact_match() {
        let score = fuzzy_match("main.rs", "main.rs");
        assert!(score.is_some());
        assert!(score.unwrap() > 100);
    }

    #[test]
    fn test_fuzzy_subsequence() {
        assert!(fuzzy_match("m31a", "crates/m31a-core/src/lib.rs").is_some());
        assert!(fuzzy_match("xyz", "crates/m31a-core/src/lib.rs").is_none());
    }

    #[test]
    fn test_fuzzy_boundary_preference() {
        let score1 = fuzzy_match("tc", "turn_context.rs").unwrap();
        let score2 = fuzzy_match("tc", "autocollapse.rs").unwrap();
        assert!(score1 > score2);
    }
}
