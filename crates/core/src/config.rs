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
    /// Optional: drop tokens whose character length is >= this value.
    /// Valid range: 1..=10.  Both min and max must be set to activate.
    pub drop_length_min: Option<usize>,
    /// Optional: drop tokens whose character length is <= this value.
    /// Valid range: 1..=10.  Both min and max must be set to activate.
    pub drop_length_max: Option<usize>,
    pub disk_buckets: usize,
    pub disk_alphabetical_mode: DiskAlphabeticalMode,
    pub disk_run_bytes: usize,
    /// When `true`, the engine collects per-file token and duplicate counts.
    /// Only available in RAM mode; has no effect (and produces `None`) in Disk mode.
    pub per_file_stats: bool,
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
            drop_length_min: None,
            drop_length_max: None,
            disk_buckets: 256,
            disk_alphabetical_mode: DiskAlphabeticalMode::FastBucketLocal,
            disk_run_bytes: 256 * 1024 * 1024,
            per_file_stats: false,
        }
    }
}

impl Config {
    pub fn validate(&self) -> anyhow::Result<()> {
        anyhow::ensure!(!self.inputs.is_empty(), "no input files provided");
        anyhow::ensure!(
            !self.output.as_os_str().is_empty(),
            "output path is required"
        );
        anyhow::ensure!(
            !self.output_separator.is_empty(),
            "output separator cannot be empty"
        );

        if let (Some(min), Some(max)) = (self.drop_length_min, self.drop_length_max) {
            anyhow::ensure!(
                min >= 1 && min <= 10,
                "drop_length_min must be between 1 and 10"
            );
            anyhow::ensure!(
                max >= 1 && max <= 10,
                "drop_length_max must be between 1 and 10"
            );
            anyhow::ensure!(
                min <= max,
                "drop_length_min ({min}) must be <= drop_length_max ({max})"
            );
        }

        if matches!(self.mode, Mode::Disk) {
            anyhow::ensure!(self.disk_buckets >= 8, "disk_buckets too small");
            anyhow::ensure!(self.disk_run_bytes >= 1_000_000, "disk_run_bytes too small");
        }

        Ok(())
    }

    /// Returns `true` if the token should be dropped based on its character
    /// length.  Only active when both `drop_length_min` and `drop_length_max`
    /// are set.
    #[inline]
    pub fn should_drop_by_length(&self, token: &str) -> bool {
        if let (Some(min), Some(max)) = (self.drop_length_min, self.drop_length_max) {
            let len = token.chars().count();
            len >= min && len <= max
        } else {
            false
        }
    }
}
