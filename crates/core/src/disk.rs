use crate::{
    cancel::{ensure_not_canceled, CancelCheck},
    config::{Config, OutputOrdering},
    dedupe_ram::RamStore,
    progress::{ProgressEvent, ProgressSink},
    stats::Stats,
    text_line_reader::LossyLineReader,
    writer::OutputWriter,
};
use std::fs::File;
use std::io::{BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicU64, AtomicUsize, Ordering},
    Mutex,
};

const IO_BUFFER_BYTES: usize = 64 * 1024;
const MAX_REDUCE_WORKERS: usize = 8;

pub struct WritableBuckets {
    _dir: tempfile::TempDir,
    bucket_paths: Vec<PathBuf>,
    bucket_writers: Vec<BufWriter<File>>,
    hasher_state: ahash::RandomState,
}

pub struct ReducibleBuckets {
    _dir: tempfile::TempDir,
    bucket_paths: Vec<PathBuf>,
}

#[derive(Debug, Clone)]
struct BucketReduceResult {
    reduced_path: PathBuf,
    unique_tokens: u64,
    duplicates: u64,
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
            bucket_writers.push(BufWriter::with_capacity(IO_BUFFER_BYTES, f));
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
            let mut reader = LossyLineReader::new(BufReader::with_capacity(IO_BUFFER_BYTES, file));
            let mut line = String::new();

            loop {
                ensure_not_canceled(cancel)?;
                let n = reader.read_line(&mut line)?;
                if n == 0 {
                    break;
                }

                for raw in crate::token_iter::TokenIter::new(&line) {
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

        let worker_count = std::thread::available_parallelism()
            .map(|n| n.get().saturating_sub(1))
            .unwrap_or(1)
            .max(1)
            .min(self.bucket_paths.len().max(1))
            .min(MAX_REDUCE_WORKERS);

        let reduced_dir = tempfile::tempdir()?;
        let bucket_paths = &self.bucket_paths;
        let total_buckets = bucket_paths.len();
        let next_index = AtomicUsize::new(0);
        let completed = AtomicUsize::new(0);
        let total_unique = AtomicU64::new(0);
        let total_duplicates = AtomicU64::new(0);
        let results = Mutex::new(vec![None; total_buckets]);

        std::thread::scope(|scope| -> anyhow::Result<()> {
            let mut handles = Vec::with_capacity(worker_count);
            for _ in 0..worker_count {
                handles.push(scope.spawn(|| -> anyhow::Result<()> {
                    loop {
                        let bucket_index = next_index.fetch_add(1, Ordering::Relaxed);
                        if bucket_index >= total_buckets {
                            break;
                        }

                        ensure_not_canceled(cancel)?;
                        let bucket_path = &bucket_paths[bucket_index];
                        progress.on_event(ProgressEvent::StageItemStarted {
                            total: total_buckets,
                            path: bucket_path.clone(),
                        });

                        let reduced = reduce_bucket(
                            bucket_path,
                            reduced_dir.path(),
                            bucket_index,
                            config,
                            cancel,
                        )?;
                        let unique_total = total_unique
                            .fetch_add(reduced.unique_tokens, Ordering::Relaxed)
                            + reduced.unique_tokens;
                        let duplicate_total = total_duplicates
                            .fetch_add(reduced.duplicates, Ordering::Relaxed)
                            + reduced.duplicates;
                        progress.on_event(ProgressEvent::UniqueTokens(unique_total));
                        progress.on_event(ProgressEvent::Duplicates(duplicate_total));

                        {
                            let mut guard = results.lock().expect("bucket reduce results poisoned");
                            guard[bucket_index] = Some(reduced);
                        }

                        let finished = completed.fetch_add(1, Ordering::Relaxed) + 1;
                        progress.on_event(ProgressEvent::StageItemFinished {
                            completed: finished,
                            total: total_buckets,
                            path: bucket_path.clone(),
                        });
                    }
                    Ok(())
                }));
            }
            for handle in handles {
                handle.join().expect("bucket reducer thread panicked")?;
            }
            Ok(())
        })?;

        stats.unique_tokens += total_unique.load(Ordering::Relaxed);
        stats.duplicates += total_duplicates.load(Ordering::Relaxed);

        let results = results
            .into_inner()
            .expect("bucket reduce results poisoned");
        let mut out = OutputWriter::create(&config.output, config.output_separator.clone())?;
        for reduced in results.into_iter().flatten() {
            ensure_not_canceled(cancel)?;
            append_reduced_bucket(&reduced.reduced_path, &mut out)?;
        }
        out.finish()?;
        Ok(())
    }
}

fn reduce_bucket<C: CancelCheck>(
    bucket_path: &Path,
    reduced_dir: &Path,
    bucket_index: usize,
    config: &Config,
    cancel: &C,
) -> anyhow::Result<BucketReduceResult> {
    let file = File::open(bucket_path)?;
    let mut reader = LossyLineReader::new(BufReader::with_capacity(IO_BUFFER_BYTES, file));
    let mut line = String::new();

    let mut store = match config.ordering {
        OutputOrdering::UnorderedFast => RamStore::new_unordered(),
        OutputOrdering::PreserveFirstSeen | OutputOrdering::Alphabetical => RamStore::new_stable(),
    };

    let mut unique_tokens = 0u64;
    let mut duplicates = 0u64;
    let mut bucket_tokens = 0u64;

    loop {
        let n = reader.read_line(&mut line)?;
        if n == 0 {
            break;
        }
        bucket_tokens += 1;
        if bucket_tokens % 8_192 == 0 {
            ensure_not_canceled(cancel)?;
        }
        let token = line.trim_end_matches(|c: char| c == '\n' || c == '\r');

        if store.insert(token) {
            unique_tokens += 1;
        } else {
            duplicates += 1;
        }
    }

    let mut tokens = store.into_tokens();
    if matches!(config.ordering, OutputOrdering::Alphabetical) {
        tokens.sort_unstable();
    }

    let reduced_path = reduced_dir.join(format!("bucket_{bucket_index:04}_reduced.txt"));
    let file = File::create(&reduced_path)?;
    let mut writer = BufWriter::with_capacity(IO_BUFFER_BYTES, file);
    for token in tokens {
        ensure_not_canceled(cancel)?;
        writer.write_all(token.as_bytes())?;
        writer.write_all(b"\n")?;
    }
    writer.flush()?;

    Ok(BucketReduceResult {
        reduced_path,
        unique_tokens,
        duplicates,
    })
}

fn append_reduced_bucket(path: &Path, out: &mut OutputWriter) -> anyhow::Result<()> {
    let file = File::open(path)?;
    let mut reader = LossyLineReader::new(BufReader::with_capacity(IO_BUFFER_BYTES, file));
    let mut line = String::new();

    loop {
        let n = reader.read_line(&mut line)?;
        if n == 0 {
            break;
        }
        let token = line.trim_end_matches(|c: char| c == '\n' || c == '\r');
        out.write_token(token)?;
    }

    Ok(())
}
