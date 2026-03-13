use crate::cancel::{ensure_not_canceled, CancelCheck};
use anyhow::Context;
use std::io::{BufWriter, Write};
use std::path::Path;
use tempfile::NamedTempFile;

const EPUB_WRITE_BUFFER_CAPACITY: usize = 256 * 1024;

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
pub fn epub_to_temp_text<C: CancelCheck>(path: &Path, cancel: &C) -> anyhow::Result<NamedTempFile> {
    let mut doc = epub::doc::EpubDoc::new(path)
        .with_context(|| format!("failed to open EPUB '{}'", path.display()))?;

    let mut tmp =
        NamedTempFile::new().context("failed to create temporary file for EPUB text extraction")?;
    let mut writer = BufWriter::with_capacity(EPUB_WRITE_BUFFER_CAPACITY, tmp.as_file_mut());

    // Clone spine idrefs so we can call mutable methods on `doc` inside the loop.
    // epub crate v2 exposes spine as Vec<SpineItem>; each item has an `idref` field.
    let spine: Vec<String> = doc.spine.iter().map(|s| s.idref.clone()).collect();

    for id in &spine {
        ensure_not_canceled(cancel)?;
        if let Some((content, _mime)) = doc.get_resource(id) {
            let html = String::from_utf8_lossy(&content);
            write_html_as_text(&html, &mut writer)
                .context("failed to write EPUB chapter text to temporary file")?;
            // Ensure a newline separates each chapter so tokens don't merge
            // across chapter boundaries.
            writer
                .write_all(b"\n")
                .context("failed to write chapter separator to temporary file")?;
        }
    }

    writer
        .flush()
        .context("failed to flush EPUB text temporary file")?;
    drop(writer);

    Ok(tmp)
}

/// Strips HTML/XML tags from a string and writes only the text content.
///
/// Each closing `>` is replaced with a space so that words from adjacent tags
/// (e.g. `<b>foo</b><i>bar</i>`) are not merged into a single token.
///
/// After tag removal, the most common named HTML/XML entities are decoded so
/// that text like `&amp;`, `&nbsp;`, `&lt;`, etc. do not produce junk tokens
/// (`amp`, `nbsp`, `lt`) in the deduplication output.
fn write_html_as_text<W: Write>(html: &str, out: &mut W) -> std::io::Result<()> {
    let mut in_tag = false;
    let mut chars = html.chars().peekable();

    while let Some(c) = chars.next() {
        match c {
            '<' => in_tag = true,
            '>' => {
                in_tag = false;
                out.write_all(b" ")?;
            }
            '&' if !in_tag => write_entity_or_literal(&mut chars, out)?,
            _ if !in_tag => write_char(out, c)?,
            _ => {}
        }
    }
    Ok(())
}

fn write_entity_or_literal<W: Write>(
    chars: &mut std::iter::Peekable<std::str::Chars<'_>>,
    out: &mut W,
) -> std::io::Result<()> {
    let mut entity = String::from("&");
    let mut complete = false;

    while let Some(&next) = chars.peek() {
        if next == ';' {
            entity.push(next);
            chars.next();
            complete = true;
            break;
        }
        if !(next.is_ascii_alphanumeric() || next == '#' || next == 'x' || next == 'X') {
            break;
        }
        entity.push(next);
        chars.next();
        if entity.len() > 12 {
            break;
        }
    }

    let replacement = if complete {
        match entity.as_str() {
            "&nbsp;" => Some(" "),
            "&lt;" => Some("<"),
            "&gt;" => Some(">"),
            "&quot;" => Some("\""),
            "&apos;" => Some("'"),
            "&amp;" => Some("&"),
            _ => None,
        }
    } else {
        None
    };

    match replacement {
        Some(decoded) => out.write_all(decoded.as_bytes()),
        None => out.write_all(entity.as_bytes()),
    }
}

fn write_char<W: Write>(out: &mut W, c: char) -> std::io::Result<()> {
    let mut buf = [0u8; 4];
    out.write_all(c.encode_utf8(&mut buf).as_bytes())
}

#[cfg(test)]
mod tests {
    use super::write_html_as_text;

    fn render(html: &str) -> String {
        let mut out = Vec::new();
        write_html_as_text(html, &mut out).expect("render html");
        String::from_utf8(out).expect("utf8")
    }

    #[test]
    fn strips_tags_without_merging_words() {
        assert_eq!(render("<b>foo</b><i>bar</i>"), " foo  bar ");
    }

    #[test]
    fn decodes_common_entities_inline() {
        assert_eq!(render("Tom &amp; Jerry &lt;3&nbsp;"), "Tom & Jerry <3 ");
    }

    #[test]
    fn preserves_unknown_entities_as_literal_text() {
        assert_eq!(render("A &copy; B"), "A &copy; B");
    }
}
