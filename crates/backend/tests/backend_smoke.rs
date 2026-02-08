use dedupe_backend::{
    ApiDiskAlphabeticalMode, ApiMode, ApiOrdering, BackendJobEvent, BackendService, CancelJobRequest,
    StartJobConfig, StartJobRequest,
};
use std::fs;
use std::time::{Duration, Instant};

fn make_ram_request(input: String, output: String) -> StartJobRequest {
    StartJobRequest {
        config: StartJobConfig {
            inputs: vec![input],
            output,
            output_separator: ",".to_string(),
            interpret_separator_escapes: false,
            mode: ApiMode::Ram,
            ordering: ApiOrdering::PreserveFirstSeen,
            trim: true,
            drop_empty: true,
            disk_buckets: 64,
            disk_alphabetical_mode: ApiDiskAlphabeticalMode::FastBucketLocal,
            disk_run_bytes: 2 * 1024 * 1024,
        },
    }
}

fn make_disk_request(input: String, output: String) -> StartJobRequest {
    StartJobRequest {
        config: StartJobConfig {
            inputs: vec![input],
            output,
            output_separator: "\\n".to_string(),
            interpret_separator_escapes: true,
            mode: ApiMode::Disk,
            ordering: ApiOrdering::Alphabetical,
            trim: true,
            drop_empty: true,
            disk_buckets: 64,
            disk_alphabetical_mode: ApiDiskAlphabeticalMode::GlobalPerfect,
            disk_run_bytes: 1_000_000,
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
