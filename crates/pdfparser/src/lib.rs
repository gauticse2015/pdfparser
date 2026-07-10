//! pdfparser — native PDF extraction library (Phase T text + Phase V tables).
//!
//! # Example
//! ```no_run
//! use pdfparser::{Document, TextOptions};
//! let doc = Document::open("file.pdf").unwrap();
//! let text = doc.page(0).unwrap().text(&TextOptions::default()).unwrap();
//! println!("{text}");
//! ```
#![deny(missing_docs)]

mod document;
mod extract;
mod font_load;
mod options;

pub use document::{Document, Page};
pub use extract::extract_document;
pub use options::{ExtractOptions, OpenOptions, TextOptions};
pub use pdfparser_core::{Error, LimitKind, ResourceLimits, Result};
pub use pdfparser_export::{to_json, to_json_pretty};
pub use pdfparser_ir::{
    DocumentMetadata, Element, ExtractWarning, ExtractedDocument, ExtractedPage, Matrix3x2,
    ObjectId, Point, Rect, TextRun, WarningCode, SCHEMA_VERSION,
};
pub use pdfparser_tables::{
    Table, TableCell, TableMethod, TableModeSet, TableOptions, TablePreset,
};

/// Crate version.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
