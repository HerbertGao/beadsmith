//! bead-cli end-to-end integration tests (M6, tasks §6.7 / §6.8).
//!
//! Zero new dependencies: drives the built binary via `std::process::Command`
//! on `env!("CARGO_BIN_EXE_bead-cli")`, uses `env!("CARGO_TARGET_TMPDIR")` for
//! scratch dirs (no `tempfile`), and does **not** parse JSON here (no
//! `serde_json` in bead-cli — JSON shape is asserted in bead-core's pipeline
//! tests; CLI only checks UTF-8 + a leading `{`). `samples/` and `palettes/`
//! live at the repo root, but the integration-test CWD is the package dir
//! (`crates/bead-cli`), so inputs are resolved via `CARGO_MANIFEST_DIR`.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Absolute path to the built `bead-cli` binary under test.
const BIN: &str = env!("CARGO_BIN_EXE_bead-cli");

/// Repo-root-relative input asset, resolved from the package manifest dir
/// (`CARGO_MANIFEST_DIR` == `crates/bead-cli`; two levels up is the repo root).
fn asset(rel: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(rel)
}

/// A unique scratch dir under `CARGO_TARGET_TMPDIR`, keyed by `tag` + pid so the
/// two tests never collide. Recreated fresh (removed if it already exists).
fn scratch(tag: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("{tag}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("create scratch dir");
    dir
}

/// 6.7 — CLI end-to-end: generate writes four non-empty deterministic files,
/// and the palette / stub subcommands return the documented exit codes.
#[test]
fn cli_generate_and_palette_subcommands() {
    let work = scratch("cli-e2e");
    let input = asset("samples/gradient.png");
    let good_palette = asset("palettes/artkal_s.json");
    let out = work.join("sub");

    // --- generate: exit 0, four non-empty files -----------------------------
    let run_generate = || {
        Command::new(BIN)
            .args(["generate", "--input"])
            .arg(&input)
            .arg("--palette")
            .arg(&good_palette)
            .args(["--width", "16", "--height", "20", "--output"])
            .arg(&out)
            .output()
            .expect("run generate")
    };

    let first = run_generate();
    assert!(
        first.status.success(),
        "generate must exit 0; stderr: {}",
        String::from_utf8_lossy(&first.stderr)
    );

    let preview = out.join("preview.png");
    let grid = out.join("grid.png");
    let pattern = out.join("pattern.json");
    let summary = out.join("summary.txt");
    for f in [&preview, &grid, &pattern, &summary] {
        let meta = fs::metadata(f).unwrap_or_else(|e| panic!("missing {f:?}: {e}"));
        assert!(meta.len() > 0, "{f:?} must be non-empty");
    }

    // pattern.json: valid UTF-8 and starts with '{' (no JSON parsing in CLI).
    let json_bytes = fs::read(&pattern).expect("read pattern.json");
    let json_text = std::str::from_utf8(&json_bytes).expect("pattern.json must be valid UTF-8");
    assert!(
        json_text.trim_start().starts_with('{'),
        "pattern.json must start with '{{', got: {:?}",
        &json_text[..json_text.len().min(32)]
    );

    // summary.txt: first line is the fixed header.
    let summary_text = fs::read_to_string(&summary).expect("read summary.txt");
    assert_eq!(
        summary_text.lines().next(),
        Some("Bead Pattern Summary"),
        "summary.txt first line must be 'Bead Pattern Summary'"
    );

    // --- determinism: rerun same args overwrites byte-identical files --------
    let preview_a = fs::read(&preview).unwrap();
    let grid_a = fs::read(&grid).unwrap();
    let pattern_a = fs::read(&pattern).unwrap();
    let summary_a = fs::read(&summary).unwrap();

    let second = run_generate();
    assert!(
        second.status.success(),
        "second generate must exit 0; stderr: {}",
        String::from_utf8_lossy(&second.stderr)
    );

    assert_eq!(
        preview_a,
        fs::read(&preview).unwrap(),
        "preview.png not byte-identical on rerun"
    );
    assert_eq!(
        grid_a,
        fs::read(&grid).unwrap(),
        "grid.png not byte-identical on rerun"
    );
    assert_eq!(
        pattern_a,
        fs::read(&pattern).unwrap(),
        "pattern.json not byte-identical on rerun"
    );
    assert_eq!(
        summary_a,
        fs::read(&summary).unwrap(),
        "summary.txt not byte-identical on rerun"
    );

    // --- palette validate (good) -> exit 0 ----------------------------------
    let ok = Command::new(BIN)
        .arg("palette")
        .arg("validate")
        .arg(&good_palette)
        .output()
        .expect("run palette validate (good)");
    assert!(
        ok.status.success(),
        "palette validate <good> must exit 0; stderr: {}",
        String::from_utf8_lossy(&ok.stderr)
    );

    // --- palette validate (bad) -> non-zero + stderr names the reason -------
    let bad_palette = work.join("bad_palette.json");
    fs::write(&bad_palette, br#"{"brand":"X","colors":[]}"#).expect("write bad palette");
    let bad = Command::new(BIN)
        .arg("palette")
        .arg("validate")
        .arg(&bad_palette)
        .output()
        .expect("run palette validate (bad)");
    assert!(
        !bad.status.success(),
        "palette validate <bad> must exit non-zero"
    );
    let bad_stderr = String::from_utf8_lossy(&bad.stderr);
    assert!(
        bad_stderr.contains("no colors"),
        "palette validate <bad> stderr must surface the core reason (\"no colors\"); got: {bad_stderr:?}"
    );

    // --- stub commands: exit non-zero (clap exit 1) + "coming soon" ---------
    let list = Command::new(BIN)
        .args(["palette", "list"])
        .output()
        .expect("run palette list");
    assert!(
        !list.status.success(),
        "palette list (stub) must exit non-zero"
    );
    assert!(
        String::from_utf8_lossy(&list.stderr).contains("coming soon"),
        "palette list stderr must contain \"coming soon\""
    );

    let inspect = Command::new(BIN)
        .arg("inspect")
        .arg(work.join("whatever"))
        .output()
        .expect("run inspect");
    assert!(
        !inspect.status.success(),
        "inspect (stub) must exit non-zero"
    );
    assert!(
        String::from_utf8_lossy(&inspect.stderr).contains("coming soon"),
        "inspect stderr must contain \"coming soon\""
    );

    let _ = fs::remove_dir_all(&work);
}

#[test]
fn cli_generate_matcher_flag_accepts_known_values_and_rejects_unknown() {
    let work = scratch("cli-matcher");
    let input = asset("samples/gradient.png");
    let good_palette = asset("palettes/artkal_s.json");

    for matcher in ["rgb", "lab", "oklab"] {
        let out = work.join(format!("m-{matcher}"));
        let r = Command::new(BIN)
            .args(["generate", "--input"])
            .arg(&input)
            .arg("--palette")
            .arg(&good_palette)
            .args(["--width", "16", "--height", "20", "--output"])
            .arg(&out)
            .args(["--matcher", matcher])
            .output()
            .expect("run generate with matcher");

        assert!(
            r.status.success(),
            "generate --matcher {matcher} must succeed: {:?}",
            String::from_utf8_lossy(&r.stderr)
        );
        assert!(
            out.join("preview.png").exists(),
            "preview.png must exist for matcher {matcher}"
        );
        assert!(
            out.join("grid.png").exists(),
            "grid.png must exist for matcher {matcher}"
        );
        assert!(
            out.join("pattern.json").exists(),
            "pattern.json must exist for matcher {matcher}"
        );
        assert!(
            out.join("summary.txt").exists(),
            "summary.txt must exist for matcher {matcher}"
        );
    }

    let bad_out = work.join("invalid");
    let bad = Command::new(BIN)
        .args(["generate", "--input"])
        .arg(&input)
        .arg("--palette")
        .arg(&good_palette)
        .args(["--width", "16", "--height", "20", "--output"])
        .arg(&bad_out)
        .args(["--matcher", "hsv"])
        .output()
        .expect("run generate with invalid matcher");

    assert_eq!(
        bad.status.code(),
        Some(2),
        "invalid matcher should exit 2: {:?}",
        bad.status.code()
    );
    assert!(
        !bad.status.success(),
        "invalid matcher should be non-success"
    );
    let stderr = String::from_utf8_lossy(&bad.stderr).to_lowercase();
    assert!(stderr.contains("possible values"));
    assert!(stderr.contains("rgb") && stderr.contains("lab") && stderr.contains("oklab"));
    assert!(
        !bad_out.exists(),
        "invalid matcher should not create output path"
    );

    let _ = fs::remove_dir_all(&work);
}

/// 4.3 — `--max-colors N` limits the output bead color count to ≤ N (exit 0,
/// four non-empty files, summary color lines ≤ N); `--max-colors 0` exits
/// non-zero (1, not panic=101) with a contextual stderr message. Same fixture
/// pattern as the e2e test above; no new deps, no JSON parsing.
#[test]
fn cli_max_colors_ok_and_zero_rejected() {
    let work = scratch("cli-mc");
    let input = asset("samples/gradient.png");
    let good_palette = asset("palettes/artkal_s.json");

    // --- --max-colors 8: exit 0, four non-empty files, color count ≤ 8 -------
    let out = work.join("mc8");
    let ok = Command::new(BIN)
        .args(["generate", "--input"])
        .arg(&input)
        .arg("--palette")
        .arg(&good_palette)
        .args(["--width", "16", "--height", "20", "--output"])
        .arg(&out)
        .args(["--max-colors", "8"])
        .output()
        .expect("run generate --max-colors 8");
    assert!(
        ok.status.success(),
        "generate --max-colors 8 must exit 0; stderr: {}",
        String::from_utf8_lossy(&ok.stderr)
    );

    let preview = out.join("preview.png");
    let grid = out.join("grid.png");
    let pattern = out.join("pattern.json");
    let summary = out.join("summary.txt");
    for f in [&preview, &grid, &pattern, &summary] {
        let meta = fs::metadata(f).unwrap_or_else(|e| panic!("missing {f:?}: {e}"));
        assert!(meta.len() > 0, "{f:?} must be non-empty");
    }

    // summary.txt: 5 header lines (Bead Pattern Summary / Size / Total Beads /
    // Palette / blank) then one line per used color -> color count = lines - 5.
    let summary_text = fs::read_to_string(&summary).expect("read summary.txt");
    assert_eq!(
        summary_text.lines().next(),
        Some("Bead Pattern Summary"),
        "summary.txt first line must be 'Bead Pattern Summary'"
    );
    let color_count = summary_text.lines().count().saturating_sub(5);
    assert!(
        color_count <= 8,
        "summarized color count ({color_count}) must be ≤ --max-colors 8; summary:\n{summary_text}"
    );

    // --- --max-colors 0: rejected, non-zero exit (1), contextual stderr, no panic
    let bad = Command::new(BIN)
        .args(["generate", "--input"])
        .arg(&input)
        .arg("--palette")
        .arg(&good_palette)
        .args(["--width", "16", "--height", "20", "--output"])
        .arg(work.join("zero"))
        .args(["--max-colors", "0"])
        .output()
        .expect("run generate --max-colors 0");
    assert!(
        !bad.status.success(),
        "generate --max-colors 0 must exit non-zero (got success)"
    );
    assert_eq!(
        bad.status.code(),
        Some(1),
        "generate --max-colors 0 must exit 1 (not panic=101 / signal=None); got {:?}",
        bad.status.code()
    );
    let bad_stderr = String::from_utf8_lossy(&bad.stderr);
    assert!(
        bad_stderr.contains("max_colors"),
        "generate --max-colors 0 stderr must surface the error context (\"max_colors\"); got: {bad_stderr:?}"
    );

    let _ = fs::remove_dir_all(&work);
}

/// 4.2 — `--despeckle N` (N >= 1) exits 0 and writes four non-empty files;
/// `--despeckle 0` is a legal no-op producing output byte-identical to omitting
/// the flag; a non-`u32` value (e.g. `x`) is rejected by clap with exit code 2
/// (not panic=101), and no output dir is created. Same fixture/no-deps style.
#[test]
fn cli_despeckle_flag_ok_zero_noop_and_non_u32_rejected() {
    let work = scratch("cli-despeckle");
    let input = asset("samples/gradient.png");
    let good_palette = asset("palettes/artkal_s.json");

    let run = |out: &Path, extra: &[&str]| {
        let mut cmd = Command::new(BIN);
        cmd.args(["generate", "--input"])
            .arg(&input)
            .arg("--palette")
            .arg(&good_palette)
            .args(["--width", "16", "--height", "20", "--output"])
            .arg(out)
            .args(extra);
        cmd.output().expect("run generate")
    };

    let four = |out: &Path| {
        [
            out.join("preview.png"),
            out.join("grid.png"),
            out.join("pattern.json"),
            out.join("summary.txt"),
        ]
    };

    // --- --despeckle 1: exit 0, four non-empty files -------------------------
    let out1 = work.join("d1");
    let r1 = run(&out1, &["--despeckle", "1"]);
    assert!(
        r1.status.success(),
        "generate --despeckle 1 must exit 0; stderr: {}",
        String::from_utf8_lossy(&r1.stderr)
    );
    for f in four(&out1) {
        let meta = fs::metadata(&f).unwrap_or_else(|e| panic!("missing {f:?}: {e}"));
        assert!(meta.len() > 0, "{f:?} must be non-empty");
    }

    // --- --despeckle 0 == omitting the flag (legal no-op, byte-identical) ----
    let out0 = work.join("d0");
    let none = work.join("dnone");
    let r0 = run(&out0, &["--despeckle", "0"]);
    let rn = run(&none, &[]);
    assert!(r0.status.success(), "generate --despeckle 0 must exit 0");
    assert!(
        rn.status.success(),
        "generate without --despeckle must exit 0"
    );
    for (a, b) in four(&out0).iter().zip(four(&none).iter()) {
        assert_eq!(
            fs::read(a).unwrap(),
            fs::read(b).unwrap(),
            "--despeckle 0 must be a no-op: {a:?} differs from {b:?}"
        );
    }

    // --- --despeckle x (non-u32): clap rejects, exit 2, no output dir --------
    let bad_out = work.join("dbad");
    let bad = run(&bad_out, &["--despeckle", "x"]);
    assert_eq!(
        bad.status.code(),
        Some(2),
        "non-u32 --despeckle must exit 2 (not panic=101 / signal=None); got {:?}",
        bad.status.code()
    );
    assert!(
        !bad.status.success(),
        "non-u32 --despeckle must be non-success"
    );
    assert!(
        !bad_out.exists(),
        "non-u32 --despeckle must not create output path"
    );

    let _ = fs::remove_dir_all(&work);
}

/// 6.8 — filesystem failures exit non-zero (never panic) with path context.
/// Two representative cases (write-side: --output is an existing plain file;
/// read-side: --input does not exist); parent-not-writable / disk-full share
/// the same anyhow `.context` catch-all path and are covered by argument.
#[test]
fn cli_fs_failures_nonzero_not_panic() {
    let work = scratch("cli-fs");
    let input = asset("samples/gradient.png");
    let good_palette = asset("palettes/artkal_s.json");

    // --- write-side: --output is an existing *plain file* (create_dir_all fails)
    let file_as_output = work.join("not_a_dir");
    fs::write(&file_as_output, b"i am a regular file").expect("write blocking file");
    let out = Command::new(BIN)
        .args(["generate", "--input"])
        .arg(&input)
        .arg("--palette")
        .arg(&good_palette)
        .args(["--width", "16", "--height", "20", "--output"])
        .arg(&file_as_output)
        .output()
        .expect("run generate (output is a file)");
    assert!(
        !out.status.success(),
        "generate with file --output must exit non-zero (not silently succeed)"
    );
    // business failure must exit 1 (anyhow `main` returns 1 on Err); asserting
    // == Some(1) catches a panic (unwind → 101) and a signal/abort (None) too —
    // is_some() alone would pass on a panicking binary (review M6-code-R1/Codex+RC).
    assert_eq!(
        out.status.code(),
        Some(1),
        "generate with file --output must exit 1 (not panic=101 / signal=None)"
    );
    let out_stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        out_stderr.contains("not_a_dir"),
        "stderr must name the offending output path; got: {out_stderr:?}"
    );

    // --- read-side: --input does not exist ---------------------------------
    let missing = work.join("does_not_exist.png");
    let inp = Command::new(BIN)
        .args(["generate", "--input"])
        .arg(&missing)
        .arg("--palette")
        .arg(&good_palette)
        .args(["--width", "16", "--height", "20", "--output"])
        .arg(work.join("out"))
        .output()
        .expect("run generate (missing input)");
    assert!(
        !inp.status.success(),
        "generate with missing --input must exit non-zero"
    );
    assert_eq!(
        inp.status.code(),
        Some(1),
        "generate with missing --input must exit 1 (not panic=101 / signal=None)"
    );
    let inp_stderr = String::from_utf8_lossy(&inp.stderr);
    assert!(
        inp_stderr.contains("does_not_exist.png"),
        "stderr must name the missing input path; got: {inp_stderr:?}"
    );

    let _ = fs::remove_dir_all(&work);
}
