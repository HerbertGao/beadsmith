//! Color matching: map raw RGB cells to palette indices. This is the single
//! hand-off from `PixelGrid` (raw RGB) to `BeadPattern` (palette indices); see
//! design D5/D6.
//!
//! **Invariants split by matcher** (design risk note / §1.4): **no `sqrt`** and
//! **no `rayon`** hold for *all* matchers — nearest-color comparison always
//! compares squared distance (`√` is monotonic, so it preserves the argmin).
//! **`no f32`** is specific to [`RgbMatcher`] (pure integer, bit-identical
//! across architectures); [`LabMatcher`] and [`OklabMatcher`] deliberately
//! introduce `f32` for perceptual distance, so they are byte-stable only
//! **same-machine** (canonical arm64 golden + same-machine CLI==FFI, like
//! `Lanczos3`'s `f32::sin`).
//!
//! The [`ColorMatcher`] trait is the seam between matchers: [`RgbMatcher`] is
//! the algorithm-Phase-1 RGB squared-Euclidean implementation; [`LabMatcher`]
//! and [`OklabMatcher`] are algorithm-Phase-3 perceptual implementations.
//! `ColorMatcher` must stay object-safe (D2): it is used as `&dyn ColorMatcher`
//! here and `Box<dyn>` in the M6 pipeline.

use crate::models::{BeadPattern, PixelGrid};
use crate::palette::Palette;
use crate::BeadError;

/// Maps an arbitrary RGB triple to the closest palette index.
///
/// Total function: for a non-empty palette, every `[u8; 3]` maps to a valid
/// index. Must remain object-safe (`&dyn ColorMatcher`) — no generic methods,
/// no `Self`-returning methods, no associated types in signatures (design D2).
pub trait ColorMatcher {
    /// Returns the index of the palette color closest to `target`.
    fn find_best_match(&self, target: [u8; 3]) -> u16;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MatcherKind {
    Rgb,
    Lab,
    #[default]
    Oklab,
}

/// Shared `new`-time palette-size guard for both matchers (design D7; reuses
/// `InvalidPalette`, no new variant): empty `colors` → `InvalidPalette`
/// (`reason` contains "no colors"); `colors.len() > 65536` → `InvalidPalette`
/// (`reason` contains "more than"), guarding `index as u16` truncation. The
/// boundary is exact: legal indices are `0..=65535` (`u16::MAX == 65535`), so
/// `len == 65536` is accepted and `65537` is the first rejected length. Never
/// panics.
fn check_palette_len(palette: &Palette) -> Result<(), BeadError> {
    if palette.colors.is_empty() {
        return Err(BeadError::InvalidPalette {
            reason: "matcher: palette has no colors".to_string(),
        });
    }
    if palette.colors.len() > 65536 {
        return Err(BeadError::InvalidPalette {
            reason: "matcher: palette has more than 65536 colors".to_string(),
        });
    }
    Ok(())
}

/// Phase 1 matcher: RGB squared-Euclidean distance, lowest index on a tie.
///
/// Holds an order-preserving snapshot of the palette's RGB taken at
/// construction (a performance seam, and the source of the lowest-index tie
/// rule: snapshot index `i` ≡ `palette.colors[i]`, design D2/D3). Because it is
/// a snapshot, mutating the original `Palette` after `new` does not affect the
/// matcher — this is the intended value semantics (palette is an immutable
/// input; design risk note).
#[derive(Debug)]
pub struct RgbMatcher {
    /// Order-preserving RGB snapshot; `colors[i]` ≡ `palette.colors[i].rgb`.
    colors: Vec<[u8; 3]>,
}

impl RgbMatcher {
    /// Build a matcher from a one-time, order-preserving RGB snapshot;
    /// size-validated via [`check_palette_len`]. Never panics.
    pub fn new(palette: &Palette) -> Result<RgbMatcher, BeadError> {
        check_palette_len(palette)?;
        let colors: Vec<[u8; 3]> = palette.colors.iter().map(|c| c.rgb).collect();
        Ok(RgbMatcher { colors })
    }
}

impl ColorMatcher for RgbMatcher {
    fn find_best_match(&self, target: [u8; 3]) -> u16 {
        // `new` guarantees a non-empty snapshot of <= 65536 colors, so this
        // loop always sets `best` and the index fits in u16. The hot path is a
        // total function — no `Result`, no panic (design D3).
        let mut best_i: usize = 0;
        let mut best_d: u32 = u32::MAX;
        for (i, c) in self.colors.iter().enumerate() {
            // Widen each component to i32 BEFORE subtracting (a u8 difference
            // would underflow); square fits in i32; accumulate in u32 (max
            // 3 * 255^2 = 195075, which overflows u16). No sqrt — squared
            // distance preserves ordering and stays pure-integer (design D3).
            let dr = target[0] as i32 - c[0] as i32;
            let dg = target[1] as i32 - c[1] as i32;
            let db = target[2] as i32 - c[2] as i32;
            let d = (dr * dr + dg * dg + db * db) as u32;

            // Strict `<` only: equal distances do NOT update, so the lowest
            // index wins on a tie (and on duplicate-RGB exact hits). This is a
            // determinism gate (CLAUDE rule 2 / design D3.3).
            if d < best_d {
                best_d = d;
                best_i = i;
            }
        }
        best_i as u16
    }
}

fn linearize(rgb: [u8; 3]) -> [f32; 3] {
    let lin = |c: u8| -> f32 {
        let c = c as f32 / 255.0;
        if c <= 0.04045 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    };
    [lin(rgb[0]), lin(rgb[1]), lin(rgb[2])]
}

/// Convert an sRGB `[u8; 3]` to CIELAB `[L*, a*, b*]` (`f32`), D65 white point.
///
/// Standard pipeline (design D5): each channel `/255` → inverse-sRGB-gamma
/// linearization → XYZ via the sRGB/D65 matrix → L\*a\*b\* with the `6/29`
/// threshold piecewise + `cbrt`. Uses plain IEEE `f32` ops only — **no
/// `mul_add` / FMA** (a fused multiply-add could codegen-diverge between the CLI
/// binary and the FFI staticlib/cdylib and break same-machine byte equality,
/// T4). For any `[u8; 3]` every step is finite, so the result never has a NaN.
fn srgb_to_lab(rgb: [u8; 3]) -> [f32; 3] {
    // 1) channel /255 then inverse-gamma linearize into linear-light sRGB.
    let [r, g, b] = linearize(rgb);

    // 2) linear sRGB -> XYZ via the sRGB / D65 matrix. Plain `*` and `+` (no
    //    mul_add): the column sums are exactly the D65 white below, so white
    //    maps to x=y=z=1.
    let x = 0.412_456_4 * r + 0.357_576_1 * g + 0.180_437_5 * b;
    let y = 0.212_672_9 * r + 0.715_152_2 * g + 0.072_175_0 * b;
    let z = 0.019_333_9 * r + 0.119_192 * g + 0.950_304_1 * b;

    // 3) normalize by the D65 reference white, then apply f(t) with the 6/29
    //    threshold; below delta^3 the linear segment keeps f(t) finite at t->0.
    let f = |t: f32| -> f32 {
        const DELTA: f32 = 6.0 / 29.0;
        if t > DELTA * DELTA * DELTA {
            t.cbrt()
        } else {
            t / (3.0 * DELTA * DELTA) + 4.0 / 29.0
        }
    };
    let fx = f(x / 0.950_47); // Xn (D65) == matrix X column sum
    let fy = f(y); //            Yn == 1.0
    let fz = f(z / 1.088_83); // Zn (D65) == matrix Z column sum

    // 4) L*a*b*.
    let l = 116.0 * fy - 16.0;
    let a = 500.0 * (fx - fy);
    let bb = 200.0 * (fy - fz);
    [l, a, bb]
}

/// Convert an sRGB `[u8; 3]` to Oklab `[L, a, b]` (`f32`).
///
/// Uses Bjorn Ottosson's standard linear-sRGB -> LMS -> cbrt -> Oklab
/// matrices, with the same inverse-gamma linearization as [`srgb_to_lab`].
fn srgb_to_oklab(rgb: [u8; 3]) -> [f32; 3] {
    let [r, g, b] = linearize(rgb);

    let l = 0.412_221_46 * r + 0.536_332_55 * g + 0.051_445_994 * b;
    let m = 0.211_903_5 * r + 0.680_699_5 * g + 0.107_396_96 * b;
    let s = 0.088_302_46 * r + 0.281_718_85 * g + 0.629_978_7 * b;

    let l_ = l.cbrt();
    let m_ = m.cbrt();
    let s_ = s.cbrt();

    let l_ok = 0.210_454_26 * l_ + 0.793_617_8 * m_ - 0.004_072_047 * s_;
    let a_ok = 1.977_998_5 * l_ - 2.428_592_2 * m_ + 0.450_593_7 * s_;
    let b_ok = 0.025_904_037 * l_ + 0.782_771_77 * m_ - 0.808_675_77 * s_;

    [l_ok, a_ok, b_ok]
}

/// Phase 3 matcher: CIELAB + ΔE76 (perceptual distance), lowest index on a tie.
///
/// Mirrors [`RgbMatcher`]'s structure (design D4): an order-preserving snapshot
/// taken at construction — `colors[i]` ≡ `srgb_to_lab(palette.colors[i].rgb)`,
/// carrying the lowest-index tie rule — with the same value semantics (mutating
/// the source `Palette` after `new` does not affect the matcher). Unlike
/// `RgbMatcher` it stores `f32` Lab, so its results are **not** bit-identical
/// across architectures (`cbrt`/`powf` differ per libm; design D6) — only
/// same-machine deterministic.
#[derive(Debug)]
pub struct LabMatcher {
    /// Order-preserving Lab snapshot; `colors[i]` ≡ `srgb_to_lab(palette.colors[i].rgb)`.
    colors: Vec<[f32; 3]>,
}

impl LabMatcher {
    /// Build a matcher from a one-time, order-preserving Lab snapshot;
    /// size-validated via [`check_palette_len`] (same guards as `RgbMatcher`).
    /// Never panics.
    pub fn new(palette: &Palette) -> Result<LabMatcher, BeadError> {
        check_palette_len(palette)?;
        // One-time sRGB->Lab conversion, amortized over all pixels (design D4).
        let colors: Vec<[f32; 3]> = palette.colors.iter().map(|c| srgb_to_lab(c.rgb)).collect();
        Ok(LabMatcher { colors })
    }
}

impl ColorMatcher for LabMatcher {
    fn find_best_match(&self, target: [u8; 3]) -> u16 {
        // Same search skeleton as RgbMatcher::find_best_match, but in Lab: scan
        // linearly, compare the SUM OF SQUARED Lab component differences
        // (= ΔE76² — `√` is monotonic so squared distance preserves the argmin;
        // design D1), strict `<` update so the lowest index wins on a tie. Plain
        // f32 ops, no mul_add/FMA (T4). `new` guarantees a non-empty snapshot of
        // <= 65536 colors, so `best_i` is always set and fits in u16 — a total
        // function: no `Result`, no panic, and finite Lab in -> no NaN out (D5).
        let t = srgb_to_lab(target);
        let mut best_i: usize = 0;
        let mut best_d: f32 = f32::INFINITY;
        for (i, c) in self.colors.iter().enumerate() {
            let dl = t[0] - c[0];
            let da = t[1] - c[1];
            let db = t[2] - c[2];
            let d = dl * dl + da * da + db * db;
            if d < best_d {
                best_d = d;
                best_i = i;
            }
        }
        best_i as u16
    }
}

/// Phase 3 matcher: Oklab + ΔEok² (perceptual distance), lowest index on a tie.
///
/// Mirrors [`LabMatcher`]'s structure with an order-preserving Oklab snapshot
/// taken at construction — `colors[i]` ≡ `srgb_to_oklab(palette.colors[i].rgb)`.
/// Like Lab, this uses `f32` and is same-machine deterministic rather than
/// cross-architecture bit-identical.
#[derive(Debug)]
pub struct OklabMatcher {
    /// Order-preserving Oklab snapshot; `colors[i]` ≡ `srgb_to_oklab(palette.colors[i].rgb)`.
    colors: Vec<[f32; 3]>,
}

impl OklabMatcher {
    /// Build a matcher from a one-time, order-preserving Oklab snapshot;
    /// size-validated via [`check_palette_len`] (same guards as `RgbMatcher`).
    /// Never panics.
    pub fn new(palette: &Palette) -> Result<OklabMatcher, BeadError> {
        check_palette_len(palette)?;
        let colors: Vec<[f32; 3]> = palette
            .colors
            .iter()
            .map(|c| srgb_to_oklab(c.rgb))
            .collect();
        Ok(OklabMatcher { colors })
    }
}

impl ColorMatcher for OklabMatcher {
    fn find_best_match(&self, target: [u8; 3]) -> u16 {
        let t = srgb_to_oklab(target);
        let mut best_i: usize = 0;
        let mut best_d: f32 = f32::INFINITY;
        for (i, c) in self.colors.iter().enumerate() {
            let dl = t[0] - c[0];
            let da = t[1] - c[1];
            let db = t[2] - c[2];
            let d = dl * dl + da * da + db * db;
            if d < best_d {
                best_d = d;
                best_i = i;
            }
        }
        best_i as u16
    }
}

/// Map a `PixelGrid` (raw RGB) to a `BeadPattern` (palette indices).
///
/// Row-major one-to-one: `cells[i] = matcher.find_best_match(grid.pixels[i])`
/// for the same `i = y * width + x`, no coordinate conversion. `width` and
/// `height` are carried over verbatim. This is the single point where
/// `PixelGrid` (raw color) hands off to `BeadPattern` (indices); the
/// `PixelGrid` is not returned to external callers (design D6).
///
/// **Precondition** (same contract as `PixelGrid`, design D5): `grid.pixels.len()
/// == grid.width as usize * grid.height as usize` (length arithmetic in `usize`,
/// never `u32` multiply). This invariant is produced by `resize_image`; because
/// `PixelGrid` fields are `pub`, an external caller can construct one that
/// violates it — that is a caller contract violation. `match_pattern` iterates
/// `grid.pixels` to produce `cells` (so `cells.len() == grid.pixels.len()`,
/// which equals `width * height` when the precondition holds); it is a total
/// function and does **not** re-check (no `Result`). A degenerate grid with
/// `width == 0` or `height == 0` (empty `pixels` under the precondition)
/// correctly yields `cells.len() == 0`.
pub fn match_pattern(grid: &PixelGrid, matcher: &dyn ColorMatcher) -> BeadPattern {
    let cells: Vec<u16> = grid
        .pixels
        .iter()
        .map(|px| matcher.find_best_match(*px))
        .collect();
    BeadPattern {
        width: grid.width,
        height: grid.height,
        cells,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::palette::PaletteColor;

    // Small helper: build a Palette from (code, rgb) pairs. Names are not
    // load-bearing for matching, so reuse the code.
    fn palette_from(colors: &[(&str, [u8; 3])]) -> Palette {
        Palette {
            brand: "Test".to_string(),
            colors: colors
                .iter()
                .map(|(code, rgb)| PaletteColor {
                    code: code.to_string(),
                    name: code.to_string(),
                    rgb: *rgb,
                })
                .collect(),
        }
    }

    // 5.1 — exact hit maps to zero distance: a pixel equal to a palette color's
    // RGB returns that color's index.
    #[test]
    fn exact_hit_maps_to_zero_distance() {
        let palette = palette_from(&[("A", [10, 20, 30]), ("B", [200, 100, 50]), ("C", [0, 0, 0])]);
        let matcher = RgbMatcher::new(&palette).expect("valid palette");

        assert_eq!(matcher.find_best_match([10, 20, 30]), 0);
        assert_eq!(matcher.find_best_match([200, 100, 50]), 1);
        assert_eq!(matcher.find_best_match([0, 0, 0]), 2);
    }

    // 5.2 — off-palette pixel maps to the nearest (min squared distance) color.
    #[test]
    fn off_palette_maps_to_nearest() {
        // index 0 = [0,0,0], index 1 = [100,100,100].
        // target [10,10,10]: d to 0 = 3*100 = 300; d to 1 = 3*8100 = 24300.
        // 0 is nearer -> index 0.
        let palette = palette_from(&[("BLACK", [0, 0, 0]), ("GRAY", [100, 100, 100])]);
        let matcher = RgbMatcher::new(&palette).expect("valid palette");

        assert_eq!(matcher.find_best_match([10, 10, 10]), 0);
        // target [90,90,90]: d to 0 = 3*8100 = 24300; d to 1 = 3*100 = 300.
        // 1 is nearer -> index 1.
        assert_eq!(matcher.find_best_match([90, 90, 90]), 1);
    }

    // 5.2b — pin the i32 widening AND u32 accumulator guards. A u8-subtraction
    // implementation would panic/wrap on a negative difference; a u16
    // accumulator would wrap/truncate near the max distance.
    #[test]
    fn distance_guards_widening_and_accumulator() {
        // (1) negative component difference. target [0,0,0]:
        //   index 0 = [255,255,255] (d = 3*255^2 = 195075),
        //   index 1 = [0,0,0] (d = 0, the exact hit and the nearest).
        // Against index 0, every channel computes target - color = 0 - 255: a u8
        // subtraction would underflow (panic/wrap), while widening to i32 gives
        // -255 (squared 65025). Correct result is index 1.
        let palette = palette_from(&[("WHITE", [255, 255, 255]), ("BLACK", [0, 0, 0])]);
        let matcher = RgbMatcher::new(&palette).expect("valid palette");
        assert_eq!(matcher.find_best_match([0, 0, 0]), 1);

        // (2) near-max distance: target [0,0,0] vs index 0 = [255,255,255]
        // (d = 195075), index 1 = [254,254,254] (d = 193548). The second is
        // nearer. A u16 accumulator overflows on both -> panics in debug
        // (caught there); in release it wraps to 64003 vs 62476, which happen
        // to KEEP their order, so this pair alone does NOT catch a wrapping-u16
        // bug under --release. Case (3) below adds the release value guard.
        let palette2 = palette_from(&[("W255", [255, 255, 255]), ("W254", [254, 254, 254])]);
        let matcher2 = RgbMatcher::new(&palette2).expect("valid palette");
        assert_eq!(matcher2.find_best_match([0, 0, 0]), 1);

        // (3) release-profile value discrimination: a pair whose order REVERSES
        // under u16 truncation. target [0,0,0]:
        //   index 0 = [148,148,148] true d = 3*148^2 = 65712 (wraps to 176),
        //   index 1 = [147,147,147] true d = 3*147^2 = 64827 (no wrap).
        // u32 (correct): 64827 < 65712 -> index 1 (the genuinely nearer color).
        // A u16-width accumulator (or a final `as u16` truncation) sees
        // 176 < 64827 -> index 0 (wrong). This assert therefore fails such a
        // mutant in BOTH profiles (debug: overflow panic on the 65712 add;
        // release: wrong value), pinning the u32 accumulator width.
        let palette3 = palette_from(&[("D148", [148, 148, 148]), ("D147", [147, 147, 147])]);
        let matcher3 = RgbMatcher::new(&palette3).expect("valid palette");
        assert_eq!(matcher3.find_best_match([0, 0, 0]), 1);
    }

    // 5.3 — tie break returns the lowest index, deterministically. Two palette
    // colors equidistant from the target (target is exactly between them).
    #[test]
    fn tie_break_returns_lowest_index() {
        // index 0 = [0,0,0], index 1 = [20,0,0]. target [10,0,0] is midway:
        // d to 0 = 100, d to 1 = 100 (a tie). Lowest index -> 0.
        let palette = palette_from(&[("LO", [0, 0, 0]), ("HI", [20, 0, 0])]);
        let matcher = RgbMatcher::new(&palette).expect("valid palette");

        assert_eq!(matcher.find_best_match([10, 0, 0]), 0);
        // repeated calls are identical (determinism gate)
        assert_eq!(matcher.find_best_match([10, 0, 0]), 0);
        assert_eq!(matcher.find_best_match([10, 0, 0]), 0);
    }

    // 5.3b — exact hit with duplicate RGB returns the lowest index. validate
    // only guarantees unique `code`, not unique RGB; two colors with identical
    // RGB both have distance 0 to the target, so strict `<` picks the lower.
    #[test]
    fn exact_hit_duplicate_rgb_returns_lowest_index() {
        let palette = palette_from(&[
            ("DUP_A", [42, 42, 42]),
            ("DUP_B", [42, 42, 42]), // same RGB, different code
        ]);
        let matcher = RgbMatcher::new(&palette).expect("valid palette");

        // both distances are 0; lowest index wins.
        assert_eq!(matcher.find_best_match([42, 42, 42]), 0);
    }

    // 5.4 — match_pattern: shape preserved and row-major one-to-one.
    #[test]
    fn match_pattern_shape_and_rowmajor() {
        // index 0 = black, index 1 = white.
        let palette = palette_from(&[("BLACK", [0, 0, 0]), ("WHITE", [255, 255, 255])]);
        let matcher = RgbMatcher::new(&palette).expect("valid palette");

        // 2x2 grid satisfying pixels.len() == w*h.
        let grid = PixelGrid {
            width: 2,
            height: 2,
            pixels: vec![
                [0, 0, 0],       // -> 0
                [255, 255, 255], // -> 1
                [10, 10, 10],    // nearer black -> 0
                [240, 240, 240], // nearer white -> 1
            ],
        };
        let pattern = match_pattern(&grid, &matcher);

        assert_eq!(pattern.width, 2);
        assert_eq!(pattern.height, 2);
        assert_eq!(pattern.cells.len(), 4);

        // spot-check a cell: cells[i] == find_best_match(pixels[i])
        assert_eq!(pattern.cells[0], matcher.find_best_match(grid.pixels[0]));
        assert_eq!(pattern.cells[3], matcher.find_best_match(grid.pixels[3]));
        assert_eq!(pattern.cells, vec![0, 1, 0, 1]);

        // degenerate grid: width == 0 (empty pixels under the precondition) ->
        // cells.len() == 0, no panic.
        let empty = PixelGrid {
            width: 0,
            height: 5,
            pixels: vec![],
        };
        let empty_pattern = match_pattern(&empty, &matcher);
        assert_eq!(empty_pattern.width, 0);
        assert_eq!(empty_pattern.height, 5);
        assert_eq!(empty_pattern.cells.len(), 0);
    }

    // 5.5 — determinism: two runs are PartialEq, plus a hardcoded cross-arch
    // integer golden.
    #[test]
    fn match_pattern_is_deterministic() {
        let palette = palette_from(&[
            ("BLACK", [0, 0, 0]),       // index 0
            ("WHITE", [255, 255, 255]), // index 1
            ("MID", [128, 128, 128]),   // index 2
        ]);
        let matcher = RgbMatcher::new(&palette).expect("valid palette");

        // Fixed small grid covering: exact hit, equidistant tie, off-palette.
        let grid = PixelGrid {
            width: 3,
            height: 2,
            pixels: vec![
                [0, 0, 0],       // exact hit -> 0
                [255, 255, 255], // exact hit -> 1
                [128, 128, 128], // exact hit -> 2
                [10, 10, 10],    // off-palette, nearest black -> 0
                [200, 200, 200], // off-palette, nearest white -> 1
                [64, 64, 64],    // off-palette: d to 0 = 3*64^2=12288,
                                 //   d to 2 = 3*64^2=12288 -> tie, lowest -> 0
            ],
        };

        // (1) same input twice -> PartialEq-equal BeadPattern.
        let first = match_pattern(&grid, &matcher);
        let second = match_pattern(&grid, &matcher);
        assert_eq!(first, second);

        // (2) cross-arch bit-exact golden. Pure integer math -> identical on
        // arm64 and x86_64, so the expected Vec<u16> can be hardcoded.
        // ponytail: 整数匹配跨架构位精确，可硬编码 golden；M2 Lanczos3 f32 才不敢
        assert_eq!(first.cells, vec![0, 1, 2, 0, 1, 0]);
        assert_eq!(first.width, 3);
        assert_eq!(first.height, 2);
    }

    // 5.7 — empty palette rejected: new() -> Err(InvalidPalette), reason
    // contains "no colors", no panic. Only assert variant + keyword.
    #[test]
    fn empty_palette_rejected() {
        let palette = Palette {
            brand: "Empty".to_string(),
            colors: vec![],
        };
        let err = RgbMatcher::new(&palette).expect_err("empty palette must be rejected");
        match err {
            BeadError::InvalidPalette { reason } => {
                assert!(
                    reason.contains("no colors"),
                    "reason must mention no colors, got: {reason:?}"
                );
            }
            other => panic!("expected InvalidPalette, got {other:?}"),
        }
    }

    // 5.8 — oversized palette rejected at the true boundary: 65537 -> Err,
    // reason contains "more than"; 65536 -> Ok (index 65535 is representable).
    #[test]
    fn oversized_palette_rejected() {
        // ponytail: 6.5 万个 dummy String 仅此测试一次性分配，可接受
        let make = |n: usize| Palette {
            brand: "Big".to_string(),
            colors: (0..n)
                .map(|i| PaletteColor {
                    code: i.to_string(),
                    name: i.to_string(),
                    rgb: [0, 0, 0],
                })
                .collect(),
        };

        // 65537 colors -> rejected.
        let err = RgbMatcher::new(&make(65537)).expect_err("65537 colors must be rejected");
        match err {
            BeadError::InvalidPalette { reason } => {
                assert!(
                    reason.contains("more than"),
                    "reason must mention more than, got: {reason:?}"
                );
            }
            other => panic!("expected InvalidPalette, got {other:?}"),
        }

        // 65536 colors -> accepted (indices 0..=65535 all fit in u16).
        assert!(
            RgbMatcher::new(&make(65536)).is_ok(),
            "65536 colors must be accepted (u16::MAX == 65535)"
        );
    }

    // 5.9 — single-color palette: any pixel matches index 0, no panic.
    #[test]
    fn single_color_palette_ok() {
        let palette = palette_from(&[("ONLY", [123, 45, 67])]);
        let matcher = RgbMatcher::new(&palette).expect("valid palette");

        assert_eq!(matcher.find_best_match([0, 0, 0]), 0);
        assert_eq!(matcher.find_best_match([255, 255, 255]), 0);
        assert_eq!(matcher.find_best_match([123, 45, 67]), 0);
    }

    // ---- LabMatcher (Phase 3) -------------------------------------------------

    // 2.1 — sRGB->Lab known values: black -> L≈0, white -> L≈100 (both neutral),
    // and the standard sRGB red reference; all finite (no NaN for u8 input).
    #[test]
    fn lab_conversion_known_values() {
        let black = srgb_to_lab([0, 0, 0]);
        assert!(
            black[0].abs() < 0.01,
            "black L must be ~0, got {}",
            black[0]
        );
        assert!(black[1].abs() < 0.01 && black[2].abs() < 0.01);

        let white = srgb_to_lab([255, 255, 255]);
        assert!(
            (white[0] - 100.0).abs() < 0.05,
            "white L must be ~100, got {}",
            white[0]
        );
        assert!(white[1].abs() < 0.05 && white[2].abs() < 0.05);

        // standard sRGB red [255,0,0] -> Lab ≈ (53.24, 80.09, 67.20).
        let red = srgb_to_lab([255, 0, 0]);
        assert!((red[0] - 53.24).abs() < 0.5, "red L: {}", red[0]);
        assert!((red[1] - 80.09).abs() < 0.5, "red a: {}", red[1]);
        assert!((red[2] - 67.20).abs() < 0.5, "red b: {}", red[2]);

        // bounded u8 input -> every component finite.
        for v in black.iter().chain(white.iter()).chain(red.iter()) {
            assert!(v.is_finite(), "Lab component must be finite, got {v}");
        }
    }

    // 2.2 — exact hit maps to distance 0 (same RGB -> same Lab, same machine);
    // duplicate RGB -> lowest index wins (strict `<`).
    #[test]
    fn lab_exact_hit_and_duplicate_rgb_lowest_index() {
        let palette = palette_from(&[("A", [10, 20, 30]), ("B", [200, 100, 50]), ("C", [0, 0, 0])]);
        let matcher = LabMatcher::new(&palette).expect("valid palette");
        assert_eq!(matcher.find_best_match([10, 20, 30]), 0);
        assert_eq!(matcher.find_best_match([200, 100, 50]), 1);
        assert_eq!(matcher.find_best_match([0, 0, 0]), 2);

        let dup = palette_from(&[("DUP_A", [42, 42, 42]), ("DUP_B", [42, 42, 42])]);
        let m2 = LabMatcher::new(&dup).expect("valid palette");
        assert_eq!(m2.find_best_match([42, 42, 42]), 0);
    }

    // 2.3 — off-palette maps to the PERCEPTUALLY nearest color, which can differ
    // from RgbMatcher's pick (proving this is Lab matching, not an RGB alias).
    #[test]
    fn lab_off_palette_can_differ_from_rgb() {
        // index 0 = navy, index 1 = olive. Dark gray [40,40,40] is closer to navy
        // in RGB squared-Euclidean (d=10944 < 17088 -> index 0) but closer to
        // olive in Lab ΔE76² (d≈4657.6 < 6452.7 -> index 1): the matchers disagree.
        let palette = palette_from(&[("NAVY", [0, 0, 128]), ("OLIVE", [128, 128, 0])]);
        let lab = LabMatcher::new(&palette).expect("valid palette");
        let rgb = RgbMatcher::new(&palette).expect("valid palette");

        let target = [40u8, 40, 40];
        let lab_idx = lab.find_best_match(target);
        let rgb_idx = rgb.find_best_match(target);
        assert_eq!(rgb_idx, 0, "RgbMatcher picks navy on [40,40,40]");
        assert_eq!(
            lab_idx, 1,
            "LabMatcher picks olive (perceptually nearer) on [40,40,40]"
        );
        assert_ne!(
            lab_idx, rgb_idx,
            "LabMatcher must differ from RgbMatcher on a perceptual mismatch"
        );
    }

    // 2.4 — Lab tie returns the lowest index (two equal-Lab colors via shared
    // RGB tie at a nonzero distance), plus the construction guards mirror
    // RgbMatcher exactly (empty / 65537 rejected, 65536 accepted).
    #[test]
    fn lab_tie_lowest_index_and_guards() {
        // TIE_A / TIE_B share RGB -> identical Lab -> identical distance to the
        // target (which is nearer to them than to FAR). Strict `<` -> index 0.
        let palette = palette_from(&[
            ("TIE_A", [100, 150, 200]),
            ("TIE_B", [100, 150, 200]),
            ("FAR", [0, 0, 0]),
        ]);
        let matcher = LabMatcher::new(&palette).expect("valid palette");
        assert_eq!(matcher.find_best_match([105, 150, 200]), 0);
        // repeated calls identical (determinism gate).
        assert_eq!(matcher.find_best_match([105, 150, 200]), 0);

        // guards: empty palette rejected, reason contains "no colors".
        let empty = Palette {
            brand: "Empty".to_string(),
            colors: vec![],
        };
        match LabMatcher::new(&empty) {
            Err(BeadError::InvalidPalette { reason }) => assert!(
                reason.contains("no colors"),
                "reason must mention no colors, got: {reason:?}"
            ),
            other => panic!("expected InvalidPalette, got {other:?}"),
        }

        // boundary: 65537 rejected ("more than"), 65536 accepted.
        let make = |n: usize| Palette {
            brand: "Big".to_string(),
            colors: (0..n)
                .map(|i| PaletteColor {
                    code: i.to_string(),
                    name: i.to_string(),
                    rgb: [0, 0, 0],
                })
                .collect(),
        };
        match LabMatcher::new(&make(65537)) {
            Err(BeadError::InvalidPalette { reason }) => assert!(
                reason.contains("more than"),
                "reason must mention more than, got: {reason:?}"
            ),
            other => panic!("expected InvalidPalette, got {other:?}"),
        }
        assert!(
            LabMatcher::new(&make(65536)).is_ok(),
            "65536 colors must be accepted (u16::MAX == 65535)"
        );
    }

    // 2.5 — same-machine determinism: repeated match yields byte-equal cells.
    #[test]
    fn lab_match_pattern_deterministic_same_machine() {
        let palette = palette_from(&[
            ("BLACK", [0, 0, 0]),
            ("WHITE", [255, 255, 255]),
            ("RED", [255, 0, 0]),
            ("GREEN", [0, 255, 0]),
            ("BLUE", [0, 0, 255]),
        ]);
        let matcher = LabMatcher::new(&palette).expect("valid palette");
        let grid = PixelGrid {
            width: 3,
            height: 2,
            pixels: vec![
                [0, 0, 0],
                [255, 255, 255],
                [200, 40, 40],
                [40, 200, 40],
                [40, 40, 200],
                [123, 77, 200],
            ],
        };
        let first = match_pattern(&grid, &matcher);
        let second = match_pattern(&grid, &matcher);
        assert_eq!(
            first.cells, second.cells,
            "same-machine repeated match must be byte-identical"
        );
        assert_eq!(first, second);
    }

    // ---- OklabMatcher (Phase 3) ----------------------------------------------

    #[test]
    fn oklab_conversion_known_values() {
        let black = srgb_to_oklab([0, 0, 0]);
        assert!(
            black[0].abs() < 0.0001,
            "black L must be ~0, got {}",
            black[0]
        );
        assert!(black[1].abs() < 0.0001 && black[2].abs() < 0.0001);

        let white = srgb_to_oklab([255, 255, 255]);
        assert!(
            (white[0] - 1.0).abs() < 0.0001,
            "white L must be ~1, got {}",
            white[0]
        );
        assert!(white[1].abs() < 0.0001 && white[2].abs() < 0.0001);

        // standard sRGB red [255,0,0] -> Oklab ≈ (0.62796, 0.22486, 0.12585).
        let red = srgb_to_oklab([255, 0, 0]);
        assert!((red[0] - 0.627_96).abs() < 0.001, "red L: {}", red[0]);
        assert!((red[1] - 0.224_86).abs() < 0.001, "red a: {}", red[1]);
        assert!((red[2] - 0.125_85).abs() < 0.001, "red b: {}", red[2]);

        for v in black.iter().chain(white.iter()).chain(red.iter()) {
            assert!(v.is_finite(), "Oklab component must be finite, got {v}");
        }
    }

    #[test]
    fn oklab_exact_hit_and_duplicate_rgb_lowest_index() {
        let palette = palette_from(&[("A", [10, 20, 30]), ("B", [200, 100, 50]), ("C", [0, 0, 0])]);
        let matcher = OklabMatcher::new(&palette).expect("valid palette");
        assert_eq!(matcher.find_best_match([10, 20, 30]), 0);
        assert_eq!(matcher.find_best_match([200, 100, 50]), 1);
        assert_eq!(matcher.find_best_match([0, 0, 0]), 2);

        let dup = palette_from(&[("DUP_A", [42, 42, 42]), ("DUP_B", [42, 42, 42])]);
        let m2 = OklabMatcher::new(&dup).expect("valid palette");
        assert_eq!(m2.find_best_match([42, 42, 42]), 0);
    }

    #[test]
    fn oklab_tie_lowest_index_and_guards() {
        let palette = palette_from(&[
            ("TIE_A", [100, 150, 200]),
            ("TIE_B", [100, 150, 200]),
            ("FAR", [0, 0, 0]),
        ]);
        let matcher = OklabMatcher::new(&palette).expect("valid palette");
        assert_eq!(matcher.find_best_match([105, 150, 200]), 0);
        assert_eq!(matcher.find_best_match([105, 150, 200]), 0);

        let empty = Palette {
            brand: "Empty".to_string(),
            colors: vec![],
        };
        match OklabMatcher::new(&empty) {
            Err(BeadError::InvalidPalette { reason }) => assert!(
                reason.contains("no colors"),
                "reason must mention no colors, got: {reason:?}"
            ),
            other => panic!("expected InvalidPalette, got {other:?}"),
        }

        let make = |n: usize| Palette {
            brand: "Big".to_string(),
            colors: (0..n)
                .map(|i| PaletteColor {
                    code: i.to_string(),
                    name: i.to_string(),
                    rgb: [0, 0, 0],
                })
                .collect(),
        };
        match OklabMatcher::new(&make(65537)) {
            Err(BeadError::InvalidPalette { reason }) => assert!(
                reason.contains("more than"),
                "reason must mention more than, got: {reason:?}"
            ),
            other => panic!("expected InvalidPalette, got {other:?}"),
        }
        assert!(
            OklabMatcher::new(&make(65536)).is_ok(),
            "65536 colors must be accepted (u16::MAX == 65535)"
        );
    }

    #[test]
    fn oklab_differs_from_lab_in_blue() {
        let palette = palette_from(&[
            ("DEEP_BLUE", [0, 0, 180]),
            ("SAT_BLUE", [0, 0, 255]),
            ("BLUE_PURPLE", [80, 0, 210]),
            ("PURPLE", [120, 0, 180]),
            ("LAVENDER_BLUE", [90, 40, 230]),
        ]);
        let target = [0u8, 0, 200];
        let lab = LabMatcher::new(&palette).expect("valid palette");
        let oklab = OklabMatcher::new(&palette).expect("valid palette");

        let lab_idx = lab.find_best_match(target);
        let oklab_idx = oklab.find_best_match(target);
        assert_eq!(lab_idx, 2, "LabMatcher picks blue-purple");
        assert_eq!(oklab_idx, 0, "OklabMatcher picks deep blue");
        assert_ne!(
            oklab_idx, lab_idx,
            "OklabMatcher must differ from LabMatcher on this blue-region sample"
        );
    }

    #[test]
    fn matcher_kind_default_is_oklab() {
        assert_eq!(MatcherKind::default(), MatcherKind::Oklab);
    }
}
