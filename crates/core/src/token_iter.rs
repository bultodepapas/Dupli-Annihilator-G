#[inline]
fn is_delim(c: char) -> bool {
    c.is_whitespace() || c == ',' || c == ';'
}

#[inline]
fn is_ascii_delim(b: u8) -> bool {
    b.is_ascii_whitespace() || b == b',' || b == b';'
}

enum TokenIterInner<'a> {
    Ascii {
        s: &'a str,
        bytes: &'a [u8],
        pos: usize,
    },
    Unicode {
        s: &'a str,
        chars: std::str::CharIndices<'a>,
    },
}

pub struct TokenIter<'a> {
    inner: TokenIterInner<'a>,
}

impl<'a> TokenIter<'a> {
    pub fn new(s: &'a str) -> Self {
        let inner = if s.is_ascii() {
            TokenIterInner::Ascii {
                s,
                bytes: s.as_bytes(),
                pos: 0,
            }
        } else {
            TokenIterInner::Unicode {
                s,
                chars: s.char_indices(),
            }
        };
        Self { inner }
    }
}

impl<'a> Iterator for TokenIter<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<Self::Item> {
        match &mut self.inner {
            TokenIterInner::Ascii { s, bytes, pos } => {
                while *pos < bytes.len() && is_ascii_delim(bytes[*pos]) {
                    *pos += 1;
                }
                if *pos >= bytes.len() {
                    return None;
                }

                let start = *pos;
                while *pos < bytes.len() && !is_ascii_delim(bytes[*pos]) {
                    *pos += 1;
                }

                Some(&s[start..*pos])
            }
            TokenIterInner::Unicode { s, chars } => {
                let start = loop {
                    let (i, c) = chars.next()?;
                    if !is_delim(c) {
                        break i;
                    }
                };

                let end = loop {
                    match chars.next() {
                        None => break s.len(),
                        Some((i, c)) if is_delim(c) => break i,
                        Some(_) => {}
                    }
                };

                Some(&s[start..end])
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::TokenIter;

    fn collect(line: &str) -> Vec<&str> {
        TokenIter::new(line).collect()
    }

    #[test]
    fn tokenizes_ascii_delimiters_with_fast_path() {
        assert_eq!(collect("a,b; c\t\rd"), vec!["a", "b", "c", "d"]);
    }

    #[test]
    fn tokenizes_unicode_whitespace_with_fallback() {
        assert_eq!(collect("uno\u{2003}dos, tres"), vec!["uno", "dos", "tres"]);
    }

    #[test]
    fn ignores_bom_when_stripped_upstream() {
        assert_eq!(collect("alpha,beta"), vec!["alpha", "beta"]);
    }
}
