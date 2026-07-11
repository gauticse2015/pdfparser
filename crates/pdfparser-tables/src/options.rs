//! Table detection options and presets.
//!
//! Tunable thresholds live here so detectors stay free of silent magic numbers.
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
    /// Whitespace stream (S3).
    pub stream: bool,
    /// Hybrid partial borders (S4).
    pub hybrid: bool,
}

impl TableModeSet {
    /// All detectors.
    pub fn all() -> Self {
        Self {
            structure: true,
            lattice: true,
            stream: true,
            hybrid: true,
        }
    }

    /// Lattice-only.
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

/// Progressive presets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum TablePreset {
    /// Off.
    Off,
    /// Lattice only.
    LatticeOnly,
    /// Lattice + stream.
    LatticeStream,
    /// Full pipeline.
    Full,
}

/// Public table options.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct TableOptions {
    /// Master switch (default false).
    pub detect_tables: bool,
    /// Detector bits.
    pub modes: TableModeSet,
    /// Min confidence to emit (lattice/hybrid).
    pub min_table_confidence: f32,
    /// Min confidence for stream candidates.
    pub min_confidence_stream: f32,
    /// Cap candidates kept per page after NMS.
    pub max_tables_per_page: u32,
    /// Snap tolerance for lattice lines (user units).
    pub line_snap_tol: f32,
    /// Min cell size (user units).
    pub min_cell_size: f32,
    /// Multi-page stitch (D1).
    pub stitch_multipage: bool,
    /// Form vs table discriminator (P1).
    pub form_discriminator: bool,
    /// Side-by-side empty-gutter split (P4).
    pub side_by_side_split: bool,

    // --- Over-segmentation / data-table scrub (document-level) ---
    /// When logical table count exceeds this, low-scoring candidates are dropped.
    /// Neutral default matches design soft-max per-page scale.
    pub overseg_trigger: u32,
    /// After over-seg filtering, keep at most this many logical tables (0 = unlimited).
    pub max_logical_tables: u32,
    /// Minimum data-table score (0..1) to retain when over-segmented.
    pub min_data_table_score: f32,

    // --- Split / stitch geometry ---
    /// Minimum absolute gutter width (user units) for side-by-side split.
    pub min_gutter_gap: f32,
    /// Gutter must be at least this fraction of median column width.
    pub min_gutter_vs_col: f32,
    /// Fraction of page height treated as top/bottom band for multi-page stitch.
    pub stitch_band_frac: f32,
    /// Max mean column-center delta (user units) for multi-page stitch.
    pub stitch_max_col_dx: f32,
    /// Min header text similarity for multi-page stitch.
    pub stitch_min_header_sim: f32,

    // --- Stream prose / layout filters ---
    /// Reject 2-col stream cells with mean chars above this and low numeric density.
    pub stream_max_prose_mean_chars: f32,
    /// Min column-separation score for stream emission.
    pub stream_min_col_sep: f32,
    /// Min multi-column body bands for stream detection.
    pub stream_min_body_bands: u32,
    /// Vertical gap (as multiple of median font size) that splits stream regions.
    pub stream_region_gap_font_mult: f32,
    /// Absolute floor (user units) for stream multi-region vertical gap.
    pub stream_region_gap_min: f32,

    // --- Lattice geometry ---
    /// Min axis-aligned segment length (user units) to participate in lattice.
    pub lattice_min_seg_len: f32,
    /// Expand segment ends when testing H∩V joints (broken-corner recovery).
    pub lattice_joint_gap: f32,
    /// Minimum joints (line crossings) for a connected component to be a table.
    pub lattice_min_joints: u32,
    /// Max lattice rows (safety cap).
    pub lattice_max_rows: u32,
    /// Max lattice cols (safety cap).
    pub lattice_max_cols: u32,
    /// Reject grids with empty-cell fraction ≥ this and fewer than `lattice_min_filled_cells`.
    pub lattice_empty_frac_reject: f32,
    /// Min filled cells when applying empty-fraction reject.
    pub lattice_min_filled_cells: u32,
    /// Min fill rate (else reject unless filled cells ≥ min filled).
    pub lattice_min_fill_rate: f32,
    /// Edge completeness below this ⇒ `weak_edges` (orchestration).
    pub lattice_weak_edge_threshold: f32,
    /// Min fraction of a cell side that must be covered by a rule to count as edged.
    pub lattice_edge_cover_frac: f32,

    // --- Hybrid ---
    /// When hybrid recovers ≥3×3, confidence is at least this (NMS fairness vs weak lattice).
    pub hybrid_min_conf_when_grid: f32,

    // --- NMS / strong lattice ---
    /// Min confidence for a lattice table to count as "strong" (blocks overlapping hybrid).
    pub strong_lattice_min_conf: f32,
}

impl Default for TableOptions {
    fn default() -> Self {
        Self {
            detect_tables: false,
            modes: TableModeSet::lattice_only(),
            min_table_confidence: 0.55,
            min_confidence_stream: 0.50,
            max_tables_per_page: 32,
            line_snap_tol: 2.0,
            min_cell_size: 3.0,
            stitch_multipage: true,
            form_discriminator: true,
            side_by_side_split: true,
            overseg_trigger: 8,
            max_logical_tables: 32,
            min_data_table_score: 0.42,
            min_gutter_gap: 15.0,
            min_gutter_vs_col: 0.6,
            stitch_band_frac: 0.30,
            stitch_max_col_dx: 12.0,
            stitch_min_header_sim: 0.85,
            stream_max_prose_mean_chars: 70.0,
            stream_min_col_sep: 0.30,
            stream_min_body_bands: 3,
            stream_region_gap_font_mult: 2.5,
            stream_region_gap_min: 16.0,
            lattice_min_seg_len: 5.0,
            lattice_joint_gap: 3.5,
            lattice_min_joints: 4,
            lattice_max_rows: 80,
            lattice_max_cols: 40,
            lattice_empty_frac_reject: 0.90,
            lattice_min_filled_cells: 4,
            lattice_min_fill_rate: 0.08,
            lattice_weak_edge_threshold: 0.55,
            lattice_edge_cover_frac: 0.45,
            hybrid_min_conf_when_grid: 0.72,
            strong_lattice_min_conf: 0.65,
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
                modes: TableModeSet {
                    structure: false,
                    lattice: true,
                    stream: true,
                    hybrid: true,
                },
                ..Self::default()
            },
        }
    }
}
