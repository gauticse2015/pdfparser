//! Geometry-justified policy for Engine V2 exclusive AutoRouter.
//!
//! Product Auto/Full use this policy via [`ProposalPolicy::from_options`] so
//! thresholds stay aligned with [`crate::TableOptions`] (single source of truth).

use crate::evidence::RegionKind;
use crate::options::TableOptions;
use pdfparser_ir::Rect;

/// Proposal / ownership policy for AutoRouter (normative defaults).
#[derive(Debug, Clone)]
pub struct ProposalPolicy {
    /// Min joints for ruled region (aligned with lattice min joints when known).
    pub min_joints_ruled: u32,
    /// Min table area as fraction of page.
    pub min_area_frac: f32,
    /// Whitespace/empty reject for ruled chrome.
    pub whitespace_reject: f32,
    /// IoU against owned ruled/partial region that blocks another proposal.
    pub ownership_iou_tau: f32,
    /// Containment fraction for ownership / nested child-in-parent test.
    pub ownership_contain_frac: f32,
    /// Pad (user units) applied to owned bboxes when testing ownership.
    ///
    /// Design uses `max(2pt, 0.5 * median_font)`; callers may set from page stats.
    pub ownership_pad: f32,
    /// Line score to enter ruled builder.
    pub line_score_ruled_enter: f32,
    /// Line score for partial-ruled.
    pub line_score_partial_enter: f32,
    /// Text score for borderless enter.
    pub text_score_borderless_enter: f32,
    /// Min structure score to emit a table.
    pub min_structure_score_emit: f32,
    /// Same-kind NMS IoU.
    pub partition_nms_iou: f32,
    /// Allow text densify as secondary recovery (mirrors lattice densify flag).
    pub allow_text_densify: bool,
    /// Nested pair: max (smaller/larger) area ratio (true nest, not twin).
    pub nested_max_area_ratio: f32,
    /// Nested pair: min (smaller/larger) area ratio (reject tiny corner FPs).
    pub nested_min_area_ratio: f32,
    /// Nested pair: min absolute area of the smaller rect (user units²).
    pub nested_min_child_area: f32,
    /// Nested pair: min width and height of the smaller rect.
    pub nested_min_child_side: f32,
}

impl Default for ProposalPolicy {
    fn default() -> Self {
        Self {
            min_joints_ruled: 5,
            min_area_frac: 5e-4,
            whitespace_reject: 0.90,
            ownership_iou_tau: 0.15,
            ownership_contain_frac: 0.85,
            ownership_pad: 2.0,
            line_score_ruled_enter: 0.55,
            line_score_partial_enter: 0.25,
            text_score_borderless_enter: 0.50,
            min_structure_score_emit: 0.45,
            partition_nms_iou: 0.60,
            allow_text_densify: true,
            // Child ≤55% of parent (not near-duplicate), ≥3% and ≥ min area/side
            // so decorative corner grids do not count as nested multi-tables.
            nested_max_area_ratio: 0.55,
            nested_min_area_ratio: 0.03,
            nested_min_child_area: 2_500.0,
            nested_min_child_side: 36.0,
        }
    }
}

impl ProposalPolicy {
    /// Build router policy from product [`TableOptions`] (SSOT for shared knobs).
    pub fn from_options(opts: &TableOptions) -> Self {
        let mut p = Self::default();
        // Ruled joint gate: at least lattice min joints, never below design floor 4.
        p.min_joints_ruled = opts.lattice_min_joints.max(4);
        p.whitespace_reject = opts.lattice_empty_frac_reject.clamp(0.5, 0.99);
        p.allow_text_densify = opts.lattice_text_densify;
        // Area floor as fraction of letter-ish page when options give absolute area.
        // Callers still pass real page area into area_frac on proposals.
        if opts.lattice_min_table_area > 0.0 {
            p.min_area_frac = (opts.lattice_min_table_area / (612.0 * 792.0)).max(1e-5);
        }
        p
    }
}

/// Intersection area of two axis-aligned rects (0 if no overlap).
pub fn intersection_area(a: Rect, b: Rect) -> f32 {
    let x0 = a.x0.max(b.x0);
    let y0 = a.y0.max(b.y0);
    let x1 = a.x1.min(b.x1);
    let y1 = a.y1.min(b.y1);
    let w = (x1 - x0).max(0.0);
    let h = (y1 - y0).max(0.0);
    w * h
}

/// Area of a rect.
pub fn rect_area(r: Rect) -> f32 {
    r.width() * r.height()
}

/// Standard intersection-over-union of two rects.
pub fn rect_iou(a: Rect, b: Rect) -> f32 {
    let inter = intersection_area(a, b);
    let ua = rect_area(a) + rect_area(b) - inter;
    if ua <= 0.0 {
        0.0
    } else {
        inter / ua
    }
}

/// Expand a bbox by a uniform pad (clamped so empty rects stay empty).
pub fn pad_rect(r: Rect, pad: f32) -> Rect {
    if pad <= 0.0 {
        return r;
    }
    Rect {
        x0: r.x0 - pad,
        y0: r.y0 - pad,
        x1: r.x1 + pad,
        y1: r.y1 + pad,
    }
}

/// Whether a region kind is a hard owner (Ruled / Partial block other claims).
#[inline]
pub fn is_hard_owner(kind: RegionKind) -> bool {
    matches!(kind, RegionKind::RuledContour | RegionKind::PartialRuled)
}

/// True when two rects form a **nested table** pair (child largely inside parent
/// and substantially smaller, but not a tiny corner fragment).
///
/// Used so exclusive partition / ownership keep both an outer form lattice and
/// an inner rate table (Italian insurance / similar nested ruled grids), without
/// treating decorative corner grids as independent nested tables.
pub fn is_nested_table_pair(a: Rect, b: Rect, policy: &ProposalPolicy) -> bool {
    let inter = intersection_area(a, b);
    if inter <= 0.0 {
        return false;
    }
    let aa = rect_area(a).max(1e-3);
    let ab = rect_area(b).max(1e-3);
    let (small_r, large_r, smaller, larger) = if aa <= ab {
        (a, b, aa, ab)
    } else {
        (b, a, ab, aa)
    };
    let _ = large_r;
    let contain_small = inter / smaller;
    let area_ratio = smaller / larger;
    if contain_small < policy.ownership_contain_frac {
        return false;
    }
    if area_ratio > policy.nested_max_area_ratio || area_ratio < policy.nested_min_area_ratio {
        return false;
    }
    if smaller < policy.nested_min_child_area {
        return false;
    }
    let sw = small_r.width().abs();
    let sh = small_r.height().abs();
    if sw < policy.nested_min_child_side || sh < policy.nested_min_child_side {
        return false;
    }
    true
}

/// Ownership test: does `owner` (hard kind) block claim bbox `b` under `policy`?
///
/// Normative (design K5 / partition):
/// - IoU(b, owner) ≥ `ownership_iou_tau`, or
/// - containment either way ≥ `ownership_contain_frac`.
///
/// **Nested exception:** a smaller ruled region inside a larger one (or the
/// outer claim when a nested child already owns) does **not** block — both
/// emit as separate tables.
pub fn ownership_blocks(b: Rect, owner: Rect, owner_kind: RegionKind, policy: &ProposalPolicy) -> bool {
    if !is_hard_owner(owner_kind) {
        return false;
    }
    // Nested multi-table: parent↔child must both survive exclusive partition.
    if is_nested_table_pair(b, owner, policy) {
        return false;
    }
    if rect_iou(b, owner) >= policy.ownership_iou_tau {
        return true;
    }
    let inter = intersection_area(b, owner);
    let ab = rect_area(b);
    let ao = rect_area(owner);
    if ab > 0.0 && inter / ab >= policy.ownership_contain_frac {
        return true;
    }
    if ao > 0.0 && inter / ao >= policy.ownership_contain_frac {
        return true;
    }
    false
}

/// True if claim bbox overlaps any hard-owned region in `owned`.
///
/// `owned` entries are `(padded_bbox, kind)` as stored by the router after accept.
pub fn overlaps_owned(
    b: Rect,
    owned: &[(Rect, RegionKind)],
    policy: &ProposalPolicy,
) -> bool {
    owned
        .iter()
        .any(|&(ob, ok)| ownership_blocks(b, ob, ok, policy))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::evidence::RegionKind;

    fn r(x0: f32, y0: f32, x1: f32, y1: f32) -> Rect {
        Rect { x0, y0, x1, y1 }
    }

    #[test]
    fn hard_owner_iou_blocks() {
        let policy = ProposalPolicy::default();
        let owner = r(0.0, 0.0, 100.0, 100.0);
        let claim = r(10.0, 10.0, 90.0, 90.0);
        assert!(ownership_blocks(
            claim,
            owner,
            RegionKind::RuledContour,
            &policy
        ));
        assert!(!ownership_blocks(
            claim,
            owner,
            RegionKind::BorderlessText,
            &policy
        ));
    }

    #[test]
    fn distant_claim_not_blocked() {
        let policy = ProposalPolicy::default();
        let owner = r(0.0, 0.0, 50.0, 50.0);
        let claim = r(200.0, 200.0, 300.0, 300.0);
        assert!(!ownership_blocks(
            claim,
            owner,
            RegionKind::PartialRuled,
            &policy
        ));
    }

    #[test]
    fn containment_blocks_small_claim_inside() {
        let policy = ProposalPolicy::default();
        let owner = r(0.0, 0.0, 100.0, 100.0);
        // Near-same-size overlap (not nested — area ratio > 0.55) still blocks.
        let claim = r(10.0, 10.0, 90.0, 90.0);
        assert!(
            ownership_blocks(claim, owner, RegionKind::RuledContour, &policy),
            "large-overlap same-scale claims still exclusive"
        );
    }

    #[test]
    fn nested_pair_does_not_block_either_way() {
        let policy = ProposalPolicy::default();
        let outer = r(30.0, 50.0, 560.0, 800.0);
        // Inner ~300×80 = 24k area; outer ~530×750 ≈ 397k → ratio ~0.06, sides OK.
        let inner = r(180.0, 280.0, 480.0, 360.0);
        assert!(
            is_nested_table_pair(outer, inner, &policy),
            "substantial inner rate grid must nest"
        );
        // Child must not block parent, parent must not block child.
        assert!(!ownership_blocks(
            outer,
            inner,
            RegionKind::RuledContour,
            &policy
        ));
        assert!(!ownership_blocks(
            inner,
            outer,
            RegionKind::RuledContour,
            &policy
        ));
    }

    #[test]
    fn tiny_corner_fragment_is_not_nested_pair() {
        let policy = ProposalPolicy::default();
        let outer = r(30.0, 50.0, 560.0, 800.0);
        // ~40×40 corner chrome — area and side floors must reject.
        let corner = r(40.0, 720.0, 80.0, 760.0);
        assert!(
            !is_nested_table_pair(outer, corner, &policy),
            "tiny corner must not count as nested multi-table"
        );
        // Ownership should still block the corner claim under the outer.
        assert!(ownership_blocks(
            corner,
            outer,
            RegionKind::RuledContour,
            &policy
        ));
    }

    #[test]
    fn from_options_aligns_joint_and_densify() {
        let mut opts = crate::options::TableOptions::default();
        opts.lattice_min_joints = 6;
        opts.lattice_text_densify = false;
        opts.lattice_empty_frac_reject = 0.88;
        let p = ProposalPolicy::from_options(&opts);
        assert_eq!(p.min_joints_ruled, 6);
        assert!(!p.allow_text_densify);
        assert!((p.whitespace_reject - 0.88).abs() < 1e-6);
    }
}
