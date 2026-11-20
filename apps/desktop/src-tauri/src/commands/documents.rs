//! OpenXML DOCX, XLSX, and PDF document preview parsers for M31A Desktop.

use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocxParagraph {
    pub text: String,
    pub style: Option<String>,
    pub is_heading: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocxTable {
    pub rows: Vec<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", content = "data", rename_all = "snake_case")]
pub enum DocxBlock {
    Paragraph(DocxParagraph),
    Table(DocxTable),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocxPreviewPayload {
    pub file_name: String,
    pub blocks: Vec<DocxBlock>,
    pub total_paragraphs: usize,
    pub total_words: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct XlsxSheet {
    pub name: String,
    pub rows: Vec<Vec<String>>,
    pub max_columns: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct XlsxPreviewPayload {
    pub file_name: String,
    pub sheets: Vec<XlsxSheet>,
    pub active_sheet_index: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PdfPreviewPayload {
    pub file_name: String,
    pub page_count: usize,
    pub title: Option<String>,
    pub author: Option<String>,
    pub file_size_bytes: u64,
    pub text_preview: String,
}

/// Helper function to parse OpenXML DOCX archive into structured paragraphs and tables.
pub fn parse_docx_file(file_path: &Path) -> Result<DocxPreviewPayload, String> {
    let file = File::open(file_path).map_err(|e| format!("Failed to open DOCX file: {e}"))?;
    let mut archive = zip::ZipArchive::new(BufReader::new(file))
        .map_err(|e| format!("Invalid DOCX zip archive: {e}"))?;

    let mut doc_xml_content = String::new();
    {
        let mut doc_file = archive
            .by_name("word/document.xml")
            .map_err(|e| format!("DOCX missing word/document.xml: {e}"))?;
        doc_file
            .read_to_string(&mut doc_xml_content)
            .map_err(|e| format!("Failed to read word/document.xml: {e}"))?;
    }

    let file_name = file_path
        .file_name()
        .map(|f| f.to_string_lossy().to_string())
        .unwrap_or_else(|| "document.docx".to_string());

    let mut blocks = Vec::new();
    let mut total_words = 0;
    let mut total_paragraphs = 0;

    // Process tables and paragraphs by tokenizing XML tags
    // 1. Check for tables <w:tbl>...</w:tbl>
    let mut search_pos = 0;
    while let Some(p_start) = doc_xml_content[search_pos..].find("<w:p") {
        let actual_p_start = search_pos + p_start;
        // Check if there is an intervening table
        if let Some(tbl_start) = doc_xml_content[search_pos..actual_p_start].find("<w:tbl") {
            let actual_tbl_start = search_pos + tbl_start;
            if let Some(tbl_end) = doc_xml_content[actual_tbl_start..].find("</w:tbl>") {
                let tbl_slice = &doc_xml_content[actual_tbl_start..actual_tbl_start + tbl_end + 8];
                let table = parse_docx_table_xml(tbl_slice);
                if !table.rows.is_empty() {
                    blocks.push(DocxBlock::Table(table));
                }
                search_pos = actual_tbl_start + tbl_end + 8;
                continue;
            }
        }

        if let Some(p_end) = doc_xml_content[actual_p_start..].find("</w:p>") {
            let p_slice = &doc_xml_content[actual_p_start..actual_p_start + p_end + 6];
            let paragraph = parse_docx_paragraph_xml(p_slice);
            if !paragraph.text.trim().is_empty() {
                total_words += paragraph.text.split_whitespace().count();
                total_paragraphs += 1;
                blocks.push(DocxBlock::Paragraph(paragraph));
            }
            search_pos = actual_p_start + p_end + 6;
        } else {
            break;
        }
    }

    Ok(DocxPreviewPayload {
        file_name,
        blocks,
        total_paragraphs,
        total_words,
    })
}

fn extract_text_from_runs(xml: &str) -> String {
    let mut text = String::new();
    let mut pos = 0;
    while let Some(t_idx) = xml[pos..].find("<w:t") {
        let actual_t_idx = pos + t_idx;
        let candidate = &xml[actual_t_idx..];
        // Ensure we match <w:t> or <w:t ...>, NOT <w:tc> or other tags
        if candidate.starts_with("<w:t>") || candidate.starts_with("<w:t ") {
            if let Some(tag_close) = candidate.find('>') {
                let content_start = actual_t_idx + tag_close + 1;
                if let Some(t_end) = xml[content_start..].find("</w:t>") {
                    text.push_str(&xml[content_start..content_start + t_end]);
                    pos = content_start + t_end + 6;
                } else {
                    break;
                }
            } else {
                break;
            }
        } else {
            pos = actual_t_idx + 4;
        }
    }
    text
}

fn parse_docx_paragraph_xml(xml: &str) -> DocxParagraph {
    let mut style = None;

    // Style extraction
    if let Some(style_start) = xml.find("<w:pStyle w:val=\"") {
        let rest = &xml[style_start + 17..];
        if let Some(end) = rest.find('"') {
            let val = &rest[..end];
            style = Some(val.to_string());
        }
    }

    let is_heading = style
        .as_ref()
        .map(|s| s.to_lowercase().contains("heading") || s.to_lowercase().contains("title"))
        .unwrap_or(false);

    let text = extract_text_from_runs(xml);

    DocxParagraph {
        text,
        style,
        is_heading,
    }
}

fn parse_docx_table_xml(xml: &str) -> DocxTable {
    let mut rows = Vec::new();
    let mut pos = 0;
    while let Some(tr_start) = xml[pos..].find("<w:tr") {
        let actual_tr_start = pos + tr_start;
        if let Some(tr_end) = xml[actual_tr_start..].find("</w:tr>") {
            let tr_slice = &xml[actual_tr_start..actual_tr_start + tr_end + 7];
            let mut row_cells = Vec::new();

            let mut tc_pos = 0;
            while let Some(tc_start) = tr_slice[tc_pos..].find("<w:tc") {
                let actual_tc_start = tc_pos + tc_start;
                if let Some(tc_end) = tr_slice[actual_tc_start..].find("</w:tc>") {
                    let tc_slice = &tr_slice[actual_tc_start..actual_tc_start + tc_end + 7];
                    let cell_text = extract_text_from_runs(tc_slice);
                    row_cells.push(cell_text.trim().to_string());
                    tc_pos = actual_tc_start + tc_end + 7;
                } else {
                    break;
                }
            }

            if !row_cells.is_empty() {
                rows.push(row_cells);
            }
            pos = actual_tr_start + tr_end + 7;
        } else {
            break;
        }
    }

    DocxTable { rows }
}

/// Helper function to parse OpenXML XLSX archive into tabular sheets and cells.
pub fn parse_xlsx_file(file_path: &Path) -> Result<XlsxPreviewPayload, String> {
    let file = File::open(file_path).map_err(|e| format!("Failed to open XLSX file: {e}"))?;
    let mut archive = zip::ZipArchive::new(BufReader::new(file))
        .map_err(|e| format!("Invalid XLSX zip archive: {e}"))?;

    // 1. Read shared strings if present
    let mut shared_strings = Vec::new();
    if let Ok(mut ss_file) = archive.by_name("xl/sharedStrings.xml") {
        let mut ss_content = String::new();
        let _ = ss_file.read_to_string(&mut ss_content);
        let mut pos = 0;
        while let Some(si_start) = ss_content[pos..].find("<si>") {
            let actual_si = pos + si_start;
            if let Some(si_end) = ss_content[actual_si..].find("</si>") {
                let si_slice = &ss_content[actual_si..actual_si + si_end + 5];
                let mut si_text = String::new();
                let mut t_pos = 0;
                while let Some(t_start) = si_slice[t_pos..].find("<t") {
                    let actual_t = t_pos + t_start;
                    if let Some(tag_close) = si_slice[actual_t..].find('>') {
                        let content_start = actual_t + tag_close + 1;
                        if let Some(t_end) = si_slice[content_start..].find("</t>") {
                            si_text.push_str(&si_slice[content_start..content_start + t_end]);
                            t_pos = content_start + t_end + 4;
                        } else {
                            break;
                        }
                    } else {
                        break;
                    }
                }
                shared_strings.push(si_text);
                pos = actual_si + si_end + 5;
            } else {
                break;
            }
        }
    }

    // 2. Discover sheets
    let mut sheet_names = Vec::new();
    if let Ok(mut wb_file) = archive.by_name("xl/workbook.xml") {
        let mut wb_content = String::new();
        let _ = wb_file.read_to_string(&mut wb_content);
        let mut pos = 0;
        while let Some(sheet_start) = wb_content[pos..].find("<sheet ") {
            let actual_start = pos + sheet_start;
            if let Some(tag_close) = wb_content[actual_start..].find("/>") {
                let tag_slice = &wb_content[actual_start..actual_start + tag_close];
                if let Some(name_pos) = tag_slice.find("name=\"") {
                    let rest = &tag_slice[name_pos + 6..];
                    if let Some(end) = rest.find('"') {
                        sheet_names.push(rest[..end].to_string());
                    }
                }
                pos = actual_start + tag_close + 2;
            } else {
                break;
            }
        }
    }

    if sheet_names.is_empty() {
        sheet_names.push("Sheet1".to_string());
    }

    let mut sheets = Vec::new();
    for (i, name) in sheet_names.iter().enumerate() {
        let sheet_path = format!("xl/worksheets/sheet{}.xml", i + 1);
        if let Ok(mut sheet_file) = archive.by_name(&sheet_path) {
            let mut sheet_content = String::new();
            let _ = sheet_file.read_to_string(&mut sheet_content);
            let (rows, max_cols) = parse_xlsx_sheet_xml(&sheet_content, &shared_strings);
            sheets.push(XlsxSheet {
                name: name.clone(),
                rows,
                max_columns: max_cols,
            });
        }
    }

    let file_name = file_path
        .file_name()
        .map(|f| f.to_string_lossy().to_string())
        .unwrap_or_else(|| "workbook.xlsx".to_string());

    Ok(XlsxPreviewPayload {
        file_name,
        sheets,
        active_sheet_index: 0,
    })
}

fn parse_xlsx_sheet_xml(xml: &str, shared_strings: &[String]) -> (Vec<Vec<String>>, usize) {
    let mut rows = Vec::new();
    let mut max_cols = 0;

    let mut pos = 0;
    while let Some(row_start) = xml[pos..].find("<row ") {
        let actual_row = pos + row_start;
        if let Some(row_end) = xml[actual_row..].find("</row>") {
            let row_slice = &xml[actual_row..actual_row + row_end + 6];
            let mut cells = Vec::new();

            let mut c_pos = 0;
            while let Some(c_start) = row_slice[c_pos..].find("<c ") {
                let actual_c = c_pos + c_start;
                if let Some(c_end) = row_slice[actual_c..].find("</c>") {
                    let c_slice = &row_slice[actual_c..actual_c + c_end + 4];
                    let is_shared_string = c_slice.contains("t=\"s\"");

                    let mut cell_value = String::new();
                    if let Some(v_start) = c_slice.find("<v>") {
                        let content_start = v_start + 3;
                        if let Some(v_end) = c_slice[content_start..].find("</v>") {
                            let val_str = &c_slice[content_start..content_start + v_end];
                            if is_shared_string {
                                if let Ok(idx) = val_str.parse::<usize>()
                                    && let Some(shared_val) = shared_strings.get(idx)
                                {
                                    cell_value = shared_val.clone();
                                }
                            } else {
                                cell_value = val_str.to_string();
                            }
                        }
                    }

                    cells.push(cell_value);
                    c_pos = actual_c + c_end + 4;
                } else {
                    break;
                }
            }

            if cells.len() > max_cols {
                max_cols = cells.len();
            }
            if !cells.is_empty() {
                rows.push(cells);
            }
            pos = actual_row + row_end + 6;
        } else {
            break;
        }
    }

    (rows, max_cols)
}

/// Helper function to parse PDF metadata and extract text previews.
pub fn parse_pdf_file(file_path: &Path) -> Result<PdfPreviewPayload, String> {
    let metadata = std::fs::metadata(file_path)
        .map_err(|e| format!("Failed to read PDF file metadata: {e}"))?;
    let file_size_bytes = metadata.len();

    let mut file = File::open(file_path).map_err(|e| format!("Failed to open PDF file: {e}"))?;
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer)
        .map_err(|e| format!("Failed to read PDF bytes: {e}"))?;

    let file_name = file_path
        .file_name()
        .map(|f| f.to_string_lossy().to_string())
        .unwrap_or_else(|| "document.pdf".to_string());

    // Basic PDF parsing: count pages via /Type /Page occurrences
    let content_lossy = String::from_utf8_lossy(&buffer);
    let mut page_count = content_lossy.matches("/Type /Page").count();
    if page_count == 0 {
        page_count = 1;
    }

    // Extract title if present in Info dict
    let mut title = None;
    if let Some(title_pos) = content_lossy.find("/Title (") {
        let rest = &content_lossy[title_pos + 8..];
        if let Some(end) = rest.find(')') {
            title = Some(rest[..end].to_string());
        }
    }

    // Extract author if present
    let mut author = None;
    if let Some(author_pos) = content_lossy.find("/Author (") {
        let rest = &content_lossy[author_pos + 9..];
        if let Some(end) = rest.find(')') {
            author = Some(rest[..end].to_string());
        }
    }

    // Extract printable ASCII text runs
    let text_preview = content_lossy
        .lines()
        .filter(|line| !line.starts_with('%') && !line.starts_with("endobj") && line.len() > 10)
        .take(30)
        .collect::<Vec<&str>>()
        .join("\n");

    Ok(PdfPreviewPayload {
        file_name,
        page_count,
        title,
        author,
        file_size_bytes,
        text_preview,
    })
}

// ---------------------------------------------------------------------------
// Tauri IPC Commands
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn preview_docx_document(path: String) -> Result<DocxPreviewPayload, String> {
    parse_docx_file(Path::new(&path))
}

#[tauri::command]
pub async fn preview_xlsx_spreadsheet(path: String) -> Result<XlsxPreviewPayload, String> {
    parse_xlsx_file(Path::new(&path))
}

#[tauri::command]
pub async fn preview_pdf_document(path: String) -> Result<PdfPreviewPayload, String> {
    parse_pdf_file(Path::new(&path))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_parse_docx_archive() {
        let mut temp_file = NamedTempFile::new().unwrap();
        {
            let mut zip = zip::ZipWriter::new(&mut temp_file);
            let options = zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Stored);

            zip.start_file("word/document.xml", options).unwrap();
            let doc_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
    <w:body>
        <w:p>
            <w:pPr><w:pStyle w:val="Heading1"/></w:pPr>
            <w:r><w:t>Architecture Specification</w:t></w:r>
        </w:p>
        <w:p>
            <w:r><w:t>This is a paragraph describing M31A autonomous systems.</w:t></w:r>
        </w:p>
        <w:tbl>
            <w:tr>
                <w:tc><w:p><w:r><w:t>Module</w:t></w:r></w:p></w:tc>
                <w:tc><w:p><w:r><w:t>Status</w:t></w:r></w:p></w:tc>
            </w:tr>
            <w:tr>
                <w:tc><w:p><w:r><w:t>m31a-cua</w:t></w:r></w:p></w:tc>
                <w:tc><w:p><w:r><w:t>Verified</w:t></w:r></w:p></w:tc>
            </w:tr>
        </w:tbl>
    </w:body>
</w:document>"#;
            zip.write_all(doc_xml.as_bytes()).unwrap();
            zip.finish().unwrap();
        }

        let res = parse_docx_file(temp_file.path()).expect("DOCX parsing failed");
        assert_eq!(res.total_paragraphs, 2);
        assert_eq!(res.blocks.len(), 3);

        match &res.blocks[0] {
            DocxBlock::Paragraph(p) => {
                assert_eq!(p.text, "Architecture Specification");
                assert!(p.is_heading);
            }
            _ => panic!("Expected Paragraph"),
        }

        match &res.blocks[2] {
            DocxBlock::Table(tbl) => {
                assert_eq!(tbl.rows.len(), 2);
                assert_eq!(tbl.rows[0], vec!["Module", "Status"]);
                assert_eq!(tbl.rows[1], vec!["m31a-cua", "Verified"]);
            }
            _ => panic!("Expected Table"),
        }
    }

    #[test]
    fn test_parse_xlsx_archive() {
        let mut temp_file = NamedTempFile::new().unwrap();
        {
            let mut zip = zip::ZipWriter::new(&mut temp_file);
            let options = zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Stored);

            zip.start_file("xl/workbook.xml", options).unwrap();
            let wb_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
    <sheets>
        <sheet name="Metrics" sheetId="1" r:id="rId1"/>
    </sheets>
</workbook>"#;
            zip.write_all(wb_xml.as_bytes()).unwrap();

            zip.start_file("xl/sharedStrings.xml", options).unwrap();
            let ss_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<sst xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
    <si><t>Benchmark</t></si>
    <si><t>Throughput</t></si>
    <si><t>Latency</t></si>
</sst>"#;
            zip.write_all(ss_xml.as_bytes()).unwrap();

            zip.start_file("xl/worksheets/sheet1.xml", options).unwrap();
            let sheet1_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
    <sheetData>
        <row r="1">
            <c r="A1" t="s"><v>0</v></c>
            <c r="B1" t="s"><v>1</v></c>
        </row>
        <row r="2">
            <c r="A2" t="s"><v>2</v></c>
            <c r="B2"><v>55.54</v></c>
        </row>
    </sheetData>
</worksheet>"#;
            zip.write_all(sheet1_xml.as_bytes()).unwrap();
            zip.finish().unwrap();
        }

        let res = parse_xlsx_file(temp_file.path()).expect("XLSX parsing failed");
        assert_eq!(res.sheets.len(), 1);
        let sheet = &res.sheets[0];
        assert_eq!(sheet.name, "Metrics");
        assert_eq!(sheet.rows.len(), 2);
        assert_eq!(sheet.rows[0], vec!["Benchmark", "Throughput"]);
        assert_eq!(sheet.rows[1], vec!["Latency", "55.54"]);
    }

    #[test]
    fn test_parse_pdf_metadata() {
        let mut temp_file = NamedTempFile::new().unwrap();
        let pdf_content = b"%PDF-1.4\n1 0 obj\n<< /Title (M31A Whitepaper) /Author (Eshan Roy) >>\nendobj\n2 0 obj\n<< /Type /Page >>\nendobj\n3 0 obj\n<< /Type /Page >>\nendobj\n%%EOF";
        temp_file.write_all(pdf_content).unwrap();

        let res = parse_pdf_file(temp_file.path()).expect("PDF parsing failed");
        assert_eq!(res.page_count, 2);
        assert_eq!(res.title.as_deref(), Some("M31A Whitepaper"));
        assert_eq!(res.author.as_deref(), Some("Eshan Roy"));
    }
}
