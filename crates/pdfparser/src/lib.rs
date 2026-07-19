//! pdfparser — native PDF extraction (text, tables, images/links/forms/outline).
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
mod raster_images;

pub use document::{Document, Page};
pub use extract::extract_document;
pub use options::{ExtractOptions, OpenOptions, TextOptions};
pub use pdfparser_core::{
    DocumentObjects, Error, FormField, ImageObject, LimitKind, LinkAnnotation, ResourceLimits,
    Result,
};
pub use pdfparser_export::{to_json, to_json_pretty};
pub use pdfparser_ir::{
    DocumentMetadata, Element, ExtractWarning, ExtractedDocument, ExtractedPage, Matrix3x2,
    ObjectId, Point, Rect, TextRun, WarningCode, SCHEMA_VERSION,
};
pub use pdfparser_tables::{
    detect_tables_page_with_diagnostics,
    page_evidence_from_inputs,
    // Engine V2 foundation (diagnostics / evidence); product Auto uses V2 router.
    EvidenceDiagnostics,
    MethodMix,
    PageEvidence,
    Table,
    TableCell,
    TableMethod,
    TableModeSet,
    TableOptions,
    TablePreset,
    TableTuning,
    TABLE_TUNING_KEYS,
};

/// Crate version.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
