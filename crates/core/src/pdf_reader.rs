use crate::cancel::{ensure_not_canceled, CancelCheck};
use anyhow::Context;
use lopdf::{encryption::DecryptionError, Document, Error as LopdfError};
use pdf_extract::{output_doc_page, OutputError, PlainTextOutput};
use std::io::{BufWriter, Write};
use std::path::Path;
use tempfile::NamedTempFile;

const PDF_WRITE_BUFFER_CAPACITY: usize = 256 * 1024;

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
pub fn pdf_to_temp_text<C: CancelCheck>(path: &Path, cancel: &C) -> anyhow::Result<NamedTempFile> {
    ensure_not_canceled(cancel)?;

    let mut doc =
        Document::load(path).with_context(|| format!("failed to open PDF '{}'", path.display()))?;
    maybe_decrypt(&mut doc)
        .with_context(|| format!("failed to decrypt PDF '{}'", path.display()))?;

    let mut page_numbers: Vec<u32> = doc.get_pages().into_keys().collect();
    page_numbers.sort_unstable();

    let mut tmp =
        NamedTempFile::new().context("failed to create temporary file for PDF text extraction")?;
    let mut writer = BufWriter::with_capacity(PDF_WRITE_BUFFER_CAPACITY, tmp.as_file_mut());

    for page_num in page_numbers {
        ensure_not_canceled(cancel)?;
        {
            let output_target: &mut dyn Write = &mut writer;
            let mut output = PlainTextOutput::new(output_target);
            output_doc_page(&doc, &mut output, page_num).with_context(|| {
                format!(
                    "failed to extract text from PDF '{}' page {}",
                    path.display(),
                    page_num
                )
            })?;
        }
        writer
            .write_all(b"\n")
            .context("failed to write page separator to temporary file")?;
    }

    writer
        .flush()
        .context("failed to flush PDF text temporary file")?;
    drop(writer);

    Ok(tmp)
}

fn maybe_decrypt(doc: &mut Document) -> Result<(), OutputError> {
    if !doc.is_encrypted() {
        return Ok(());
    }

    if let Err(err) = doc.decrypt("") {
        if let LopdfError::Decryption(DecryptionError::IncorrectPassword) = err {
            eprintln!(
                "Encrypted documents must be decrypted with a password using \
                 {{extract_text|extract_text_from_mem|output_doc}}_encrypted"
            );
        }
        return Err(OutputError::PdfError(err));
    }

    Ok(())
}
