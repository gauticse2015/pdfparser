//! Product page/document table detection orchestrator.
//!
//! Detectors (lattice / hybrid / network / classic stream) produce candidates;
//! product Auto then finalizes via Engine V2 exclusive AutoRouter unless
//! `legacy_router=true` forces soup NMS rollback.

use crate::evidence::{ProposalOrigin, RegionKind, RegionProposal};
use crate::form::{apply_form_discriminator, scrub_document_table_fps};
use crate::geom;
use crate::hybrid::detect_hybrid_tables;
use crate::lattice::detect_lattice_tables;
use crate::network::detect_network_tables;
use crate::options::TableOptions;
use crate::policy::{is_nested_table_pair, ProposalPolicy};
use crate::raster::RasterPage;
use crate::router::{partition, sort_tables_by_emit_order, vertical_merge, DEFAULT_X_IOU_MIN};
use crate::split::split_side_by_side;
use crate::stitch::{materialize_stitched, stitch_document};
use crate::stream::detect_stream_tables;
use crate::types::{Table, TableMethod};
use pdfparser_content::RuleSegment;
use pdfparser_ir::{Rect, TextRun};
use std::collections::HashSet;

/// Letter-page area stand-in when page dims are not available for `area_frac`.
const PAGE_AREA_EST: f32 = 612.0 * 792.0;

/// Placeholder median text-line gap for K26 (user units) when page stats absent.
const ROUTER_MEDIAN_LINE_GAP: f32 = 12.0;

/// Detect tables on a single page from text runs + rule segments.
///
/// Page size defaults to US Letter when unknown (area_frac only).
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
/// When `opts.raster_line_detect` is true and `raster_pages` is non-empty, line
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
        let lat = detect_lattice_tables(page_index, runs, rules, opts, raster_pages);
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
        opts.exclusive_under_strong_lattice && (has_strong_lattice || has_solid_ruled);

    if opts.modes.hybrid {
        let hybrid = detect_hybrid_tables(page_index, runs, rules, opts);
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
        let borderless = detect_network_tables(page_index, runs, opts);
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
            } else if opts.exclusive_under_strong_lattice && has_strong_lattice {
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
            let classic = detect_stream_tables(page_index, runs, opts);
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
        let mut recovered = detect_network_tables(page_index, runs, opts);
        if recovered.is_empty() && opts.allow_classic_stream {
            recovered = detect_stream_tables(page_index, runs, opts);
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

    if opts.side_by_side_split {
        cands = split_side_by_side(cands, runs, opts);
    }
    if opts.form_discriminator {
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
        let recovered = detect_network_tables(page_index, runs, opts);
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

    let min_conf = opts.min_confidence_stream.min(opts.min_table_confidence);
    let mut kept = nms(cands, min_conf, opts.nms_containment_frac);
    kept.retain(|t| match t.method {
        TableMethod::Stream => t.confidence >= opts.min_confidence_stream,
        _ => t.confidence >= opts.min_table_confidence,
    });
    kept.truncate(opts.max_tables_per_page as usize);
    kept
}

/// Engine V2 finalize: proposals → K26 vertical_merge → exclusive partition →
/// identity-based emit → exclusive cleanup → K27 emit order.
fn finalize_engine_v2(
    mut cands: Vec<Table>,
    opts: &TableOptions,
    rules: &[RuleSegment],
    raster_pages: &[RasterPage],
    page_size: Option<(f32, f32)>,
) -> Vec<Table> {
    let min_conf = opts.min_confidence_stream.min(opts.min_table_confidence);
    cands.retain(|t| t.confidence >= min_conf);
    cands.retain(|t| match t.method {
        TableMethod::Stream => t.confidence >= opts.min_confidence_stream,
        _ => t.confidence >= opts.min_table_confidence,
    });

    let page_area = page_area(page_size);
    let policy = ProposalPolicy::from_options(opts);

    let proposals: Vec<RegionProposal> = cands
        .iter()
        .enumerate()
        .map(|(i, t)| table_to_proposal(t, i, page_area, &policy))
        .collect();
    // Contour seeds are computed for diagnostics only. They must not enter
    // partition as hard owners: they have no detector table, and when full-page
    // render is opportunistic they flakily blocked legitimate stream/network
    // tables (e.g. borderless prose-gap fixtures).
    let contour_seeds = if opts.raster_line_detect && !raster_pages.is_empty() {
        contour_seed_proposals(raster_pages, rules, opts, page_area)
    } else {
        Vec::new()
    };
    let merged = vertical_merge(
        proposals,
        ROUTER_MEDIAN_LINE_GAP,
        DEFAULT_X_IOU_MIN,
        &policy,
    );
    let accepted = partition(merged, &policy);

    // Identity-based emit: each accepted proposal contributes at most one table
    // from its source_indices (best quality). K26 merges collapse to one emit.
    let mut kept = emit_tables_from_accepted(&cands, &accepted);
    kept = engine_v2_exclusive_cleanup(kept, opts, &policy);

    for t in &mut kept {
        if !t.notes.iter().any(|n| n == "engine_v2_router") {
            t.notes.push("engine_v2_router".into());
        }
        if contour_seeds
            .iter()
            .any(|p| geom::iou(t.bbox, p.bbox) >= 0.35)
            && !t.notes.iter().any(|n| n == "contour_seed_match")
        {
            t.notes.push("contour_seed_match".into());
        }
    }
    sort_tables_by_emit_order(&mut kept);
    kept.truncate(opts.max_tables_per_page as usize);
    kept
}

fn page_area(page_size: Option<(f32, f32)>) -> f32 {
    match page_size {
        Some((w, h)) if w > 1.0 && h > 1.0 => (w * h).max(1.0),
        _ => PAGE_AREA_EST,
    }
}

/// Pick tables for accepted proposals by source index (not loose bbox match).
fn emit_tables_from_accepted(cands: &[Table], accepted: &[RegionProposal]) -> Vec<Table> {
    let mut used: HashSet<usize> = HashSet::new();
    let mut kept: Vec<Table> = Vec::new();
    for p in accepted {
        if p.source_indices.is_empty() {
            continue; // contour seed without detector table
        }
        let best = p
            .source_indices
            .iter()
            .copied()
            .filter(|&i| i < cands.len() && !used.contains(&i))
            .max_by(|&i, &j| {
                quality_score(&cands[i])
                    .partial_cmp(&quality_score(&cands[j]))
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then_with(|| method_rank(cands[i].method).cmp(&method_rank(cands[j].method)))
            });
        if let Some(i) = best {
            used.insert(i);
            kept.push(cands[i].clone());
        }
    }
    kept
}

/// Post-partition method exclusivity for Engine V2.
///
/// Keeps nested ruled parent/child; drops stream FPs under ruled tables and
/// weak 2-col prose streams when any lattice/hybrid is present.
fn engine_v2_exclusive_cleanup(
    mut kept: Vec<Table>,
    opts: &TableOptions,
    nest: &ProposalPolicy,
) -> Vec<Table> {
    if kept.len() < 2 {
        return kept;
    }

    // Prefer lattice over overlapping hybrid (same as pre-router demotion).
    kept = prefer_lattice_over_overlapping_hybrid(kept);
    // Fuse vertically stacked same-col lattices (fragmented multi-region CC).
    // Geometric only: same page, same col count, high X-overlap, modest Y-gap.
    kept = merge_stacked_same_col_lattices(kept);

    // Only solid *lattice* owns the page for stream exclusivity.
    // Hybrid partial frames must NOT kill stream (campaign donors: hybrid
    // over-wide densify + stream 56×7).
    let lattice_bboxes: Vec<pdfparser_ir::Rect> = kept
        .iter()
        .filter(|t| t.method == TableMethod::Lattice && is_solid_ruled_table(t, opts))
        .map(|t| t.bbox)
        .collect();
    let has_solid_lattice = !lattice_bboxes.is_empty();

    if has_solid_lattice {
        // Keep multi-table recovery streams; drop other borderless under lattice.
        kept.retain(|t| {
            if !matches!(t.method, TableMethod::Stream | TableMethod::DenseNumeric) {
                return true;
            }
            is_multitable_stream_recovery(t, &lattice_bboxes)
                || t.multitable_stream_recovery
                || t.stream_vs_overwide_hybrid
        });
    } else {
        let recall = !kept
            .iter()
            .any(|t| matches!(t.method, TableMethod::Lattice) && is_solid_ruled_table(t, opts));
        kept.retain(|t| {
            if matches!(t.method, TableMethod::Stream | TableMethod::DenseNumeric) {
                return borderless_passes_precision(t, opts, recall) || t.stream_vs_overwide_hybrid;
            }
            true
        });
        // ICDAR multipage financial pages: borderless_recall can emit many
        // mid-size stream fragments per page. Typically
        // one dominant table. Drop small/stream fragments that are clearly subordinate
        // to a dominant stream on the same page (area < 45% of max).
        kept = prune_subordinate_stream_fragments(kept);
    }

    // High-IoU different-method pairs: keep higher quality_score (not nested).
    let mut out: Vec<Table> = Vec::new();
    let mut order: Vec<Table> = kept;
    order.sort_by(|a, b| {
        quality_score(b)
            .partial_cmp(&quality_score(a))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    for c in order {
        let clash = out.iter().any(|k| {
            if is_nested_table_pair(c.bbox, k.bbox, nest) {
                return false;
            }
            let iou = geom::iou(c.bbox, k.bbox);
            let ov = region_overlap(c.bbox, k.bbox);
            iou >= 0.35 || ov >= 0.50
        });
        if !clash {
            out.push(c);
        }
    }
    out
}

/// Build ruled proposals from raster contour seeds (region hints only).
fn contour_seed_proposals(
    raster_pages: &[RasterPage],
    rules: &[RuleSegment],
    opts: &TableOptions,
    page_area: f32,
) -> Vec<RegionProposal> {
    use crate::raster::{config_for_raster_page, contour_seeds_from_page};
    let mut out = Vec::new();
    let page_area = page_area.max(1.0);
    for rp in raster_pages {
        if rp.width < 40 || rp.height < 40 {
            continue;
        }
        let cfg = config_for_raster_page(
            rp,
            opts.raster_adaptive_radius,
            opts.raster_adaptive_bias,
            opts.raster_min_kernel,
            opts.raster_min_seg_px,
            opts.raster_merge_gap_px,
            opts.raster_pos_snap_px,
        );
        let seeds = contour_seeds_from_page(rp, rules, &cfg, true, 5e-4);
        for s in seeds {
            let (x0, y0, x1, y1) = s.to_page_bbox(rp);
            let bbox = Rect { x0, y0, x1, y1 };
            let area = (bbox.width() * bbox.height()).max(0.0);
            let area_frac = (area / page_area).clamp(0.0, 1.0);
            out.push(RegionProposal {
                kind: RegionKind::RuledContour,
                bbox,
                line_score: 0.70,
                text_score: 0.0,
                // Seeds are area-gated; joint_count is unknown (0). Gate uses
                // min_joints only when joints are known — pass joint floor so
                // seeds can own regions as hard owners when line_score allows.
                joint_count: opts.lattice_min_joints.max(4),
                area_frac,
                whitespace_est: 0.0,
                origin: ProposalOrigin::ContourSeed,
                source_indices: Vec::new(),
            });
        }
    }
    out
}

/// Map a detector table to a router proposal with real structure signals.
fn table_to_proposal(
    t: &Table,
    source_idx: usize,
    page_area: f32,
    policy: &ProposalPolicy,
) -> RegionProposal {
    let kind = match t.method {
        TableMethod::Lattice => RegionKind::RuledContour,
        TableMethod::Hybrid => RegionKind::PartialRuled,
        TableMethod::Stream | TableMethod::DenseNumeric => RegionKind::BorderlessText,
        _ => RegionKind::BorderlessText,
    };

    let line_score = match t.method {
        TableMethod::Lattice | TableMethod::Hybrid => {
            let base = if t.edge_score > 0.0 {
                t.edge_score.max(t.confidence)
            } else {
                t.confidence
            };
            base.clamp(0.0, 1.0)
        }
        _ => 0.0,
    };
    let text_score = match t.method {
        TableMethod::Stream | TableMethod::DenseNumeric => t.confidence.clamp(0.0, 1.0),
        TableMethod::Hybrid => {
            let fr = if t.fill_rate > 0.0 {
                t.fill_rate
            } else {
                t.confidence * 0.5
            };
            fr.clamp(0.0, 1.0)
        }
        TableMethod::Lattice => {
            if t.fill_rate > 0.0 {
                t.fill_rate.clamp(0.0, 1.0)
            } else {
                (t.confidence * 0.5).clamp(0.0, 1.0)
            }
        }
        _ => t.confidence.clamp(0.0, 1.0),
    };

    // Prefer real joint_count from lattice; fall back only when unknown so
    // gates remain meaningful (never invent rows×cols as "joints").
    let joint_count = match t.method {
        TableMethod::Lattice => {
            if t.joint_count > 0 {
                t.joint_count
            } else {
                // Unknown joints: use policy min so filled lattices still pass
                // when edge_score/confidence already survived detector gates.
                policy.min_joints_ruled
            }
        }
        TableMethod::Hybrid => {
            if t.joint_count > 0 {
                t.joint_count
            } else {
                t.rows.saturating_add(t.cols).max(2)
            }
        }
        _ => 0,
    };

    let area = (t.bbox.width() * t.bbox.height()).max(0.0);
    let area_frac = (area / page_area.max(1.0)).clamp(0.0, 1.0);

    // When fill is known, empty-cell frac; when unknown leave 0 (do not invent
    // chrome). Cap just below whitespace_reject so filled tables are not rejected
    // solely by empty_frac ≈ reject threshold noise.
    let whitespace_est = if t.fill_rate > 0.0 {
        (1.0 - t.fill_rate).clamp(0.0, (policy.whitespace_reject - 0.01).max(0.0))
    } else {
        0.0
    };

    RegionProposal {
        kind,
        bbox: t.bbox,
        line_score,
        text_score,
        joint_count,
        area_frac,
        whitespace_est,
        origin: ProposalOrigin::Detector,
        source_indices: vec![source_idx],
    }
}

fn is_strong_lattice(t: &Table, opts: &TableOptions) -> bool {
    if t.method != TableMethod::Lattice
        || t.rows < 2
        || t.confidence < opts.strong_lattice_min_conf
        || t.weak_edges
    {
        return false;
    }
    if t.cols >= 3 {
        return true;
    }
    // 2-col lattices: strong only if wide enough to be a real side-by-side
    // table, not a partial corner fragment (disease table left strip ~100u).
    // Side-by-side stress fixture tables are ~150–170u wide.
    t.cols == 2 && t.bbox.width() >= 140.0 && t.rows >= 4
}

/// Solid ruled table for page ownership (slightly looser than strong_lattice).
///
/// Phase-1 V3: if any such table exists, borderless detectors stay off.
fn is_solid_ruled_table(t: &Table, opts: &TableOptions) -> bool {
    if t.method != TableMethod::Lattice || t.weak_edges {
        return false;
    }
    if t.rows < 2 || t.cols < 2 {
        return false;
    }
    if t.confidence < opts.min_table_confidence.min(0.55) {
        return false;
    }
    // Reject near-empty chrome frames (page borders / form rules).
    let empty_frac = lattice_empty_frac(t);
    if empty_frac >= 0.90 {
        return false;
    }
    // Tall few-col lattices grown mostly by text_row densify (NIPA page-rule soup
    // → 47×3) must NOT own the page and kill multi-col stream recovery (gold ~50×22).
    // Thresholds from tuning so document-type overrides stay consistent.
    let heavy_y_densify = t.text_row_recovery
        || t.notes
            .iter()
            .any(|n| n.starts_with("text_row_recovery") || n.contains("synthetic_h="));
    if t.cols <= opts.tuning.solid_lattice_stream_safe_max_cols
        && t.rows >= opts.tuning.solid_lattice_stream_safe_min_rows
        && heavy_y_densify
    {
        return false;
    }
    t.cols >= 2 && t.rows >= 2
}

fn lattice_empty_frac(t: &Table) -> f32 {
    let n = t.rows.saturating_mul(t.cols).max(1) as f32;
    if t.cells.is_empty() {
        return 0.5;
    }
    let empty = t.cells.iter().filter(|c| c.text.trim().is_empty()).count() as f32;
    // Prefer schema size when cell list matches grid; else use list length.
    let denom = if t.cells.len() as u32 >= t.rows.saturating_mul(t.cols) {
        n
    } else {
        t.cells.len().max(1) as f32
    };
    empty / denom
}

/// Borderless/stream gates.
///
/// Merge vertically stacked Lattice tables with equal column counts when the
/// gap is modest and X-overlap is high. Targets multi-region CC shred that
/// splits one ruled grid into two same-width fragments (not doc-specific).
fn merge_stacked_same_col_lattices(mut tabs: Vec<Table>) -> Vec<Table> {
    if tabs.len() <= 1 {
        return tabs;
    }
    tabs.sort_by(|a, b| {
        a.page.cmp(&b.page).then_with(|| {
            b.bbox
                .y1
                .partial_cmp(&a.bbox.y1)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
    });
    let mut out: Vec<Table> = Vec::new();
    for t in tabs {
        if out.is_empty() {
            out.push(t);
            continue;
        }
        let prev = out.last().unwrap();
        let both_lat = prev.method == TableMethod::Lattice && t.method == TableMethod::Lattice;
        let same_page = prev.page == t.page;
        let same_cols = prev.cols == t.cols && prev.cols >= 2;
        let gap = prev.bbox.y0 - t.bbox.y1;
        let x0 = prev.bbox.x0.max(t.bbox.x0);
        let x1 = prev.bbox.x1.min(t.bbox.x1);
        let ov = (x1 - x0).max(0.0) / prev.bbox.width().min(t.bbox.width()).max(1.0);
        let w_ratio = {
            let pw = prev.bbox.width().max(1.0);
            let tw = t.bbox.width().max(1.0);
            (pw / tw).max(tw / pw)
        };
        // Modest gap: up to ~2 median row heights estimated from prev height/rows.
        let row_h = (prev.bbox.height() / prev.rows.max(1) as f32).max(4.0);
        let max_gap = row_h * 2.5;
        if both_lat
            && same_page
            && same_cols
            && gap >= -2.0
            && gap <= max_gap
            && ov >= 0.70
            && w_ratio <= 1.25
            && prev.rows + t.rows <= 100
        {
            let mut merged = prev.clone();
            // Skip duplicate header if first row text matches.
            let skip_header = t.rows >= 1
                && prev.rows >= 1
                && (0..prev.cols as usize).all(|c| {
                    let a = prev
                        .cells
                        .iter()
                        .find(|cell| cell.row == 0 && cell.col == c as u32)
                        .map(|cell| cell.text.trim())
                        .unwrap_or("");
                    let b = t
                        .cells
                        .iter()
                        .find(|cell| cell.row == 0 && cell.col == c as u32)
                        .map(|cell| cell.text.trim())
                        .unwrap_or("");
                    !a.is_empty() && a.eq_ignore_ascii_case(b)
                });
            let start = if skip_header { 1u32 } else { 0u32 };
            let off = prev.rows;
            for cell in &t.cells {
                if cell.row < start {
                    continue;
                }
                let mut nc = cell.clone();
                nc.row = cell.row - start + off;
                merged.cells.push(nc);
            }
            let added = t.rows.saturating_sub(start);
            merged.rows = off + added;
            merged.bbox.x0 = prev.bbox.x0.min(t.bbox.x0);
            merged.bbox.y0 = t.bbox.y0.min(prev.bbox.y0);
            merged.bbox.x1 = prev.bbox.x1.max(t.bbox.x1);
            merged.bbox.y1 = prev.bbox.y1.max(t.bbox.y1);
            merged.confidence = prev.confidence.max(t.confidence) * 0.98;
            merged.text_row_recovery = prev.text_row_recovery || t.text_row_recovery;
            merged.text_col_recovery = prev.text_col_recovery || t.text_col_recovery;
            merged
                .notes
                .push(format!("lattice_stack_merge +{added}rows"));
            *out.last_mut().unwrap() = merged;
        } else {
            out.push(t);
        }
    }
    out
}

/// When a page has several borderless tables and no solid lattice, keep only
/// streams that are competitive in area with the largest one (ICDAR shred).
/// Always keeps ≥1 stream if present; never drops lattice/hybrid.
fn prune_subordinate_stream_fragments(kept: Vec<Table>) -> Vec<Table> {
    use std::collections::HashMap;
    let mut by_page: HashMap<u32, Vec<usize>> = HashMap::new();
    for (i, t) in kept.iter().enumerate() {
        if matches!(t.method, TableMethod::Stream | TableMethod::DenseNumeric) {
            by_page.entry(t.page).or_default().push(i);
        }
    }
    let mut drop: std::collections::HashSet<usize> = std::collections::HashSet::new();
    for (_page, idxs) in by_page {
        if idxs.len() < 3 {
            // 1–2 streams: leave alone (side-by-side multi-table legitimate).
            continue;
        }
        let max_area = idxs
            .iter()
            .map(|&i| kept[i].bbox.width().max(1.0) * kept[i].bbox.height().max(1.0))
            .fold(0.0f32, f32::max);
        if max_area <= 1.0 {
            continue;
        }
        // Keep streams with area ≥ 45% of max OR rows ≥ 0.7 * max_rows.
        let max_rows = idxs.iter().map(|&i| kept[i].rows).max().unwrap_or(0);
        for &i in &idxs {
            let a = kept[i].bbox.width().max(1.0) * kept[i].bbox.height().max(1.0);
            let tall = max_rows > 0 && kept[i].rows * 10 >= max_rows * 7;
            if a < max_area * 0.45 && !tall {
                drop.insert(i);
            }
        }
        // Safety: never drop all streams on the page.
        let remain = idxs.iter().filter(|i| !drop.contains(i)).count();
        if remain == 0 {
            // Keep the largest only.
            if let Some(&best) = idxs.iter().max_by(|&&a, &&b| {
                let aa = kept[a].bbox.width() * kept[a].bbox.height();
                let bb = kept[b].bbox.width() * kept[b].bbox.height();
                aa.partial_cmp(&bb).unwrap_or(std::cmp::Ordering::Equal)
            }) {
                drop.remove(&best);
            }
        }
    }
    if drop.is_empty() {
        return kept;
    }
    kept.into_iter()
        .enumerate()
        .filter_map(|(i, mut t)| {
            if drop.contains(&i) {
                None
            } else {
                if matches!(t.method, TableMethod::Stream | TableMethod::DenseNumeric) {
                    t.notes.push("stream_subordinate_prune".into());
                }
                Some(t)
            }
        })
        .collect()
}

/// `recall_mode` (Phase-2): page has no solid ruled tables — allow stronger
/// multi-col structures that fail harsh prose cuts, while still rejecting
/// notices/forms/tiny fragments.
fn borderless_passes_precision(t: &Table, opts: &TableOptions, recall_mode: bool) -> bool {
    if t.rows < 3 || t.cols < 2 {
        return false;
    }
    // Camelot-class: need enough cells to be a real table (≥6 filled-ish).
    let filled = t.cells.iter().filter(|c| !c.text.trim().is_empty()).count();
    if filled < 6 && t.rows.saturating_mul(t.cols) < 12 {
        return false;
    }
    let mean = stream_mean_cell_chars(t);
    let num = stream_numeric_density(t);
    let structure_ok = t.cols >= 3 && t.rows >= 4 && filled >= 8;

    // Notice / form metadata grids (NIST withdrawn style) — always reject.
    if looks_like_notice_metadata(t) {
        return false;
    }
    // Tax form field snippets — always reject.
    if looks_like_tax_form_fields(t) {
        return false;
    }

    if recall_mode {
        let dense_numeric = num >= 0.30 && t.cols >= 3 && t.rows >= 4;
        // High-fill multi-col grids (campaign donors 56×7, liabilities ~30×10)
        // are real borderless data tables — not form worksheets.
        let dense_data_grid = t.cols >= 4
            && t.rows >= 6
            && filled >= 24
            && (t.fill_rate >= 0.55
                || filled as f32 / (t.rows.saturating_mul(t.cols).max(1) as f32) >= 0.55)
            && mean < 48.0;
        // Soft conf floor for structured / dense-numeric grids.
        let conf_floor = if dense_numeric || dense_data_grid {
            0.48
        } else if structure_ok {
            (opts.min_confidence_stream * 0.85).min(0.55)
        } else {
            opts.min_confidence_stream
        };
        if t.confidence < conf_floor {
            return false;
        }
        // Cap giant *form-like* networks (IRS worksheets): large + sparse/low-fill
        // OR giant with weak numeric and long label cells. Dense high-fill grids pass.
        if t.rows >= 20 && t.cols >= 6 && !dense_data_grid && !dense_numeric {
            return false;
        }
        if t.rows.saturating_mul(t.cols) >= 200 && !dense_data_grid && !dense_numeric {
            // Still allow medium-fill multi-col with strong conf (network class).
            if !(t.cols >= 5 && t.fill_rate >= 0.45 && t.confidence >= 0.75 && mean < 40.0) {
                return false;
            }
        }
        // IRS header strip (OMB / Department of the Treasury)
        if looks_like_irs_header_strip(t) {
            return false;
        }
        // Prose reject: still drop paragraph bands, but allow medium mean when
        // multi-col + some numeric OR large grid.
        if t.cols <= 2 && mean >= opts.stream_max_prose_mean_chars * 0.50 && num < 0.20 {
            return false;
        }
        if mean >= opts.stream_max_prose_mean_chars * 0.90
            && num < 0.12
            && !structure_ok
            && !dense_numeric
            && !dense_data_grid
        {
            return false;
        }
        // Weak 2-col alpha lists
        if t.cols == 2 && num < 0.12 && mean > 14.0 {
            return false;
        }
        return true;
    }

    // Precision mode (default / under ruled co-existence checks)
    if mean >= opts.stream_max_prose_mean_chars * 0.55 && num < 0.25 {
        return false;
    }
    if mean >= 28.0 && num < 0.15 {
        return false;
    }
    if t.confidence < opts.min_confidence_stream {
        return false;
    }
    if t.cols == 2 && num < 0.12 && mean > 12.0 {
        return false;
    }
    true
}

fn looks_like_tax_form_fields(t: &Table) -> bool {
    let blob = t
        .cells
        .iter()
        .map(|c| c.text.to_ascii_lowercase())
        .collect::<Vec<_>>()
        .join(" ");
    let tax_hits = (blob.contains("social security") as u32)
        + (blob.contains("employer id") as u32)
        + (blob.contains("enter code from instructions") as u32)
        + (blob.contains("(ssn)") as u32)
        + (blob.contains("(ein)") as u32)
        + (blob.contains("accounting method") as u32)
        + (blob.contains("business address") as u32)
        + (blob.contains("principal business or profession") as u32)
        + (blob.contains("profit or loss from business") as u32)
        + (blob.contains("schedule c") as u32)
        + (blob.contains("schedule d") as u32)
        + (blob.contains("omb no.") as u32)
        + (blob.contains("form 1099") as u32)
        + (blob.contains("short-term transactions") as u32)
        + (blob.contains("long-term transactions") as u32)
        + (blob.contains("proceeds (sales price)") as u32)
        + (blob.contains("cost (or other basis)") as u32)
        + (blob.contains("totals for all short-term") as u32)
        + (blob.contains("totals for all long-term") as u32);
    let num = stream_numeric_density(t);
    (tax_hits >= 1 && num < 0.45) || (tax_hits >= 2 && num < 0.55)
}

fn looks_like_irs_header_strip(t: &Table) -> bool {
    let blob = t
        .cells
        .iter()
        .take(12)
        .map(|c| c.text.to_ascii_lowercase())
        .collect::<Vec<_>>()
        .join(" ");
    (blob.contains("department of the treasury")
        || blob.contains("omb no.")
        || blob.contains("irs use only")
        || blob.contains("internal revenue"))
        && t.rows <= 8
}

/// Phase-2: strong multi-col numeric stream that coexists with lattice (census).
fn is_multitable_stream_recovery(s: &Table, lattices: &[pdfparser_ir::Rect]) -> bool {
    if s.cols < 4 || s.rows < 6 {
        return false;
    }
    if stream_numeric_density(s) < 0.28 {
        return false;
    }
    if s.confidence < 0.65 {
        return false;
    }
    // Require meaningful size (avoid thin FP bands next to lattices).
    if s.bbox.width() < 120.0 || s.bbox.height() < 80.0 {
        return false;
    }
    // Must not substantially overlap any lattice.
    for &lat in lattices {
        if geom::iou(s.bbox, lat) >= 0.18 {
            return false;
        }
        if containment_ratio(s.bbox, lat) >= 0.30 {
            return false;
        }
        if containment_ratio(lat, s.bbox) >= 0.40 {
            return false;
        }
        // Same vertical band / stacked: require clear y-separation or x-separation
        let y_overlap = (s.bbox.y1.min(lat.y1) - s.bbox.y0.max(lat.y0)).max(0.0);
        let x_overlap = (s.bbox.x1.min(lat.x1) - s.bbox.x0.max(lat.x0)).max(0.0);
        if y_overlap > 0.5 * s.bbox.height().min(lat.height())
            && x_overlap > 0.5 * s.bbox.width().min(lat.width())
        {
            return false;
        }
    }
    true
}

fn looks_like_notice_metadata(t: &Table) -> bool {
    let mut hits = 0u32;
    for c in t.cells.iter().take(24) {
        let s = c.text.to_ascii_lowercase();
        if s.contains("name of standard")
            || s.contains("withdrawn")
            || s.contains("warning notice")
            || s.contains("series/number")
            || s.contains("fips")
            || s.starts_with("1.")
            || s.starts_with("2.")
        {
            hits += 1;
        }
    }
    hits >= 2 && stream_numeric_density(t) < 0.25
}

fn stream_mean_cell_chars(t: &Table) -> f32 {
    let cells: Vec<_> = t
        .cells
        .iter()
        .filter(|c| !c.text.trim().is_empty())
        .collect();
    if cells.is_empty() {
        return 0.0;
    }
    let sum: usize = cells.iter().map(|c| c.text.chars().count()).sum();
    sum as f32 / cells.len() as f32
}

/// Page-border / form-box lattices that are not data tables (Phase-1).
///
/// Conservative: only drop near-empty chrome. Do **not** drop filled label
/// lattices (outer nested forms) or multi-col report tables.
fn is_chrome_lattice_fp(t: &Table) -> bool {
    if t.method != TableMethod::Lattice {
        return false;
    }
    let empty = lattice_empty_frac(t);
    let num = stream_numeric_density(t);
    let mean = stream_mean_cell_chars(t);
    let filled = t.cells.iter().filter(|c| !c.text.trim().is_empty()).count();
    // Near-empty ruled frame (page border / empty checkbox grid)
    if empty >= 0.88 && num < 0.12 && filled <= 6 {
        return true;
    }
    // Tiny empty-ish box
    if t.rows <= 3 && t.cols <= 3 && filled <= 2 && num < 0.15 {
        return true;
    }
    // 2-col prose notice (NIST withdrawn) — long alpha cells, no numbers
    if t.cols == 2 && mean >= 28.0 && num < 0.12 && t.rows >= 8 {
        return true;
    }
    // Sparse form worksheet: high empty + weak edges (IRS Schedule C style).
    // Line numbers inflate numeric_density — also check dotted leaders.
    if empty >= 0.55 && t.cols >= 6 && (t.weak_edges || t.edge_score < 0.55) {
        return true;
    }
    // Dotted leader forms (". . .") with weak lattice edges
    let dotted = t
        .cells
        .iter()
        .filter(|c| c.text.contains(". .") || c.text.matches('.').count() >= 3)
        .count();
    if dotted >= 4 && t.weak_edges && empty >= 0.40 {
        return true;
    }
    // Tax form field snippets (SSN / EIN / "enter code" / Schedule D capital gains)
    let blob = t
        .cells
        .iter()
        .map(|c| c.text.to_ascii_lowercase())
        .collect::<Vec<_>>()
        .join(" ");
    if (blob.contains("social security")
        || blob.contains("employer id")
        || blob.contains("enter code from instructions")
        || blob.contains("(ssn)")
        || blob.contains("(ein)")
        || blob.contains("proceeds (sales price)")
        || blob.contains("cost (or other basis)")
        || blob.contains("adjustments to gain or loss")
        || blob.contains("schedule d")
        || blob.contains("capital gain or (loss)"))
        && num < 0.40
    {
        return true;
    }
    false
}

/// Exclusive lattice should block stream only when the stream is essentially
/// inside the lattice (or not much larger). A full-width borderless table that
/// merely overlaps a tiny ruled corner must be kept (Phase 14 multi-table).
fn should_suppress_stream_under_lattices(
    stream_bbox: pdfparser_ir::Rect,
    lattices: &[pdfparser_ir::Rect],
) -> bool {
    let s_area = (stream_bbox.width() * stream_bbox.height()).max(1.0);
    for &lat in lattices {
        let overlap = region_overlap(stream_bbox, lat);
        let iou = geom::iou(stream_bbox, lat);
        if overlap < 0.40 && iou < 0.35 {
            continue;
        }
        let l_area = (lat.width() * lat.height()).max(1.0);
        // Stream mostly contained in lattice → suppress.
        if containment_ratio(stream_bbox, lat) >= 0.55 {
            return true;
        }
        // Stream substantially larger than lattice corner → keep stream.
        if s_area > l_area * 2.0 {
            continue;
        }
        // Comparable size + overlap → suppress (classic exclusive).
        return true;
    }
    false
}

fn prefer_lattice_over_overlapping_hybrid(mut cands: Vec<Table>) -> Vec<Table> {
    let lattices: Vec<pdfparser_ir::Rect> = cands
        .iter()
        .filter(|t| t.method == TableMethod::Lattice)
        .map(|t| t.bbox)
        .collect();
    if lattices.is_empty() {
        return cands;
    }
    cands.retain(|t| {
        if t.method != TableMethod::Hybrid {
            return true;
        }
        // Drop hybrid if it largely overlaps any lattice (lattice is preferred for ruled).
        !lattices.iter().any(|&lb| {
            containment_ratio(t.bbox, lb) >= 0.50
                || containment_ratio(lb, t.bbox) >= 0.50
                || geom::iou(t.bbox, lb) >= 0.40
        })
    });
    cands
}

/// Drop sparse over-wide hybrid (or weak lattice) when a high-fill multi-col
/// stream/network already covers the same region.
///
/// Hybrid line-sensing on borderless Quartz/Tabula PDFs invents many empty
/// gutter columns (e.g. 56×27) while network recovers the true schema (56×7).
/// Method-rank NMS would otherwise keep Hybrid over Stream.
fn prefer_stream_over_sparse_hybrid(mut cands: Vec<Table>) -> Vec<Table> {
    if cands.len() < 2 {
        return cands;
    }
    let strong_streams: Vec<(pdfparser_ir::Rect, u32, f32, f32)> = cands
        .iter()
        .filter(|t| {
            matches!(t.method, TableMethod::Stream | TableMethod::DenseNumeric)
                && t.cols >= 3
                && t.rows >= 4
                && t.confidence >= 0.65
                && t.fill_rate >= 0.55
        })
        .map(|t| (t.bbox, t.cols, t.confidence, t.fill_rate))
        .collect();
    if strong_streams.is_empty() {
        return cands;
    }
    cands.retain(|t| {
        if !matches!(t.method, TableMethod::Hybrid | TableMethod::Lattice) {
            return true;
        }
        !strong_streams.iter().any(|&(sb, sc, sconf, sfill)| {
            let overlap = region_overlap(t.bbox, sb) >= 0.40 || geom::iou(t.bbox, sb) >= 0.30;
            if !overlap {
                return false;
            }
            let over_wide = (t.cols as f32) >= (sc as f32) * 1.5 + 1.0;
            let sparse = t.fill_rate > 0.0 && t.fill_rate + 0.12 < sfill;
            let weaker = t.confidence + 0.05 < sconf;
            // Drop only when stream is clearly the better schema for the region.
            (over_wide || sparse) && (weaker || sfill >= 0.70)
        })
    });
    cands
}

/// Drop or demote lattice tables that look like vertical column-group slices
/// when a wider multi-column stream/network table **overlaps the same region**.
///
/// Motivating case: census Table 324 upper stream + overlapping 2-col lattice
/// strip (over-detect). Prefer the wider multi-col table.
///
/// Phase 15: do **not** drop a vertically disjoint 2-col lattice (e.g. Table 325
/// lower on the page) just because an upper wide stream exists.
fn demote_lattice_column_slices(mut cands: Vec<Table>) -> Vec<Table> {
    if cands.len() < 2 {
        return cands;
    }
    let wide_streams: Vec<pdfparser_ir::Rect> = cands
        .iter()
        .filter(|t| {
            matches!(t.method, TableMethod::Stream | TableMethod::DenseNumeric)
                && t.cols >= 4
                && t.rows >= 4
                && t.confidence >= 0.55
        })
        .map(|t| t.bbox)
        .collect();
    if wide_streams.is_empty() {
        // Still demote tiny corners vs large multi-col lattices on-page.
        let has_large = cands.iter().any(|t| t.cols >= 4 && t.rows >= 3);
        if has_large {
            for t in &mut cands {
                if t.method == TableMethod::Lattice && t.cols <= 2 && t.rows <= 4 {
                    t.confidence *= 0.45;
                    t.notes.push("demoted_tiny_lattice_corner".into());
                }
            }
        }
        return cands;
    }

    cands.retain(|t| {
        if t.method != TableMethod::Lattice {
            return true;
        }
        let skinny = t.cols <= 2 && t.rows >= 8;
        if !skinny {
            return true;
        }
        // Only drop if this skinny lattice overlaps a wide stream (same region).
        let overlaps_wide = wide_streams
            .iter()
            .any(|&wb| region_overlap(t.bbox, wb) >= 0.25 || geom::iou(t.bbox, wb) >= 0.20);
        if overlaps_wide {
            return false;
        }
        true
    });
    // Soft demote remaining overlapping 2-col lattices.
    for t in &mut cands {
        if t.method == TableMethod::Lattice && t.cols <= 2 && t.rows >= 8 {
            let overlaps_wide = wide_streams
                .iter()
                .any(|&wb| region_overlap(t.bbox, wb) >= 0.25 || geom::iou(t.bbox, wb) >= 0.20);
            if overlaps_wide {
                t.confidence *= 0.55;
                t.notes.push("demoted_lattice_column_slice".into());
            }
        }
    }
    // Only demote tiny lattice corners against a much larger multi-col **stream**
    // (not hybrid — hybrid often re-detects the same ruled region at 3–4 cols and
    // must not erase a valid 3×2 lattice; sensing 95).
    let large_streams: Vec<(pdfparser_ir::Rect, f32)> = cands
        .iter()
        .filter(|t| {
            matches!(t.method, TableMethod::Stream | TableMethod::DenseNumeric)
                && t.cols >= 4
                && t.rows >= 3
        })
        .map(|t| (t.bbox, (t.bbox.width() * t.bbox.height()).max(1.0)))
        .collect();
    if !large_streams.is_empty() {
        for t in &mut cands {
            if t.method == TableMethod::Lattice && t.cols <= 2 && t.rows <= 4 {
                let t_area = (t.bbox.width() * t.bbox.height()).max(1.0);
                let overlaps_large = large_streams.iter().any(|&(lb, la)| {
                    la >= t_area * 2.0
                        && (region_overlap(t.bbox, lb) >= 0.25 || geom::iou(t.bbox, lb) >= 0.20)
                });
                if overlaps_large {
                    t.confidence *= 0.45;
                    t.notes.push("demoted_tiny_lattice_corner".into());
                }
            }
        }
    }
    cands
}

fn overlaps_any(bbox: pdfparser_ir::Rect, regions: &[pdfparser_ir::Rect]) -> bool {
    regions
        .iter()
        .any(|&kb| region_overlap(kb, bbox) >= 0.40 || geom::iou(kb, bbox) >= 0.35)
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
    if opts.form_discriminator {
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
    if opts.form_discriminator {
        logical = scrub_document_table_fps(logical, opts);
    }
    (page_tables, logical)
}

fn nms(mut cands: Vec<Table>, min_conf: f32, containment_frac: f32) -> Vec<Table> {
    // Align with final retain: do not admit candidates below product min conf.
    cands.retain(|t| t.confidence >= min_conf);
    cands.sort_by(|a, b| {
        method_rank(b.method)
            .cmp(&method_rank(a.method))
            .then_with(|| {
                quality_score(b)
                    .partial_cmp(&quality_score(a))
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
    });
    let nest_policy = ProposalPolicy::default();
    let mut out: Vec<Table> = Vec::new();
    for c in cands {
        // Drop if contained in a kept table — unless nested (inner rate grid).
        if out.iter().any(|k| {
            containment_ratio(c.bbox, k.bbox) >= containment_frac
                && !is_nested_table_pair(c.bbox, k.bbox, &nest_policy)
        }) {
            continue;
        }
        let c_rank = method_rank(c.method);
        out.retain(|k| {
            if method_rank(k.method) > c_rank {
                return true;
            }
            let contained = containment_ratio(k.bbox, c.bbox) >= containment_frac;
            if !contained {
                return true;
            }
            // Keep nested child when adding outer parent.
            is_nested_table_pair(k.bbox, c.bbox, &nest_policy)
        });
        let overlaps = out.iter().any(|k| {
            if is_nested_table_pair(k.bbox, c.bbox, &nest_policy) {
                return false;
            }
            let ov = region_overlap(k.bbox, c.bbox);
            ov >= 0.28 || geom::iou(k.bbox, c.bbox) >= 0.35
        });
        if !overlaps {
            out.push(c);
        }
    }
    out
}

fn containment_ratio(inner: pdfparser_ir::Rect, outer: pdfparser_ir::Rect) -> f32 {
    let x0 = inner.x0.max(outer.x0);
    let y0 = inner.y0.max(outer.y0);
    let x1 = inner.x1.min(outer.x1);
    let y1 = inner.y1.min(outer.y1);
    let w = (x1 - x0).max(0.0);
    let h = (y1 - y0).max(0.0);
    let inter = w * h;
    let area = (inner.width() * inner.height()).max(1.0);
    inter / area
}

fn quality_score(t: &Table) -> f32 {
    let edge = if t.edge_score > 0.0 {
        t.edge_score
    } else {
        0.5
    };
    let fill = if t.fill_rate > 0.0 { t.fill_rate } else { 0.5 };
    let weak_pen = if t.weak_edges { 0.85 } else { 1.0 };
    (0.55 * t.confidence + 0.25 * fill + 0.20 * edge) * weak_pen
}

fn region_overlap(a: pdfparser_ir::Rect, b: pdfparser_ir::Rect) -> f32 {
    let x0 = a.x0.max(b.x0);
    let y0 = a.y0.max(b.y0);
    let x1 = a.x1.min(b.x1);
    let y1 = a.y1.min(b.y1);
    let w = (x1 - x0).max(0.0);
    let h = (y1 - y0).max(0.0);
    let inter = w * h;
    if inter <= 0.0 {
        return 0.0;
    }
    let aa = (a.width() * a.height()).max(1.0);
    let ba = (b.width() * b.height()).max(1.0);
    inter / aa.min(ba)
}

fn method_rank(m: TableMethod) -> u8 {
    match m {
        TableMethod::Structure => 5,
        TableMethod::Lattice => 4,
        TableMethod::Hybrid => 3,
        TableMethod::Stream => 1,
        TableMethod::DenseNumeric => 2,
        _ => 0,
    }
}

fn stream_numeric_density(t: &Table) -> f32 {
    let mut ne = 0u32;
    let mut num = 0u32;
    for c in &t.cells {
        let s = c.text.trim();
        if s.is_empty() {
            continue;
        }
        ne += 1;
        let t = s
            .trim_matches(|ch: char| ch == '$' || ch == '%' || ch == '(' || ch == ')' || ch == ',');
        if t.is_empty() {
            continue;
        }
        let digits = t.chars().filter(|ch| ch.is_ascii_digit()).count();
        let alpha = t.chars().filter(|ch| ch.is_alphabetic()).count();
        if digits >= 1 && digits >= alpha {
            num += 1;
        }
    }
    if ne == 0 {
        0.0
    } else {
        num as f32 / ne as f32
    }
}

#[cfg(test)]
mod phase12_slice_tests {
    use super::*;
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
    use super::is_strong_lattice;
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
