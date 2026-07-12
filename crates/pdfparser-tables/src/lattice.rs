//! Lattice detector (legacy name): thin adapter over ruled-table extract.
//!
//! Production path and public API stay at [`detect_lattice_tables`]. Core logic
//! lives in [`crate::builders::ruled::detect_ruled_tables`] (PR4a extract parity).

pub use crate::builders::ruled::detect_ruled_tables as detect_lattice_tables;
