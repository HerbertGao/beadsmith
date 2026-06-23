//! Engine data models. M2 introduces `PixelGrid`, the raw-RGB intermediate
//! produced before color matching. M3 adds `BeadCell` / `BeadPattern` and maps
//! a `PixelGrid` into the final pattern.

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
