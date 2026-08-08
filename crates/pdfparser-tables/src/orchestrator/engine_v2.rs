//! Engine V2 finalize + legacy NMS.
use super::classify::{
    borderless_passes_precision, is_multitable_stream_recovery, is_solid_ruled_table,
};
use super::geom_util::{containment_ratio, method_rank, quality_score, region_overlap};
use super::prefer::prefer_lattice_over_overlapping_hybrid;
use crate::constants::{LETTER_PAGE_AREA, ROUTER_MEDIAN_LINE_GAP};
use crate::evidence::{ProposalOrigin, RegionKind, RegionProposal};
use crate::geom;
use crate::options::TableOptions;
use crate::policy::{is_nested_table_pair, ProposalPolicy};
use crate::raster::RasterPage;
use crate::router::{merge_then_partition, sort_tables_by_emit_order};
use crate::types::{Table, TableMethod};
use pdfparser_content::RuleSegment;
use pdfparser_ir::Rect;
use std::collections::HashSet;

pub(crate) const PAGE_AREA_EST: f32 = LETTER_PAGE_AREA;

/// Engine V2 finalize: proposals → `merge_then_partition` (K26 + exclusive
/// partition, **no sort**) → identity-based emit (walk = partition order) →
/// exclusive cleanup → K27 emit order.
pub(crate) fn finalize_engine_v2(
    mut cands: Vec<Table>,
    opts: &TableOptions,
    rules: &[RuleSegment],
    raster_pages: &[RasterPage],
    page_size: Option<(f32, f32)>,
) -> Vec<Table> {
    let min_conf = opts
        .advanced
        .min_confidence_stream
        .min(opts.min_table_confidence);
    cands.retain(|t| t.confidence >= min_conf);
    cands.retain(|t| match t.method {
        TableMethod::Stream => t.confidence >= opts.advanced.min_confidence_stream,
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
    let contour_seeds = if opts.advanced.raster_line_detect && !raster_pages.is_empty() {
        contour_seed_proposals(raster_pages, rules, opts, page_area)
    } else {
        Vec::new()
    };
    // H18: merge_then_partition, not route_proposals — sort would change emit
    // walk when source_indices overlap.
    let accepted = merge_then_partition(proposals, ROUTER_MEDIAN_LINE_GAP, &policy);

    // Identity-based emit: each accepted proposal contributes at most one table
    // from its source_indices (best quality). K26 merges collapse to one emit.
    let mut kept = emit_tables_from_accepted(&cands, &accepted);
    kept = engine_v2_exclusive_cleanup(kept, opts, &policy);

    // Telemetry only (A1.9): uniqueness is a loop bool, not a notes-string gate.
    // Each table is visited once; notes stay diagnostic writes.
    for t in &mut kept {
        let contour_seed_match = contour_seeds
            .iter()
            .any(|p| geom::iou(t.bbox, p.bbox) >= 0.35);
        if contour_seed_match {
            t.notes.push("contour_seed_match".into());
        }
    }
    sort_tables_by_emit_order(&mut kept);
    kept.truncate(opts.max_tables_per_page as usize);
    kept
}

pub(crate) fn page_area(page_size: Option<(f32, f32)>) -> f32 {
    match page_size {
        Some((w, h)) if w > 1.0 && h > 1.0 => (w * h).max(1.0),
        _ => PAGE_AREA_EST,
    }
}

/// Pick tables for accepted proposals by source index (not loose bbox match).
pub(crate) fn emit_tables_from_accepted(
    cands: &[Table],
    accepted: &[RegionProposal],
) -> Vec<Table> {
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
pub(crate) fn engine_v2_exclusive_cleanup(
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
pub(crate) fn contour_seed_proposals(
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
            opts.advanced.raster_adaptive_radius,
            opts.advanced.raster_adaptive_bias,
            opts.advanced.raster_min_kernel,
            opts.advanced.raster_min_seg_px,
            opts.advanced.raster_merge_gap_px,
            opts.advanced.raster_pos_snap_px,
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
                joint_count: opts.advanced.lattice_min_joints.max(4),
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
pub(crate) fn table_to_proposal(
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
pub(crate) fn merge_stacked_same_col_lattices(tabs: Vec<Table>) -> Vec<Table> {
    crate::stack_merge::merge_stacked_same_col(
        tabs,
        crate::stack_merge::methods_ok_lattice,
        |prev| {
            let row_h = (prev.bbox.height() / prev.rows.max(1) as f32).max(4.0);
            row_h * 2.5
        },
        crate::stack_merge::StackMergePolicy {
            min_cols: 2,
            max_total_rows: 100,
            min_x_overlap: 0.70,
            max_width_ratio: 1.25,
            gap_lo: -2.0,
            note_prefix: "lattice_stack_merge",
            copy_text_recovery: true,
        },
    )
}

/// When a page has several borderless tables and no solid lattice, keep only
/// streams that are competitive in area with the largest one (ICDAR shred).
/// Always keeps ≥1 stream if present; never drops lattice/hybrid.
pub(crate) fn prune_subordinate_stream_fragments(kept: Vec<Table>) -> Vec<Table> {
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
pub(crate) fn nms(mut cands: Vec<Table>, min_conf: f32, containment_frac: f32) -> Vec<Table> {
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
