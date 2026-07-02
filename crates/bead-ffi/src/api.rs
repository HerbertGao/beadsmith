//! The single bridge function exposed to Dart, plus the FRB mirrors of the
//! `bead-core` types that cross the boundary.
//!
//! Zero business logic (CLAUDE rule 4): the bridge calls only `load_palette`,
//! `generate_pattern`, and `pattern_json` — never an internal pipeline stage,
//! and never re-assembles the image→match→stats→render flow. The boundary is
//! `width` / `height` **plus** the three optional engine options `max_colors` /
//! `despeckle` / `generator` (the set CLI already exposes and mobile UI needs).
//! `filter` / `cell_size` / `shape` / `matcher` stay closed — they take the
//! engine `Default` exactly as the CLI does. `generator` crosses as an FRB mirror
//! of `bead_core::GeneratorKind`; its Dart→core marshalling is trivial (same
//! nature as the CLI's `From<CliGenerator>`), not bridge logic.

use bead_core::pipeline::pattern_json;
use bead_core::{generate_pattern, load_palette, GenerateOptions};
use flutter_rust_bridge::frb;

// The FRB-generated glue (`frb_generated.rs`) refers to the mirrored core types
// as `crate::api::BeadPattern` / `crate::api::ColorStat`, so they must be public
// at this path. Re-export the real bead-core types (no DTO copy) — the mirror
// structs below describe their field shape to FRB.
pub use bead_core::{BeadPattern, ColorStat, GeneratorKind};

// ---- FRB mirrors of the bead-core types that cross the boundary -------------
//
// `BeadPattern` / `ColorStat` already derive `Clone` in bead-core (verified in
// models/mod.rs), so the mirror needs no core change. We mirror them on the
// bead-ffi side with FRB's `mirror` mechanism so bead-core carries no `#[frb]`
// annotation (CLAUDE rule 1). The mirror struct fields must match the real
// type's fields exactly; FRB uses them to generate the Dart class and the
// marshalling code, then moves the real bead-core values across.

/// FRB mirror of [`bead_core::BeadPattern`]. Fields mirror the real type
/// (`width` / `height` / `cells`) — see `bead-core/src/models/mod.rs`.
#[frb(mirror(BeadPattern))]
pub struct _BeadPattern {
    pub width: u32,
    pub height: u32,
    pub cells: Vec<u16>,
}

/// FRB mirror of [`bead_core::ColorStat`]. Fields mirror the real type
/// (`code` / `name` / `count`).
#[frb(mirror(ColorStat))]
pub struct _ColorStat {
    pub code: String,
    pub name: String,
    pub count: u32,
}

/// FRB mirror of [`bead_core::GeneratorKind`]. Variants mirror the real enum
/// (`Staged` | `Gerstner`) so FRB emits the Dart enum and marshals the Dart
/// value back to the real `bead_core::GeneratorKind` — trivial value conversion
/// (same nature as the CLI's `From<CliGenerator>`), no bridge logic and no
/// `bead-core` change.
#[frb(mirror(GeneratorKind))]
pub enum _GeneratorKind {
    Staged,
    Gerstner,
}

/// The structured result handed back to Dart for one generation call.
///
/// This is bead-ffi's own boundary DTO (not a core type): `GenerateResult` is
/// deliberately non-`Clone` and FRB cannot mirror-move a non-`Clone` owner
/// field-by-field through a single mirror, so the bridge function destructures
/// the owned `GenerateResult` and reassembles its fields here. `pattern` /
/// `stats` ride across as the mirrored structured types (not JSON strings);
/// `pattern_json` is the separately-serialized `pattern.json` body for M9
/// persistence (Dart must not hand-assemble it — D-Output).
#[derive(Debug)]
pub struct GenerateOutput {
    /// The color-matched pattern (mirrored `BeadPattern`).
    pub pattern: BeadPattern,
    /// Per-color statistics (mirrored `ColorStat` list).
    pub stats: Vec<ColorStat>,
    /// The directly-copyable INIT "Summary Format" text.
    pub summary: String,
    /// The palette's `brand` label.
    pub brand: String,
    /// Rendered preview PNG bytes.
    pub preview_png: Vec<u8>,
    /// Rendered grid PNG bytes.
    pub grid_png: Vec<u8>,
    /// The serialized `pattern.json` body (byte-identical to the CLI's).
    pub pattern_json: String,
}

/// Generate a complete bead pattern from image bytes + a palette JSON string.
///
/// The boundary is `width` / `height` plus three optional engine options
/// (`max_colors` / `despeckle` / `generator`). The bridge:
/// 1. `load_palette(palette_json.as_bytes())` — `load_palette` takes `&[u8]`, so
///    the JSON `String` is passed as its UTF-8 bytes,
/// 2. builds `GenerateOptions { width, height, max_colors, despeckle, generator,
///    ..Default::default() }` — filter/cell_size/shape/matcher stay engine
///    default (Triangle/10/Square/Oklab). When the three widened options are
///    unset (`None`/`None`/`Staged`) this is field-identical to the old
///    `{ width, height, ..Default::default() }`, so the default path is
///    byte-for-byte unchanged,
/// 3. calls `generate_pattern`, then `pattern_json` on the result.
///
/// The three options are forwarded verbatim — the bridge adds no reduction /
/// despeckle / generation algorithm and no validation. `max_colors = Some(0)`
/// is rejected by the engine (`GreedyReducer::new` → `BeadError::InvalidImage`),
/// not here; `despeckle = Some(0)` is a legal no-op (the asymmetry is the
/// engine's, not the bridge's).
///
/// On any failure the `BeadError` is flattened to its `Display` string at the
/// boundary (`.to_string()`) and returned as `Err(String)`; FRB raises it as a
/// Dart exception. Structured `BeadError` never crosses — its `PaletteParse`
/// (`serde_json::Error`) / `ImageDecode` / `ImageEncode` (`::image::ImageError`)
/// payloads are external types FRB cannot marshal (D-Errors).
pub fn generate(
    image_bytes: Vec<u8>,
    palette_json: String,
    width: u32,
    height: u32,
    max_colors: Option<u32>,
    despeckle: Option<u32>,
    generator: GeneratorKind,
) -> Result<GenerateOutput, String> {
    generate_inner(
        &image_bytes,
        &palette_json,
        width,
        height,
        max_colors,
        despeckle,
        generator,
    )
    .map_err(|e| e.to_string())
}

/// Private inner bridge that preserves the structured `BeadError`.
///
/// Split out so the in-crate `#[test]`s can assert the precise `BeadError`
/// variant for each bad input (tasks 3.3 layer ①), while the exported `generate`
/// flattens it to the `Display` string (layer ②). Calls only the three existing
/// public bead-core entry points — no re-orchestration (CLAUDE rule 4).
fn generate_inner(
    image_bytes: &[u8],
    palette_json: &str,
    width: u32,
    height: u32,
    max_colors: Option<u32>,
    despeckle: Option<u32>,
    generator: GeneratorKind,
) -> Result<GenerateOutput, bead_core::BeadError> {
    let palette = load_palette(palette_json.as_bytes())?;
    let opts = GenerateOptions {
        width,
        height,
        max_colors,
        despeckle,
        generator,
        ..Default::default()
    };
    let result = generate_pattern(image_bytes, &palette, &opts)?;
    let pattern_json = pattern_json(&result);

    // GenerateResult is non-Clone: move its fields out into our boundary DTO.
    let bead_core::GenerateResult {
        pattern,
        stats,
        summary,
        brand,
        preview_png,
        grid_png,
    } = result;

    Ok(GenerateOutput {
        pattern,
        stats,
        summary,
        brand,
        preview_png,
        grid_png,
        pattern_json,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use bead_core::BeadError;

    /// A minimal valid palette JSON (two distinct colors).
    const VALID_PALETTE: &str = r##"{
        "brand": "Test",
        "colors": [
            { "code": "R", "name": "Red",   "rgb": "#FF0000" },
            { "code": "B", "name": "Blue",  "rgb": "#0000FF" }
        ]
    }"##;

    /// A small valid PNG (gradient family), encoded in-memory.
    fn valid_png(w: u32, h: u32) -> Vec<u8> {
        let img = ::image::RgbImage::from_fn(w, h, |x, y| {
            ::image::Rgb([(x % 256) as u8, (y % 256) as u8, ((x + y) % 256) as u8])
        });
        let mut buf = std::io::Cursor::new(Vec::new());
        ::image::DynamicImage::ImageRgb8(img)
            .write_to(&mut buf, ::image::ImageFormat::Png)
            .expect("test PNG must encode");
        buf.into_inner()
    }

    /// Default-path bridge call: the three widened options unset
    /// (`None` / `None` / `Staged`). Keeps the pre-widening tests reading as
    /// before and documents "default path" at each call site.
    fn gen_default(
        image: &[u8],
        palette: &str,
        w: u32,
        h: u32,
    ) -> Result<GenerateOutput, BeadError> {
        generate_inner(image, palette, w, h, None, None, GeneratorKind::Staged)
    }

    // ---- happy path: the bridge returns all products -----------------------

    #[test]
    fn generate_returns_all_products() {
        let png = valid_png(32, 40);
        let out = gen_default(&png, VALID_PALETTE, 16, 20).expect("generation must succeed");

        assert_eq!(out.pattern.width, 16);
        assert_eq!(out.pattern.height, 20);
        assert_eq!(out.pattern.cells.len(), 16 * 20);
        assert_eq!(out.brand, "Test");
        assert!(!out.stats.is_empty());
        assert!(!out.summary.is_empty());
        assert!(!out.preview_png.is_empty());
        assert!(!out.grid_png.is_empty());
        // pattern_json parses and echoes the structured fields.
        let v: serde_json::Value = serde_json::from_str(&out.pattern_json).expect("valid JSON");
        assert_eq!(v["width"].as_u64(), Some(16));
        assert_eq!(v["height"].as_u64(), Some(20));
        assert_eq!(v["cells"].as_array().map(|a| a.len()), Some(16 * 20));
    }

    // ---- each bad input → the expected BeadError variant -------------------
    //
    // The exported `generate` flattens these to `err.to_string()` (asserted in
    // `display_flattening_matches_to_string` below); these tests pin the variant
    // mapping the design records (D-Errors).

    #[test]
    fn undecodable_image_yields_image_decode() {
        let err = gen_default(b"not an image", VALID_PALETTE, 8, 8)
            .expect_err("garbage image bytes must fail");
        assert!(
            matches!(err, BeadError::ImageDecode(_)),
            "expected ImageDecode, got {err:?}"
        );
    }

    #[test]
    fn empty_image_bytes_yields_image_decode() {
        let err = gen_default(b"", VALID_PALETTE, 8, 8).expect_err("empty image bytes must fail");
        assert!(
            matches!(err, BeadError::ImageDecode(_)),
            "expected ImageDecode, got {err:?}"
        );
    }

    #[test]
    fn malformed_palette_json_yields_palette_parse() {
        let png = valid_png(16, 16);
        let err = gen_default(&png, "{ not json", 8, 8)
            .expect_err("syntactically broken palette JSON must fail");
        assert!(
            matches!(err, BeadError::PaletteParse(_)),
            "expected PaletteParse, got {err:?}"
        );
    }

    #[test]
    fn empty_colors_palette_yields_invalid_palette() {
        let png = valid_png(16, 16);
        let palette = r##"{ "brand": "Empty", "colors": [] }"##;
        let err = gen_default(&png, palette, 8, 8).expect_err("zero-color palette must fail");
        assert!(
            matches!(err, BeadError::InvalidPalette { .. }),
            "expected InvalidPalette, got {err:?}"
        );
    }

    #[test]
    fn duplicate_code_palette_yields_invalid_palette() {
        let png = valid_png(16, 16);
        let palette = r##"{
            "brand": "Dupe",
            "colors": [
                { "code": "S01", "name": "One", "rgb": "#111111" },
                { "code": "S01", "name": "Two", "rgb": "#222222" }
            ]
        }"##;
        let err = gen_default(&png, palette, 8, 8).expect_err("duplicate code must fail");
        assert!(
            matches!(err, BeadError::InvalidPalette { .. }),
            "expected InvalidPalette, got {err:?}"
        );
    }

    #[test]
    fn malformed_hex_palette_yields_invalid_palette() {
        let png = valid_png(16, 16);
        let palette = r##"{
            "brand": "BadHex",
            "colors": [
                { "code": "C01", "name": "Bad", "rgb": "#00GG00" }
            ]
        }"##;
        let err = gen_default(&png, palette, 8, 8).expect_err("malformed hex must fail");
        assert!(
            matches!(err, BeadError::InvalidPalette { .. }),
            "expected InvalidPalette, got {err:?}"
        );
    }

    #[test]
    fn zero_width_yields_invalid_image() {
        let png = valid_png(16, 16);
        let err = gen_default(&png, VALID_PALETTE, 0, 8).expect_err("width==0 must fail");
        assert!(
            matches!(err, BeadError::InvalidImage { .. }),
            "expected InvalidImage, got {err:?}"
        );
    }

    #[test]
    fn zero_height_yields_invalid_image() {
        let png = valid_png(16, 16);
        let err = gen_default(&png, VALID_PALETTE, 8, 0).expect_err("height==0 must fail");
        assert!(
            matches!(err, BeadError::InvalidImage { .. }),
            "expected InvalidImage, got {err:?}"
        );
    }

    // ---- the exported boundary flattens to to_string() ---------------------
    //
    // The message Dart sees (the `Err(String)` from `generate`) must equal the
    // `BeadError`'s `Display` string — for every bad-input class. This pins the
    // boundary contract independently of the variant mapping above.

    #[test]
    fn display_flattening_matches_to_string() {
        let png = valid_png(16, 16);
        // (image, palette, width, height) cases, paired with the inner error.
        let cases: Vec<(&[u8], &str, u32, u32)> = vec![
            (b"not an image", VALID_PALETTE, 8, 8),
            (b"", VALID_PALETTE, 8, 8),
            (&png, "{ not json", 8, 8),
            (&png, r##"{ "brand": "E", "colors": [] }"##, 8, 8),
            (&png, VALID_PALETTE, 0, 8),
            (&png, VALID_PALETTE, 8, 0),
        ];
        for (img, pal, w, h) in cases {
            let inner = gen_default(img, pal, w, h).expect_err("must fail");
            let exported = generate(
                img.to_vec(),
                pal.to_string(),
                w,
                h,
                None,
                None,
                GeneratorKind::Staged,
            )
            .expect_err("exported boundary must also fail");
            assert_eq!(
                exported,
                inner.to_string(),
                "boundary message must equal BeadError Display string"
            );
        }
    }

    // ---- widened options: forwarding + default-path invariance --------------

    /// A four-color palette so `max_colors` reduction is observable.
    const RICH_PALETTE: &str = r##"{
        "brand": "Rich",
        "colors": [
            { "code": "R", "name": "Red",   "rgb": "#FF0000" },
            { "code": "G", "name": "Green", "rgb": "#00FF00" },
            { "code": "B", "name": "Blue",  "rgb": "#0000FF" },
            { "code": "W", "name": "White", "rgb": "#FFFFFF" }
        ]
    }"##;

    /// A quadrant image (R/G/B/W) — matches ≥4 distinct beads so reduction and
    /// the Gerstner path are actually exercised. Source > target keeps S >= 1.
    fn quadrant_png(w: u32, h: u32) -> Vec<u8> {
        let img = ::image::RgbImage::from_fn(w, h, |x, y| {
            let px = match (x < w / 2, y < h / 2) {
                (true, true) => [255, 0, 0],
                (false, true) => [0, 255, 0],
                (true, false) => [0, 0, 255],
                (false, false) => [255, 255, 255],
            };
            ::image::Rgb(px)
        });
        let mut buf = std::io::Cursor::new(Vec::new());
        ::image::DynamicImage::ImageRgb8(img)
            .write_to(&mut buf, ::image::ImageFormat::Png)
            .expect("test PNG must encode");
        buf.into_inner()
    }

    // ---- three options unset == the pre-widening default path ---------------
    //
    // The widened boundary with `None` / `None` / `Staged` must be field-identical
    // to the old `GenerateOptions { width, height, ..Default::default() }` run
    // through the same core entry points (no default-path regression).
    #[test]
    fn three_options_unset_matches_old_default_boundary() {
        let png = valid_png(32, 40);
        let out = gen_default(&png, VALID_PALETTE, 16, 20).expect("bridge default path");

        // Reference: the exact construction the bridge used before the widening.
        let palette = load_palette(VALID_PALETTE.as_bytes()).expect("palette must load");
        let opts = GenerateOptions {
            width: 16,
            height: 20,
            ..Default::default()
        };
        let reference = generate_pattern(&png, &palette, &opts).expect("reference run");

        assert_eq!(out.pattern, reference.pattern);
        assert_eq!(out.stats, reference.stats);
        assert_eq!(out.summary, reference.summary);
        assert_eq!(out.brand, reference.brand);
        assert_eq!(out.preview_png, reference.preview_png);
        assert_eq!(out.grid_png, reference.grid_png);
        assert_eq!(out.pattern_json, pattern_json(&reference));
    }

    // ---- max_colors is forwarded (changes the color count) ------------------
    #[test]
    fn max_colors_forwarded_changes_stats_count() {
        let png = quadrant_png(24, 24);
        let unset = generate_inner(
            &png,
            RICH_PALETTE,
            12,
            12,
            None,
            None,
            GeneratorKind::Staged,
        )
        .expect("unset run");
        let capped = generate_inner(
            &png,
            RICH_PALETTE,
            12,
            12,
            Some(2),
            None,
            GeneratorKind::Staged,
        )
        .expect("capped run");

        assert!(
            unset.stats.len() > 2,
            "fixture must match >2 colors so the cap is observable, got {}",
            unset.stats.len()
        );
        assert!(
            capped.stats.len() <= 2,
            "max_colors=Some(2) must cap distinct colors, got {}",
            capped.stats.len()
        );
        assert!(
            capped.stats.len() < unset.stats.len(),
            "max_colors must be forwarded and reduce the color count"
        );
    }

    /// A deterministic noisy multi-color image (LCG-driven vivid hues). Unlike a
    /// clean quadrant or a smooth gradient — both of which color identically under
    /// Staged and Gerstner — its high-frequency detail makes the two generators
    /// disagree (superpixel averaging vs resize+match), so it drives the
    /// Gerstner≠Staged differential.
    fn rich_png(w: u32, h: u32) -> Vec<u8> {
        const BASE: [[u8; 3]; 12] = [
            [234, 0, 0],
            [0, 200, 0],
            [0, 0, 220],
            [255, 210, 0],
            [255, 120, 0],
            [150, 0, 200],
            [0, 200, 200],
            [255, 0, 150],
            [120, 80, 0],
            [0, 120, 60],
            [255, 255, 255],
            [20, 20, 20],
        ];
        let mut state: u64 = 0x1234_5678;
        let img = ::image::RgbImage::from_fn(w, h, |_x, _y| {
            state = (state.wrapping_mul(1103515245).wrapping_add(12345)) & 0x7FFF_FFFF;
            ::image::Rgb(BASE[(state as usize) % BASE.len()])
        });
        let mut buf = std::io::Cursor::new(Vec::new());
        ::image::DynamicImage::ImageRgb8(img)
            .write_to(&mut buf, ::image::ImageFormat::Png)
            .expect("test PNG must encode");
        buf.into_inner()
    }

    // ---- the Gerstner mirror maps and runs the Gerstner path ----------------
    #[test]
    fn gerstner_mirror_forwarded_and_runs() {
        let png = quadrant_png(24, 24); // source > 12x12 target -> Gerstner S >= 1
        let out = generate_inner(
            &png,
            RICH_PALETTE,
            12,
            12,
            None,
            None,
            GeneratorKind::Gerstner,
        )
        .expect("Gerstner path must generate");

        // Structure invariants (f32 path — assert shape, not exact bytes).
        assert_eq!(out.pattern.width, 12);
        assert_eq!(out.pattern.height, 12);
        assert_eq!(out.pattern.cells.len(), 12 * 12);
        assert!(
            out.pattern.cells.iter().all(|&c| (c as usize) < 4),
            "every cell must be a legal palette index"
        );
        assert_eq!(
            out.stats.iter().map(|s| s.count).sum::<u32>(),
            12 * 12,
            "stats counts must sum to the total bead count"
        );

        // Non-vacuity: Gerstner must not silently resolve to the Staged path. The
        // clean R/G/B/W quadrants above happen to color identically under both
        // generators, so a high-frequency noisy image (superpixel averaging vs
        // resize+match diverge on detail) drives the differential — a
        // Gerstner→Staged mis-map (option ignored) would make these equal.
        let rich = rich_png(32, 40);
        let ger = generate_inner(
            &rich,
            RICH_PALETTE,
            16,
            20,
            None,
            None,
            GeneratorKind::Gerstner,
        )
        .expect("Gerstner rich run");
        let staged = generate_inner(
            &rich,
            RICH_PALETTE,
            16,
            20,
            None,
            None,
            GeneratorKind::Staged,
        )
        .expect("Staged rich run");
        assert_ne!(
            ger.pattern, staged.pattern,
            "Gerstner output must differ from Staged on a structured multi-color image"
        );
    }

    /// A speckled image: a solid red field with a few isolated single blue pixels.
    /// Source == target so the staged resize is identity, so each blue pixel stays
    /// an isolated size-1 same-color component (a speck) in the matched pattern —
    /// exactly what `despeckle` merges away.
    fn speckled_png(w: u32, h: u32) -> Vec<u8> {
        let mut img = ::image::RgbImage::from_pixel(w, h, ::image::Rgb([255, 0, 0]));
        for (x, y) in [(2u32, 2u32), (6, 9), (11, 4), (4, 13)] {
            if x < w && y < h {
                img.put_pixel(x, y, ::image::Rgb([0, 0, 255]));
            }
        }
        let mut buf = std::io::Cursor::new(Vec::new());
        ::image::DynamicImage::ImageRgb8(img)
            .write_to(&mut buf, ::image::ImageFormat::Png)
            .expect("test PNG must encode");
        buf.into_inner()
    }

    // ---- despeckle is forwarded (a large window merges specks) ---------------
    //
    // Non-vacuous: `despeckle == Some(0)` equals `None`, so it proves nothing
    // about forwarding. On a speckled pattern (isolated single-bead components),
    // `despeckle == Some(4)` must merge them and change the pattern; if the option
    // were dropped the two runs would be identical.
    #[test]
    fn despeckle_forwarded_merges_specks() {
        let png = speckled_png(16, 16);
        let none = generate_inner(
            &png,
            RICH_PALETTE,
            16,
            16,
            None,
            None,
            GeneratorKind::Staged,
        )
        .expect("no-despeckle run");
        let cleaned = generate_inner(
            &png,
            RICH_PALETTE,
            16,
            16,
            None,
            Some(4),
            GeneratorKind::Staged,
        )
        .expect("despeckle run");
        assert_ne!(
            none.pattern, cleaned.pattern,
            "despeckle=Some(4) must merge isolated specks and change the pattern"
        );
    }

    // ---- max_colors=Some(0) flattens to Err; despeckle=Some(0) is Ok --------
    //
    // Asymmetric: the engine (`GreedyReducer::new`) rejects `max_colors == 0` as
    // `InvalidImage`, which the boundary flattens to `Err(String)` — the bridge
    // adds no validation and does not panic. `despeckle == Some(0)` is a legal
    // no-op that passes through.
    #[test]
    fn max_colors_zero_errs_while_despeckle_zero_is_legal() {
        let png = valid_png(16, 16);

        let err = generate(
            png.clone(),
            VALID_PALETTE.to_string(),
            8,
            8,
            Some(0),
            None,
            GeneratorKind::Staged,
        )
        .expect_err("max_colors=Some(0) must flatten to Err");
        // Pin the EXACT flattened Display: the exported boundary returns precisely
        // the inner BeadError's Display string — no wrapping, no re-phrasing.
        let inner = generate_inner(
            &png,
            VALID_PALETTE,
            8,
            8,
            Some(0),
            None,
            GeneratorKind::Staged,
        )
        .expect_err("max_colors=Some(0) must fail at the engine");
        assert_eq!(
            err,
            inner.to_string(),
            "flattened boundary message must equal the inner BeadError Display"
        );
        assert_eq!(
            err,
            "invalid image: reducer: max_colors must be >= 1, got 0"
        );

        let ok = generate(
            png,
            VALID_PALETTE.to_string(),
            8,
            8,
            None,
            Some(0),
            GeneratorKind::Staged,
        )
        .expect("despeckle=Some(0) is a legal no-op");
        assert_eq!(ok.pattern.cells.len(), 8 * 8);
    }
}
