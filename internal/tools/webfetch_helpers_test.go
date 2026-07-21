package tools

import (
	"strings"
	"testing"

	"github.com/eshanized/M31A/internal/tools/search"
)

func TestHtmlToMarkdown_Headings(t *testing.T) {
	t.Parallel()
	input := "<h1>Title</h1><h2>Subtitle</h2><p>text</p>"
	got := search.HtmlToMarkdown(input)
	if got == "" {
		t.Error("expected non-empty markdown output")
	}
}

func TestHtmlToMarkdown_Links(t *testing.T) {
	t.Parallel()
	input := `<p>Visit <a href="https://example.com">Example</a></p>`
	got := search.HtmlToMarkdown(input)
	if got == "" {
		t.Error("expected non-empty output")
	}
}

func TestHtmlToMarkdown_BoldItalic(t *testing.T) {
	t.Parallel()
	input := "<strong>bold</strong> and <em>italic</em>"
	got := search.HtmlToMarkdown(input)
	if got == "" {
		t.Error("expected non-empty output")
	}
}

func TestHtmlToMarkdown_StripScriptStyle(t *testing.T) {
	t.Parallel()
	input := "<script>alert('x')</script><style>.foo{}</style><p>visible</p>"
	got := search.HtmlToMarkdown(input)
	if got == "" {
		t.Error("expected 'visible' content after stripping script/style")
	}
}

func TestHtmlToText(t *testing.T) {
	t.Parallel()
	input := "<p>Hello <b>World</b></p><script>alert('x')</script>"
	got := search.HtmlToText(input)
	if got == "" {
		t.Error("expected non-empty text output")
	}
}

func TestStripTags_Script(t *testing.T) {
	t.Parallel()
	input := "before<script>evil</script>after"
	got := search.StripTags(input, "script")
	if got != "beforeafter" {
		t.Errorf("search.StripTags = %q, want 'beforeafter'", got)
	}
}

func TestStripTags_Multiple(t *testing.T) {
	t.Parallel()
	input := "<style>css</style>text<script>js</script>"
	got := search.StripTags(input, "script", "style")
	if got != "text" {
		t.Errorf("search.StripTags = %q, want 'text'", got)
	}
}

func TestReplaceBlockTag(t *testing.T) {
	t.Parallel()
	input := "<p>hello</p><p>world</p>"
	lower := strings.ToLower(input)
	got, _ := search.ReplaceBlockTag(input, lower, "p", "\n")
	if got == input {
		t.Error("expected replacement to occur")
	}
}

func TestReplaceInlineTag(t *testing.T) {
	t.Parallel()
	input := "<strong>bold</strong>"
	lower := strings.ToLower(input)
	got := search.ReplaceInlineTag(input, lower, "strong", "**")
	if got != "**bold**" {
		t.Errorf("search.ReplaceInlineTag = %q, want '**bold**'", got)
	}
}

func TestConvertLinks(t *testing.T) {
	t.Parallel()
	input := `<a href="https://example.com">Example</a>`
	lower := strings.ToLower(input)
	got := search.ConvertLinks(input, lower)
	expected := "[Example](https://example.com)"
	if got != expected {
		t.Errorf("search.ConvertLinks = %q, want %q", got, expected)
	}
}

func TestConvertLinks_NoHref(t *testing.T) {
	t.Parallel()
	input := "<a>no link</a>"
	lower := strings.ToLower(input)
	got := search.ConvertLinks(input, lower)
	if got == "" {
		t.Error("expected non-empty output for link without href")
	}
}

func TestConvertLinks_MultipleLinks(t *testing.T) {
	t.Parallel()
	input := `<a href="url1">Link1</a> and <a href="url2">Link2</a>`
	lower := strings.ToLower(input)
	got := search.ConvertLinks(input, lower)
	if got == input {
		t.Error("expected links to be converted")
	}
}
