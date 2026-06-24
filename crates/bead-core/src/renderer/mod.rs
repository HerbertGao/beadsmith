//! Renders a [`BeadPattern`] + [`Palette`] into in-memory PNG bytes — a
//! `render_preview` (the finished bead-art look, no coordinates) and a
//! `render_grid` (row/column numbers + grid lines for laying out beads). Pure
//! data in, PNG bytes out — **no filesystem, no UI, no platform assumptions**
//! (CLAUDE rule 1); the CLI/FFI writes the bytes to `preview.png` / `grid.png`.
//!
//! Color is derived **only** from `BeadPattern.cells` (palette indices): each
//! cell `idx = cells[y * width + x]` looks up `palette.colors[idx].rgb`. The
//! renderer **never** reads `PixelGrid`'s raw RGB, **never** reconstructs data
//! from a rendered image, and **never** stores anything back onto the pattern
//! (CLAUDE rule 3 / color-matching forward constraint). `BeadPattern` is the
//! source of truth.
//!
//! The local module is `renderer`, but the external `image` crate is referenced
//! throughout as `::image::…` (a bare `image::…` would resolve to the sibling
//! `image` module and fail to compile) — same convention as the `image` module.

use ::image::codecs::png::{CompressionType, FilterType, PngEncoder};
use ::image::{Rgb, RgbImage};

use crate::models::BeadPattern;
use crate::palette::Palette;
use crate::BeadError;

/// Options controlling how a [`BeadPattern`] is rendered. Mirrors the
/// `ResizeOptions` precedent (an options struct with `Default`); callers write
/// `RenderOptions::default()` or set `cell_size` (design D2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RenderOptions {
    /// Pixel edge length of each rendered bead. Default `10`; guarded `> 0`
    /// (and `>= 5` for `render_grid`, see design D7).
    pub cell_size: u32,
    /// Bead shape. Only `Square` is implemented in M5.
    pub shape: BeadShape,
}

impl Default for RenderOptions {
    fn default() -> Self {
        RenderOptions {
            cell_size: 10,
            shape: BeadShape::Square,
        }
    }
}

/// The bead glyph shape. `#[non_exhaustive]` so future shapes are a
/// non-breaking addition (design D2).
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BeadShape {
    /// A solid filled square — the only shape rendered in M5.
    Square,
    // ponytail: 仅 Square；Circle/Ring 留后续 change，#[non_exhaustive] 是 seam，不现在写占位分支
}

// ----------------------------------------------------------------------------
// PNG encoding (D3)
// ----------------------------------------------------------------------------

// Concrete named encoding parameters — NOT `::Default`. `CompressionType::Fast`
// maps directly to `png::Compression::Fast` (whereas `::Default` →
// `png::Compression::Balanced`, an extra remappable indirection). The exact
// bytes are NOT a SemVer guarantee upstream, so pinning these only buys
// same-version byte stability + intent clarity (design D3).
const PNG_COMPRESSION: CompressionType = CompressionType::Fast;
const PNG_FILTER: FilterType = FilterType::Adaptive;

/// Encode an `RgbImage` to PNG bytes (8-bit RGB, non-interlaced) in memory.
///
/// `PngEncoder` is non-interlaced by default and `RgbImage` is RGB8, so the
/// output is 8-bit RGB non-interlaced without extra setup — this is the
/// precondition that makes the "decode back and compare pixels" round-trip
/// lossless (design D3).
fn encode_png(img: &RgbImage) -> Result<Vec<u8>, BeadError> {
    // ponytail: 不靠 write_to 隐式默认（跨版本漂）；钉 concrete 常量只保同版本字节稳，
    // 跨版本不保（上游明文）、frozen golden 归 M7（D3）。
    let mut buf = Vec::new();
    let enc = PngEncoder::new_with_quality(&mut buf, PNG_COMPRESSION, PNG_FILTER);
    // write_with_encoder returns ImageResult<()>. Its internal assert_eq! (buffer
    // len == 3*w*h) is unreachable here: `img` is paint_*'s own RgbImage, whose
    // len is `3*out_w*out_h` by construction. We map only the Err arm (D8).
    img.write_with_encoder(enc)
        .map_err(|source| BeadError::ImageEncode { source })?;
    Ok(buf)
}

// ----------------------------------------------------------------------------
// Color lookup (D6)
// ----------------------------------------------------------------------------

/// Sentinel for an unresolvable cell (magenta — the traditional "missing
/// texture" color). Both reachable invariant violations land here (design D6).
const MISSING: [u8; 3] = [255, 0, 255];

/// The color of the cell at row-major position `pos`, via two layered `.get()`s
/// (never a bare index).
///
/// `pos` out of bounds (a too-short `cells`) or an out-of-bounds palette index
/// both yield the [`MISSING`] sentinel (design D6).
fn cell_rgb(grid: &BeadPattern, palette: &Palette, pos: usize) -> [u8; 3] {
    // ponytail: 两类可达违约（过短 cells 的 pos 越界 / 越界 palette 下标）都落品红哨兵、
    // 不 panic（同 statistics D4 姿态；裸 cells[pos]/colors[idx] 会 panic，M2）。
    grid.cells
        .get(pos)
        .and_then(|&idx| palette.colors.get(idx as usize))
        .map(|c| c.rgb)
        .unwrap_or(MISSING)
}

// ----------------------------------------------------------------------------
// preview (D4 / D7)
// ----------------------------------------------------------------------------

/// Paint a no-coordinate preview: each cell is a `cell × cell` solid square at
/// row-major position `pos = y*width + x`, colored via [`cell_rgb`]. Dimensions
/// are already validated by [`render_preview`].
fn paint_preview(grid: &BeadPattern, palette: &Palette, cell: u32) -> RgbImage {
    let w = grid.width;
    let h = grid.height;
    let mut img = RgbImage::new(w * cell, h * cell);
    let wu = w as usize;
    for y in 0..h {
        for x in 0..w {
            // usize arithmetic for the row-major position (design D4 / models).
            let pos = y as usize * wu + x as usize;
            let rgb = Rgb(cell_rgb(grid, palette, pos));
            let px0 = x * cell;
            let py0 = y * cell;
            for dy in 0..cell {
                for dx in 0..cell {
                    img.put_pixel(px0 + dx, py0 + dy, rgb);
                }
            }
        }
    }
    img
}

/// Geometry + total-buffer guard for preview, **entirely in `u128` with strict
/// ordering** (design D7 / R2-B1 / R3-M-ord). Returns the validated
/// `(out_w, out_h)` as `u32`.
fn preview_dims_checked(w: u32, h: u32, cell: u32) -> Result<(u32, u32), BeadError> {
    // ponytail: 定序是封闭前提——夹 out<=u32::MAX 前算 3*out² 会溢出连 u128 都兜不住
    //           （R3-M-ord）；RgbImage::new 内部 3*w*h usize .expect 会 panic（R2-B1）。
    let out_w = w as u128 * cell as u128;
    let out_h = h as u128 * cell as u128;

    // ① reject out dimensions > u32::MAX FIRST.
    if out_w > u32::MAX as u128 || out_h > u32::MAX as u128 {
        return Err(BeadError::InvalidImage {
            reason: "preview output dimensions exceed u32".to_string(),
        });
    }

    // ② only now (out_* <= u32::MAX) is `3 * out_w * out_h` <= ~5.5e19 << u128::MAX.
    let bytes = 3 * out_w * out_h;
    if bytes > isize::MAX as u128 {
        return Err(BeadError::InvalidImage {
            reason: "preview output buffer exceeds isize::MAX".to_string(),
        });
    }

    Ok((out_w as u32, out_h as u32))
}

/// Render `grid` into an in-memory **PNG** of the finished bead-art look (no
/// coordinates, no grid lines, no margin) — each bead is a `cell_size × cell_size`
/// solid square colored `palette.colors[cells[i]].rgb`, row-major, output size
/// `(width*cell_size) × (height*cell_size)`.
///
/// **Precondition**: `palette` is the same unmodified palette the matcher that
/// produced `grid` used (and `cells.len() == width * height`); a violation does
/// not panic but renders the offending cell as the magenta sentinel (design D6,
/// see `cell_rgb`).
///
/// Returns `Err(BeadError::InvalidImage { .. })` (never panics, in debug or
/// release) for `width == 0`, `height == 0`, `cell_size == 0`, or an output
/// buffer too large for `u32` dimensions / `isize::MAX` (design D7). PNG
/// encoding failure returns `Err(BeadError::ImageEncode { .. })` (D8).
pub fn render_preview(
    grid: &BeadPattern,
    palette: &Palette,
    opts: &RenderOptions,
) -> Result<Vec<u8>, BeadError> {
    if grid.width == 0 {
        return Err(BeadError::InvalidImage {
            reason: "pattern width is 0".to_string(),
        });
    }
    if grid.height == 0 {
        return Err(BeadError::InvalidImage {
            reason: "pattern height is 0".to_string(),
        });
    }
    if opts.cell_size == 0 {
        return Err(BeadError::InvalidImage {
            reason: "cell_size is 0".to_string(),
        });
    }

    // Full geometry + buffer guard (u128, strict order) before any allocation.
    preview_dims_checked(grid.width, grid.height, opts.cell_size)?;

    encode_png(&paint_preview(grid, palette, opts.cell_size))
}

// ----------------------------------------------------------------------------
// grid: bitmap digit font (D5 / D9)
// ----------------------------------------------------------------------------

/// 3-wide × 5-high bitmap glyphs for digits `0..=9`. Each row's low 3 bits are
/// pixels, MSB = left. Hand-written so coordinate digits need **zero font
/// dependency** and are pure-integer → bit-identical across architectures (a
/// font-rendering crate's antialiasing / hinting would break determinism, D9).
const DIGITS_3X5: [[u8; 5]; 10] = [
    [0b111, 0b101, 0b101, 0b101, 0b111], // 0
    [0b010, 0b110, 0b010, 0b010, 0b111], // 1
    [0b111, 0b001, 0b111, 0b100, 0b111], // 2
    [0b111, 0b001, 0b111, 0b001, 0b111], // 3
    [0b101, 0b101, 0b111, 0b001, 0b001], // 4
    [0b111, 0b100, 0b111, 0b001, 0b111], // 5
    [0b111, 0b100, 0b111, 0b101, 0b111], // 6
    [0b111, 0b001, 0b010, 0b100, 0b100], // 7
    [0b111, 0b101, 0b111, 0b101, 0b111], // 8
    [0b111, 0b101, 0b111, 0b001, 0b111], // 9
];

/// Number of decimal digits in `n` (`n == 0 → 1`, else `floor(log10) + 1`),
/// integer loop only. Generic over the unsigned integer type so it serves both
/// the `u128` context (`grid_geom_checked`'s `row_digits`) and the `u32` context
/// (`paint_grid`'s per-label `num_w(digits(n))`) with identical semantics
/// (CR nit-2). Called only with values `>= STEP (10)` → `d >= 2`, so
/// `num_w(d) = d*4*scale - scale` never underflows.
fn decimal_digits<T>(mut n: T) -> u32
where
    T: Copy + PartialEq + PartialOrd + From<u8> + core::ops::DivAssign,
{
    let zero = T::from(0u8);
    let ten = T::from(10u8);
    if n == zero {
        return 1;
    }
    let mut d = 0u32;
    while n > zero {
        n /= ten;
        d += 1;
    }
    d
}

// ----------------------------------------------------------------------------
// grid: fixed style constants (D5)
// ----------------------------------------------------------------------------

// ponytail: M5 固定不可配；要可配再长 RenderOptions 字段——技术上破坏、靠
//           Default+..default() 缓解，见 D2。
/// Background color.
const BG: [u8; 3] = [255, 255, 255];
/// Thin per-cell grid line color.
const THIN: [u8; 3] = [200, 200, 200];
/// Bold (every-STEP) separator line color.
const BOLD: [u8; 3] = [120, 120, 120];
/// Coordinate text color.
const TEXT: [u8; 3] = [0, 0, 0];
/// Bold line width (pixels).
const BOLD_W: u32 = 2;
/// Coordinate labeling interval (every 10th boundary).
const STEP: u32 = 10;

// ----------------------------------------------------------------------------
// grid: digit drawing (D5)
// ----------------------------------------------------------------------------

/// Draw the decimal digits of `value` as [`TEXT`] pixels starting at top-left
/// `(x0, y0)`, each glyph integer-scaled by `scale` (glyph width `3*scale`,
/// inter-digit gap `scale`). Per-pixel bounds-checked — any pixel outside the
/// image is skipped, never a panic (a backstop; the margin formulas already
/// guarantee labels fall in-image, design D5).
fn draw_number(img: &mut RgbImage, x0: u32, y0: u32, value: u32, scale: u32) {
    let text = Rgb(TEXT);
    // Render most-significant digit first; advance x by glyph width + gap.
    let digits = decimal_digits(value);
    let mut cursor_x = x0;
    // Extract digits MSB-first using a descending power of ten.
    let mut divisor = 1u32;
    for _ in 1..digits {
        divisor = divisor.saturating_mul(10);
    }
    let mut remaining = value;
    for _ in 0..digits {
        let digit = (remaining / divisor) as usize;
        remaining %= divisor.max(1);
        if divisor > 1 {
            divisor /= 10;
        }
        let glyph = &DIGITS_3X5[digit];
        for (row, bits) in glyph.iter().enumerate() {
            for col in 0..3u32 {
                // MSB = left: bit (2 - col).
                if (bits >> (2 - col)) & 1 == 1 {
                    let base_x = cursor_x + col * scale;
                    let base_y = y0 + row as u32 * scale;
                    for sy in 0..scale {
                        for sx in 0..scale {
                            let px = base_x + sx;
                            let py = base_y + sy;
                            if let Some(p) = img.get_pixel_mut_checked(px, py) {
                                *p = text;
                            }
                        }
                    }
                }
            }
        }
        // glyph width 3*scale + inter-digit gap scale.
        cursor_x += 3 * scale + scale;
    }
}

// ----------------------------------------------------------------------------
// grid: geometry guard (D5 / D7)
// ----------------------------------------------------------------------------

/// Validated grid geometry. All fields are `u32` and `<= u32::MAX`; built only
/// by [`grid_geom_checked`] after the `u128` + ordered guard. Includes `cell`
/// because `paint_grid` needs it for cell origins / blocks / lines / label
/// anchors (review R4-M1).
struct GridGeom {
    cell: u32,
    scale: u32,
    pad: u32,
    margin_left: u32,
    margin_top: u32,
    out_w: u32,
    out_h: u32,
}

/// Compute the full grid geometry **entirely in `u128`**, validate, and cast to
/// `u32`. Strict ordering (design D7 / R3-B1 / R3-M-ord): **no margin/scale is
/// ever computed in `u32`** (a large `cell_size` would overflow before the
/// guard), and `bytes` is computed **only after** `out_*` is clamped to
/// `<= u32::MAX` (otherwise `3 * out²` overflows even `u128`).
fn grid_geom_checked(width: u32, height: u32, cell: u32) -> Result<GridGeom, BeadError> {
    // ponytail: 定序+u128 是封闭前提——margin 在 u32 算或夹前算 bytes 都会溢出
    //           （R3-B1/R3-M-ord 三审收敛）。
    let width = width as u128;
    let height = height as u128;
    let cell = cell as u128;
    let step = STEP as u128;

    let scale = (cell / 5).max(1);
    let pad = scale;
    let has_col = width >= step;
    let has_row = height >= step;

    let max_row_label = if has_row { (height / step) * step } else { 0 };
    let row_digits = decimal_digits(max_row_label) as u128;
    // num_w(d) = d*4*scale - scale (only ever called with d >= 1 here).
    let num_w = |d: u128| d * 4 * scale - scale;

    let margin_top = if has_col { 7 * scale } else { 0 };
    let margin_left = if has_row {
        num_w(row_digits) + 2 * pad
    } else {
        0
    };

    let out_w = margin_left + width * cell;
    let out_h = margin_top + height * cell;

    // ① reject out dimensions > u32::MAX FIRST (this also clamps margins, since
    //    margin_* <= out_*).
    if out_w > u32::MAX as u128 || out_h > u32::MAX as u128 {
        return Err(BeadError::InvalidImage {
            reason: "grid output dimensions exceed u32".to_string(),
        });
    }

    // ② only now (out_* <= u32::MAX) is `3 * out_w * out_h` <= ~5.5e19 << u128::MAX.
    let bytes = 3 * out_w * out_h;
    if bytes > isize::MAX as u128 {
        return Err(BeadError::InvalidImage {
            reason: "grid output buffer exceeds isize::MAX".to_string(),
        });
    }

    // ③ all guards passed → every value is <= u32::MAX, so the casts are safe.
    Ok(GridGeom {
        cell: cell as u32,
        scale: scale as u32,
        pad: pad as u32,
        margin_left: margin_left as u32,
        margin_top: margin_top as u32,
        out_w: out_w as u32,
        out_h: out_h as u32,
    })
}

// ----------------------------------------------------------------------------
// grid: painting (D5)
// ----------------------------------------------------------------------------

/// Pixel width of a `d`-digit number at `scale`: `d*3*scale + (d-1)*scale =
/// d*4*scale - scale`. Only ever called with `d >= 1`.
fn num_w(d: u32, scale: u32) -> u32 {
    d * 4 * scale - scale
}

/// Set the pixel at `(x, y)` to `rgb` only if it is in-bounds (`x < g.out_w &&
/// y < g.out_h`) — never a bare `put_pixel` (which panics out of bounds, D5).
fn set_px_checked(img: &mut RgbImage, x: u32, y: u32, rgb: Rgb<u8>) {
    if let Some(p) = img.get_pixel_mut_checked(x, y) {
        *p = rgb;
    }
}

/// Paint the grid image (beads + overlay lines + 1-indexed labels) from the
/// validated [`GridGeom`]. Uses only `g.*` (never a bare `cell`, never re-derives
/// cell/scale/margin — those would re-introduce u32 overflow, review R4-M1).
fn paint_grid(grid: &BeadPattern, palette: &Palette, g: &GridGeom) -> RgbImage {
    let width = grid.width;
    let height = grid.height;
    let wu = width as usize;

    // Background fill (margins are BG; lines/labels/beads overlay it).
    let mut img = RgbImage::from_pixel(g.out_w, g.out_h, Rgb(BG));

    // --- beads: cell (x,y) origin = (margin_left + x*cell, margin_top + y*cell) ---
    for y in 0..height {
        for x in 0..width {
            let pos = y as usize * wu + x as usize;
            let rgb = Rgb(cell_rgb(grid, palette, pos));
            let px0 = g.margin_left + x * g.cell;
            let py0 = g.margin_top + y * g.cell;
            for dy in 0..g.cell {
                for dx in 0..g.cell {
                    set_px_checked(&mut img, px0 + dx, py0 + dy, rgb);
                }
            }
        }
    }

    // --- grid lines (overlay, only inside the cell area; bounds-checked) ---
    // Two passes so BOLD always wins at crossings regardless of axis order: a
    // single per-axis pass let a later thin-horizontal write overwrite a
    // bold-vertical pixel at their intersection. Pass 0 draws thin (non-STEP)
    // boundaries, pass 1 draws bold (STEP) boundaries — bold is drawn last.
    for bold_pass in [false, true] {
        let (color, line_w) = if bold_pass {
            (Rgb(BOLD), BOLD_W)
        } else {
            (Rgb(THIN), 1)
        };
        // Vertical boundaries bx in 0..=width at x = margin_left + bx*cell,
        // along y in [margin_top, out_h).
        for bx in 0..=width {
            if (bx % STEP == 0) != bold_pass {
                continue;
            }
            let x_base = g.margin_left + bx * g.cell;
            for off in 0..line_w {
                // Last/right boundary lands at x = out_w (off-image): clamp to
                // out_w-1 (saturating_add so the offset can't overflow u32).
                let x = x_base.saturating_add(off).min(g.out_w - 1);
                for y in g.margin_top..g.out_h {
                    set_px_checked(&mut img, x, y, color);
                }
            }
        }
        // Horizontal boundaries by in 0..=height at y = margin_top + by*cell,
        // along x in [margin_left, out_w).
        for by in 0..=height {
            if (by % STEP == 0) != bold_pass {
                continue;
            }
            let y_base = g.margin_top + by * g.cell;
            for off in 0..line_w {
                let y = y_base.saturating_add(off).min(g.out_h - 1);
                for x in g.margin_left..g.out_w {
                    set_px_checked(&mut img, x, y, color);
                }
            }
        }
    }

    // --- column labels (1-indexed, every STEP), right-aligned at the boundary ---
    let mut n = STEP;
    while n <= width {
        let digits = decimal_digits(n);
        // right edge x = margin_left + n*cell; left edge = right - num_w(digits).
        let x = g.margin_left + n * g.cell - num_w(digits, g.scale);
        draw_number(&mut img, x, g.pad, n, g.scale);
        n += STEP;
    }

    // --- row labels (1-indexed, every STEP), right-aligned in the left margin ---
    let mut n = STEP;
    while n <= height {
        let digits = decimal_digits(n);
        // right edge x = margin_left - pad; left edge = right - num_w(digits).
        let x_left = g.margin_left - g.pad - num_w(digits, g.scale);
        let y = g.margin_top + (n - 1) * g.cell;
        draw_number(&mut img, x_left, y, n, g.scale);
        n += STEP;
    }

    img
}

/// Render `grid` into an in-memory **PNG** with coordinates: the bead grid plus
/// a thin line at every cell boundary, a bold line every 10th boundary, and
/// **1-indexed** row/column numbers (`10, 20, …`) drawn with the built-in bitmap
/// digit font (no font dependency). Line/text/background colors, bold width, and
/// the labeling interval (10) are fixed constants (M5 non-configurable). Grid
/// lines overlay the cell-edge pixels (they do not add to the output size).
///
/// Geometry is fully deterministic integer geometry (design D5); output size is
/// `margin + dimension × cell_size`.
///
/// **Requires `cell_size >= 5`** (`< 5` → `Err`): below this the coordinate
/// labels would vertically clip / horizontally overlap (design D5 / D7).
/// `render_preview` has no such limit.
///
/// **Precondition**: same as `render_preview` (palette is the matcher's
/// unmodified palette; out-of-bounds / missing cells render as the magenta
/// sentinel, never a panic, design D6).
///
/// Returns `Err(BeadError::InvalidImage { .. })` (never panics, debug or
/// release) for `width == 0`, `height == 0`, `cell_size == 0`, `cell_size < 5`,
/// or an output buffer too large (design D7). PNG encoding failure returns
/// `Err(BeadError::ImageEncode { .. })` (D8).
pub fn render_grid(
    grid: &BeadPattern,
    palette: &Palette,
    opts: &RenderOptions,
) -> Result<Vec<u8>, BeadError> {
    if grid.width == 0 {
        return Err(BeadError::InvalidImage {
            reason: "pattern width is 0".to_string(),
        });
    }
    if grid.height == 0 {
        return Err(BeadError::InvalidImage {
            reason: "pattern height is 0".to_string(),
        });
    }
    if opts.cell_size == 0 {
        return Err(BeadError::InvalidImage {
            reason: "cell_size is 0".to_string(),
        });
    }
    if opts.cell_size < 5 {
        return Err(BeadError::InvalidImage {
            reason: "cell_size must be >= 5 for grid".to_string(),
        });
    }

    // Full geometry + buffer guard (u128, strict order, includes margins) before
    // any allocation.
    let g = grid_geom_checked(grid.width, grid.height, opts.cell_size)?;

    encode_png(&paint_grid(grid, palette, &g))
}

// ----------------------------------------------------------------------------
// Tests (§5) — expected values derived from the SPEC / D5 geometry formulas,
// not from the implementation's behavior. PNG bytes are decoded back via
// `::image::load_from_memory(..).to_rgb8()` and pixels are asserted.
// ----------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::BeadPattern;
    use crate::palette::{Palette, PaletteColor};

    /// Build a palette whose color `i` has rgb `rgbs[i]` (code/name synthesized).
    fn palette_with(rgbs: &[[u8; 3]]) -> Palette {
        Palette {
            brand: "Test".to_string(),
            colors: rgbs
                .iter()
                .enumerate()
                .map(|(i, &rgb)| PaletteColor {
                    code: format!("C{i:02}"),
                    name: format!("Color {i}"),
                    rgb,
                })
                .collect(),
        }
    }

    /// Decode PNG bytes back to an RgbImage for pixel assertions.
    fn decode(bytes: &[u8]) -> ::image::RgbImage {
        ::image::load_from_memory(bytes)
            .expect("rendered PNG must decode")
            .to_rgb8()
    }

    fn px(img: &::image::RgbImage, x: u32, y: u32) -> [u8; 3] {
        img.get_pixel(x, y).0
    }

    // ----- 5.1 preview size + per-cell color -----
    #[test]
    fn preview_size_and_per_cell_color() {
        // 3x2 grid, cell_size 4. Palette indices 0..=3 with distinct colors.
        let w = 3u32;
        let h = 2u32;
        let c = 4u32;
        let rgbs = [[10, 20, 30], [40, 50, 60], [70, 80, 90], [100, 110, 120]];
        let palette = palette_with(&rgbs);
        // cells row-major: y*w + x
        let cells: Vec<u16> = vec![0, 1, 2, 3, 0, 1];
        let grid = BeadPattern {
            width: w,
            height: h,
            cells: cells.clone(),
        };
        let opts = RenderOptions {
            cell_size: c,
            shape: BeadShape::Square,
        };

        let bytes = render_preview(&grid, &palette, &opts).expect("render_preview ok");
        let img = decode(&bytes);

        // size == (w*c) x (h*c)
        assert_eq!(img.width(), w * c);
        assert_eq!(img.height(), h * c);

        // every cell, sampling multiple offsets incl. the 4 corners of each cell.
        for y in 0..h {
            for x in 0..w {
                let idx = cells[(y * w + x) as usize] as usize;
                let expected = rgbs[idx];
                for &(dx, dy) in &[
                    (0u32, 0u32),
                    (c - 1, 0),
                    (0, c - 1),
                    (c - 1, c - 1),
                    (c / 2, c / 2),
                ] {
                    assert_eq!(
                        px(&img, x * c + dx, y * c + dy),
                        expected,
                        "preview cell ({x},{y}) offset ({dx},{dy})"
                    );
                }
            }
        }

        // image-level four corners
        assert_eq!(px(&img, 0, 0), rgbs[cells[0] as usize]); // (0,0)
        assert_eq!(
            px(&img, w * c - 1, 0),
            rgbs[cells[(w - 1) as usize] as usize]
        ); // top-right
        assert_eq!(
            px(&img, 0, h * c - 1),
            rgbs[cells[((h - 1) * w) as usize] as usize]
        ); // bottom-left
        assert_eq!(
            px(&img, w * c - 1, h * c - 1),
            rgbs[cells[((h - 1) * w + (w - 1)) as usize] as usize]
        ); // bottom-right
    }

    // ----- 5.2 grid geometry exact (13x13, cell=10, scale=2) -----
    #[test]
    fn grid_geometry_exact() {
        // D5 hand-computed geometry for 13x13, cell=10:
        //   scale = max(1, 10/5) = 2 ; pad = 2 ; STEP = 10
        //   has_col = has_row = true (13 >= 10)
        //   max_row_label = (13/10)*10 = 10 ; row_digits = 2
        //   num_w(2) = 2*4*2 - 2 = 14
        //   margin_top  = 7*scale = 14
        //   margin_left = num_w(2) + 2*pad = 14 + 4 = 18
        //   out_w = 18 + 13*10 = 148 ; out_h = 14 + 13*10 = 144
        let w = 13u32;
        let h = 13u32;
        let cell = 10u32;
        let scale = 2u32;
        let pad = 2u32;
        let margin_left = 18u32;
        let margin_top = 14u32;
        let out_w = 148u32;
        let out_h = 144u32;

        // distinct palette so beads are not BG/THIN/BOLD/TEXT by accident.
        let rgbs = [[12u8, 34, 56], [200, 10, 10]];
        let palette = palette_with(&rgbs);
        let cells: Vec<u16> = vec![0u16; (w * h) as usize];
        let grid = BeadPattern {
            width: w,
            height: h,
            cells,
        };
        let opts = RenderOptions {
            cell_size: cell,
            shape: BeadShape::Square,
        };

        let bytes = render_grid(&grid, &palette, &opts).expect("render_grid ok");
        let img = decode(&bytes);

        // ① decoded size matches the D5 formula
        assert_eq!(img.width(), out_w, "out_w");
        assert_eq!(img.height(), out_h, "out_h");

        // ② vertical boundaries: 10th is BOLD, 1..9 are THIN.
        //    Sample at a y between two horizontal boundaries (y = margin_top + 5),
        //    so the vertical line isn't overwritten by a horizontal line.
        let vy = margin_top + 5; // 19; not on any horizontal boundary (14,24,..)
        for bx in 1u32..=9 {
            let x = margin_left + bx * cell;
            assert_eq!(
                px(&img, x, vy),
                THIN,
                "vertical boundary {bx} must be THIN at x={x}"
            );
        }
        let x10 = margin_left + 10 * cell; // 118
        assert_eq!(
            px(&img, x10, vy),
            BOLD,
            "10th vertical boundary must be BOLD"
        );

        // horizontal boundaries: 10th is BOLD, 1..9 are THIN.
        // Sample at an x between two vertical boundaries (x = margin_left + 5).
        let hx = margin_left + 5; // 23; not on any vertical boundary
        for by in 1u32..=9 {
            let y = margin_top + by * cell;
            assert_eq!(
                px(&img, hx, y),
                THIN,
                "horizontal boundary {by} must be THIN at y={y}"
            );
        }
        let y10 = margin_top + 10 * cell; // 114
        assert_eq!(
            px(&img, hx, y10),
            BOLD,
            "10th horizontal boundary must be BOLD"
        );

        // Bold must win over thin at perpendicular crossings (regardless of
        // draw order). This (bold-vertical bx=10 × thin-horizontal by=1) is the
        // load-bearing regression guard: the pre-fix single-pass order (all
        // vertical, then all horizontal) drew the bold vertical first, then a
        // thin horizontal overwrote it here → THIN; this assert fails on that.
        assert_eq!(
            px(&img, margin_left + 10 * cell, margin_top + cell),
            BOLD,
            "bold vertical must win over thin horizontal at their crossing"
        );
        // thin-vertical (bx=1) × bold-horizontal (by=10) — bold won pre-fix too
        // (horizontals drawn last), so this documents the orientation, it is not
        // an independent regression pin.
        assert_eq!(
            px(&img, margin_left + cell, margin_top + 10 * cell),
            BOLD,
            "bold horizontal must win over thin vertical at their crossing"
        );
        // bold-vertical (bx=10) × bold-horizontal (by=10) — completes the 2×2
        // crossing matrix.
        assert_eq!(
            px(&img, margin_left + 10 * cell, margin_top + 10 * cell),
            BOLD,
            "bold × bold crossing must be BOLD"
        );

        // ③ column label "10" at its right-aligned anchor:
        //    right edge = margin_left + 10*cell = 118; glyph width num_w(2)=14;
        //    so the label occupies x in [104, 118), y starting at pad=2.
        //    Assert the EXACT glyph pixels prove it's "10" (digit 1 then digit 0),
        //    not "11" / misaligned. We reconstruct the expected glyph bitmap from
        //    DIGITS_3X5 at the spec-derived anchor and compare every pixel in the
        //    glyph bounding box.
        let label_right = margin_left + 10 * cell; // 118
        let label_w = num_w(2, scale); // 14
        let label_x0 = label_right - label_w; // 104
        let label_y0 = pad; // 2

        assert_eq!(label_x0, 104, "label left anchor");
        assert!(
            label_right <= out_w,
            "label right edge must not exceed out_w"
        );

        assert_glyphs(&img, label_x0, label_y0, &[1, 0], scale);

        // Cross-check the label is NOT "11": pixel that is set in '0' col-pattern
        // but clear in '1' must differ. Specifically, glyph '0' has its top row
        // 0b111 (all 3 cols set), while glyph '1' top row is 0b010 (only middle).
        // The second digit's top-left pixel: for '0' it is TEXT, for '1' it'd be BG.
        let second_digit_x0 = label_x0 + (3 * scale + scale); // 104 + 8 = 112
                                                              // '0' row0 = 0b111 -> col0 (leftmost) is set -> TEXT
        assert_eq!(
            px(&img, second_digit_x0, label_y0),
            TEXT,
            "second digit must be '0' (top-left set), proving label is '10' not '11'"
        );

        // ④ A dim-<10 grid still returns Ok with that axis margin == 0, no panic.
        //    width=5 (<10) -> no column labels, margin_top=0; height=13 -> row labels.
        let narrow = BeadPattern {
            width: 5,
            height: 13,
            cells: vec![0u16; 5 * 13],
        };
        let nbytes = render_grid(&narrow, &palette, &opts).expect("narrow grid ok");
        let nimg = decode(&nbytes);
        // has_col = false -> margin_top = 0 ; has_row = true -> margin_left = 18
        // out_w = 18 + 5*10 = 68 ; out_h = 0 + 13*10 = 130
        assert_eq!(nimg.width(), 18 + 5 * 10, "narrow out_w");
        assert_eq!(nimg.height(), 13 * 10, "narrow out_h (margin_top==0)");
    }

    /// Assert the exact glyph pixels for a sequence of digits drawn at top-left
    /// `(x0, y0)` with integer `scale`, using the spec font `DIGITS_3X5`. Every
    /// pixel inside each glyph's 3x5 (scaled) box must be TEXT where the bitmap
    /// bit is set and must NOT be TEXT where it is clear (proving the exact digit
    /// and alignment).
    fn assert_glyphs(img: &::image::RgbImage, x0: u32, y0: u32, digits: &[usize], scale: u32) {
        let mut cursor_x = x0;
        for &d in digits {
            let glyph = &DIGITS_3X5[d];
            for (row, bits) in glyph.iter().enumerate() {
                for col in 0..3u32 {
                    let set = (bits >> (2 - col)) & 1 == 1;
                    let base_x = cursor_x + col * scale;
                    let base_y = y0 + row as u32 * scale;
                    for sy in 0..scale {
                        for sx in 0..scale {
                            let p = px(img, base_x + sx, base_y + sy);
                            if set {
                                assert_eq!(
                                    p,
                                    TEXT,
                                    "glyph digit {d} row {row} col {col} ({},{}) must be TEXT",
                                    base_x + sx,
                                    base_y + sy
                                );
                            } else {
                                assert_ne!(
                                    p,
                                    TEXT,
                                    "glyph digit {d} row {row} col {col} ({},{}) must NOT be TEXT",
                                    base_x + sx,
                                    base_y + sy
                                );
                            }
                        }
                    }
                }
            }
            cursor_x += 3 * scale + scale;
        }
    }

    // ----- 5.2b multi-digit (>=100) label fits in margin -----
    #[test]
    fn grid_multidigit_label_fits_margin() {
        // height = 105 (>=100) -> max_row_label = 100, row_digits = 3.
        // width = 13 so column labels exist too. cell = 5 -> scale = 1, pad = 1.
        //   num_w(3) = 3*4*1 - 1 = 11 ; margin_left = 11 + 2 = 13
        let w = 13u32;
        let h = 105u32;
        let cell = 5u32;
        let scale = 1u32;
        let pad = 1u32;
        let margin_left = 13u32;
        let margin_top = 7u32; // 7*scale

        let palette = palette_with(&[[1u8, 2, 3]]);
        let grid = BeadPattern {
            width: w,
            height: h,
            cells: vec![0u16; (w * h) as usize],
        };
        let opts = RenderOptions {
            cell_size: cell,
            shape: BeadShape::Square,
        };

        let bytes = render_grid(&grid, &palette, &opts).expect("render_grid ok");
        let img = decode(&bytes);

        // The row label "100" is at row n=100: right edge = margin_left - pad = 12,
        // left edge = 12 - num_w(3) = 12 - 11 = 1, y = margin_top + (100-1)*cell.
        let label_right = margin_left - pad; // 12
        let label_w = num_w(3, scale); // 11
        let label_x0 = label_right - label_w; // 1
        let label_y0 = margin_top + (100 - 1) * cell;

        // All glyph pixels of "100" must land within [0, margin_left) horizontally
        // and within image vertically -> fits in margin, no clip, no panic.
        // (label_x0 is u32, so it is non-negative by construction — no underflow.)
        assert!(
            label_x0 + label_w <= margin_left,
            "label '100' right edge must stay within margin_left"
        );
        assert!(
            label_y0 + 5 * scale <= img.height(),
            "label '100' must fit vertically"
        );

        // Assert the exact glyphs "1","0","0" appear at that anchor.
        assert_glyphs(&img, label_x0, label_y0, &[1, 0, 0], scale);
    }

    // ----- 5.2c non-STEP-multiple dims: no panic, size correct -----
    #[test]
    fn grid_non_step_multiple_dims_no_panic() {
        // 13x17, cell=5: neither dim is a multiple of 10 -> last right/bottom THIN
        // boundary lands at out_w/out_h (off-image) and must be clamped, not panic.
        // scale=1, pad=1. has_col(13>=10)=true, has_row(17>=10)=true.
        //   max_row_label = (17/10)*10 = 10 -> row_digits=2 ; num_w(2)=2*4-1=7
        //   margin_left = 7 + 2 = 9 ; margin_top = 7
        //   out_w = 9 + 13*5 = 74 ; out_h = 7 + 17*5 = 92
        let w = 13u32;
        let h = 17u32;
        let cell = 5u32;
        let palette = palette_with(&[[9u8, 9, 9]]);
        let grid = BeadPattern {
            width: w,
            height: h,
            cells: vec![0u16; (w * h) as usize],
        };
        let opts = RenderOptions {
            cell_size: cell,
            shape: BeadShape::Square,
        };

        let bytes = render_grid(&grid, &palette, &opts).expect("render_grid ok (no panic)");
        let img = decode(&bytes);
        assert_eq!(img.width(), 9 + 13 * 5, "out_w");
        assert_eq!(img.height(), 7 + 17 * 5, "out_h");
    }

    // ----- 5.3 RenderOptions::default -----
    #[test]
    fn render_options_default() {
        let opts = RenderOptions::default();
        assert_eq!(opts.cell_size, 10);
        assert_eq!(opts.shape, BeadShape::Square);
    }

    // ----- 5.4 invalid input sentinel, not panic -----
    #[test]
    fn invalid_input_sentinel_not_panic() {
        let cell = 5u32; // valid for both preview and grid
        let opts = RenderOptions {
            cell_size: cell,
            shape: BeadShape::Square,
        };

        // (a) out-of-range index + smaller palette -> offending cell == MISSING.
        // 2x1 grid; palette has only 1 color (index 0). cell[1] = index 5 (oob).
        let palette_small = palette_with(&[[10u8, 20, 30]]);
        let grid_oob = BeadPattern {
            width: 2,
            height: 1,
            cells: vec![0, 5],
        };
        for which in ["preview", "grid"] {
            let bytes = if which == "preview" {
                render_preview(&grid_oob, &palette_small, &opts)
            } else {
                render_grid(&grid_oob, &palette_small, &opts)
            }
            .unwrap_or_else(|e| panic!("{which} must not error on oob index: {e:?}"));
            let img = decode(&bytes);
            // cell (1,0) is out of range -> MISSING. Both dims are < 10, so the
            // grid margins are 0 -> the cell origin is x*cell for both preview and
            // grid. Sample the center of the offending cell at column x=1.
            let (ox, oy) = (cell + cell / 2, cell / 2);
            assert_eq!(
                px(&img, ox, oy),
                MISSING,
                "{which}: out-of-range cell must be MISSING sentinel"
            );
        }

        // (b) too-short cells (len < w*h) -> missing cell == MISSING.
        // 2x1 grid but only 1 cell provided; position 1 is missing.
        let palette_ok = palette_with(&[[10u8, 20, 30]]);
        let grid_short = BeadPattern {
            width: 2,
            height: 1,
            cells: vec![0], // len 1 < 2
        };
        for which in ["preview", "grid"] {
            let bytes = if which == "preview" {
                render_preview(&grid_short, &palette_ok, &opts)
            } else {
                render_grid(&grid_short, &palette_ok, &opts)
            }
            .unwrap_or_else(|e| panic!("{which} must not error on short cells: {e:?}"));
            let img = decode(&bytes);
            // column x=1 origin = cell (both dims < 10 -> margins 0).
            let (ox, oy) = (cell + cell / 2, cell / 2);
            assert_eq!(
                px(&img, ox, oy),
                MISSING,
                "{which}: missing cell (short cells) must be MISSING sentinel"
            );
            // and the present cell (0) must be its real color
            assert_eq!(
                px(&img, cell / 2, cell / 2),
                [10, 20, 30],
                "{which}: present cell must be its palette color"
            );
        }

        // (c) empty palette -> whole image sentinel, no panic.
        let palette_empty = Palette {
            brand: "Empty".to_string(),
            colors: vec![],
        };
        let grid_any = BeadPattern {
            width: 2,
            height: 2,
            cells: vec![0, 0, 0, 0],
        };
        for which in ["preview", "grid"] {
            let bytes = if which == "preview" {
                render_preview(&grid_any, &palette_empty, &opts)
            } else {
                render_grid(&grid_any, &palette_empty, &opts)
            }
            .unwrap_or_else(|e| panic!("{which} must not error on empty palette: {e:?}"));
            let img = decode(&bytes);
            // Every bead cell center must be MISSING (margins are <10 dims -> 0).
            for y in 0..2u32 {
                for x in 0..2u32 {
                    assert_eq!(
                        px(&img, x * cell + cell / 2, y * cell + cell / 2),
                        MISSING,
                        "{which}: empty-palette cell ({x},{y}) must be MISSING"
                    );
                }
            }
        }
    }

    // ----- 5.5 degenerate / oversize -> Err, not panic -----
    #[test]
    fn degenerate_and_oversize_returns_err_not_panic() {
        let palette = palette_with(&[[1u8, 2, 3]]);

        // (a) width==0 / height==0 / cell_size==0 -> Err(InvalidImage) for both.
        let zero_w = BeadPattern {
            width: 0,
            height: 4,
            cells: vec![],
        };
        let zero_h = BeadPattern {
            width: 4,
            height: 0,
            cells: vec![],
        };
        let normal = BeadPattern {
            width: 4,
            height: 4,
            cells: vec![0u16; 16],
        };
        let ok_opts = RenderOptions {
            cell_size: 5,
            shape: BeadShape::Square,
        };
        let zero_cell = RenderOptions {
            cell_size: 0,
            shape: BeadShape::Square,
        };

        for (g, o, label) in [
            (&zero_w, &ok_opts, "width==0"),
            (&zero_h, &ok_opts, "height==0"),
            (&normal, &zero_cell, "cell_size==0"),
        ] {
            assert!(
                matches!(
                    render_preview(g, &palette, o),
                    Err(BeadError::InvalidImage { .. })
                ),
                "preview {label} must be Err(InvalidImage)"
            );
            assert!(
                matches!(
                    render_grid(g, &palette, o),
                    Err(BeadError::InvalidImage { .. })
                ),
                "grid {label} must be Err(InvalidImage)"
            );
        }

        // (b) grid cell_size in {1,2,3,4} -> grid Err, preview Ok.
        for cs in 1u32..=4 {
            let opts = RenderOptions {
                cell_size: cs,
                shape: BeadShape::Square,
            };
            assert!(
                matches!(
                    render_grid(&normal, &palette, &opts),
                    Err(BeadError::InvalidImage { .. })
                ),
                "grid cell_size={cs} must be Err(InvalidImage)"
            );
            assert!(
                render_preview(&normal, &palette, &opts).is_ok(),
                "preview cell_size={cs} must be Ok"
            );
        }

        // (c) oversize pub-constructed BeadPattern{cells:vec![]} -> Err, not panic,
        //     never reaching RgbImage::new.
        // (i) preview width=height=u32::MAX, cell_size=1.
        let huge1 = BeadPattern {
            width: u32::MAX,
            height: u32::MAX,
            cells: vec![],
        };
        assert!(
            matches!(
                render_preview(
                    &huge1,
                    &palette,
                    &RenderOptions {
                        cell_size: 1,
                        shape: BeadShape::Square
                    }
                ),
                Err(BeadError::InvalidImage { .. })
            ),
            "(i) preview u32::MAX x u32::MAX, cell 1 must be Err"
        );

        // (ii) width=height=250_000_000, cell_size=10.
        let huge2 = BeadPattern {
            width: 250_000_000,
            height: 250_000_000,
            cells: vec![],
        };
        let opts10 = RenderOptions {
            cell_size: 10,
            shape: BeadShape::Square,
        };
        assert!(
            matches!(
                render_preview(&huge2, &palette, &opts10),
                Err(BeadError::InvalidImage { .. })
            ),
            "(ii) preview 250M x 250M, cell 10 must be Err"
        );
        assert!(
            matches!(
                render_grid(&huge2, &palette, &opts10),
                Err(BeadError::InvalidImage { .. })
            ),
            "(ii) grid 250M x 250M, cell 10 must be Err"
        );

        // (iii) R3-B1 margin corner: render_grid on 10x10 with cell_size such that
        //       9*scale > u32::MAX, i.e. scale > u32::MAX/9. scale = cell/5, so
        //       cell ~ 2_386_092_945 -> scale ~ 477_218_589, 9*scale > u32::MAX.
        //       Margin computed in u128 must not overflow; result must be Err.
        let grid10 = BeadPattern {
            width: 10,
            height: 10,
            cells: vec![],
        };
        let huge_cell = RenderOptions {
            cell_size: 2_386_092_945,
            shape: BeadShape::Square,
        };
        assert!(
            matches!(
                render_grid(&grid10, &palette, &huge_cell),
                Err(BeadError::InvalidImage { .. })
            ),
            "(iii) grid margin-corner must be Err (margin computed in u128)"
        );

        // (iv) R3-M-ord ordering lock: width=height=cell_size=u32::MAX.
        //      Correct ordering returns Err at step ① (out_* > u32::MAX) and never
        //      multiplies bytes; a mis-ordered impl would u128-overflow-panic.
        let max_grid = BeadPattern {
            width: u32::MAX,
            height: u32::MAX,
            cells: vec![],
        };
        let max_cell = RenderOptions {
            cell_size: u32::MAX,
            shape: BeadShape::Square,
        };
        assert!(
            matches!(
                render_preview(&max_grid, &palette, &max_cell),
                Err(BeadError::InvalidImage { .. })
            ),
            "(iv) preview u32::MAX cubed must be Err (ordering: reject out before bytes)"
        );
        assert!(
            matches!(
                render_grid(&max_grid, &palette, &max_cell),
                Err(BeadError::InvalidImage { .. })
            ),
            "(iv) grid u32::MAX cubed must be Err (ordering: reject out before bytes)"
        );
    }

    // ----- 5.6 render only from cells (color comes from palette[cells[i]].rgb) -----
    #[test]
    fn render_only_from_cells() {
        let cell = 5u32;
        let opts = RenderOptions {
            cell_size: cell,
            shape: BeadShape::Square,
        };
        let cells: Vec<u16> = vec![0, 1, 0, 1];
        let grid = BeadPattern {
            width: 2,
            height: 2,
            cells,
        };

        // Two equal-length palettes with DIFFERENT rgb.
        let pal_a = palette_with(&[[10u8, 20, 30], [40, 50, 60]]);
        let pal_b = palette_with(&[[200u8, 100, 0], [0, 150, 250]]);

        for which in ["preview", "grid"] {
            let bytes_a = if which == "preview" {
                render_preview(&grid, &pal_a, &opts)
            } else {
                render_grid(&grid, &pal_a, &opts)
            }
            .expect("ok");
            let bytes_b = if which == "preview" {
                render_preview(&grid, &pal_b, &opts)
            } else {
                render_grid(&grid, &pal_b, &opts)
            }
            .expect("ok");

            assert_ne!(
                bytes_a, bytes_b,
                "{which}: different palettes must produce different pixels"
            );

            // And concretely: cell (0,0) pixel tracks each palette's color 0.
            let img_a = decode(&bytes_a);
            let img_b = decode(&bytes_b);
            // dims (<10) -> margins 0 in both -> origin x*cell.
            assert_eq!(
                px(&img_a, cell / 2, cell / 2),
                [10, 20, 30],
                "{which} A cell0"
            );
            assert_eq!(
                px(&img_b, cell / 2, cell / 2),
                [200, 100, 0],
                "{which} B cell0"
            );
        }
    }

    // ----- 5.7 deterministic (byte-equal same run + decoded pixels == expected) -----
    #[test]
    fn render_is_deterministic() {
        let cell = 6u32;
        let opts = RenderOptions {
            cell_size: cell,
            shape: BeadShape::Square,
        };
        // 3x2 grid; cell index 4 is OUT OF RANGE for a 3-color palette -> sentinel.
        let rgbs = [[11u8, 22, 33], [44, 55, 66], [77, 88, 99]];
        let palette = palette_with(&rgbs);
        let cells: Vec<u16> = vec![0, 1, 2, 4, 0, 1]; // index 4 oob at pos 3 = (0,1)
        let grid = BeadPattern {
            width: 3,
            height: 2,
            cells: cells.clone(),
        };

        // (a) byte-equal across two calls, for preview and grid.
        let p1 = render_preview(&grid, &palette, &opts).expect("ok");
        let p2 = render_preview(&grid, &palette, &opts).expect("ok");
        assert_eq!(p1, p2, "render_preview must be byte-deterministic");

        let g1 = render_grid(&grid, &palette, &opts).expect("ok");
        let g2 = render_grid(&grid, &palette, &opts).expect("ok");
        assert_eq!(g1, g2, "render_grid must be byte-deterministic");

        // (b) decoded preview pixels equal hand-computed expected (incl. sentinel).
        let img = decode(&p1);
        for y in 0..2u32 {
            for x in 0..3u32 {
                let idx = cells[(y * 3 + x) as usize] as usize;
                let expected = rgbs.get(idx).copied().unwrap_or(MISSING);
                // sample center of the cell
                assert_eq!(
                    px(&img, x * cell + cell / 2, y * cell + cell / 2),
                    expected,
                    "deterministic preview cell ({x},{y})"
                );
            }
        }
    }
}
