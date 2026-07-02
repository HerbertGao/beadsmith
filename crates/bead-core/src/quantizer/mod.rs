//! Palette-aware bead-color reduction: merge an already-matched `BeadPattern`
//! down to fewer distinct bead colors. This is the optional Phase-2 stage —
//! it runs **after** `match_pattern` (post-match, pattern→pattern), not before
//! it, so it only ever merges *existing* bead colors rather than inventing
//! intermediate colors on the pre-match pixel grid (design D1/D2).
//!
//! The reduction seam is [`BeadReducer`] (mirroring [`crate::matcher::ColorMatcher`]):
//! object-safe (`&dyn BeadReducer` / `Box<dyn BeadReducer>`), configuration
//! taken at `new`, value semantics (immutable snapshot after construction).
//! [`GreedyReducer`] is the greedy least-usage merge implementation; its
//! perceptual metric reuses the matcher's shared sRGB→space conversion
//! (`srgb_to_lab`/`srgb_to_oklab`, no local copy) and a byte-for-byte-isomorphic
//! squared-sum distance (design D3/D5).

use crate::matcher::{check_palette_len, srgb_to_lab, srgb_to_oklab, MatcherKind};
use crate::models::BeadPattern;
use crate::palette::Palette;
use crate::BeadError;

/// Reduces a `BeadPattern` to fewer distinct bead colors, producing a new
/// `BeadPattern` of the same shape (`width`/`height`/`cells.len()` carried
/// over). Must remain object-safe (`&dyn BeadReducer` / `Box<dyn BeadReducer>`)
/// — no generic methods, no `Self`-returning methods, no associated types.
///
/// `reduce` does **not** return `Result`. **Precondition** for the no-panic
/// guarantee (color-reduction spec / design D5): every `cells` value is a valid
/// palette index (`< palette.colors.len()` for the palette given to the
/// reducer's constructor) **and** the pattern was matched against that same
/// palette. An out-of-bounds index is a caller contract violation and **may
/// panic** (unlike `count_colors`, which skips out-of-range indices — `reduce`
/// must index a color snapshot, so it cannot meaningfully remap an out-of-range
/// cell). Within the precondition `reduce` is total. An empty pattern
/// (`cells.len() == 0`) is returned verbatim, never panicking.
pub trait BeadReducer {
    /// Returns a new `BeadPattern` whose distinct bead-color count is bounded by
    /// the reducer's configured maximum. Total under the precondition above.
    fn reduce(&self, pattern: &BeadPattern) -> BeadPattern;
}

/// Order-preserving snapshot of every palette color in the matcher's space,
/// taken at construction. Snapshot index `i` ≡ `palette.colors[i]`, carrying the
/// lowest-index tie rule (same value semantics as the matchers).
#[derive(Debug)]
enum ColorSnapshot {
    /// Integer RGB — cross-architecture bit-exact distance (like `RgbMatcher`).
    Rgb(Vec<[u8; 3]>),
    /// CIELAB or Oklab `f32` — same-machine deterministic (like `LabMatcher` /
    /// `OklabMatcher`). Both use the identical squared-sum formula.
    Perceptual(Vec<[f32; 3]>),
}

impl ColorSnapshot {
    fn len(&self) -> usize {
        match self {
            ColorSnapshot::Rgb(v) => v.len(),
            ColorSnapshot::Perceptual(v) => v.len(),
        }
    }

    /// Among the in-use colors (excluding `sac`), the perceptually nearest to
    /// `sac` by squared distance; the lowest palette index wins a tie (strict
    /// `<` while scanning ascending — same rule as `ColorMatcher`). At least one
    /// other in-use color is guaranteed by the caller, so a real index is
    /// returned. No `sqrt`; no `mul_add` on the `f32` path (design D3/D5, T4).
    fn nearest_target(&self, sac: usize, in_use: &[bool]) -> usize {
        let mut best_i = 0usize;
        match self {
            ColorSnapshot::Rgb(cols) => {
                let s = cols[sac];
                let mut best_d = u32::MAX;
                for (i, c) in cols.iter().enumerate() {
                    if i == sac || !in_use[i] {
                        continue;
                    }
                    // Widen to i32 before subtracting; accumulate in u32
                    // (max 3*255^2 = 195075 > u16). Pure integer, cross-arch
                    // bit-exact — same formula as `RgbMatcher` (design D3).
                    let dr = s[0] as i32 - c[0] as i32;
                    let dg = s[1] as i32 - c[1] as i32;
                    let db = s[2] as i32 - c[2] as i32;
                    let d = (dr * dr + dg * dg + db * db) as u32;
                    if d < best_d {
                        best_d = d;
                        best_i = i;
                    }
                }
            }
            ColorSnapshot::Perceptual(cols) => {
                let s = cols[sac];
                let mut best_d = f32::INFINITY;
                for (i, c) in cols.iter().enumerate() {
                    if i == sac || !in_use[i] {
                        continue;
                    }
                    // Sum of squared component diffs (= ΔE76² / ΔEok²); `√` is
                    // monotonic so squared distance preserves the argmin. Plain
                    // f32 ops, no mul_add/FMA (same as Lab/Oklab matchers).
                    let dl = s[0] - c[0];
                    let da = s[1] - c[1];
                    let db = s[2] - c[2];
                    let d = dl * dl + da * da + db * db;
                    if d < best_d {
                        best_d = d;
                        best_i = i;
                    }
                }
            }
        }
        best_i
    }
}

/// Palette-aware bead-color reducer: greedy least-usage merge (design D2).
///
/// Holds an order-preserving color snapshot in the matcher's space plus the
/// `max_colors` upper bound, both taken at construction — immutable after `new`
/// (value semantics, same as the matchers).
#[derive(Debug)]
pub struct GreedyReducer {
    snapshot: ColorSnapshot,
    max_colors: u32,
}

impl GreedyReducer {
    /// Build a reducer for `palette` in `matcher`'s color space with the given
    /// bead-color upper bound.
    ///
    /// Validation order is fixed (decides error priority, design D4): `max_colors
    /// == 0` → `Err(BeadError::InvalidImage)` (`reason` mentions "max_colors";
    /// reuses the zero-dimension variant, no new variant) **before** any palette
    /// check; only then is the palette size-validated via the matcher's shared
    /// [`check_palette_len`] (empty / > 65536 → `InvalidPalette`). The per-color
    /// coordinate snapshot reuses the matcher's shared sRGB→space conversion (no
    /// local copy). Never panics.
    pub fn new(
        palette: &Palette,
        matcher: MatcherKind,
        max_colors: u32,
    ) -> Result<GreedyReducer, BeadError> {
        if max_colors == 0 {
            return Err(BeadError::InvalidImage {
                reason: "reducer: max_colors must be >= 1, got 0".to_string(),
            });
        }
        check_palette_len(palette)?;
        let snapshot = match matcher {
            MatcherKind::Rgb => ColorSnapshot::Rgb(palette.colors.iter().map(|c| c.rgb).collect()),
            MatcherKind::Lab => ColorSnapshot::Perceptual(
                palette.colors.iter().map(|c| srgb_to_lab(c.rgb)).collect(),
            ),
            MatcherKind::Oklab => ColorSnapshot::Perceptual(
                palette
                    .colors
                    .iter()
                    .map(|c| srgb_to_oklab(c.rgb))
                    .collect(),
            ),
        };
        Ok(GreedyReducer {
            snapshot,
            max_colors,
        })
    }
}

impl BeadReducer for GreedyReducer {
    fn reduce(&self, pattern: &BeadPattern) -> BeadPattern {
        let n = self.snapshot.len();

        // Usage per palette index. An in-precondition `cells` value is `< n`;
        // an out-of-bounds index panics here (contract violation, design D5).
        let mut usage = vec![0u32; n];
        for &c in &pattern.cells {
            usage[c as usize] += 1;
        }

        // "In use" = usage > 0. `d` is the distinct bead-color count.
        let mut in_use: Vec<bool> = usage.iter().map(|&u| u > 0).collect();
        let mut d = in_use.iter().filter(|&&b| b).count() as u32;

        // short-circuit no-op (incl. empty pattern d==0): return verbatim.
        if d <= self.max_colors {
            return pattern.clone();
        }

        // `rep[i]` = current representative for original palette color `i`.
        // Merging chains collapse through it, so output stays valid indices.
        let mut rep: Vec<u16> = (0..n as u16).collect();

        while d > self.max_colors {
            // Sacrifice: least-used in-use color; tie → LARGER index. Scanning
            // ascending with `<=` lets the larger index win a usage tie.
            let mut sac = 0usize;
            let mut best_u = u32::MAX;
            for (i, &used) in in_use.iter().enumerate() {
                if used && usage[i] <= best_u {
                    best_u = usage[i];
                    sac = i;
                }
            }

            // Target: nearest remaining in-use color; tie → smaller index.
            let tgt = self.snapshot.nearest_target(sac, &in_use);

            // Merge sacrifice into target: repoint every color currently mapped
            // to `sac` (itself + anything earlier merged into it), fold usage.
            let sac16 = sac as u16;
            let tgt16 = tgt as u16;
            for r in rep.iter_mut() {
                if *r == sac16 {
                    *r = tgt16;
                }
            }
            usage[tgt] += usage[sac];
            usage[sac] = 0;
            in_use[sac] = false;
            d -= 1;
        }

        let cells: Vec<u16> = pattern.cells.iter().map(|&c| rep[c as usize]).collect();
        BeadPattern {
            width: pattern.width,
            height: pattern.height,
            cells,
        }
    }
}

#[cfg(test)]
mod reducer_tests {
    use super::*;
    use crate::matcher::{ColorMatcher, RgbMatcher};
    use crate::palette::PaletteColor;

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

    fn distinct(cells: &[u16]) -> Vec<u16> {
        let mut v = cells.to_vec();
        v.sort_unstable();
        v.dedup();
        v
    }

    // 2.2 — new(0) rejected with InvalidImage("max_colors") BEFORE palette
    // check; new(>=1) + valid palette Ok; new(>=1) + bad palette InvalidPalette.
    #[test]
    fn new_validation_order_max_colors_before_palette() {
        let good = palette_from(&[("A", [0, 0, 0]), ("B", [255, 255, 255])]);
        let empty = Palette {
            brand: "E".to_string(),
            colors: vec![],
        };

        // max_colors == 0 rejected even when palette is also illegal -> the
        // max_colors error wins (fixed priority).
        for pal in [&good, &empty] {
            let err = GreedyReducer::new(pal, MatcherKind::Rgb, 0)
                .expect_err("max_colors == 0 must be rejected");
            match err {
                BeadError::InvalidImage { reason } => assert!(
                    reason.contains("max_colors"),
                    "reason must mention max_colors, got: {reason:?}"
                ),
                other => panic!("expected InvalidImage, got {other:?}"),
            }
        }

        // valid: max_colors >= 1 + valid palette.
        assert!(GreedyReducer::new(&good, MatcherKind::Rgb, 1).is_ok());
        assert!(GreedyReducer::new(&good, MatcherKind::Oklab, 24).is_ok());

        // max_colors >= 1 but empty palette -> InvalidPalette ("no colors").
        match GreedyReducer::new(&empty, MatcherKind::Rgb, 4) {
            Err(BeadError::InvalidPalette { reason }) => assert!(
                reason.contains("no colors"),
                "reason must mention no colors, got: {reason:?}"
            ),
            other => panic!("expected InvalidPalette, got {other:?}"),
        }
    }

    // 5.1 — shape preserved; empty pattern is a verbatim no-op (no panic).
    #[test]
    fn shape_preserved_and_empty_no_op() {
        let pal = palette_from(&[
            ("A", [0, 0, 0]),
            ("B", [255, 255, 255]),
            ("C", [10, 20, 30]),
        ]);
        let reducer = GreedyReducer::new(&pal, MatcherKind::Rgb, 1).expect("valid");

        let pattern = BeadPattern {
            width: 3,
            height: 2,
            cells: vec![0, 1, 2, 0, 1, 2],
        };
        let out = reducer.reduce(&pattern);
        assert_eq!(out.width, 3);
        assert_eq!(out.height, 2);
        assert_eq!(out.cells.len(), 6);

        // empty pattern -> returned verbatim, no panic.
        let empty = BeadPattern {
            width: 0,
            height: 4,
            cells: vec![],
        };
        let out_empty = reducer.reduce(&empty);
        assert_eq!(out_empty.width, 0);
        assert_eq!(out_empty.height, 4);
        assert_eq!(out_empty.cells, Vec::<u16>::new());
    }

    // 5.2 — d <= max_colors natural no-op (cell-identical); 1 <= max_colors < d
    // upper bound holds; max_colors == 1 collapses to a single bead color.
    #[test]
    fn no_op_upper_bound_and_single() {
        let pal = palette_from(&[
            ("A", [0, 0, 0]),
            ("B", [80, 80, 80]),
            ("C", [160, 160, 160]),
            ("D", [240, 240, 240]),
        ]);
        // pattern uses 3 distinct beads (0,1,3); d == 3.
        let pattern = BeadPattern {
            width: 3,
            height: 2,
            cells: vec![0, 1, 3, 0, 1, 3],
        };

        // max_colors >= d -> cell-identical no-op (d==3).
        for mc in [3u32, 4, 100] {
            let r = GreedyReducer::new(&pal, MatcherKind::Rgb, mc).expect("valid");
            let out = r.reduce(&pattern);
            assert_eq!(out.cells, pattern.cells, "max_colors={mc} must be a no-op");
        }

        // 1 <= max_colors < d -> distinct count <= max_colors.
        for mc in [1u32, 2] {
            let r = GreedyReducer::new(&pal, MatcherKind::Rgb, mc).expect("valid");
            let out = r.reduce(&pattern);
            assert!(
                distinct(&out.cells).len() <= mc as usize,
                "max_colors={mc} expects distinct <= {mc}, got {}",
                distinct(&out.cells).len()
            );
        }

        // max_colors == 1 -> all cells one index.
        let r1 = GreedyReducer::new(&pal, MatcherKind::Rgb, 1).expect("valid");
        let out1 = r1.reduce(&pattern);
        assert_eq!(
            distinct(&out1.cells).len(),
            1,
            "max_colors==1 -> single bead"
        );
    }

    // full-image single color: d==1 short-circuits, verbatim, no panic.
    #[test]
    fn all_same_color_no_op() {
        let pal = palette_from(&[("A", [0, 0, 0]), ("B", [255, 255, 255])]);
        let pattern = BeadPattern {
            width: 2,
            height: 2,
            cells: vec![1, 1, 1, 1],
        };
        let r = GreedyReducer::new(&pal, MatcherKind::Oklab, 1).expect("valid");
        assert_eq!(r.reduce(&pattern).cells, vec![1, 1, 1, 1]);
    }

    // 5.3 — merges only flow between real beads: every output index appeared in
    // the input, and no palette-external color is produced.
    #[test]
    fn output_indices_subset_of_input() {
        let pal = palette_from(&[
            ("A", [0, 0, 0]),
            ("B", [8, 0, 0]),
            ("C", [255, 0, 0]),
            ("D", [247, 0, 0]),
        ]);
        let pattern = BeadPattern {
            width: 7,
            height: 1,
            cells: vec![0, 0, 0, 1, 2, 2, 3],
        };
        let input_used = distinct(&pattern.cells);
        let r = GreedyReducer::new(&pal, MatcherKind::Rgb, 2).expect("valid");
        let out = r.reduce(&pattern);
        for &c in &out.cells {
            assert!(
                input_used.contains(&c),
                "output index {c} not present in input {input_used:?}"
            );
            assert!((c as usize) < pal.colors.len(), "index {c} out of palette");
        }
    }

    // 5.4 + 5.5 — RGB cross-arch bit-exact hardcoded golden (the spec's two
    // concrete merge examples), plus determinism (repeated reduce byte-equal).
    #[test]
    fn rgb_golden_and_determinism() {
        // Example 1 (sacrifice tie -> larger index). c0=(0,0,0) c1=(8,0,0)
        // c2=(255,0,0) c3=(247,0,0), Rgb, max_colors=2.
        // cells=[0,0,0,1,2,2,3] -> [0,0,0,0,2,2,2].
        // ponytail: 整数度量跨架构位精确，可硬编码 golden（arm64 == x86_64）
        let pal1 = palette_from(&[
            ("c0", [0, 0, 0]),
            ("c1", [8, 0, 0]),
            ("c2", [255, 0, 0]),
            ("c3", [247, 0, 0]),
        ]);
        let p1 = BeadPattern {
            width: 7,
            height: 1,
            cells: vec![0, 0, 0, 1, 2, 2, 3],
        };
        let r1 = GreedyReducer::new(&pal1, MatcherKind::Rgb, 2).expect("valid");
        let out1 = r1.reduce(&p1);
        assert_eq!(out1.cells, vec![0, 0, 0, 0, 2, 2, 2]);
        assert_eq!(out1.width, 7);
        assert_eq!(out1.height, 1);
        // determinism: repeated reduce is byte-identical.
        assert_eq!(r1.reduce(&p1).cells, out1.cells);

        // Example 2 (target tie -> smaller index). c0=(0,0,0) c1=(10,0,0)
        // c2=(20,0,0), Rgb, max_colors=2. cells=[0,0,2,2,1] -> [0,0,2,2,0].
        let pal2 = palette_from(&[("c0", [0, 0, 0]), ("c1", [10, 0, 0]), ("c2", [20, 0, 0])]);
        let p2 = BeadPattern {
            width: 5,
            height: 1,
            cells: vec![0, 0, 2, 2, 1],
        };
        let r2 = GreedyReducer::new(&pal2, MatcherKind::Rgb, 2).expect("valid");
        let out2 = r2.reduce(&p2);
        assert_eq!(out2.cells, vec![0, 0, 2, 2, 0]);
        assert_eq!(r2.reduce(&p2).cells, out2.cells);
    }

    // 5.5 — Lab/Oklab path same-machine determinism (repeated reduce byte-equal).
    #[test]
    fn perceptual_same_machine_determinism() {
        let pal = palette_from(&[
            ("BLACK", [0, 0, 0]),
            ("WHITE", [255, 255, 255]),
            ("RED", [255, 0, 0]),
            ("GREEN", [0, 255, 0]),
            ("BLUE", [0, 0, 255]),
        ]);
        let pattern = BeadPattern {
            width: 3,
            height: 2,
            cells: vec![0, 1, 2, 3, 4, 2],
        };
        for kind in [MatcherKind::Lab, MatcherKind::Oklab] {
            let r = GreedyReducer::new(&pal, kind, 2).expect("valid");
            let first = r.reduce(&pattern);
            let second = r.reduce(&pattern);
            assert_eq!(first.cells, second.cells, "{kind:?} must be byte-stable");
            assert!(distinct(&first.cells).len() <= 2);
        }
    }

    // 5.6 — selection equivalence: the target the reducer picks for a sacrifice
    // among a set of retained colors equals `find_best_match` of the sacrifice's
    // RGB over a sub-palette holding exactly those retained colors (relative
    // order preserved so the lowest-index tie agrees across both). Guards
    // "reduce-nearest == match-nearest" against conversion drift. Only uses the
    // exposed `find_best_match`.
    #[test]
    fn reduce_nearest_equals_match_nearest() {
        // Full palette; sacrifice = index 4, retained = {0,1,2,3} (kept in
        // relative order). One merge (max_colors=4, d=5) forces exactly that
        // sacrifice (index 4 is least-used: usage 1 vs 2 for the rest) and the
        // reducer's target must equal find_best_match over the sub-palette.
        let colors: [(&str, [u8; 3]); 5] = [
            ("k0", [0, 0, 0]),
            ("k1", [60, 60, 60]),
            ("k2", [120, 120, 120]),
            ("k3", [200, 30, 30]),
            ("sac", [90, 88, 92]),
        ];
        let pal = palette_from(&colors);
        // usage: 0..3 each twice, index 4 once -> sacrifice is index 4.
        let pattern = BeadPattern {
            width: 3,
            height: 3,
            cells: vec![0, 0, 1, 1, 2, 2, 3, 3, 4],
        };
        let sub = palette_from(&colors[0..4]); // retained, relative order preserved
        let sac_rgb = colors[4].1;

        for kind in [MatcherKind::Rgb, MatcherKind::Lab, MatcherKind::Oklab] {
            let r = GreedyReducer::new(&pal, kind, 4).expect("valid");
            let out = r.reduce(&pattern);
            // sacrifice (index 4) is gone; the cell it occupied now holds the
            // reducer's chosen target.
            let reducer_target = out.cells[8];
            assert!(reducer_target < 4, "target must be a retained color");

            // find_best_match over the sub-palette, same matcher kind.
            let sub_target = match kind {
                MatcherKind::Rgb => RgbMatcher::new(&sub).unwrap().find_best_match(sac_rgb),
                MatcherKind::Lab => crate::matcher::LabMatcher::new(&sub)
                    .unwrap()
                    .find_best_match(sac_rgb),
                MatcherKind::Oklab => crate::matcher::OklabMatcher::new(&sub)
                    .unwrap()
                    .find_best_match(sac_rgb),
            };
            assert_eq!(
                reducer_target, sub_target,
                "{kind:?}: reduce target {reducer_target} must equal find_best_match {sub_target}"
            );
        }
    }
}
