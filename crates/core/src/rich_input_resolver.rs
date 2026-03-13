use crate::{
    cancel::{ensure_not_canceled, is_canceled_error, CancelCheck},
    config::Config,
    epub_reader, pdf_reader,
    progress::{ProgressEvent, ProgressSink},
};
use anyhow::anyhow;
use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::thread;
use tempfile::NamedTempFile;

const MAX_RICH_INPUT_WORKERS: usize = 8;

#[derive(Debug)]
pub(crate) struct RichInputResolution {
    pub(crate) resolved_config: Config,
    pub(crate) temp_files: Vec<NamedTempFile>,
    pub(crate) failures: Vec<(PathBuf, String)>,
    pub(crate) path_aliases: HashMap<PathBuf, PathBuf>,
}

#[derive(Debug, Clone, Copy)]
enum RichInputKind {
    Pdf,
    Epub,
}

#[derive(Debug, Clone)]
struct RichInputTask {
    input_idx: usize,
    path: PathBuf,
    kind: RichInputKind,
}

#[derive(Debug)]
struct RichInputOutcome {
    input_idx: usize,
    original_path: PathBuf,
    temp_file: Option<NamedTempFile>,
    failure: Option<String>,
}

#[derive(Debug, Default)]
struct ProgressState {
    completed: usize,
}

struct RichProgressReporter<'a, P> {
    total: usize,
    progress: &'a P,
    state: Mutex<ProgressState>,
}

impl<'a, P: ProgressSink> RichProgressReporter<'a, P> {
    fn new(total: usize, progress: &'a P) -> Self {
        Self {
            total,
            progress,
            state: Mutex::new(ProgressState::default()),
        }
    }

    fn started(&self, path: &Path) -> anyhow::Result<()> {
        let _state = self
            .state
            .lock()
            .map_err(|_| anyhow!("rich input progress lock poisoned"))?;
        self.progress.on_event(ProgressEvent::StageItemStarted {
            total: self.total,
            path: path.to_path_buf(),
        });
        Ok(())
    }

    fn finished(&self, path: &Path) -> anyhow::Result<()> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| anyhow!("rich input progress lock poisoned"))?;
        state.completed += 1;
        self.progress.on_event(ProgressEvent::StageItemFinished {
            completed: state.completed,
            total: self.total,
            path: path.to_path_buf(),
        });
        Ok(())
    }
}

pub(crate) fn resolve_rich_inputs<P: ProgressSink, C: CancelCheck>(
    config: &Config,
    progress: &P,
    cancel: &C,
) -> anyhow::Result<RichInputResolution> {
    let mut resolved_inputs: Vec<Option<PathBuf>> = vec![None; config.inputs.len()];
    let mut rich_tasks = Vec::new();

    for (input_idx, path) in config.inputs.iter().enumerate() {
        if pdf_reader::is_pdf(path) {
            rich_tasks.push(RichInputTask {
                input_idx,
                path: path.clone(),
                kind: RichInputKind::Pdf,
            });
        } else if epub_reader::is_epub(path) {
            rich_tasks.push(RichInputTask {
                input_idx,
                path: path.clone(),
                kind: RichInputKind::Epub,
            });
        } else {
            resolved_inputs[input_idx] = Some(path.clone());
        }
    }

    let total_rich = rich_tasks.len();
    if total_rich == 0 {
        return Ok(RichInputResolution {
            resolved_config: config.clone(),
            temp_files: Vec::new(),
            failures: Vec::new(),
            path_aliases: HashMap::new(),
        });
    }

    progress.on_event(ProgressEvent::Stage("ExtractingText"));

    let outcomes = extract_rich_inputs(&rich_tasks, total_rich, progress, cancel)?;
    let mut temp_files = Vec::with_capacity(total_rich);
    let mut failures = Vec::new();
    let mut path_aliases = HashMap::new();

    for outcome in outcomes {
        if let Some(tmp) = outcome.temp_file {
            let temp_path = tmp.path().to_path_buf();
            path_aliases.insert(temp_path.clone(), outcome.original_path);
            resolved_inputs[outcome.input_idx] = Some(temp_path);
            temp_files.push(tmp);
        } else if let Some(err) = outcome.failure {
            failures.push((outcome.original_path, err));
        }
    }

    let mut resolved_config = config.clone();
    resolved_config.inputs = resolved_inputs.into_iter().flatten().collect();

    Ok(RichInputResolution {
        resolved_config,
        temp_files,
        failures,
        path_aliases,
    })
}

fn extract_rich_inputs<P: ProgressSink, C: CancelCheck>(
    tasks: &[RichInputTask],
    total_rich: usize,
    progress: &P,
    cancel: &C,
) -> anyhow::Result<Vec<RichInputOutcome>> {
    let workers = rich_input_worker_count(total_rich);
    let reporter = RichProgressReporter::new(total_rich, progress);
    if workers <= 1 {
        let mut outcomes = Vec::with_capacity(tasks.len());
        for task in tasks {
            outcomes.push(process_rich_input_task(task, &reporter, cancel)?);
        }
        outcomes.sort_by_key(|outcome| outcome.input_idx);
        return Ok(outcomes);
    }

    let queue = Mutex::new(VecDeque::from(tasks.to_vec()));
    let results = Mutex::new(Vec::with_capacity(tasks.len()));

    thread::scope(|scope| -> anyhow::Result<()> {
        let mut handles = Vec::with_capacity(workers);
        for _ in 0..workers {
            handles.push(scope.spawn(|| -> anyhow::Result<()> {
                loop {
                    ensure_not_canceled(cancel)?;
                    let task = {
                        let mut queue = queue
                            .lock()
                            .map_err(|_| anyhow!("rich input work queue lock poisoned"))?;
                        queue.pop_front()
                    };
                    let Some(task) = task else {
                        return Ok(());
                    };

                    let outcome = process_rich_input_task(&task, &reporter, cancel)?;
                    let mut results = results
                        .lock()
                        .map_err(|_| anyhow!("rich input result lock poisoned"))?;
                    results.push(outcome);
                }
            }));
        }

        for handle in handles {
            handle
                .join()
                .map_err(|_| anyhow!("rich input worker panicked"))??;
        }
        Ok(())
    })?;

    let mut outcomes = results
        .into_inner()
        .map_err(|_| anyhow!("rich input result lock poisoned"))?;
    outcomes.sort_by_key(|outcome| outcome.input_idx);
    Ok(outcomes)
}

fn process_rich_input_task<P: ProgressSink, C: CancelCheck>(
    task: &RichInputTask,
    reporter: &RichProgressReporter<'_, P>,
    cancel: &C,
) -> anyhow::Result<RichInputOutcome> {
    ensure_not_canceled(cancel)?;
    reporter.started(&task.path)?;

    let extraction = match task.kind {
        RichInputKind::Pdf => pdf_reader::pdf_to_temp_text(&task.path, cancel),
        RichInputKind::Epub => epub_reader::epub_to_temp_text(&task.path, cancel),
    };

    let outcome = match extraction {
        Ok(temp_file) => RichInputOutcome {
            input_idx: task.input_idx,
            original_path: task.path.clone(),
            temp_file: Some(temp_file),
            failure: None,
        },
        Err(err) => {
            if is_canceled_error(&err) {
                return Err(err);
            }
            RichInputOutcome {
                input_idx: task.input_idx,
                original_path: task.path.clone(),
                temp_file: None,
                failure: Some(format!("{err:#}")),
            }
        }
    };

    reporter.finished(&task.path)?;
    Ok(outcome)
}

fn rich_input_worker_count(total_rich: usize) -> usize {
    if total_rich <= 1 {
        return total_rich;
    }

    let available = thread::available_parallelism()
        .map(|count| count.get())
        .unwrap_or(1);
    let bounded = available.saturating_sub(1).clamp(1, MAX_RICH_INPUT_WORKERS);
    total_rich.min(bounded)
}
