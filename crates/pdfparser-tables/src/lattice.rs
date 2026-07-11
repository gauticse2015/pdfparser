//! S2 lattice detector: multi-region ruled grids (CC of crossing lines) + R9 text assign.
//!
//! - Connected components of H∩V segments → one table per grid region
//! - Collinear coalesce + single joint-gap model (from TableOptions)
//! - Anchors from joints + line coordinates only (no orthogonal endpoint injection)
//! - Dense grid after dropping thin gaps; edge-measured confidence; typed weak_edges
use crate::geom::{
    assign_runs_exclusive, bbox_of_cells, cluster_coords, grid_regularity_score,
};
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

    for t in &mut tables {
        strip_trailing_footer_totals(t);
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

    // Vertical lines (columns): strict joint count + span — drops short phantom ticks.
    // Horizontal lines (rows): joint count only (or looser span) — multi-level headers often
    // have short H rules only under sub-columns (Act/Bud), which must be kept for structure.
    let min_jpl = opts.lattice_min_joints_per_line.max(1) as usize;
    let xs = filter_joint_supported_coords(&xs, joints, tol, true, min_jpl, 0.40);
    let ys = filter_joint_supported_coords(&ys, joints, tol, false, min_jpl, 0.22);
    if xs.len() < 3 || ys.len() < 3 {
        return None;
    }

    // Drop thin gaps → dense retained line sets (renumbered).
    let mut xs = collapse_thin_gaps(&xs, min_cell);
    let mut y_ttb = collapse_thin_gaps(&ys, min_cell);
    y_ttb.sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));

    // False underlines / double rules: H anchors ~2× text bands → rebuild from text.
    let mut synthetic_h_ys: Vec<f32> = Vec::new();
    let mut text_row_recovery = false;
    if opts.lattice_collapse_overdense_h {
        if let Some((y_new, synth)) = collapse_overdense_h_from_text(
            &y_ttb,
            runs,
            xs[0],
            *xs.last().unwrap_or(&xs[0]),
            min_cell,
            opts.lattice_overdense_h_factor,
        ) {
            y_ttb = y_new;
            synthetic_h_ys = synth;
            text_row_recovery = true;
        }
    }

    // Sparse intermediate H rules (e.g. outer + every Nth body line) under-count
    // rows vs multi-column text bands. Densify Y anchors from text midpoints
    // *within* existing H gaps — never invent rows outside the ruled frame.
    if !text_row_recovery {
        let y_before_text = y_ttb.len();
        let (y_densified, synth) =
            densify_y_from_text_bands(&y_ttb, runs, xs[0], *xs.last().unwrap_or(&xs[0]), min_cell);
        y_ttb = y_densified;
        synthetic_h_ys = synth;
        if y_ttb.len() as u32 > opts.lattice_max_rows + 1 {
            // Too many inferred rows — fall back to pure H-line anchors.
            y_ttb = collapse_thin_gaps(&ys, min_cell);
            y_ttb.sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));
            synthetic_h_ys.clear();
        }
        text_row_recovery = y_ttb.len() > y_before_text;
    }

    let nrows = y_ttb.len().saturating_sub(1);
    let ncols = xs.len().saturating_sub(1);
    if nrows < 2 || ncols < 2 {
        return None;
    }
    if nrows as u32 > opts.lattice_max_rows || ncols as u32 > opts.lattice_max_cols {
        return None;
    }

    let mut h_local: Vec<HSeg> = h_idx.iter().map(|&i| h_segs[i]).collect();
    // Virtual H rules at text-inferred separators so rowspan merge does not
    // re-collapse densified rows, and edge completeness stays meaningful.
    if text_row_recovery {
        let x0 = xs[0];
        let x1 = *xs.last().unwrap_or(&x0);
        for &y in &synthetic_h_ys {
            h_local.push(HSeg { y, x0, x1 });
        }
    }
    let v_local: Vec<VSeg> = v_idx.iter().map(|&i| v_segs[i]).collect();
    let cover_frac = opts.lattice_edge_cover_frac;

    // Dense nrows×ncols cells (geometry first; text via exclusive assignment).
    let mut grid: Vec<Vec<RawCell>> = Vec::with_capacity(nrows);
    let mut edge_hits = 0u32;
    let mut edge_total = 0u32;
    let mut flat_bboxes: Vec<Rect> = Vec::with_capacity(nrows * ncols);

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
            flat_bboxes.push(bbox);
            row_cells.push(RawCell {
                bbox,
                text: String::new(),
                edges,
                active: true,
                colspan: 1,
                rowspan: 1,
            });
        }
        grid.push(row_cells);
    }

    // One run → one cell (avoids boundary double-counts that block colspan).
    let texts = assign_runs_exclusive(runs, &flat_bboxes);
    let mut filled = 0usize;
    for (i, text) in texts.into_iter().enumerate() {
        let r = i / ncols;
        let c = i % ncols;
        if !text.trim().is_empty() {
            filled += 1;
        }
        grid[r][c].text = text;
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

    // Re-bind runs onto active masters only (union bboxes after span growth).
    // Covered slots stay empty — ICDAR-style blanks under col/row spans.
    {
        let mut master_idx: Vec<(usize, usize)> = Vec::new();
        let mut master_boxes: Vec<Rect> = Vec::new();
        for (r, row) in grid.iter().enumerate() {
            for (c, cell) in row.iter().enumerate() {
                if cell.active {
                    master_idx.push((r, c));
                    master_boxes.push(cell.bbox);
                }
            }
        }
        let texts = assign_runs_exclusive(runs, &master_boxes);
        for ((r, c), text) in master_idx.into_iter().zip(texts) {
            grid[r][c].text = text;
        }
        for row in grid.iter_mut() {
            for cell in row.iter_mut() {
                if !cell.active {
                    cell.text.clear();
                }
            }
        }
    }

    // Dense emission: masters carry colspan/rowspan; covered slots stay empty strings
    // so structure matches ICDAR-style grids (text at top-left of span, blanks elsewhere).
    let (cells, max_row, max_col) = emit_cells_dense(&grid);
    if cells.is_empty() || max_row < 2 || max_col < 2 {
        return None;
    }

    // Tiny chrome (caption 2×2, empty form labels): not data tables.
    let filled_final = cells.iter().filter(|c| !c.text.trim().is_empty()).count();
    if max_row <= opts.lattice_min_side_for_tiny_reject
        && max_col <= opts.lattice_min_side_for_tiny_reject
        && filled_final <= opts.lattice_tiny_max_filled as usize
    {
        // Allow if cells carry substantial text (real tiny data table)
        let mean_chars = cells
            .iter()
            .filter(|c| !c.text.trim().is_empty())
            .map(|c| c.text.trim().chars().count())
            .sum::<usize>() as f32
            / filled_final.max(1) as f32;
        if mean_chars < 12.0 {
            return None;
        }
    }

    let bbox = bbox_of_cells(&cells);
    let area = bbox.width().max(0.0) * bbox.height().max(0.0);
    if area < opts.lattice_min_table_area {
        return None;
    }
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
    if text_row_recovery {
        notes.push(format!(
            "text_row_recovery synthetic_h={}",
            synthetic_h_ys.len()
        ));
    }
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

/// Keep clustered line coordinates that:
/// 1) participate in at least `min_joints` crossings, and
/// 2) those joints span at least `min_span_frac` of the full joint extent
///    along the orthogonal axis (drops short mid-table ticks).
///
/// Vertical lines use joint.x (span measured in y); horizontal use joint.y (span in x).
fn filter_joint_supported_coords(
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

/// When H rules are sparse relative to multi-column text bands, insert Y
/// separators at midpoints between consecutive text bands *inside* each H gap.
///
/// Returns densified top-to-bottom Y anchors and the Y coordinates of any
/// newly inserted (synthetic) separators.
///
/// Generic: no fixture IDs; only uses geometry of multi-col text bands within
/// the ruled frame. Full grids (one multi-col band per H gap) are unchanged.
// ─── Invoice footer / totals row post-process ────────────────────────────────

/// Strip trailing Subtotal/Tax/Total footer rows from an invoice line-item lattice.
///
/// Many invoices draw totals inside the same ruled grid as SKU lines. Lattice
/// geometry correctly finds the full grid; product/gold want the items body only
/// (header + line items). Only runs when the table looks like line items and the
/// trailing block has totals keywords — financial metric grids (Revenue/…) are
/// left intact.
fn strip_trailing_footer_totals(table: &mut Table) {
    let nrows = table.rows as usize;
    let ncols = table.cols as usize;
    if nrows < 3 || ncols < 2 || table.cells.is_empty() {
        return;
    }

    let mut grid: Vec<Vec<String>> = vec![vec![String::new(); ncols]; nrows];
    for c in &table.cells {
        let r = c.row as usize;
        let col = c.col as usize;
        if r < nrows && col < ncols {
            // Prefer first non-empty if duplicates (span placeholders).
            if grid[r][col].trim().is_empty() && !c.text.trim().is_empty() {
                grid[r][col] = c.text.clone();
            } else if grid[r][col].is_empty() {
                grid[r][col] = c.text.clone();
            }
        }
    }

    let header = &grid[0];
    let body = &grid[1..];
    if !looks_like_invoice_line_items(header, body) {
        return;
    }

    // Walk up from the bottom while rows look like totals footers.
    let mut cut = nrows;
    while cut > 1 {
        let r = cut - 1;
        if is_footer_totals_row(&grid[r]) {
            cut -= 1;
        } else {
            break;
        }
    }
    // Keep header + ≥1 body row; require at least one strip.
    if cut < 2 || cut >= nrows {
        return;
    }
    // Safety: stripped block must carry an explicit totals keyword.
    let stripped_has_kw = (cut..nrows).any(|r| row_has_totals_keyword(&grid[r]));
    if !stripped_has_kw {
        return;
    }

    let n_stripped = nrows - cut;
    table.cells.retain(|c| (c.row as usize) < cut);
    table.rows = cut as u32;
    if !table.cells.is_empty() {
        table.bbox = bbox_of_cells(&table.cells);
    }
    let filled = table
        .cells
        .iter()
        .filter(|c| !c.text.trim().is_empty())
        .count();
    let total = (table.rows as usize)
        .saturating_mul(table.cols as usize)
        .max(1);
    table.fill_rate = filled as f32 / total as f32;
    table.notes.push(format!("footer_totals_stripped n={n_stripped}"));
}

fn looks_like_invoice_line_items(header: &[String], body: &[Vec<String>]) -> bool {
    let mut hits = 0u32;
    for cell in header {
        let t = cell.trim().to_lowercase();
        if t.is_empty() {
            continue;
        }
        if matches!(
            t.as_str(),
            "sku"
                | "qty"
                | "quantity"
                | "description"
                | "unit"
                | "amount"
                | "price"
                | "item"
                | "total"
                | "line"
                | "#"
                | "no"
                | "no."
                | "part"
                | "code"
                | "product"
                | "desc"
                | "cost"
        ) || t == "unit price"
            || t == "line total"
            || t == "item #"
            || t == "part no"
            || t == "part no."
            || t.contains("sku")
            || t == "qty."
        {
            hits += 1;
        }
    }
    if hits >= 2 {
        return true;
    }
    if body.is_empty() {
        return false;
    }
    let skuish = body
        .iter()
        .filter(|r| {
            let c0 = r.first().map(|s| s.trim()).unwrap_or("");
            let c1 = r.get(1).map(|s| s.trim()).unwrap_or("");
            is_line_item_id(c0) || is_line_item_id(c1)
        })
        .count();
    skuish * 2 >= body.len()
}

fn is_line_item_id(s: &str) -> bool {
    if s.is_empty() || cell_is_totals_label(s) {
        return false;
    }
    if s.chars().all(|c| c.is_ascii_digit()) && s.len() <= 4 {
        return true;
    }
    let upper = s.to_ascii_uppercase();
    if upper.starts_with("SKU") {
        return true;
    }
    let has_digit = s.chars().any(|c| c.is_ascii_digit());
    let has_alpha = s.chars().any(|c| c.is_ascii_alphabetic());
    has_digit && has_alpha && s.len() <= 16
}

fn is_footer_totals_row(cells: &[String]) -> bool {
    let filled: Vec<&str> = cells
        .iter()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();
    if filled.is_empty() {
        return false;
    }
    if !row_has_totals_keyword(cells) {
        return false;
    }
    let n = cells.len().max(1);
    let sparse = filled.len() as f32 / n as f32 <= 0.55;
    let first = cells.first().map(|s| s.trim()).unwrap_or("");
    let first_empty = first.is_empty();
    let first_totals = cell_is_totals_label(first);
    let left_half = (n + 1) / 2;
    let left_empty = cells
        .iter()
        .take(left_half)
        .filter(|c| c.trim().is_empty())
        .count();
    let left_mostly_empty = left_empty as f32 / left_half.max(1) as f32 >= 0.5;
    sparse || first_empty || first_totals || left_mostly_empty
}

fn row_has_totals_keyword(cells: &[String]) -> bool {
    cells.iter().any(|c| cell_is_totals_label(c))
}

/// True when cell text is a totals/footer label (Subtotal, Tax, Amount Due, …).
fn cell_is_totals_label(s: &str) -> bool {
    let t = s
        .trim()
        .to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if t.is_empty() {
        return false;
    }
    const PHRASES: &[&str] = &[
        "subtotal",
        "sub total",
        "grand total",
        "amount due",
        "balance due",
        "total due",
        "net due",
        "sales tax",
        "amount payable",
        "balance payable",
        "total amount",
        "invoice total",
        "order total",
        "net total",
        "tax total",
    ];
    for p in PHRASES {
        if t.contains(p) {
            return true;
        }
    }
    // Tax as primary label — not "tax rate" notes on metric rows.
    if (t.starts_with("tax") && !t.starts_with("tax rate"))
        || t == "vat"
        || t.starts_with("vat ")
        || t == "gst"
        || t.starts_with("gst ")
    {
        return true;
    }
    // "Total" / "Total TOKEN_…" / short "… total" labels.
    if t == "total" || t.starts_with("total ") {
        return true;
    }
    if t.ends_with(" total") && t.len() < 40 {
        return true;
    }
    false
}

/// When H-line count is clearly over-dense vs multi-col text bands (false underlines),
/// rebuild row anchors from text band centers + outer frame.
fn collapse_overdense_h_from_text(
    y_ttb: &[f32],
    runs: &[TextRun],
    frame_x0: f32,
    frame_x1: f32,
    min_cell: f32,
    overdense_factor: f32,
) -> Option<(Vec<f32>, Vec<f32>)> {
    if y_ttb.len() < 4 {
        return None;
    }
    let bands = multi_col_band_centers(runs, frame_x0, frame_x1, y_ttb, min_cell);
    if bands.len() < 3 {
        return None;
    }
    let h_rows = y_ttb.len().saturating_sub(1);
    if (h_rows as f32) < bands.len() as f32 * overdense_factor.max(1.15) {
        return None;
    }
    // Outer frame from existing H extremes; separators between consecutive text bands.
    let y_top = y_ttb.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    let y_bot = y_ttb.iter().copied().fold(f32::INFINITY, f32::min);
    let mut anchors = vec![y_top];
    let mut synth = Vec::new();
    for w in bands.windows(2) {
        let mid = (w[0] + w[1]) * 0.5;
        if (anchors.last().copied().unwrap_or(y_top) - mid).abs() >= min_cell * 0.8 {
            anchors.push(mid);
            synth.push(mid);
        }
    }
    anchors.push(y_bot);
    anchors.sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));
    let collapsed = collapse_thin_gaps(&anchors, min_cell * 0.85);
    let mut collapsed = collapsed;
    collapsed.sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));
    if collapsed.len().saturating_sub(1) < 2 {
        return None;
    }
    // Must reduce row count toward text bands.
    if collapsed.len().saturating_sub(1) >= h_rows {
        return None;
    }
    Some((collapsed, synth))
}

/// Multi-col text band centers (top-to-bottom) inside frame defined by y_ttb extremes.
fn multi_col_band_centers(
    runs: &[TextRun],
    frame_x0: f32,
    frame_x1: f32,
    y_ttb: &[f32],
    min_cell: f32,
) -> Vec<f32> {
    let y_top = y_ttb.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    let y_bot = y_ttb.iter().copied().fold(f32::INFINITY, f32::min);
    if !y_top.is_finite() || !y_bot.is_finite() {
        return Vec::new();
    }
    let pad = 1.0f32;
    let inside: Vec<&TextRun> = runs
        .iter()
        .filter(|r| {
            if r.text.trim().is_empty() {
                return false;
            }
            let cx = (r.bbox.x0 + r.bbox.x1) * 0.5;
            let cy = r.bbox.y_center();
            cx >= frame_x0 - pad
                && cx <= frame_x1 + pad
                && cy <= y_top + pad
                && cy >= y_bot - pad
        })
        .collect();
    if inside.len() < 4 {
        return Vec::new();
    }
    let fs = {
        let mut v: Vec<f32> = inside
            .iter()
            .map(|r| r.font_size)
            .filter(|s| *s > 0.0)
            .collect();
        if v.is_empty() {
            10.0
        } else {
            v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            v[v.len() / 2]
        }
    };
    let y_tol = (0.45 * fs).max(2.0);
    let mut items = inside;
    items.sort_by(|a, b| {
        b.bbox
            .y_center()
            .partial_cmp(&a.bbox.y_center())
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| {
                a.bbox
                    .x0
                    .partial_cmp(&b.bbox.x0)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
    });
    let mut bands: Vec<Vec<&TextRun>> = Vec::new();
    for r in items {
        if let Some(band) = bands.last_mut() {
            let by = band.iter().map(|x| x.bbox.y_center()).sum::<f32>() / band.len() as f32;
            if (r.bbox.y_center() - by).abs() <= y_tol {
                band.push(r);
                continue;
            }
        }
        bands.push(vec![r]);
    }
    let mut centers: Vec<f32> = bands
        .iter()
        .filter(|b| b.len() >= 2)
        .map(|b| b.iter().map(|r| r.bbox.y_center()).sum::<f32>() / b.len() as f32)
        .filter(|&c| c < y_top - min_cell * 0.15 && c > y_bot + min_cell * 0.15)
        .collect();
    centers = cluster_coords(&centers, y_tol * 0.6);
    centers.sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));
    centers
}

/// Densify missing vertical anchors from multi-row text x-projections.
/// Kept for future partial-V work; not called in production path.
#[allow(dead_code)]
fn densify_x_from_text_cols(
    xs: &[f32],
    runs: &[TextRun],
    frame_y_top: f32,
    frame_y_bot: f32,
    min_cell: f32,
) -> (Vec<f32>, Vec<f32>) {
    if xs.len() < 2 {
        return (xs.to_vec(), Vec::new());
    }
    let x0 = xs[0];
    let x1 = *xs.last().unwrap_or(&x0);
    let y_hi = frame_y_top.max(frame_y_bot);
    let y_lo = frame_y_top.min(frame_y_bot);
    let pad = 1.0f32;
    let inside: Vec<&TextRun> = runs
        .iter()
        .filter(|r| {
            if r.text.trim().is_empty() {
                return false;
            }
            let cx = (r.bbox.x0 + r.bbox.x1) * 0.5;
            let cy = r.bbox.y_center();
            cx >= x0 - pad && cx <= x1 + pad && cy <= y_hi + pad && cy >= y_lo - pad
        })
        .collect();
    if inside.len() < 6 {
        return (xs.to_vec(), Vec::new());
    }
    let fs = {
        let mut v: Vec<f32> = inside
            .iter()
            .map(|r| r.font_size)
            .filter(|s| *s > 0.0)
            .collect();
        if v.is_empty() {
            10.0
        } else {
            v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            v[v.len() / 2]
        }
    };
    let x_tol = (0.55 * fs).max(3.0);
    // Cluster run left edges and centers; keep clusters that appear on many rows.
    let mut lefts: Vec<f32> = inside.iter().map(|r| r.bbox.x0).collect();
    lefts = cluster_coords(&lefts, x_tol);
    // Count multi-row support: for each candidate x, how many distinct y-bands touch it.
    let y_tol = (0.45 * fs).max(2.0);
    let mut band_ys: Vec<f32> = inside.iter().map(|r| r.bbox.y_center()).collect();
    band_ys = cluster_coords(&band_ys, y_tol);
    if band_ys.len() < 3 {
        return (xs.to_vec(), Vec::new());
    }
    let mut col_xs: Vec<f32> = Vec::new();
    for &cand in &lefts {
        if cand <= x0 + min_cell * 0.3 || cand >= x1 - min_cell * 0.3 {
            continue;
        }
        let mut rows_hit = 0u32;
        for &by in &band_ys {
            let hit = inside.iter().any(|r| {
                (r.bbox.y_center() - by).abs() <= y_tol && (r.bbox.x0 - cand).abs() <= x_tol
            });
            if hit {
                rows_hit += 1;
            }
        }
        if rows_hit as usize >= (band_ys.len() * 2 / 5).max(2) {
            col_xs.push(cand);
        }
    }
    if col_xs.len() < 2 {
        return (xs.to_vec(), Vec::new());
    }
    col_xs = cluster_coords(&col_xs, x_tol * 0.8);
    col_xs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    let v_cols = xs.len().saturating_sub(1);
    // Only densify for true partial-V undercount: text columns roughly 2× V gaps
    // (every-other V). Full-V tables often have multi-token cells whose left edges
    // look like "extra columns" — must not densify those (regression: painted 6×5).
    if v_cols < 3 || col_xs.len() < v_cols * 2 {
        return (xs.to_vec(), Vec::new());
    }

    let mut xs_sorted = xs.to_vec();
    xs_sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    // Count V gaps that contain ≥2 multi-row text columns (true missing-V signal).
    let mut multi_text_gaps = 0u32;
    let mut text_in_gaps = 0u32;
    for w in xs_sorted.windows(2) {
        let n = col_xs
            .iter()
            .filter(|&&c| c > w[0] + 0.5 && c < w[1] - 0.5)
            .count() as u32;
        text_in_gaps += n;
        if n >= 2 {
            multi_text_gaps += 1;
        }
    }
    // Majority of gaps under-ruled, and mean ≥1.8 text cols per gap.
    if multi_text_gaps * 2 < v_cols as u32 {
        return (xs.to_vec(), Vec::new());
    }
    if (text_in_gaps as f32) / (v_cols as f32) < 1.8 {
        return (xs.to_vec(), Vec::new());
    }

    // Build densified xs: keep outer V, insert midlines between adjacent text cols
    // only when a V gap contains ≥2 supported text column left-edges.
    let mut out = vec![xs[0]];
    let mut synthetic = Vec::new();
    for w in xs_sorted.windows(2) {
        let g0 = w[0];
        let g1 = w[1];
        let mut in_gap: Vec<f32> = col_xs
            .iter()
            .copied()
            .filter(|&c| c > g0 + 0.5 && c < g1 - 0.5)
            .collect();
        in_gap.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        // Never invent a column from a single text cluster inside a V gap.
        if in_gap.len() >= 2 {
            for pair in in_gap.windows(2) {
                let mid = (pair[0] + pair[1]) * 0.5;
                let prev = *out.last().unwrap();
                if (mid - prev).abs() >= min_cell && (g1 - mid).abs() >= min_cell {
                    out.push(mid);
                    synthetic.push(mid);
                }
            }
        }
        out.push(g1);
    }
    let mut collapsed = collapse_thin_gaps(&out, min_cell);
    collapsed.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let synthetic: Vec<f32> = collapsed
        .iter()
        .copied()
        .filter(|&x| !xs_sorted.iter().any(|&vx| (vx - x).abs() <= min_cell * 0.5))
        .collect();
    if collapsed.len() <= xs_sorted.len() {
        return (xs.to_vec(), Vec::new());
    }
    (collapsed, synthetic)
}

fn densify_y_from_text_bands(
    y_ttb: &[f32],
    runs: &[TextRun],
    frame_x0: f32,
    frame_x1: f32,
    min_cell: f32,
) -> (Vec<f32>, Vec<f32>) {
    if y_ttb.len() < 2 {
        return (y_ttb.to_vec(), Vec::new());
    }
    let y_top = y_ttb
        .iter()
        .copied()
        .fold(f32::NEG_INFINITY, f32::max);
    let y_bot = y_ttb.iter().copied().fold(f32::INFINITY, f32::min);
    if !y_top.is_finite() || !y_bot.is_finite() || y_top - y_bot < min_cell * 2.0 {
        return (y_ttb.to_vec(), Vec::new());
    }

    // Text strictly inside the ruled frame (pad slightly for centers on edges).
    let pad = 1.0f32;
    let inside: Vec<&TextRun> = runs
        .iter()
        .filter(|r| {
            if r.text.trim().is_empty() {
                return false;
            }
            let cx = (r.bbox.x0 + r.bbox.x1) * 0.5;
            let cy = r.bbox.y_center();
            cx >= frame_x0 - pad
                && cx <= frame_x1 + pad
                && cy <= y_top + pad
                && cy >= y_bot - pad
        })
        .collect();
    if inside.len() < 4 {
        return (y_ttb.to_vec(), Vec::new());
    }

    let fs = {
        let mut v: Vec<f32> = inside
            .iter()
            .map(|r| r.font_size)
            .filter(|s| *s > 0.0)
            .collect();
        if v.is_empty() {
            10.0
        } else {
            v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            v[v.len() / 2]
        }
    };
    let y_tol = (0.45 * fs).max(2.0);

    // Band by Y without requiring a contiguous TextRun slice.
    let mut items = inside;
    items.sort_by(|a, b| {
        b.bbox
            .y_center()
            .partial_cmp(&a.bbox.y_center())
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| {
                a.bbox
                    .x0
                    .partial_cmp(&b.bbox.x0)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
    });
    let mut bands: Vec<Vec<&TextRun>> = Vec::new();
    for r in items {
        if let Some(band) = bands.last_mut() {
            let by = band.iter().map(|x| x.bbox.y_center()).sum::<f32>() / band.len() as f32;
            if (r.bbox.y_center() - by).abs() <= y_tol {
                band.push(r);
                continue;
            }
        }
        bands.push(vec![r]);
    }

    // Multi-column bands only: single-run prose / rowspan labels do not invent rows.
    let mut band_centers: Vec<f32> = bands
        .iter()
        .filter(|b| b.len() >= 2)
        .map(|b| b.iter().map(|r| r.bbox.y_center()).sum::<f32>() / b.len() as f32)
        .filter(|&c| c < y_top - min_cell * 0.25 && c > y_bot + min_cell * 0.25)
        .collect();
    if band_centers.len() < 3 {
        return (y_ttb.to_vec(), Vec::new());
    }
    band_centers = cluster_coords(&band_centers, y_tol * 0.6);
    band_centers.sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));

    let h_rows = y_ttb.len().saturating_sub(1);
    // Only densify when text clearly implies more rows than H-line gaps.
    if band_centers.len() <= h_rows {
        return (y_ttb.to_vec(), Vec::new());
    }
    // Require at least two extra text bands vs H-derived rows (avoids tiny noise).
    if band_centers.len() < h_rows + 2 {
        return (y_ttb.to_vec(), Vec::new());
    }

    let mut out: Vec<f32> = Vec::with_capacity(band_centers.len() + y_ttb.len());
    let mut synthetic: Vec<f32> = Vec::new();
    out.push(y_ttb[0]);

    for w in y_ttb.windows(2) {
        let gap_top = w[0];
        let gap_bot = w[1];
        if gap_top <= gap_bot {
            out.push(gap_bot);
            continue;
        }
        // Multi-col band centers strictly inside this H gap.
        let mut in_gap: Vec<f32> = band_centers
            .iter()
            .copied()
            .filter(|&c| c < gap_top - 0.5 && c > gap_bot + 0.5)
            .collect();
        in_gap.sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));

        if in_gap.len() >= 2 {
            for pair in in_gap.windows(2) {
                let mid = (pair[0] + pair[1]) * 0.5;
                let prev = *out.last().unwrap();
                // Keep clear of existing anchors and the gap floor.
                if (prev - mid).abs() >= min_cell && (mid - gap_bot).abs() >= min_cell {
                    // Avoid near-duplicates of real H lines already in out / next.
                    if (mid - gap_top).abs() >= min_cell {
                        out.push(mid);
                        synthetic.push(mid);
                    }
                }
            }
        }
        out.push(gap_bot);
    }

    // Collapse any accidental thin pairs; re-sort top-to-bottom.
    let mut collapsed = collapse_thin_gaps(&out, min_cell);
    collapsed.sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));

    // Recompute synthetic as densified anchors not near original H lines.
    let synthetic: Vec<f32> = collapsed
        .iter()
        .copied()
        .filter(|&y| !y_ttb.iter().any(|&hy| (hy - y).abs() <= min_cell * 0.5))
        .collect();

    if collapsed.len() <= y_ttb.len() {
        return (y_ttb.to_vec(), Vec::new());
    }
    (collapsed, synthetic)
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

/// Span merge on a dense grid (Camelot-style missing-edge spans).
///
/// Horizontal (colspan):
/// - Shared V edge absent on both sides.
/// - Absorb empty neighbors only (never glue two non-empty side-by-side
///   cells — that would merge "FY24"+"Act" when a short header H is missed).
///
/// Vertical (rowspan):
/// - Shared H edge absent on both sides (geometry only).
/// - Allow absorbing non-empty below: multi-line category labels often place
///   "Fruit" and "TOKEN_…" on different y-bands of one visual rowspan.
/// - Text is cleared on absorbed cells; masters are re-filled from runs on the
///   union bbox after merge (see caller).
fn merge_spans_dense(grid: &mut [Vec<RawCell>]) {
    let nrows = grid.len();
    if nrows == 0 {
        return;
    }
    let ncols = grid[0].len();

    // Horizontal colspan: master | empty, missing V between them
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
                let right_empty = grid[r][c_end + 1].text.trim().is_empty();
                let left_empty = grid[r][c].text.trim().is_empty();
                // Absorb empty into filled, or empty into empty (grow placeholder)
                let can = !grid[r][c].edges.right
                    && !grid[r][c_end + 1].edges.left
                    && (right_empty || left_empty)
                    && !(!left_empty && !right_empty);
                if !can {
                    break;
                }
                // Prefer non-empty as master: if left empty and right has text, swap roles
                if left_empty && !right_empty {
                    // Move text to left master, keep right as covered empty
                    grid[r][c].text = std::mem::take(&mut grid[r][c_end + 1].text);
                    grid[r][c].edges.left = grid[r][c].edges.left || grid[r][c_end + 1].edges.left;
                }
                let right_bbox = grid[r][c_end + 1].bbox;
                let right_edge = grid[r][c_end + 1].edges.right;
                let add_span = grid[r][c_end + 1].colspan;
                grid[r][c].bbox = grid[r][c].bbox.union(right_bbox);
                grid[r][c].edges.right = right_edge;
                grid[r][c].colspan += add_span;
                grid[r][c_end + 1].active = false;
                grid[r][c_end + 1].text.clear();
                c_end += 1;
            }
            c = c_end + 1;
        }
    }

    // Vertical rowspan: missing shared H — geometry-driven (text reassigned later).
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
                if grid[r_end + 1][c].colspan != grid[r][c].colspan {
                    break;
                }
                let can = !grid[r][c].edges.bottom && !grid[r_end + 1][c].edges.top;
                if !can {
                    break;
                }
                // Drop bottom text into void; exclusive re-assign on union bbox
                // reconstructs "Fruit TOKEN_…" without stringly concat.
                let bot_bbox = grid[r_end + 1][c].bbox;
                let bot_edge = grid[r_end + 1][c].edges.bottom;
                let add_span = grid[r_end + 1][c].rowspan;
                grid[r][c].bbox = grid[r][c].bbox.union(bot_bbox);
                grid[r][c].edges.bottom = bot_edge;
                grid[r][c].rowspan += add_span;
                grid[r_end + 1][c].active = false;
                grid[r_end + 1][c].text.clear();
                r_end += 1;
            }
            r = r_end + 1;
        }
    }
}

/// Emit a full rectangular cell matrix: active masters keep text + spans;
/// covered (inactive) slots are empty 1×1 cells for structure/gold alignment.
fn emit_cells_dense(grid: &[Vec<RawCell>]) -> (Vec<TableCell>, u32, u32) {
    let nrows = grid.len() as u32;
    let ncols = grid.first().map(|r| r.len() as u32).unwrap_or(0);
    let mut out = Vec::new();
    // Mark coverage by masters
    let mut covered = vec![vec![false; ncols as usize]; nrows as usize];
    for (r, row) in grid.iter().enumerate() {
        for (c, cell) in row.iter().enumerate() {
            if !cell.active {
                continue;
            }
            let rs = cell.rowspan.max(1) as usize;
            let cs = cell.colspan.max(1) as usize;
            for rr in r..(r + rs).min(nrows as usize) {
                for cc in c..(c + cs).min(ncols as usize) {
                    if rr == r && cc == c {
                        continue;
                    }
                    covered[rr][cc] = true;
                }
            }
            out.push(TableCell {
                row: r as u32,
                col: c as u32,
                rowspan: cell.rowspan.max(1),
                colspan: cell.colspan.max(1),
                bbox: cell.bbox,
                text: cell.text.clone(),
                is_header: r == 0 || (r == 1 && cell.text.trim().is_empty() == false && r < 2),
                confidence: 0.9,
            });
        }
    }
    // Empty placeholders for covered positions (ICDAR-style blanks under spans)
    for r in 0..nrows as usize {
        for c in 0..ncols as usize {
            if covered[r][c] && !grid[r][c].active {
                out.push(TableCell {
                    row: r as u32,
                    col: c as u32,
                    rowspan: 1,
                    colspan: 1,
                    bbox: grid[r][c].bbox,
                    text: String::new(),
                    is_header: r == 0,
                    confidence: 0.85,
                });
            } else if !grid[r][c].active && !covered[r][c] {
                // Inactive but not marked covered — still emit empty for density
                out.push(TableCell {
                    row: r as u32,
                    col: c as u32,
                    rowspan: 1,
                    colspan: 1,
                    bbox: grid[r][c].bbox,
                    text: String::new(),
                    is_header: r == 0,
                    confidence: 0.8,
                });
            }
        }
    }
    out.sort_by(|a, b| a.row.cmp(&b.row).then(a.col.cmp(&b.col)));
    (out, nrows, ncols)
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

    #[test]
    fn joint_filter_drops_singleton_and_short_span() {
        // Full V lines at 0,50,100; singleton at 25; short mid-span at 75 (two joints only mid-y)
        let mut joints = vec![];
        for x in [0.0_f32, 50.0, 100.0] {
            for y in [0.0_f32, 50.0, 100.0] {
                joints.push((x, y));
            }
        }
        joints.push((25.0, 50.0)); // singleton
        joints.push((75.0, 40.0));
        joints.push((75.0, 60.0)); // short span vs global 100
        let coords = vec![0.0, 25.0, 50.0, 75.0, 100.0];
        let kept = filter_joint_supported_coords(&coords, &joints, 2.0, true, 2, 0.45);
        assert!(
            !kept.iter().any(|&x| (x - 25.0).abs() < 1.0),
            "singleton dropped: {kept:?}"
        );
        assert!(
            !kept.iter().any(|&x| (x - 75.0).abs() < 1.0),
            "short-span phantom dropped: {kept:?}"
        );
        assert_eq!(kept.len(), 3, "{kept:?}");
    }

    fn mk_run(x0: f32, y0: f32, text: &str) -> TextRun {
        TextRun {
            text: text.into(),
            bbox: Rect {
                x0,
                y0,
                x1: x0 + 20.0,
                y1: y0 + 8.0,
            },
            transform: pdfparser_ir::Matrix3x2::identity(),
            font_name: None,
            font_size: 8.0,
            mapping_confidence: 1.0,
            metrics_confidence: 1.0,
            mcid: None,
            invisible: false,
            from_actual_text: false,
        }
    }

    #[test]
    fn densify_y_subdivides_sparse_h_gaps() {
        // 5 H lines (4 gaps) but 12 multi-col text bands — classic partial body H.
        // Gaps of 3 bands each between H at 700, 652, 604, 556, 508 (every 48pt).
        let y_h = vec![700.0_f32, 652.0, 604.0, 556.0, 508.0];
        let mut runs = Vec::new();
        // 12 row centers from 692 down by 16
        for i in 0..12 {
            let y = 692.0 - 16.0 * i as f32;
            for (xi, label) in [(40.0, "A"), (90.0, "B"), (140.0, "C")].iter() {
                runs.push(mk_run(*xi, y - 4.0, label));
            }
        }
        let (densified, synth) = densify_y_from_text_bands(&y_h, &runs, 30.0, 180.0, 3.0);
        let nrows = densified.len().saturating_sub(1);
        assert_eq!(
            nrows, 12,
            "expected 12 rows from text densify, got {nrows} ys={densified:?} synth={synth:?}"
        );
        assert!(
            !synth.is_empty(),
            "expected synthetic H separators, got none"
        );
    }

    #[test]
    fn densify_y_noop_when_h_matches_text() {
        // One multi-col band per H gap → no densify.
        let y_h = vec![100.0_f32, 80.0, 60.0, 40.0];
        let mut runs = Vec::new();
        for y in [90.0_f32, 70.0, 50.0] {
            for xi in [10.0_f32, 50.0, 90.0] {
                runs.push(mk_run(xi, y - 4.0, "x"));
            }
        }
        let (densified, synth) = densify_y_from_text_bands(&y_h, &runs, 0.0, 120.0, 3.0);
        assert_eq!(densified.len(), y_h.len(), "ys={densified:?}");
        assert!(synth.is_empty());
    }
    #[test]
    fn strip_footer_totals_on_invoice_grid() {
        use crate::types::{Table, TableCell, TableMethod};
        use pdfparser_ir::Rect;
        let mut cells = Vec::new();
        let rows = [
            vec!["SKU", "Description", "Qty", "Unit", "Amount"],
            vec!["SKU-A", "Svc A", "1", "10", "10"],
            vec!["SKU-B", "Svc B", "2", "5", "10"],
            vec!["", "Subtotal", "", "", "20"],
            vec!["", "Total", "", "", "20"],
        ];
        for (r, row) in rows.iter().enumerate() {
            for (c, text) in row.iter().enumerate() {
                cells.push(TableCell {
                    row: r as u32, col: c as u32, rowspan: 1, colspan: 1,
                    bbox: Rect { x0: c as f32 * 20.0, y0: 100.0 - r as f32 * 10.0, x1: (c as f32 + 1.0) * 20.0, y1: 110.0 - r as f32 * 10.0 },
                    text: (*text).into(), is_header: r == 0, confidence: 1.0,
                });
            }
        }
        let mut table = Table {
            bbox: Rect { x0: 0.0, y0: 50.0, x1: 100.0, y1: 120.0 },
            page: 0, method: TableMethod::Lattice, confidence: 1.0,
            rows: 5, cols: 5, cells, header_rows: 1,
            continued_from_previous_page: false, continued_to_next_page: false,
            logical_table_id: None, strategy_provenance: vec![], notes: vec![],
            edge_score: 1.0, fill_rate: 0.8, weak_edges: false,
        };
        strip_trailing_footer_totals(&mut table);
        assert_eq!(table.rows, 3, "stripped totals rows");
        assert!(table.notes.iter().any(|n| n.contains("footer_totals_stripped")));
    }

    #[test]
    fn detect_lattice_full_stroke_grid() {
        use crate::options::{TableOptions, TablePreset};
        use pdfparser_content::RuleSegment;
        let mut rules = Vec::new();
        for y in [0.0_f32, 40.0, 80.0, 120.0] {
            rules.push(RuleSegment {
                x0: 0.0,
                y0: y,
                x1: 100.0,
                y1: y,
            });
        }
        for x in [0.0_f32, 50.0, 100.0] {
            rules.push(RuleSegment {
                x0: x,
                y0: 0.0,
                x1: x,
                y1: 120.0,
            });
        }
        let mut runs = Vec::new();
        let labels = [["A", "B"], ["C", "D"], ["E", "F"]];
        for (r, row) in labels.iter().enumerate() {
            for (c, lab) in row.iter().enumerate() {
                let x0 = 5.0 + c as f32 * 50.0;
                let y0 = 90.0 - r as f32 * 40.0;
                runs.push(mk_run(x0, y0, lab));
            }
        }
        let opts = TableOptions::from_preset(TablePreset::Full);
        let tabs = detect_lattice_tables(0, &runs, &rules, &opts);
        assert!(!tabs.is_empty(), "expected lattice table");
        assert!(tabs[0].rows >= 2 && tabs[0].cols >= 2);
    }

}
