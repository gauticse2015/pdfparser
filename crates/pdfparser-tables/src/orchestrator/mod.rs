//! Product table page orchestrator.
//!
//! Product [`crate::TablePreset::Auto`] / [`crate::TablePreset::Full`] run
//! detectors then Engine V2 exclusive AutoRouter (`use_engine_v2 && !legacy_router`).
//! Set `TableOptions.legacy_router = true` to force soup NMS rollback.
//!
//! Historical name: this module replaced `legacy::` after Gate G1 flip.

mod page;

pub use page::{
    detect_tables_document, detect_tables_document_with_raster, detect_tables_page,
    detect_tables_page_with_raster,
};
