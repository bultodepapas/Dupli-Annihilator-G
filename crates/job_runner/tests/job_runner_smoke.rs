use dedupe_core::{Config, DiskAlphabeticalMode, Mode, OutputOrdering};
use dedupe_job_runner::{JobEvent, JobManager};
use std::fs;
use std::time::{Duration, Instant};

fn make_ram_config(input: std::path::PathBuf, output: std::path::PathBuf) -> Config {
    Config {
        inputs: vec![input],
        output,
        output_separator: ",".to_string(),
        mode: Mode::Ram,
        ordering: OutputOrdering::PreserveFirstSeen,
        trim: true,
        drop_empty: true,
        disk_buckets: 64,
        disk_alphabetical_mode: DiskAlphabeticalMode::FastBucketLocal,
        disk_run_bytes: 2 * 1024 * 1024,
    }
}

fn make_disk_config(input: std::path::PathBuf, output: std::path::PathBuf) -> Config {
    Config {
        inputs: vec![input],
        output,
        output_separator: "\n".to_string(),
        mode: Mode::Disk,
        ordering: OutputOrdering::Alphabetical,
        trim: true,
        drop_empty: true,
        disk_buckets: 64,
        disk_alphabetical_mode: DiskAlphabeticalMode::GlobalPerfect,
        disk_run_bytes: 1_000_000,
    }
}

#[test]
fn start_job_emits_done_terminal_event() {
    let dir = tempfile::tempdir().expect("tempdir");
    let input = dir.path().join("in.txt");
    let output = dir.path().join("out.txt");
    fs::write(&input, "b,a,a;B\nb ; c, c, perro, Perro, PERRO").expect("write input");

    let manager = JobManager::new();
    let job_id = manager
        .start_job(make_ram_config(input, output.clone()))
        .expect("start job");

    let deadline = Instant::now() + Duration::from_secs(5);
    let mut saw_started = false;
    let mut saw_done = false;

    while Instant::now() < deadline {
        if let Some(event) = manager.next_event_timeout(Duration::from_millis(200)) {
            match event {
                JobEvent::Started { job_id: id } if id == job_id => saw_started = true,
                JobEvent::Done { job_id: id, stats } if id == job_id => {
                    saw_done = true;
                    assert!(stats.unique_tokens >= 1);
                    break;
                }
                JobEvent::Error {
                    job_id: id,
                    message,
                } if id == job_id => {
                    panic!("unexpected error: {message}")
                }
                JobEvent::Canceled { job_id: id } if id == job_id => {
                    panic!("unexpected canceled event")
                }
                _ => {}
            }
        }
    }

    assert!(saw_started, "missing started event");
    assert!(saw_done, "missing done event");
    assert!(!manager.is_running(), "manager should be idle after done");
}

#[test]
fn cancel_job_emits_canceled_terminal_event() {
    let dir = tempfile::tempdir().expect("tempdir");
    let input = dir.path().join("in.txt");
    let output = dir.path().join("out.txt");

    let mut payload = String::new();
    for i in 0..100_000 {
        payload.push_str(&format!("token_{i} alpha beta gamma delta epsilon zeta\n"));
    }
    fs::write(&input, payload).expect("write input");

    let manager = JobManager::new();
    let job_id = manager
        .start_job(make_disk_config(input, output))
        .expect("start job");
    assert!(manager.cancel_job(job_id), "cancel must be acknowledged");

    let deadline = Instant::now() + Duration::from_secs(20);
    let mut saw_canceled = false;

    while Instant::now() < deadline {
        if let Some(event) = manager.next_event_timeout(Duration::from_millis(250)) {
            match event {
                JobEvent::Canceled { job_id: id } if id == job_id => {
                    saw_canceled = true;
                    break;
                }
                JobEvent::Done { job_id: id, .. } if id == job_id => {
                    panic!("job completed before cancellation was applied")
                }
                JobEvent::Error {
                    job_id: id,
                    message,
                } if id == job_id => {
                    panic!("unexpected error after cancellation: {message}")
                }
                _ => {}
            }
        }
    }

    assert!(saw_canceled, "missing canceled event");
    assert!(!manager.is_running(), "manager should be idle after cancel");
}
