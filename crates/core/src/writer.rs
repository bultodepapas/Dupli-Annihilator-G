use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;

pub struct OutputWriter {
    writer: BufWriter<File>,
    sep: Vec<u8>,
    is_first: bool,
}

impl OutputWriter {
    pub fn create(path: &Path, sep: String) -> std::io::Result<Self> {
        Self::with_capacity(path, sep, 64 * 1024)
    }

    pub fn with_capacity(path: &Path, sep: String, capacity: usize) -> std::io::Result<Self> {
        let file = File::create(path)?;
        Ok(Self {
            writer: BufWriter::with_capacity(capacity, file),
            sep: sep.into_bytes(),
            is_first: true,
        })
    }

    pub fn write_token(&mut self, token: &str) -> std::io::Result<()> {
        if self.is_first {
            self.is_first = false;
        } else {
            self.writer.write_all(&self.sep)?;
        }
        self.writer.write_all(token.as_bytes())
    }

    pub fn finish(mut self) -> std::io::Result<()> {
        self.writer.flush()
    }
}
