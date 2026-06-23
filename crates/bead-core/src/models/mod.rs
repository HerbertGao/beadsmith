//! Engine data models. M2 introduces `PixelGrid`, the raw-RGB intermediate
//! produced before color matching. M3 adds `BeadPattern { cells: Vec<u16> }`
//! (row-major palette indices) and maps a `PixelGrid` into it; from then on
//! `BeadPattern` is the source of truth (CLAUDE rule 3).

/// A fixed-size, row-major grid of raw RGB pixels — one cell per bead, before
/// color matching. This is the M2 source of truth (pre-matching); it is a
/// transitional intermediate, not the final result (see design D1).
///
/// Pixels are stored row-major: `pixels[y * width + x]` is the cell at
/// `(x, y)`. The invariant `pixels.len() == width * height` holds. Length and
/// index arithmetic must be done in `usize` (`width as usize * height as
/// usize`), never as `u32` multiply/add, to avoid overflow on large grids.
///
/// The invariant is maintained by `resize_image`. Because the fields are
/// `pub`, an external caller can construct a `PixelGrid` directly; such a
/// caller is responsible for satisfying `pixels.len() == width * height`,
/// otherwise downstream by-index cell access is wrong (see design D1).
#[derive(Debug, Clone, PartialEq)]
pub struct PixelGrid {
    /// Grid width in cells.
    pub width: u32,
    /// Grid height in cells.
    pub height: u32,
    /// Row-major raw RGB pixels; `pixels[y * width + x]` is cell `(x, y)`.
    pub pixels: Vec<[u8; 3]>,
}

/// The color-matched pattern — the source of truth from M3 onward (CLAUDE
/// rule 3 / design D1/D6). Preview (M5), statistics (M4), and exports all
/// derive from it; nothing is reconstructed from a rendered image.
///
/// Cells are stored row-major: `cells[y * width + x]` is the palette index of
/// cell `(x, y)`. Each `u16` is an index into the matcher's palette (declared
/// JSON order, see [`crate::matcher`]). The invariant `cells.len() == width *
/// height` holds. Length and index arithmetic must be done in `usize` (`width
/// as usize * height as usize`), never as `u32` multiply/add, to avoid overflow
/// on large grids.
///
/// The invariant is produced by [`crate::matcher::match_pattern`] from a valid
/// `PixelGrid`. Because the fields are `pub`, an external caller can construct a
/// `BeadPattern` directly; such a caller is responsible for satisfying
/// `cells.len() == width * height`, otherwise by-index cell access is wrong
/// (same caveat as `PixelGrid`).
///
/// There is **no `stats` field** in M3 — per-color statistics arrive in M4
/// (design D4).
///
/// Derives `PartialEq` (for `assert_eq!` and golden comparison) but
/// deliberately **not** `Eq` — same choice as `PixelGrid` (design D1).
#[derive(Debug, Clone, PartialEq)]
pub struct BeadPattern {
    /// Pattern width in cells.
    pub width: u32,
    /// Pattern height in cells.
    pub height: u32,
    /// Row-major palette indices; `cells[y * width + x]` is cell `(x, y)`.
    pub cells: Vec<u16>,
}

impl BeadPattern {
    /// The palette index at `(x, y)`, or `None` if out of bounds.
    ///
    /// Index arithmetic is done in `usize`. `x < width && y < height` returns
    /// `Some(cells[y * width + x])`; any out-of-bounds coordinate returns
    /// `None`.
    pub fn cell_at(&self, x: u32, y: u32) -> Option<u16> {
        if x >= self.width || y >= self.height {
            return None;
        }
        let idx = y as usize * self.width as usize + x as usize;
        self.cells.get(idx).copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // 5.6 — cell_at: row-major access and out-of-bounds -> None
    #[test]
    fn cell_at_rowmajor_and_oob() {
        // 3x2 pattern, cells laid out row-major.
        let pattern = BeadPattern {
            width: 3,
            height: 2,
            cells: vec![10, 11, 12, 20, 21, 22],
        };

        // in-bounds: cell_at(x, y) == cells[y * width + x]
        assert_eq!(pattern.cell_at(0, 0), Some(10));
        assert_eq!(pattern.cell_at(2, 0), Some(12));
        assert_eq!(pattern.cell_at(0, 1), Some(20));
        assert_eq!(pattern.cell_at(2, 1), Some(22));

        // out of bounds in x, y, or both -> None
        assert_eq!(pattern.cell_at(3, 0), None);
        assert_eq!(pattern.cell_at(0, 2), None);
        assert_eq!(pattern.cell_at(3, 2), None);
    }
}
