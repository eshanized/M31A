//! Strict prompt and text asset templating engine.
//!
//! Provides compile-time and runtime placeholder interpolation (`{{ placeholder }}`),
//! literal escaping (`{{{{` -> `{{`, `}}}}` -> `}}`), placeholder extraction,
//! and strict/lenient rendering validation.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::error::Error;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TemplateParseError {
    EmptyPlaceholder { start: usize },
    NestedPlaceholder { start: usize },
    UnmatchedClosingDelimiter { start: usize },
    UnterminatedPlaceholder { start: usize },
}

impl fmt::Display for TemplateParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyPlaceholder { start } => write!(f, "Empty placeholder at offset {}", start),
            Self::NestedPlaceholder { start } => {
                write!(f, "Nested placeholder starting at offset {}", start)
            }
            Self::UnmatchedClosingDelimiter { start } => {
                write!(f, "Unmatched closing '}}' at offset {}", start)
            }
            Self::UnterminatedPlaceholder { start } => {
                write!(f, "Unterminated placeholder at offset {}", start)
            }
        }
    }
}

impl Error for TemplateParseError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TemplateRenderError {
    MissingValue { name: String },
}

impl fmt::Display for TemplateRenderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingValue { name } => {
                write!(f, "Missing template value for placeholder '{}'", name)
            }
        }
    }
}

impl Error for TemplateRenderError {}

/// Combined error type for template parsing and rendering.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TemplateError {
    Parse(TemplateParseError),
    Render(TemplateRenderError),
}

impl fmt::Display for TemplateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse(e) => write!(f, "Template parse error: {}", e),
            Self::Render(e) => write!(f, "Template render error: {}", e),
        }
    }
}

impl Error for TemplateError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Parse(e) => Some(e),
            Self::Render(e) => Some(e),
        }
    }
}

impl From<TemplateParseError> for TemplateError {
    fn from(err: TemplateParseError) -> Self {
        Self::Parse(err)
    }
}

impl From<TemplateRenderError> for TemplateError {
    fn from(err: TemplateRenderError) -> Self {
        Self::Render(err)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum TemplateSegment {
    Literal(String),
    Placeholder(String),
}

/// A parsed template ready for strict or lenient variable rendering.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Template {
    segments: Vec<TemplateSegment>,
    placeholders: BTreeSet<String>,
}

impl Template {
    /// Parse a template string with `{{ name }}` placeholders and `{{{{` / `}}}}` escapes.
    pub fn parse(input: &str) -> Result<Self, TemplateParseError> {
        let mut segments = Vec::new();
        let mut placeholders = BTreeSet::new();
        let mut chars = input.char_indices().peekable();
        let mut literal_buf = String::new();

        while let Some((idx, ch)) = chars.next() {
            if ch == '{' {
                if let Some(&(_, next_ch)) = chars.peek()
                    && next_ch == '{'
                {
                    chars.next(); // consume second '{'

                    // Check for escape '{{{{'
                    if let Some(&(_, third_ch)) = chars.peek()
                        && third_ch == '{'
                    {
                        chars.next();
                        if let Some(&(_, fourth_ch)) = chars.peek()
                            && fourth_ch == '{'
                        {
                            chars.next();
                            literal_buf.push_str("{{");
                            continue;
                        }
                    }

                    // Flush any pending literal
                    if !literal_buf.is_empty() {
                        segments.push(TemplateSegment::Literal(std::mem::take(&mut literal_buf)));
                    }

                    // Parse placeholder content until '}}'
                    let mut placeholder_name = String::new();
                    let placeholder_start = idx;
                    let mut terminated = false;

                    while let Some((inner_idx, p_ch)) = chars.next() {
                        if p_ch == '{' {
                            return Err(TemplateParseError::NestedPlaceholder { start: inner_idx });
                        }
                        if p_ch == '}'
                            && let Some(&(_, next_p_ch)) = chars.peek()
                            && next_p_ch == '}'
                        {
                            chars.next(); // consume second '}'
                            let trimmed = placeholder_name.trim();
                            if trimmed.is_empty() {
                                return Err(TemplateParseError::EmptyPlaceholder {
                                    start: placeholder_start,
                                });
                            }
                            let name = trimmed.to_string();
                            placeholders.insert(name.clone());
                            segments.push(TemplateSegment::Placeholder(name));
                            terminated = true;
                            break;
                        }
                        placeholder_name.push(p_ch);
                    }

                    if !terminated {
                        return Err(TemplateParseError::UnterminatedPlaceholder {
                            start: placeholder_start,
                        });
                    }

                    continue;
                }
                literal_buf.push('{');
            } else if ch == '}' {
                if let Some(&(_, next_ch)) = chars.peek()
                    && next_ch == '}'
                {
                    chars.next();
                    // Check for escape '}}}}'
                    if let Some(&(_, third_ch)) = chars.peek()
                        && third_ch == '}'
                    {
                        chars.next();
                        if let Some(&(_, fourth_ch)) = chars.peek()
                            && fourth_ch == '}'
                        {
                            chars.next();
                            literal_buf.push_str("}}");
                            continue;
                        }
                    }
                    return Err(TemplateParseError::UnmatchedClosingDelimiter { start: idx });
                }
                literal_buf.push('}');
            } else {
                literal_buf.push(ch);
            }
        }

        if !literal_buf.is_empty() {
            segments.push(TemplateSegment::Literal(literal_buf));
        }

        Ok(Self {
            segments,
            placeholders,
        })
    }

    /// Return all variable placeholders defined in this template.
    pub fn placeholders(&self) -> &BTreeSet<String> {
        &self.placeholders
    }

    /// Strictly render the template. Returns an error if any placeholder is missing from `values`.
    pub fn render<S: AsRef<str>>(
        &self,
        values: &HashMap<String, S>,
    ) -> Result<String, TemplateRenderError> {
        let mut out = String::new();
        for segment in &self.segments {
            match segment {
                TemplateSegment::Literal(s) => out.push_str(s),
                TemplateSegment::Placeholder(name) => {
                    if let Some(val) = values.get(name) {
                        out.push_str(val.as_ref());
                    } else {
                        return Err(TemplateRenderError::MissingValue { name: name.clone() });
                    }
                }
            }
        }
        Ok(out)
    }

    /// Strictly render from a BTreeMap.
    pub fn render_btree<S: AsRef<str>>(
        &self,
        values: &BTreeMap<String, S>,
    ) -> Result<String, TemplateRenderError> {
        let mut out = String::new();
        for segment in &self.segments {
            match segment {
                TemplateSegment::Literal(s) => out.push_str(s),
                TemplateSegment::Placeholder(name) => {
                    if let Some(val) = values.get(name) {
                        out.push_str(val.as_ref());
                    } else {
                        return Err(TemplateRenderError::MissingValue { name: name.clone() });
                    }
                }
            }
        }
        Ok(out)
    }

    /// Leniently render the template, leaving unknown placeholders in place (`{{ name }}`).
    pub fn render_lenient<S: AsRef<str>>(&self, values: &HashMap<String, S>) -> String {
        let mut out = String::new();
        for segment in &self.segments {
            match segment {
                TemplateSegment::Literal(s) => out.push_str(s),
                TemplateSegment::Placeholder(name) => {
                    if let Some(val) = values.get(name) {
                        out.push_str(val.as_ref());
                    } else {
                        out.push_str("{{ ");
                        out.push_str(name);
                        out.push_str(" }}");
                    }
                }
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_template_parse_and_strict_render() {
        let tmpl = Template::parse("Hello {{ name }}, welcome to {{ project }}!").unwrap();
        assert_eq!(tmpl.placeholders().len(), 2);
        assert!(tmpl.placeholders().contains("name"));
        assert!(tmpl.placeholders().contains("project"));

        let mut vars = HashMap::new();
        vars.insert("name".to_string(), "Developer");
        vars.insert("project".to_string(), "M31A");

        let rendered = tmpl.render(&vars).unwrap();
        assert_eq!(rendered, "Hello Developer, welcome to M31A!");
    }

    #[test]
    fn test_template_missing_value_error() {
        let tmpl = Template::parse("Mission: {{ objective }} by {{ deadline }}").unwrap();
        let mut vars = HashMap::new();
        vars.insert("objective".to_string(), "Achieve 100% parity");

        let err = tmpl.render(&vars).unwrap_err();
        assert_eq!(
            err,
            TemplateRenderError::MissingValue {
                name: "deadline".to_string()
            }
        );

        // Lenient rendering preserves unreplaced tag
        let lenient = tmpl.render_lenient(&vars);
        assert_eq!(lenient, "Mission: Achieve 100% parity by {{ deadline }}");
    }

    #[test]
    fn test_template_escaped_delimiters() {
        let tmpl = Template::parse("Literal {{{{ code }}}} and {{ var }}").unwrap();
        let mut vars = HashMap::new();
        vars.insert("var".to_string(), "evaluated");

        let rendered = tmpl.render(&vars).unwrap();
        assert_eq!(rendered, "Literal {{ code }} and evaluated");
    }

    #[test]
    fn test_template_parse_errors() {
        assert!(matches!(
            Template::parse("Hello {{ }}"),
            Err(TemplateParseError::EmptyPlaceholder { .. })
        ));
        assert!(matches!(
            Template::parse("Hello {{ unclosed"),
            Err(TemplateParseError::UnterminatedPlaceholder { .. })
        ));
        assert!(matches!(
            Template::parse("Hello }} unmatched"),
            Err(TemplateParseError::UnmatchedClosingDelimiter { .. })
        ));
        assert!(matches!(
            Template::parse("Hello {{ nested {{ }} }}"),
            Err(TemplateParseError::NestedPlaceholder { .. })
        ));
    }
}
