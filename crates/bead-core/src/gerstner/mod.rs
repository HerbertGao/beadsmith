//! Gerstner superpixel generation — an opt-in, deterministic SLIC-variant front
//! end that jointly downsamples + assigns a cropped source image into a `w×h`
//! bead grid, then snaps each cell's Oklab cluster centroid to the fixed palette
//! (see the `gerstner-superpixel` spec, the authoritative determinism contract).
//!
//! Determinism is a hard gate. Every rule below is byte-for-byte reproducible on
//! the same machine (f32 → same-machine canonical, like `OklabMatcher`): real
//! per-axis step, explicit round-0 centroids, original-grid-anchored candidate
//! sets, snapshot-style assign-then-update, fixed row-major f32 accumulation
//! order, fixed tie-break (smallest seed index), and exactly `T` rounds. No
//! randomness, no `rayon`, no `HashMap`/`HashSet` iteration-order leak, no
//! `mul_add`/FMA.

use crate::matcher::{check_palette_len, srgb_to_oklab};
use crate::models::BeadPattern;
use crate::palette::Palette;
use crate::BeadError;

/// Which generation front end `generate_pattern` uses (the branch itself lives
/// in `pipeline`). `Staged` is the default staged path (`image_to_grid` →
/// matcher); `Gerstner` is the opt-in superpixel path (this module).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GeneratorKind {
    /// Default staged path (crop → resize → per-pixel palette match).
    #[default]
    Staged,
    /// Opt-in Gerstner superpixel path (deterministic SLIC + Oklab snap).
    Gerstner,
}

/// Compactness weight `m` on the spatial term (fixed compile-time constant,
/// visually tuned — not a runtime input, not in the CLI). Squared into the
/// distance as `m²·((Δx/S_x)² + (Δy/S_y)²)`.
const M: f32 = 0.1;

/// Fixed iteration count `T` (compile-time constant). Exactly `T` assign+update
/// rounds run — no residual/threshold early stop (a determinism requirement).
const T: usize = 10;

/// Gerstner front end: assign the already-cropped source `img` (`W×H`) into a
/// `width×height` bead grid via a deterministic SLIC variant, then snap each
/// cell's Oklab cluster centroid to the fixed `palette`, producing a full-board
/// `BeadPattern` (`cells.len() == width*height`, every cell a legal palette
/// index — never invents an intermediate color).
///
/// Cropping to the target aspect ratio is the pipeline's job (`crop_center`);
/// this takes the cropped source. Returns `Err(BeadError::InvalidImage)` on the
/// upsampling guard (`W < width || H < height`, i.e. `S < 1`), and
/// `Err(BeadError::InvalidPalette)` on an empty/oversized palette (same guard as
/// the matchers). Never panics.
pub(crate) fn superpixel_assign(
    img: &::image::RgbImage,
    palette: &Palette,
    width: u32,
    height: u32,
) -> Result<BeadPattern, BeadError> {
    check_palette_len(palette)?;

    let big_w = img.width();
    let big_h = img.height();

    // Upsampling guard: Gerstner requires target ≤ source (S_x, S_y >= 1). A
    // target larger than the cropped source (S < 1) would collapse many seeds
    // onto the same pixel and degenerate the window — reject, do NOT enter the
    // degenerate path (the Staged path is unaffected and may still upscale).
    if big_w < width || big_h < height {
        return Err(BeadError::InvalidImage {
            reason: format!(
                "Gerstner requires target <= source (S >= 1): target {width}x{height} exceeds cropped source {big_w}x{big_h}"
            ),
        });
    }
    // width/height > 0 is implied here: big_w >= width and a valid decoded image
    // has big_w >= 1, so width >= 1 only when width <= big_w; but width == 0
    // would make width <= big_w trivially true. The pipeline validates target
    // dimensions before calling (mirrors image_to_grid); still, guard defensively
    // so w*h > 0 and the grids below are non-empty.
    if width == 0 || height == 0 {
        return Err(BeadError::InvalidImage {
            reason: "Gerstner target width or height is 0".to_string(),
        });
    }

    let w = width as usize;
    let h = height as usize;
    let big_w_us = big_w as usize;
    let big_h_us = big_h as usize;
    let seed_count = w * h;

    // Real, per-axis step (f32, NOT rounded). Guard above guarantees S >= 1.
    let s_x = big_w as f32 / width as f32;
    let s_y = big_h as f32 / height as f32;

    // One-time source-pixel Oklab snapshot, row-major (`px_lab[y*W + x]`). Reuses
    // the crate's canonical srgb_to_oklab (same conversion as the matchers).
    let px_lab: Vec<[f32; 3]> = img.pixels().map(|p| srgb_to_oklab(p.0)).collect();

    // Seed state, indexed by seed k = j*w + i (row-major, one per cell):
    //   centroid_lab[k] — Oklab centroid; centroid_pos[k] — ORIGINAL source pixel
    //   coords (not normalized; the distance normalizes Δ by S later).
    let mut centroid_lab: Vec<[f32; 3]> = vec![[0.0; 3]; seed_count];
    let mut centroid_pos: Vec<[f32; 2]> = vec![[0.0; 2]; seed_count];

    // Round-0 centroid: seed (i,j) center ((i+0.5)S_x,(j+0.5)S_y) → nearest
    // integer pixel (f32::round, ties away from zero) → clamp to [0,W)×[0,H) →
    // that pixel's Oklab + its (original) position.
    for j in 0..h {
        for i in 0..w {
            let k = j * w + i;
            let cx = (i as f32 + 0.5) * s_x;
            let cy = (j as f32 + 0.5) * s_y;
            let px = (cx.round() as i64).clamp(0, big_w as i64 - 1) as usize;
            let py = (cy.round() as i64).clamp(0, big_h as i64 - 1) as usize;
            centroid_lab[k] = px_lab[py * big_w_us + px];
            centroid_pos[k] = [px as f32, py as f32];
        }
    }

    // Assignment buffer (seed index per source pixel), reused each round.
    let mut assign: Vec<usize> = vec![0; big_w_us * big_h_us];

    for _round in 0..T {
        // --- Full assignment (reads the current/previous-round centroid
        // snapshot; never mutates a centroid mid-pass). ---
        for y in 0..big_h_us {
            for x in 0..big_w_us {
                let pidx = y * big_w_us + x;
                let lab = px_lab[pidx];

                // Original-grid home cell (clamped) — fixed, independent of seed
                // drift, so coverage never fails: the home seed is always a
                // candidate, so every pixel has >= 1 candidate.
                let gi = ((x as f32 / s_x).floor() as i64).clamp(0, w as i64 - 1);
                let gj = ((y as f32 / s_y).floor() as i64).clamp(0, h as i64 - 1);

                let mut best_d = f32::INFINITY;
                let mut best_k = gj as usize * w + gi as usize;
                // Iterate dj outer (-1..=1), di inner (-1..=1): candidate seed
                // indices nk = nj*w+ni are visited in strictly increasing order,
                // so strict `<` picks the smallest seed index on a tie.
                for dj in -1i64..=1 {
                    let nj = gj + dj;
                    if nj < 0 || nj >= h as i64 {
                        continue;
                    }
                    for di in -1i64..=1 {
                        let ni = gi + di;
                        if ni < 0 || ni >= w as i64 {
                            continue;
                        }
                        let nk = nj as usize * w + ni as usize;
                        let c = centroid_lab[nk];
                        let dl = lab[0] - c[0];
                        let da = lab[1] - c[1];
                        let db = lab[2] - c[2];
                        let dpx = (x as f32 - centroid_pos[nk][0]) / s_x;
                        let dpy = (y as f32 - centroid_pos[nk][1]) / s_y;
                        // Distance = ΔOklab² + m²·((Δx/S_x)²+(Δy/S_y)²). Plain f32
                        // ops, no mul_add. Not sqrt-ed (monotonic; preserves
                        // argmin).
                        let d = dl * dl + da * da + db * db + M * M * (dpx * dpx + dpy * dpy);
                        if d < best_d {
                            best_d = d;
                            best_k = nk;
                        }
                    }
                }
                assign[pidx] = best_k;
            }
        }

        // --- Full centroid update (row-major single pass, fixed f32 accumulation
        // order; Vec-indexed accumulators, no HashMap). Empty clusters keep their
        // previous centroid. ---
        let mut sum_lab: Vec<[f32; 3]> = vec![[0.0; 3]; seed_count];
        let mut sum_pos: Vec<[f32; 2]> = vec![[0.0; 2]; seed_count];
        let mut count: Vec<u32> = vec![0; seed_count];
        for y in 0..big_h_us {
            for x in 0..big_w_us {
                let pidx = y * big_w_us + x;
                let k = assign[pidx];
                let lab = px_lab[pidx];
                sum_lab[k][0] += lab[0];
                sum_lab[k][1] += lab[1];
                sum_lab[k][2] += lab[2];
                sum_pos[k][0] += x as f32;
                sum_pos[k][1] += y as f32;
                count[k] += 1;
            }
        }
        for k in 0..seed_count {
            let n = count[k];
            if n > 0 {
                let nf = n as f32;
                centroid_lab[k] = [sum_lab[k][0] / nf, sum_lab[k][1] / nf, sum_lab[k][2] / nf];
                centroid_pos[k] = [sum_pos[k][0] / nf, sum_pos[k][1] / nf];
            }
            // else: empty cluster keeps its previous-round centroid (unchanged).
        }
    }

    // Palette Oklab snapshot (order-preserving; same conversion + tie rule as
    // OklabMatcher, but the argmin input is an Oklab centroid, not RGB — so it is
    // a distinct Oklab-coordinate argmin, not `find_best_match`).
    let pal_lab: Vec<[f32; 3]> = palette
        .colors
        .iter()
        .map(|c| srgb_to_oklab(c.rgb))
        .collect();

    // Each cell's representative color = its cluster's final (round-T) Oklab
    // centroid → nearest palette color (ΔEok² argmin, strict `<`, lowest index on
    // a tie). Cells are laid out row-major (k = j*w+i), matching BeadPattern.
    let cells: Vec<u16> = (0..seed_count)
        .map(|k| oklab_argmin(centroid_lab[k], &pal_lab))
        .collect();

    Ok(BeadPattern {
        width,
        height,
        cells,
    })
}

/// Nearest palette color to an Oklab coordinate: ΔEok² (`(ΔL)²+(Δa)²+(Δb)²`, not
/// sqrt-ed), strict `<` so the lowest index wins on a tie — same rule as
/// `OklabMatcher`, but keyed on an Oklab coordinate rather than RGB. `pal_lab` is
/// non-empty (guarded by `check_palette_len`) and ≤ 65536 long, so `best` is
/// always set and fits in `u16`.
fn oklab_argmin(target: [f32; 3], pal_lab: &[[f32; 3]]) -> u16 {
    let mut best_i: usize = 0;
    let mut best_d: f32 = f32::INFINITY;
    for (i, c) in pal_lab.iter().enumerate() {
        let dl = target[0] - c[0];
        let da = target[1] - c[1];
        let db = target[2] - c[2];
        let d = dl * dl + da * da + db * db;
        if d < best_d {
            best_d = d;
            best_i = i;
        }
    }
    best_i as u16
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::palette::PaletteColor;
    use ::image::{Rgb, RgbImage};

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

    /// A small synthetic RGB image: left half red, right half blue (hand-built,
    /// no binary fixture).
    fn two_color_img(w: u32, h: u32) -> RgbImage {
        RgbImage::from_fn(w, h, |x, _| {
            if x < w / 2 {
                Rgb([200, 20, 20])
            } else {
                Rgb([20, 20, 200])
            }
        })
    }

    /// A diagonal gradient image (distinct per-pixel colors → forces seed drift
    /// across rounds).
    fn gradient_img(w: u32, h: u32) -> RgbImage {
        RgbImage::from_fn(w, h, |x, y| {
            Rgb([(x % 256) as u8, (y % 256) as u8, ((x + y) % 256) as u8])
        })
    }

    fn rgb_bw_palette() -> Palette {
        palette_from(&[
            ("R", [255, 0, 0]),
            ("G", [0, 255, 0]),
            ("B", [0, 0, 255]),
            ("K", [0, 0, 0]),
            ("W", [255, 255, 255]),
        ])
    }

    // 6.1 — same shape: width/height carried over, cells.len() == w*h, every cell
    // a legal palette index.
    #[test]
    fn gerstner_same_shape_and_legal_indices() {
        let img = two_color_img(16, 16);
        let palette = rgb_bw_palette();
        let (w, h) = (4u32, 4u32);
        let pat = superpixel_assign(&img, &palette, w, h).expect("must generate");
        assert_eq!(pat.width, w);
        assert_eq!(pat.height, h);
        assert_eq!(pat.cells.len() as u32, w * h);
        assert!(pat
            .cells
            .iter()
            .all(|&c| (c as usize) < palette.colors.len()));
    }

    // 6.2 — same-machine repeated generation is byte-for-byte equal.
    #[test]
    fn gerstner_repeat_byte_identical() {
        let img = gradient_img(20, 24);
        let palette = rgb_bw_palette();
        let (w, h) = (5u32, 6u32);
        let a = superpixel_assign(&img, &palette, w, h).expect("first");
        let b = superpixel_assign(&img, &palette, w, h).expect("second");
        assert_eq!(
            a, b,
            "same-machine repeated Gerstner must be byte-identical"
        );
    }

    // 6.3 — every output color is inside the palette (no off-board color).
    #[test]
    fn gerstner_colors_in_palette() {
        let img = gradient_img(24, 24);
        let palette = rgb_bw_palette();
        let pat = superpixel_assign(&img, &palette, 6, 6).expect("must generate");
        assert!(pat
            .cells
            .iter()
            .all(|&c| (c as usize) < palette.colors.len()));
    }

    // 6.6 — upsampling guard: target > source (S < 1) → Err(InvalidImage) whose
    // reason names the Gerstner constraint; never panics.
    #[test]
    fn gerstner_upsampling_rejected() {
        let img = two_color_img(4, 4);
        let palette = rgb_bw_palette();
        // target 8x8 > source 4x4 → S < 1.
        let err = superpixel_assign(&img, &palette, 8, 8).expect_err("upsampling must reject");
        match err {
            BeadError::InvalidImage { reason } => {
                assert!(
                    reason.contains("Gerstner") && reason.contains("target"),
                    "reason must name the Gerstner target/source constraint, got: {reason:?}"
                );
            }
            other => panic!("expected InvalidImage, got {other:?}"),
        }
        // one-axis upsampling also rejects (H < h).
        assert!(matches!(
            superpixel_assign(&img, &palette, 4, 8),
            Err(BeadError::InvalidImage { .. })
        ));
    }

    // 6.7 — every source pixel is assigned even after seeds drift (T>1 on a
    // gradient): output has no undefined cell (all legal indices), and it is
    // reproducible (fixed tie-break → smallest seed index).
    #[test]
    fn gerstner_every_pixel_assigned_after_drift() {
        // A gradient with distinct pixels forces seeds to move across the T
        // rounds; the original-grid-anchored candidate set must still cover every
        // pixel, so no cell is left undefined.
        let img = gradient_img(30, 18);
        let palette = rgb_bw_palette();
        let (w, h) = (5u32, 3u32);
        let pat = superpixel_assign(&img, &palette, w, h).expect("must generate");
        assert_eq!(pat.cells.len() as u32, w * h);
        assert!(
            pat.cells
                .iter()
                .all(|&c| (c as usize) < palette.colors.len()),
            "no undefined/out-of-board cell after seed drift"
        );
        // reproducible (deterministic tie-break).
        let again = superpixel_assign(&img, &palette, w, h).expect("again");
        assert_eq!(pat, again);
    }

    // Palette guard: empty palette rejected with InvalidPalette (shared
    // check_palette_len), before any assignment.
    #[test]
    fn gerstner_empty_palette_rejected() {
        let img = two_color_img(8, 8);
        let empty = Palette {
            brand: "Empty".to_string(),
            colors: vec![],
        };
        match superpixel_assign(&img, &empty, 4, 4) {
            Err(BeadError::InvalidPalette { reason }) => {
                assert!(reason.contains("no colors"), "got: {reason:?}")
            }
            other => panic!("expected InvalidPalette, got {other:?}"),
        }
    }

    // The two-color fixture snaps left cells to red and right cells to blue — the
    // clusters resolve to the perceptually nearest palette bead, proving the
    // Oklab snap is spatially coherent (not scrambled).
    #[test]
    fn gerstner_two_color_snaps_left_red_right_blue() {
        let img = two_color_img(16, 8);
        let palette = rgb_bw_palette(); // R=0, B=2
        let (w, h) = (4u32, 2u32);
        let pat = superpixel_assign(&img, &palette, w, h).expect("must generate");
        // left column of cells -> red (index 0), right column -> blue (index 2).
        for j in 0..h as usize {
            let left = pat.cells[j * w as usize];
            let right = pat.cells[j * w as usize + (w as usize - 1)];
            assert_eq!(left, 0, "leftmost cell should snap to red");
            assert_eq!(right, 2, "rightmost cell should snap to blue");
        }
    }
}
