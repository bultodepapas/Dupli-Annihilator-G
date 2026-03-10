use ahash::RandomState;
use anyhow::Context;
use hashbrown::HashSet;
use std::io::BufReader;
use std::path::Path;

use crate::text_line_reader::LossyLineReader;
use crate::token_iter::TokenIter;

pub struct WordChecker {
    words: HashSet<Box<str>, RandomState>,
}

impl WordChecker {
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        let file = std::fs::File::open(path)
            .with_context(|| format!("cannot open wordlist: {}", path.display()))?;
        let mut reader = LossyLineReader::new(BufReader::new(file));
        let mut words: HashSet<Box<str>, RandomState> =
            HashSet::with_hasher(RandomState::new());
        let mut line = String::new();
        while reader.read_line(&mut line)? > 0 {
            for token in TokenIter::new(&line) {
                if !words.contains(token) {
                    words.insert(token.into());
                }
            }
        }
        Ok(Self { words })
    }

    #[inline]
    pub fn contains(&self, word: &str) -> bool {
        self.words.contains(word)
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.words.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.words.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn write_temp(content: &str) -> NamedTempFile {
        let mut f = NamedTempFile::new().unwrap();
        f.write_all(content.as_bytes()).unwrap();
        f
    }

    #[test]
    fn test_basic_lookup() {
        let f = write_temp("apple banana cherry\norange");
        let wc = WordChecker::load(f.path()).unwrap();
        assert_eq!(wc.len(), 4);
        assert!(wc.contains("apple"));
        assert!(wc.contains("orange"));
        assert!(!wc.contains("grape"));
    }

    #[test]
    fn test_empty_file() {
        let f = write_temp("");
        let wc = WordChecker::load(f.path()).unwrap();
        assert!(wc.is_empty());
        assert!(!wc.contains("anything"));
    }

    #[test]
    fn test_bom_stripped() {
        let f = write_temp("\u{feff}hello world");
        let wc = WordChecker::load(f.path()).unwrap();
        assert!(wc.contains("hello"));
        assert!(wc.contains("world"));
    }

    #[test]
    fn test_deduplication() {
        let f = write_temp("word word word");
        let wc = WordChecker::load(f.path()).unwrap();
        assert_eq!(wc.len(), 1);
    }
}
