//! Table detection options and presets.
//!
//! # Product surface (GATE-5 / G5.3)
//!
//! [`TableOptions`] exposes **≤12 top-level public fields**. Numeric detector
//! knobs live on nested [`TableAdvancedOptions`] and are reachable via
//! [`std::ops::Deref`] / [`DerefMut`] so existing `opts.lattice_min_joints`
//! call sites keep compiling. Serde flattens advanced knobs for JSON compat.
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};
use std::ops::{Deref, DerefMut};

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
    /// Product default: Engine V2 exclusive router (same as [`EngineV2`]).
    ///
    /// Flipped when real_structure Auto ≡ EngineV2 (nested multi-table + G1 parity).
    /// Rollback: set `TableOptions.legacy_router = true`.
    Auto,
    /// Engine V2 path (exclusive AutoRouter). Equivalent to product [`Auto`] post-flip.
    ///
    /// Builders remain the production lattice/hybrid/network stack; routing is
    /// exclusive partition with nested-lattice keep. See `docs/design-table-engine-v2.md`.
    EngineV2,
    /// Best-effort quality path: same as [`EngineV2`] but requests full-page render
    /// line sensing when the compile feature is available (`enable_full_page_render=true`).
    ///
    /// Pure-Rust / feature-off builds ignore the render request fail-soft.
    HighQuality,
    /// Latency path: Engine V2 router, **never** full-page render (even opportunistic).
    ///
    /// Same builders as Auto; hard-off for render probes. Use for batch / p95 budgets.
    Fast,
}

/// Nested detector knobs (not part of the ≤12 product surface).
///
/// Accessed via [`TableOptions`] Deref, or `opts.advanced.*`. Subject to
/// deprecation / policy versioning; prefer presets for product code.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct TableAdvancedOptions {
    /// Min confidence for borderless/network.
    pub min_confidence_stream: f32,
    /// Snap tolerance for lattice lines (user units).
    pub line_snap_tol: f32,
    /// Min cell size (user units).
    pub min_cell_size: f32,
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
    /// Text densify X/Y recovery for partial ruled grids (stub cols / missing H).
    pub lattice_text_densify: bool,
    /// Recover H/V rules from raster bitmaps (embedded Image XObjects).
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
    /// Keep lattice grids with little/no extractable text when rules came from raster.
    pub raster_allow_empty_cells: bool,
    /// Document-type tunable geometry / densify thresholds (settings dict).
    ///
    /// System defaults; override per call via [`crate::TableTuning::apply_overrides`]
    /// or CLI `--table-setting key=value` for complex document classes.
    pub tuning: crate::TableTuning,
}

impl Default for TableAdvancedOptions {
    fn default() -> Self {
        Self {
            min_confidence_stream: 0.50,
            line_snap_tol: 2.0,
            min_cell_size: 3.0,
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
            lattice_text_densify: true,
            raster_line_detect: true,
            raster_adaptive_radius: 6,
            raster_adaptive_bias: 12,
            raster_min_kernel: 15,
            raster_min_seg_px: 8,
            raster_merge_gap_px: 4,
            raster_pos_snap_px: 2.0,
            raster_allow_empty_cells: true,
            tuning: crate::TableTuning::default(),
        }
    }
}

/// Public table options — **≤12 top-level fields** (product surface).
///
/// Advanced knobs: [`TableAdvancedOptions`] via [`Self::advanced`] or Deref.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct TableOptions {
    /// Master switch (default false).
    pub detect_tables: bool,
    /// Detector bits.
    pub modes: TableModeSet,
    /// Min confidence to emit (lattice/hybrid).
    pub min_table_confidence: f32,
    /// Cap tables kept per page after NMS.
    pub max_tables_per_page: u32,
    /// Multi-page stitch.
    pub stitch_multipage: bool,
    /// Runtime opt-in for full-page render line sensing.
    ///
    /// Compile feature alone must never enable render. Set by [`TablePreset::HighQuality`].
    pub enable_full_page_render: bool,
    /// Allow opportunistic full-page render (K25) when feature is compiled and probes fire.
    ///
    /// Embedders set `false` for a hard off. Default `true`. Independent of
    /// [`Self::enable_full_page_render`] (explicit on). [`TablePreset::Fast`] sets false.
    pub allow_auto_render: bool,
    /// Force legacy orchestrator even if `use_engine_v2` (rollback switch).
    pub legacy_router: bool,
    /// Prefer Engine V2 exclusive router (product Auto post-flip).
    pub use_engine_v2: bool,
    /// Permit classic whitespace stream as network fallback.
    ///
    /// Product Auto/Full/EngineV2/HQ/Fast keep this **false** (G5.4). Network is the
    /// borderless path; classic stream remains available for experiments / LatticeStream.
    pub allow_classic_stream: bool,
    /// Collect shadow diagnostics (method mix, rule counts) without changing routing.
    ///
    /// Enabled by default on [`TablePreset::EngineV2`] and [`TablePreset::HighQuality`].
    pub shadow_diagnostics: bool,
    /// Nested detector knobs (serde-flattened for JSON compat).
    #[cfg_attr(feature = "serde", serde(flatten))]
    pub advanced: TableAdvancedOptions,
}

/// Names of the product surface fields (GATE-5 G5.3).
pub const PRODUCT_TABLE_OPTION_FIELDS: &[&str] = &[
    "detect_tables",
    "modes",
    "min_table_confidence",
    "max_tables_per_page",
    "stitch_multipage",
    "enable_full_page_render",
    "allow_auto_render",
    "legacy_router",
    "use_engine_v2",
    "allow_classic_stream",
    "shadow_diagnostics",
    "advanced",
];

impl Default for TableOptions {
    fn default() -> Self {
        Self {
            detect_tables: false,
            modes: TableModeSet::lattice_only(),
            min_table_confidence: 0.55,
            max_tables_per_page: 32,
            stitch_multipage: true,
            enable_full_page_render: false,
            allow_auto_render: true,
            legacy_router: true,
            use_engine_v2: false,
            allow_classic_stream: false,
            shadow_diagnostics: false,
            advanced: TableAdvancedOptions::default(),
        }
    }
}

impl Deref for TableOptions {
    type Target = TableAdvancedOptions;
    fn deref(&self) -> &Self::Target {
        &self.advanced
    }
}

impl DerefMut for TableOptions {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.advanced
    }
}

impl TableOptions {
    /// Top-level product field count (must be ≤12 for GATE-5).
    pub const fn product_field_count() -> usize {
        PRODUCT_TABLE_OPTION_FIELDS.len()
    }

    /// Override one tuning setting (document-type accuracy knobs).
    ///
    /// See [`crate::TableTuning`] / [`crate::TABLE_TUNING_KEYS`].
    pub fn with_tuning_override(mut self, key: &str, value: f64) -> Result<Self, String> {
        self.advanced.tuning.set(key, value)?;
        Ok(self)
    }

    /// Apply a settings dict of tuning overrides (key → value).
    pub fn apply_tuning_overrides<I, K>(&mut self, pairs: I) -> Result<(), String>
    where
        I: IntoIterator<Item = (K, f64)>,
        K: AsRef<str>,
    {
        self.advanced.tuning.apply_overrides(pairs)
    }

    /// Parse CLI-style `key=value` fragments into [`Self::tuning`].
    pub fn apply_tuning_kv_string(&mut self, s: &str) -> Result<(), String> {
        self.advanced.tuning.apply_kv_string(s)
    }

    /// Apply a preset.
    pub fn from_preset(preset: TablePreset) -> Self {
        match preset {
            TablePreset::Off => Self::default(),
            TablePreset::LatticeOnly => Self {
                detect_tables: true,
                modes: TableModeSet::lattice_only(),
                allow_classic_stream: false,
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
                // LatticeStream is the explicit classic/network experiment path.
                allow_classic_stream: true,
                ..Self::default()
            },
            // Post flip: Auto/Full use exclusive Engine V2 router (parity with EngineV2).
            TablePreset::Full | TablePreset::Auto => {
                let mut o = Self {
                    detect_tables: true,
                    modes: TableModeSet {
                        structure: false,
                        lattice: true,
                        stream: true,
                        hybrid: true,
                    },
                    max_tables_per_page: 12,
                    stitch_multipage: true,
                    use_engine_v2: true,
                    legacy_router: false,
                    shadow_diagnostics: false,
                    enable_full_page_render: false,
                    allow_auto_render: true,
                    allow_classic_stream: false,
                    ..Self::default()
                };
                o.exclusive_under_strong_lattice = true;
                o.min_confidence_stream = 0.62;
                o.stream_max_prose_mean_chars = 48.0;
                o
            }
            TablePreset::EngineV2 => {
                let mut o = Self {
                    detect_tables: true,
                    modes: TableModeSet {
                        structure: false,
                        lattice: true,
                        stream: true,
                        hybrid: true,
                    },
                    max_tables_per_page: 12,
                    stitch_multipage: true,
                    use_engine_v2: true,
                    legacy_router: false,
                    shadow_diagnostics: true,
                    enable_full_page_render: false,
                    allow_auto_render: true,
                    allow_classic_stream: false,
                    ..Self::default()
                };
                o.exclusive_under_strong_lattice = true;
                o.min_confidence_stream = 0.62;
                o.stream_max_prose_mean_chars = 48.0;
                o
            }
            TablePreset::HighQuality => {
                let mut o = Self {
                    detect_tables: true,
                    modes: TableModeSet {
                        structure: false,
                        lattice: true,
                        stream: true,
                        hybrid: true,
                    },
                    max_tables_per_page: 12,
                    stitch_multipage: true,
                    use_engine_v2: true,
                    legacy_router: false,
                    shadow_diagnostics: true,
                    enable_full_page_render: true,
                    allow_auto_render: true,
                    allow_classic_stream: false,
                    ..Self::default()
                };
                o.exclusive_under_strong_lattice = true;
                o.min_confidence_stream = 0.62;
                o.stream_max_prose_mean_chars = 48.0;
                o
            }
            TablePreset::Fast => {
                let mut o = Self {
                    detect_tables: true,
                    modes: TableModeSet {
                        structure: false,
                        lattice: true,
                        stream: true,
                        hybrid: true,
                    },
                    max_tables_per_page: 12,
                    stitch_multipage: true,
                    use_engine_v2: true,
                    legacy_router: false,
                    shadow_diagnostics: false,
                    // Hard off: no explicit and no opportunistic full-page render.
                    enable_full_page_render: false,
                    allow_auto_render: false,
                    allow_classic_stream: false,
                    ..Self::default()
                };
                o.exclusive_under_strong_lattice = true;
                o.min_confidence_stream = 0.62;
                o.stream_max_prose_mean_chars = 48.0;
                o
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn product_surface_at_most_12() {
        assert!(
            TableOptions::product_field_count() <= 12,
            "G5.3 product surface fields = {}",
            TableOptions::product_field_count()
        );
        assert_eq!(PRODUCT_TABLE_OPTION_FIELDS.len(), 12);
    }

    #[test]
    fn auto_disables_classic_stream() {
        for p in [
            TablePreset::Auto,
            TablePreset::Full,
            TablePreset::EngineV2,
            TablePreset::HighQuality,
            TablePreset::Fast,
        ] {
            let o = TableOptions::from_preset(p);
            assert!(
                !o.allow_classic_stream,
                "{p:?} must not enable classic stream (G5.4)"
            );
        }
    }

    #[test]
    fn fast_never_renders() {
        let o = TableOptions::from_preset(TablePreset::Fast);
        assert!(!o.enable_full_page_render);
        assert!(!o.allow_auto_render);
        assert!(o.use_engine_v2);
        assert!(!o.legacy_router);
    }

    #[test]
    fn deref_advanced_knobs() {
        let mut o = TableOptions::default();
        o.lattice_min_joints = 9;
        assert_eq!(o.advanced.lattice_min_joints, 9);
        assert_eq!(o.lattice_min_joints, 9);
    }

    #[test]
    fn tuning_override_via_options() {
        let mut o = TableOptions::from_preset(TablePreset::Auto);
        o.apply_tuning_kv_string("densify_y_skip_numeric_frac=0.05")
            .unwrap();
        assert!((o.tuning.densify_y_skip_numeric_frac - 0.05).abs() < 1e-6);
        let o2 = TableOptions::from_preset(TablePreset::Auto)
            .with_tuning_override("densify_x_explode_abs_cols", 20.0)
            .unwrap();
        assert_eq!(o2.tuning.densify_x_explode_abs_cols, 20);
    }
}
