//! Table builders: algorithm cores that turn page geometry into tables.
//!
//! - [`ruled`] — joint-CC lattice / ruled-grid extract
//! - [`densify`] — text densify X/Y, thin-gap collapse, empty interior columns
//! - [`ruled`] — lattice builder + topology trim helpers

pub mod densify;
pub mod ruled;

pub use ruled::detect_ruled_tables;
