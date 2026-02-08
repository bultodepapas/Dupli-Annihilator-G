use std::io::BufRead;

pub struct LossyLineReader<R: BufRead> {
    inner: R,
    raw: Vec<u8>,
    first_line: bool,
}

impl<R: BufRead> LossyLineReader<R> {
    pub fn new(inner: R) -> Self {
        Self {
            inner,
            raw: Vec::with_capacity(8 * 1024),
            first_line: true,
        }
    }

    pub fn read_line(&mut self, out: &mut String) -> std::io::Result<usize> {
        self.raw.clear();
        let n = self.inner.read_until(b'\n', &mut self.raw)?;
        out.clear();
        if n == 0 {
            return Ok(0);
        }

        let decoded = String::from_utf8_lossy(&self.raw);
        let text = if self.first_line {
            self.first_line = false;
            decoded.strip_prefix('\u{feff}').unwrap_or(decoded.as_ref())
        } else {
            decoded.as_ref()
        };
        out.push_str(text);
        Ok(n)
    }
}
