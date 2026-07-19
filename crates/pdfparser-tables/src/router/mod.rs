//! Exclusive AutoRouter geometry (Engine V2) — pure bbox ops, no PDF I/O.
//!
//! Pipeline (design rev 2.3):
//! 1. [`vertical_merge`] (K26) — rejoin header/body splits
//! 2. [`partition`] — gates + priority Ruled > Partial > Borderless + ownership
//! 3. [`emit_order_key`] / [`sort_by_emit_order`] (K27) — (−y1, x0)
//!
//! Product Auto/Full call this via the page orchestrator (`use_engine_v2` +
//! `!legacy_router`).

use std::cmp::Ordering;

use pdfparser_ir::Rect;

use crate::evidence::{RegionKind, RegionProposal};
use crate::policy::{is_nested_table_pair, overlaps_owned, pad_rect, rect_iou, ProposalPolicy};
use crate::types::Table;

/// Default minimum x-axis IoU for K26 vertical merge.
pub const DEFAULT_X_IOU_MIN: f32 = 0.60;

/// y-gap multiplier vs median line gap for K26 merge.
pub const Y_GAP_MEDIAN_MULT: f32 = 1.5;

// ---------------------------------------------------------------------------
// Kind priority / scoring
// ---------------------------------------------------------------------------

/// Router priority: Ruled > Partial > Borderless > Residual.
#[inline]
pub fn kind_priority(kind: RegionKind) -> u8 {
    match kind {
        RegionKind::RuledContour => 3,
        RegionKind::PartialRuled => 2,
        RegionKind::BorderlessText => 1,
        RegionKind::Residual => 0,
    }
}

/// Structure prior used only for partition ordering.
#[inline]
pub fn structure_prior(p: &RegionProposal) -> f32 {
    0.6 * p.line_score + 0.4 * p.text_score
}

/// Kinds that may merge under K26: same kind, or Ruled ↔ Partial.
#[inline]
pub fn kinds_mergeable(a: RegionKind, b: RegionKind) -> bool {
    if a == b {
        return true;
    }
    matches!(
        (a, b),
        (RegionKind::RuledContour, RegionKind::PartialRuled)
            | (RegionKind::PartialRuled, RegionKind::RuledContour)
    )
}

// ---------------------------------------------------------------------------
// Geometry helpers (x-IoU / y-gap)
// ---------------------------------------------------------------------------

/// Horizontal (x-axis only) IoU of two rects.
pub fn x_iou(a: Rect, b: Rect) -> f32 {
    let x0 = a.x0.max(b.x0);
    let x1 = a.x1.min(b.x1);
    let inter = (x1 - x0).max(0.0);
    let ua = a.width() + b.width() - inter;
    if ua <= 0.0 {
        0.0
    } else {
        inter / ua
    }
}

/// Vertical gap between two rects when `above` is the topper (higher y1).
///
/// PDF y-up: gap = `above.y0 - below.y1`. Negative ⇒ vertical overlap.
pub fn y_gap(above: Rect, below: Rect) -> f32 {
    above.y0 - below.y1
}

/// Merge two proposals into a union bbox; keep higher-priority kind and max scores.
fn merge_pair(a: &RegionProposal, b: &RegionProposal) -> RegionProposal {
    let kind = if kind_priority(a.kind) >= kind_priority(b.kind) {
        a.kind
    } else {
        b.kind
    };
    let origin = if a.origin == b.origin {
        a.origin
    } else {
        // Prefer detector origin when merging seed + detector.
        use crate::evidence::ProposalOrigin;
        if matches!(a.origin, ProposalOrigin::Detector)
            || matches!(b.origin, ProposalOrigin::Detector)
        {
            ProposalOrigin::Detector
        } else {
            a.origin
        }
    };
    let mut source_indices = a.source_indices.clone();
    for &i in &b.source_indices {
        if !source_indices.contains(&i) {
            source_indices.push(i);
        }
    }
    RegionProposal {
        kind,
        bbox: a.bbox.union(b.bbox),
        line_score: a.line_score.max(b.line_score),
        text_score: a.text_score.max(b.text_score),
        joint_count: a.joint_count.max(b.joint_count),
        // Union is larger; use max as a conservative stand-in without page dims.
        area_frac: a.area_frac.max(b.area_frac),
        whitespace_est: a.whitespace_est.min(b.whitespace_est),
        origin,
        source_indices,
    }
}

// ---------------------------------------------------------------------------
// K26 — vertical merge
// ---------------------------------------------------------------------------

/// Merge vertically stacked compatible proposals (K26).
///
/// Pairs merge when:
/// - kinds are compatible (same, or Ruled↔Partial),
/// - x-IoU ≥ `x_iou_min` (default [`DEFAULT_X_IOU_MIN`]),
/// - y-gap ≤ `Y_GAP_MEDIAN_MULT * median_line_gap` (overlap counts as gap ≤ 0),
/// - col schema is treated as always compatible in this pure-geometry phase.
///
/// Nested parent/child pairs (see [`is_nested_table_pair`]) never merge.
///
/// Repeats to a fixed point. Input order is not preserved.
pub fn vertical_merge(
    mut proposals: Vec<RegionProposal>,
    median_line_gap: f32,
    x_iou_min: f32,
    policy: &ProposalPolicy,
) -> Vec<RegionProposal> {
    if proposals.len() < 2 {
        return proposals;
    }
    let max_gap = Y_GAP_MEDIAN_MULT * median_line_gap.max(0.0);
    let x_min = if x_iou_min > 0.0 {
        x_iou_min
    } else {
        DEFAULT_X_IOU_MIN
    };

    loop {
        // Top first (higher y1).
        proposals.sort_by(|a, b| {
            b.bbox
                .y1
                .partial_cmp(&a.bbox.y1)
                .unwrap_or(Ordering::Equal)
                .then_with(|| a.bbox.x0.partial_cmp(&b.bbox.x0).unwrap_or(Ordering::Equal))
        });

        let mut merged_any = false;
        let mut used = vec![false; proposals.len()];
        let mut next = Vec::with_capacity(proposals.len());

        for i in 0..proposals.len() {
            if used[i] {
                continue;
            }
            let mut cur = proposals[i].clone();
            // Greedy chain: try to absorb later (lower) proposals.
            for j in (i + 1)..proposals.len() {
                if used[j] {
                    continue;
                }
                let other = &proposals[j];
                if !kinds_mergeable(cur.kind, other.kind) {
                    continue;
                }
                if x_iou(cur.bbox, other.bbox) < x_min {
                    continue;
                }
                // cur is above (sorted) or may overlap after prior merges.
                let (above, below) = if cur.bbox.y1 >= other.bbox.y1 {
                    (cur.bbox, other.bbox)
                } else {
                    (other.bbox, cur.bbox)
                };
                let gap = y_gap(above, below);
                if gap > max_gap {
                    continue;
                }
                // Nested tables (inner rate grid inside outer form) must not
                // fuse into one region — that would drop one of two gold tables.
                if is_nested_table_pair(cur.bbox, other.bbox, policy) {
                    continue;
                }
                cur = merge_pair(&cur, other);
                used[j] = true;
                merged_any = true;
            }
            used[i] = true;
            next.push(cur);
        }

        proposals = next;
        if !merged_any {
            break;
        }
    }
    proposals
}

// ---------------------------------------------------------------------------
// Partition — gates + ownership + same-kind NMS
// ---------------------------------------------------------------------------

/// Independent enter / reject gates for a proposal.
pub fn passes_gate(p: &RegionProposal, policy: &ProposalPolicy) -> bool {
    match p.kind {
        RegionKind::RuledContour => {
            p.joint_count >= policy.min_joints_ruled
                && p.area_frac >= policy.min_area_frac
                && p.whitespace_est < policy.whitespace_reject
                && p.line_score >= policy.line_score_ruled_enter
        }
        RegionKind::PartialRuled => p.line_score >= policy.line_score_partial_enter,
        RegionKind::BorderlessText => p.text_score >= policy.text_score_borderless_enter,
        RegionKind::Residual => p.text_score >= policy.text_score_borderless_enter,
    }
}

/// Exclusive region partition (design AutoRouter).
///
/// Call [`vertical_merge`] first for K26. This step:
/// 1. Drops proposals failing kind-specific gates
/// 2. Sorts by priority ↓, structure_prior ↓, area_frac ↑
/// 3. Accepts if not ownership-blocked by hard owners and not same-kind NMS
pub fn partition(proposals: Vec<RegionProposal>, policy: &ProposalPolicy) -> Vec<RegionProposal> {
    let mut survivors: Vec<RegionProposal> = proposals
        .into_iter()
        .filter(|p| passes_gate(p, policy))
        .collect();

    survivors.sort_by(|a, b| {
        kind_priority(b.kind)
            .cmp(&kind_priority(a.kind))
            .then_with(|| {
                structure_prior(b)
                    .partial_cmp(&structure_prior(a))
                    .unwrap_or(Ordering::Equal)
            })
            .then_with(|| {
                a.area_frac
                    .partial_cmp(&b.area_frac)
                    .unwrap_or(Ordering::Equal)
            })
    });

    let mut owned: Vec<(Rect, RegionKind)> = Vec::new();
    let mut accepted: Vec<RegionProposal> = Vec::new();

    for p in survivors {
        if overlaps_owned(p.bbox, &owned, policy) {
            continue;
        }
        // Same-kind NMS: drop near-duplicates, but keep nested parent/child.
        let nms_hit = accepted.iter().any(|q| {
            q.kind == p.kind
                && rect_iou(p.bbox, q.bbox) >= policy.partition_nms_iou
                && !is_nested_table_pair(p.bbox, q.bbox, policy)
        });
        if nms_hit {
            continue;
        }
        owned.push((pad_rect(p.bbox, policy.ownership_pad), p.kind));
        accepted.push(p);
    }
    accepted
}

// ---------------------------------------------------------------------------
// K27 — emit order
// ---------------------------------------------------------------------------

/// Normative emit-order key: top-to-bottom then left-to-right (−y1, x0).
#[inline]
pub fn emit_order_key(bbox: &Rect) -> (f32, f32) {
    (-bbox.y1, bbox.x0)
}

/// Compare two bboxes for K27 emit order.
pub fn cmp_emit_order(a: &Rect, b: &Rect) -> Ordering {
    emit_order_key(a)
        .0
        .partial_cmp(&emit_order_key(b).0)
        .unwrap_or(Ordering::Equal)
        .then_with(|| {
            emit_order_key(a)
                .1
                .partial_cmp(&emit_order_key(b).1)
                .unwrap_or(Ordering::Equal)
        })
}

/// Sort rects by K27 emit order (in place).
pub fn sort_rects_by_emit_order(rects: &mut [Rect]) {
    rects.sort_by(cmp_emit_order);
}

/// Sort tables by K27 emit order (in place).
pub fn sort_tables_by_emit_order(tables: &mut [Table]) {
    tables.sort_by(|a, b| cmp_emit_order(&a.bbox, &b.bbox));
}

/// Convenience: merge then partition (full router geometry pass).
pub fn route_proposals(
    proposals: Vec<RegionProposal>,
    median_line_gap: f32,
    policy: &ProposalPolicy,
) -> Vec<RegionProposal> {
    let merged = vertical_merge(proposals, median_line_gap, DEFAULT_X_IOU_MIN, policy);
    let mut accepted = partition(merged, policy);
    accepted.sort_by(|a, b| cmp_emit_order(&a.bbox, &b.bbox));
    accepted
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::evidence::RegionKind;
    use crate::types::{Table, TableMethod};

    fn bbox(x0: f32, y0: f32, x1: f32, y1: f32) -> Rect {
        Rect { x0, y0, x1, y1 }
    }

    fn prop(
        kind: RegionKind,
        x0: f32,
        y0: f32,
        x1: f32,
        y1: f32,
        line: f32,
        text: f32,
        joints: u32,
        area: f32,
    ) -> RegionProposal {
        RegionProposal {
            kind,
            bbox: bbox(x0, y0, x1, y1),
            line_score: line,
            text_score: text,
            joint_count: joints,
            area_frac: area,
            whitespace_est: 0.0,
            origin: crate::evidence::ProposalOrigin::Detector,
            source_indices: Vec::new(),
        }
    }

    fn dummy_table(x0: f32, y0: f32, x1: f32, y1: f32) -> Table {
        Table {
            bbox: bbox(x0, y0, x1, y1),
            page: 0,
            method: TableMethod::Lattice,
            confidence: 1.0,
            rows: 2,
            cols: 2,
            cells: Vec::new(),
            header_rows: 0,
            continued_from_previous_page: false,
            continued_to_next_page: false,
            logical_table_id: None,
            strategy_provenance: Vec::new(),
            notes: Vec::new(),
            edge_score: 0.0,
            fill_rate: 0.0,
            weak_edges: false,
            joint_count: 0,
            text_row_recovery: false,
            text_col_recovery: false,
            multitable_stream_recovery: false,
            stream_vs_overwide_hybrid: false,
        }
    }

    #[test]
    fn k26_header_body_vertical_merge() {
        // Header lattice (top) + body partial (below), aligned in x.
        let header = prop(
            RegionKind::RuledContour,
            50.0,
            400.0,
            350.0,
            450.0,
            0.8,
            0.5,
            8,
            0.05,
        );
        let body = prop(
            RegionKind::PartialRuled,
            52.0,
            200.0,
            348.0,
            390.0,
            0.4,
            0.6,
            2,
            0.08,
        );
        // median gap ~12; actual gap = 400 - 390 = 10 ≤ 1.5*12
        let policy = ProposalPolicy::default();
        let out = vertical_merge(vec![header, body], 12.0, DEFAULT_X_IOU_MIN, &policy);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].kind, RegionKind::RuledContour);
        assert!((out[0].bbox.y0 - 200.0).abs() < 1e-3);
        assert!((out[0].bbox.y1 - 450.0).abs() < 1e-3);
    }

    #[test]
    fn k26_skips_low_x_iou() {
        let a = prop(
            RegionKind::RuledContour,
            0.0,
            300.0,
            100.0,
            400.0,
            0.8,
            0.5,
            8,
            0.05,
        );
        let b = prop(
            RegionKind::RuledContour,
            200.0,
            100.0,
            300.0,
            280.0,
            0.8,
            0.5,
            8,
            0.05,
        );
        let policy = ProposalPolicy::default();
        let out = vertical_merge(vec![a, b], 50.0, DEFAULT_X_IOU_MIN, &policy);
        assert_eq!(out.len(), 2);
    }

    #[test]
    fn k26_same_kind_borderless_merges() {
        let top = prop(
            RegionKind::BorderlessText,
            40.0,
            350.0,
            300.0,
            420.0,
            0.1,
            0.7,
            0,
            0.04,
        );
        let bot = prop(
            RegionKind::BorderlessText,
            42.0,
            200.0,
            298.0,
            340.0,
            0.1,
            0.75,
            0,
            0.05,
        );
        let policy = ProposalPolicy::default();
        let out = vertical_merge(vec![top, bot], 10.0, DEFAULT_X_IOU_MIN, &policy);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].kind, RegionKind::BorderlessText);
    }

    #[test]
    fn k26_gap_too_large_no_merge() {
        let top = prop(
            RegionKind::RuledContour,
            50.0,
            500.0,
            350.0,
            550.0,
            0.8,
            0.5,
            8,
            0.05,
        );
        let bot = prop(
            RegionKind::RuledContour,
            50.0,
            100.0,
            350.0,
            200.0,
            0.8,
            0.5,
            8,
            0.05,
        );
        // gap = 500 - 200 = 300; median 10 → max 15
        let policy = ProposalPolicy::default();
        let out = vertical_merge(vec![top, bot], 10.0, DEFAULT_X_IOU_MIN, &policy);
        assert_eq!(out.len(), 2);
    }

    #[test]
    fn partition_ruled_owns_over_borderless() {
        let policy = ProposalPolicy::default();
        let ruled = prop(
            RegionKind::RuledContour,
            50.0,
            200.0,
            350.0,
            500.0,
            0.8,
            0.4,
            12,
            0.10,
        );
        let borderless = prop(
            RegionKind::BorderlessText,
            60.0,
            220.0,
            340.0,
            480.0,
            0.1,
            0.9,
            0,
            0.08,
        );
        let out = partition(vec![ruled, borderless], &policy);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].kind, RegionKind::RuledContour);
    }

    #[test]
    fn partition_rejects_chrome_ticks() {
        let policy = ProposalPolicy::default();
        // Tiny / few joints → fail gates
        let ticks = prop(
            RegionKind::RuledContour,
            0.0,
            0.0,
            10.0,
            10.0,
            0.2,
            0.0,
            2,
            1e-6,
        );
        let real = prop(
            RegionKind::RuledContour,
            50.0,
            100.0,
            400.0,
            500.0,
            0.85,
            0.5,
            16,
            0.12,
        );
        let out = partition(vec![ticks, real], &policy);
        assert_eq!(out.len(), 1);
        assert!(out[0].joint_count >= 5);
        assert!(out[0].area_frac >= policy.min_area_frac);
    }

    #[test]
    fn partition_rejects_high_whitespace_ruled() {
        let policy = ProposalPolicy::default();
        let mut chrome = prop(
            RegionKind::RuledContour,
            50.0,
            100.0,
            400.0,
            500.0,
            0.9,
            0.1,
            20,
            0.15,
        );
        chrome.whitespace_est = 0.95;
        let out = partition(vec![chrome], &policy);
        assert!(out.is_empty());
    }

    #[test]
    fn partition_borderless_when_no_ruled() {
        let policy = ProposalPolicy::default();
        let bl = prop(
            RegionKind::BorderlessText,
            40.0,
            100.0,
            300.0,
            400.0,
            0.05,
            0.7,
            0,
            0.06,
        );
        let out = partition(vec![bl], &policy);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].kind, RegionKind::BorderlessText);
    }

    #[test]
    fn partition_same_kind_nms() {
        let policy = ProposalPolicy::default();
        let a = prop(
            RegionKind::RuledContour,
            50.0,
            200.0,
            350.0,
            500.0,
            0.9,
            0.5,
            20,
            0.10,
        );
        // Near-duplicate, slightly lower score, larger area
        let b = prop(
            RegionKind::RuledContour,
            55.0,
            205.0,
            345.0,
            495.0,
            0.7,
            0.4,
            10,
            0.11,
        );
        let out = partition(vec![a, b], &policy);
        assert_eq!(out.len(), 1);
    }

    #[test]
    fn partition_nested_ruled_both_kept() {
        // Outer form lattice + inner rate table (Italian insurance style).
        let policy = ProposalPolicy::default();
        let outer = prop(
            RegionKind::RuledContour,
            30.0,
            50.0,
            560.0,
            800.0,
            0.87,
            0.45,
            24,
            0.35,
        );
        let inner = prop(
            RegionKind::RuledContour,
            180.0,
            280.0,
            480.0,
            360.0,
            0.95,
            0.9,
            15,
            0.04,
        );
        let out = partition(vec![outer.clone(), inner.clone()], &policy);
        assert_eq!(
            out.len(),
            2,
            "nested outer+inner ruled must both survive, got {out:?}"
        );
        // Either order
        let has_outer = out.iter().any(|p| rect_iou(p.bbox, outer.bbox) > 0.8);
        let has_inner = out.iter().any(|p| rect_iou(p.bbox, inner.bbox) > 0.8);
        assert!(has_outer && has_inner);
    }

    #[test]
    fn vertical_merge_skips_nested_pair() {
        let outer = prop(
            RegionKind::RuledContour,
            30.0,
            50.0,
            560.0,
            800.0,
            0.9,
            0.5,
            20,
            0.3,
        );
        let inner = prop(
            RegionKind::RuledContour,
            180.0,
            280.0,
            480.0,
            360.0,
            0.95,
            0.9,
            12,
            0.04,
        );
        // Large median gap would still allow merge via overlap (gap≤0); nesting must block.
        let policy = ProposalPolicy::default();
        let out = vertical_merge(vec![outer, inner], 50.0, 0.50, &policy);
        assert_eq!(out.len(), 2, "nested pair must not K26-merge: {out:?}");
    }

    #[test]
    fn partition_side_by_side_both_kept() {
        let policy = ProposalPolicy::default();
        let left = prop(
            RegionKind::RuledContour,
            20.0,
            100.0,
            200.0,
            400.0,
            0.85,
            0.5,
            12,
            0.06,
        );
        let right = prop(
            RegionKind::RuledContour,
            250.0,
            100.0,
            430.0,
            400.0,
            0.85,
            0.5,
            12,
            0.06,
        );
        let out = partition(vec![left, right], &policy);
        assert_eq!(out.len(), 2);
    }

    #[test]
    fn k27_emit_order_top_then_left() {
        let mut tables = vec![
            dummy_table(100.0, 50.0, 200.0, 150.0),  // bottom
            dummy_table(50.0, 300.0, 150.0, 400.0),  // top-left
            dummy_table(200.0, 300.0, 300.0, 400.0), // top-right
        ];
        sort_tables_by_emit_order(&mut tables);
        assert!((tables[0].bbox.x0 - 50.0).abs() < 1e-3);
        assert!((tables[1].bbox.x0 - 200.0).abs() < 1e-3);
        assert!((tables[2].bbox.y1 - 150.0).abs() < 1e-3);
    }

    #[test]
    fn emit_order_key_ordering() {
        let top = bbox(10.0, 300.0, 50.0, 400.0);
        let bot = bbox(10.0, 100.0, 50.0, 200.0);
        assert_eq!(cmp_emit_order(&top, &bot), Ordering::Less);
        let k_top = emit_order_key(&top);
        let k_bot = emit_order_key(&bot);
        assert!(k_top.0 < k_bot.0); // -400 < -200
    }

    #[test]
    fn route_proposals_header_body_then_order() {
        let policy = ProposalPolicy::default();
        let header = prop(
            RegionKind::RuledContour,
            50.0,
            400.0,
            350.0,
            450.0,
            0.8,
            0.5,
            8,
            0.05,
        );
        let body = prop(
            RegionKind::PartialRuled,
            52.0,
            200.0,
            348.0,
            390.0,
            0.4,
            0.6,
            2,
            0.08,
        );
        let other = prop(
            RegionKind::BorderlessText,
            400.0,
            50.0,
            550.0,
            150.0,
            0.0,
            0.8,
            0,
            0.03,
        );
        let out = route_proposals(vec![header, body, other], 12.0, &policy);
        // Merged header+body + distant borderless
        assert_eq!(out.len(), 2);
        // Top table first (merged region y1=450)
        assert!(out[0].bbox.y1 > out[1].bbox.y1);
        assert_eq!(out[0].kind, RegionKind::RuledContour);
    }

    #[test]
    fn x_iou_basic() {
        let a = bbox(0.0, 0.0, 100.0, 50.0);
        let b = bbox(50.0, 100.0, 150.0, 200.0);
        // x overlap 50, union 150 → 1/3
        let v = x_iou(a, b);
        assert!((v - 50.0 / 150.0).abs() < 1e-4);
    }

    #[test]
    fn k26_merge_unions_source_indices() {
        let policy = ProposalPolicy::default();
        let mut header = prop(
            RegionKind::RuledContour,
            50.0,
            400.0,
            350.0,
            450.0,
            0.8,
            0.5,
            8,
            0.05,
        );
        header.source_indices = vec![0];
        let mut body = prop(
            RegionKind::PartialRuled,
            52.0,
            200.0,
            348.0,
            390.0,
            0.4,
            0.6,
            2,
            0.08,
        );
        body.source_indices = vec![1];
        let out = vertical_merge(vec![header, body], 12.0, DEFAULT_X_IOU_MIN, &policy);
        assert_eq!(out.len(), 1);
        assert!(out[0].source_indices.contains(&0));
        assert!(out[0].source_indices.contains(&1));
    }

    #[test]
    fn partition_rejects_tiny_corner_under_outer() {
        // Outer form owns page; tiny corner chrome must not survive as nested keep.
        let policy = ProposalPolicy::default();
        let outer = prop(
            RegionKind::RuledContour,
            30.0,
            50.0,
            560.0,
            800.0,
            0.90,
            0.5,
            24,
            0.35,
        );
        let corner = prop(
            RegionKind::RuledContour,
            40.0,
            720.0,
            80.0,
            760.0,
            0.70,
            0.2,
            6,
            0.002,
        );
        let out = partition(vec![outer, corner], &policy);
        assert_eq!(
            out.len(),
            1,
            "tiny corner blocked by outer ownership: {out:?}"
        );
        assert!(out[0].bbox.width() > 100.0);
    }
}
