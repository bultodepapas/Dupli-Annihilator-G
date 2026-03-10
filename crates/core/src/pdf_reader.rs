use anyhow::Context;
use std::io::Write;
use std::path::Path;
use tempfile::NamedTempFile;

/// Returns `true` if the path has a `.pdf` extension (case-insensitive).
pub fn is_pdf(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| e.eq_ignore_ascii_case("pdf"))
        .unwrap_or(false)
}

/// Extracts all text from a PDF file and writes it into a UTF-8 temporary text file.
///
/// The caller **must keep the returned `NamedTempFile` alive** for as long as its
/// path is in use — dropping it deletes the underlying file automatically.
///
/// # Errors
/// Returns an error if the PDF cannot be opened, parsed, or if the temporary file
/// cannot be created or written.
pub fn pdf_to_temp_text(path: &Path) -> anyhow::Result<NamedTempFile> {
    let text = pdf_extract::extract_text(path)
        .with_context(|| format!("failed to extract text from PDF '{}'", path.display()))?;

    let mut tmp =
        NamedTempFile::new().context("failed to create temporary file for PDF text extraction")?;

    tmp.write_all(text.as_bytes())
        .context("failed to write extracted PDF text to temporary file")?;

    tmp.flush()
        .context("failed to flush PDF text temporary file")?;

    Ok(tmp)
}
