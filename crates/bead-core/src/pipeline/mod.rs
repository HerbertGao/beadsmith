//! The single **generation/orchestration** entry point for external callers
//! (CLI, FFI) — CLAUDE rule 4. [`generate_pattern`] faithfully chains the
//! existing primitives (`image_to_grid` → matcher → statistics → renderer)
//! into one call; it introduces **no new algorithm**. The supporting helpers
//! ([`pattern_json`], and the crate's `load_palette`) stay public so frontends
//! can compose the file-shaped output.
//!
//! A single `Palette` value is threaded through the matcher, statistics, and
//! renderer (the single-`Palette` invariant, design D1) — eliminating the
//! M4-D4 hazard where stats and pixels could be derived from different
//! palettes.
//!
//! `bead-core` has **no filesystem** (CLAUDE rule 1): the caller reads the
//! image bytes and the `&Palette` and passes them in; this module returns
//! in-memory data (pattern, stats, summary, PNG bytes) and a serialized JSON
//! `String`. Persisting any of it is the frontend's job.

use crate::image::{image_to_grid, ResizeOptions};
use crate::matcher::{match_pattern, LabMatcher};
use crate::models::{BeadPattern, ColorStat};
use crate::palette::Palette;
use crate::quantizer::{MedianCutQuantizer, Quantizer};
use crate::renderer::{render_grid, render_preview, RenderOptions};
use crate::statistics::{count_colors, generate_summary, total_beads};
use crate::BeadError;

/// Options for [`generate_pattern`]: target grid dimensions plus the resize,
/// optional color-reduction, and render sub-options. `width` / `height` are the
/// bead-grid size in cells.
///
/// Plain `Default` (no `#[non_exhaustive]`, design D3): callers construct it
/// with struct-update syntax, e.g. `GenerateOptions { width, height,
/// ..Default::default() }`. `max_colors` defaults to `None` (no color
/// reduction — the grid passes through to matching verbatim); `Some(n)` runs
/// [`MedianCutQuantizer`] after `image_to_grid` and before `match_pattern`,
/// reducing the grid to ≤`n` representative colors. `Some(0)` is rejected
/// inside `generate_pattern` by `MedianCutQuantizer::new` (reuses
/// `BeadError::InvalidImage`, no new variant), not by `GenerateOptions` itself.
// ponytail: Default 的 0×0 非「能跑配置」、是 ..default() 填充便利；维度非法由 image_to_grid 既有 0-守卫干净返 Err、不 panic。
// derive(Default) 即产 width:0/height:0/resize:Default(Lanczos3)/render:Default(cell_size 10)/max_colors:None，恰是 D3 钉的默认值——用 derive、不手写 impl（更地道、无需 clippy allow）。
#[derive(Debug, Clone, PartialEq, Default)]
pub struct GenerateOptions {
    /// Target grid width in cells.
    pub width: u32,
    /// Target grid height in cells.
    pub height: u32,
    /// How the source image is resized into the grid (filter choice).
    pub resize: ResizeOptions,
    /// How the pattern is rendered to PNG (cell size, shape).
    pub render: RenderOptions,
    /// Optional Phase-2 color reduction: `Some(n)` reduces the grid to ≤`n`
    /// representative colors via [`MedianCutQuantizer`] before color matching;
    /// `None` (the default) skips the stage so the grid reaches matching
    /// verbatim (byte-identical to the pre-reduction pipeline).
    pub max_colors: Option<u32>,
}

/// The full result of [`generate_pattern`]: the source-of-truth pattern, its
/// derived statistics and summary, the palette `brand`, and the two rendered
/// PNG byte buffers.
///
/// Deliberately does **not** derive `Clone`: no consumer needs to clone the
/// whole result (which carries two PNG byte buffers); the CLI writes the
/// fields out and drops it. YAGNI — if M8 needs `Clone` it can be added
/// non-breakingly then.
#[derive(Debug)]
pub struct GenerateResult {
    /// The color-matched pattern — the source of truth.
    pub pattern: BeadPattern,
    /// Per-color statistics derived from `pattern` (count-desc, lowest-index tie).
    pub stats: Vec<ColorStat>,
    /// The directly-copyable INIT "Summary Format" text.
    pub summary: String,
    // ponytail: brand 入结果（= palette.brand 克隆）→ pattern_json 只取 &GenerateResult、不单收 palette，杜绝配错（D2/M6-R3-Codex）；代价一次 String 克隆，相对两块 PNG 可忽略
    /// The palette's `brand` label (cloned from `palette.brand`).
    pub brand: String,
    /// Rendered preview PNG bytes (the finished bead-art look).
    pub preview_png: Vec<u8>,
    /// Rendered grid PNG bytes (row/column numbers + grid lines).
    pub grid_png: Vec<u8>,
}

/// Generate a complete bead pattern from image bytes + a palette, in one call.
///
/// Faithfully chains the existing primitives in a fixed order, threading the
/// **same** `palette` through matcher, statistics, and renderer (the
/// single-`Palette` invariant, design D1). The fixed order is:
/// `image_to_grid` → optional color reduction (when `opts.max_colors ==
/// Some(n)`, `MedianCutQuantizer::new(n)?.quantize(&grid)`; when `None`, the
/// stage is skipped and the grid is passed through verbatim — byte-identical
/// to the pre-reduction pipeline) → `LabMatcher::new(palette)` →
/// `match_pattern` → `count_colors` / `generate_summary` → `render_preview` /
/// `render_grid`. Errors from any stage propagate via `?` as their existing
/// [`BeadError`] variant — no new variant is introduced (design D7). In
/// particular `opts.max_colors == Some(0)` surfaces `MedianCutQuantizer::new`'s
/// `Err(InvalidImage { reason })` (reason mentions "max_colors"), before any
/// matching or rendering.
pub fn generate_pattern(
    image_bytes: &[u8],
    palette: &Palette,
    opts: &GenerateOptions,
) -> Result<GenerateResult, BeadError> {
    let grid = image_to_grid(image_bytes, opts.width, opts.height, &opts.resize)?;
    let grid = match opts.max_colors {
        Some(n) => MedianCutQuantizer::new(n)?.quantize(&grid),
        None => grid,
    };
    let m = LabMatcher::new(palette)?;
    let pattern = match_pattern(&grid, &m);
    let stats = count_colors(&pattern, palette);
    let summary = generate_summary(&pattern, palette);
    let preview_png = render_preview(&pattern, palette, &opts.render)?;
    let grid_png = render_grid(&pattern, palette, &opts.render)?;

    Ok(GenerateResult {
        pattern,
        stats,
        summary,
        brand: palette.brand.clone(),
        preview_png,
        grid_png,
    })
}

/// Private serialization shape for `pattern.json`. Field order fixes the JSON
/// key order: `brand`, then the `flatten`ed `BeadPattern` (`width`, `height`,
/// `cells`), then `total`, then `stats`.
#[derive(serde::Serialize)]
struct PatternFile<'a> {
    brand: &'a str,
    #[serde(flatten)]
    pattern: &'a BeadPattern,
    total: u32,
    stats: &'a [ColorStat],
}

/// Serialize a [`GenerateResult`] into the `pattern.json` body.
///
/// Returns a `String` (not a `Result`): `PatternFile` is pure data (no
/// non-string map keys, no fallible custom `Serialize`), so serialization
/// cannot fail. Takes only `&GenerateResult` — `brand` is read from
/// `result.brand`, so no separate `palette` argument can be misconfigured
/// (D2/M6-R3).
// ponytail: 纯数据序列化不可失败 → 返 String、不引 Result/新错误变体（BeadError 无序列化变体、不误用 PaletteParse）；flatten 承载顶层 brand/total、不立 BeadPattern 的 DTO 镜像（D4/D5）
// ponytail: 前向约束——PatternFile 须保持纯数据；任一可达字段加自定义 Serialize/map 键会使 .expect() 可达、panic（review M6-R2/CR nit）
pub fn pattern_json(result: &GenerateResult) -> String {
    serde_json::to_string(&PatternFile {
        brand: &result.brand,
        pattern: &result.pattern,
        total: total_beads(&result.pattern),
        stats: &result.stats,
    })
    .expect("PatternFile 是纯数据（无非字符串 map 键/无会失败的 Serialize），序列化不可失败")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::palette::PaletteColor;

    // ---- test fixtures --------------------------------------------------------

    /// Build a `Palette` from `(code, name, rgb)` triples. Names are load-bearing
    /// for `ColorStat` / summary, so they are explicit; codes must stay unique
    /// (the matcher / stats / serialization invariants assume it).
    fn palette_from(brand: &str, colors: &[(&str, &str, [u8; 3])]) -> Palette {
        Palette {
            brand: brand.to_string(),
            colors: colors
                .iter()
                .map(|(code, name, rgb)| PaletteColor {
                    code: code.to_string(),
                    name: name.to_string(),
                    rgb: *rgb,
                })
                .collect(),
        }
    }

    /// A small deterministic palette with clearly distinguishable colors.
    fn demo_palette() -> Palette {
        palette_from(
            "Test",
            &[
                ("R", "Red", [255, 0, 0]),
                ("G", "Green", [0, 255, 0]),
                ("B", "Blue", [0, 0, 255]),
                ("K", "Black", [0, 0, 0]),
                ("W", "White", [255, 255, 255]),
            ],
        )
    }

    /// Encode a small deterministic RGB image to PNG bytes.
    ///
    /// Uses the M2 gradient-family formula `r = x%256, g = y%256, b = (x+y)%256`
    /// (pure-integer source pixels), encoded with the default PNG settings — the
    /// pipeline decodes it back, so the exact encoder choice is not load-bearing.
    fn demo_png(w: u32, h: u32) -> Vec<u8> {
        let img = ::image::RgbImage::from_fn(w, h, |x, y| {
            ::image::Rgb([(x % 256) as u8, (y % 256) as u8, ((x + y) % 256) as u8])
        });
        let mut buf = std::io::Cursor::new(Vec::new());
        ::image::DynamicImage::ImageRgb8(img)
            .write_to(&mut buf, ::image::ImageFormat::Png)
            .expect("encoding the test PNG must succeed");
        buf.into_inner()
    }

    /// Decode PNG bytes back to an `RgbImage`, panicking if they are not valid.
    fn decode(bytes: &[u8]) -> ::image::RgbImage {
        ::image::load_from_memory(bytes)
            .expect("rendered PNG must decode")
            .to_rgb8()
    }

    fn demo_opts(width: u32, height: u32) -> GenerateOptions {
        GenerateOptions {
            width,
            height,
            ..Default::default()
        }
    }

    // ---- 6.1 ------------------------------------------------------------------

    // generate_pattern faithfully chains the primitives: its result equals
    // calling each primitive individually, with the expected shape (D10.1).
    #[test]
    fn generate_pattern_chains_faithfully() {
        let palette = demo_palette();
        let (w, h) = (16u32, 20u32);
        let bytes = demo_png(32, 40); // 32:40 == 16:20 -> crop is a no-op
        let opts = demo_opts(w, h);

        let result = generate_pattern(&bytes, &palette, &opts).expect("generate must succeed");

        // shape
        assert_eq!(result.pattern.width, w);
        assert_eq!(result.pattern.height, h);
        assert_eq!(result.pattern.cells.len() as u32, w * h);
        assert_eq!(result.brand, palette.brand);

        // Re-run each primitive individually and compare one-for-one — proving the
        // pipeline introduces no difference.
        let grid = image_to_grid(&bytes, w, h, &opts.resize).expect("grid");
        let m = LabMatcher::new(&palette).expect("matcher");
        let expected_pattern = match_pattern(&grid, &m);
        let expected_stats = count_colors(&expected_pattern, &palette);
        let expected_summary = generate_summary(&expected_pattern, &palette);
        let expected_preview =
            render_preview(&expected_pattern, &palette, &opts.render).expect("preview");
        let expected_grid_png =
            render_grid(&expected_pattern, &palette, &opts.render).expect("grid png");

        assert_eq!(result.pattern, expected_pattern);
        assert_eq!(result.stats, expected_stats);
        assert_eq!(result.summary, expected_summary);
        assert_eq!(result.preview_png, expected_preview);
        assert_eq!(result.grid_png, expected_grid_png);

        // stats == count_colors(...) restated explicitly (spec wording).
        assert_eq!(result.stats, count_colors(&result.pattern, &palette));
        assert_eq!(result.summary, generate_summary(&result.pattern, &palette));

        // both PNGs are non-empty and decode to a real image.
        assert!(!result.preview_png.is_empty());
        assert!(!result.grid_png.is_empty());
        let preview_img = decode(&result.preview_png);
        let grid_img = decode(&result.grid_png);
        assert_eq!(
            (preview_img.width(), preview_img.height()),
            (w * opts.render.cell_size, h * opts.render.cell_size)
        );
        // grid png carries margins/labels so it is strictly larger than preview.
        assert!(grid_img.width() >= preview_img.width());
        assert!(grid_img.height() >= preview_img.height());
    }

    // ---- 6.2 ------------------------------------------------------------------

    // Single-Palette invariant: stats' code/name and the rendered pixel rgb come
    // from the SAME palette. ColorStat carries only code/name (no index), so the
    // index is recovered via `palette.colors.iter().position(|c| c.code == code)`
    // (unique-code guarantee) and the rendered rgb at a matching cell is compared
    // against `palette.colors[index].rgb` (D10.2).
    #[test]
    fn single_palette_invariant() {
        let palette = demo_palette();
        let (w, h) = (12u32, 12u32);
        let bytes = demo_png(24, 24);
        let opts = demo_opts(w, h);

        let result = generate_pattern(&bytes, &palette, &opts).expect("generate must succeed");

        let preview = decode(&result.preview_png);
        let cell = opts.render.cell_size;

        // For each ColorStat, recover its palette index from the code, then find a
        // cell that uses that index and assert the rendered pixel == that palette
        // color's rgb. This proves stats' code/name and render's rgb are anchored
        // to the same palette.
        for stat in &result.stats {
            let index = palette
                .colors
                .iter()
                .position(|c| c.code == stat.code)
                .expect("every stat code must exist in the palette (single-palette invariant)");
            // name must also match the same palette entry.
            assert_eq!(palette.colors[index].name, stat.name);

            let expected_rgb = palette.colors[index].rgb;

            // find the first cell that uses this index.
            let cell_pos = result
                .pattern
                .cells
                .iter()
                .position(|&c| c as usize == index)
                .expect("a used color's index must appear in cells");
            let cx = (cell_pos as u32) % w;
            let cy = (cell_pos as u32) / w;
            // sample the center of that cell in the preview (no margins in preview).
            let px = preview
                .get_pixel(cx * cell + cell / 2, cy * cell + cell / 2)
                .0;
            assert_eq!(
                px, expected_rgb,
                "cell ({cx},{cy}) renders {px:?} but stat {:?} maps to palette rgb {expected_rgb:?}",
                stat.code
            );
        }
    }

    // ---- 6.3 ------------------------------------------------------------------

    // pattern.json shape: parse it back (serde_json is in scope in bead-core),
    // assert the key set { brand, width, height, cells, total, stats } without
    // assuming order, the total == cells.len() == Σ stats.count identity, cells
    // is an integer array, each stat has code/name/count; serialization is
    // byte-stable across one/two calls (D10.3).
    #[test]
    fn pattern_json_shape() {
        let palette = demo_palette();
        let (w, h) = (10u32, 10u32);
        let bytes = demo_png(20, 20);
        let opts = demo_opts(w, h);

        let result = generate_pattern(&bytes, &palette, &opts).expect("generate must succeed");
        let json = pattern_json(&result);

        let value: serde_json::Value =
            serde_json::from_str(&json).expect("pattern_json must produce valid JSON");
        let obj = value.as_object().expect("top-level JSON must be an object");

        // key set (presence only; order is not asserted).
        for key in ["brand", "width", "height", "cells", "total", "stats"] {
            assert!(
                obj.contains_key(key),
                "pattern.json must contain key {key:?}"
            );
        }
        // exact key SET: spec pins exactly these 6; a stray serialized field (e.g. a
        // new BeadPattern/PatternFile field) must fail loudly (review M6-code-R1/Codex).
        assert_eq!(
            obj.len(),
            6,
            "pattern.json must have exactly 6 top-level keys, got {:?}",
            obj.keys().collect::<Vec<_>>()
        );

        // brand matches.
        assert_eq!(obj["brand"].as_str(), Some(palette.brand.as_str()));

        // width / height match the pattern.
        assert_eq!(obj["width"].as_u64(), Some(w as u64));
        assert_eq!(obj["height"].as_u64(), Some(h as u64));

        // cells is an integer array.
        let cells = obj["cells"].as_array().expect("cells must be an array");
        assert!(
            cells.iter().all(|c| c.is_u64()),
            "cells must be an array of integers"
        );

        // total == cells.len() == Σ stats.count.
        let total = obj["total"].as_u64().expect("total must be an integer");
        assert_eq!(total, cells.len() as u64, "total must equal cells.len()");
        assert_eq!(total, (w * h) as u64);

        let stats = obj["stats"].as_array().expect("stats must be an array");
        let mut sum: u64 = 0;
        for stat in stats {
            let s = stat.as_object().expect("each stat must be an object");
            assert!(s.contains_key("code"), "stat must have code");
            assert!(s.contains_key("name"), "stat must have name");
            assert!(s.contains_key("count"), "stat must have count");
            assert!(s["code"].is_string());
            assert!(s["name"].is_string());
            sum += s["count"].as_u64().expect("count must be an integer");
        }
        assert_eq!(sum, total, "Σ stats.count must equal total");

        // serialization is byte-stable: same result serializes identically.
        assert_eq!(json, pattern_json(&result));
    }

    // ---- 6.4 ------------------------------------------------------------------

    // models_serialize: ColorStat and BeadPattern serialize successfully with the
    // expected shape (D10.3b — compile-time Serialize + runtime shape).
    #[test]
    fn models_serialize() {
        let stat = ColorStat {
            code: "C01".to_string(),
            name: "Crimson".to_string(),
            count: 7,
        };
        let stat_json = serde_json::to_string(&stat).expect("ColorStat must serialize");
        let stat_val: serde_json::Value = serde_json::from_str(&stat_json).expect("valid JSON");
        let stat_obj = stat_val.as_object().expect("ColorStat -> object");
        assert_eq!(stat_obj["code"].as_str(), Some("C01"));
        assert_eq!(stat_obj["name"].as_str(), Some("Crimson"));
        assert_eq!(stat_obj["count"].as_u64(), Some(7));

        let pattern = BeadPattern {
            width: 2,
            height: 3,
            cells: vec![0, 1, 2, 0, 1, 2],
        };
        let pat_json = serde_json::to_string(&pattern).expect("BeadPattern must serialize");
        let pat_val: serde_json::Value = serde_json::from_str(&pat_json).expect("valid JSON");
        let pat_obj = pat_val.as_object().expect("BeadPattern -> object");
        assert_eq!(pat_obj["width"].as_u64(), Some(2));
        assert_eq!(pat_obj["height"].as_u64(), Some(3));
        let cells = pat_obj["cells"].as_array().expect("cells -> array");
        assert_eq!(cells.len(), 6);
        assert!(cells.iter().all(|c| c.is_u64()));
    }

    // ---- 6.5 ------------------------------------------------------------------

    // Errors pass through unchanged: bad image bytes -> ImageDecode; zero target
    // dimension -> InvalidImage (not panic) with a reason from image_to_grid's
    // own target-dimension guard (proving failure precedes match/render); an
    // invalid palette -> the corresponding BeadError (D10.4).
    #[test]
    fn pipeline_errors_passthrough() {
        let palette = demo_palette();

        // ① bad image bytes -> ImageDecode.
        let err = generate_pattern(b"not an image", &palette, &demo_opts(8, 8))
            .expect_err("garbage bytes must fail to decode");
        assert!(
            matches!(err, BeadError::ImageDecode(_)),
            "expected ImageDecode, got {err:?}"
        );

        // ② width==0 -> InvalidImage, reason from image_to_grid's target guard.
        let bytes = demo_png(16, 16);
        let err = generate_pattern(&bytes, &palette, &demo_opts(0, 8))
            .expect_err("width==0 must be rejected");
        match err {
            BeadError::InvalidImage { reason } => assert!(
                reason.contains("target width"),
                "reason must come from image_to_grid's target guard, got: {reason:?}"
            ),
            other => panic!("expected InvalidImage, got {other:?}"),
        }

        // ② height==0 -> InvalidImage, reason from image_to_grid's target guard.
        let err = generate_pattern(&bytes, &palette, &demo_opts(8, 0))
            .expect_err("height==0 must be rejected");
        match err {
            BeadError::InvalidImage { reason } => assert!(
                reason.contains("target height"),
                "reason must come from image_to_grid's target guard, got: {reason:?}"
            ),
            other => panic!("expected InvalidImage, got {other:?}"),
        }

        // ③ invalid palette (empty colors) -> InvalidPalette via LabMatcher::new.
        let empty_palette = Palette {
            brand: "Empty".to_string(),
            colors: vec![],
        };
        let err = generate_pattern(&bytes, &empty_palette, &demo_opts(8, 8))
            .expect_err("empty palette must be rejected");
        assert!(
            matches!(err, BeadError::InvalidPalette { .. }),
            "expected InvalidPalette, got {err:?}"
        );
    }

    // ---- 6.6 ------------------------------------------------------------------

    // Determinism: the same (bytes, palette, opts) produce equal pattern / stats
    // / summary / brand and byte-identical PNGs across two calls (D10.5).
    #[test]
    fn pipeline_deterministic() {
        let palette = demo_palette();
        let (w, h) = (14u32, 18u32);
        let bytes = demo_png(28, 36);
        let opts = demo_opts(w, h);

        let first = generate_pattern(&bytes, &palette, &opts).expect("first run");
        let second = generate_pattern(&bytes, &palette, &opts).expect("second run");

        assert_eq!(first.pattern, second.pattern);
        assert_eq!(first.stats, second.stats);
        assert_eq!(first.summary, second.summary);
        assert_eq!(first.brand, second.brand);
        assert_eq!(
            first.preview_png, second.preview_png,
            "preview PNG must be byte-equal"
        );
        assert_eq!(
            first.grid_png, second.grid_png,
            "grid PNG must be byte-equal"
        );
    }

    // ---- 3.5 ------------------------------------------------------------------

    // max_colors=None vs. ..Default::default(): both omit the reduction stage,
    // so the GenerateResult is field-equal (default path unchanged — spec
    // "max_colors=None 时默认路径逐字节不变").
    #[test]
    fn max_colors_none_matches_default() {
        let palette = demo_palette();
        let (w, h) = (12u32, 12u32);
        let bytes = demo_png(24, 24);

        let opts_none = GenerateOptions {
            width: w,
            height: h,
            max_colors: None,
            ..Default::default()
        };
        let opts_default = demo_opts(w, h); // max_colors via Default -> None

        let none = generate_pattern(&bytes, &palette, &opts_none).expect("none must succeed");
        let def = generate_pattern(&bytes, &palette, &opts_default).expect("default must succeed");

        assert_eq!(none.pattern, def.pattern);
        assert_eq!(none.stats, def.stats);
        assert_eq!(none.summary, def.summary);
        assert_eq!(none.brand, def.brand);
        assert_eq!(none.preview_png, def.preview_png);
        assert_eq!(none.grid_png, def.grid_png);
    }

    // max_colors == Some(0): generate_pattern returns Err(InvalidImage) with a
    // reason mentioning "max_colors" (surfaced from MedianCutQuantizer::new),
    // and does not panic (spec "max_colors==0 返回确定性 Err 而非 panic").
    #[test]
    fn max_colors_zero_rejected() {
        let palette = demo_palette();
        let bytes = demo_png(16, 16);
        let opts = GenerateOptions {
            width: 8,
            height: 8,
            max_colors: Some(0),
            ..Default::default()
        };
        let err = generate_pattern(&bytes, &palette, &opts)
            .expect_err("max_colors == Some(0) must be rejected");
        match err {
            BeadError::InvalidImage { reason } => assert!(
                reason.contains("max_colors"),
                "reason must mention max_colors, got: {reason:?}"
            ),
            other => panic!("expected InvalidImage, got {other:?}"),
        }
    }

    // max_colors == Some(n) below the grid's distinct-color count: the resulting
    // distinct bead colors (stats.len()) must be ≤ n (upper-bound semantics —
    // spec "Some(n) 小于全色时 stats 的不同色数 ≤ n").
    #[test]
    fn max_colors_bounds_distinct_stats() {
        let palette = demo_palette();
        // Use a grid that is visibly richer than the reduction bound so the
        // test proves the quantizer path is actually exercised.
        let (w, h) = (12u32, 12u32);
        let img = ::image::RgbImage::from_fn(w, h, |x, y| {
            let px = match (x < w / 2, y < h / 2) {
                (true, true) => [255, 0, 0],
                (false, true) => [0, 255, 0],
                (true, false) => [0, 0, 255],
                (false, false) => [255, 255, 255],
            };
            ::image::Rgb(px)
        });
        let mut cursor = std::io::Cursor::new(Vec::new());
        ::image::DynamicImage::ImageRgb8(img)
            .write_to(&mut cursor, ::image::ImageFormat::Png)
            .expect("encoding the test PNG must succeed");
        let bytes = cursor.into_inner();
        let n = 2u32;
        let opts = GenerateOptions {
            width: w,
            height: h,
            max_colors: Some(n),
            ..Default::default()
        };
        let grid = image_to_grid(&bytes, w, h, &opts.resize).expect("grid");
        let raw_distinct = {
            let mut pixels = grid.pixels.clone();
            pixels.sort_unstable();
            pixels.dedup();
            pixels.len()
        };
        assert!(
            raw_distinct > n as usize,
            "fixture must exceed max_colors before quantization, got {raw_distinct}"
        );

        let quantizer = MedianCutQuantizer::new(n).expect("valid quantizer");
        let quantized_grid = quantizer.quantize(&grid);
        let quantized_distinct = {
            let mut pixels = quantized_grid.pixels.clone();
            pixels.sort_unstable();
            pixels.dedup();
            pixels.len()
        };
        assert!(
            quantized_distinct <= n as usize,
            "quantized grid must be reduced to <= {n} colors, got {quantized_distinct}"
        );

        let matcher = LabMatcher::new(&palette).expect("matcher");
        let expected = match_pattern(&quantized_grid, &matcher);
        let baseline = match_pattern(&grid, &matcher);
        assert_ne!(
            expected, baseline,
            "fixture must make the reduction stage observable"
        );

        let result = generate_pattern(&bytes, &palette, &opts).expect("quantized run succeeds");
        assert_eq!(result.pattern, expected);
        assert!(
            result.stats.len() <= n as usize,
            "stats.len() ({}) must be <= max_colors ({n})",
            result.stats.len()
        );
    }
}
