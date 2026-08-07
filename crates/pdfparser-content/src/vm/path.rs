//! Path construction, dash expansion, clip, and thin-fill rules.
use super::RuleSegment;
use pdfparser_ir::{Matrix3x2, Rect};

/// Intersect two axis-aligned rects (may be empty).
pub(crate) fn intersect_rect(a: Rect, b: Rect) -> Rect {
    Rect {
        x0: a.x0.max(b.x0),
        y0: a.y0.max(b.y0),
        x1: a.x1.min(b.x1),
        y1: a.y1.min(b.y1),
    }
}

/// Clip a near-axis-aligned rule to an optional clip rect (PR2c).
/// Returns `None` if fully outside or degenerate.
pub(crate) fn clip_rule_segment(seg: RuleSegment, clip: Option<Rect>) -> Option<RuleSegment> {
    let Some(c) = clip else {
        return Some(seg);
    };
    if c.x1 <= c.x0 || c.y1 <= c.y0 {
        return None;
    }
    let tol = 1.5f32;
    if seg.is_horizontal(tol) {
        let y = (seg.y0 + seg.y1) * 0.5;
        if y < c.y0 - tol || y > c.y1 + tol {
            return None;
        }
        let x0 = seg.x0.min(seg.x1).max(c.x0);
        let x1 = seg.x0.max(seg.x1).min(c.x1);
        if x1 - x0 < 1.0 {
            return None;
        }
        return Some(RuleSegment {
            x0,
            y0: y,
            x1,
            y1: y,
        });
    }
    if seg.is_vertical(tol) {
        let x = (seg.x0 + seg.x1) * 0.5;
        if x < c.x0 - tol || x > c.x1 + tol {
            return None;
        }
        let y0 = seg.y0.min(seg.y1).max(c.y0);
        let y1 = seg.y0.max(seg.y1).min(c.y1);
        if y1 - y0 < 1.0 {
            return None;
        }
        return Some(RuleSegment {
            x0: x,
            y0,
            x1: x,
            y1,
        });
    }
    None
}

/// Expand an axis-aligned stroked segment through a PDF dash pattern.
///
/// Walks distance along `seg`, alternating on/off from `dash` (phase applied).
/// Emits a [`RuleSegment`] for each ON interval with length ≥ 1.0.
/// Empty / all-zero dash → single solid segment (caller usually checks empty).
pub(crate) fn expand_dash_segment(seg: RuleSegment, dash: &[f32], phase: f32) -> Vec<RuleSegment> {
    if dash.is_empty() || dash.iter().all(|&d| d <= 0.0) {
        return if seg.len() >= 1.0 {
            vec![seg]
        } else {
            Vec::new()
        };
    }

    // PDF: odd-length arrays are effectively doubled to even length.
    let mut pattern: Vec<f32> = dash.iter().map(|&d| d.max(0.0)).collect();
    if pattern.len() % 2 == 1 {
        let copy = pattern.clone();
        pattern.extend_from_slice(&copy);
    }
    let pattern_len: f32 = pattern.iter().sum();
    if pattern_len <= 0.0 {
        return if seg.len() >= 1.0 {
            vec![seg]
        } else {
            Vec::new()
        };
    }

    let total_len = seg.len();
    if total_len < 1.0 {
        return Vec::new();
    }

    let dx = (seg.x1 - seg.x0) / total_len;
    let dy = (seg.y1 - seg.y0) / total_len;

    // Locate start position inside the repeating pattern.
    let mut dist_in_pattern = phase.rem_euclid(pattern_len);
    let mut idx = 0usize;
    let mut acc = 0.0f32;
    while idx < pattern.len() {
        let next = acc + pattern[idx];
        if next > dist_in_pattern + 1e-6 {
            break;
        }
        acc = next;
        idx += 1;
    }
    if idx >= pattern.len() {
        idx = 0;
        acc = 0.0;
        dist_in_pattern = 0.0;
    }
    let mut remaining_in_elem = (pattern[idx] - (dist_in_pattern - acc)).max(0.0);
    if remaining_in_elem < 1e-6 {
        idx = (idx + 1) % pattern.len();
        remaining_in_elem = pattern[idx];
    }
    let mut is_on = idx % 2 == 0;

    let mut out = Vec::new();
    let mut pos = 0.0f32;
    while pos < total_len - 1e-6 {
        let avail = total_len - pos;
        let step = remaining_in_elem.min(avail).max(0.0);
        if step < 1e-8 {
            // Zero-length dash element: advance pattern without moving.
            idx = (idx + 1) % pattern.len();
            remaining_in_elem = pattern[idx];
            is_on = idx % 2 == 0;
            continue;
        }
        if is_on && step >= 1.0 {
            out.push(RuleSegment {
                x0: seg.x0 + dx * pos,
                y0: seg.y0 + dy * pos,
                x1: seg.x0 + dx * (pos + step),
                y1: seg.y0 + dy * (pos + step),
            });
        }
        pos += step;
        remaining_in_elem -= step;
        if remaining_in_elem < 1e-6 {
            idx = (idx + 1) % pattern.len();
            remaining_in_elem = pattern[idx];
            is_on = idx % 2 == 0;
        }
    }
    out
}

#[derive(Default)]
pub(crate) struct PathBuilder {
    start: Option<(f32, f32)>,
    current: Option<(f32, f32)>,
    segs: Vec<((f32, f32), (f32, f32))>,
}

impl PathBuilder {
    pub(crate) fn clear(&mut self) {
        self.start = None;
        self.current = None;
        self.segs.clear();
    }
    pub(crate) fn move_to(&mut self, x: f32, y: f32) {
        self.start = Some((x, y));
        self.current = Some((x, y));
    }
    pub(crate) fn line_to(&mut self, x: f32, y: f32) {
        if let Some(cur) = self.current {
            self.segs.push((cur, (x, y)));
        }
        self.current = Some((x, y));
        if self.start.is_none() {
            self.start = Some((x, y));
        }
    }
    pub(crate) fn close(&mut self) {
        if let (Some(s), Some(c)) = (self.start, self.current) {
            if (s.0 - c.0).abs() > 1e-4 || (s.1 - c.1).abs() > 1e-4 {
                self.segs.push((c, s));
            }
            self.current = self.start;
        }
    }
    pub(crate) fn rect(&mut self, x: f32, y: f32, w: f32, h: f32) {
        self.move_to(x, y);
        self.line_to(x + w, y);
        self.line_to(x + w, y + h);
        self.line_to(x, y + h);
        self.close();
    }
    pub(crate) fn segments_user(&self, ctm: &Matrix3x2) -> Vec<RuleSegment> {
        let mut out = Vec::new();
        for ((x0, y0), (x1, y1)) in &self.segs {
            let p0 = ctm.apply(*x0, *y0);
            let p1 = ctm.apply(*x1, *y1);
            out.push(RuleSegment {
                x0: p0.x,
                y0: p0.y,
                x1: p1.x,
                y1: p1.y,
            });
        }
        out
    }

    /// Axis-aligned bounding box of the path in user space (after CTM).
    pub(crate) fn axis_aligned_bbox_user(&self, ctm: &Matrix3x2) -> Option<Rect> {
        let segs = self.segments_user(ctm);
        if segs.is_empty() {
            return None;
        }
        let mut x0 = f32::INFINITY;
        let mut y0 = f32::INFINITY;
        let mut x1 = f32::NEG_INFINITY;
        let mut y1 = f32::NEG_INFINITY;
        for s in &segs {
            x0 = x0.min(s.x0.min(s.x1));
            y0 = y0.min(s.y0.min(s.y1));
            x1 = x1.max(s.x0.max(s.x1));
            y1 = y1.max(s.y0.max(s.y1));
        }
        if !x0.is_finite() || (x1 - x0) < 1e-3 || (y1 - y0) < 1e-3 {
            return None;
        }
        Some(Rect { x0, y0, x1, y1 })
    }

    /// Convert thin axis-aligned filled shapes into lattice rule segments.
    ///
    /// A path whose axis-aligned bounding box has one side ≤ `thin_max` and the
    /// other ≥ `min_len` is treated as a single horizontal or vertical rule
    /// through the box center (the painted “line”).
    ///
    /// **Subpath-aware:** PDF exporters often accumulate many closed thin
    /// rectangles then paint with a single `f`. Using the union bbox would look
    /// like a fat area and drop all rules. Split into connected closed subpaths
    /// (break when a segment does not continue from the previous endpoint) and
    /// emit a rule per thin subpath.
    pub(crate) fn thin_fill_rules(
        &self,
        ctm: &Matrix3x2,
        thin_max: f32,
        min_len: f32,
    ) -> Vec<RuleSegment> {
        if self.segs.is_empty() {
            return Vec::new();
        }
        let mut out = Vec::new();
        let mut sub: Vec<PathSegEnds> = Vec::new();
        let flush = |sub: &mut Vec<PathSegEnds>, out: &mut Vec<RuleSegment>, ctm: &Matrix3x2| {
            if sub.is_empty() {
                return;
            }
            if let Some(seg) = thin_fill_bbox_rule(sub, ctm, thin_max, min_len) {
                out.push(seg);
            }
            sub.clear();
        };
        for &(a, b) in &self.segs {
            if let Some((_, prev_b)) = sub.last() {
                let cont = (prev_b.0 - a.0).abs() < 1e-3 && (prev_b.1 - a.1).abs() < 1e-3;
                if !cont {
                    flush(&mut sub, &mut out, ctm);
                }
            }
            sub.push((a, b));
        }
        flush(&mut sub, &mut out, ctm);
        // Fallback: whole-path bbox (single open/closed thin rect).
        if out.is_empty() {
            if let Some(seg) = thin_fill_bbox_rule(&self.segs, ctm, thin_max, min_len) {
                out.push(seg);
            }
        }
        out
    }
}

/// Endpoint pair for a path segment in user space.
type PathSegEnds = ((f32, f32), (f32, f32));

/// Axis-aligned thin bbox → one H or V rule, or None.
fn thin_fill_bbox_rule(
    segs: &[PathSegEnds],
    ctm: &Matrix3x2,
    thin_max: f32,
    min_len: f32,
) -> Option<RuleSegment> {
    if segs.is_empty() {
        return None;
    }
    let mut x0 = f32::INFINITY;
    let mut y0 = f32::INFINITY;
    let mut x1 = f32::NEG_INFINITY;
    let mut y1 = f32::NEG_INFINITY;
    for ((ax, ay), (bx, by)) in segs {
        for &(x, y) in &[(ax, ay), (bx, by)] {
            let p = ctm.apply(*x, *y);
            x0 = x0.min(p.x);
            y0 = y0.min(p.y);
            x1 = x1.max(p.x);
            y1 = y1.max(p.y);
        }
    }
    if !x0.is_finite() {
        return None;
    }
    let w = (x1 - x0).abs();
    let h = (y1 - y0).abs();
    if h <= thin_max && w >= min_len {
        let y = (y0 + y1) * 0.5;
        return Some(RuleSegment {
            x0: x0.min(x1),
            y0: y,
            x1: x0.max(x1),
            y1: y,
        });
    }
    if w <= thin_max && h >= min_len {
        let x = (x0 + x1) * 0.5;
        return Some(RuleSegment {
            x0: x,
            y0: y0.min(y1),
            x1: x,
            y1: y0.max(y1),
        });
    }
    None
}
