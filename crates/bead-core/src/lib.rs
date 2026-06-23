//! Beadsmith engine. Pure data in, pure data out — no UI, no filesystem,
//! no platform assumptions. See ARCHITECTURE.md.

use thiserror::Error;

pub mod palette;

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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_displays() {
        assert_eq!(BeadError::NotImplemented.to_string(), "not yet implemented");
    }
}
