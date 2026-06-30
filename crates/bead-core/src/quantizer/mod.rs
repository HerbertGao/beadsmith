//! Color quantization: reduce a `PixelGrid` to fewer distinct colors before
//! color matching. This is the optional Phase-2 stage (after `image_to_grid`,
//! before `match_pattern`); see design D1/D2.
//!
//! The [`Quantizer`] trait is the seam between quantizers, mirroring
//! [`crate::matcher::ColorMatcher`]: object-safe (`&dyn Quantizer` /
//! `Box<dyn Quantizer>`), configuration taken at `new`, value semantics
//! (immutable snapshot after construction). [`MedianCutQuantizer`] is the
//! Phase-2 RGB Median-Cut implementation — pure integer (no `f32`, no
//! `sqrt`), bit-identical across architectures (like `RgbMatcher`).

use crate::models::PixelGrid;
use crate::BeadError;

/// Reduces a `PixelGrid` to fewer distinct colors, producing a new
/// `PixelGrid` of the same shape (`width`/`height` carried over). This is the
/// optional stage before color matching (design D1). Must remain object-safe
/// — no generic methods, no `Self`-returning methods, no associated types in
/// signatures.
///
/// `quantize` is a total function: it does not return `Result` and does not
/// panic; an empty grid (`width == 0` or `height == 0` → `pixels.len() == 0`)
/// is returned verbatim.
pub trait Quantizer {
    /// Returns a new `PixelGrid` whose distinct-color count is bounded by the
    /// quantizer's configured maximum. Never panics.
    fn quantize(&self, grid: &PixelGrid) -> PixelGrid;
}

/// Phase 2 quantizer: RGB Median Cut, fully deterministic rules (design D2).
///
/// Holds the `max_colors` snapshot taken at construction. Because it is a
/// snapshot, the quantizer is immutable after `new` — the intended value
/// semantics (same as `RgbMatcher`).
#[derive(Debug)]
pub struct MedianCutQuantizer {
    max_colors: u32,
}

impl MedianCutQuantizer {
    /// Build a quantizer with the given color-count upper bound. Rejects
    /// `max_colors == 0` via [`BeadError::InvalidImage`] (`reason` mentions
    /// "max_colors"; reuses the zero-dimension variant — no new variant, design
    /// D1). Never panics.
    pub fn new(max_colors: u32) -> Result<MedianCutQuantizer, BeadError> {
        if max_colors == 0 {
            return Err(BeadError::InvalidImage {
                reason: "quantizer: max_colors must be >= 1, got 0".to_string(),
            });
        }
        Ok(MedianCutQuantizer { max_colors })
    }
}

impl Quantizer for MedianCutQuantizer {
    fn quantize(&self, grid: &PixelGrid) -> PixelGrid {
        let pixels = &grid.pixels;

        // Step 0 short-circuit (design D2): exact distinct-color count via
        // sort+dedup (deterministic, no HashMap/HashSet iteration order). If
        // d <= max_colors (incl. empty grid d==0), return the input verbatim —
        // a guaranteed no-op and the empty-grid safe path (no sum/count
        // division below).
        let d = distinct_color_count(pixels);
        if d <= self.max_colors {
            return grid.clone();
        }

        // Each bucket holds the row-major pixel indices it owns. The initial
        // bucket owns every pixel; `pixels` is non-empty here (d > max_colors >= 1).
        let mut buckets: Vec<Vec<usize>> = vec![(0..pixels.len()).collect()];

        while buckets.len() < self.max_colors as usize {
            // Find the splittable bucket + channel with the largest single-
            // channel spread (max - min). Tie → lower bucket index, then R < G < B.
            let (best_bi, best_ch) = match pick_split(pixels, &buckets) {
                Some(t) => t,
                None => break, // no splittable bucket remains
            };

            // Sort the selected bucket's pixels by a strict total-order key
            // `(selected-channel value, R, G, B, row-major index)` — the final
            // key is unique, so this is a true total order regardless of sort
            // stability (design D2 step 2b).
            let mut indices = buckets[best_bi].clone();
            indices.sort_by(|&a, &b| {
                let pa = &pixels[a];
                let pb = &pixels[b];
                (pa[best_ch], pa[0], pa[1], pa[2], a).cmp(&(pb[best_ch], pb[0], pb[1], pb[2], b))
            });

            // Split at the median index `len/2`: lower half [0, mid) replaces
            // bucket i in place, upper half [mid, len) is inserted at i+1 (the
            // remaining buckets shift right); bucket order stays deterministic
            // (design D2 step 2c).
            let mid = indices.len() / 2;
            let upper = indices.split_off(mid);
            buckets[best_bi] = indices;
            buckets.insert(best_bi + 1, upper);
        }

        // Representative color per bucket = per-channel mean with u64
        // accumulator and integer-truncating division (design D2 step 3).
        let reps: Vec<[u8; 3]> = buckets.iter().map(|b| representative(pixels, b)).collect();

        // Map each pixel to its bucket's representative, preserving row-major
        // order (design D2 step 4).
        let mut out_pixels: Vec<[u8; 3]> = vec![[0, 0, 0]; pixels.len()];
        for (bi, b) in buckets.iter().enumerate() {
            let rep = reps[bi];
            for &pi in b {
                out_pixels[pi] = rep;
            }
        }

        PixelGrid {
            width: grid.width,
            height: grid.height,
            pixels: out_pixels,
        }
    }
}

/// Exact distinct-color count via deterministic sort + dedup (no
/// `HashMap`/`HashSet` iteration order; design D2 step 0).
fn distinct_color_count(pixels: &[[u8; 3]]) -> u32 {
    let mut sorted = pixels.to_vec();
    sorted.sort_unstable();
    sorted.dedup();
    sorted.len() as u32
}

/// Among buckets with at least two pixels and a non-zero spread on some
/// channel, return the `(bucket_index, channel)` with the largest single-
/// channel spread; ties broken by lower bucket index then by R < G < B
/// (design D2 step 2a). Returns `None` when no splittable bucket remains.
fn pick_split(pixels: &[[u8; 3]], buckets: &[Vec<usize>]) -> Option<(usize, usize)> {
    let mut best_bi: Option<usize> = None;
    let mut best_ch: usize = 0;
    let mut best_sp: u32 = 0;
    for (bi, b) in buckets.iter().enumerate() {
        if b.len() < 2 {
            continue;
        }
        let spreads = channel_spreads(pixels, b);
        // Pick the channel with the largest spread; strict `>` keeps R < G < B
        // on a tie (R considered first).
        let mut ch = 0usize;
        let mut sp = spreads[0];
        if spreads[1] > sp {
            ch = 1;
            sp = spreads[1];
        }
        if spreads[2] > sp {
            ch = 2;
            sp = spreads[2];
        }
        if sp == 0 {
            // All pixels in this bucket are the same color — not splittable.
            continue;
        }
        // Larger spread wins; on a tie keep the earlier (lower-bucket) entry, so
        // bucket-index tiebreak is satisfied by strict `>` only.
        if best_bi.is_none() || sp > best_sp {
            best_bi = Some(bi);
            best_ch = ch;
            best_sp = sp;
        }
    }
    best_bi.map(|bi| (bi, best_ch))
}

/// Per-channel spread (`max - min`) across the pixels of a bucket, as `u32` to
/// avoid any subtraction edge case (max >= min always holds).
fn channel_spreads(pixels: &[[u8; 3]], bucket: &[usize]) -> [u32; 3] {
    let mut mn = [u8::MAX; 3];
    let mut mx = [u8::MIN; 3];
    for &pi in bucket {
        let px = &pixels[pi];
        for c in 0..3 {
            if px[c] < mn[c] {
                mn[c] = px[c];
            }
            if px[c] > mx[c] {
                mx[c] = px[c];
            }
        }
    }
    [
        mx[0] as u32 - mn[0] as u32,
        mx[1] as u32 - mn[1] as u32,
        mx[2] as u32 - mn[2] as u32,
    ]
}

/// Per-channel mean of a bucket's pixels: `sum: u64 / count` with integer
/// truncation (design D2 step 3). The bucket is guaranteed non-empty, so no
/// division by zero. `u64` accumulator guards against `255·N > u32::MAX` on
/// large grids.
fn representative(pixels: &[[u8; 3]], bucket: &[usize]) -> [u8; 3] {
    let mut sum = [0u64; 3];
    for &pi in bucket {
        let px = &pixels[pi];
        sum[0] += px[0] as u64;
        sum[1] += px[1] as u64;
        sum[2] += px[2] as u64;
    }
    let count = bucket.len() as u64;
    [
        (sum[0] / count) as u8,
        (sum[1] / count) as u8,
        (sum[2] / count) as u8,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    // 2.1 — fixed small grid with splittable colors, fixed max_colors →
    // hardcoded expected output; repeated calls identical (cross-arch bit-exact,
    // pure integer so the expected Vec is hardcoded).
    #[test]
    fn fixed_input_hardcoded_output() {
        let grid = PixelGrid {
            width: 2,
            height: 2,
            pixels: vec![[0, 0, 0], [10, 10, 10], [100, 100, 100], [255, 255, 255]],
        };
        let q = MedianCutQuantizer::new(2).expect("valid");

        let out = q.quantize(&grid);
        assert_eq!(out.width, 2);
        assert_eq!(out.height, 2);
        assert_eq!(out.pixels.len(), 4);

        // Bucket-1 split: all 4 pixels share R=G=B, spread R==255 on every
        // channel; R<G<B picks R, sort key (R,R,G,B,idx): [0,10,100,255], median
        // idx 2 -> lower {[0,0,0],[10,10,10]} (rep [5,5,5]) + upper
        // {[100,100,100],[255,255,255]} (rep [177,177,177]).
        assert_eq!(
            out.pixels,
            vec![[5, 5, 5], [5, 5, 5], [177, 177, 177], [177, 177, 177],]
        );

        let out2 = q.quantize(&grid);
        assert_eq!(out.pixels, out2.pixels);
    }

    // 2.2 — new(0) rejected with InvalidImage mentioning "max_colors"; new(>=1)
    // accepted; no panic.
    #[test]
    fn new_max_colors_validation() {
        let err = MedianCutQuantizer::new(0).expect_err("max_colors == 0 must be rejected");
        match err {
            BeadError::InvalidImage { reason } => assert!(
                reason.contains("max_colors"),
                "reason must mention max_colors, got: {reason:?}"
            ),
            other => panic!("expected InvalidImage, got {other:?}"),
        }
        assert!(MedianCutQuantizer::new(1).is_ok());
        assert!(MedianCutQuantizer::new(24).is_ok());
        assert!(MedianCutQuantizer::new(u32::MAX).is_ok());
    }

    // 2.3 — max_colors >= distinct color count (incl. far above) is an exact
    // no-op via step 0 short-circuit. Includes the skewed-distribution counter
    // example A×8, B×1 @ max_colors=4 (k=2): must still be pixel-identical —
    // without the short-circuit this case would change colors (median-index
    // splitting spends the budget peeling the A half, leaving a residual
    // [A,B] bucket whose mean shifts colors).
    #[test]
    fn no_op_when_max_colors_at_or_above_distinct() {
        let cases: Vec<(PixelGrid, u32)> = vec![(
            PixelGrid {
                width: 4,
                height: 2,
                pixels: vec![
                    [10, 10, 10],
                    [10, 10, 10],
                    [10, 10, 10],
                    [10, 10, 10],
                    [10, 10, 10],
                    [10, 10, 10],
                    [10, 10, 10],
                    [10, 10, 10],
                    [200, 200, 200],
                ],
            },
            4, // k=2 distinct colors, max_colors=4 >= k
        )];
        for (grid, mc) in cases {
            let q = MedianCutQuantizer::new(mc).expect("valid");
            let out = q.quantize(&grid);
            assert_eq!(out.pixels, grid.pixels, "must be pixel-identical no-op");
        }

        // explicit: 9-pixel grid A×8 + B×1, max_colors=4 (k=2). Verify the
        // counter-example is a true no-op (the case that would break "stop when
        // bucket collapses" emergence).
        let skewed = PixelGrid {
            width: 3,
            height: 3,
            pixels: vec![
                [10, 10, 10],
                [10, 10, 10],
                [10, 10, 10],
                [10, 10, 10],
                [10, 10, 10],
                [10, 10, 10],
                [10, 10, 10],
                [10, 10, 10],
                [200, 200, 200],
            ],
        };
        let q = MedianCutQuantizer::new(4).expect("valid");
        let out = q.quantize(&skewed);
        assert_eq!(
            out.pixels, skewed.pixels,
            "A×8,B×1 @ max_colors=4 must be no-op"
        );

        // above: max_colors == k as well, and far-above.
        let q_k = MedianCutQuantizer::new(2).expect("valid");
        assert_eq!(q_k.quantize(&skewed).pixels, skewed.pixels);
        let q_far = MedianCutQuantizer::new(1000).expect("valid");
        assert_eq!(q_far.quantize(&skewed).pixels, skewed.pixels);
    }

    // 2.4 — 1 <= max_colors < distinct color count → output distinct count
    // <= max_colors (upper-bound semantics, not "exactly N").
    #[test]
    fn upper_bound_when_n_below_distinct() {
        let grid = PixelGrid {
            width: 4,
            height: 1,
            pixels: vec![[0, 0, 0], [80, 80, 80], [160, 160, 160], [240, 240, 240]],
        };
        for mc in [1u32, 2, 3] {
            let q = MedianCutQuantizer::new(mc).expect("valid");
            let out = q.quantize(&grid);
            let mut cols: Vec<[u8; 3]> = out.pixels.clone();
            cols.sort();
            cols.dedup();
            assert!(
                cols.len() <= mc as usize,
                "max_colors={} expects distinct <= {}, got {}",
                mc,
                mc,
                cols.len()
            );
        }
    }

    // 2.5 — max_colors == 1 -> single bucket, whole-image mean color (legal,
    // no error). Empty grid (w==0 or h==0, empty pixels) returned verbatim, no
    // panic (no sum/count computed).
    #[test]
    fn single_bucket_mean_and_empty_grid_no_panic() {
        // max_colors == 1: single bucket = all pixels, mean color.
        let grid = PixelGrid {
            width: 2,
            height: 1,
            pixels: vec![[0, 0, 0], [255, 255, 255]],
        };
        let q = MedianCutQuantizer::new(1).expect("valid");
        let out = q.quantize(&grid);
        assert_eq!(out.width, 2);
        assert_eq!(out.height, 1);
        // mean = (0+255)/2 = 127 integer-truncated.
        assert_eq!(out.pixels, vec![[127, 127, 127], [127, 127, 127]]);

        // empty grid: width==0 (any height), pixels empty -> verbatim, no panic.
        let empty_w = PixelGrid {
            width: 0,
            height: 5,
            pixels: vec![],
        };
        let q = MedianCutQuantizer::new(8).expect("valid");
        let out_w = q.quantize(&empty_w);
        assert_eq!(out_w.width, 0);
        assert_eq!(out_w.height, 5);
        assert_eq!(out_w.pixels, Vec::<[u8; 3]>::new());

        // empty grid: height==0.
        let empty_h = PixelGrid {
            width: 5,
            height: 0,
            pixels: vec![],
        };
        let out_h = q.quantize(&empty_h);
        assert_eq!(out_h.width, 5);
        assert_eq!(out_h.height, 0);
        assert_eq!(out_h.pixels, Vec::<[u8; 3]>::new());
    }
}
