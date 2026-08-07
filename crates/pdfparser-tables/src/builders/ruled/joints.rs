//! Joint graph, H/V coalesce, and edge flags for ruled lattices.
#![allow(clippy::needless_range_loop)]
#![allow(clippy::type_complexity)]

use pdfparser_content::RuleSegment;
use pdfparser_ir::{Rect, TextRun};

#[derive(Clone, Copy)]
pub(crate) struct Edges {
    pub(crate) left: bool,
    pub(crate) right: bool,
    pub(crate) top: bool,
    pub(crate) bottom: bool,
}

/// Drop horizontal rules that sit under many text baselines (underline deco).
///
/// Applied only after raster/combined sensing so clean vector lattices are untouched.
pub(crate) fn suppress_text_baseline_h_rules(
    rules: &[RuleSegment],
    runs: &[TextRun],
    tol: f32,
) -> Vec<RuleSegment> {
    use crate::geom::{band_runs, median_font_size};
    let fs = median_font_size(runs);
    let thr = (0.35 * fs).max(1.5);
    let bands = band_runs(runs, thr.max(2.5));
    if bands.len() < 3 {
        return rules.to_vec();
    }
    rules
        .iter()
        .copied()
        .filter(|r| {
            if !r.is_horizontal(tol) {
                return true;
            }
            let y = (r.y0 + r.y1) * 0.5;
            let x0 = r.x0.min(r.x1);
            let x1 = r.x0.max(r.x1);
            let len = (x1 - x0).max(1.0);
            let mut hits = 0u32;
            for band in &bands {
                let by: f32 =
                    band.iter().map(|t| t.bbox.y0).sum::<f32>() / band.len().max(1) as f32;
                if (by - y).abs() > thr {
                    continue;
                }
                let bx0 = band.iter().map(|t| t.bbox.x0).fold(f32::INFINITY, f32::min);
                let bx1 = band
                    .iter()
                    .map(|t| t.bbox.x1)
                    .fold(f32::NEG_INFINITY, f32::max);
                let ox0 = x0.max(bx0);
                let ox1 = x1.min(bx1);
                if ox1 - ox0 >= 0.50 * len {
                    hits += 1;
                }
            }
            // Keep structural H rules (few co-located baselines); drop underline soup.
            hits < 3
        })
        .collect()
}

// ─── Segment types (typed H vs V) ───────────────────────────────────────────

#[derive(Clone, Copy, Debug)]
pub(crate) struct HSeg {
    pub(crate) y: f32,
    pub(crate) x0: f32,
    pub(crate) x1: f32,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct VSeg {
    pub(crate) x: f32,
    pub(crate) y0: f32,
    pub(crate) y1: f32,
}

pub(crate) fn coalesce_h(segs: &[HSeg], tol: f32) -> Vec<HSeg> {
    if segs.is_empty() {
        return Vec::new();
    }
    let mut items = segs.to_vec();
    items.sort_by(|a, b| {
        a.y.partial_cmp(&b.y)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.x0.partial_cmp(&b.x0).unwrap_or(std::cmp::Ordering::Equal))
    });
    let mut groups: Vec<Vec<HSeg>> = Vec::new();
    for s in items {
        if let Some(g) = groups.last_mut() {
            let gy = g.iter().map(|x| x.y).sum::<f32>() / g.len() as f32;
            if (s.y - gy).abs() <= tol {
                g.push(s);
                continue;
            }
        }
        groups.push(vec![s]);
    }
    let mut out = Vec::new();
    for g in groups {
        let y = g.iter().map(|x| x.y).sum::<f32>() / g.len() as f32;
        let mut intervals: Vec<(f32, f32)> =
            g.iter().map(|s| (s.x0.min(s.x1), s.x0.max(s.x1))).collect();
        intervals.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
        let mut cur_a = intervals[0].0;
        let mut cur_b = intervals[0].1;
        for &(a, b) in &intervals[1..] {
            if a <= cur_b + tol * 2.0 {
                cur_b = cur_b.max(b);
            } else {
                out.push(HSeg {
                    y,
                    x0: cur_a,
                    x1: cur_b,
                });
                cur_a = a;
                cur_b = b;
            }
        }
        out.push(HSeg {
            y,
            x0: cur_a,
            x1: cur_b,
        });
    }
    out
}

pub(crate) fn coalesce_v(segs: &[VSeg], tol: f32) -> Vec<VSeg> {
    if segs.is_empty() {
        return Vec::new();
    }
    let mut items = segs.to_vec();
    items.sort_by(|a, b| {
        a.x.partial_cmp(&b.x)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.y0.partial_cmp(&b.y0).unwrap_or(std::cmp::Ordering::Equal))
    });
    let mut groups: Vec<Vec<VSeg>> = Vec::new();
    for s in items {
        if let Some(g) = groups.last_mut() {
            let gx = g.iter().map(|x| x.x).sum::<f32>() / g.len() as f32;
            if (s.x - gx).abs() <= tol {
                g.push(s);
                continue;
            }
        }
        groups.push(vec![s]);
    }
    let mut out = Vec::new();
    for g in groups {
        let x = g.iter().map(|s| s.x).sum::<f32>() / g.len() as f32;
        let mut intervals: Vec<(f32, f32)> =
            g.iter().map(|s| (s.y0.min(s.y1), s.y0.max(s.y1))).collect();
        intervals.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
        let mut cur_a = intervals[0].0;
        let mut cur_b = intervals[0].1;
        for &(a, b) in &intervals[1..] {
            if a <= cur_b + tol * 2.0 {
                cur_b = cur_b.max(b);
            } else {
                out.push(VSeg {
                    x,
                    y0: cur_a,
                    y1: cur_b,
                });
                cur_a = a;
                cur_b = b;
            }
        }
        out.push(VSeg {
            x,
            y0: cur_a,
            y1: cur_b,
        });
    }
    out
}

// ─── Union-find components ──────────────────────────────────────────────────

pub(crate) struct UnionFind {
    parent: Vec<usize>,
    rank: Vec<u8>,
}

impl UnionFind {
    fn new(n: usize) -> Self {
        Self {
            parent: (0..n).collect(),
            rank: vec![0; n],
        }
    }
    fn find(&mut self, mut i: usize) -> usize {
        while self.parent[i] != i {
            self.parent[i] = self.parent[self.parent[i]];
            i = self.parent[i];
        }
        i
    }
    fn union(&mut self, a: usize, b: usize) {
        let ra = self.find(a);
        let rb = self.find(b);
        if ra == rb {
            return;
        }
        if self.rank[ra] < self.rank[rb] {
            self.parent[ra] = rb;
        } else if self.rank[ra] > self.rank[rb] {
            self.parent[rb] = ra;
        } else {
            self.parent[rb] = ra;
            self.rank[ra] += 1;
        }
    }
}

/// Joint if H and V cross within expanded segment ends.
pub(crate) fn segments_cross_hv(
    h: &HSeg,
    v: &VSeg,
    snap_tol: f32,
    joint_gap: f32,
) -> Option<(f32, f32)> {
    let hx0 = h.x0 - joint_gap;
    let hx1 = h.x1 + joint_gap;
    let vy0 = v.y0 - joint_gap;
    let vy1 = v.y1 + joint_gap;
    let x = v.x;
    let y = h.y;
    // Line coordinates must align within snap_tol of the geometric ideal (they are exact by construction).
    // Crossing requires the joint to lie within the *expanded* segment ranges.
    if x + snap_tol >= hx0 && x - snap_tol <= hx1 && y + snap_tol >= vy0 && y - snap_tol <= vy1 {
        Some((x, y))
    } else {
        None
    }
}

pub(crate) fn cluster_line_components(
    h_segs: &[HSeg],
    v_segs: &[VSeg],
    snap_tol: f32,
    joint_gap: f32,
    min_joints: usize,
) -> Vec<(Vec<usize>, Vec<usize>, Vec<(f32, f32)>)> {
    let n_h = h_segs.len();
    let n_v = v_segs.len();
    let mut uf = UnionFind::new(n_h + n_v);
    let mut joints_map: Vec<((usize, usize), (f32, f32))> = Vec::new();

    for (hi, h) in h_segs.iter().enumerate() {
        for (vi, v) in v_segs.iter().enumerate() {
            if let Some(pt) = segments_cross_hv(h, v, snap_tol, joint_gap) {
                uf.union(hi, n_h + vi);
                joints_map.push(((hi, vi), pt));
            }
        }
    }

    use std::collections::HashMap;
    let mut by_root: HashMap<usize, (Vec<usize>, Vec<usize>, Vec<(f32, f32)>)> = HashMap::new();
    for hi in 0..n_h {
        let r = uf.find(hi);
        by_root.entry(r).or_default().0.push(hi);
    }
    for vi in 0..n_v {
        let r = uf.find(n_h + vi);
        by_root.entry(r).or_default().1.push(vi);
    }
    for ((hi, _), pt) in joints_map {
        let r = uf.find(hi);
        by_root.entry(r).or_default().2.push(pt);
    }

    by_root
        .into_values()
        .filter(|(_, _, j)| j.len() >= min_joints)
        .collect()
}

// ─── Grid construction ──────────────────────────────────────────────────────
pub(crate) fn filter_joint_supported_coords(
    coords: &[f32],
    joints: &[(f32, f32)],
    tol: f32,
    vertical: bool,
    min_joints: usize,
    min_span_frac: f32,
) -> Vec<f32> {
    if joints.is_empty() {
        return Vec::new();
    }
    let (g0, g1) = if vertical {
        let ys: Vec<f32> = joints.iter().map(|p| p.1).collect();
        (
            ys.iter().copied().fold(f32::INFINITY, f32::min),
            ys.iter().copied().fold(f32::NEG_INFINITY, f32::max),
        )
    } else {
        let xs: Vec<f32> = joints.iter().map(|p| p.0).collect();
        (
            xs.iter().copied().fold(f32::INFINITY, f32::min),
            xs.iter().copied().fold(f32::NEG_INFINITY, f32::max),
        )
    };
    let global_span = (g1 - g0).abs().max(1.0);

    coords
        .iter()
        .copied()
        .filter(|&c| {
            let on_line: Vec<f32> = joints
                .iter()
                .filter(|(jx, jy)| {
                    if vertical {
                        (jx - c).abs() <= tol
                    } else {
                        (jy - c).abs() <= tol
                    }
                })
                .map(|(jx, jy)| if vertical { *jy } else { *jx })
                .collect();
            if on_line.len() < min_joints {
                return false;
            }
            let lo = on_line.iter().copied().fold(f32::INFINITY, f32::min);
            let hi = on_line.iter().copied().fold(f32::NEG_INFINITY, f32::max);
            let span = (hi - lo).abs();
            span >= global_span * min_span_frac
        })
        .collect()
}

/// Redistribute multi-token text across empty same-row cells.
///
/// Fires when a row has exactly one (or few) non-empty cells whose whitespace
/// token count matches the empty span width — classic TJ-string whole-row dump.
///
/// Phase-4: also spill a multi-numeric token cell rightward into empty neighbors
pub(crate) fn edge_flags(
    bbox: Rect,
    h_segs: &[HSeg],
    v_segs: &[VSeg],
    tol: f32,
    cover_frac: f32,
) -> Edges {
    let cover_h = |y: f32, x0: f32, x1: f32| -> bool {
        let mut covered = 0.0f32;
        let need = (x1 - x0).max(1.0) * cover_frac;
        for s in h_segs {
            if (s.y - y).abs() > tol * 1.5 {
                continue;
            }
            let a = s.x0.max(x0);
            let b = s.x1.min(x1);
            if b > a {
                covered += b - a;
            }
        }
        covered >= need
    };
    let cover_v = |x: f32, y0: f32, y1: f32| -> bool {
        let mut covered = 0.0f32;
        let need = (y1 - y0).max(1.0) * cover_frac;
        for s in v_segs {
            if (s.x - x).abs() > tol * 1.5 {
                continue;
            }
            let a = s.y0.max(y0);
            let b = s.y1.min(y1);
            if b > a {
                covered += b - a;
            }
        }
        covered >= need
    };
    Edges {
        left: cover_v(bbox.x0, bbox.y0, bbox.y1),
        right: cover_v(bbox.x1, bbox.y0, bbox.y1),
        top: cover_h(bbox.y1, bbox.x0, bbox.x1),
        bottom: cover_h(bbox.y0, bbox.x0, bbox.x1),
    }
}
