use crate::config::Mode;
use std::path::PathBuf;
use std::time::Duration;

/// Per-file breakdown collected when `Config::per_file_stats` is enabled.
/// Only available for RAM mode; Disk mode returns `None`.
#[derive(Debug, Default, Clone)]
pub struct FileStats {
    /// Original input path (as given in `Config::inputs`).
    pub path: PathBuf,
    /// File size in bytes, or `None` if the metadata read failed.
    pub file_bytes: Option<u64>,
    /// Total tokens seen in this file (after separator splitting, before filters).
    pub tokens_seen: u64,
    /// Tokens that were already in the global set when this file was processed
    /// (duplicates from a previous file, or repeated within this file).
    pub duplicates: u64,
    /// New unique tokens contributed by this file to the global set.
    pub unique_new: u64,
    /// Tokens dropped by the length filter in this file.
    pub filtered_by_length: u64,
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct AutoDecisionTelemetry {
    pub available_memory_bytes: u64,
    pub total_memory_bytes: u64,
    pub usable_memory_bytes: u64,
    pub safety_margin_bytes: u64,
    pub estimated_ram_bytes: u64,
    pub sample_tokens: u64,
    pub sample_unique_ratio: f64,
    pub sample_duplicate_ratio: f64,
    pub decision_reason: String,
}

#[derive(Debug, Default, Clone)]
pub struct Stats {
    pub files: usize,
    pub tokens_seen: u64,
    pub unique_tokens: u64,
    pub duplicates: u64,
    pub filtered_by_length: u64,
    pub elapsed: Duration,
    pub mode_effective: Option<Mode>,
    /// PDFs that failed to extract, collected as `(path, error_message)`.
    /// The job continues without them; they are surfaced as warnings in the summary.
    pub failed_pdfs: Vec<(PathBuf, String)>,
    /// Per-file breakdown. `None` when `Config::per_file_stats` is `false`
    /// or when running in Disk mode (not supported).
    pub per_file: Option<Vec<FileStats>>,
    /// Additional telemetry emitted when `Mode::Auto` performs a host-aware
    /// decision based on memory and workload sampling.
    pub auto_telemetry: Option<AutoDecisionTelemetry>,
}
