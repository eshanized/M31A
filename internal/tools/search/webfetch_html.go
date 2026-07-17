package search

import (
	"fmt"
	"html"
	"strings"
)

func HtmlToMarkdown(rawHTML string) string {
	// Strip script and style elements first
	rawHTML = StripTags(rawHTML, "script", "style")

	// Compute lowercase once for all case-insensitive tag matching
	lower := strings.ToLower(rawHTML)

	// Handle tables: convert <table> to markdown tables
	rawHTML, lower = convertTables(rawHTML, lower)

	// Handle common block elements with newlines
	rawHTML, lower = ReplaceBlockTag(rawHTML, lower, "p", "\n\n")
	rawHTML, lower = ReplaceBlockTag(rawHTML, lower, "div", "\n")
	rawHTML, lower = ReplaceBlockTag(rawHTML, lower, "br", "\n")
	rawHTML, lower = ReplaceBlockTag(rawHTML, lower, "hr", "\n---\n")

	// Handle headings with proper markdown
	rawHTML, lower = ReplaceBlockTag(rawHTML, lower, "h1", "\n# ")
	rawHTML, lower = ReplaceBlockTag(rawHTML, lower, "h2", "\n## ")
	rawHTML, lower = ReplaceBlockTag(rawHTML, lower, "h3", "\n### ")
	rawHTML, lower = ReplaceBlockTag(rawHTML, lower, "h4", "\n#### ")
	rawHTML, lower = ReplaceBlockTag(rawHTML, lower, "h5", "\n##### ")
	rawHTML, lower = ReplaceBlockTag(rawHTML, lower, "h6", "\n###### ")

	// Handle blockquotes
	rawHTML, lower = ReplaceBlockTag(rawHTML, lower, "blockquote", "\n> ")

	// Handle unordered lists
	rawHTML, lower = ReplaceBlockTag(rawHTML, lower, "ul", "\n")
	rawHTML, _ = ReplaceBlockTag(rawHTML, lower, "li", "\n- ")

	// Handle ordered lists (convert <li> inside <ol> to numbered)
	rawHTML = convertOrderedList(rawHTML)

	// Handle links: <a href="url">text</a> -> [text](url)
	rawHTML = ConvertLinks(rawHTML, strings.ToLower(rawHTML))
	lower = strings.ToLower(rawHTML)

	// Handle images: <img src="url" alt="text"> -> ![text](url)
	rawHTML = convertImages(rawHTML, lower)
	lower = strings.ToLower(rawHTML)

	// Handle inline formatting
	rawHTML = ReplaceInlineTag(rawHTML, lower, "strong", "**")
	lower = strings.ToLower(rawHTML)
	rawHTML = ReplaceInlineTag(rawHTML, lower, "b", "**")
	lower = strings.ToLower(rawHTML)
	rawHTML = ReplaceInlineTag(rawHTML, lower, "em", "*")
	lower = strings.ToLower(rawHTML)
	rawHTML = ReplaceInlineTag(rawHTML, lower, "i", "*")
	lower = strings.ToLower(rawHTML)
	rawHTML = ReplaceInlineTag(rawHTML, lower, "code", "`")
	lower = strings.ToLower(rawHTML)
	rawHTML = ReplaceInlineTag(rawHTML, lower, "kbd", "`")
	lower = strings.ToLower(rawHTML)
	rawHTML = ReplaceInlineTag(rawHTML, lower, "samp", "`")
	lower = strings.ToLower(rawHTML)
	rawHTML = ReplaceInlineTag(rawHTML, lower, "pre", "\n```\n")

	// Handle <abbr title="...">text</abbr> -> text (title)
	rawHTML = convertAbbreviations(rawHTML, strings.ToLower(rawHTML))

	// Strip all remaining tags
	rawHTML = StripAllTags(rawHTML)

	// Decode common HTML entities
	rawHTML = decodeHTMLEntities(rawHTML)

	// Normalize whitespace
	rawHTML = NormalizeWhitespace(rawHTML)

	return strings.TrimSpace(rawHTML)
}

// convertTables converts HTML tables to markdown table format.
func convertTables(rawHTML, lower string) (string, string) {
	type tableMatch struct {
		start int
		end   int
		html  string
	}

	var tables []tableMatch
	searchPos := 0

	for {
		idx := strings.Index(lower[searchPos:], "<table")
		if idx == -1 {
			break
		}
		start := searchPos + idx

		// Find matching </table>
		endIdx := strings.Index(lower[start:], "</table>")
		if endIdx == -1 {
			break
		}
		end := start + endIdx + len("</table>")

		tables = append(tables, tableMatch{
			start: start,
			end:   end,
			html:  rawHTML[start:end],
		})
		searchPos = end
	}

	if len(tables) == 0 {
		return rawHTML, lower
	}

	// Process tables in reverse order to preserve positions
	for i := len(tables) - 1; i >= 0; i-- {
		t := tables[i]
		md := HtmlTableToMarkdown(t.html)
		rawHTML = rawHTML[:t.start] + "\n" + md + "\n" + rawHTML[t.end:]
	}

	return rawHTML, strings.ToLower(rawHTML)
}

// HtmlTableToMarkdown converts a single HTML table to markdown.
func HtmlTableToMarkdown(tableHTML string) string {
	var rows [][]string
	var headerIdx int

	// Find all <tr> sections
	lower := strings.ToLower(tableHTML)
	searchPos := 0
	for {
		trIdx := strings.Index(lower[searchPos:], "<tr")
		if trIdx == -1 {
			break
		}
		trStart := searchPos + trIdx
		trEndIdx := strings.Index(lower[trStart:], "</tr>")
		if trEndIdx == -1 {
			break
		}
		trEnd := trStart + trEndIdx + len("</tr>")
		trHTML := tableHTML[trStart:trEnd]

		// Check if this is a header row
		if strings.Contains(strings.ToLower(trHTML), "<th>") {
			headerIdx = len(rows)
		}

		// Extract cell contents
		var cells []string
		cellLower := strings.ToLower(trHTML)
		cellSearchPos := 0
		for {
			thIdx := strings.Index(cellLower[cellSearchPos:], "<t") // matches <th> and <td>
			if thIdx == -1 {
				break
			}
			cellStart := cellSearchPos + thIdx
			// Find >
			gtIdx := strings.IndexByte(cellLower[cellStart:], '>')
			if gtIdx == -1 {
				break
			}
			cellContentStart := cellStart + gtIdx + 1
			// Find closing tag
			closeTagIdx := strings.Index(cellLower[cellContentStart:], "</t")
			if closeTagIdx == -1 {
				break
			}
			cellContent := strings.TrimSpace(tableHTML[cellContentStart : cellContentStart+closeTagIdx])
			// Strip any inner tags
			cellContent = StripAllTags(cellContent)
			cellContent = decodeHTMLEntities(cellContent)
			cellContent = strings.TrimSpace(cellContent)
			// Escape pipes in cell content
			cellContent = strings.ReplaceAll(cellContent, "|", "\\|")
			cells = append(cells, cellContent)
			cellSearchPos = cellContentStart + closeTagIdx + 4
		}
		if len(cells) > 0 {
			rows = append(rows, cells)
		}
		searchPos = trEnd
	}

	if len(rows) == 0 {
		return tableHTML
	}

	// Determine max columns
	maxCols := 0
	for _, row := range rows {
		if len(row) > maxCols {
			maxCols = len(row)
		}
	}

	var sb strings.Builder
	for i, row := range rows {
		// Pad row to max columns
		for len(row) < maxCols {
			row = append(row, "")
		}
		sb.WriteString("| ")
		sb.WriteString(strings.Join(row, " | "))
		sb.WriteString(" |\n")

		// Add separator after header
		if i == headerIdx || (i == 0 && len(rows) > 1) {
			separators := make([]string, maxCols)
			for j := range separators {
				separators[j] = "---"
			}
			sb.WriteString("| ")
			sb.WriteString(strings.Join(separators, " | "))
			sb.WriteString(" |\n")
		}
	}

	return sb.String()
}

// convertOrderedList converts HTML ordered lists to numbered markdown lists.
func convertOrderedList(rawHTML string) string {
	// Find <ol>...</ol> blocks and convert <li> to numbered items
	lower := strings.ToLower(rawHTML)
	result := rawHTML
	searchPos := 0

	for {
		olIdx := strings.Index(lower[searchPos:], "<ol")
		if olIdx == -1 {
			break
		}
		olStart := searchPos + olIdx
		olEndIdx := strings.Index(lower[olStart:], "</ol>")
		if olEndIdx == -1 {
			break
		}
		olEnd := olStart + olEndIdx + len("</ol>")
		olHTML := rawHTML[olStart:olEnd]

		// Replace <li> with numbered items
		var sb strings.Builder
		counter := 1
		liLower := strings.ToLower(olHTML)
		liSearchPos := 0
		liPrevEnd := 0
		for {
			liIdx := strings.Index(liLower[liSearchPos:], "<li")
			if liIdx == -1 {
				// Copy remaining content
				sb.WriteString(olHTML[liPrevEnd:])
				break
			}
			liStart := liSearchPos + liIdx
			// Copy content before this <li>
			sb.WriteString(olHTML[liPrevEnd:liStart])

			// Find >
			gtIdx := strings.IndexByte(liLower[liStart:], '>')
			if gtIdx == -1 {
				sb.WriteString(olHTML[liPrevEnd:])
				break
			}
			contentStart := liStart + gtIdx + 1
			// Find </li>
			closeIdx := strings.Index(liLower[contentStart:], "</li>")
			if closeIdx == -1 {
				sb.WriteString(olHTML[liPrevEnd:])
				break
			}
			content := strings.TrimSpace(olHTML[contentStart : contentStart+closeIdx])
			fmt.Fprintf(&sb, "\n%d. %s", counter, content)
			counter++
			liPrevEnd = contentStart + closeIdx + len("</li>")
			liSearchPos = liPrevEnd
		}

		result = result[:olStart] + sb.String() + result[olEnd:]
		lower = strings.ToLower(result)
		searchPos = olStart + sb.Len()
	}

	return result
}

// convertImages converts <img> tags to markdown image syntax.
func convertImages(rawHTML, lower string) string {
	type imgMatch struct {
		start int
		end   int
		alt   string
		src   string
	}
	var matches []imgMatch

	searchPos := 0
	for {
		idx := strings.Index(lower[searchPos:], "<img")
		if idx == -1 {
			break
		}
		start := searchPos + idx
		// Find self-closing />
		endIdx := strings.Index(lower[start:], "/>")
		if endIdx == -1 {
			// Try >
			endIdx = strings.Index(lower[start:], ">")
			if endIdx == -1 {
				break
			}
		}
		end := start + endIdx + 2
		tagHTML := rawHTML[start:end]
		tagLower := lower[start:end]

		// Extract src
		src := ""
		if srcIdx := strings.Index(tagLower, "src="); srcIdx >= 0 {
			srcStart := srcIdx + 5
			if srcStart < len(tagLower) {
				quote := tagLower[srcStart]
				if quote == '"' || quote == '\'' {
					srcEnd := strings.Index(tagLower[srcStart+1:], string(quote))
					if srcEnd >= 0 {
						src = tagHTML[srcStart+1 : srcStart+1+srcEnd]
					}
				}
			}
		}

		// Extract alt
		alt := ""
		if altIdx := strings.Index(tagLower, "alt="); altIdx >= 0 {
			altStart := altIdx + 5
			if altStart < len(tagLower) {
				quote := tagLower[altStart]
				if quote == '"' || quote == '\'' {
					altEnd := strings.Index(tagLower[altStart+1:], string(quote))
					if altEnd >= 0 {
						alt = tagHTML[altStart+1 : altStart+1+altEnd]
					}
				}
			}
		}

		if src != "" {
			matches = append(matches, imgMatch{start: start, end: end, alt: alt, src: src})
		}
		searchPos = end
	}

	if len(matches) == 0 {
		return rawHTML
	}

	// Apply in reverse order
	result := rawHTML
	for i := len(matches) - 1; i >= 0; i-- {
		m := matches[i]
		md := fmt.Sprintf("![%s](%s)", m.alt, m.src)
		result = result[:m.start] + md + result[m.end:]
	}
	return result
}

// convertAbbreviations removes <abbr> tags, keeping just the text content.
func convertAbbreviations(rawHTML, lower string) string {
	// Simple approach: just strip <abbr ...> and </abbr> tags
	rawHTML = ReplaceInlineTag(rawHTML, lower, "abbr", "")
	return rawHTML
}

// htmlToText extracts plain text from HTML.
func HtmlToText(rawHTML string) string {
	rawHTML = StripTags(rawHTML, "script", "style")
	lower := strings.ToLower(rawHTML)
	rawHTML, lower = ReplaceBlockTag(rawHTML, lower, "p", "\n\n")
	rawHTML, lower = ReplaceBlockTag(rawHTML, lower, "br", "\n")
	rawHTML, lower = ReplaceBlockTag(rawHTML, lower, "li", "\n- ")
	rawHTML, _ = ReplaceBlockTag(rawHTML, lower, "blockquote", "\n> ")
	rawHTML = StripAllTags(rawHTML)

	// Decode entities
	rawHTML = decodeHTMLEntities(rawHTML)

	return NormalizeWhitespace(rawHTML)
}

// StripTags removes all occurrences of the given HTML tags and their content.
// Uses strings.Builder for efficient string construction instead of O(N²) concatenation.
func StripTags(rawHTML string, tags ...string) string {
	for _, tag := range tags {
		openTag := "<" + tag
		closeTag := "</" + tag + ">"
		var b strings.Builder
		b.Grow(len(rawHTML))
		pos := 0
		for {
			start := strings.Index(rawHTML[pos:], openTag)
			if start == -1 {
				break
			}
			start += pos

			tagEnd := strings.IndexByte(rawHTML[start:], '>')
			if tagEnd == -1 {
				break
			}
			tagEnd += start + 1

			// Check for self-closing tag (/> at end)
			if strings.HasSuffix(rawHTML[start:tagEnd], "/") {
				b.WriteString(rawHTML[pos:start])
				pos = tagEnd
				continue
			}

			// Find closing tag
			closeStart := strings.Index(rawHTML[tagEnd:], closeTag)
			if closeStart == -1 {
				break
			}
			closeStart += tagEnd
			closeEnd := closeStart + len(closeTag)
			b.WriteString(rawHTML[pos:start])
			pos = closeEnd
		}
		b.WriteString(rawHTML[pos:])
		rawHTML = b.String()
	}
	return rawHTML
}

// replaceBlockTag replaces block-level HTML tags with the given replacement string.
// Returns both the modified HTML and its pre-computed lowercase, avoiding the
// caller needing to recompute strings.ToLower after every call.
func ReplaceBlockTag(rawHTML, lower, tag, replacement string) (string, string) {
	openTag := "<" + tag
	closeTag := "</" + tag + ">"
	var result strings.Builder
	result.Grow(len(rawHTML))
	pos := 0
	for {
		start := strings.Index(lower[pos:], openTag)
		if start == -1 {
			break
		}
		start += pos

		tagEnd := strings.IndexByte(rawHTML[start:], '>')
		if tagEnd == -1 {
			break
		}
		tagEnd += start + 1

		closeStart := strings.Index(lower[tagEnd:], closeTag)
		if closeStart == -1 {
			break
		}
		closeStart += tagEnd
		closeEnd := closeStart + len(closeTag)

		result.WriteString(rawHTML[pos:start])
		result.WriteString(replacement)
		result.WriteString(rawHTML[tagEnd:closeStart])
		pos = closeEnd
	}
	result.WriteString(rawHTML[pos:])
	newHTML := result.String()
	return newHTML, strings.ToLower(newHTML)
}

// replaceInlineTag replaces inline HTML tags with the given marker around inner content.
// Collects all match positions first, then applies replacements in reverse order
// so earlier positions remain valid. Only one ToLower recompute is needed after
// the caller uses the result.
func ReplaceInlineTag(rawHTML, lower, tag, marker string) string {
	openTag := "<" + tag
	closeTag := "</" + tag + ">"

	type tagMatch struct {
		openStart  int // position of '<' of opening tag
		closeStart int // position of '<' of closing tag
		closeEnd   int // position after '>' of closing tag
	}
	var matches []tagMatch

	searchPos := 0
	for {
		start := strings.Index(lower[searchPos:], openTag)
		if start == -1 {
			break
		}
		start += searchPos

		tagEnd := strings.IndexByte(rawHTML[start:], '>')
		if tagEnd == -1 {
			break
		}
		tagEnd += start + 1

		closeStart := strings.Index(lower[tagEnd:], closeTag)
		if closeStart == -1 {
			break
		}
		closeStart += tagEnd
		closeEnd := closeStart + len(closeTag)

		matches = append(matches, tagMatch{
			openStart:  start,
			closeStart: closeStart,
			closeEnd:   closeEnd,
		})
		searchPos = closeEnd
	}

	if len(matches) == 0 {
		return rawHTML
	}

	// Build result in forward order
	var b strings.Builder
	b.Grow(len(rawHTML) + len(matches)*(len(marker)*2))
	prevEnd := 0
	for _, m := range matches {
		b.WriteString(rawHTML[prevEnd:m.openStart])
		b.WriteString(marker)
		// Find the '>' of the opening tag to get inner content start
		openGT := strings.IndexByte(rawHTML[m.openStart:], '>')
		if openGT == -1 {
			continue
		}
		innerStart := m.openStart + openGT + 1 // position after '>'
		b.WriteString(rawHTML[innerStart:m.closeStart])
		b.WriteString(marker)
		prevEnd = m.closeEnd
	}
	b.WriteString(rawHTML[prevEnd:])
	return b.String()
}

// ConvertLinks converts <a href="url">text</a> to [text](url).
// Collects all link matches first, then applies replacements in reverse order
// so position shifts don't invalidate earlier indices.
func ConvertLinks(rawHTML, lower string) string {
	type linkMatch struct {
		start    int
		closeEnd int
		url      string
		text     string
	}
	var matches []linkMatch

	searchPos := 0
	for {
		start := strings.Index(lower[searchPos:], "<a ")
		if start == -1 {
			break
		}
		start += searchPos

		// Find href
		hrefIdx := strings.Index(lower[start:], "href=")
		if hrefIdx == -1 {
			// No href, skip this tag
			tagEnd := strings.IndexByte(rawHTML[start:], '>')
			if tagEnd == -1 {
				break
			}
			searchPos = start + tagEnd + 1
			continue
		}
		hrefStart := start + hrefIdx + 5 // len("href=") = 5

		if hrefStart >= len(rawHTML) {
			break
		}

		// Extract URL (handle quoted and unquoted)
		var linkURL string
		quoteChar := rawHTML[hrefStart]
		if quoteChar == '"' || quoteChar == '\'' {
			urlStart := hrefStart + 1
			if urlStart >= len(rawHTML) {
				break
			}
			urlEnd := strings.Index(rawHTML[urlStart:], string(quoteChar))
			if urlEnd == -1 {
				break
			}
			linkURL = rawHTML[urlStart : urlStart+urlEnd]
		} else {
			urlEnd := hrefStart
			for urlEnd < len(rawHTML) && rawHTML[urlEnd] != ' ' && rawHTML[urlEnd] != '>' && rawHTML[urlEnd] != '\t' && rawHTML[urlEnd] != '\n' {
				urlEnd++
			}
			linkURL = rawHTML[hrefStart:urlEnd]
		}

		// Find end of opening tag
		openGT := strings.IndexByte(rawHTML[start:], '>')
		if openGT == -1 {
			break
		}
		tagEndPos := start + openGT + 1

		// Find closing tag
		closeStart := strings.Index(lower[tagEndPos:], "</a>")
		if closeStart == -1 {
			break
		}
		closeStart += tagEndPos
		closeEnd := closeStart + 4 // len("</a>")

		text := rawHTML[tagEndPos:closeStart]
		matches = append(matches, linkMatch{
			start:    start,
			closeEnd: closeEnd,
			url:      linkURL,
			text:     text,
		})
		searchPos = closeEnd
	}

	if len(matches) == 0 {
		return rawHTML
	}

	// Build the result by writing non-matched segments in forward order
	result := make([]byte, 0, len(rawHTML)+len(matches)*4)
	result = append(result, rawHTML[:matches[0].start]...)
	for i, m := range matches {
		result = append(result, '[')
		result = append(result, m.text...)
		result = append(result, ']')
		result = append(result, '(')
		result = append(result, m.url...)
		result = append(result, ')')
		if i+1 < len(matches) {
			result = append(result, rawHTML[m.closeEnd:matches[i+1].start]...)
		}
	}
	result = append(result, rawHTML[matches[len(matches)-1].closeEnd:]...)
	return string(result)
}

// decodeHTMLEntities replaces HTML entities with their character equivalents.
// Uses the standard library's html.UnescapeString for comprehensive coverage
// of named, numeric, and hex entities. Non-breaking spaces are normalised to
// regular spaces for downstream text processing.
func decodeHTMLEntities(s string) string {
	s = html.UnescapeString(s)
	return strings.ReplaceAll(s, "\u00a0", " ")
}

func StripAllTags(rawHTML string) string {
	var b strings.Builder
	inTag := false
	for _, ch := range rawHTML {
		if ch == '<' {
			inTag = true
			continue
		}
		if ch == '>' {
			inTag = false
			continue
		}
		if !inTag {
			b.WriteRune(ch)
		}
	}
	return b.String()
}

func NormalizeWhitespace(s string) string {
	var b strings.Builder
	b.Grow(len(s))
	prevNewlines := 0
	inSpace := false
	for _, ch := range s {
		switch ch {
		case '\n':
			prevNewlines++
			inSpace = false
			if prevNewlines <= 2 {
				b.WriteRune(ch)
			}
		case ' ', '\t':
			prevNewlines = 0
			if !inSpace {
				b.WriteRune(' ')
				inSpace = true
			}
		default:
			prevNewlines = 0
			inSpace = false
			b.WriteRune(ch)
		}
	}
	return strings.TrimSpace(b.String())
}