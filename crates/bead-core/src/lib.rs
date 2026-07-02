//! Beadsmith engine. Pure data in, pure data out — no UI, no filesystem,
//! no platform assumptions. See ARCHITECTURE.md.

use thiserror::Error;

pub mod image;
pub mod matcher;
pub mod models;
pub mod palette;
pub mod pipeline;
pub mod quantizer;
pub mod renderer;
pub mod statistics;

pub use image::{crop_center, decode_image, image_to_grid, resize_image, ResizeOptions};
pub use matcher::{match_pattern, ColorMatcher, MatcherKind, OklabMatcher, RgbMatcher};
pub use models::{BeadPattern, ColorStat, PixelGrid};
pub use palette::{load_palette, validate_palette, Palette, PaletteColor};
pub use pipeline::{generate_pattern, GenerateOptions, GenerateResult};
pub use quantizer::{BeadReducer, GreedyReducer};
pub use renderer::{render_grid, render_preview, BeadShape, RenderOptions};
pub use statistics::{count_colors, generate_summary, total_beads};

/// The shared engine error type. Fallible public APIs return
/// `Result<T, BeadError>`; total APIs (e.g. `match_pattern`,
/// `ColorMatcher::find_best_match`) return their value directly.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum BeadError {
    #[error("not yet implemented")]
    NotImplemented,

    /// Palette JSON failed to parse (syntax error, missing required field,
    /// or wrong type). Wraps the underlying `serde_json` error.
    #[error("failed to parse palette JSON")]
    PaletteParse(#[from] serde_json::Error),

    /// Palette parsed but violates a semantic invariant (empty colors,
    /// duplicate code, or malformed hex). `reason` is deterministic and
    /// names the single offending object.
    #[error("invalid palette: {reason}")]
    InvalidPalette { reason: String },

    /// Image bytes failed to decode (corrupt, truncated, or a format that
    /// was not compiled in). Wraps the underlying `image` crate error.
    #[error("failed to decode image")]
    ImageDecode(#[from] ::image::ImageError),

    /// Image decoded but a requested operation is semantically invalid
    /// (zero target dimension, zero-dimension source, a crop that floors to a
    /// zero dimension, or `max_colors == 0` passed to a quantizer). `reason`
    /// is deterministic and names the offending value.
    #[error("invalid image: {reason}")]
    InvalidImage { reason: String },

    /// PNG encoding failed. A dimension-guarded, valid in-memory buffer never
    /// actually triggers this; it exists only so the renderer does not panic on
    /// a reachable API (`ImageDecode` already owns `#[from] ::image::ImageError`,
    /// so this variant uses a named field to wrap the error manually rather than
    /// duplicating `#[from]`). See design D8.
    #[error("failed to encode image")]
    ImageEncode { source: ::image::ImageError },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_displays() {
        assert_eq!(BeadError::NotImplemented.to_string(), "not yet implemented");
    }
}
