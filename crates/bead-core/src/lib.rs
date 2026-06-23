//! Beadsmith engine. Pure data in, pure data out — no UI, no filesystem,
//! no platform assumptions. See ARCHITECTURE.md.

use thiserror::Error;

/// Engine error type. Every public API returns `Result<T, BeadError>`.
#[derive(Debug, Error)]
pub enum BeadError {
    #[error("not yet implemented")]
    NotImplemented,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_displays() {
        assert_eq!(BeadError::NotImplemented.to_string(), "not yet implemented");
    }
}
