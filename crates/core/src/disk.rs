use crate::{
    cancel::{ensure_not_canceled, CancelCheck},
    config::{Config, OutputOrdering},
    dedupe_ram::RamStore,
    progress::{ProgressEvent, ProgressSink},
    stats::Stats,
    text_line_reader::LossyLineReader,
    token_iter::TokenIter,
    writer::OutputWriter,
};
use std::fs::File;
use std::io::{BufReader, BufWriter, Write};
use std::path::PathBuf;

/// Typestate: bucket files are open for writing.
/// Consumed by [`WritableBuckets::partition_inputs`], which flushes and drops
/// all writers before returning a [`ReducibleBuckets`].  This ensures no write
/// handle is alive when the reduce phase opens the same files for reading —
/// important on Windows where a file open for writing blocks readers.
pub struct WritableBuckets {
    _dir: tempfile::TempDir,
    bucket_paths: Vec<PathBuf>,
    bucket_writers: Vec<BufWriter<File>>,
    hasher_state: ahash::RandomState,
}

/// Typestate: bucket files have been written and closed; ready for reading.
pub struct ReducibleBuckets {
    _dir: tempfile::TempDir,
    bucket_paths: Vec<PathBuf>,
}

impl WritableBuckets {
    pub fn new(n: usize) -> anyhow::Result<Self> {
        let dir = tempfile::tempdir()?;
        let mut bucket_paths = Vec::with_capacity(n);
        let mut bucket_writers = Vec::with_capacity(n);

        for i in 0..n {
            let p = dir.path().join(format!("bucket_{i:04}.txt"));
            let f = File::create(&p)?;
            bucket_paths.push(p);
            bucket_writers.push(BufWriter::new(f));
        }

        Ok(Self {
            _dir: dir,
            bucket_paths,
            bucket_writers,
            hasher_state: ahash::RandomState::new(),
        })
    }

    #[inline]
    fn bucket_index(token: &str, n: usize, state: &ahash::RandomState) -> usize {
        (state.hash_one(token) as usize) % n
    }

    /// Partitions all input tokens into hash buckets and returns a
    /// [`ReducibleBuckets`] with all write handles closed.
    pub fn partition_inputs<P: ProgressSink, C: CancelCheck>(
        mut self,
        config: &Config,
        progress: &P,
        stats: &mut Stats,
        cancel: &C,
    ) -> anyhow::Result<ReducibleBuckets> {
        progress.on_event(ProgressEvent::Stage("PartitioningBuckets"));

        for (idx, path) in config.inputs.iter().enumerate() {
            ensure_not_canceled(cancel)?;
            progress.on_event(ProgressEvent::FileStarted {
                index: idx + 1,
                total: config.inputs.len(),
            });

            let file = File::open(path)?;
            let mut reader = LossyLineReader::new(BufReader::new(file));
            let mut line = String::new();

            loop {
                ensure_not_canceled(cancel)?;
                let n = reader.read_line(&mut line)?;
                if n == 0 {
                    break;
                }

                for raw in TokenIter::new(&line) {
                    stats.tokens_seen += 1;
                    if stats.tokens_seen % 8_192 == 0 {
                        ensure_not_canceled(cancel)?;
                    }
                    if stats.tokens_seen % 100_000 == 0 {
                        progress.on_event(ProgressEvent::TokensSeen(stats.tokens_seen));
                    }

                    let mut token = raw;
                    if config.trim {
                        token = token.trim();
                    }
                    if config.drop_empty && token.is_empty() {
                        continue;
                    }
                    if config.should_drop_by_length(token) {
                        stats.filtered_by_length += 1;
                        if stats.filtered_by_length % 100_000 == 0 {
                            progress.on_event(ProgressEvent::FilteredByLength(
                                stats.filtered_by_length,
                            ));
                        }
                        continue;
                    }

                    let bi =
                        Self::bucket_index(token, self.bucket_writers.len(), &self.hasher_state);
                    let writer = &mut self.bucket_writers[bi];
                    writer.write_all(token.as_bytes())?;
                    writer.write_all(b"\n")?;
                }
            }

            progress.on_event(ProgressEvent::FileFinished {
                index: idx + 1,
                total: config.inputs.len(),
            });
        }

        for writer in &mut self.bucket_writers {
            writer.flush()?;
        }

        // Drop all write handles before returning — `self.bucket_writers` is
        // not moved into `ReducibleBuckets`, so it is dropped here.
        Ok(ReducibleBuckets {
            _dir: self._dir,
            bucket_paths: self.bucket_paths,
        })
    }
}

impl ReducibleBuckets {
    pub fn reduce_to_output<P: ProgressSink, C: CancelCheck>(
        &self,
        config: &Config,
        progress: &P,
        stats: &mut Stats,
        cancel: &C,
    ) -> anyhow::Result<()> {
        progress.on_event(ProgressEvent::Stage("ReducingBuckets"));
        let mut out = OutputWriter::create(&config.output, config.output_separator.clone())?;

        for (i, path) in self.bucket_paths.iter().enumerate() {
            ensure_not_canceled(cancel)?;
            progress.on_event(ProgressEvent::FileStarted {
                index: i + 1,
                total: self.bucket_paths.len(),
            });

            let file = File::open(path)?;
            let mut reader = LossyLineReader::new(BufReader::new(file));
            let mut line = String::new();

            let mut store = match config.ordering {
                OutputOrdering::UnorderedFast => RamStore::new_unordered(),
                OutputOrdering::PreserveFirstSeen | OutputOrdering::Alphabetical => {
                    RamStore::new_stable()
                }
            };

            let mut bucket_tokens: u64 = 0;
            loop {
                let n = reader.read_line(&mut line)?;
                if n == 0 {
                    break;
                }
                bucket_tokens += 1;
                if bucket_tokens % 8_192 == 0 {
                    ensure_not_canceled(cancel)?;
                }
                // Bucket files are pre-filtered by partition_inputs: tokens are
                // already trimmed, non-empty, and length-filtered. No re-checking needed.
                // Trim the trailing newline preserved by LossyLineReader::read_line.
                let token = line.trim_end_matches(|c: char| c == '\n' || c == '\r');

                if store.insert(token) {
                    stats.unique_tokens += 1;
                    if stats.unique_tokens % 100_000 == 0 {
                        progress.on_event(ProgressEvent::UniqueTokens(stats.unique_tokens));
                    }
                } else {
                    stats.duplicates += 1;
                    if stats.duplicates % 100_000 == 0 {
                        progress.on_event(ProgressEvent::Duplicates(stats.duplicates));
                    }
                }
            }

            let mut tokens = store.into_tokens();
            if matches!(config.ordering, OutputOrdering::Alphabetical) {
                tokens.sort_unstable();
            }

            for token in tokens {
                ensure_not_canceled(cancel)?;
                out.write_token(&token)?;
            }

            progress.on_event(ProgressEvent::FileFinished {
                index: i + 1,
                total: self.bucket_paths.len(),
            });
        }

        out.finish()?;
        Ok(())
    }
}
