use dedupe_core::{run, Config, DiskAlphabeticalMode, Mode, NoProgress, OutputOrdering};
use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

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
        disk_buckets: 64,
        disk_alphabetical_mode: DiskAlphabeticalMode::FastBucketLocal,
        disk_run_bytes: 2 * 1024 * 1024,
    }
}

#[test]
fn ram_preserve_first_seen_is_case_sensitive() {
    let dir = tempfile::tempdir().expect("tempdir");
    let input = dir.path().join("in.txt");
    let output = dir.path().join("out.txt");

    fs::write(&input, "b,a,a;B\nb ; c, c, perro, Perro, PERRO").expect("write input");

    let cfg = make_cfg(input, output.clone(), Mode::Ram, OutputOrdering::PreserveFirstSeen);
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

    let cfg = make_cfg(input, output.clone(), Mode::Ram, OutputOrdering::Alphabetical);
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
    let auto_cfg = make_cfg(input, out_auto.clone(), Mode::Auto, OutputOrdering::PreserveFirstSeen);

    run(&ram_cfg, NoProgress).expect("run ram");
    run(&auto_cfg, NoProgress).expect("run auto");

    let ram_out = fs::read_to_string(out_ram).expect("read ram");
    let auto_out = fs::read_to_string(out_auto).expect("read auto");
    assert_eq!(ram_out, auto_out);
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

    let mut cfg = make_cfg(input, output.clone(), Mode::Disk, OutputOrdering::Alphabetical);
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

    let mut cfg = make_cfg(input, output.clone(), Mode::Ram, OutputOrdering::PreserveFirstSeen);
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

    let cfg = make_cfg(input, output.clone(), Mode::Disk, OutputOrdering::PreserveFirstSeen);
    run(&cfg, NoProgress).expect("run");

    let out = fs::read_to_string(output).expect("read output");
    let got: BTreeSet<_> = out.split(',').map(|s| s.to_string()).collect();
    let expected: BTreeSet<_> = ["uno", "dos", "tres", "cuatro", "cinco"]
        .iter()
        .map(|s| s.to_string())
        .collect();

    assert_eq!(got, expected);
}
