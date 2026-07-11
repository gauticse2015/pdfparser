//! Table detection options and presets.
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// Which detectors to run.
#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct TableModeSet {
    /// Structure tree tables (unused today).
    pub structure: bool,
    /// Ruled lattice.
    pub lattice: bool,
    /// Borderless (network).
    pub stream: bool,
    /// Hybrid partial borders.
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
    /// Lattice + borderless network.
    LatticeStream,
    /// Full production pipeline.
    Full,
    /// Product default (same as Full).
    Auto,
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
    /// Min confidence for borderless/network.
    pub min_confidence_stream: f32,
    /// Cap tables kept per page after NMS.
    pub max_tables_per_page: u32,
    /// Snap tolerance for lattice lines (user units).
    pub line_snap_tol: f32,
    /// Min cell size (user units).
    pub min_cell_size: f32,
    /// Multi-page stitch.
    pub stitch_multipage: bool,
    /// Form vs table discriminator.
    pub form_discriminator: bool,
    /// Side-by-side empty-gutter split.
    pub side_by_side_split: bool,
    /// Over-seg trigger count.
    pub overseg_trigger: u32,
    /// Max logical tables after scrub (0 = unlimited).
    pub max_logical_tables: u32,
    /// Min data-table score under over-seg.
    pub min_data_table_score: f32,
    /// Min absolute gutter for side-by-side split.
    pub min_gutter_gap: f32,
    /// Gutter as fraction of median column width.
    pub min_gutter_vs_col: f32,
    /// Stitch band as fraction of page height.
    pub stitch_band_frac: f32,
    /// Max mean column-center delta for stitch.
    pub stitch_max_col_dx: f32,
    /// Min header similarity for stitch.
    pub stitch_min_header_sim: f32,
    /// Reject prose-like cells above this mean char count (2-col).
    pub stream_max_prose_mean_chars: f32,
    /// Min column-separation score for stream/network.
    pub stream_min_col_sep: f32,
    /// Min multi-column body bands.
    pub stream_min_body_bands: u32,
    /// Vertical gap (× font size) that splits borderless regions.
    pub stream_region_gap_font_mult: f32,
    /// Absolute floor for borderless region gap.
    pub stream_region_gap_min: f32,
    /// Min rule segment length for lattice.
    pub lattice_min_seg_len: f32,
    /// Joint gap expand for broken corners.
    pub lattice_joint_gap: f32,
    /// Min joints per connected component.
    pub lattice_min_joints: u32,
    /// Max lattice rows.
    pub lattice_max_rows: u32,
    /// Max lattice cols.
    pub lattice_max_cols: u32,
    /// Empty-cell fraction reject threshold.
    pub lattice_empty_frac_reject: f32,
    /// Min filled cells with empty-frac reject.
    pub lattice_min_filled_cells: u32,
    /// Min fill rate.
    pub lattice_min_fill_rate: f32,
    /// Edge score below this ⇒ weak_edges.
    pub lattice_weak_edge_threshold: f32,
    /// Min fraction of cell side covered by a rule.
    pub lattice_edge_cover_frac: f32,
    /// Min joints along a candidate grid line.
    pub lattice_min_joints_per_line: u32,
    /// Min table area (user units²).
    pub lattice_min_table_area: f32,
    /// Tiny grid reject max side.
    pub lattice_min_side_for_tiny_reject: u32,
    /// Tiny grid reject max filled cells.
    pub lattice_tiny_max_filled: u32,
    /// Hybrid confidence floor when ≥3×3.
    pub hybrid_min_conf_when_grid: f32,
    /// Lattice confidence for “strong” (blocks overlapping borderless).
    pub strong_lattice_min_conf: f32,
    /// Containment NMS fraction.
    pub nms_containment_frac: f32,
    /// If strong lattice exists, drop overlapping borderless tables.
    pub exclusive_under_strong_lattice: bool,
    /// Collapse overdense H-lines using text bands (false underlines).
    pub lattice_collapse_overdense_h: bool,
    /// Trigger overdense-H collapse when H-rows > text_bands × factor.
    pub lattice_overdense_h_factor: f32,

    // --- Raster line sensing (Camelot-class morphology on page images) ---
    /// Recover H/V rules from raster bitmaps (embedded Image XObjects).
    ///
    /// Pipeline: adaptive threshold → morph close (dashed) → directional open →
    /// run extract → joint-graph filter → regularity gate → lattice merge.
    pub raster_line_detect: bool,
    /// Adaptive threshold window half-size (pixels). Typical 6–10.
    pub raster_adaptive_radius: usize,
    /// Adaptive threshold bias (darker than local mean − bias → ink). Typical 8–15.
    pub raster_adaptive_bias: u8,
    /// Floor for morph kernel length (pixels); actual kernel scales with image size.
    pub raster_min_kernel: usize,
    /// Floor for min emitted segment length (pixels).
    pub raster_min_seg_px: usize,
    /// Merge collinear raster runs within this gap (pixels).
    pub raster_merge_gap_px: usize,
    /// Snap tolerance when clustering raster line positions (pixels).
    pub raster_pos_snap_px: f32,
    /// Keep lattice grids with little/no extractable text when rules came from raster
    /// (text is painted into the image). Only strong multi-cell grids pass.
    pub raster_allow_empty_cells: bool,
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
            stream_region_gap_font_mult: 4.0,
            stream_region_gap_min: 24.0,
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
            lattice_min_joints_per_line: 2,
            lattice_min_table_area: 900.0,
            lattice_min_side_for_tiny_reject: 2,
            lattice_tiny_max_filled: 4,
            hybrid_min_conf_when_grid: 0.72,
            strong_lattice_min_conf: 0.65,
            nms_containment_frac: 0.82,
            exclusive_under_strong_lattice: true,
            lattice_collapse_overdense_h: true,
            lattice_overdense_h_factor: 1.35,
            raster_line_detect: true,
            raster_adaptive_radius: 6,
            raster_adaptive_bias: 12,
            raster_min_kernel: 15,
            raster_min_seg_px: 8,
            raster_merge_gap_px: 4,
            raster_pos_snap_px: 2.0,
            raster_allow_empty_cells: true,
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
            TablePreset::Full | TablePreset::Auto => Self {
                detect_tables: true,
                modes: TableModeSet {
                    structure: false,
                    lattice: true,
                    stream: true,
                    hybrid: true,
                },
                exclusive_under_strong_lattice: true,
                // Multipage continued tables are a product feature.
                stitch_multipage: true,
                ..Self::default()
            },
        }
    }
}
