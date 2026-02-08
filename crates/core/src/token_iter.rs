#[inline]
fn is_delim(c: char) -> bool {
    c.is_whitespace() || c == ',' || c == ';'
}

pub struct TokenIter<'a> {
    s: &'a str,
    pos: usize,
}

impl<'a> TokenIter<'a> {
    pub fn new(s: &'a str) -> Self {
        Self { s, pos: 0 }
    }
}

impl<'a> Iterator for TokenIter<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<Self::Item> {
        let len = self.s.len();

        while self.pos < len {
            let c = self.s[self.pos..].chars().next()?;
            if is_delim(c) {
                self.pos += c.len_utf8();
            } else {
                break;
            }
        }

        if self.pos >= len {
            return None;
        }

        let start = self.pos;

        while self.pos < len {
            let c = self.s[self.pos..].chars().next()?;
            if is_delim(c) {
                break;
            }
            self.pos += c.len_utf8();
        }

        Some(&self.s[start..self.pos])
    }
}
