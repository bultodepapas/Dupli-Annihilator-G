use crate::{
    config::{Config, OutputOrdering},
    dedupe_ram::RamStore,
    progress::{ProgressEvent, ProgressSink},
    stats::Stats,
    token_iter::TokenIter,
    writer::OutputWriter,
};
use ahash::AHasher;
use std::fs::File;
use std::hash::Hasher;
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::PathBuf;

pub struct DiskBuckets {
    _dir: tempfile::TempDir,
    bucket_paths: Vec<PathBuf>,
    bucket_writers: Vec<BufWriter<File>>,
}

impl DiskBuckets {
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
        })
    }

    #[inline]
    fn bucket_index(token: &str, n: usize) -> usize {
        let mut hasher = AHasher::default();
        hasher.write(token.as_bytes());
        (hasher.finish() as usize) % n
    }

    pub fn partition_inputs<P: ProgressSink>(
        &mut self,
        config: &Config,
        progress: &P,
        stats: &mut Stats,
    ) -> anyhow::Result<()> {
        progress.on_event(ProgressEvent::Stage("PartitioningBuckets"));

        for (idx, path) in config.inputs.iter().enumerate() {
            progress.on_event(ProgressEvent::FileStarted {
                index: idx + 1,
                total: config.inputs.len(),
            });

            let file = File::open(path)?;
            let mut reader = BufReader::new(file);
            let mut line = String::new();

            loop {
                line.clear();
                let n = reader.read_line(&mut line)?;
                if n == 0 {
                    break;
                }

                for raw in TokenIter::new(&line) {
                    stats.tokens_seen += 1;
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

                    let bi = Self::bucket_index(token, self.bucket_writers.len());
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

        Ok(())
    }

    pub fn reduce_to_output<P: ProgressSink>(
        &self,
        config: &Config,
        progress: &P,
        stats: &mut Stats,
    ) -> anyhow::Result<()> {
        progress.on_event(ProgressEvent::Stage("ReducingBuckets"));
        let mut out = OutputWriter::create(&config.output, config.output_separator.clone())?;

        for (i, path) in self.bucket_paths.iter().enumerate() {
            progress.on_event(ProgressEvent::FileStarted {
                index: i + 1,
                total: self.bucket_paths.len(),
            });

            let file = File::open(path)?;
            let reader = BufReader::new(file);

            let mut store = match config.ordering {
                OutputOrdering::UnorderedFast => RamStore::new_unordered(),
                OutputOrdering::PreserveFirstSeen | OutputOrdering::Alphabetical => {
                    RamStore::new_stable()
                }
            };

            for line in reader.lines() {
                let mut token = line?;
                if config.trim {
                    token = token.trim().to_string();
                }
                if config.drop_empty && token.is_empty() {
                    continue;
                }

                if store.insert(&token) {
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
                out.write_token(&token)?;
            }

            progress.on_event(ProgressEvent::FileFinished {
                index: i + 1,
                total: self.bucket_paths.len(),
            });
        }

        progress.on_event(ProgressEvent::Stage("WritingOutput"));
        out.finish()?;
        Ok(())
    }
}
