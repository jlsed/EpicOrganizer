use std::fs;
use std::io::Read;
use std::path::Path;

use quick_xml::events::Event;

const MAX_TEXT_BYTES: u64 = 64 * 1024;
const MAX_EXTRACTED_CHARS: usize = 256 * 1024;

/// Which reader `extract_text` will use for a path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Text,
    Pdf,
    Docx,
    Unsupported,
}

pub fn classify(path: &Path) -> Kind {
    match extension(path).as_deref() {
        Some("pdf") => Kind::Pdf,
        Some("docx") => Kind::Docx,
        Some(ext) if is_text_extension(ext) => Kind::Text,
        _ => Kind::Unsupported,
    }
}

/// Extract plain text for the model. `Ok(None)` means the type is not
/// supported; `Err` means the type is supported but extraction failed.
pub fn extract_text(path: &Path) -> Result<Option<String>, String> {
    match classify(path) {
        Kind::Unsupported => Ok(None),
        Kind::Text => read_text_file(path).map(Some),
        Kind::Pdf => extract_pdf(path).map(Some),
        Kind::Docx => extract_docx(path).map(Some),
    }
}

fn extension(path: &Path) -> Option<String> {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(str::to_ascii_lowercase)
}

fn is_text_extension(ext: &str) -> bool {
    matches!(
        ext,
        "txt" | "md" | "markdown" | "csv" | "tsv" | "json" | "jsonl" | "ndjson" | "log" | "yaml"
            | "yml" | "toml" | "ini" | "cfg" | "conf" | "xml" | "html" | "htm" | "css" | "js"
            | "mjs" | "cjs" | "ts" | "tsx" | "jsx" | "py" | "rs" | "go" | "java" | "kt" | "cs"
            | "c" | "h" | "cpp" | "hpp" | "sh" | "ps1" | "bat" | "cmd" | "sql"
    )
}

fn read_text_file(path: &Path) -> Result<String, String> {
    let file = fs::File::open(path).map_err(|e| format!("cannot read: {e}"))?;
    let mut bytes = Vec::new();
    file.take(MAX_TEXT_BYTES)
        .read_to_end(&mut bytes)
        .map_err(|e| format!("cannot read: {e}"))?;
    Ok(cap_text(String::from_utf8_lossy(&bytes).replace('\0', "")))
}

fn extract_pdf(path: &Path) -> Result<String, String> {
    let text = pdf_extract::extract_text(path).map_err(|e| format!("PDF extraction failed: {e}"))?;
    Ok(cap_text(text.replace('\0', "")))
}

fn extract_docx(path: &Path) -> Result<String, String> {
    let file = fs::File::open(path).map_err(|e| format!("cannot open: {e}"))?;
    let mut archive =
        zip::ZipArchive::new(file).map_err(|e| format!("not a valid .docx archive: {e}"))?;
    let mut document = archive
        .by_name("word/document.xml")
        .map_err(|_| "the .docx has no word/document.xml".to_string())?;
    let mut xml = String::new();
    document
        .read_to_string(&mut xml)
        .map_err(|e| format!("cannot read word/document.xml: {e}"))?;
    docx_xml_text(&xml).map(cap_text)
}

fn docx_xml_text(xml: &str) -> Result<String, String> {
    let mut reader = quick_xml::Reader::from_str(xml);
    reader.config_mut().trim_text(false);
    let mut out = String::new();
    let mut in_text = false;
    loop {
        match reader.read_event() {
            Ok(Event::Start(event)) => match event.name().as_ref() {
                "w:t" => in_text = true,
                "w:tab" => out.push('\t'),
                "w:br" => out.push('\n'),
                _ => {}
            },
            Ok(Event::End(event)) => match event.name().as_ref() {
                "w:t" => in_text = false,
                "w:p" if !out.ends_with('\n') => out.push('\n'),
                _ => {}
            },
            Ok(Event::Empty(event)) => match event.name().as_ref() {
                "w:tab" => out.push('\t'),
                "w:br" => out.push('\n'),
                _ => {}
            },
            Ok(Event::Text(event)) if in_text => out.push_str(event.as_ref()),
            Ok(Event::CData(event)) if in_text => out.push_str(event.as_ref()),
            Ok(Event::GeneralRef(event)) if in_text => {
                if let Some(resolved) = resolve_entity(event.as_ref()) {
                    out.push_str(&resolved);
                }
            }
            Ok(Event::Eof) => break,
            Err(error) => return Err(format!("invalid word/document.xml: {error}")),
            _ => {}
        }
    }
    Ok(out)
}

fn resolve_entity(name: &str) -> Option<String> {
    match name {
        "amp" => Some("&".to_string()),
        "lt" => Some("<".to_string()),
        "gt" => Some(">".to_string()),
        "quot" => Some("\"".to_string()),
        "apos" => Some("'".to_string()),
        _ => {
            let code = name
                .strip_prefix("#x")
                .or_else(|| name.strip_prefix("#X"))
                .and_then(|hex| u32::from_str_radix(hex, 16).ok())
                .or_else(|| name.strip_prefix('#').and_then(|dec| dec.parse().ok()));
            code.and_then(char::from_u32).map(|c| c.to_string())
        }
    }
}

fn cap_text(text: String) -> String {
    let mut chars = text.chars();
    let capped: String = chars.by_ref().take(MAX_EXTRACTED_CHARS).collect();
    if chars.next().is_some() {
        format!("{capped}\n…[content truncated]")
    } else {
        capped
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write_docx(path: &Path, document_xml: &str) {
        let file = fs::File::create(path).unwrap();
        let mut writer = zip::ZipWriter::new(file);
        writer
            .start_file("word/document.xml", zip::write::SimpleFileOptions::default())
            .unwrap();
        writer.write_all(document_xml.as_bytes()).unwrap();
        writer.finish().unwrap();
    }

    #[test]
    fn classifies_supported_types() {
        assert_eq!(classify(Path::new("a.txt")), Kind::Text);
        assert_eq!(classify(Path::new("a.PDF")), Kind::Pdf);
        assert_eq!(classify(Path::new("a.docx")), Kind::Docx);
        assert_eq!(classify(Path::new("a.png")), Kind::Unsupported);
        assert_eq!(classify(Path::new("no-extension")), Kind::Unsupported);
    }

    #[test]
    fn extracts_docx_text_with_paragraphs_and_entities() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("notes.docx");
        write_docx(
            &path,
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    <w:p><w:r><w:t>Confidential SSN 123-45-6789</w:t></w:r></w:p>
    <w:p><w:r><w:t xml:space="preserve">A &amp; B</w:t></w:r></w:p>
  </w:body>
</w:document>"#,
        );
        let text = extract_text(&path).unwrap().unwrap();
        assert!(text.contains("Confidential SSN 123-45-6789"), "got: {text}");
        assert!(text.contains("A & B"), "got: {text}");
        assert!(text.contains('\n'));
    }

    #[test]
    fn extracts_text_from_a_pdf() {
        use lopdf::content::{Content, Operation};
        use lopdf::{dictionary, Document, Object, Stream};

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sample.pdf");

        let mut doc = Document::with_version("1.5");
        let pages_id = doc.new_object_id();
        let font_id = doc.add_object(dictionary! {
            "Type" => "Font",
            "Subtype" => "Type1",
            "BaseFont" => "Helvetica",
        });
        let resources_id = doc.add_object(dictionary! {
            "Font" => dictionary! { "F1" => font_id },
        });
        let content = Content {
            operations: vec![
                Operation::new("BT", vec![]),
                Operation::new("Tf", vec![Object::Name(b"F1".to_vec()), Object::Real(24.0)]),
                Operation::new("Td", vec![Object::Integer(72), Object::Integer(720)]),
                Operation::new(
                    "Tj",
                    vec![Object::string_literal("Confidential SSN 123-45-6789")],
                ),
                Operation::new("ET", vec![]),
            ],
        };
        let content_id = doc.add_object(Stream::new(dictionary! {}, content.encode().unwrap()));
        let page_id = doc.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "MediaBox" => vec![
                Object::Integer(0),
                Object::Integer(0),
                Object::Integer(612),
                Object::Integer(792),
            ],
            "Resources" => resources_id,
            "Contents" => content_id,
        });
        let pages = dictionary! {
            "Type" => "Pages",
            "Kids" => vec![page_id.into()],
            "Count" => Object::Integer(1),
        };
        doc.objects.insert(pages_id, Object::Dictionary(pages));
        let catalog_id = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages_id });
        doc.trailer.set("Root", catalog_id);
        doc.save(&path).unwrap();

        let text = extract_text(&path).unwrap().unwrap();
        assert!(text.contains("Confidential"), "got: {text}");
        assert!(text.contains("123-45-6789"), "got: {text}");
    }

    #[test]
    fn unsupported_returns_none_and_broken_docx_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let image = dir.path().join("image.png");
        fs::write(&image, [0u8, 159, 146, 150]).unwrap();
        assert_eq!(extract_text(&image).unwrap(), None);

        let broken = dir.path().join("broken.docx");
        fs::write(&broken, b"not a zip").unwrap();
        assert!(extract_text(&broken).is_err());
    }

    #[test]
    fn text_extraction_is_capped() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("long.txt");
        fs::write(&path, "a".repeat(MAX_TEXT_BYTES as usize + 100)).unwrap();
        let text = extract_text(&path).unwrap().unwrap();
        assert_eq!(text.chars().count(), MAX_TEXT_BYTES as usize);

        let capped = cap_text("a".repeat(MAX_EXTRACTED_CHARS + 100));
        assert!(capped.contains("[content truncated]"));
        assert!(capped.chars().count() < MAX_EXTRACTED_CHARS + 100);
    }
}
