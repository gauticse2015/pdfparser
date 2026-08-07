//! Product page/document table detection orchestrator.
//!
//! Detectors produce candidates; product Auto finalizes via Engine V2 exclusive
//! AutoRouter unless `legacy_router=true` forces soup NMS rollback.

#![allow(clippy::type_complexity)]

use super::classify::{
    borderless_passes_precision, is_chrome_lattice_fp, is_multitable_stream_recovery,
    is_solid_ruled_table, is_strong_lattice, looks_like_notice_metadata,
    looks_like_tax_form_fields, stream_mean_cell_chars, stream_numeric_density,
};
use super::engine_v2::{finalize_engine_v2, nms};
use super::geom_util::{containment_ratio, overlaps_any, region_overlap};
use super::prefer::{
    demote_lattice_column_slices, prefer_lattice_over_overlapping_hybrid,
    prefer_stream_over_sparse_hybrid, should_suppress_stream_under_lattices,
};
use crate::detectors::{
    HybridDetector, LatticeDetector, NetworkDetector, StreamDetector, TableDetector,
};
use crate::form::{apply_form_discriminator, scrub_document_table_fps};
use crate::geom;
use crate::options::TableOptions;
use crate::raster::RasterPage;
use crate::split::split_side_by_side;
use crate::stitch::{materialize_stitched, stitch_document};
use crate::types::{Table, TableMethod};
use pdfparser_content::RuleSegment;
use pdfparser_ir::TextRun;

/// Detect tables on a single page from text runs + rule segments.
///
/// Page size defaults to US Letter when unknown (`area_frac` only).
pub fn detect_tables_page(
    page_index: u32,
    runs: &[TextRun],
    rules: &[RuleSegment],
    opts: &TableOptions,
) -> Vec<Table> {
    detect_tables_page_with_raster(page_index, runs, rules, opts, &[], None)
}

/// Detect tables with optional raster page bitmaps (embedded images / renders).
///
/// When `opts.advanced.raster_line_detect` is true and `raster_pages` is non-empty, line
/// segments are recovered via morphology and merged into the lattice rule set.
///
/// `page_size` is `(width, height)` in user units (media box). When `None`,
/// area fractions use a US Letter stand-in.
pub fn detect_tables_page_with_raster(
    page_index: u32,
    runs: &[TextRun],
    rules: &[RuleSegment],
    opts: &TableOptions,
    raster_pages: &[RasterPage],
    page_size: Option<(f32, f32)>,
) -> Vec<Table> {
    if !opts.detect_tables {
        return Vec::new();
    }

    let mut cands = Vec::new();

    if opts.modes.lattice {
        let lat = LatticeDetector.detect(page_index, runs, rules, opts, raster_pages, page_size);
        // Phase-1: drop near-empty / form-chrome lattice before ownership.
        cands.extend(lat.into_iter().filter(|t| !is_chrome_lattice_fp(t)));
    }

    let strong_lattice_bboxes: Vec<pdfparser_ir::Rect> = cands
        .iter()
        .filter(|t| is_strong_lattice(t, opts))
        .map(|t| t.bbox)
        .collect();
    let has_strong_lattice = !strong_lattice_bboxes.is_empty();
    // Phase-1 V3: any solid ruled grid owns the page (Camelot-class exclusivity).
    // Borderless/stream must not co-emit when lattice already found a real table.
    let has_solid_ruled = cands.iter().any(|t| is_solid_ruled_table(t, opts));
    let ruled_owns_page =
        opts.advanced.exclusive_under_strong_lattice && (has_strong_lattice || has_solid_ruled);

    if opts.modes.hybrid {
        let hybrid = HybridDetector.detect(page_index, runs, rules, opts, raster_pages, page_size);
        if ruled_owns_page {
            // Hybrid only for non-overlapping partial regions outside ruled ownership.
            let own = if has_strong_lattice {
                strong_lattice_bboxes.clone()
            } else {
                cands
                    .iter()
                    .filter(|t| is_solid_ruled_table(t, opts))
                    .map(|t| t.bbox)
                    .collect()
            };
            for h in hybrid {
                if !overlaps_any(h.bbox, &own) {
                    cands.push(h);
                }
            }
        } else if !has_strong_lattice {
            cands.extend(hybrid);
        } else {
            for h in hybrid {
                if !overlaps_any(h.bbox, &strong_lattice_bboxes) {
                    cands.push(h);
                }
            }
        }
    }

    // Borderless path (Phase-2 dual mode):
    // - Recall mode when page has no solid ruled tables (recover ICDAR-class
    //   borderless / partial pages without reopening multi-detector soup).
    // - Precision / multitable-recovery mode when ruled owns the page.
    if opts.modes.stream {
        let borderless =
            NetworkDetector.detect(page_index, runs, rules, opts, raster_pages, page_size);
        let mut network_added = 0usize;
        let lattice_bboxes: Vec<pdfparser_ir::Rect> = cands
            .iter()
            .filter(|t| t.method == TableMethod::Lattice)
            .map(|t| t.bbox)
            .collect();
        let recall_mode = !ruled_owns_page && lattice_bboxes.is_empty();
        for mut s in borderless {
            if !borderless_passes_precision(&s, opts, recall_mode) {
                continue;
            }
            if ruled_owns_page {
                // Multi-table recovery only: dense numeric, little overlap with ruled.
                if !is_multitable_stream_recovery(&s, &lattice_bboxes) {
                    continue;
                }
                s.multitable_stream_recovery = true;
                s.notes.push("multitable_stream_recovery".into());
            } else if opts.advanced.exclusive_under_strong_lattice && has_strong_lattice {
                if should_suppress_stream_under_lattices(s.bbox, &strong_lattice_bboxes) {
                    continue;
                }
            } else if has_strong_lattice && overlaps_any(s.bbox, &strong_lattice_bboxes) {
                s.confidence *= 0.50;
                s.notes.push("demoted_under_lattice".into());
            }
            if has_strong_lattice && s.cols == 2 && stream_numeric_density(&s) < 0.10 {
                s.confidence *= 0.40;
                s.notes.push("demoted_weak_2col".into());
            }
            if recall_mode {
                s.notes.push("borderless_recall".into());
            }
            network_added += 1;
            cands.push(s);
        }
        // Classic stream fallback:
        //  - always when network empty and no solid lattice (Phase-1)
        //  - Phase-2 recall: also when network empty on non-ruled pages (even if
        //    weak lattice fragments exist), so painted/partial pages recover.
        let only_weak_lattice = !has_strong_lattice
            && !has_solid_ruled
            && cands
                .iter()
                .filter(|t| t.method == TableMethod::Lattice)
                .all(|t| t.cols <= 2 || t.bbox.width() < 140.0);
        let want_fallback = network_added == 0
            && (cands.is_empty() || only_weak_lattice || (recall_mode && !has_solid_ruled));
        if want_fallback && opts.allow_classic_stream {
            let classic =
                StreamDetector.detect(page_index, runs, rules, opts, raster_pages, page_size);
            for mut s in classic {
                let min_cols = if recall_mode { 2 } else { 3 };
                let min_rows = if recall_mode { 3 } else { 4 };
                if s.cols < min_cols || s.rows < min_rows {
                    continue;
                }
                if !borderless_passes_precision(&s, opts, recall_mode) {
                    continue;
                }
                let dup = cands.iter().any(|c| {
                    containment_ratio(s.bbox, c.bbox) >= 0.55
                        || containment_ratio(c.bbox, s.bbox) >= 0.55
                        || geom::iou(s.bbox, c.bbox) >= 0.40
                });
                if dup {
                    continue;
                }
                s.notes.push(if recall_mode {
                    "classic_stream_recall".into()
                } else {
                    "classic_stream_fallback".into()
                });
                cands.push(s);
            }
        }
    }

    // Phase-2: hybrid over-wide densify (campaign donors class) — force classic
    // stream recovery when network missed and hybrid exploded columns.
    let hybrid_over_wide = cands
        .iter()
        .any(|t| t.method == TableMethod::Hybrid && t.cols >= 14 && t.rows >= 10);
    let has_good_stream = cands.iter().any(|t| {
        matches!(t.method, TableMethod::Stream | TableMethod::DenseNumeric)
            && t.cols >= 3
            && t.cols <= 12
            && t.rows >= 4
    });
    if hybrid_over_wide && !has_good_stream && opts.modes.stream {
        // Prefer network first (better on Quartz/export tables), then classic (opt-in).
        let mut recovered =
            NetworkDetector.detect(page_index, runs, rules, opts, raster_pages, page_size);
        if recovered.is_empty() && opts.allow_classic_stream {
            recovered =
                StreamDetector.detect(page_index, runs, rules, opts, raster_pages, page_size);
        }
        for mut s in recovered {
            if s.cols < 3 || s.cols > 14 || s.rows < 8 {
                continue;
            }
            // Looser conf for recovering from hybrid densify explosion.
            if s.confidence < 0.50 {
                continue;
            }
            if looks_like_notice_metadata(&s) || looks_like_tax_form_fields(&s) {
                continue;
            }
            // Prefer multi-col filled grids over form strips
            let filled = s.cells.iter().filter(|c| !c.text.trim().is_empty()).count();
            if filled < 20 {
                continue;
            }
            s.stream_vs_overwide_hybrid = true;
            s.notes.push("stream_vs_overwide_hybrid".into());
            cands.push(s);
        }
    }

    // Prefer lattice over hybrid when they heavily overlap (sensing 95).
    cands = prefer_lattice_over_overlapping_hybrid(cands);
    // Prefer dense multi-col stream/network over sparse over-wide hybrid that
    // re-fragmented the same borderless region (Quartz/Tabula stream PDFs).
    cands = prefer_stream_over_sparse_hybrid(cands);
    // Drop remaining over-wide hybrids when a reasonable stream coexists.
    let stream_refs: Vec<(pdfparser_ir::Rect, u32, u32)> = cands
        .iter()
        .filter(|s| matches!(s.method, TableMethod::Stream | TableMethod::DenseNumeric))
        .map(|s| (s.bbox, s.cols, s.rows))
        .collect();
    cands.retain(|t| {
        if t.method == TableMethod::Hybrid && t.cols >= 14 {
            !stream_refs.iter().any(|&(sb, sc, sr)| {
                sc < t.cols
                    && sr >= ((t.rows as f32) * 0.5) as u32
                    && (geom::iou(sb, t.bbox) >= 0.25 || region_overlap(sb, t.bbox) >= 0.35)
            })
        } else {
            true
        }
    });

    if opts.advanced.side_by_side_split {
        cands = split_side_by_side(cands, runs, opts);
    }
    if opts.advanced.form_discriminator {
        cands = apply_form_discriminator(cands, opts);
    }
    // Phase-4: if form disc removed all solid lattice that had suppressed
    // borderless, re-admit only *dense multi-col numeric* network tables
    // (NIPA glued GDP grids). Tight gates prevent arxiv/NIST prose FPs.
    if opts.modes.stream
        && !cands.iter().any(|t| {
            matches!(t.method, TableMethod::Lattice | TableMethod::Hybrid)
                && is_solid_ruled_table(t, opts)
        })
        && !cands
            .iter()
            .any(|t| matches!(t.method, TableMethod::Stream | TableMethod::DenseNumeric))
    {
        let recovered =
            NetworkDetector.detect(page_index, runs, rules, opts, raster_pages, page_size);
        for mut s in recovered {
            if !borderless_passes_precision(&s, opts, true) {
                continue;
            }
            // Strict: multi-col statistical grids only (not 2–3 col notices).
            let num = stream_numeric_density(&s);
            let mean = stream_mean_cell_chars(&s);
            if s.cols < 8 || s.rows < 10 || s.confidence < 0.70 {
                continue;
            }
            if num < 0.40 || mean > 28.0 {
                continue;
            }
            let filled = s.cells.iter().filter(|c| !c.text.trim().is_empty()).count();
            if filled < 40 {
                continue;
            }
            s.notes.push("stream_recover_after_form".into());
            cands.push(s);
        }
    }
    // Phase 12: demote narrow high-row lattice slices when a wider multi-col
    // stream/network table already covers the page (census / dual-region FPs).
    cands = demote_lattice_column_slices(cands);

    // Engine V2 exclusive AutoRouter (product Auto post-flip).
    // Rollback: opts.legacy_router = true → soup NMS below.
    if opts.use_engine_v2 && !opts.legacy_router {
        return finalize_engine_v2(cands, opts, rules, raster_pages, page_size);
    }

    let min_conf = opts
        .advanced
        .min_confidence_stream
        .min(opts.min_table_confidence);
    let mut kept = nms(cands, min_conf, opts.advanced.nms_containment_frac);
    kept.retain(|t| match t.method {
        TableMethod::Stream => t.confidence >= opts.advanced.min_confidence_stream,
        _ => t.confidence >= opts.min_table_confidence,
    });
    kept.truncate(opts.max_tables_per_page as usize);
    kept
}

/// Detect tables for all pages; optional stitch and over-seg scrub.
///
/// This entry point has no raster bitmaps (runs + rules only). Image-line
/// sensing is a no-op here — use [`detect_tables_document_with_raster`] or the
/// `pdfparser` façade `document_tables` for embedded-image grids.
pub fn detect_tables_document(
    pages: &[(u32, &[TextRun], &[RuleSegment])],
    page_heights: &[f32],
    opts: &TableOptions,
) -> (Vec<Vec<Table>>, Vec<Table>) {
    let mut page_tables: Vec<Vec<Table>> = pages
        .iter()
        .map(|(idx, runs, rules)| {
            detect_tables_page_with_raster(*idx, runs, rules, opts, &[], None)
        })
        .collect();

    if opts.stitch_multipage {
        stitch_document(&mut page_tables, page_heights, opts);
    }

    let mut logical = if opts.stitch_multipage {
        materialize_stitched(&page_tables)
    } else {
        page_tables.iter().flatten().cloned().collect()
    };
    if opts.advanced.form_discriminator {
        logical = scrub_document_table_fps(logical, opts);
    }
    (page_tables, logical)
}

/// Document-level detect with per-page raster bitmaps for line sensing.
pub fn detect_tables_document_with_raster(
    pages: &[(u32, &[TextRun], &[RuleSegment], &[RasterPage])],
    page_heights: &[f32],
    opts: &TableOptions,
) -> (Vec<Vec<Table>>, Vec<Table>) {
    let mut page_tables: Vec<Vec<Table>> = pages
        .iter()
        .map(|(idx, runs, rules, rasters)| {
            detect_tables_page_with_raster(*idx, runs, rules, opts, rasters, None)
        })
        .collect();

    if opts.stitch_multipage {
        stitch_document(&mut page_tables, page_heights, opts);
    }

    let mut logical = if opts.stitch_multipage {
        materialize_stitched(&page_tables)
    } else {
        page_tables.iter().flatten().cloned().collect()
    };
    if opts.advanced.form_discriminator {
        logical = scrub_document_table_fps(logical, opts);
    }
    (page_tables, logical)
}

#[cfg(test)]
mod phase12_slice_tests {
    use super::super::engine_v2::emit_tables_from_accepted;
    use super::super::prefer::demote_lattice_column_slices;
    use super::*;
    use crate::evidence::{ProposalOrigin, RegionKind, RegionProposal};
    use pdfparser_ir::Rect;

    fn dummy_table(method: TableMethod, rows: u32, cols: u32, bbox: Rect, conf: f32) -> Table {
        let mut cells = Vec::new();
        for r in 0..rows {
            for c in 0..cols {
                cells.push(crate::types::TableCell {
                    row: r,
                    col: c,
                    rowspan: 1,
                    colspan: 1,
                    text: format!("{r},{c}"),
                    bbox: Rect {
                        x0: bbox.x0 + c as f32,
                        y0: bbox.y0 + r as f32,
                        x1: bbox.x0 + c as f32 + 1.0,
                        y1: bbox.y0 + r as f32 + 1.0,
                    },
                    is_header: false,
                    confidence: conf,
                });
            }
        }
        Table {
            bbox,
            page: 0,
            method,
            confidence: conf,
            rows,
            cols,
            cells,
            header_rows: 0,
            continued_from_previous_page: false,
            continued_to_next_page: false,
            logical_table_id: None,
            strategy_provenance: vec![],
            notes: vec![],
            edge_score: 0.8,
            fill_rate: 0.5,
            weak_edges: false,
            joint_count: 0,
            text_row_recovery: false,
            text_col_recovery: false,
            multitable_stream_recovery: false,
            stream_vs_overwide_hybrid: false,
        }
    }

    #[test]
    fn emit_from_accepted_picks_one_source_after_merge() {
        // K26-style multi-source proposal must emit a single best table.
        let a = dummy_table(
            TableMethod::Lattice,
            5,
            4,
            Rect {
                x0: 50.0,
                y0: 400.0,
                x1: 350.0,
                y1: 450.0,
            },
            0.80,
        );
        let mut b = dummy_table(
            TableMethod::Lattice,
            20,
            4,
            Rect {
                x0: 52.0,
                y0: 200.0,
                x1: 348.0,
                y1: 390.0,
            },
            0.92,
        );
        b.joint_count = 40;
        let cands = vec![a, b];
        let accepted = vec![RegionProposal {
            kind: RegionKind::RuledContour,
            bbox: Rect {
                x0: 50.0,
                y0: 200.0,
                x1: 350.0,
                y1: 450.0,
            },
            line_score: 0.9,
            text_score: 0.5,
            joint_count: 40,
            area_frac: 0.1,
            whitespace_est: 0.1,
            origin: ProposalOrigin::Detector,
            source_indices: vec![0, 1],
        }];
        let out = emit_tables_from_accepted(&cands, &accepted);
        assert_eq!(out.len(), 1, "merged sources → one emit");
        assert!((out[0].confidence - 0.92).abs() < 1e-6);
    }

    #[test]
    fn emit_skips_contour_seed_without_sources() {
        let cands = vec![dummy_table(
            TableMethod::Lattice,
            4,
            4,
            Rect {
                x0: 0.0,
                y0: 0.0,
                x1: 100.0,
                y1: 100.0,
            },
            0.9,
        )];
        let accepted = vec![RegionProposal {
            kind: RegionKind::RuledContour,
            bbox: Rect {
                x0: 0.0,
                y0: 0.0,
                x1: 100.0,
                y1: 100.0,
            },
            line_score: 0.7,
            text_score: 0.0,
            joint_count: 8,
            area_frac: 0.05,
            whitespace_est: 0.0,
            origin: ProposalOrigin::ContourSeed,
            source_indices: vec![],
        }];
        let out = emit_tables_from_accepted(&cands, &accepted);
        assert!(out.is_empty(), "seed-only proposals do not invent tables");
    }

    #[test]
    fn demote_lattice_column_slices_drops_skinny() {
        let stream = dummy_table(
            TableMethod::Stream,
            20,
            6,
            Rect {
                x0: 30.0,
                y0: 50.0,
                x1: 400.0,
                y1: 400.0,
            },
            0.9,
        );
        // Overlapping y-range with stream (same region slice), not vertically disjoint.
        let lattice = dummy_table(
            TableMethod::Lattice,
            30,
            2,
            Rect {
                x0: 140.0,
                y0: 100.0,
                x1: 300.0,
                y1: 350.0,
            },
            0.91,
        );
        let out = demote_lattice_column_slices(vec![stream, lattice]);
        assert_eq!(
            out.len(),
            1,
            "skinny lattice dropped when overlapping wide stream"
        );
        assert_eq!(out[0].method, TableMethod::Stream);
    }

    #[test]
    fn demote_keeps_vertically_disjoint_skinny_lattice() {
        let stream = dummy_table(
            TableMethod::Stream,
            20,
            6,
            Rect {
                x0: 30.0,
                y0: 50.0,
                x1: 400.0,
                y1: 250.0,
            },
            0.9,
        );
        let lattice = dummy_table(
            TableMethod::Lattice,
            30,
            2,
            Rect {
                x0: 140.0,
                y0: 340.0,
                x1: 300.0,
                y1: 590.0,
            },
            0.91,
        );
        let out = demote_lattice_column_slices(vec![stream, lattice]);
        assert_eq!(out.len(), 2, "disjoint lower lattice kept for multi-table");
    }

    #[test]
    fn demote_lattice_column_slices_keeps_wide_lattice() {
        let stream = dummy_table(
            TableMethod::Stream,
            10,
            4,
            Rect {
                x0: 30.0,
                y0: 50.0,
                x1: 200.0,
                y1: 200.0,
            },
            0.7,
        );
        let lattice = dummy_table(
            TableMethod::Lattice,
            15,
            5,
            Rect {
                x0: 30.0,
                y0: 220.0,
                x1: 500.0,
                y1: 600.0,
            },
            0.9,
        );
        let out = demote_lattice_column_slices(vec![stream, lattice]);
        assert!(out
            .iter()
            .any(|t| t.method == TableMethod::Lattice && t.cols == 5));
    }

    #[test]
    fn demote_noop_without_wide_stream() {
        let lattice = dummy_table(
            TableMethod::Lattice,
            30,
            2,
            Rect {
                x0: 100.0,
                y0: 100.0,
                x1: 250.0,
                y1: 500.0,
            },
            0.9,
        );
        let out = demote_lattice_column_slices(vec![lattice]);
        assert_eq!(out.len(), 1);
    }
}

#[cfg(test)]
mod phase13_strong_lattice {
    use super::super::classify::is_strong_lattice;
    use crate::options::TableOptions;
    use crate::types::{Table, TableMethod};
    use pdfparser_ir::Rect;

    fn tab(cols: u32, rows: u32, conf: f32) -> Table {
        Table {
            bbox: Rect {
                x0: 0.0,
                y0: 0.0,
                x1: 100.0,
                y1: 100.0,
            },
            page: 0,
            method: TableMethod::Lattice,
            confidence: conf,
            rows,
            cols,
            cells: vec![],
            header_rows: 0,
            continued_from_previous_page: false,
            continued_to_next_page: false,
            logical_table_id: None,
            strategy_provenance: vec![],
            notes: vec![],
            edge_score: 0.9,
            fill_rate: 0.5,
            weak_edges: false,
            joint_count: 0,
            text_row_recovery: false,
            text_col_recovery: false,
            multitable_stream_recovery: false,
            stream_vs_overwide_hybrid: false,
        }
    }

    #[test]
    fn is_strong_lattice_wide_two_col_not_tiny_corner() {
        let o = TableOptions::default();
        // Wide 2-col (side-by-side fixture ~150u)
        let mut wide = tab(2, 5, 0.99);
        wide.bbox = Rect {
            x0: 100.0,
            y0: 0.0,
            x1: 250.0,
            y1: 80.0,
        };
        assert!(is_strong_lattice(&wide, &o));
        // Tiny corner fragment (~100u) must NOT be strong
        let mut tiny = tab(2, 3, 0.99);
        tiny.bbox = Rect {
            x0: 24.0,
            y0: 435.0,
            x1: 126.0,
            y1: 580.0,
        };
        assert!(!is_strong_lattice(&tiny, &o));
        assert!(is_strong_lattice(&tab(3, 5, 0.99), &o));
    }

    #[test]
    fn suppress_stream_keeps_large_over_tiny_lattice() {
        let tiny = pdfparser_ir::Rect {
            x0: 24.0,
            y0: 435.0,
            x1: 126.0,
            y1: 580.0,
        };
        let large = pdfparser_ir::Rect {
            x0: 20.0,
            y0: 400.0,
            x1: 580.0,
            y1: 720.0,
        };
        assert!(
            !super::should_suppress_stream_under_lattices(large, &[tiny]),
            "full-width stream must survive tiny lattice corner"
        );
        let inside = pdfparser_ir::Rect {
            x0: 30.0,
            y0: 440.0,
            x1: 120.0,
            y1: 570.0,
        };
        assert!(super::should_suppress_stream_under_lattices(
            inside,
            &[tiny]
        ));
    }
}
