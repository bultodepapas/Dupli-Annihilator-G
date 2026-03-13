use dedupe_core::{Config, DiskAlphabeticalMode, Mode, OutputOrdering};
use dedupe_job_runner::{JobEvent, JobManager, RunTerminalStatus};
use lopdf::content::{Content, Operation};
use lopdf::dictionary;
use lopdf::{Document, Object, Stream};
use std::fs;
use std::path::Path;
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
        drop_length_min: None,
        drop_length_max: None,
        disk_buckets: 64,
        disk_alphabetical_mode: DiskAlphabeticalMode::FastBucketLocal,
        disk_run_bytes: 2 * 1024 * 1024,
        per_file_stats: false,
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
        drop_length_min: None,
        drop_length_max: None,
        disk_buckets: 64,
        disk_alphabetical_mode: DiskAlphabeticalMode::GlobalPerfect,
        disk_run_bytes: 1_000_000,
        per_file_stats: false,
    }
}

fn make_auto_config(input: std::path::PathBuf, output: std::path::PathBuf) -> Config {
    Config {
        mode: Mode::Auto,
        ..make_ram_config(input, output)
    }
}

fn write_pdf(path: &Path, pages: &[&str]) {
    let mut doc = Document::with_version("1.5");
    let pages_id = doc.new_object_id();

    let font_id = doc.add_object(lopdf::dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Courier",
    });
    let resources_id = doc.add_object(lopdf::dictionary! {
        "Font" => lopdf::dictionary! {
            "F1" => font_id,
        },
    });

    let mut kids = Vec::with_capacity(pages.len());
    for page_text in pages {
        let content = Content {
            operations: vec![
                Operation::new("BT", vec![]),
                Operation::new("Tf", vec!["F1".into(), 24.into()]),
                Operation::new("Td", vec![100.into(), 700.into()]),
                Operation::new("Tj", vec![Object::string_literal(*page_text)]),
                Operation::new("ET", vec![]),
            ],
        };
        let content_id = doc.add_object(Stream::new(
            lopdf::dictionary! {},
            content.encode().expect("encode pdf content"),
        ));
        let page_id = doc.add_object(lopdf::dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "Contents" => content_id,
        });
        kids.push(page_id.into());
    }

    let pages_dict = lopdf::dictionary! {
        "Type" => "Pages",
        "Kids" => kids,
        "Count" => pages.len() as i64,
        "Resources" => resources_id,
        "MediaBox" => vec![0.into(), 0.into(), 595.into(), 842.into()],
    };
    doc.objects.insert(pages_id, Object::Dictionary(pages_dict));

    let catalog_id = doc.add_object(lopdf::dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
    });
    doc.trailer.set("Root", catalog_id);
    doc.compress();
    doc.save(path).expect("save pdf");
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

#[test]
fn job_event_topics_and_json_are_stable() {
    let ev = JobEvent::Progress {
        job_id: 7,
        stage: Some("Tokenizing".to_string()),
        files_done: 1,
        files_total: 3,
        stage_items_done: 0,
        stage_items_total: 0,
        current_input_path: None,
        progress_ppm: Some(333_333),
        tokens_seen: 10_000,
        unique_tokens: 9_000,
        duplicates: 1_000,
        throughput_tps: 25_000,
        elapsed_ms: 500,
        eta_ms: Some(1_000),
    };

    assert_eq!(ev.topic(), "job://progress");
    let v = ev.to_json_value();
    assert_eq!(v.get("type").and_then(|x| x.as_str()), Some("progress"));
    assert_eq!(v.get("job_id").and_then(|x| x.as_u64()), Some(7));
}

#[test]
fn done_job_emits_summary_with_actionable_metrics() {
    let dir = tempfile::tempdir().expect("tempdir");
    let input = dir.path().join("in.txt");
    let output = dir.path().join("out.txt");
    fs::write(&input, "a,b,b;c\na c").expect("write input");

    let manager = JobManager::new();
    let job_id = manager
        .start_job(make_ram_config(input, output.clone()))
        .expect("start job");

    let deadline = Instant::now() + Duration::from_secs(5);
    let mut saw_summary = false;
    while Instant::now() < deadline {
        let Some(event) = manager.next_event_timeout(Duration::from_millis(200)) else {
            continue;
        };

        if let JobEvent::Summary {
            job_id: id,
            summary,
        } = event
        {
            if id != job_id {
                continue;
            }
            saw_summary = true;
            assert_eq!(summary.status, RunTerminalStatus::Success);
            assert_eq!(summary.output_path, output.to_string_lossy());
            assert_eq!(summary.tokens_seen, 6);
            assert_eq!(summary.unique_tokens, 3);
            assert_eq!(summary.duplicates, 3);
            assert_eq!(summary.reduction_pct, 50.0);
            assert_eq!(summary.uniq_pct, 50.0);
            assert!(summary.output_bytes > 0);
            assert!(summary.avg_throughput_tps > 0);
            assert_eq!(summary.mode, "ram");
            assert_eq!(summary.mode_effective, "ram");
            assert_eq!(summary.ordering, "preserve_first_seen");
            assert_eq!(summary.output_separator_raw, ",");
            assert_eq!(summary.output_separator_preview, ",");
            assert_eq!(summary.auto_decision_reason, None);
            break;
        }
    }

    assert!(saw_summary, "missing summary event");
}

#[test]
fn auto_summary_includes_decision_telemetry() {
    let dir = tempfile::tempdir().expect("tempdir");
    let input = dir.path().join("in.txt");
    let output = dir.path().join("out.txt");
    fs::write(&input, "a,b,b;c\na c").expect("write input");

    let manager = JobManager::new();
    let job_id = manager
        .start_job(make_auto_config(input, output))
        .expect("start job");

    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        let Some(event) = manager.next_event_timeout(Duration::from_millis(200)) else {
            continue;
        };

        if let JobEvent::Summary {
            job_id: id,
            summary,
        } = event
        {
            if id != job_id {
                continue;
            }
            assert_eq!(summary.mode, "auto");
            assert!(summary.auto_sample_tokens.unwrap_or(0) > 0);
            assert!(summary.auto_decision_reason.is_some());
            return;
        }
    }

    panic!("missing auto summary event");
}

#[test]
fn extracting_text_progress_is_monotonic_and_clears_current_path() {
    let dir = tempfile::tempdir().expect("tempdir");
    let output = dir.path().join("out.txt");

    let mut inputs = Vec::new();
    for idx in 0..3 {
        let pdf = dir.path().join(format!("rich_{idx}.pdf"));
        write_pdf(
            &pdf,
            &["alpha bravo", "charlie delta", "echo foxtrot", "golf hotel"],
        );
        inputs.push(pdf);
    }

    let mut cfg = make_ram_config(inputs[0].clone(), output);
    cfg.inputs = inputs;

    let manager = JobManager::new();
    let job_id = manager.start_job(cfg).expect("start job");

    let deadline = Instant::now() + Duration::from_secs(10);
    let mut last_done = 0usize;
    let mut saw_extracting_progress = false;
    let mut saw_active_path = false;
    let mut saw_cleared_path = false;

    while Instant::now() < deadline {
        let Some(event) = manager.next_event_timeout(Duration::from_millis(200)) else {
            continue;
        };

        match event {
            JobEvent::Progress {
                job_id: id,
                stage,
                stage_items_done,
                stage_items_total,
                current_input_path,
                ..
            } if id == job_id => {
                if stage.as_deref() != Some("ExtractingText") {
                    continue;
                }

                saw_extracting_progress = true;
                assert!(
                    stage_items_done <= stage_items_total,
                    "stage_items_done exceeded total: {stage_items_done} > {stage_items_total}"
                );
                assert!(
                    stage_items_done >= last_done,
                    "stage_items_done regressed: {stage_items_done} < {last_done}"
                );
                last_done = stage_items_done;

                if stage_items_done < stage_items_total && current_input_path.is_some() {
                    saw_active_path = true;
                }
                if stage_items_done == stage_items_total && current_input_path.is_none() {
                    saw_cleared_path = true;
                }
            }
            JobEvent::Done { job_id: id, .. } if id == job_id => break,
            JobEvent::Error {
                job_id: id,
                message,
            } if id == job_id => panic!("unexpected error: {message}"),
            JobEvent::Canceled { job_id: id } if id == job_id => {
                panic!("unexpected canceled event")
            }
            _ => {}
        }
    }

    assert!(saw_extracting_progress, "missing extracting-text progress");
    assert_eq!(
        last_done, 3,
        "extracting-text completion count must reach total"
    );
    assert!(
        saw_active_path,
        "missing active current_input_path during extraction"
    );
    assert!(
        saw_cleared_path,
        "current_input_path should clear when extracting-text work completes"
    );
}
