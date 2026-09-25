//! ProjectionRenderer trait for deterministic markdown output (PLN-03).

/// Trait implemented by all deterministic planning projection generators.
pub trait ProjectionRenderer {
    /// Renders the domain projection into a complete Markdown string including YAML frontmatter.
    fn render(&self) -> String;
}
