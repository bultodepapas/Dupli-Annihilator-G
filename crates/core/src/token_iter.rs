#[inline]
fn is_delim(c: char) -> bool {
    c.is_whitespace() || c == ',' || c == ';'
}

pub struct TokenIter<'a> {
    s: &'a str,
    chars: std::str::CharIndices<'a>,
}

impl<'a> TokenIter<'a> {
    pub fn new(s: &'a str) -> Self {
        Self {
            s,
            chars: s.char_indices(),
        }
    }
}

impl<'a> Iterator for TokenIter<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<Self::Item> {
        // Skip leading delimiter characters; find the byte offset of the first
        // non-delimiter (= start of the next token).
        let start = loop {
            let (i, c) = self.chars.next()?;
            if !is_delim(c) {
                break i;
            }
        };

        // Consume non-delimiter characters to find where the token ends.
        let end = loop {
            match self.chars.next() {
                None => break self.s.len(),
                Some((i, c)) if is_delim(c) => break i,
                Some(_) => {}
            }
        };

        Some(&self.s[start..end])
    }
}
