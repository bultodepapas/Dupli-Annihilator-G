use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Auto,
    Ram,
    Disk,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputOrdering {
    PreserveFirstSeen,
    Alphabetical,
    UnorderedFast,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiskAlphabeticalMode {
    FastBucketLocal,
    GlobalPerfect,
}

#[derive(Debug, Clone)]
pub struct Config {
    pub inputs: Vec<PathBuf>,
    pub output: PathBuf,
    pub output_separator: String,
    pub mode: Mode,
    pub ordering: OutputOrdering,
    pub trim: bool,
    pub drop_empty: bool,
    pub disk_buckets: usize,
    pub disk_alphabetical_mode: DiskAlphabeticalMode,
    pub disk_run_bytes: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            inputs: Vec::new(),
            output: PathBuf::new(),
            output_separator: "\n".to_string(),
            mode: Mode::Ram,
            ordering: OutputOrdering::PreserveFirstSeen,
            trim: true,
            drop_empty: true,
            disk_buckets: 256,
            disk_alphabetical_mode: DiskAlphabeticalMode::FastBucketLocal,
            disk_run_bytes: 256 * 1024 * 1024,
        }
    }
}

impl Config {
    pub fn validate(&self) -> anyhow::Result<()> {
        anyhow::ensure!(!self.inputs.is_empty(), "no input files provided");
        anyhow::ensure!(!self.output.as_os_str().is_empty(), "output path is required");
        anyhow::ensure!(
            !self.output_separator.is_empty(),
            "output separator cannot be empty"
        );

        if matches!(self.mode, Mode::Disk) {
            anyhow::ensure!(self.disk_buckets >= 8, "disk_buckets too small");
            anyhow::ensure!(
                self.disk_run_bytes >= 1_000_000,
                "disk_run_bytes too small"
            );
        }

        Ok(())
    }
}
