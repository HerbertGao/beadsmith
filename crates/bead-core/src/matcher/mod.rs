//! Color matching: map raw RGB cells to palette indices. Pure integer math —
//! no `f32`, no `sqrt`, no `rayon`. This is the single hand-off from
//! `PixelGrid` (raw RGB) to `BeadPattern` (palette indices); see design D5/D6.
//!
//! The [`ColorMatcher`] trait is the seam for future matchers (Phase 2's
//! CIELAB/ΔE matcher is the known second implementation); [`RgbMatcher`] is the
//! Phase 1 RGB squared-Euclidean implementation. `ColorMatcher` must stay
//! object-safe (D2): it is used as `&dyn ColorMatcher` here and `Box<dyn>` in
//! the M6 pipeline.

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
    /// Build a matcher from a one-time, order-preserving palette snapshot.
    ///
    /// Rejects two cases (design D7; reuses `InvalidPalette`, no new variant):
    /// - empty `colors` → `InvalidPalette` (`reason` contains "no colors"),
    /// - `colors.len() > 65536` → `InvalidPalette` (`reason` contains "more
    ///   than"), guarding against `index as u16` silently truncating. The
    ///   boundary is exact: legal indices are `0..=65535` (`u16::MAX == 65535`),
    ///   so `len == 65536` is accepted; the first overflowing length is 65537.
    ///
    /// Never panics.
    pub fn new(palette: &Palette) -> Result<RgbMatcher, BeadError> {
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
}
