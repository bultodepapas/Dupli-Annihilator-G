use std::path::PathBuf;

/// Typed error returned by [`Config::validate`].
///
/// Using a concrete enum (rather than anyhow string messages) lets callers
/// downcast and categorize errors without fragile substring matching.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("no input files provided")]
    NoInputs,
    #[error("output path is required")]
    NoOutput,
    #[error("output separator cannot be empty")]
    EmptySeparator,
    #[error("drop_length_min must be between 1 and 10")]
    DropLengthMinOutOfRange,
    #[error("drop_length_max must be between 1 and 10")]
    DropLengthMaxOutOfRange,
    #[error("drop_length_min ({0}) must be <= drop_length_max ({1})")]
    DropLengthRangeInverted(usize, usize),
    #[error("disk_buckets too small")]
    DiskBucketsTooSmall,
    #[error("disk_run_bytes too small")]
    DiskRunBytesTooSmall,
}

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
    /// The lower bound of the length-filter range (inclusive).
    /// When both `drop_length_min` and `drop_length_max` are set, tokens whose
    /// character length satisfies `min <= len <= max` are dropped.
    /// Valid range: 1..=10.
    pub drop_length_min: Option<usize>,
    /// The upper bound of the length-filter range (inclusive).
    /// See `drop_length_min` for the complete behaviour description.
    /// Valid range: 1..=10.
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
            mode: Mode::Auto,
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
    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.inputs.is_empty() {
            return Err(ConfigError::NoInputs);
        }
        if self.output.as_os_str().is_empty() {
            return Err(ConfigError::NoOutput);
        }
        if self.output_separator.is_empty() {
            return Err(ConfigError::EmptySeparator);
        }

        if let (Some(min), Some(max)) = (self.drop_length_min, self.drop_length_max) {
            if !(1..=10).contains(&min) {
                return Err(ConfigError::DropLengthMinOutOfRange);
            }
            if !(1..=10).contains(&max) {
                return Err(ConfigError::DropLengthMaxOutOfRange);
            }
            if min > max {
                return Err(ConfigError::DropLengthRangeInverted(min, max));
            }
        }

        if matches!(self.mode, Mode::Disk) {
            if self.disk_buckets < 8 {
                return Err(ConfigError::DiskBucketsTooSmall);
            }
            if self.disk_run_bytes < 1_000_000 {
                return Err(ConfigError::DiskRunBytesTooSmall);
            }
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
