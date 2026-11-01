//! Terminal LaTeX Math Equation Renderer (`latex`).
//!
//! Transforms LaTeX mathematical expressions into crisp, Unicode-rendered mathematical
//! typography suitable for ANSI terminal display and Ratatui widgets without requiring
//! an external TeX distribution.
//!
//! Provides both inline 1D Unicode glyph replacement and layered 2D terminal math layout
//! for fractions, square roots, summations, integrals, and matrices.

use std::collections::HashMap;

/// Maps standard LaTeX Greek letter command strings to Unicode glyphs.
static GREEK_MAP: &[(&str, &str)] = &[
    ("alpha", "α"),
    ("beta", "β"),
    ("gamma", "γ"),
    ("delta", "δ"),
    ("epsilon", "ε"),
    ("varepsilon", "ϵ"),
    ("zeta", "ζ"),
    ("eta", "η"),
    ("theta", "θ"),
    ("vartheta", "ϑ"),
    ("iota", "ι"),
    ("kappa", "κ"),
    ("lambda", "λ"),
    ("mu", "μ"),
    ("nu", "ν"),
    ("xi", "ξ"),
    ("pi", "π"),
    ("varpi", "ϖ"),
    ("rho", "ρ"),
    ("varrho", "ϱ"),
    ("sigma", "σ"),
    ("varsigma", "ς"),
    ("tau", "τ"),
    ("upsilon", "υ"),
    ("phi", "ϕ"),
    ("varphi", "φ"),
    ("chi", "χ"),
    ("psi", "ψ"),
    ("omega", "ω"),
    ("Alpha", "Α"),
    ("Beta", "Β"),
    ("Gamma", "Γ"),
    ("Delta", "Δ"),
    ("Epsilon", "Ε"),
    ("Zeta", "Ζ"),
    ("Eta", "Η"),
    ("Theta", "Θ"),
    ("Iota", "Ι"),
    ("Kappa", "Κ"),
    ("Lambda", "Λ"),
    ("Mu", "Μ"),
    ("Nu", "Ν"),
    ("Xi", "Ξ"),
    ("Pi", "Π"),
    ("Rho", "Ρ"),
    ("Sigma", "Σ"),
    ("Tau", "Τ"),
    ("Upsilon", "Υ"),
    ("Phi", "Φ"),
    ("Chi", "Χ"),
    ("Psi", "Ψ"),
    ("Omega", "Ω"),
];

/// Maps LaTeX operator and relation symbols to Unicode glyphs.
static SYMBOL_MAP: &[(&str, &str)] = &[
    ("pm", "±"),
    ("mp", "∓"),
    ("times", "×"),
    ("div", "÷"),
    ("cdot", "·"),
    ("ast", "∗"),
    ("star", "⋆"),
    ("circ", "∘"),
    ("bullet", "∙"),
    ("oplus", "⊕"),
    ("otimes", "⊗"),
    ("infty", "∞"),
    ("partial", "∂"),
    ("nabla", "∇"),
    ("sum", "∑"),
    ("prod", "∏"),
    ("int", "∫"),
    ("oint", "∮"),
    ("iint", "∬"),
    ("iiint", "∭"),
    ("leq", "≤"),
    ("le", "≤"),
    ("geq", "≥"),
    ("ge", "≥"),
    ("neq", "≠"),
    ("approx", "≈"),
    ("equiv", "≡"),
    ("sim", "∼"),
    ("simeq", "≃"),
    ("in", "∈"),
    ("notin", "∉"),
    ("ni", "∋"),
    ("subset", "⊂"),
    ("supset", "⊃"),
    ("subseteq", "⊆"),
    ("supseteq", "⊇"),
    ("cup", "∪"),
    ("cap", "∩"),
    ("setminus", "∖"),
    ("forall", "∀"),
    ("exists", "∃"),
    ("nexists", "∄"),
    ("to", "→"),
    ("rightarrow", "→"),
    ("leftarrow", "←"),
    ("Rightarrow", "⇒"),
    ("Leftarrow", "⇐"),
    ("leftrightarrow", "↔"),
    ("Leftrightarrow", "⇔"),
    ("mapsto", "↦"),
    ("land", "∧"),
    ("lor", "∨"),
    ("neg", "¬"),
    ("lnot", "¬"),
    ("hbar", "ℏ"),
    ("ell", "ℓ"),
    ("Re", "ℜ"),
    ("Im", "ℑ"),
    ("bot", "⊥"),
    ("top", "⊤"),
    ("angle", "∠"),
    ("emptyset", "∅"),
    ("checkmark", "✓"),
];

/// Converts an ASCII character into its Unicode superscript equivalent if available.
pub fn to_superscript_char(c: char) -> Option<char> {
    match c {
        '0' => Some('⁰'),
        '1' => Some('¹'),
        '2' => Some('²'),
        '3' => Some('³'),
        '4' => Some('⁴'),
        '5' => Some('⁵'),
        '6' => Some('⁶'),
        '7' => Some('⁷'),
        '8' => Some('⁸'),
        '9' => Some('⁹'),
        '+' => Some('⁺'),
        '-' => Some('⁻'),
        '=' => Some('⁼'),
        '(' => Some('⁽'),
        ')' => Some('⁾'),
        'a' => Some('ᵃ'),
        'b' => Some('ᵇ'),
        'c' => Some('ᶜ'),
        'd' => Some('ᵈ'),
        'e' => Some('ᵉ'),
        'f' => Some('ᶠ'),
        'g' => Some('ᵍ'),
        'h' => Some('ʰ'),
        'i' => Some('ⁱ'),
        'j' => Some('ʲ'),
        'k' => Some('ᵏ'),
        'l' => Some('ˡ'),
        'm' => Some('ᵐ'),
        'n' => Some('ⁿ'),
        'o' => Some('ᵒ'),
        'p' => Some('ᵖ'),
        'r' => Some('ʳ'),
        's' => Some('ˢ'),
        't' => Some('ᵗ'),
        'u' => Some('ᵘ'),
        'v' => Some('ᵛ'),
        'w' => Some('ʷ'),
        'x' => Some('ˣ'),
        'y' => Some('ʸ'),
        'z' => Some('ᶻ'),
        _ => None,
    }
}

/// Converts an ASCII character into its Unicode subscript equivalent if available.
pub fn to_subscript_char(c: char) -> Option<char> {
    match c {
        '0' => Some('₀'),
        '1' => Some('₁'),
        '2' => Some('₂'),
        '3' => Some('₃'),
        '4' => Some('₄'),
        '5' => Some('₅'),
        '6' => Some('₆'),
        '7' => Some('₇'),
        '8' => Some('₈'),
        '9' => Some('₉'),
        '+' => Some('₊'),
        '-' => Some('₋'),
        '=' => Some('₌'),
        '(' => Some('₍'),
        ')' => Some('₎'),
        'a' => Some('ₐ'),
        'e' => Some('ₑ'),
        'h' => Some('ₕ'),
        'i' => Some('ᵢ'),
        'j' => Some('ⱼ'),
        'k' => Some('ₖ'),
        'l' => Some('ₗ'),
        'm' => Some('ₘ'),
        'n' => Some('ₙ'),
        'o' => Some('ₒ'),
        'p' => Some('ₚ'),
        'r' => Some('ᵣ'),
        's' => Some('ₛ'),
        't' => Some('ₜ'),
        'u' => Some('ᵤ'),
        'v' => Some('ᵥ'),
        'x' => Some('ₓ'),
        _ => None,
    }
}

/// Renders a string of characters to superscripts.
pub fn to_superscript(s: &str) -> String {
    s.chars()
        .map(|c| to_superscript_char(c).unwrap_or(c))
        .collect()
}

/// Renders a string of characters to subscripts.
pub fn to_subscript(s: &str) -> String {
    s.chars()
        .map(|c| to_subscript_char(c).unwrap_or(c))
        .collect()
}

/// 2D box representing a rendered mathematical typography block.
#[derive(Clone, Debug)]
pub struct MathBox {
    pub lines: Vec<String>,
    pub baseline: usize,
    pub width: usize,
}

impl MathBox {
    pub fn new(text: &str) -> Self {
        let width = text.chars().count();
        Self {
            lines: vec![text.to_string()],
            baseline: 0,
            width,
        }
    }

    /// Horizontally concatenates a sequence of MathBoxes aligning on their baselines.
    pub fn concat(boxes: Vec<MathBox>, spacing: usize) -> Self {
        if boxes.is_empty() {
            return Self::new("");
        }

        let max_above = boxes.iter().map(|b| b.baseline).max().unwrap_or(0);
        let max_below = boxes
            .iter()
            .map(|b| b.lines.len().saturating_sub(1 + b.baseline))
            .max()
            .unwrap_or(0);

        let total_height = max_above + 1 + max_below;
        let baseline = max_above;

        let mut output_lines = vec![String::new(); total_height];
        let sep = " ".repeat(spacing);

        for (box_idx, b) in boxes.iter().enumerate() {
            let offset_y = max_above - b.baseline;

            for (row, output_line) in output_lines.iter_mut().enumerate() {
                if box_idx > 0 {
                    output_line.push_str(&sep);
                }

                if row >= offset_y && row < offset_y + b.lines.len() {
                    let line = &b.lines[row - offset_y];
                    output_line.push_str(line);
                    let pad = b.width.saturating_sub(line.chars().count());
                    output_line.push_str(&" ".repeat(pad));
                } else {
                    output_line.push_str(&" ".repeat(b.width));
                }
            }
        }

        let width = output_lines[0].chars().count();
        Self {
            lines: output_lines,
            baseline,
            width,
        }
    }

    /// Creates a 2D fraction block with a horizontal division bar.
    pub fn fraction(num: MathBox, den: MathBox) -> Self {
        let w = num.width.max(den.width) + 2;
        let bar = "─".repeat(w);

        let pad_num_left = (w - num.width) / 2;
        let pad_num_right = w - num.width - pad_num_left;

        let pad_den_left = (w - den.width) / 2;
        let pad_den_right = w - den.width - pad_den_left;

        let mut lines = Vec::new();

        // Numerator lines
        for l in num.lines {
            lines.push(format!(
                "{}{}{}",
                " ".repeat(pad_num_left),
                l,
                " ".repeat(pad_num_right)
            ));
        }

        // Fraction bar is the baseline
        let baseline = lines.len();
        lines.push(bar);

        // Denominator lines
        for l in den.lines {
            lines.push(format!(
                "{}{}{}",
                " ".repeat(pad_den_left),
                l,
                " ".repeat(pad_den_right)
            ));
        }

        Self {
            lines,
            baseline,
            width: w,
        }
    }

    /// Creates a 2D square root block with top overline.
    pub fn sqrt(radicand: MathBox, root_idx: Option<String>) -> Self {
        let w = radicand.width + 2;
        let prefix = root_idx
            .map(|idx| format!("{}√", to_superscript(&idx)))
            .unwrap_or_else(|| "√".to_string());
        let prefix_len = prefix.chars().count();

        let top_bar = format!("{}┌{}┐", " ".repeat(prefix_len), "─".repeat(w));
        let mut lines = vec![top_bar];

        for (idx, l) in radicand.lines.iter().enumerate() {
            let p = if idx == radicand.baseline {
                &prefix
            } else {
                &" ".repeat(prefix_len)
            };
            lines.push(format!("{}│ {} │", p, l));
        }

        let width = lines[0].chars().count();
        let baseline = radicand.baseline + 1;

        Self {
            lines,
            baseline,
            width,
        }
    }

    /// Creates a 2D summation, product, or integral with stacked limits.
    pub fn large_operator(
        symbol: &str,
        sub: Option<&str>,
        sup: Option<&str>,
        body: MathBox,
    ) -> Self {
        let op_width = symbol.chars().count().max(3);

        let sup_str = sup.unwrap_or("");
        let sub_str = sub.unwrap_or("");

        let max_w = op_width
            .max(sup_str.chars().count())
            .max(sub_str.chars().count());

        let pad_sup = (max_w.saturating_sub(sup_str.chars().count())) / 2;
        let pad_op = (max_w.saturating_sub(op_width)) / 2;
        let pad_sub = (max_w.saturating_sub(sub_str.chars().count())) / 2;

        let line_sup = format!(
            "{}{}{}",
            " ".repeat(pad_sup),
            sup_str,
            " ".repeat(max_w.saturating_sub(sup_str.chars().count() + pad_sup))
        );
        let line_op = format!(
            "{}{}{}",
            " ".repeat(pad_op),
            symbol,
            " ".repeat(max_w.saturating_sub(op_width + pad_op))
        );
        let line_sub = format!(
            "{}{}{}",
            " ".repeat(pad_sub),
            sub_str,
            " ".repeat(max_w.saturating_sub(sub_str.chars().count() + pad_sub))
        );

        let op_box = Self {
            lines: vec![line_sup, line_op, line_sub],
            baseline: 1,
            width: max_w,
        };

        Self::concat(vec![op_box, body], 1)
    }

    /// Creates a 2D matrix surrounded by brackets.
    pub fn matrix(rows: Vec<Vec<MathBox>>, bracket_kind: char) -> Self {
        if rows.is_empty() {
            return Self::new("");
        }

        let num_cols = rows.iter().map(|r| r.len()).max().unwrap_or(0);
        let mut col_widths = vec![0; num_cols];

        for r in &rows {
            for (c_idx, cell) in r.iter().enumerate() {
                col_widths[c_idx] = col_widths[c_idx].max(cell.width);
            }
        }

        let mut row_boxes = Vec::new();
        for r in rows {
            let mut padded_cells = Vec::new();
            for (c_idx, cell) in r.into_iter().enumerate() {
                let pad = col_widths[c_idx].saturating_sub(cell.width);
                let padded = if pad > 0 {
                    MathBox::concat(vec![cell, MathBox::new(&" ".repeat(pad))], 0)
                } else {
                    cell
                };
                padded_cells.push(padded);
            }
            row_boxes.push(MathBox::concat(padded_cells, 2));
        }

        let mut all_lines = Vec::new();
        for rb in row_boxes {
            all_lines.extend(rb.lines);
        }

        let height = all_lines.len();
        let mut framed = Vec::new();

        for (i, l) in all_lines.iter().enumerate() {
            let (left_br, right_br) = match bracket_kind {
                '[' => ('[', ']'),
                '|' => ('│', '│'),
                _ => {
                    if height == 1 {
                        ('(', ')')
                    } else if i == 0 {
                        ('⎛', '⎞')
                    } else if i == height - 1 {
                        ('⎝', '⎠')
                    } else {
                        ('⎜', '⎟')
                    }
                }
            };
            framed.push(format!("{left_br} {l} {right_br}"));
        }

        let width = framed[0].chars().count();
        let baseline = height / 2;

        Self {
            lines: framed,
            baseline,
            width,
        }
    }
}

/// Renders a LaTeX math expression into a 2D terminal box-drawing mathematical typography layout.
pub fn render_latex_2d(source: &str) -> String {
    let trimmed = source.trim().trim_matches('$').trim();
    if trimmed.is_empty() {
        return String::new();
    }

    // Check for matrices: \begin{pmatrix} ... \end{pmatrix}
    if let Some((_, matrix_body)) = trimmed.split_once(r"\begin{pmatrix}")
        && let Some((body, _)) = matrix_body.split_once(r"\end{pmatrix}")
    {
        return render_matrix_body(body, '(');
    }
    if let Some((_, matrix_body)) = trimmed.split_once(r"\begin{bmatrix}")
        && let Some((body, _)) = matrix_body.split_once(r"\end{bmatrix}")
    {
        return render_matrix_body(body, '[');
    }
    if let Some((_, matrix_body)) = trimmed.split_once(r"\begin{matrix}")
        && let Some((body, _)) = matrix_body.split_once(r"\end{matrix}")
    {
        return render_matrix_body(body, ' ');
    }

    // Check for fractions: \frac{...}{...}
    if let Some((pre, rest)) = trimmed.split_once(r"\frac") {
        let chars: Vec<char> = rest.chars().collect();
        let (num_str, i1) = extract_bracketed(&chars, 0);
        let (den_str, _) = extract_bracketed(&chars, i1);

        let pre_box = MathBox::new(&render_latex(pre.trim()));
        let num_box = MathBox::new(&render_latex(&num_str));
        let den_box = MathBox::new(&render_latex(&den_str));

        let frac_box = MathBox::fraction(num_box, den_box);
        let combined = if pre.trim().is_empty() {
            frac_box
        } else {
            MathBox::concat(vec![pre_box, frac_box], 1)
        };
        return combined.lines.join("\n");
    }

    // Check for square root: \sqrt{...}
    if let Some((pre, rest)) = trimmed.split_once(r"\sqrt") {
        let chars: Vec<char> = rest.chars().collect();
        let mut root_idx = None;
        let mut start_idx = 0;
        if !chars.is_empty() && chars[0] == '[' {
            let (idx, next_i) = extract_square_bracketed(&chars, 0);
            root_idx = Some(idx);
            start_idx = next_i;
        }
        let (radicand_str, _) = extract_bracketed(&chars, start_idx);
        let rad_box = MathBox::new(&render_latex(&radicand_str));
        let sqrt_box = MathBox::sqrt(rad_box, root_idx);

        let pre_box = MathBox::new(&render_latex(pre.trim()));
        let combined = if pre.trim().is_empty() {
            sqrt_box
        } else {
            MathBox::concat(vec![pre_box, sqrt_box], 1)
        };
        return combined.lines.join("\n");
    }

    // Check for summation: \sum_{...}^{...}
    if let Some((pre, rest)) = trimmed.split_once(r"\sum") {
        let (sub, sup, body_str) = parse_stacked_limits(rest);
        let body_box = MathBox::new(&render_latex(&body_str));
        let op_box = MathBox::large_operator("∑", sub.as_deref(), sup.as_deref(), body_box);
        let pre_box = MathBox::new(&render_latex(pre.trim()));
        let combined = if pre.trim().is_empty() {
            op_box
        } else {
            MathBox::concat(vec![pre_box, op_box], 1)
        };
        return combined.lines.join("\n");
    }

    // Check for integral: \int_{...}^{...}
    if let Some((pre, rest)) = trimmed.split_once(r"\int") {
        let (sub, sup, body_str) = parse_stacked_limits(rest);
        let body_box = MathBox::new(&render_latex(&body_str));
        let op_box = MathBox::large_operator("∫", sub.as_deref(), sup.as_deref(), body_box);
        let pre_box = MathBox::new(&render_latex(pre.trim()));
        let combined = if pre.trim().is_empty() {
            op_box
        } else {
            MathBox::concat(vec![pre_box, op_box], 1)
        };
        return combined.lines.join("\n");
    }

    // Fallback: 1D rendered expression
    render_latex(trimmed)
}

fn parse_stacked_limits(rest: &str) -> (Option<String>, Option<String>, String) {
    let mut sub = None;
    let mut sup = None;
    let mut remainder = rest.trim();

    while remainder.starts_with('_') || remainder.starts_with('^') {
        if let Some(r) = remainder.strip_prefix('_') {
            let chars: Vec<char> = r.chars().collect();
            let (tok, next_i) = extract_script_token(&chars, 0);
            sub = Some(tok);
            remainder = r[next_i..].trim();
        } else if let Some(r) = remainder.strip_prefix('^') {
            let chars: Vec<char> = r.chars().collect();
            let (tok, next_i) = extract_script_token(&chars, 0);
            sup = Some(tok);
            remainder = r[next_i..].trim();
        }
    }

    (sub, sup, remainder.to_string())
}

fn render_matrix_body(body: &str, bracket_kind: char) -> String {
    let mut rows = Vec::new();
    for row_str in body.split(r"\\") {
        let mut row = Vec::new();
        for cell_str in row_str.split('&') {
            row.push(MathBox::new(&render_latex(cell_str.trim())));
        }
        if !row.is_empty() {
            rows.push(row);
        }
    }
    MathBox::matrix(rows, bracket_kind).lines.join("\n")
}

/// Renders a LaTeX math expression into an inline Unicode string.
pub fn render_latex(source: &str) -> String {
    let trimmed = source.trim().trim_matches('$').trim();
    if trimmed.is_empty() {
        return String::new();
    }

    let greek_lookup: HashMap<&str, &str> = GREEK_MAP.iter().cloned().collect();
    let symbol_lookup: HashMap<&str, &str> = SYMBOL_MAP.iter().cloned().collect();

    let mut out = String::with_capacity(trimmed.len());
    let chars: Vec<char> = trimmed.chars().collect();
    let n = chars.len();
    let mut i = 0;

    while i < n {
        let c = chars[i];

        if c == '\\' {
            i += 1;
            let start = i;
            while i < n && chars[i].is_ascii_alphabetic() {
                i += 1;
            }
            let cmd: String = chars[start..i].iter().collect();

            if cmd == "frac" {
                let (num, next_i) = extract_bracketed(&chars, i);
                i = next_i;
                let (den, next_i2) = extract_bracketed(&chars, i);
                i = next_i2;

                let rendered_num = render_latex(&num);
                let rendered_den = render_latex(&den);
                out.push_str(&format!("({}/{})", rendered_num, rendered_den));
                continue;
            } else if cmd == "sqrt" {
                let mut root_idx = None;
                if i < n && chars[i] == '[' {
                    let (idx, next_i) = extract_square_bracketed(&chars, i);
                    i = next_i;
                    root_idx = Some(idx);
                }
                let (radicand, next_i) = extract_bracketed(&chars, i);
                i = next_i;
                let rendered_rad = render_latex(&radicand);
                if let Some(idx) = root_idx {
                    if idx == "3" {
                        out.push_str(&format!("∛({})", rendered_rad));
                    } else if idx == "4" {
                        out.push_str(&format!("∜({})", rendered_rad));
                    } else {
                        out.push_str(&format!("{}^√({})", to_superscript(&idx), rendered_rad));
                    }
                } else {
                    out.push_str(&format!("√({})", rendered_rad));
                }
                continue;
            } else if let Some(&glyph) = greek_lookup.get(cmd.as_str()) {
                out.push_str(glyph);
                continue;
            } else if let Some(&glyph) = symbol_lookup.get(cmd.as_str()) {
                out.push_str(glyph);
                continue;
            } else if matches!(
                cmd.as_str(),
                "sin" | "cos" | "tan" | "log" | "ln" | "exp" | "lim" | "det" | "min" | "max"
            ) {
                out.push_str(&cmd);
                out.push(' ');
                continue;
            } else if cmd.is_empty() && i < n {
                out.push(chars[i]);
                i += 1;
                continue;
            } else {
                out.push_str(&cmd);
                continue;
            }
        } else if c == '^' {
            i += 1;
            let (content, next_i) = extract_script_token(&chars, i);
            i = next_i;
            let converted = to_superscript(&content);
            if converted == content
                && !content.is_empty()
                && !content.chars().all(|ch| to_superscript_char(ch).is_some())
            {
                out.push('^');
                out.push_str(&content);
            } else {
                out.push_str(&converted);
            }
        } else if c == '_' {
            i += 1;
            let (content, next_i) = extract_script_token(&chars, i);
            i = next_i;
            let converted = to_subscript(&content);
            if converted == content
                && !content.is_empty()
                && !content.chars().all(|ch| to_subscript_char(ch).is_some())
            {
                out.push('_');
                out.push_str(&content);
            } else {
                out.push_str(&converted);
            }
        } else {
            out.push(c);
            i += 1;
        }
    }

    out
}

fn extract_bracketed(chars: &[char], mut i: usize) -> (String, usize) {
    while i < chars.len() && chars[i].is_whitespace() {
        i += 1;
    }
    if i >= chars.len() {
        return (String::new(), i);
    }
    if chars[i] == '{' {
        i += 1;
        let mut depth = 1;
        let start = i;
        while i < chars.len() && depth > 0 {
            if chars[i] == '{' {
                depth += 1;
            } else if chars[i] == '}' {
                depth -= 1;
            }
            if depth > 0 {
                i += 1;
            }
        }
        let content: String = chars[start..i].iter().collect();
        if i < chars.len() && chars[i] == '}' {
            i += 1;
        }
        (content, i)
    } else {
        (chars[i].to_string(), i + 1)
    }
}

fn extract_square_bracketed(chars: &[char], mut i: usize) -> (String, usize) {
    while i < chars.len() && chars[i].is_whitespace() {
        i += 1;
    }
    if i < chars.len() && chars[i] == '[' {
        i += 1;
        let start = i;
        while i < chars.len() && chars[i] != ']' {
            i += 1;
        }
        let content: String = chars[start..i].iter().collect();
        if i < chars.len() && chars[i] == ']' {
            i += 1;
        }
        (content, i)
    } else {
        (String::new(), i)
    }
}

fn extract_script_token(chars: &[char], mut i: usize) -> (String, usize) {
    if i >= chars.len() {
        return (String::new(), i);
    }
    if chars[i] == '{' {
        extract_bracketed(chars, i)
    } else if chars[i] == '\\' {
        i += 1;
        let start = i;
        while i < chars.len() && chars[i].is_ascii_alphabetic() {
            i += 1;
        }
        let cmd: String = chars[start..i].iter().collect();
        let rendered = render_latex(&format!("\\{cmd}"));
        (rendered, i)
    } else {
        (chars[i].to_string(), i + 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_superscripts_and_subscripts() {
        assert_eq!(render_latex("x^2 + y_1 = z^{10}"), "x² + y₁ = z¹⁰");
    }

    #[test]
    fn test_greek_and_symbols() {
        assert_eq!(
            render_latex(r"\alpha + \beta \times \gamma \leq \delta"),
            "α + β × γ ≤ δ"
        );
    }

    #[test]
    fn test_fraction_and_sqrt() {
        assert_eq!(render_latex(r"\frac{a + b}{c}"), "(a + b/c)");
        assert_eq!(render_latex(r"\sqrt{x^2 + 1}"), "√(x² + 1)");
        assert_eq!(render_latex(r"\sqrt[3]{8}"), "∛(8)");
    }

    #[test]
    fn test_integral_and_infinity() {
        assert_eq!(render_latex(r"\int_0^\infty f(x) dx"), "∫₀^∞ f(x) dx");
    }

    #[test]
    fn test_2d_fraction_rendering() {
        let res = render_latex_2d(r"\frac{a + b}{c - d}");
        assert!(res.contains("a + b"));
        assert!(res.contains("c - d"));
        assert!(res.contains('─'));
    }

    #[test]
    fn test_2d_sqrt_rendering() {
        let res = render_latex_2d(r"\sqrt{x^2 + y^2}");
        assert!(res.contains("x² + y²"));
        assert!(res.contains('┌'));
        assert!(res.contains('√'));
    }

    #[test]
    fn test_2d_matrix_rendering() {
        let matrix_src = r"\begin{pmatrix} 1 & 2 \\ 3 & 4 \end{pmatrix}";
        let res = render_latex_2d(matrix_src);
        assert!(res.contains('1'));
        assert!(res.contains('2'));
        assert!(res.contains('3'));
        assert!(res.contains('4'));
        assert!(res.contains('⎛') || res.contains('('));
    }

    #[test]
    fn test_2d_summation_rendering() {
        let sum_src = r"\sum_{i=1}^{n} x_i";
        let res = render_latex_2d(sum_src);
        assert!(res.contains('∑'));
        assert!(res.contains('n'));
        assert!(res.contains("i=1"));
        assert!(res.contains("xᵢ"));
    }
}
