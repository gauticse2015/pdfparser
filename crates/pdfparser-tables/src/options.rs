//! Table detection options and presets.
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// Which detectors to run.
#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct TableModeSet {
    /// Structure tree tables (S1).
    pub structure: bool,
    /// Ruled lattice (S2).
    pub lattice: bool,
    /// Whitespace stream (S3) — Phase V.
    pub stream: bool,
    /// Hybrid partial borders (S4) — Phase V.
    pub hybrid: bool,
}

impl TableModeSet {
    /// All detectors (stream/hybrid may no-op until Phase V).
    pub fn all() -> Self {
        Self {
            structure: true,
            lattice: true,
            stream: true,
            hybrid: true,
        }
    }

    /// Lattice-only (Phase U default when tables on).
    pub fn lattice_only() -> Self {
        Self {
            structure: false,
            lattice: true,
            stream: false,
            hybrid: false,
        }
    }
}

impl Default for TableModeSet {
    fn default() -> Self {
        Self::lattice_only()
    }
}

/// Progressive presets (R2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum TablePreset {
    /// Off.
    Off,
    /// Lattice only (Phase U).
    LatticeOnly,
    /// Lattice + stream (Phase V).
    LatticeStream,
    /// Full pipeline.
    Full,
}

/// Public table options.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct TableOptions {
    /// Master switch (default false — K21).
    pub detect_tables: bool,
    /// Detector bits.
    pub modes: TableModeSet,
    /// Min confidence to emit.
    pub min_table_confidence: f32,
    /// Cap per page.
    pub max_tables_per_page: u32,
    /// Snap tolerance for lattice lines (user units).
    pub line_snap_tol: f32,
    /// Min cell size.
    pub min_cell_size: f32,
}

impl Default for TableOptions {
    fn default() -> Self {
        Self {
            detect_tables: false,
            modes: TableModeSet::lattice_only(),
            min_table_confidence: 0.55,
            max_tables_per_page: 32,
            line_snap_tol: 2.0,
            min_cell_size: 3.0,
        }
    }
}

impl TableOptions {
    /// Apply a preset.
    pub fn from_preset(preset: TablePreset) -> Self {
        match preset {
            TablePreset::Off => Self::default(),
            TablePreset::LatticeOnly => Self {
                detect_tables: true,
                modes: TableModeSet::lattice_only(),
                ..Self::default()
            },
            TablePreset::LatticeStream => Self {
                detect_tables: true,
                modes: TableModeSet {
                    structure: false,
                    lattice: true,
                    stream: true,
                    hybrid: false,
                },
                ..Self::default()
            },
            TablePreset::Full => Self {
                detect_tables: true,
                modes: TableModeSet::all(),
                ..Self::default()
            },
        }
    }
}
