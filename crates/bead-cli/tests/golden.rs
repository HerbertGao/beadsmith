//! Library-level golden tests (M7, tasks §2.1–§2.6).
//!
//! These freeze the four engine outputs for a fixed input by calling
//! `bead_core::generate_pattern` and comparing the four product bytes
//! (`pattern.json` / `summary.txt` / `preview.png` / `grid.png`) against the
//! committed golden files under the repo-root `tests/golden/`. The CLI
//! (`main.rs`) writes those same four byte blocks verbatim via `fs::write`
//! (no transformation), so freezing the library output transitively covers the
//! CLI contract (CLAUDE rule 5) without driving a subprocess.
//!
//! Cross-platform posture (design D1): byte golden is asserted **only on the
//! canonical platform (arm64 Linux; CI reference ubuntu-24.04-arm)** because the
//! default `Lanczos3` resize weights go through `f32::sin` and the default
//! `OklabMatcher` (and `LabMatcher`) goes through `cbrt`/`powf`, whose ULP is
//! not guaranteed identical across architectures / libm implementations. The other
//! platforms (x86-64 Linux, macOS, Windows) run the same tests but only assert
//! float-independent structural invariants (`golden_structure_*`).
//!
//! Zero new dependencies: this file uses only `bead-core` + std. It does **not**
//! parse JSON — structural assertions read the `GenerateResult` struct directly
//! and do byte-position checks on the `pattern_json` string; PNGs are decoded
//! via `bead_core::decode_image`. So `bead-cli` gains neither `serde_json` nor
//! `image`. `pattern_json` lives at `bead_core::pipeline::pattern_json` (not the
//! crate root), imported by full path as in `main.rs:9`.

use std::fs;
use std::path::{Path, PathBuf};

use bead_core::pipeline::pattern_json;
use bead_core::{
    decode_image, generate_pattern, load_palette, total_beads, GenerateOptions, GeneratorKind,
    MatcherKind,
};

/// A repo-root-relative path, resolved from the package manifest dir
/// (`CARGO_MANIFEST_DIR` == `crates/bead-cli`; two levels up is the repo root) —
/// the same convention as `cli.rs` and M6's e2e tests.
fn repo_root(rel: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(rel)
}

/// Run the engine on the fixed golden input and return the result.
///
/// Fixed input (design D4): committed 32×40 `samples/gradient.png` resized **to**
/// 16×20 (`16:20 == 32:40`, so the center crop is a no-op — the fixture is not
/// regenerated), `palettes/artkal_s.json`, and `GenerateOptions { width: 16,
/// height: 20, .. }` whose `Default` supplies the real `Lanczos3` filter,
/// `cell_size 10`, and matcher `Oklab` (no CLI flag, design D4).
fn fixed_result() -> bead_core::GenerateResult {
    let img_bytes = fs::read(repo_root("samples/gradient.png")).expect("read samples/gradient.png");
    let pal_bytes =
        fs::read(repo_root("palettes/artkal_s.json")).expect("read palettes/artkal_s.json");
    let palette = load_palette(&pal_bytes).expect("load_palette(artkal_s)");
    let opts = GenerateOptions {
        width: 16,
        height: 20,
        ..Default::default()
    };
    assert_eq!(opts.matcher, MatcherKind::Oklab);
    generate_pattern(&img_bytes, &palette, &opts).expect("generate_pattern on fixed input")
}

/// A small hand-built palette (inline JSON, loaded via `load_palette` — no `image`
/// dev-dep needed) whose beads span the **dark** gamut of `samples/gradient.png`
/// (its channels only reach ~[31, 39, 70], so `artkal_s` would snap the entire
/// board to one near-black bead — a useless golden). These five dark beads spread
/// across that gamut so the Gerstner superpixel centroids distribute across
/// several beads, making the frozen output sensitive to the determinism mechanics
/// (accumulation order, tie-break, drift) the golden must guard.
const GERSTNER_PALETTE_JSON: &str = r##"{
    "brand": "GerstnerGolden",
    "colors": [
        { "code": "K",  "name": "Ink",    "rgb": "#000000" },
        { "code": "R",  "name": "Rust",   "rgb": "#1E0520" },
        { "code": "G",  "name": "Moss",   "rgb": "#052628" },
        { "code": "B",  "name": "Bright", "rgb": "#1E2644" },
        { "code": "M",  "name": "Mud",    "rgb": "#0F1423" }
    ]
}"##;

/// Gerstner golden fixture (tasks §7.4). Reuses the committed **synthetic**
/// `samples/gradient.png` (a formula-built gradient, **not** a binary photo — the
/// spec bans committed photo fixtures, which this is not) with a fixed small dark
/// palette, `generator == Gerstner`, and defaults for everything else. 16×20 keeps
/// `target ≤ source` (source 32×40, so `S_x = S_y = 2 ≥ 1`, satisfying Gerstner's
/// upsampling guard) and reuses the 4:5 no-op crop. `bead-cli` has no `image`
/// dev-dep (see the module header), so the source can't be hand-built here; the
/// gradient is a synthetic stand-in that still forces seed drift across the T
/// rounds (distinct per-pixel colors), which is what the golden must exercise.
fn gerstner_palette() -> bead_core::Palette {
    load_palette(GERSTNER_PALETTE_JSON.as_bytes()).expect("load_palette(GERSTNER_PALETTE_JSON)")
}

fn gerstner_opts() -> GenerateOptions {
    GenerateOptions {
        width: 16,
        height: 20,
        generator: GeneratorKind::Gerstner,
        ..Default::default()
    }
}

fn gerstner_result() -> bead_core::GenerateResult {
    let img_bytes = fs::read(repo_root("samples/gradient.png")).expect("read samples/gradient.png");
    let palette = gerstner_palette();
    let opts = gerstner_opts();
    assert_eq!(opts.matcher, MatcherKind::Oklab);
    generate_pattern(&img_bytes, &palette, &opts).expect("Gerstner generate_pattern on fixed input")
}

/// Pure comparison — **does not read disk, does not consult `BLESS`** (design
/// D6). On equality it returns; on mismatch it writes `actual` to
/// `tests/golden/<name>.actual` (gitignored side-output) and panics with a
/// clear message naming `<name>` and the reblessing command.
fn compare_or_panic(name: &str, expected: &[u8], actual: &[u8]) {
    if expected == actual {
        return;
    }
    let actual_path = repo_root("tests/golden").join(format!("{name}.actual"));
    // The side-output is a diagnostic aid, not a gate: if the write fails, still
    // panic with the real mismatch message (keeps `#[should_panic(expected=…)]`
    // robust and never masks the actual golden diff behind a write error).
    if let Err(e) = fs::write(&actual_path, actual) {
        eprintln!("warning: could not write side-output {actual_path:?}: {e}");
    }
    panic!(
        "golden mismatch for `{name}`: bytes differ from `tests/golden/{name}` \
         (expected {} bytes, actual {} bytes). Inspect by diffing `tests/golden/{name}` vs \
         `tests/golden/{name}.actual`. If this change is intentional, regenerate the golden \
         files with `BLESS=1 cargo test -p bead-cli --test golden` and review with \
         `git diff tests/golden/`.",
        expected.len(),
        actual.len()
    );
}

/// Canonical-gated golden assertion (design D6 / D10.3).
///
/// `CANONICAL` is true only on arm64 Linux (CI reference ubuntu-24.04-arm). When `BLESS`
/// is set it regenerates the golden — but only on canonical (the `assert!`
/// refuses elsewhere, so non-canonical bytes can never be committed). Without
/// `BLESS`, byte comparison runs **only on canonical**; non-canonical platforms
/// early-return (their coverage is `golden_structure_all_platforms`).
fn assert_golden(name: &str, actual: &[u8]) {
    const CANONICAL: bool = cfg!(target_os = "linux") && cfg!(target_arch = "aarch64");

    if std::env::var("BLESS").is_ok() {
        // CANONICAL is a compile-time const, so clippy flags the assert as
        // constant — but a const-true/false guard is exactly the intent (refuse
        // to write non-canonical bytes), so the runtime `assert!` form is kept.
        #[allow(clippy::assertions_on_constants)]
        {
            assert!(
                CANONICAL,
                "BLESS is canonical-only (arm64 Linux): refusing to write non-canonical bytes for `{name}`"
            );
        }
        let path = repo_root("tests/golden").join(name);
        fs::write(&path, actual).unwrap_or_else(|e| panic!("failed to bless {path:?}: {e}"));
        return;
    }

    if !CANONICAL {
        return;
    }

    let path = repo_root("tests/golden").join(name);
    let golden = fs::read(&path).unwrap_or_else(|e| {
        panic!(
            "failed to read golden {path:?}: {e}. If golden files are missing, regenerate them on \
             the canonical (arm64 Linux) platform with `BLESS=1 cargo test -p bead-cli --test golden`."
        )
    });
    compare_or_panic(name, &golden, actual);
}

/// §2.4a — canonical byte freeze (design D10.1). On non-canonical platforms each
/// `assert_golden` call is a self-gated no-op; on canonical it asserts each of
/// the four products is byte-identical to its committed golden.
#[test]
fn golden_matches_canonical() {
    let result = fixed_result();
    assert_golden("pattern.json", pattern_json(&result).as_bytes());
    assert_golden("summary.txt", result.summary.as_bytes());
    assert_golden("preview.png", &result.preview_png);
    assert_golden("grid.png", &result.grid_png);
}

/// §2.4b — float-independent structural invariants (design D10.2). Runs on all
/// three platforms with zero new dependencies (struct + `decode_image` + std
/// string ops only).
#[test]
fn golden_structure_all_platforms() {
    let result = fixed_result();

    // ① PNG: both decode; preview is exactly 160×200; grid matches its geometry
    //    formula. For 16×20, cell_size 10: scale = max(1, 10/5) = 2, pad = 2,
    //    STEP = 10. has_col (16>=10) and has_row (20>=10) are both true.
    //    max_row_label = (20/10)*10 = 20 -> row_digits = 2 ; num_w(2) = 2*4*2-2 = 14.
    //    margin_top = 7*scale = 14 ; margin_left = num_w(2) + 2*pad = 14 + 4 = 18.
    //    out_w = 18 + 16*10 = 178 ; out_h = 14 + 20*10 = 214.
    let preview = decode_image(&result.preview_png).expect("preview.png must decode");
    assert_eq!(
        preview.dimensions(),
        (160, 200),
        "preview.png must be 160×200 (16·10 × 20·10)"
    );
    let grid = decode_image(&result.grid_png).expect("grid.png must decode");
    assert_eq!(
        grid.dimensions(),
        (178, 214),
        "grid.png must be 178×214 (margin_left 18 + 16·10, margin_top 14 + 20·10)"
    );

    // ② pattern.json: assert directly on the source-of-truth struct (rule 3),
    //    then verify the serialized key order via byte-position monotonicity.
    let pal_bytes =
        fs::read(repo_root("palettes/artkal_s.json")).expect("read palettes/artkal_s.json");
    let palette = load_palette(&pal_bytes).expect("load_palette(artkal_s)");

    assert_eq!(result.pattern.width, 16, "pattern width");
    assert_eq!(result.pattern.height, 20, "pattern height");
    assert_eq!(result.pattern.cells.len(), 320, "cells.len() == 16·20");
    assert_eq!(total_beads(&result.pattern), 320, "total_beads == 320");
    for &cell in &result.pattern.cells {
        assert!(
            (cell as usize) < palette.colors.len(),
            "every cell index must be < palette.colors.len() ({})",
            palette.colors.len()
        );
    }

    // key order: brand < width < height < cells < total < stats (byte positions
    // in the serialized string must be strictly increasing).
    let json = pattern_json(&result);
    let pos = |key: &str| -> usize {
        json.find(key)
            .unwrap_or_else(|| panic!("pattern.json must contain key {key:?}"))
    };
    let brand = pos("\"brand\"");
    let width = pos("\"width\"");
    let height = pos("\"height\"");
    let cells = pos("\"cells\"");
    let total = pos("\"total\"");
    let stats = pos("\"stats\"");
    assert!(
        brand < width && width < height && height < cells && cells < total && total < stats,
        "pattern.json key order must be brand < width < height < cells < total < stats; \
         got positions brand={brand} width={width} height={height} cells={cells} \
         total={total} stats={stats}"
    );

    // ③ summary.txt: header structure + per-color body counts sum to 320. The
    //    body starts after the FIRST blank line (header is the 4 lines before
    //    it). Only sum the body color lines — the header's `Total Beads: 320`
    //    also matches `rsplit_once(": ")` and would double-count to 640.
    let summary = &result.summary;
    let mut lines = summary.lines();
    assert_eq!(
        lines.next(),
        Some("Bead Pattern Summary"),
        "summary first line"
    );
    assert_eq!(lines.next(), Some("Size: 16 x 20"), "summary size line");
    assert_eq!(lines.next(), Some("Total Beads: 320"), "summary total line");
    assert_eq!(
        lines.next(),
        Some("Palette: Artkal S"),
        "summary palette line"
    );
    assert_eq!(lines.next(), Some(""), "summary blank-line separator");

    // body = everything after the first blank line; sum each color line's count.
    let (_, body) = summary
        .split_once("\n\n")
        .expect("summary must have a blank-line separator between header and body");
    let mut body_sum: u32 = 0;
    for line in body.lines() {
        let (_, count) = line
            .rsplit_once(": ")
            .unwrap_or_else(|| panic!("body color line must contain \": \"; got {line:?}"));
        body_sum += count
            .parse::<u32>()
            .unwrap_or_else(|e| panic!("body color line count must parse as u32 ({line:?}): {e}"));
    }
    assert_eq!(
        body_sum, 320,
        "Σ of body color-line counts must equal 320 (header excluded)"
    );

    // ④ grid.png: (0,0) is the top-left margin -> BG; the bold separator pixel at
    //    (118,19) is BOLD. Geometry (margin_left 18, margin_top 14, cell 10):
    //    vertical bold line at bx=10 sits at x = 18 + 10·10 = 118; y = 14 + 5 = 19
    //    avoids horizontal boundaries (14, 24, …) and the label band. Both
    //    coordinates land on STEP-multiple boundaries / pure-integer geometry, so
    //    they are float-independent and stable across platforms.
    const BG: [u8; 3] = [255, 255, 255];
    const BOLD: [u8; 3] = [120, 120, 120];
    assert_eq!(grid.get_pixel(0, 0).0, BG, "grid (0,0) must be BG");
    assert_eq!(
        grid.get_pixel(118, 19).0,
        BOLD,
        "grid (118,19) must be the BOLD separator line"
    );
}

/// §7.4 — Gerstner canonical byte freeze. Freezes `pattern.json` (which carries
/// both `cells` and the derived `stats`) for the Gerstner path, gated exactly like
/// the Staged golden: on canonical (arm64 Linux) it asserts byte equality against
/// `tests/golden/gerstner_pattern.json`; elsewhere it is a self-gated no-op (the
/// structural coverage below runs everywhere). The canonical golden is blessed on
/// arm64 with `BLESS=1 cargo test -p bead-cli --test golden` (canonical-only — the
/// f32 SLIC/Oklab path is same-machine deterministic, not cross-arch bit-exact),
/// like every other committed golden. This freeze is what catches structural
/// regressions (accumulation-order change, tie-flip) that a same-machine repeat
/// test cannot — a self-consistent recompute stays self-consistent even if the
/// output silently shifts.
#[test]
fn gerstner_golden_matches_canonical() {
    let result = gerstner_result();
    assert_golden("gerstner_pattern.json", pattern_json(&result).as_bytes());
}

/// §7.4 — Gerstner float-independent structural invariants (all platforms). Guards
/// the shape/legality of the Gerstner board and, critically, that the Gerstner
/// front end actually ran (its `cells` differ from the Staged path at the same
/// size) — a cross-machine guard that does not depend on f32 ULP.
#[test]
fn gerstner_golden_structure_all_platforms() {
    let result = gerstner_result();
    let palette = gerstner_palette();

    // shape: full board, every cell a legal palette index (never off-board / never
    // an invented intermediate color).
    assert_eq!(result.pattern.width, 16, "Gerstner pattern width");
    assert_eq!(result.pattern.height, 20, "Gerstner pattern height");
    assert_eq!(result.pattern.cells.len(), 320, "cells.len() == 16·20");
    assert_eq!(total_beads(&result.pattern), 320, "total_beads == 320");
    for &cell in &result.pattern.cells {
        assert!(
            (cell as usize) < palette.colors.len(),
            "every Gerstner cell index must be < palette.colors.len() ({})",
            palette.colors.len()
        );
    }

    // not degenerate: the gradient source + spread dark palette must resolve to
    // more than one bead color (a single-color board would signal the
    // superpixel/snap collapsed).
    let distinct = {
        let mut v = result.pattern.cells.clone();
        v.sort_unstable();
        v.dedup();
        v.len()
    };
    assert!(
        distinct > 1,
        "Gerstner board must use >1 distinct bead color on a gradient, got {distinct}"
    );

    // the Gerstner branch really ran: its board differs from the Staged board at
    // the same size / palette / options. Float-independent (compares index Vecs).
    let img_bytes = fs::read(repo_root("samples/gradient.png")).expect("read samples/gradient.png");
    let staged_opts = GenerateOptions {
        generator: GeneratorKind::Staged,
        ..gerstner_opts()
    };
    let staged = generate_pattern(&img_bytes, &palette, &staged_opts).expect("staged run");
    assert_ne!(
        result.pattern.cells, staged.pattern.cells,
        "Gerstner cells must differ from the Staged path (proving the branch executed)"
    );
}

/// §2.6 — fail-loudly self-test (design D10.6). Calls the pure `compare_or_panic`
/// **directly** (never via `assert_golden`/`BLESS`/the write-golden branch), so
/// it panics on every platform and under `BLESS=1` alike. The `selftest.actual`
/// it writes is gitignored and never touches a committed golden.
#[test]
#[should_panic(expected = "golden mismatch for `selftest`")]
fn compare_or_panic_detects_mismatch() {
    compare_or_panic("selftest", b"AAAA", b"BBBB");
}
