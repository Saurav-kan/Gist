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
        
        // Removed config extensions (json, yaml, yml, toml, ini) - now handled by metadata-only indexing
        matches!(ext.as_str(), "txt" | "md" | "js" | "ts" | "py" | "rs" | "java" | "cpp" | "c" | "h" | "hpp" | "xml" | "html" | "css" | "log")
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
        let path = file_path.to_string();
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            pdf_extract::extract_text(&path)
        })) {
            Ok(Ok(text)) => Ok(text),
            Ok(Err(e)) => Err(e.into()),
            Err(_) => anyhow::bail!("PDF parsing failed (unsupported encoding or malformed file)"),
        }
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

pub struct ImageParser;

impl DocumentParser for ImageParser {
    fn can_parse(&self, file_path: &str) -> bool {
        let ext = Path::new(file_path)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();
        
        matches!(ext.as_str(), "jpg" | "jpeg" | "png" | "gif" | "bmp" | "webp" | "svg" | "ico" | "tiff" | "tif")
    }

    fn extract_text(&self, file_path: &str) -> Result<String> {
        // For images, we can't extract text content, but we can use the filename
        // This allows images to be indexed and searched by filename/metadata
        let file_name = Path::new(file_path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();
        
        // Return filename as "text" so it can be embedded
        // This allows semantic search on image filenames
        Ok(format!("image file: {}", file_name))
    }
}

pub struct ParserRegistry {
    parsers: Vec<Box<dyn DocumentParser>>,
    excluded_extensions: Vec<String>,
}

impl ParserRegistry {
    pub fn new(config: &crate::config::FileTypeFilters) -> Self {
        let mut parsers: Vec<Box<dyn DocumentParser>> = vec![Box::new(TextParser)];
        
        // Always include image parser (images are indexed by filename)
        parsers.push(Box::new(ImageParser));
        
        if config.include_pdf {
            parsers.push(Box::new(PdfParser));
        }
        if config.include_docx {
            parsers.push(Box::new(DocxParser));
        }
        if config.include_xlsx {
            parsers.push(Box::new(XlsxParser));
        }
        
        Self { 
            parsers,
            excluded_extensions: config.excluded_extensions.iter().map(|s| s.to_lowercase()).collect(),
        }
    }

    fn is_excluded(&self, file_path: &str) -> bool {
        if self.excluded_extensions.is_empty() {
            return false;
        }

        let ext = Path::new(file_path)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();
        
        self.excluded_extensions.iter().any(|e| e == &ext)
    }

    pub fn extract_text(&self, file_path: &str) -> Result<String> {
        if self.is_excluded(file_path) {
            anyhow::bail!("File type is globally excluded: {}", file_path);
        }

        for parser in &self.parsers {
            if parser.can_parse(file_path) {
                return parser.extract_text(file_path);
            }
        }
        
        anyhow::bail!("No parser available for file: {}", file_path)
    }

    pub fn can_parse(&self, file_path: &str) -> bool {
        if self.is_excluded(file_path) {
            return false;
        }
        self.parsers.iter().any(|p| p.can_parse(file_path))
    }
}
