use anyhow::Result;
use std::path::Path;

pub trait DocumentParser: Send + Sync {
    fn can_parse(&self, file_path: &str) -> bool;
    fn extract_text(&self, file_path: &str) -> Result<String>;
}

pub struct TextParser;

impl DocumentParser for TextParser {
    fn can_parse(&self, file_path: &str) -> bool {
        let ext = Path::new(file_path)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();
        
        matches!(ext.as_str(), "txt" | "md" | "js" | "ts" | "py" | "rs" | "java" | "cpp" | "c" | "h" | "hpp" | "json" | "xml" | "html" | "css" | "yaml" | "yml" | "toml" | "ini" | "log")
    }

    fn extract_text(&self, file_path: &str) -> Result<String> {
        Ok(std::fs::read_to_string(file_path)?)
    }
}

pub struct PdfParser;

impl DocumentParser for PdfParser {
    fn can_parse(&self, file_path: &str) -> bool {
        Path::new(file_path)
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.eq_ignore_ascii_case("pdf"))
            .unwrap_or(false)
    }

    fn extract_text(&self, file_path: &str) -> Result<String> {
        let text = pdf_extract::extract_text(file_path)?;
        Ok(text)
    }
}

pub struct DocxParser;

impl DocumentParser for DocxParser {
    fn can_parse(&self, file_path: &str) -> bool {
        Path::new(file_path)
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.eq_ignore_ascii_case("docx"))
            .unwrap_or(false)
    }

    fn extract_text(&self, file_path: &str) -> Result<String> {
        use docx_rs::read_docx;
        use std::fs::File;
        use std::io::Read;
        
        let mut file = File::open(file_path)?;
        let mut buf = Vec::new();
        file.read_to_end(&mut buf)?;
        
        let docx = read_docx(&buf)?;
        let mut text_parts = Vec::new();
        
        for child in docx.document.children.iter() {
            if let docx_rs::DocumentChild::Paragraph(para) = child {
                let mut para_text_parts = Vec::new();
                for run_child in para.children.iter() {
                    if let docx_rs::ParagraphChild::Run(run) = run_child {
                        let run_text: String = run.children.iter().filter_map(|text| {
                            if let docx_rs::RunChild::Text(t) = text {
                                Some(t.text.clone())
                            } else {
                                None
                            }
                        }).collect::<Vec<_>>().join("");
                        if !run_text.is_empty() {
                            para_text_parts.push(run_text);
                        }
                    }
                }
                if !para_text_parts.is_empty() {
                    text_parts.push(para_text_parts.join(" "));
                }
            }
        }
        
        Ok(text_parts.join("\n"))
    }
}

pub struct XlsxParser;

impl DocumentParser for XlsxParser {
    fn can_parse(&self, file_path: &str) -> bool {
        Path::new(file_path)
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.eq_ignore_ascii_case("xlsx") || e.eq_ignore_ascii_case("xls"))
            .unwrap_or(false)
    }

    fn extract_text(&self, file_path: &str) -> Result<String> {
        use calamine::{open_workbook, Reader, Xlsx, Data};
        
        let mut workbook: Xlsx<_> = open_workbook(file_path)?;
        let mut text_parts = Vec::new();
        
        for sheet_name in workbook.sheet_names().to_vec() {
            if let Ok(range) = workbook.worksheet_range(&sheet_name) {
                for row in range.rows() {
                    let row_text: Vec<String> = row
                        .iter()
                        .map(|cell: &Data| cell.to_string())
                        .collect();
                    text_parts.push(row_text.join(" "));
                }
            }
        }
        
        Ok(text_parts.join("\n"))
    }
}

pub struct ParserRegistry {
    parsers: Vec<Box<dyn DocumentParser>>,
}

impl ParserRegistry {
    pub fn new(config: &crate::config::FileTypeFilters) -> Self {
        let mut parsers: Vec<Box<dyn DocumentParser>> = vec![Box::new(TextParser)];
        
        if config.include_pdf {
            parsers.push(Box::new(PdfParser));
        }
        if config.include_docx {
            parsers.push(Box::new(DocxParser));
        }
        if config.include_xlsx {
            parsers.push(Box::new(XlsxParser));
        }
        
        Self { parsers }
    }

    pub fn extract_text(&self, file_path: &str) -> Result<String> {
        for parser in &self.parsers {
            if parser.can_parse(file_path) {
                return parser.extract_text(file_path);
            }
        }
        
        anyhow::bail!("No parser available for file: {}", file_path)
    }

    pub fn can_parse(&self, file_path: &str) -> bool {
        self.parsers.iter().any(|p| p.can_parse(file_path))
    }
}
