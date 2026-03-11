use anyhow::Context;
use std::io::Write;
use std::path::Path;
use tempfile::NamedTempFile;

/// Returns `true` if the path has an `.epub` extension (case-insensitive).
pub fn is_epub(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| e.eq_ignore_ascii_case("epub"))
        .unwrap_or(false)
}

/// Extracts all text from an EPUB file and writes it into a UTF-8 temporary
/// text file.
///
/// Chapters are iterated in spine order.  HTML/XML tags are stripped so only
/// plain text tokens reach the deduplication engine.
///
/// The caller **must keep the returned `NamedTempFile` alive** for as long as
/// its path is in use — dropping it deletes the underlying file automatically.
pub fn epub_to_temp_text(path: &Path) -> anyhow::Result<NamedTempFile> {
    let mut doc = epub::doc::EpubDoc::new(path)
        .with_context(|| format!("failed to open EPUB '{}'", path.display()))?;

    let mut tmp =
        NamedTempFile::new().context("failed to create temporary file for EPUB text extraction")?;

    // Clone the spine so we can call mutable methods on `doc` inside the loop.
    let spine: Vec<String> = doc.spine.clone();

    for id in &spine {
        if let Some((content, _mime)) = doc.get_resource(id) {
            let html = String::from_utf8_lossy(&content);
            let text = strip_tags(&html);
            tmp.write_all(text.as_bytes())
                .context("failed to write EPUB chapter text to temporary file")?;
            // Ensure a newline separates each chapter so tokens don't merge
            // across chapter boundaries.
            tmp.write_all(b"\n")
                .context("failed to write chapter separator to temporary file")?;
        }
    }

    tmp.flush()
        .context("failed to flush EPUB text temporary file")?;

    Ok(tmp)
}

/// Strips HTML/XML tags from a string and returns only the text content.
///
/// Each closing `>` is replaced with a space so that words from adjacent tags
/// (e.g. `<b>foo</b><i>bar</i>`) are not merged into a single token.
fn strip_tags(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut in_tag = false;

    for c in html.chars() {
        match c {
            '<' => in_tag = true,
            '>' => {
                in_tag = false;
                out.push(' ');
            }
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }

    out
}
