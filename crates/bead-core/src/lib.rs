//! Beadsmith engine. Pure data in, pure data out — no UI, no filesystem,
//! no platform assumptions. See ARCHITECTURE.md.

use thiserror::Error;

pub mod image;
pub mod models;
pub mod palette;

pub use image::{crop_center, decode_image, image_to_grid, resize_image, ResizeOptions};
pub use models::PixelGrid;
pub use palette::{load_palette, validate_palette, Palette, PaletteColor};

/// Engine error type. Every public API returns `Result<T, BeadError>`.
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
    /// (zero target dimension, zero-dimension source, or a crop that floors
    /// to a zero dimension). `reason` is deterministic and names the
    /// offending dimension.
    #[error("invalid image: {reason}")]
    InvalidImage { reason: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_displays() {
        assert_eq!(BeadError::NotImplemented.to_string(), "not yet implemented");
    }
}
