//! Palette loading and validation. Bytes in, data out — no filesystem.
//!
//! `load_palette` parses JSON bytes into a [`Palette`], parsing each `rgb`
//! string into `[u8; 3]` and validating structure (non-empty colors, unique
//! codes, well-formed hex) fail-fast in a fixed order. Determinism is a hard
//! requirement: no `HashMap`/`HashSet` anywhere — duplicate detection uses an
//! ordered `Vec` scan and errors name a single offending `code`.

use serde::Deserialize;

use crate::BeadError;

/// A single bead color: a stable `code`, a human-readable `name`, and an
/// `rgb` triple already parsed from `"#RRGGBB"`.
///
/// Derives `PartialEq` (for `assert_eq!` and deterministic comparison) but
/// deliberately **not** `Eq` — see design D2.
#[derive(Debug, Clone, PartialEq)]
pub struct PaletteColor {
    pub code: String,
    pub name: String,
    pub rgb: [u8; 3],
}

/// A bead palette: a free-form `brand` label and an ordered list of colors.
/// Color order matches the source JSON.
#[derive(Debug, Clone, PartialEq)]
pub struct Palette {
    pub brand: String,
    pub colors: Vec<PaletteColor>,
}

/// Private deserialization DTO — a dumb mapping of the JSON shape. `rgb` stays
/// a `String` here; hex parsing happens in `load_palette`. No
/// `#[serde(deny_unknown_fields)]`: extra fields are silently ignored
/// (lenient, forward-compatible — see design D2). Missing required fields
/// still surface as a serde error → `BeadError::PaletteParse`.
#[derive(Deserialize)]
struct RawPalette {
    brand: String,
    colors: Vec<RawColor>,
}

#[derive(Deserialize)]
struct RawColor {
    code: String,
    name: String,
    rgb: String,
}

/// Parse a strict `"#RRGGBB"` hex string into `[u8; 3]`.
///
/// Accepts only: a leading `#`, then exactly 6 ASCII hex digits
/// (case-insensitive). Rejects `#RGB` shorthand, a missing `#`, and any other
/// length. Returns an error **without** a `code` — `load_palette` wraps it
/// into `InvalidPalette` and supplies the offending color's `code`.
fn parse_hex(s: &str) -> Result<[u8; 3], BeadError> {
    let digits = s
        .strip_prefix('#')
        .ok_or_else(|| BeadError::InvalidPalette {
            reason: format!("rgb {s:?} must start with '#'"),
        })?;

    // Byte length AND ASCII-hex-digit: the all-ASCII check makes the byte slices
    // below land on char boundaries (no panic on multi-byte UTF-8 like "#€000")
    // and rejects sign chars `+`/`-` that `from_str_radix` would otherwise accept.
    if digits.len() != 6 || !digits.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(BeadError::InvalidPalette {
            reason: format!("rgb {s:?} must be exactly 6 hex digits after '#'"),
        });
    }

    let mut rgb = [0u8; 3];
    for (i, chunk) in rgb.iter_mut().enumerate() {
        let byte = &digits[i * 2..i * 2 + 2];
        *chunk = u8::from_str_radix(byte, 16).map_err(|_| BeadError::InvalidPalette {
            reason: format!("rgb {s:?} contains non-hex characters"),
        })?;
    }
    Ok(rgb)
}

/// Load and validate a palette from JSON bytes.
///
/// Parses `RawPalette`, then validates fail-fast in a fixed order so the
/// "first error" is deterministic (design D5):
/// 1. `colors` is non-empty,
/// 2. each `rgb` parses (in `colors` order; first bad hex returns, naming its
///    `code`),
/// 3. `code` values are unique (first duplicate returns, naming the `code`).
///
/// Duplicate detection uses an ordered `Vec` scan — no `HashMap` (design D8).
pub fn load_palette(bytes: &[u8]) -> Result<Palette, BeadError> {
    let raw: RawPalette = serde_json::from_slice(bytes)?;

    // (1) non-empty colors
    if raw.colors.is_empty() {
        return Err(BeadError::InvalidPalette {
            reason: "palette has no colors".to_string(),
        });
    }

    // (2) parse hex in order; first bad hex returns, naming its code
    let mut colors: Vec<PaletteColor> = Vec::with_capacity(raw.colors.len());
    for raw_color in &raw.colors {
        let rgb = parse_hex(&raw_color.rgb).map_err(|e| match e {
            BeadError::InvalidPalette { reason } => BeadError::InvalidPalette {
                reason: format!("color {:?}: {reason}", raw_color.code),
            },
            other => other,
        })?;
        colors.push(PaletteColor {
            code: raw_color.code.clone(),
            name: raw_color.name.clone(),
            rgb,
        });
    }

    let palette = Palette {
        brand: raw.brand,
        colors,
    };

    // (3) unique codes
    validate_palette(&palette)?;

    Ok(palette)
}

/// Re-check structural invariants of an already-constructed `Palette`:
/// `colors` is non-empty and all `code` values are unique.
///
/// Hex is parsed to `[u8; 3]` at load time, so a constructed `Palette` is
/// hex-valid by type — `validate_palette` does not (and cannot) re-check it.
/// Duplicate detection uses an ordered `Vec` scan, no `HashMap` (design D8).
pub fn validate_palette(palette: &Palette) -> Result<(), BeadError> {
    if palette.colors.is_empty() {
        return Err(BeadError::InvalidPalette {
            reason: "palette has no colors".to_string(),
        });
    }

    let mut seen: Vec<&str> = Vec::with_capacity(palette.colors.len());
    for color in &palette.colors {
        if seen.contains(&color.code.as_str()) {
            return Err(BeadError::InvalidPalette {
                reason: format!("duplicate color code {:?}", color.code),
            });
        }
        seen.push(&color.code);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // 5.1 — Done-when 「合法调色板」
    #[test]
    fn load_valid_palette_parses_all_colors() {
        let bytes = br##"{
            "brand": "Test",
            "colors": [
                { "code": "T01", "name": "First",  "rgb": "#0A0B0C" },
                { "code": "T02", "name": "Second", "rgb": "#FFFFFF" },
                { "code": "T03", "name": "Third",  "rgb": "#000000" }
            ]
        }"##;

        let palette = load_palette(bytes).expect("valid palette must load");
        assert_eq!(palette.brand, "Test");
        assert_eq!(palette.colors.len(), 3);

        // order preserved
        assert_eq!(palette.colors[0].code, "T01");
        assert_eq!(palette.colors[1].code, "T02");
        assert_eq!(palette.colors[2].code, "T03");

        // hex parsed: #0A0B0C -> [10, 11, 12]
        assert_eq!(palette.colors[0].rgb, [10, 11, 12]);
        assert_eq!(palette.colors[1].rgb, [255, 255, 255]);
        assert_eq!(palette.colors[2].rgb, [0, 0, 0]);
    }

    // 5.2 — Done-when 「坏 hex」
    #[test]
    fn load_rejects_malformed_hex() {
        let bytes = br##"{
            "brand": "Test",
            "colors": [
                { "code": "BAD", "name": "Bad", "rgb": "#00GG00" }
            ]
        }"##;

        let err = load_palette(bytes).expect_err("malformed hex must be rejected");
        match err {
            BeadError::InvalidPalette { reason } => {
                // reason names the offending code
                assert!(
                    reason.contains("BAD"),
                    "reason must name the offending code, got: {reason:?}"
                );
            }
            other => panic!("expected InvalidPalette, got {other:?}"),
        }
    }

    // 5.3 — Done-when 「空调色板」
    #[test]
    fn load_rejects_empty_colors() {
        let bytes = br##"{ "brand": "Empty", "colors": [] }"##;

        let err = load_palette(bytes).expect_err("empty colors must be rejected");
        assert!(
            matches!(err, BeadError::InvalidPalette { .. }),
            "expected InvalidPalette, got {err:?}"
        );
    }

    // 5.4 — duplicate codes (all colors must use valid hex; see tasks.md §5 note)
    #[test]
    fn load_rejects_duplicate_codes() {
        let bytes = br##"{
            "brand": "Dupe",
            "colors": [
                { "code": "S01", "name": "First",  "rgb": "#111111" },
                { "code": "S01", "name": "Second", "rgb": "#222222" }
            ]
        }"##;

        let err = load_palette(bytes).expect_err("duplicate codes must be rejected");
        match err {
            BeadError::InvalidPalette { reason } => {
                assert!(
                    reason.contains("S01"),
                    "reason must name the duplicate code, got: {reason:?}"
                );
            }
            other => panic!("expected InvalidPalette, got {other:?}"),
        }
    }

    // 5.5 — boundaries: wrong length / missing '#' / lowercase accepted
    #[test]
    fn load_rejects_wrong_hex_length() {
        let bytes = br##"{
            "brand": "Test",
            "colors": [
                { "code": "C01", "name": "Short", "rgb": "#FFF" }
            ]
        }"##;

        let err = load_palette(bytes).expect_err("shorthand hex must be rejected");
        assert!(
            matches!(err, BeadError::InvalidPalette { .. }),
            "expected InvalidPalette, got {err:?}"
        );
    }

    #[test]
    fn load_rejects_missing_hash() {
        let bytes = br##"{
            "brand": "Test",
            "colors": [
                { "code": "C01", "name": "NoHash", "rgb": "000000" }
            ]
        }"##;

        let err = load_palette(bytes).expect_err("missing '#' must be rejected");
        assert!(
            matches!(err, BeadError::InvalidPalette { .. }),
            "expected InvalidPalette, got {err:?}"
        );
    }

    #[test]
    fn load_accepts_lowercase_hex() {
        let bytes = br##"{
            "brand": "Test",
            "colors": [
                { "code": "C01", "name": "Lower", "rgb": "#aabbcc" }
            ]
        }"##;

        let palette = load_palette(bytes).expect("lowercase hex must be accepted");
        assert_eq!(palette.colors[0].rgb, [170, 187, 204]);
    }

    // 5.6 — parse errors: only assert the variant, not serde_json Display text
    #[test]
    fn load_rejects_malformed_json() {
        let err = load_palette(b"{ not json").expect_err("malformed JSON must be rejected");
        assert!(
            matches!(err, BeadError::PaletteParse(_)),
            "expected PaletteParse, got {err:?}"
        );
    }

    #[test]
    fn load_rejects_missing_field() {
        // missing required `rgb` field -> serde missing-field error -> PaletteParse
        let bytes = br##"{
            "brand": "Test",
            "colors": [
                { "code": "C01", "name": "NoRgb" }
            ]
        }"##;

        let err = load_palette(bytes).expect_err("missing field must be rejected");
        assert!(
            matches!(err, BeadError::PaletteParse(_)),
            "expected PaletteParse, got {err:?}"
        );
    }

    // 5.7 — validate passes on a loaded palette
    #[test]
    fn validate_passes_on_loaded_palette() {
        let bytes = br##"{
            "brand": "Test",
            "colors": [
                { "code": "C01", "name": "One", "rgb": "#010203" },
                { "code": "C02", "name": "Two", "rgb": "#040506" }
            ]
        }"##;

        let palette = load_palette(bytes).expect("valid palette must load");
        assert!(validate_palette(&palette).is_ok());
    }

    // 5.7b — validate rejects an empty hand-built palette (guards against no-op)
    #[test]
    fn validate_rejects_empty() {
        let palette = Palette {
            brand: "Empty".to_string(),
            colors: vec![],
        };

        let err = validate_palette(&palette).expect_err("empty palette must be rejected");
        assert!(
            matches!(err, BeadError::InvalidPalette { .. }),
            "expected InvalidPalette, got {err:?}"
        );
    }

    // 5.7c — validate rejects duplicate codes in a hand-built palette (guards no-op)
    #[test]
    fn validate_rejects_duplicate_codes() {
        let palette = Palette {
            brand: "Dupe".to_string(),
            colors: vec![
                PaletteColor {
                    code: "S01".to_string(),
                    name: "First".to_string(),
                    rgb: [1, 1, 1],
                },
                PaletteColor {
                    code: "S01".to_string(),
                    name: "Second".to_string(),
                    rgb: [2, 2, 2],
                },
            ],
        };

        let err = validate_palette(&palette).expect_err("duplicate codes must be rejected");
        match err {
            BeadError::InvalidPalette { reason } => {
                assert!(
                    reason.contains("S01"),
                    "reason must name the duplicate code, got: {reason:?}"
                );
            }
            other => panic!("expected InvalidPalette, got {other:?}"),
        }
    }

    // 5.8 — Done-when 「附带 Artkal 调色板可加载」
    #[test]
    fn bundled_artkal_palette_loads() {
        let bytes = include_bytes!("../../../../palettes/artkal_s.json");
        let palette = load_palette(bytes).expect("bundled Artkal palette must load");

        assert!(
            !palette.colors.is_empty(),
            "bundled palette must be non-empty"
        );

        // codes are unique (ordered Vec scan, no HashSet — matches D8)
        let mut seen: Vec<&str> = Vec::with_capacity(palette.colors.len());
        for color in &palette.colors {
            assert!(
                !seen.contains(&color.code.as_str()),
                "bundled palette has duplicate code: {:?}",
                color.code
            );
            seen.push(&color.code);
        }
    }

    // 5.9 — determinism (D5 fixed validation order)
    #[test]
    fn load_is_deterministic() {
        // (a) same bytes twice -> equal Palette
        let valid = br##"{
            "brand": "Det",
            "colors": [
                { "code": "C01", "name": "One", "rgb": "#0A0B0C" },
                { "code": "C02", "name": "Two", "rgb": "#AABBCC" }
            ]
        }"##;
        let first = load_palette(valid).expect("must load");
        let second = load_palette(valid).expect("must load");
        assert_eq!(first, second);

        // (b) same bad-hex bytes twice -> byte-identical reason, names same code
        let bad_hex = br##"{
            "brand": "Det",
            "colors": [
                { "code": "C01", "name": "One", "rgb": "#00GG00" }
            ]
        }"##;
        let r1 = match load_palette(bad_hex).expect_err("bad hex must fail") {
            BeadError::InvalidPalette { reason } => reason,
            other => panic!("expected InvalidPalette, got {other:?}"),
        };
        let r2 = match load_palette(bad_hex).expect_err("bad hex must fail") {
            BeadError::InvalidPalette { reason } => reason,
            other => panic!("expected InvalidPalette, got {other:?}"),
        };
        assert_eq!(r1, r2, "bad-hex reason must be byte-identical");

        // (c) same duplicate-code bytes twice -> byte-identical reason, same code
        let dup = br##"{
            "brand": "Det",
            "colors": [
                { "code": "C01", "name": "One", "rgb": "#111111" },
                { "code": "C01", "name": "Two", "rgb": "#222222" }
            ]
        }"##;
        let d1 = match load_palette(dup).expect_err("dup must fail") {
            BeadError::InvalidPalette { reason } => reason,
            other => panic!("expected InvalidPalette, got {other:?}"),
        };
        let d2 = match load_palette(dup).expect_err("dup must fail") {
            BeadError::InvalidPalette { reason } => reason,
            other => panic!("expected InvalidPalette, got {other:?}"),
        };
        assert_eq!(d1, d2, "duplicate-code reason must be byte-identical");

        // (d) D5 order: bad hex (stage ②) precedes duplicate code (stage ③).
        // Input has BOTH a bad hex (first color) AND a duplicate code (D99 twice).
        // The reported error must be the bad hex — the first trigger in doc order.
        let both = br##"{
            "brand": "Det",
            "colors": [
                { "code": "D99", "name": "BadHex", "rgb": "#00GG00" },
                { "code": "D99", "name": "Dup",    "rgb": "#333333" }
            ]
        }"##;
        let reason = match load_palette(both).expect_err("must fail") {
            BeadError::InvalidPalette { reason } => reason,
            other => panic!("expected InvalidPalette, got {other:?}"),
        };
        // The reported error must be the hex failure (stage ②), not the duplicate
        // (stage ③): the hex-guard message says "hex digits"; the dup message says
        // "duplicate". Asserting both directions pins the D5 order regardless of the
        // exact hex wording.
        assert!(
            reason.contains("hex digits") && !reason.contains("duplicate"),
            "D5 order: bad hex (stage ②) must be reported before duplicate (stage ③), \
             got: {reason:?}"
        );
    }

    // F1 regression: a multi-byte UTF-8 rgb of byte-length 6 ("€" = 3 bytes, so
    // "#€000" is 6 bytes after '#') must return Err, never panic on a mid-codepoint
    // byte slice.
    #[test]
    fn load_rejects_non_ascii_hex() {
        let bytes =
            r##"{ "brand": "T", "colors": [ { "code": "C01", "name": "Euro", "rgb": "#€000" } ] }"##
                .as_bytes();
        let err = load_palette(bytes).expect_err("non-ASCII hex must be rejected, not panic");
        assert!(
            matches!(err, BeadError::InvalidPalette { .. }),
            "expected InvalidPalette, got {err:?}"
        );
    }

    // F2 regression: `u8::from_str_radix` accepts a leading '+', but strict #RRGGBB
    // must reject sign characters.
    #[test]
    fn load_rejects_sign_hex() {
        let bytes = br##"{
            "brand": "T",
            "colors": [
                { "code": "C01", "name": "Plus", "rgb": "#+0+0+0" }
            ]
        }"##;
        let err = load_palette(bytes).expect_err("sign char in hex must be rejected");
        assert!(
            matches!(err, BeadError::InvalidPalette { .. }),
            "expected InvalidPalette, got {err:?}"
        );
    }
}
