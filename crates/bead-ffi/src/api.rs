//! The single bridge function exposed to Dart, plus the FRB mirrors of the
//! `bead-core` types that cross the boundary.
//!
//! Zero business logic (CLAUDE rule 4): the bridge calls only `load_palette`,
//! `generate_pattern`, and `pattern_json` — never an internal pipeline stage,
//! and never re-assembles the image→match→stats→render flow. The M8 boundary is
//! `width` / `height` only; `filter` / `cell_size` / `shape` / `matcher` are
//! not caller options — they take the engine `Default` exactly as the CLI does.

use bead_core::pipeline::pattern_json;
use bead_core::{generate_pattern, load_palette, GenerateOptions};
use flutter_rust_bridge::frb;

// The FRB-generated glue (`frb_generated.rs`) refers to the mirrored core types
// as `crate::api::BeadPattern` / `crate::api::ColorStat`, so they must be public
// at this path. Re-export the real bead-core types (no DTO copy) — the mirror
// structs below describe their field shape to FRB.
pub use bead_core::{BeadPattern, ColorStat};

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
/// The M8 boundary is `width` / `height` only. The bridge:
/// 1. `load_palette(palette_json.as_bytes())` — `load_palette` takes `&[u8]`, so
///    the JSON `String` is passed as its UTF-8 bytes,
/// 2. builds `GenerateOptions { width, height, ..Default::default() }` — the
///    **exact** construction the CLI uses (filter/cell_size/shape/matcher =
///    engine default Triangle/10/Square/Oklab),
/// 3. calls `generate_pattern`, then `pattern_json` on the result.
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
) -> Result<GenerateOutput, String> {
    generate_inner(&image_bytes, &palette_json, width, height).map_err(|e| e.to_string())
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
) -> Result<GenerateOutput, bead_core::BeadError> {
    let palette = load_palette(palette_json.as_bytes())?;
    let opts = GenerateOptions {
        width,
        height,
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

    // ---- happy path: the bridge returns all products -----------------------

    #[test]
    fn generate_returns_all_products() {
        let png = valid_png(32, 40);
        let out = generate_inner(&png, VALID_PALETTE, 16, 20).expect("generation must succeed");

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

    // ---- tasks 3.3 layer ①: each bad input → the expected BeadError variant -
    //
    // The exported `generate` flattens these to `err.to_string()` (asserted in
    // `display_flattening_matches_to_string` below); these tests pin the variant
    // mapping the design records (D-Errors).

    #[test]
    fn undecodable_image_yields_image_decode() {
        let err = generate_inner(b"not an image", VALID_PALETTE, 8, 8)
            .expect_err("garbage image bytes must fail");
        assert!(
            matches!(err, BeadError::ImageDecode(_)),
            "expected ImageDecode, got {err:?}"
        );
    }

    #[test]
    fn empty_image_bytes_yields_image_decode() {
        let err =
            generate_inner(b"", VALID_PALETTE, 8, 8).expect_err("empty image bytes must fail");
        assert!(
            matches!(err, BeadError::ImageDecode(_)),
            "expected ImageDecode, got {err:?}"
        );
    }

    #[test]
    fn malformed_palette_json_yields_palette_parse() {
        let png = valid_png(16, 16);
        let err = generate_inner(&png, "{ not json", 8, 8)
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
        let err = generate_inner(&png, palette, 8, 8).expect_err("zero-color palette must fail");
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
        let err = generate_inner(&png, palette, 8, 8).expect_err("duplicate code must fail");
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
        let err = generate_inner(&png, palette, 8, 8).expect_err("malformed hex must fail");
        assert!(
            matches!(err, BeadError::InvalidPalette { .. }),
            "expected InvalidPalette, got {err:?}"
        );
    }

    #[test]
    fn zero_width_yields_invalid_image() {
        let png = valid_png(16, 16);
        let err = generate_inner(&png, VALID_PALETTE, 0, 8).expect_err("width==0 must fail");
        assert!(
            matches!(err, BeadError::InvalidImage { .. }),
            "expected InvalidImage, got {err:?}"
        );
    }

    #[test]
    fn zero_height_yields_invalid_image() {
        let png = valid_png(16, 16);
        let err = generate_inner(&png, VALID_PALETTE, 8, 0).expect_err("height==0 must fail");
        assert!(
            matches!(err, BeadError::InvalidImage { .. }),
            "expected InvalidImage, got {err:?}"
        );
    }

    // ---- tasks 3.3 layer ②: the exported boundary flattens to to_string() ---
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
            let inner = generate_inner(img, pal, w, h).expect_err("must fail");
            let exported = generate(img.to_vec(), pal.to_string(), w, h)
                .expect_err("exported boundary must also fail");
            assert_eq!(
                exported,
                inner.to_string(),
                "boundary message must equal BeadError Display string"
            );
        }
    }
}
