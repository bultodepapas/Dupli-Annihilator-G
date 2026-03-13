use dedupe_core::{
    is_canceled_error, run, run_with_control, CancellationToken, Config, DiskAlphabeticalMode,
    Mode, NoProgress, OutputOrdering, ProgressEvent, ProgressSink,
};
use lopdf::content::{Content, Operation};
use lopdf::dictionary;
use lopdf::{Document, Object, Stream};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

fn make_cfg(
    input_path: PathBuf,
    output_path: PathBuf,
    mode: Mode,
    ordering: OutputOrdering,
) -> Config {
    Config {
        inputs: vec![input_path],
        output: output_path,
        output_separator: ",".to_string(),
        mode,
        ordering,
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

#[derive(Clone)]
struct CancelOnRichStart {
    cancel: CancellationToken,
    cancel_after: usize,
    starts_seen: Arc<AtomicUsize>,
}

impl CancelOnRichStart {
    fn new(cancel: CancellationToken, cancel_after: usize) -> Self {
        Self {
            cancel,
            cancel_after,
            starts_seen: Arc::new(AtomicUsize::new(0)),
        }
    }
}

impl ProgressSink for CancelOnRichStart {
    fn on_event(&self, event: ProgressEvent) {
        if let ProgressEvent::StageItemStarted { .. } = event {
            let seen = self.starts_seen.fetch_add(1, Ordering::SeqCst) + 1;
            if seen >= self.cancel_after {
                self.cancel.cancel();
            }
        }
    }
}

#[test]
fn ram_preserve_first_seen_is_case_sensitive() {
    let dir = tempfile::tempdir().expect("tempdir");
    let input = dir.path().join("in.txt");
    let output = dir.path().join("out.txt");

    fs::write(&input, "b,a,a;B\nb ; c, c, perro, Perro, PERRO").expect("write input");

    let cfg = make_cfg(
        input,
        output.clone(),
        Mode::Ram,
        OutputOrdering::PreserveFirstSeen,
    );
    let stats = run(&cfg, NoProgress).expect("run");

    let out = fs::read_to_string(output).expect("read output");
    assert_eq!(out, "b,a,B,c,perro,Perro,PERRO");
    assert_eq!(stats.unique_tokens, 7);
}

#[test]
fn ram_alphabetical_uses_utf8_byte_order() {
    let dir = tempfile::tempdir().expect("tempdir");
    let input = dir.path().join("in.txt");
    let output = dir.path().join("out.txt");

    fs::write(&input, "b,a,B,c").expect("write input");

    let cfg = make_cfg(
        input,
        output.clone(),
        Mode::Ram,
        OutputOrdering::Alphabetical,
    );
    run(&cfg, NoProgress).expect("run");

    let out = fs::read_to_string(output).expect("read output");
    assert_eq!(out, "B,a,b,c");
}

#[test]
fn auto_mode_behaves_like_ram_in_v1() {
    let dir = tempfile::tempdir().expect("tempdir");
    let input = dir.path().join("in.txt");
    let out_ram = dir.path().join("out_ram.txt");
    let out_auto = dir.path().join("out_auto.txt");

    fs::write(&input, "x;z,y,x").expect("write input");

    let ram_cfg = make_cfg(
        input.clone(),
        out_ram.clone(),
        Mode::Ram,
        OutputOrdering::PreserveFirstSeen,
    );
    let auto_cfg = make_cfg(
        input,
        out_auto.clone(),
        Mode::Auto,
        OutputOrdering::PreserveFirstSeen,
    );

    run(&ram_cfg, NoProgress).expect("run ram");
    run(&auto_cfg, NoProgress).expect("run auto");

    let ram_out = fs::read_to_string(out_ram).expect("read ram");
    let auto_out = fs::read_to_string(out_auto).expect("read auto");
    assert_eq!(ram_out, auto_out);
}

#[test]
fn ram_preserve_first_seen_keeps_mixed_plain_and_pdf_order() {
    let dir = tempfile::tempdir().expect("tempdir");
    let input_a = dir.path().join("a.txt");
    let input_pdf_1 = dir.path().join("b.pdf");
    let input_pdf_2 = dir.path().join("c.pdf");
    let input_d = dir.path().join("d.txt");
    let output = dir.path().join("out.txt");

    fs::write(&input_a, "alpha").expect("write input a");
    write_pdf(&input_pdf_1, &["bravo"]);
    write_pdf(&input_pdf_2, &["charlie"]);
    fs::write(&input_d, "delta").expect("write input d");

    let mut cfg = make_cfg(
        input_a.clone(),
        output.clone(),
        Mode::Ram,
        OutputOrdering::PreserveFirstSeen,
    );
    cfg.inputs = vec![input_a, input_pdf_1, input_pdf_2, input_d];

    run(&cfg, NoProgress).expect("run");

    let out = fs::read_to_string(output).expect("read output");
    assert_eq!(out, "alpha,bravo,charlie,delta");
}

#[test]
fn invalid_pdf_is_reported_and_skipped() {
    let dir = tempfile::tempdir().expect("tempdir");
    let plain = dir.path().join("ok.txt");
    let broken_pdf = dir.path().join("broken.pdf");
    let output = dir.path().join("out.txt");

    fs::write(&plain, "safe").expect("write plain input");
    fs::write(&broken_pdf, "definitely not a real pdf").expect("write fake pdf");

    let mut cfg = make_cfg(
        plain.clone(),
        output.clone(),
        Mode::Ram,
        OutputOrdering::PreserveFirstSeen,
    );
    cfg.inputs = vec![plain, broken_pdf.clone()];

    let stats = run(&cfg, NoProgress).expect("run");

    let out = fs::read_to_string(output).expect("read output");
    assert_eq!(out, "safe");
    assert_eq!(stats.failed_pdfs.len(), 1);
    assert_eq!(stats.failed_pdfs[0].0, broken_pdf);
}

#[test]
fn mixed_plain_pdf_with_failed_rich_input_preserves_first_seen_order() {
    let dir = tempfile::tempdir().expect("tempdir");
    let input_a = dir.path().join("a.txt");
    let input_pdf = dir.path().join("b.pdf");
    let broken_pdf = dir.path().join("broken.pdf");
    let input_d = dir.path().join("d.txt");
    let output = dir.path().join("out.txt");

    fs::write(&input_a, "alpha").expect("write input a");
    write_pdf(&input_pdf, &["bravo"]);
    fs::write(&broken_pdf, "not a pdf").expect("write broken pdf");
    fs::write(&input_d, "delta").expect("write input d");

    let mut cfg = make_cfg(
        input_a.clone(),
        output.clone(),
        Mode::Ram,
        OutputOrdering::PreserveFirstSeen,
    );
    cfg.inputs = vec![input_a, input_pdf, broken_pdf.clone(), input_d];

    let stats = run(&cfg, NoProgress).expect("run");

    let out = fs::read_to_string(output).expect("read output");
    assert_eq!(out, "alpha,bravo,delta");
    assert_eq!(stats.failed_pdfs.len(), 1);
    assert_eq!(stats.failed_pdfs[0].0, broken_pdf);
}

#[test]
fn pdf_multi_page_extraction_keeps_page_token_boundaries() {
    let dir = tempfile::tempdir().expect("tempdir");
    let input_pdf = dir.path().join("multi.pdf");
    let output = dir.path().join("out.txt");

    write_pdf(&input_pdf, &["alpha", "beta"]);

    let cfg = make_cfg(
        input_pdf,
        output.clone(),
        Mode::Ram,
        OutputOrdering::PreserveFirstSeen,
    );
    run(&cfg, NoProgress).expect("run");

    let out = fs::read_to_string(output).expect("read output");
    assert_eq!(out, "alpha,beta");
    assert!(!out.contains("alphabeta"));
}

#[test]
fn cancel_during_parallel_rich_extraction_returns_canceled() {
    let dir = tempfile::tempdir().expect("tempdir");
    let output = dir.path().join("out.txt");

    let mut inputs = Vec::new();
    for idx in 0..3 {
        let pdf = dir.path().join(format!("doc_{idx}.pdf"));
        let pages: Vec<String> = (0..200)
            .map(|page| format!("token_{idx}_{page} repeated repeated"))
            .collect();
        let page_refs: Vec<&str> = pages.iter().map(String::as_str).collect();
        write_pdf(&pdf, &page_refs);
        inputs.push(pdf);
    }

    let mut cfg = make_cfg(
        inputs[0].clone(),
        output,
        Mode::Ram,
        OutputOrdering::PreserveFirstSeen,
    );
    cfg.inputs = inputs;

    let cancel = CancellationToken::new();
    let progress = CancelOnRichStart::new(cancel.clone(), 1);
    let err = run_with_control(&cfg, progress, cancel).expect_err("run should cancel");
    assert!(is_canceled_error(&err), "unexpected error: {err:#}");
}

#[test]
fn disk_globalperfect_produces_global_alphabetical_order() {
    let dir = tempfile::tempdir().expect("tempdir");
    let input = dir.path().join("in.txt");
    let output = dir.path().join("out.txt");

    let mut content = String::new();
    content.push_str("zeta alpha beta gamma alpha\n");
    content.push_str("delta beta epsilon\n");
    fs::write(&input, content).expect("write input");

    let mut cfg = make_cfg(
        input,
        output.clone(),
        Mode::Disk,
        OutputOrdering::Alphabetical,
    );
    cfg.disk_alphabetical_mode = DiskAlphabeticalMode::GlobalPerfect;
    cfg.output_separator = "|".to_string();

    run(&cfg, NoProgress).expect("run");
    let out = fs::read_to_string(output).expect("read output");
    assert_eq!(out, "alpha|beta|delta|epsilon|gamma|zeta");
}

#[test]
fn separator_is_applied_without_trailing_separator() {
    let dir = tempfile::tempdir().expect("tempdir");
    let input = dir.path().join("in.txt");
    let output = dir.path().join("out.txt");

    fs::write(&input, "a b c a").expect("write input");

    let mut cfg = make_cfg(
        input,
        output.clone(),
        Mode::Ram,
        OutputOrdering::PreserveFirstSeen,
    );
    cfg.output_separator = ",\n".to_string();

    run(&cfg, NoProgress).expect("run");
    let out = fs::read_to_string(output).expect("read output");
    assert_eq!(out, "a,\nb,\nc");
    assert!(!out.ends_with("\n\n"));
}

#[test]
fn disk_preserve_first_seen_keeps_exact_unique_set() {
    let dir = tempfile::tempdir().expect("tempdir");
    let input = dir.path().join("in.txt");
    let output = dir.path().join("out.txt");

    fs::write(&input, "uno dos tres dos uno cuatro\ncuatro cinco cinco").expect("write input");

    let cfg = make_cfg(
        input,
        output.clone(),
        Mode::Disk,
        OutputOrdering::PreserveFirstSeen,
    );
    run(&cfg, NoProgress).expect("run");

    let out = fs::read_to_string(output).expect("read output");
    let got: BTreeSet<_> = out.split(',').map(|s| s.to_string()).collect();
    let expected: BTreeSet<_> = ["uno", "dos", "tres", "cuatro", "cinco"]
        .iter()
        .map(|s| s.to_string())
        .collect();

    assert_eq!(got, expected);
}

#[test]
fn ram_accepts_non_utf8_input_lossy() {
    let dir = tempfile::tempdir().expect("tempdir");
    let input = dir.path().join("in.bin");
    let output = dir.path().join("out.txt");

    let bytes = [b'a', b',', 0xFF, b',', b'a', b'\n', 0xFE, b',', b'b'];
    fs::write(&input, bytes).expect("write input");

    let cfg = make_cfg(
        input,
        output.clone(),
        Mode::Ram,
        OutputOrdering::PreserveFirstSeen,
    );
    run(&cfg, NoProgress).expect("run");

    let out = fs::read_to_string(output).expect("read output");
    assert_eq!(out, format!("a,\u{FFFD},b"));
}

#[test]
fn disk_globalperfect_accepts_non_utf8_input_lossy() {
    let dir = tempfile::tempdir().expect("tempdir");
    let input = dir.path().join("in.bin");
    let output = dir.path().join("out.txt");

    let bytes = [b'a', b',', 0xFF, b',', b'a', b'\n', 0xFE, b',', b'b'];
    fs::write(&input, bytes).expect("write input");

    let mut cfg = make_cfg(
        input,
        output.clone(),
        Mode::Disk,
        OutputOrdering::Alphabetical,
    );
    cfg.disk_alphabetical_mode = DiskAlphabeticalMode::GlobalPerfect;
    cfg.output_separator = "|".to_string();

    run(&cfg, NoProgress).expect("run");

    let out = fs::read_to_string(output).expect("read output");
    assert_eq!(out, format!("a|b|\u{FFFD}"));
}
