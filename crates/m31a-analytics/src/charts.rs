//! Unicode terminal bar charts and sparkline generators.

const BARS: &[char] = &[' ', ' ', '▂', '▃', '▄', '▅', '▆', '▇', '█'];

/// Renders a horizontal bar chart given a label, value, max_value, and width.
pub fn render_bar(label: &str, value: u64, max_val: u64, width: usize) -> String {
    let bar_width = width.saturating_sub(label.len() + 10);
    if max_val == 0 || bar_width == 0 {
        return format!("{:<15} [ ] {}", label, value);
    }

    let filled_len = ((value as f64 / max_val as f64) * bar_width as f64).round() as usize;
    let filled_len = filled_len.min(bar_width);

    let mut bar = String::with_capacity(bar_width);
    for _ in 0..filled_len {
        bar.push('█');
    }
    for _ in filled_len..bar_width {
        bar.push('░');
    }

    format!("{:<15} [{}] {:>6}", label, bar, value)
}

/// Renders a single-line sparkline from a series of numbers.
pub fn render_sparkline(values: &[u64]) -> String {
    if values.is_empty() {
        return String::new();
    }

    let max = *values.iter().max().unwrap_or(&1);
    let mut out = String::with_capacity(values.len());

    for &v in values {
        if max == 0 {
            out.push(' ');
        } else {
            let idx = ((v as f64 / max as f64) * (BARS.len() - 1) as f64).round() as usize;
            out.push(BARS[idx.min(BARS.len() - 1)]);
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_bar() {
        let b = render_bar("Prompt", 50, 100, 35);
        assert!(b.contains("Prompt"));
        assert!(b.contains("50"));
    }

    #[test]
    fn test_render_sparkline() {
        let s = render_sparkline(&[10, 30, 80, 50, 100]);
        assert_eq!(s.chars().count(), 5);
    }
}
