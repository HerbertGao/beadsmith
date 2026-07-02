//! Spatial post-processing on a matched `BeadPattern` — the `pattern-cleanup`
//! capability family (`BeadPattern` in, `BeadPattern` out, same shape). Its
//! first member is [`despeckle`]: connected-component despeckling that merges
//! small same-color regions into their majority neighbor color.
//!
//! `despeckle` is a **library/reuse primitive, not a generation entry point**
//! (CLAUDE rule 4): `pipeline::generate_pattern` may call it as an optional
//! post-reduction stage, but it never re-does orchestration. Unlike
//! [`crate::quantizer::BeadReducer`] (which reduces the *color count* and needs
//! a palette + perceptual space), despeckle is a **spatial** operation — it
//! never touches color coordinates, needs no palette, and `min_region == 0` is
//! a meaningful no-op. A single implementation with no second axis of variation
//! does not earn a trait (design D2); it stays a free function.
//!
//! Determinism (design D3/D4): pure integer (`u16` indices, `u32`/`usize`
//! counts), no floats, no `rayon`, no randomness, no `HashMap`/`HashSet`
//! iteration-order leak. Fixed row-major scan, fixed 4-connectivity, fixed
//! per-edge tally, ties broken by lowest palette index → **cross-architecture
//! bit-exact**.

use crate::models::BeadPattern;

/// Remove small same-color 4-connected components from a `BeadPattern`, merging
/// each one into its **majority neighbor color**, and return a new pattern of
/// the same shape.
///
/// `min_region` is the largest component bead count that triggers cleanup:
/// every same-color 4-connected component with `bead count <= min_region` is
/// remapped; larger components are left untouched. `min_region == 0` is a
/// no-op (no component ever has zero beads).
///
/// **Input-snapshot semantics (order-independent, single pass).** Component
/// discovery, each component's bead count, and the majority-neighbor tally are
/// all read from the input `pattern`; every remap is written to an independent
/// output buffer. A prior component's remap therefore never affects a later
/// component's tally, so the output is determined solely by the input topology
/// and is independent of scan order.
///
/// **Majority neighbor = per-boundary-edge tally.** For each cell in a
/// candidate component, each of its 4-neighbors whose color differs (i.e. lies
/// outside the component) casts one vote for that neighbor's color; the same
/// neighbor color touched by several edges accumulates several votes. The color
/// with the most votes wins; ties go to the **lowest palette index**. Same-color
/// neighbors are inside the component by definition and never vote. The merge
/// target is therefore always an **existing adjacent bead color** — despeckle
/// never invents a color.
///
/// Total under the same precondition as `match_pattern`'s output (`cells` are
/// valid palette indices): it never panics on legal input. An **empty pattern**
/// (`cells.len() == 0`) — or a malformed shape (`width == 0` / `height == 0` /
/// `cells.len() != width * height`) — is returned verbatim. A component with
/// **no differently-colored neighbor** (only possible when the whole image is
/// one color) has no target and is left as-is.
pub fn despeckle(pattern: &BeadPattern, min_region: u32) -> BeadPattern {
    let width = pattern.width as usize;
    let height = pattern.height as usize;
    let cells = &pattern.cells;
    let n = cells.len();

    if n == 0 {
        return pattern.clone();
    }
    // Shape guard before the index math below (`idx % width`, neighbor offsets):
    // a malformed public input (`width == 0`/`height == 0`, or `cells.len() !=
    // width * height`) is returned verbatim rather than panicking. Pipeline
    // patterns always satisfy this; the guard defends direct `pub` callers.
    if width == 0 || height == 0 || width.checked_mul(height) != Some(n) {
        return pattern.clone();
    }

    // Output buffer is an independent copy of the INPUT snapshot: discovery and
    // tallies keep reading `cells` while remaps write `out` (input-snapshot
    // semantics → order-independent).
    let mut out = cells.clone();
    let mut visited = vec![false; n];
    // Explicit stack for the flood-fill — no recursion, so no deep-stack risk on
    // a large single-color region.
    let mut stack: Vec<usize> = Vec::new();
    let mut component: Vec<usize> = Vec::new();

    for start in 0..n {
        if visited[start] {
            continue;
        }
        let color = cells[start];

        // Discover the same-color 4-connected component containing `start`.
        component.clear();
        stack.clear();
        stack.push(start);
        visited[start] = true;
        while let Some(idx) = stack.pop() {
            component.push(idx);
            let x = idx % width;
            let y = idx / width;
            if x > 0 {
                push_if(cells, &mut visited, &mut stack, idx - 1, color);
            }
            if x + 1 < width {
                push_if(cells, &mut visited, &mut stack, idx + 1, color);
            }
            if y > 0 {
                push_if(cells, &mut visited, &mut stack, idx - width, color);
            }
            if y + 1 < height {
                push_if(cells, &mut visited, &mut stack, idx + width, color);
            }
        }

        // Only components at or below the threshold are cleanup candidates.
        if component.len() as u64 > u64::from(min_region) {
            continue;
        }

        if let Some(target) = majority_neighbor(cells, width, height, color, &component) {
            for &idx in &component {
                out[idx] = target;
            }
        }
        // No differently-colored neighbor → nothing to merge into; leave as-is.
    }

    BeadPattern {
        width: pattern.width,
        height: pattern.height,
        cells: out,
    }
}

/// Mark `ni` visited and enqueue it when it is unvisited and the same `color`
/// (4-connected flood-fill step, read from the input snapshot).
fn push_if(cells: &[u16], visited: &mut [bool], stack: &mut Vec<usize>, ni: usize, color: u16) {
    if !visited[ni] && cells[ni] == color {
        visited[ni] = true;
        stack.push(ni);
    }
}

/// Tally per-boundary-edge votes for the component's differently-colored
/// neighbors and return the winner (most votes; lowest palette index on a tie),
/// or `None` when the component has no differently-colored neighbor.
fn majority_neighbor(
    cells: &[u16],
    width: usize,
    height: usize,
    color: u16,
    component: &[usize],
) -> Option<u16> {
    // (neighbor color, votes). A `Vec` — not a hash map — keeps this free of any
    // iteration-order leak; a small component touches only a few distinct colors.
    let mut votes: Vec<(u16, u32)> = Vec::new();
    for &idx in component {
        let x = idx % width;
        let y = idx / width;
        if x > 0 {
            cast_vote(&mut votes, cells[idx - 1], color);
        }
        if x + 1 < width {
            cast_vote(&mut votes, cells[idx + 1], color);
        }
        if y > 0 {
            cast_vote(&mut votes, cells[idx - width], color);
        }
        if y + 1 < height {
            cast_vote(&mut votes, cells[idx + width], color);
        }
    }
    // Winner: most votes, ties to the lowest palette index. Deterministic
    // regardless of the order colors were first seen.
    votes
        .into_iter()
        .reduce(|best, cur| {
            if cur.1 > best.1 || (cur.1 == best.1 && cur.0 < best.0) {
                cur
            } else {
                best
            }
        })
        .map(|(c, _)| c)
}

/// Cast one vote for `nc` when it differs from the component's `color`
/// (same-color neighbors are inside the component and never vote).
fn cast_vote(votes: &mut Vec<(u16, u32)>, nc: u16, color: u16) {
    if nc == color {
        return;
    }
    match votes.iter_mut().find(|(c, _)| *c == nc) {
        Some(entry) => entry.1 += 1,
        None => votes.push((nc, 1)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Transpose a pattern (swap axes). 4-connectivity and neighbor colors are
    /// preserved, so despeckling the transpose and transposing back simulates a
    /// column-major scan — used to prove scan-order independence (5.4).
    fn transpose(p: &BeadPattern) -> BeadPattern {
        let w = p.width as usize;
        let h = p.height as usize;
        let mut cells = vec![0u16; p.cells.len()];
        for y in 0..h {
            for x in 0..w {
                cells[x * h + y] = p.cells[y * w + x];
            }
        }
        BeadPattern {
            width: p.height,
            height: p.width,
            cells,
        }
    }

    // 5.1 — a single off-color cell 4-surrounded by the background is merged into
    // the background; same shape, valid indices, no invented color.
    #[test]
    fn single_speckle_merges_into_background() {
        let pattern = BeadPattern {
            width: 3,
            height: 3,
            cells: vec![0, 0, 0, 0, 1, 0, 0, 0, 0],
        };
        let out = despeckle(&pattern, 1);
        assert_eq!(out.width, 3);
        assert_eq!(out.height, 3);
        assert_eq!(out.cells, vec![0, 0, 0, 0, 0, 0, 0, 0, 0]);
        // No invented color: every output value already existed in the input.
        assert!(out.cells.iter().all(|&c| pattern.cells.contains(&c)));
    }

    // 5.2 — a component larger than the threshold is left untouched.
    #[test]
    fn component_above_threshold_is_kept() {
        // A 2-cell run of color 1; the rest is background 0. Both components
        // exceed min_region == 1, so nothing changes.
        let pattern = BeadPattern {
            width: 3,
            height: 3,
            cells: vec![1, 1, 0, 0, 0, 0, 0, 0, 0],
        };
        let out = despeckle(&pattern, 1);
        assert_eq!(out.cells, pattern.cells);
    }

    // 5.3 — a tie on the boundary tally resolves to the lowest palette index.
    #[test]
    fn tie_resolves_to_lowest_index() {
        // Single 0-cell at the center; its 4-neighbors are two 1s and two 2s, so
        // colors 1 and 2 tie (2 votes each) → lowest index 1 wins. The color-1
        // and color-2 regions are size 4 (above threshold), so only the center
        // is remapped.
        let pattern = BeadPattern {
            width: 3,
            height: 3,
            cells: vec![1, 1, 2, 1, 0, 2, 1, 2, 2],
        };
        let out = despeckle(&pattern, 1);
        assert_eq!(out.cells, vec![1, 1, 2, 1, 1, 2, 1, 2, 2]);
        // Repeated call is identical (determinism).
        assert_eq!(despeckle(&pattern, 1).cells, out.cells);
    }

    // 5.4a — whole-image single color: no differently-colored neighbor, returned
    // verbatim, no panic.
    #[test]
    fn single_color_image_unchanged() {
        let pattern = BeadPattern {
            width: 2,
            height: 2,
            cells: vec![5, 5, 5, 5],
        };
        assert_eq!(despeckle(&pattern, 100).cells, pattern.cells);
    }

    // 5.4b — empty pattern returned verbatim, no panic.
    #[test]
    fn empty_pattern_returned_verbatim() {
        let pattern = BeadPattern {
            width: 0,
            height: 0,
            cells: vec![],
        };
        let out = despeckle(&pattern, 1);
        assert_eq!(out.width, 0);
        assert_eq!(out.height, 0);
        assert!(out.cells.is_empty());
    }

    // Malformed public input (non-empty but shape-inconsistent) is returned
    // verbatim rather than panicking on the index math. `despeckle` is `pub`, so
    // a caller can hand-build such a pattern; the guard keeps it panic-free.
    #[test]
    fn malformed_shape_returned_verbatim() {
        // width == 0 with non-empty cells (would divide-by-zero on `idx % width`).
        let zero_width = BeadPattern {
            width: 0,
            height: 3,
            cells: vec![0, 1, 2],
        };
        assert_eq!(despeckle(&zero_width, 1).cells, zero_width.cells);
        // cells.len() != width * height (would index out of bounds on neighbors).
        let mismatched = BeadPattern {
            width: 4,
            height: 4,
            cells: vec![0, 1, 0],
        };
        assert_eq!(despeckle(&mismatched, 1).cells, mismatched.cells);
    }

    // 5.4c — min_region == 0 is a no-op (no component has zero beads).
    #[test]
    fn min_region_zero_is_noop() {
        let pattern = BeadPattern {
            width: 3,
            height: 3,
            cells: vec![0, 0, 0, 0, 1, 0, 0, 0, 0],
        };
        assert_eq!(despeckle(&pattern, 0).cells, pattern.cells);
    }

    // 5.4d — two adjacent off-color single points use the INPUT snapshot, so the
    // result is independent of scan order (row-major == column-major).
    #[test]
    fn adjacent_speckles_snapshot_order_independent() {
        // Row 1 is [B, X, Y, B] with background rows above and below; X and Y are
        // distinct single points. Each sees a background majority under the input
        // snapshot → both merge to background.
        let pattern = BeadPattern {
            width: 4,
            height: 3,
            cells: vec![
                0, 0, 0, 0, //
                0, 1, 2, 0, //
                0, 0, 0, 0,
            ],
        };
        let row_major = despeckle(&pattern, 1);
        assert_eq!(row_major.cells, vec![0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);

        // Column-major scan == transpose → despeckle → transpose back.
        let col_major = transpose(&despeckle(&transpose(&pattern), 1));
        assert_eq!(col_major.cells, row_major.cells);
        assert_eq!(col_major.width, row_major.width);
        assert_eq!(col_major.height, row_major.height);
    }

    // 5.5 — determinism + cross-architecture bit-exact integer golden: a fixed
    // small pattern + min_region maps to hardcoded expected cells (pure integer,
    // identical on arm64 and x86_64).
    #[test]
    fn golden_bit_exact_integer() {
        // 4x4. A single-cell speckle (color 2) inside a color-1 block sitting on
        // a color-0 background. min_region == 1 removes only the size-1 speckle.
        let pattern = BeadPattern {
            width: 4,
            height: 4,
            cells: vec![
                0, 0, 0, 0, //
                0, 1, 1, 0, //
                0, 1, 2, 0, //
                0, 0, 0, 0,
            ],
        };
        let out = despeckle(&pattern, 1);
        // The color-2 cell at (2,2): neighbors are 1 (left), 0 (right), 1 (up),
        // 0 (down) → tie between 0 and 1 → lowest index 0 wins.
        assert_eq!(
            out.cells,
            vec![
                0, 0, 0, 0, //
                0, 1, 1, 0, //
                0, 1, 0, 0, //
                0, 0, 0, 0,
            ]
        );
        // Byte-identical across repeated calls.
        assert_eq!(despeckle(&pattern, 1).cells, out.cells);
    }
}
