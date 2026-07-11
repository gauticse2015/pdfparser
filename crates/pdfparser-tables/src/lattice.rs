//! S2 lattice detector: multi-region ruled grids (CC of crossing lines) + R9 text assign.
//!
//! - Connected components of H∩V segments → one table per grid region
//! - Collinear coalesce + single joint-gap model (from TableOptions)
//! - Anchors from joints + line coordinates only (no orthogonal endpoint injection)
//! - Dense grid after dropping thin gaps; edge-measured confidence; typed weak_edges
use crate::geom::{assign_text, bbox_of_cells, cluster_coords, grid_regularity_score};
use crate::options::TableOptions;
use crate::types::{PipelineId, Table, TableCell, TableMethod};
use pdfparser_content::RuleSegment;
use pdfparser_ir::{Rect, TextRun};

/// Detect lattice tables on a page (may emit multiple).
pub fn detect_lattice_tables(
    page_index: u32,
    runs: &[TextRun],
    rules: &[RuleSegment],
    opts: &TableOptions,
) -> Vec<Table> {
    let tol = opts.line_snap_tol;
    let min_cell = opts.min_cell_size;
    let min_seg = opts.lattice_min_seg_len;
    let joint_gap = opts.lattice_joint_gap;
    let min_joints = opts.lattice_min_joints.max(1) as usize;

    let mut h_segs: Vec<HSeg> = Vec::new();
    let mut v_segs: Vec<VSeg> = Vec::new();
    for r in rules {
        if r.len() < min_seg {
            continue;
        }
        if r.is_horizontal(tol) {
            let y = (r.y0 + r.y1) * 0.5;
            h_segs.push(HSeg {
                y,
                x0: r.x0.min(r.x1),
                x1: r.x0.max(r.x1),
            });
        } else if r.is_vertical(tol) {
            let x = (r.x0 + r.x1) * 0.5;
            v_segs.push(VSeg {
                x,
                y0: r.y0.min(r.y1),
                y1: r.y0.max(r.y1),
            });
        }
    }

    if h_segs.len() < 2 || v_segs.len() < 2 {
        return Vec::new();
    }

    h_segs = coalesce_h(&h_segs, tol);
    v_segs = coalesce_v(&v_segs, tol);

    // Single joint model: expand segments by joint_gap; pass snap tol separately.
    let clusters = cluster_line_components(&h_segs, &v_segs, tol, joint_gap, min_joints);
    let multi_component = clusters.len() > 1;

    let mut tables = Vec::new();
    for (hi, vi, joints) in &clusters {
        if let Some(t) = table_from_component(
            page_index,
            runs,
            &h_segs,
            &v_segs,
            hi,
            vi,
            joints,
            opts,
            min_cell,
            tol,
        ) {
            tables.push(t);
        }
    }

    // Global snap only when we did not already see multiple joint-rich components.
    // Multi-CC failure must not re-fuse into a page-wide mega-grid.
    if tables.is_empty() && !multi_component {
        if let Some(t) =
            table_from_global_snap(page_index, runs, &h_segs, &v_segs, opts, min_cell, tol)
        {
            tables.push(t);
        }
    }

    tables.sort_by(|a, b| {
        b.bbox
            .y1
            .partial_cmp(&a.bbox.y1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| {
                a.bbox
                    .x0
                    .partial_cmp(&b.bbox.x0)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
    });
    tables
}

// ─── Segment types (typed H vs V) ───────────────────────────────────────────

#[derive(Clone, Copy, Debug)]
struct HSeg {
    y: f32,
    x0: f32,
    x1: f32,
}

impl HSeg {
    fn len(self) -> f32 {
        (self.x1 - self.x0).abs()
    }
}

#[derive(Clone, Copy, Debug)]
struct VSeg {
    x: f32,
    y0: f32,
    y1: f32,
}

impl VSeg {
    fn len(self) -> f32 {
        (self.y1 - self.y0).abs()
    }
}

fn coalesce_h(segs: &[HSeg], tol: f32) -> Vec<HSeg> {
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
        let mut intervals: Vec<(f32, f32)> = g.iter().map(|s| (s.x0.min(s.x1), s.x0.max(s.x1))).collect();
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

fn coalesce_v(segs: &[VSeg], tol: f32) -> Vec<VSeg> {
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
        let mut intervals: Vec<(f32, f32)> = g.iter().map(|s| (s.y0.min(s.y1), s.y0.max(s.y1))).collect();
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

struct UnionFind {
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
fn segments_cross_hv(h: &HSeg, v: &VSeg, snap_tol: f32, joint_gap: f32) -> Option<(f32, f32)> {
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

fn cluster_line_components(
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

fn table_from_component(
    page_index: u32,
    runs: &[TextRun],
    h_segs: &[HSeg],
    v_segs: &[VSeg],
    h_idx: &[usize],
    v_idx: &[usize],
    joints: &[(f32, f32)],
    opts: &TableOptions,
    min_cell: f32,
    tol: f32,
) -> Option<Table> {
    // Anchors: joints + line coordinates of segments in this component only.
    // Do NOT inject H endpoints into xs or V endpoints into ys.
    let mut xs: Vec<f32> = joints.iter().map(|p| p.0).collect();
    let mut ys: Vec<f32> = joints.iter().map(|p| p.1).collect();
    for &i in v_idx {
        xs.push(v_segs[i].x);
    }
    for &i in h_idx {
        ys.push(h_segs[i].y);
    }

    let xs = cluster_coords(&xs, tol);
    let ys = cluster_coords(&ys, tol);
    if xs.len() < 3 || ys.len() < 3 {
        return None;
    }

    // Keep coordinates that have joint or segment support (line coord, not endpoints).
    let xs = filter_supported_x(&xs, v_segs, v_idx, joints, tol, opts.lattice_min_seg_len);
    let ys = filter_supported_y(&ys, h_segs, h_idx, joints, tol, opts.lattice_min_seg_len);
    if xs.len() < 3 || ys.len() < 3 {
        return None;
    }

    // Drop thin gaps → dense retained line sets (renumbered).
    let xs = collapse_thin_gaps(&xs, min_cell);
    let mut y_ttb = collapse_thin_gaps(&ys, min_cell);
    y_ttb.sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));

    let nrows = y_ttb.len().saturating_sub(1);
    let ncols = xs.len().saturating_sub(1);
    if nrows < 2 || ncols < 2 {
        return None;
    }
    if nrows as u32 > opts.lattice_max_rows || ncols as u32 > opts.lattice_max_cols {
        return None;
    }

    let h_local: Vec<HSeg> = h_idx.iter().map(|&i| h_segs[i]).collect();
    let v_local: Vec<VSeg> = v_idx.iter().map(|&i| v_segs[i]).collect();
    let cover_frac = opts.lattice_edge_cover_frac;

    // Dense nrows×ncols cells
    let mut grid: Vec<Vec<RawCell>> = Vec::with_capacity(nrows);
    let mut filled = 0usize;
    let mut edge_hits = 0u32;
    let mut edge_total = 0u32;

    for row in 0..nrows {
        let y_top = y_ttb[row];
        let y_bot = y_ttb[row + 1];
        let (y1, y0) = if y_top >= y_bot {
            (y_top, y_bot)
        } else {
            (y_bot, y_top)
        };
        let mut row_cells = Vec::with_capacity(ncols);
        for col in 0..ncols {
            let x0 = xs[col];
            let x1 = xs[col + 1];
            let bbox = Rect { x0, y0, x1, y1 };
            let edges = edge_flags(bbox, &h_local, &v_local, tol, cover_frac);
            edge_total += 4;
            if edges.left {
                edge_hits += 1;
            }
            if edges.right {
                edge_hits += 1;
            }
            if edges.top {
                edge_hits += 1;
            }
            if edges.bottom {
                edge_hits += 1;
            }
            let text = assign_text(runs, bbox);
            if !text.trim().is_empty() {
                filled += 1;
            }
            row_cells.push(RawCell {
                bbox,
                text,
                edges,
                active: true,
                colspan: 1,
                rowspan: 1,
            });
        }
        grid.push(row_cells);
    }

    let total = (nrows * ncols).max(1);
    let fill_rate = filled as f32 / total as f32;
    if fill_rate < opts.lattice_min_fill_rate && filled < 2 {
        return None;
    }
    let empty_frac = 1.0 - fill_rate;
    if empty_frac >= opts.lattice_empty_frac_reject
        && filled < opts.lattice_min_filled_cells as usize
    {
        return None;
    }

    merge_spans_dense(&mut grid);

    let cells = emit_cells(&grid);
    if cells.is_empty() {
        return None;
    }

    let max_row = cells
        .iter()
        .map(|c| c.row + c.rowspan)
        .max()
        .unwrap_or(0);
    let max_col = cells
        .iter()
        .map(|c| c.col + c.colspan)
        .max()
        .unwrap_or(0);
    if max_row < 2 || max_col < 2 {
        return None;
    }

    let bbox = bbox_of_cells(&cells);
    let grid_regularity = grid_regularity_score(&xs, &y_ttb);
    let edge_score = if edge_total == 0 {
        0.0
    } else {
        edge_hits as f32 / edge_total as f32
    };
    // Measured joint density vs full grid line crossings upper bound
    let expected_joints = (xs.len() * y_ttb.len()) as f32;
    let joint_density = if expected_joints < 1.0 {
        0.0
    } else {
        (joints.len() as f32 / expected_joints).min(1.0)
    };

    let conf = (0.30 * grid_regularity
        + 0.25 * edge_score
        + 0.20 * fill_rate
        + 0.15 * joint_density
        + 0.10 * (cells.len() as f32 / 6.0).min(1.0))
    .clamp(0.0, 1.0);

    let weak_edges = edge_score < opts.lattice_weak_edge_threshold;
    let mut notes = vec![format!(
        "lattice_cc joints={} h={} v={} xs={} ys={} edge={edge_score:.2}",
        joints.len(),
        h_idx.len(),
        v_idx.len(),
        xs.len(),
        y_ttb.len()
    )];
    if cells.iter().any(|c| c.colspan > 1 || c.rowspan > 1) {
        notes.push("spans_merged".into());
    }

    Some(Table {
        bbox,
        page: page_index,
        method: TableMethod::Lattice,
        confidence: conf,
        rows: max_row,
        cols: max_col,
        cells,
        header_rows: 1,
        continued_from_previous_page: false,
        continued_to_next_page: false,
        logical_table_id: None,
        strategy_provenance: vec![PipelineId::S2Lattice],
        notes,
        edge_score,
        fill_rate,
        weak_edges,
    })
}

fn filter_supported_x(
    coords: &[f32],
    v_segs: &[VSeg],
    v_idx: &[usize],
    joints: &[(f32, f32)],
    tol: f32,
    min_seg: f32,
) -> Vec<f32> {
    coords
        .iter()
        .copied()
        .filter(|&c| {
            joints.iter().any(|(jx, _)| (jx - c).abs() <= tol)
                || v_idx.iter().any(|&i| {
                    let s = v_segs[i];
                    (s.x - c).abs() <= tol && s.len() >= min_seg
                })
        })
        .collect()
}

fn filter_supported_y(
    coords: &[f32],
    h_segs: &[HSeg],
    h_idx: &[usize],
    joints: &[(f32, f32)],
    tol: f32,
    min_seg: f32,
) -> Vec<f32> {
    coords
        .iter()
        .copied()
        .filter(|&c| {
            joints.iter().any(|(_, jy)| (jy - c).abs() <= tol)
                || h_idx.iter().any(|&i| {
                    let s = h_segs[i];
                    (s.y - c).abs() <= tol && s.len() >= min_seg
                })
        })
        .collect()
}

/// Merge consecutive coordinates whose gap is below min_cell (keep outer of each pair).
fn collapse_thin_gaps(coords: &[f32], min_cell: f32) -> Vec<f32> {
    if coords.is_empty() {
        return Vec::new();
    }
    let mut v = coords.to_vec();
    v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let mut out = vec![v[0]];
    for &c in &v[1..] {
        let prev = *out.last().unwrap();
        if (c - prev).abs() < min_cell {
            // Absorb into previous (keep first); for outer max use last of cluster later
            // Prefer keeping both endpoints of the grid: skip interior thin lines.
            continue;
        }
        out.push(c);
    }
    // Ensure we kept the last coordinate if it was skipped as thin mid-pair
    let last = *v.last().unwrap();
    if out.last().map(|x| (*x - last).abs() > 1e-3).unwrap_or(true) {
        // If last is close to final out, replace; else push
        if let Some(p) = out.last_mut() {
            if (last - *p).abs() < min_cell {
                *p = last;
            } else {
                out.push(last);
            }
        }
    }
    out
}

#[derive(Clone, Copy)]
struct Edges {
    left: bool,
    right: bool,
    top: bool,
    bottom: bool,
}

struct RawCell {
    bbox: Rect,
    text: String,
    edges: Edges,
    active: bool,
    colspan: u32,
    rowspan: u32,
}

fn edge_flags(
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

/// Span merge on a dense grid; tracks colspan/rowspan on the surviving cell.
fn merge_spans_dense(grid: &mut [Vec<RawCell>]) {
    let nrows = grid.len();
    if nrows == 0 {
        return;
    }
    let ncols = grid[0].len();

    // Horizontal: missing shared vertical edge
    for r in 0..nrows {
        let mut c = 0usize;
        while c < ncols {
            if !grid[r][c].active {
                c += 1;
                continue;
            }
            let mut c_end = c;
            while c_end + 1 < ncols {
                if !grid[r][c_end + 1].active {
                    break;
                }
                let can = !grid[r][c].edges.right && !grid[r][c_end + 1].edges.left;
                if !can {
                    break;
                }
                let right_text = grid[r][c_end + 1].text.clone();
                let right_bbox = grid[r][c_end + 1].bbox;
                let right_edge = grid[r][c_end + 1].edges.right;
                let add_span = grid[r][c_end + 1].colspan;
                if !right_text.trim().is_empty() {
                    if !grid[r][c].text.is_empty() && !grid[r][c].text.ends_with(' ') {
                        grid[r][c].text.push(' ');
                    }
                    grid[r][c].text.push_str(&right_text);
                }
                grid[r][c].bbox = grid[r][c].bbox.union(right_bbox);
                grid[r][c].edges.right = right_edge;
                grid[r][c].colspan += add_span;
                grid[r][c_end + 1].active = false;
                c_end += 1;
            }
            c = c_end + 1;
        }
    }

    // Vertical: missing shared horizontal edge (only if both active; left cell is master)
    for c in 0..ncols {
        let mut r = 0usize;
        while r < nrows {
            if !grid[r][c].active {
                r += 1;
                continue;
            }
            let mut r_end = r;
            while r_end + 1 < nrows {
                if !grid[r_end + 1][c].active {
                    break;
                }
                // Only merge if col spans align (same master column start)
                if grid[r_end + 1][c].colspan != grid[r][c].colspan {
                    break;
                }
                let can = !grid[r][c].edges.bottom && !grid[r_end + 1][c].edges.top;
                if !can {
                    break;
                }
                let bot_text = grid[r_end + 1][c].text.clone();
                let bot_bbox = grid[r_end + 1][c].bbox;
                let bot_edge = grid[r_end + 1][c].edges.bottom;
                let add_span = grid[r_end + 1][c].rowspan;
                if !bot_text.trim().is_empty() {
                    if !grid[r][c].text.is_empty() {
                        grid[r][c].text.push('\n');
                    }
                    grid[r][c].text.push_str(&bot_text);
                }
                grid[r][c].bbox = grid[r][c].bbox.union(bot_bbox);
                grid[r][c].edges.bottom = bot_edge;
                grid[r][c].rowspan += add_span;
                grid[r_end + 1][c].active = false;
                r_end += 1;
            }
            r = r_end + 1;
        }
    }
}

fn emit_cells(grid: &[Vec<RawCell>]) -> Vec<TableCell> {
    let mut out = Vec::new();
    for (r, row) in grid.iter().enumerate() {
        for (c, cell) in row.iter().enumerate() {
            if !cell.active {
                continue;
            }
            out.push(TableCell {
                row: r as u32,
                col: c as u32,
                rowspan: cell.rowspan,
                colspan: cell.colspan,
                bbox: cell.bbox,
                text: cell.text.clone(),
                is_header: r == 0,
                confidence: 0.9,
            });
        }
    }
    out
}

fn table_from_global_snap(
    page_index: u32,
    runs: &[TextRun],
    h_segs: &[HSeg],
    v_segs: &[VSeg],
    opts: &TableOptions,
    min_cell: f32,
    tol: f32,
) -> Option<Table> {
    let joint_gap = opts.lattice_joint_gap;
    let min_joints = opts.lattice_min_joints.max(1) as usize;

    let xs = cluster_coords(&v_segs.iter().map(|s| s.x).collect::<Vec<_>>(), tol);
    let ys = cluster_coords(&h_segs.iter().map(|s| s.y).collect::<Vec<_>>(), tol);
    if xs.len() < 3 || ys.len() < 3 {
        return None;
    }

    // Joints only where both an H and V segment actually cover the crossing
    // (with joint_gap), not a full Cartesian product of all line coords.
    let mut joints = Vec::new();
    for h in h_segs {
        for v in v_segs {
            if let Some(pt) = segments_cross_hv(h, v, tol, joint_gap) {
                joints.push(pt);
            }
        }
    }
    if joints.len() < min_joints {
        return None;
    }

    let h_idx: Vec<usize> = (0..h_segs.len()).collect();
    let v_idx: Vec<usize> = (0..v_segs.len()).collect();
    let mut t = table_from_component(
        page_index,
        runs,
        h_segs,
        v_segs,
        &h_idx,
        &v_idx,
        &joints,
        opts,
        min_cell,
        tol,
    )?;
    t.notes.push("lattice_global_fallback".into());
    Some(t)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_disjoint_grids_two_components() {
        let mut h = Vec::new();
        let mut v = Vec::new();
        for y in [0.0_f32, 50.0, 100.0] {
            h.push(HSeg {
                y,
                x0: 0.0,
                x1: 100.0,
            });
            h.push(HSeg {
                y,
                x0: 200.0,
                x1: 300.0,
            });
        }
        for x in [0.0_f32, 50.0, 100.0] {
            v.push(VSeg {
                x,
                y0: 0.0,
                y1: 100.0,
            });
        }
        for x in [200.0_f32, 250.0, 300.0] {
            v.push(VSeg {
                x,
                y0: 0.0,
                y1: 100.0,
            });
        }
        let clusters = cluster_line_components(&h, &v, 2.0, 3.5, 4);
        assert!(
            clusters.len() >= 2,
            "expected ≥2 components, got {}",
            clusters.len()
        );
    }

    #[test]
    fn collapse_thin_keeps_span() {
        let xs = vec![0.0, 1.0, 50.0, 100.0]; // 1.0 is thin after 0
        let out = collapse_thin_gaps(&xs, 3.0);
        assert!(out.len() >= 3, "{out:?}");
        assert!((out[0] - 0.0).abs() < 1e-3);
        assert!((out.last().copied().unwrap() - 100.0).abs() < 1e-3);
    }
}
