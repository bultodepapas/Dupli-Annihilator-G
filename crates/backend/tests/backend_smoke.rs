use dedupe_backend::{
    ApiDiskAlphabeticalMode, ApiMode, ApiOrdering, BackendJobEvent, BackendService,
    CancelJobRequest, StartJobConfig, StartJobRequest,
};
use std::fs;
use std::time::{Duration, Instant};

fn make_ram_request(input: String, output: String) -> StartJobRequest {
    StartJobRequest {
        config: StartJobConfig {
            inputs: vec![input],
            output,
            allow_overwrite: false,
            output_separator: ",".to_string(),
            interpret_separator_escapes: false,
            mode: ApiMode::Ram,
            ordering: ApiOrdering::PreserveFirstSeen,
            trim: true,
            drop_empty: true,
            drop_length_min: None,
            drop_length_max: None,
            disk_buckets: 64,
            disk_alphabetical_mode: ApiDiskAlphabeticalMode::FastBucketLocal,
            disk_run_bytes: 2 * 1024 * 1024,
            per_file_stats: false,
        },
    }
}

fn make_disk_request(input: String, output: String) -> StartJobRequest {
    StartJobRequest {
        config: StartJobConfig {
            inputs: vec![input],
            output,
            allow_overwrite: false,
            output_separator: "\\n".to_string(),
            interpret_separator_escapes: true,
            mode: ApiMode::Disk,
            ordering: ApiOrdering::Alphabetical,
            trim: true,
            drop_empty: true,
            drop_length_min: None,
            drop_length_max: None,
            disk_buckets: 64,
            disk_alphabetical_mode: ApiDiskAlphabeticalMode::GlobalPerfect,
            disk_run_bytes: 1_000_000,
            per_file_stats: false,
        },
    }
}

#[test]
fn start_job_forwards_done_event() {
    let dir = tempfile::tempdir().expect("tempdir");
    let input = dir.path().join("in.txt");
    let output = dir.path().join("out.txt");
    fs::write(&input, "b,a,a;B\nb ; c, c, perro, Perro, PERRO").expect("write input");

    let backend = BackendService::new();
    let started = backend
        .start_job(make_ram_request(
            input.to_string_lossy().to_string(),
            output.to_string_lossy().to_string(),
        ))
        .expect("start job");

    let deadline = Instant::now() + Duration::from_secs(5);
    let mut saw_done = false;
    while Instant::now() < deadline {
        if let Some(ev) = backend.next_emitted_event_timeout(Duration::from_millis(200)) {
            if ev.topic == "job://done" {
                saw_done = true;
                let got_job_id = ev
                    .payload
                    .get("job_id")
                    .and_then(|v| v.as_u64())
                    .expect("done.job_id");
                assert_eq!(got_job_id, started.job_id);
                break;
            }
            if ev.topic == "job://error" {
                panic!("unexpected error event: {}", ev.payload);
            }
        }
    }
    assert!(saw_done, "expected done event");

    let out = fs::read_to_string(&output).expect("read output");
    assert_eq!(out, "b,a,B,c,perro,Perro,PERRO");
}

#[test]
fn cancel_job_forwards_canceled_event() {
    let dir = tempfile::tempdir().expect("tempdir");
    let input = dir.path().join("in.txt");
    let output = dir.path().join("out.txt");

    let mut payload = String::new();
    for i in 0..120_000 {
        payload.push_str(&format!("token_{i} alpha beta gamma delta epsilon zeta\n"));
    }
    fs::write(&input, payload).expect("write input");

    let backend = BackendService::new();
    let started = backend
        .start_job(make_disk_request(
            input.to_string_lossy().to_string(),
            output.to_string_lossy().to_string(),
        ))
        .expect("start job");

    let cancel = backend.cancel_job(CancelJobRequest {
        job_id: started.job_id,
    });
    assert!(cancel.acknowledged, "cancel not acknowledged");

    let deadline = Instant::now() + Duration::from_secs(20);
    let mut saw_canceled = false;
    while Instant::now() < deadline {
        if let Some(ev) = backend.next_emitted_event_timeout(Duration::from_millis(250)) {
            if ev.topic == "job://canceled" {
                saw_canceled = true;
                break;
            }
            if ev.topic == "job://done" {
                panic!("job finished before cancellation");
            }
        }
    }
    assert!(saw_canceled, "expected canceled event");
}

#[test]
fn second_start_is_rejected_while_running() {
    let dir = tempfile::tempdir().expect("tempdir");
    let input1 = dir.path().join("in1.txt");
    let output1 = dir.path().join("out1.txt");
    let input2 = dir.path().join("in2.txt");
    let output2 = dir.path().join("out2.txt");
    fs::write(&input1, "a b c d e").expect("write input1");
    fs::write(&input2, "f g h i j").expect("write input2");

    let backend = BackendService::new();
    backend
        .start_job(make_ram_request(
            input1.to_string_lossy().to_string(),
            output1.to_string_lossy().to_string(),
        ))
        .expect("first job");

    let err = backend
        .start_job(make_ram_request(
            input2.to_string_lossy().to_string(),
            output2.to_string_lossy().to_string(),
        ))
        .expect_err("second start should fail");
    assert_eq!(err.category, "job_busy");
}

#[test]
fn get_app_info_is_available() {
    let backend = BackendService::new();
    let info = backend.get_app_info();
    assert_eq!(info.app_name, "Dupli-Annihilator-G");
    assert!(!info.app_version.is_empty());
    assert!(!info.backend_version.is_empty());
    assert!(!info.update_channel.is_empty());
}

#[test]
fn invalid_config_fails_fast_on_start() {
    let backend = BackendService::new();
    let err = backend
        .start_job(StartJobRequest {
            config: StartJobConfig {
                inputs: vec![],
                output: String::new(),
                allow_overwrite: false,
                output_separator: "\n".to_string(),
                interpret_separator_escapes: false,
                mode: ApiMode::Ram,
                ordering: ApiOrdering::PreserveFirstSeen,
                trim: true,
                drop_empty: true,
                drop_length_min: None,
                drop_length_max: None,
                disk_buckets: 64,
                disk_alphabetical_mode: ApiDiskAlphabeticalMode::FastBucketLocal,
                disk_run_bytes: 2 * 1024 * 1024,
                per_file_stats: false,
            },
        })
        .expect_err("invalid config should fail before background start");

    assert_eq!(err.category, "invalid_config");
    let state = backend.get_runtime_state();
    assert!(!state.is_running);
    assert!(state.active_job_id.is_none());
}

#[test]
fn typed_event_stream_exposes_done_event() {
    let dir = tempfile::tempdir().expect("tempdir");
    let input = dir.path().join("in.txt");
    let output = dir.path().join("out.txt");
    fs::write(&input, "a a b c").expect("write input");

    let backend = BackendService::new();
    let start = backend
        .start_job(make_ram_request(
            input.to_string_lossy().to_string(),
            output.to_string_lossy().to_string(),
        ))
        .expect("start");

    let deadline = Instant::now() + Duration::from_secs(5);
    let mut saw_done = false;
    while Instant::now() < deadline {
        let Some(ev) = backend.next_event_timeout(Duration::from_millis(200)) else {
            continue;
        };
        if let BackendJobEvent::Done { job_id, .. } = ev {
            assert_eq!(job_id, start.job_id);
            saw_done = true;
            break;
        }
    }

    assert!(saw_done, "expected typed done event");
}

#[test]
fn runtime_state_transitions_running_to_idle() {
    let dir = tempfile::tempdir().expect("tempdir");
    let input = dir.path().join("in.txt");
    let output = dir.path().join("out.txt");
    let mut payload = String::new();
    for i in 0..80_000 {
        payload.push_str(&format!("token_{i} alpha beta gamma delta epsilon zeta\n"));
    }
    fs::write(&input, payload).expect("write input");

    let backend = BackendService::new();
    let start = backend
        .start_job(make_disk_request(
            input.to_string_lossy().to_string(),
            output.to_string_lossy().to_string(),
        ))
        .expect("start");

    let mut observed_running = false;
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        let state = backend.get_runtime_state();
        if state.is_running {
            observed_running = true;
            assert_eq!(state.active_job_id, Some(start.job_id));
            break;
        }
    }
    assert!(observed_running, "expected running state");

    let mut observed_idle = false;
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        if let Some(ev) = backend.next_event_timeout(Duration::from_millis(200)) {
            if let BackendJobEvent::Done { .. } = ev {
                let state = backend.get_runtime_state();
                if !state.is_running && state.active_job_id.is_none() {
                    observed_idle = true;
                    break;
                }
            }
        }
    }
    assert!(observed_idle, "expected idle state after terminal event");
}

#[test]
fn emitted_batch_respects_limit() {
    let dir = tempfile::tempdir().expect("tempdir");
    let input = dir.path().join("in.txt");
    let output = dir.path().join("out.txt");
    fs::write(&input, "x y z x y z").expect("write input");

    let backend = BackendService::new();
    backend
        .start_job(make_ram_request(
            input.to_string_lossy().to_string(),
            output.to_string_lossy().to_string(),
        ))
        .expect("start");

    let deadline = Instant::now() + Duration::from_secs(5);
    let mut saw_done = false;
    while Instant::now() < deadline {
        let batch = backend.next_emitted_events_batch(Duration::from_millis(200), 2);
        assert!(batch.len() <= 2, "batch exceeded max_events");
        if batch.is_empty() {
            continue;
        }

        if batch.iter().any(|ev| ev.topic == "job://done") {
            saw_done = true;
            break;
        }
    }

    assert!(saw_done, "expected done event in emitted batch stream");
}

#[test]
fn start_rejects_existing_output_without_overwrite_flag() {
    let dir = tempfile::tempdir().expect("tempdir");
    let input = dir.path().join("in.txt");
    let output = dir.path().join("out.txt");
    fs::write(&input, "a b c").expect("write input");
    fs::write(&output, "already here").expect("write output");

    let backend = BackendService::new();
    let err = backend
        .start_job(make_ram_request(
            input.to_string_lossy().to_string(),
            output.to_string_lossy().to_string(),
        ))
        .expect_err("should reject existing output");
    assert_eq!(err.category, "output_exists");
}

#[test]
fn start_allows_existing_output_when_overwrite_enabled() {
    let dir = tempfile::tempdir().expect("tempdir");
    let input = dir.path().join("in.txt");
    let output = dir.path().join("out.txt");
    fs::write(&input, "a b a c").expect("write input");
    fs::write(&output, "already here").expect("write output");

    let mut req = make_ram_request(
        input.to_string_lossy().to_string(),
        output.to_string_lossy().to_string(),
    );
    req.config.allow_overwrite = true;

    let backend = BackendService::new();
    backend.start_job(req).expect("start with overwrite");

    let deadline = Instant::now() + Duration::from_secs(5);
    let mut done = false;
    while Instant::now() < deadline {
        if let Some(ev) = backend.next_event_timeout(Duration::from_millis(200)) {
            if let BackendJobEvent::Done { .. } = ev {
                done = true;
                break;
            }
        }
    }
    assert!(done, "expected done");

    let out = fs::read_to_string(&output).expect("read output");
    assert_eq!(out, "a,b,c");
}
