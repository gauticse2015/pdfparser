//! Core errors.
use crate::limits::LimitKind;
use thiserror::Error;

/// Result alias.
pub type Result<T> = std::result::Result<T, Error>;

/// Core/public errors.
#[derive(Debug, Error)]
pub enum Error {
    /// I/O.
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    /// Syntax / parse.
    #[error("syntax error: {0}")]
    Syntax(String),
    /// Resource limit.
    #[error("limit exceeded: {kind:?}")]
    LimitExceeded {
        /// Kind.
        kind: LimitKind,
    },
    /// Encrypted PDF (K15).
    #[error("encrypted PDF is not supported in this version")]
    Encryption,
    /// Page index.
    #[error("page out of range: {index}")]
    PageOutOfRange {
        /// Index.
        index: u32,
    },
    /// Unsupported.
    #[error("unsupported: {0}")]
    Unsupported(String),
    /// Internal.
    #[error("internal: {0}")]
    Internal(String),
}
