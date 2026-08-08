//! Lattice detector (legacy name): thin adapter over ruled-table extract.
//!
//! **Kept** as the public-name shim ([`detect_lattice_tables`]); do not inline
//! or delete this module. Core logic lives in
//! [`crate::builders::ruled::detect_ruled_tables`] (PR4a extract parity).

pub use crate::builders::ruled::detect_ruled_tables as detect_lattice_tables;
