use std::time::Duration;

#[derive(Debug, Default, Clone)]
pub struct Stats {
    pub files: usize,
    pub tokens_seen: u64,
    pub unique_tokens: u64,
    pub duplicates: u64,
    pub filtered_by_length: u64,
    pub elapsed: Duration,
}
