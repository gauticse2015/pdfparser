//! PDF table extraction: lattice (ruled) + hybrid (partial) + network (borderless).
//!
//! # Production path ([`TablePreset::Auto`] / [`TablePreset::Full`])
//!
//! 1. Lattice on vector rules (incl. thin-fill painted rules) + optional raster lines
//! 2. Hybrid only outside strong lattice
//! 3. Network borderless (with classic stream fallback/supplement)
//! 4. **Engine V2 exclusive AutoRouter** (K26 merge + ownership partition + nested keep)
//!
//! Rollback to soup NMS: `TableOptions.legacy_router = true`.
//! See `docs/design-table-engine-v2.md`.
#![deny(missing_docs)]

mod form;
mod geom;
mod hybrid;
pub mod builders;
mod lattice;
mod network;
mod options;
mod raster;
mod split;
mod stitch;
mod stream;
mod types;

pub mod evidence;
pub mod orchestrator;
pub mod policy;
pub mod router;
pub mod providers;

/// Historical name for [`orchestrator`] (pre-G1 module path). Prefer `orchestrator`.
pub mod legacy {
    pub use crate::orchestrator::*;
}

pub use form::scrub_document_table_fps;
pub use lattice::detect_lattice_tables;
pub use network::detect_network_tables;
pub use options::{TableModeSet, TableOptions, TablePreset};
pub use raster::{
    config_for_raster_page, gray_from_rgb, gray_from_rgba, merge_rules, rules_from_raster,
    RasterConfig, RasterPage, RasterRule,
};
pub use stitch::{materialize_stitched, stitch_document};
pub use providers::{
    ExternalCliPageRenderer, NullPageRenderer, PageRenderer, ProviderError, RenderSafety,
};
/// Classic whitespace stream detector.
///
/// **Deprecated for product use:** production Auto/Full uses
/// [`detect_network_tables`] when `modes.stream` is set. Prefer network.
/// This export remains for experiments and will be feature-gated later.
#[doc(alias = "experimental_stream")]
pub use stream::detect_stream_tables;
pub use types::{PipelineId, Table, TableCell, TableMethod};

pub use evidence::{
    page_evidence_from_inputs, EvidenceDiagnostics, LineEvidence, LineSourceKind, MethodMix,
    OrientedSeg, PageEvidence, ProposalOrigin, RegionKind, RegionProposal,
};
pub use orchestrator::{
    detect_tables_document, detect_tables_document_with_raster, detect_tables_page,
    detect_tables_page_with_raster,
};
pub use policy::{
    intersection_area, is_hard_owner, is_nested_table_pair, ownership_blocks, overlaps_owned,
    pad_rect, rect_area, rect_iou, ProposalPolicy,
};
pub use router::{
    cmp_emit_order, emit_order_key, kind_priority, kinds_mergeable, partition, passes_gate,
    route_proposals, sort_rects_by_emit_order, sort_tables_by_emit_order, structure_prior,
    vertical_merge, x_iou, y_gap, DEFAULT_X_IOU_MIN, Y_GAP_MEDIAN_MULT,
};

/// Whether the table engine is available.
pub fn tables_available() -> bool {
    true
}

/// Detect tables with optional shadow diagnostics (method mix / rule counts).
///
/// Runs the product page orchestrator (Engine V2 exclusive router when
/// `use_engine_v2 && !legacy_router`). Pass real media-box width/height so
/// area gates use the correct page area.
pub fn detect_tables_page_with_diagnostics(
    page_index: u32,
    runs: &[pdfparser_ir::TextRun],
    rules: &[pdfparser_content::RuleSegment],
    opts: &TableOptions,
    raster_pages: &[RasterPage],
    page_width: f32,
    page_height: f32,
) -> (Vec<Table>, EvidenceDiagnostics) {
    let mut evidence = page_evidence_from_inputs(
        page_index,
        page_width,
        page_height,
        runs,
        rules,
        raster_pages,
    );
    let page_size = if page_width > 1.0 && page_height > 1.0 {
        Some((page_width, page_height))
    } else {
        None
    };
    let tables =
        detect_tables_page_with_raster(page_index, runs, rules, opts, raster_pages, page_size);
    evidence.diagnostics.method_mix = MethodMix::from_tables(&tables);
    evidence.diagnostics.strong_lattice_fired = tables.iter().any(|t| {
        t.method == TableMethod::Lattice
            && t.cols >= 2
            && t.rows >= 2
            && t.confidence >= opts.strong_lattice_min_conf
            && !t.weak_edges
    });
    evidence.diagnostics.engine_path = if opts.use_engine_v2 && !opts.legacy_router {
        "engine_v2".into()
    } else {
        "legacy".into()
    };
    if opts.shadow_diagnostics {
        evidence.diagnostics.notes.push("shadow_diagnostics".into());
    }
    (tables, evidence.diagnostics)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pdfparser_content::RuleSegment;
    use pdfparser_ir::{Matrix3x2, Rect, TextRun};

    fn tr(text: &str, x0: f32, y0: f32, x1: f32, y1: f32) -> TextRun {
        TextRun {
            text: text.into(),
            bbox: Rect { x0, y0, x1, y1 },
            transform: Matrix3x2::identity(),
            font_name: None,
            font_size: 10.0,
            mapping_confidence: 1.0,
            metrics_confidence: 1.0,
            mcid: None,
            invisible: false,
            from_actual_text: false,
        }
    }

    fn rule(x0: f32, y0: f32, x1: f32, y1: f32) -> RuleSegment {
        RuleSegment { x0, y0, x1, y1 }
    }

    fn lattice_grid(
        x0: f32,
        y0: f32,
        rows: u32,
        cols: u32,
        cell_w: f32,
        cell_h: f32,
    ) -> (Vec<TextRun>, Vec<RuleSegment>) {
        let mut runs = Vec::new();
        let mut rules = Vec::new();
        let x1 = x0 + cols as f32 * cell_w;
        let y1 = y0 + rows as f32 * cell_h;
        for r in 0..=rows {
            let y = y0 + r as f32 * cell_h;
            rules.push(rule(x0, y, x1, y));
        }
        for c in 0..=cols {
            let x = x0 + c as f32 * cell_w;
            rules.push(rule(x, y0, x, y1));
        }
        for r in 0..rows {
            for c in 0..cols {
                let cx0 = x0 + c as f32 * cell_w + 4.0;
                let top_y0 = y1 - (r as f32 + 1.0) * cell_h + 4.0;
                let top_y1 = top_y0 + 10.0;
                runs.push(tr(&format!("r{r}c{c}"), cx0, top_y0, cx0 + 20.0, top_y1));
            }
        }
        (runs, rules)
    }

    #[test]
    fn tables_available_true() {
        assert!(tables_available());
    }

    #[test]
    fn detect_off_returns_empty() {
        let (runs, rules) = lattice_grid(50.0, 200.0, 4, 3, 60.0, 20.0);
        assert!(detect_tables_page(0, &runs, &rules, &TableOptions::default()).is_empty());
    }

    #[test]
    fn detect_lattice_page() {
        let (runs, rules) = lattice_grid(50.0, 200.0, 5, 4, 70.0, 22.0);
        let opts = TableOptions::from_preset(TablePreset::Full);
        let tabs = detect_tables_page(0, &runs, &rules, &opts);
        assert!(!tabs.is_empty());
        assert!(matches!(
            tabs[0].method,
            TableMethod::Lattice | TableMethod::Hybrid
        ));
        assert!(tabs[0].rows >= 3 && tabs[0].cols >= 3);
    }

    #[test]
    fn auto_finds_ruled_grid() {
        let (runs, rules) = lattice_grid(50.0, 200.0, 5, 4, 70.0, 22.0);
        let opts = TableOptions::from_preset(TablePreset::Auto);
        let tabs = detect_tables_page(0, &runs, &rules, &opts);
        assert!(!tabs.is_empty());
        assert!(tabs.iter().any(|t| t.method == TableMethod::Lattice));
    }

    /// Engine V2 path: same lattice_grid fixture as [`auto_finds_ruled_grid`].
    #[test]
    fn engine_v2_finds_lattice_grid() {
        let (runs, rules) = lattice_grid(50.0, 200.0, 5, 4, 70.0, 22.0);
        let opts = TableOptions::from_preset(TablePreset::EngineV2);
        assert!(opts.use_engine_v2);
        assert!(!opts.legacy_router);
        let tabs = detect_tables_page(0, &runs, &rules, &opts);
        assert!(!tabs.is_empty(), "EngineV2 should find lattice grid");
        assert!(
            tabs.iter().any(|t| t.method == TableMethod::Lattice),
            "expected Lattice method, got {:?}",
            tabs.iter().map(|t| t.method).collect::<Vec<_>>()
        );
        assert!(
            tabs.iter()
                .any(|t| t.notes.iter().any(|n| n == "engine_v2_router")),
            "EngineV2 tables should carry engine_v2_router note"
        );
    }

    /// Auto/Full post-flip use Engine V2 router (same path as EngineV2 preset).
    #[test]
    fn auto_full_use_engine_v2_router() {
        let (runs, rules) = lattice_grid(50.0, 200.0, 5, 4, 70.0, 22.0);
        for preset in [TablePreset::Auto, TablePreset::Full] {
            let opts = TableOptions::from_preset(preset);
            assert!(opts.use_engine_v2, "{preset:?}");
            assert!(!opts.legacy_router, "{preset:?}");
            let tabs = detect_tables_page(0, &runs, &rules, &opts);
            assert!(!tabs.is_empty(), "{preset:?} should find grid");
            assert!(
                tabs.iter().any(|t| t.method == TableMethod::Lattice),
                "{preset:?} expected Lattice"
            );
            assert!(
                tabs.iter()
                    .any(|t| t.notes.iter().any(|n| n == "engine_v2_router")),
                "{preset:?} must tag engine_v2_router"
            );
        }
    }

    #[test]
    fn engine_v2_diagnostics_engine_path() {
        let (runs, rules) = lattice_grid(50.0, 200.0, 5, 4, 70.0, 22.0);
        let opts = TableOptions::from_preset(TablePreset::EngineV2);
        let (tabs, diag) =
            detect_tables_page_with_diagnostics(0, &runs, &rules, &opts, &[], 612.0, 792.0);
        assert!(!tabs.is_empty());
        assert_eq!(diag.engine_path, "engine_v2");
    }

    #[test]
    fn presets() {
        assert!(!TableOptions::from_preset(TablePreset::Off).detect_tables);
        assert!(
            TableOptions::from_preset(TablePreset::LatticeOnly)
                .modes
                .lattice
        );
        assert!(TableOptions::from_preset(TablePreset::Full).modes.hybrid);
        assert!(TableOptions::from_preset(TablePreset::Auto).exclusive_under_strong_lattice);
        assert_eq!(
            TableOptions::from_preset(TablePreset::Auto).modes.lattice,
            TableOptions::from_preset(TablePreset::Full).modes.lattice
        );
        // EngineV2 / Auto share exclusive router; EngineV2 enables shadow diagnostics.
        let v2 = TableOptions::from_preset(TablePreset::EngineV2);
        assert!(v2.detect_tables);
        assert!(v2.use_engine_v2);
        assert!(!v2.legacy_router);
        assert!(v2.shadow_diagnostics);
        assert!(v2.allow_auto_render);
        assert!(!v2.enable_full_page_render);
        let auto = TableOptions::from_preset(TablePreset::Auto);
        assert!(auto.use_engine_v2);
        assert!(!auto.legacy_router);
        // HighQuality = EngineV2 + full-page render request.
        let hq = TableOptions::from_preset(TablePreset::HighQuality);
        assert!(hq.use_engine_v2);
        assert!(hq.shadow_diagnostics);
        assert!(hq.allow_auto_render);
        assert!(hq.enable_full_page_render);
        assert!(TableOptions::default().allow_auto_render);
    }

    #[test]
    fn page_evidence_and_shadow_diagnostics() {
        let (runs, rules) = lattice_grid(50.0, 200.0, 5, 4, 70.0, 22.0);
        let mut opts = TableOptions::from_preset(TablePreset::Auto);
        opts.shadow_diagnostics = true;
        let (tabs, diag) =
            detect_tables_page_with_diagnostics(0, &runs, &rules, &opts, &[], 612.0, 792.0);
        assert!(!tabs.is_empty());
        assert!(diag.vector_rule_count >= 2);
        assert_eq!(diag.engine_path, "engine_v2");
        assert!(diag.method_mix.total() >= 1);
    }

    #[test]
    fn line_evidence_from_rules() {
        let rules = [rule(0.0, 0.0, 100.0, 0.0), rule(0.0, 0.0, 0.0, 50.0)];
        let le = LineEvidence::from_rules(&rules);
        assert_eq!(le.count_h(1.5), 1);
        assert_eq!(le.count_v(1.5), 1);
    }

    /// Product Auto/Full flipped to Engine V2 router (nested multi-table parity).
    #[test]
    fn auto_preset_uses_engine_v2_router() {
        let auto = TableOptions::from_preset(TablePreset::Auto);
        assert!(auto.use_engine_v2, "Auto uses Engine V2 router post-flip");
        assert!(
            !auto.legacy_router,
            "Auto legacy_router=false post-flip (set true to rollback)"
        );
        let full = TableOptions::from_preset(TablePreset::Full);
        assert!(full.use_engine_v2);
        assert!(!full.legacy_router);
    }

    /// Phase 16: rollback switch forces legacy path even when EngineV2 flags set.
    #[test]
    fn phase16_legacy_router_rollback() {
        let (runs, rules) = lattice_grid(50.0, 200.0, 5, 4, 70.0, 22.0);
        let mut opts = TableOptions::from_preset(TablePreset::EngineV2);
        assert!(opts.use_engine_v2);
        assert!(!opts.legacy_router);
        // Rollback: force legacy orchestrator
        opts.legacy_router = true;
        let (tabs, diag) =
            detect_tables_page_with_diagnostics(0, &runs, &rules, &opts, &[], 612.0, 792.0);
        assert!(!tabs.is_empty());
        assert_eq!(
            diag.engine_path, "legacy",
            "legacy_router=true must force engine_path=legacy"
        );
        assert!(
            tabs.iter()
                .all(|t| !t.notes.iter().any(|n| n == "engine_v2_router")),
            "rollback must not emit engine_v2_router notes"
        );
    }
}
