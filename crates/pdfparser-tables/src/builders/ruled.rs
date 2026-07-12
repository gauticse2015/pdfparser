//! Ruled table builder (S2 lattice): multi-region ruled grids (CC of crossing lines) + R9 text assign.
//!
//! Legacy-parity extract core for Engine V2 `RuledTableBuilder` (PR4a).
//! Public production entry remains [`crate::detect_lattice_tables`] via thin re-export.
//!
//! - Connected components of H∩V segments → one table per grid region
//! - Collinear coalesce + single joint-gap model (from TableOptions)
//! - Anchors from joints + line coordinates only (no orthogonal endpoint injection)
//! - Dense grid after dropping thin gaps; edge-measured confidence; typed weak_edges
use crate::geom::{assign_runs_exclusive, bbox_of_cells, cluster_coords, grid_regularity_score};
use crate::options::TableOptions;
use crate::types::{PipelineId, Table, TableCell, TableMethod};
use pdfparser_content::RuleSegment;
use pdfparser_ir::{Rect, TextRun};

use super::densify::{
    collapse_overdense_h_from_text, collapse_sparse_interior_columns, collapse_thin_gaps,
    densify_x_from_text_cols, densify_y_from_text_bands, expand_xs_exterior_text_cols,
};

/// Detect ruled (lattice) tables on a page (may emit multiple).
///
/// Same behavior as the historical `detect_lattice_tables` entry point.
pub fn detect_ruled_tables(
    page_index: u32,
    runs: &[TextRun],
    rules: &[RuleSegment],
    opts: &TableOptions,
    raster_pages: &[crate::RasterPage],
) -> Vec<Table> {
    let tol = opts.line_snap_tol;
    let min_cell = opts.min_cell_size;
    let min_seg = opts.lattice_min_seg_len;
    let joint_gap = opts.lattice_joint_gap;
    let min_joints = opts.lattice_min_joints.max(1) as usize;

    // Count axis-aligned vector rules after min_seg (ignore short junk).
    let vector_hv_count = rules
        .iter()
        .filter(|r| r.len() >= min_seg && (r.is_horizontal(tol) || r.is_vertical(tol)))
        .count();

    // Merge raster-derived rules when enabled (image-painted / scanned grids).
    // Production morph already applies joint-graph + regularity gates so charts
    // and deco images do not inject phantom lattices.
    let mut rule_buf: Vec<RuleSegment> = rules.to_vec();
    let mut used_raster = false;
    if opts.raster_line_detect && !raster_pages.is_empty() {
        // K28: stamp existing vector rules into raster ink before morph (combined).
        use crate::raster::{config_for_raster_page, merge_rules, rules_from_raster_combined};
        let mut raster_rules = Vec::new();
        for rp in raster_pages {
            // Skip tiny icons / logos — not table images.
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
            // Combined path: vector stamp ∪ morph (PR4c production wire).
            // Contour seeds for router region ownership are built in the
            // orchestrator finalize path — do not recompute them here.
            raster_rules.extend(rules_from_raster_combined(rp, rules, &cfg));
        }
        if !raster_rules.is_empty() {
            used_raster = true;
            rule_buf = merge_rules(&rule_buf, &raster_rules, tol.max(1.0));
        }
    }
    // K29: drop H rules that track many text baselines (full-page / raster false underlines).
    if used_raster && !runs.is_empty() {
        rule_buf = suppress_text_baseline_h_rules(&rule_buf, runs, tol);
    }
    let rules = rule_buf.as_slice();
    // Pure image-table pages (few axis-aligned vector rules) may keep empty cells.
    // Mixed pages with a real vector lattice keep normal fill gates.
    let raster_primary = used_raster && vector_hv_count < 4;

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
        if let Some(mut t) = table_from_component(
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
            used_raster,
            raster_primary,
        ) {
            if used_raster && (t.fill_rate < 0.10 || raster_primary) {
                t.strategy_provenance.push(PipelineId::S6RasterLines);
                t.notes.push("raster_lines".into());
            }
            tables.push(t);
        }
    }

    // Global snap only when we did not already see multiple joint-rich components.
    // Multi-CC failure must not re-fuse into a page-wide mega-grid.
    if tables.is_empty() && !multi_component {
        if let Some(mut t) = table_from_global_snap(
            page_index,
            runs,
            &h_segs,
            &v_segs,
            opts,
            min_cell,
            tol,
            used_raster,
            raster_primary,
        ) {
            if used_raster && (t.fill_rate < 0.10 || raster_primary) {
                t.strategy_provenance.push(PipelineId::S6RasterLines);
                t.notes.push("raster_lines".into());
            }
            tables.push(t);
        }
    }

    for t in &mut tables {
        if !used_raster || t.fill_rate > 0.05 {
            strip_trailing_footer_totals(t);
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

// ─── K29 raster false-underline suppress ────────────────────────────────────

/// Drop horizontal rules that sit under many text baselines (underline deco).
///
/// Applied only after raster/combined sensing so clean vector lattices are untouched.
fn suppress_text_baseline_h_rules(
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
                let bx0 = band
                    .iter()
                    .map(|t| t.bbox.x0)
                    .fold(f32::INFINITY, f32::min);
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
struct HSeg {
    y: f32,
    x0: f32,
    x1: f32,
}

#[derive(Clone, Copy, Debug)]
struct VSeg {
    x: f32,
    y0: f32,
    y1: f32,
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
    used_raster: bool,
    raster_primary: bool,
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

    xs = cluster_coords(&xs, tol);
    ys = cluster_coords(&ys, tol);
    if xs.len() < 3 || ys.len() < 3 {
        return None;
    }

    // Vertical lines (columns): strict joint count + span — drops short phantom ticks.
    // Horizontal lines (rows): joint count only (or looser span) — multi-level headers often
    // have short H rules only under sub-columns (Act/Bud), which must be kept for structure.
    let min_jpl = opts.lattice_min_joints_per_line.max(1) as usize;
    // Raster lines often have incomplete joint spans at image edges — use looser span.
    let (v_span, h_span) = if used_raster {
        (0.15, 0.10)
    } else {
        (0.40, 0.22)
    };
    xs = filter_joint_supported_coords(&xs, joints, tol, true, min_jpl, v_span);
    ys = filter_joint_supported_coords(&ys, joints, tol, false, min_jpl, h_span);
    // Recover long H rules that joint-span filter dropped (partial joints on
    // dashed/short-tick corners). Only when joint-filtered H is clearly
    // under-dense vs physical long H segments (ratio ≥ 1.5×) — avoids
    // re-introducing double-rules on already-dense grids.
    {
        let x_lo = xs.iter().copied().fold(f32::INFINITY, f32::min);
        let x_hi = xs.iter().copied().fold(f32::NEG_INFINITY, f32::max);
        let width = (x_hi - x_lo).abs().max(1.0);
        let long_h: Vec<f32> = h_idx
            .iter()
            .map(|&i| h_segs[i])
            .filter(|h| (h.x1 - h.x0).abs() >= width * 0.55)
            .map(|h| h.y)
            .collect();
        let long_clustered = cluster_coords(&long_h, tol);
        if long_clustered.len() as f32 >= ys.len() as f32 * 1.5 && long_clustered.len() > ys.len()
        {
            let mut merged = ys.clone();
            merged.extend(long_clustered);
            ys = cluster_coords(&merged, tol);
        }
    }
    if xs.len() < 3 || ys.len() < 3 {
        return None;
    }

    // Drop thin gaps → dense retained line sets (renumbered).
    xs = collapse_thin_gaps(&xs, min_cell);
    let mut y_ttb = collapse_thin_gaps(&ys, min_cell);
    y_ttb.sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));

    // Ruled anchors *before* text densify — joint density / conf use these so
    // synthetic lines do not understate structure quality.
    let xs_ruled = xs.clone();
    let ys_ruled = y_ttb.clone();

    // Sparse intermediate V rules (full H, V every Nth column) under-count
    // columns vs multi-row text left-edges. Densify X after joint filter +
    // thin-gap collapse, before building the cell grid.
    let mut synthetic_v_xs: Vec<f32> = Vec::new();
    let mut text_col_recovery = false;
    let mut synthetic_h_ys: Vec<f32> = Vec::new();
    let mut text_row_recovery = false;
    if opts.lattice_text_densify {
        let y_hi = y_ttb.iter().copied().fold(f32::NEG_INFINITY, f32::max);
        let y_lo = y_ttb.iter().copied().fold(f32::INFINITY, f32::min);
        // Stub/line-number columns often sit left of the ruled number grid
        // (BEA NIPA/GDP tables). Expand frame X before densify.
        let xs_exp = expand_xs_exterior_text_cols(&xs, runs, y_hi, y_lo, min_cell);
        if xs_exp.len() > xs.len() {
            xs = xs_exp;
            text_col_recovery = true;
        }
        let (x_densified, synth) = densify_x_from_text_cols(&xs, runs, y_hi, y_lo, min_cell);
        if x_densified.len() as u32 <= opts.lattice_max_cols + 1 && x_densified.len() > xs.len() {
            xs = x_densified;
            synthetic_v_xs = synth;
            text_col_recovery = true;
        }

        // False underlines / double rules: H anchors ≫ multi-col text bands → rebuild.
        // Never skip densify after this: a false overdense collapse left under-rowed
        // grids permanently stuck when densify was gated off.
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

        // Sparse intermediate H rules under-count rows vs text bands (multi-col or
        // regular single-run body). Always attempt densify when under-dense.
        {
            let y_before = y_ttb.clone();
            let (y_densified, synth) = densify_y_from_text_bands(
                &y_ttb,
                runs,
                xs[0],
                *xs.last().unwrap_or(&xs[0]),
                min_cell,
            );
            if y_densified.len() as u32 > opts.lattice_max_rows + 1 {
                // Too many inferred rows — keep pre-densify anchors.
                y_ttb = y_before;
            } else if y_densified.len() > y_before.len() {
                y_ttb = y_densified;
                // Prefer densify synth when it grew the grid (may replace overdense synth).
                synthetic_h_ys = synth;
                text_row_recovery = true;
            }
        }
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
    let mut v_local: Vec<VSeg> = v_idx.iter().map(|&i| v_segs[i]).collect();
    // Virtual V rules at text-inferred column separators (partial-V densify).
    if text_col_recovery {
        let y_top = y_ttb.iter().copied().fold(f32::NEG_INFINITY, f32::max);
        let y_bot = y_ttb.iter().copied().fold(f32::INFINITY, f32::min);
        for &x in &synthetic_v_xs {
            v_local.push(VSeg {
                x,
                y0: y_bot,
                y1: y_top,
            });
        }
    }
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
    // Wide multi-col runs are split in assign_runs_exclusive when bboxes span.
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
    // When PDF stores a whole data line as one string with a narrow bbox,
    // exclusive assign dumps all tokens into one cell. Redistribute
    // multi-token cells across empty same-row neighbors (tabular pattern).
    redistribute_row_tokens(&mut grid);

    let total = (nrows * ncols).max(1);
    let fill_rate = filled as f32 / total as f32;
    // Empty cells only for pure image-table pages (raster_primary): text is ink.
    // On mixed pages, decorative image grids must pass normal fill gates.
    let allow_empty = raster_primary
        && opts.raster_allow_empty_cells
        && nrows >= 2
        && ncols >= 2
        && joints.len() >= 4;
    if !allow_empty {
        if fill_rate < opts.lattice_min_fill_rate && filled < 2 {
            return None;
        }
        let empty_frac = 1.0 - fill_rate;
        if empty_frac >= opts.lattice_empty_frac_reject
            && filled < opts.lattice_min_filled_cells as usize
        {
            return None;
        }
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
    // Drop nearly empty interior columns (BEA GDP exterior densify can invent gutters).
    // Never collapse pure image lattices: all cells are text-empty so every interior
    // column would look "empty" and shred the ruled schema to 2 columns.
    let (cells, max_row, max_col) = if raster_primary && opts.raster_allow_empty_cells {
        (cells, max_row, max_col)
    } else {
        collapse_sparse_interior_columns(cells, max_row, max_col)
    };
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
    // Regularity / joint density vs *ruled* anchors (pre-densify) so synthetic
    // text-inferred lines do not understate structure quality.
    let grid_regularity = grid_regularity_score(&xs_ruled, &ys_ruled);
    let edge_score = if edge_total == 0 {
        0.0
    } else {
        edge_hits as f32 / edge_total as f32
    };
    let expected_joints = (xs_ruled.len() * ys_ruled.len()) as f32;
    let joint_density = if expected_joints < 1.0 {
        0.0
    } else {
        (joints.len() as f32 / expected_joints).min(1.0)
    };

    // Structure-only (empty) tables: weight edges/joints/regularity higher than fill.
    let conf = if fill_rate < 0.05 && (used_raster || raster_primary) {
        (0.40 * grid_regularity
            + 0.35 * edge_score
            + 0.15 * joint_density
            + 0.10 * (cells.len() as f32 / 6.0).min(1.0))
        .clamp(0.0, 1.0)
    } else {
        (0.30 * grid_regularity
            + 0.25 * edge_score
            + 0.20 * fill_rate
            + 0.15 * joint_density
            + 0.10 * (cells.len() as f32 / 6.0).min(1.0))
        .clamp(0.0, 1.0)
    };

    // Empty raster lattices: require non-weak edges and some joint density.
    if used_raster && fill_rate < 0.05 {
        if edge_score < opts.lattice_weak_edge_threshold || joint_density < 0.25 {
            return None;
        }
    }

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
    if text_col_recovery {
        notes.push(format!(
            "text_col_recovery synthetic_v={}",
            synthetic_v_xs.len()
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
        joint_count: joints.len() as u32,
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

/// Redistribute multi-token text across empty same-row cells.
///
/// Fires when a row has exactly one (or few) non-empty cells whose whitespace
/// token count matches the empty span width — classic TJ-string whole-row dump.
fn redistribute_row_tokens(grid: &mut [Vec<RawCell>]) {
    for row in grid.iter_mut() {
        let ncols = row.len();
        if ncols < 2 {
            continue;
        }
        let nonempty: Vec<usize> = row
            .iter()
            .enumerate()
            .filter(|(_, c)| c.active && !c.text.trim().is_empty())
            .map(|(i, _)| i)
            .collect();
        // One multi-token cell + rest empty, or a dense block of empty cols.
        if nonempty.len() != 1 {
            continue;
        }
        let src = nonempty[0];
        let tokens: Vec<String> = row[src]
            .text
            .split_whitespace()
            .filter(|t| !t.is_empty())
            .map(|s| s.to_string())
            .collect();
        // Full-row tabular dumps only: token count ≈ column count (not 2-token
        // headers like "FY24 LABEL" that must stay in one cell for colspan).
        if tokens.len() < ncols.saturating_sub(1) || tokens.len() > ncols {
            continue;
        }
        // Only redistribute dense data rows (majority digit-bearing tokens).
        let data_like = tokens
            .iter()
            .filter(|t| t.chars().any(|c| c.is_ascii_digit()))
            .count();
        if data_like * 2 < tokens.len() {
            continue;
        }
        // Need enough empty slots (including src) for all tokens.
        let empty_n = row.iter().filter(|c| c.text.trim().is_empty()).count();
        if empty_n + 1 < tokens.len() {
            continue;
        }
        // Align tokens to columns: if token count == ncols, 1:1 left-to-right.
        // Else place tokens into the contiguous empty region centered on src.
        let (start, end) = if tokens.len() == ncols {
            (0, ncols)
        } else {
            // Expand from src to cover tokens.len() columns without leaving the row.
            let need = tokens.len();
            let mut s = src.saturating_sub(need.saturating_sub(1) / 2);
            if s + need > ncols {
                s = ncols - need;
            }
            (s, s + need)
        };
        // Only redistribute if target columns (except src) are empty.
        if !(start..end).all(|c| c == src || row[c].text.trim().is_empty()) {
            continue;
        }
        row[src].text.clear();
        for (i, tok) in tokens.into_iter().enumerate() {
            let c = start + i;
            if c < ncols {
                row[c].text = tok;
            }
        }
    }
}

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
    table
        .notes
        .push(format!("footer_totals_stripped n={n_stripped}"));
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

fn edge_flags(bbox: Rect, h_segs: &[HSeg], v_segs: &[VSeg], tol: f32, cover_frac: f32) -> Edges {
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

/// Drop interior columns that are almost entirely empty (densify / exterior-stub
/// artifacts). Keeps first and last column always; requires ≥4 columns.
///
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
    used_raster: bool,
    raster_primary: bool,
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
        used_raster,
        raster_primary,
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
    fn expand_xs_adds_left_stub_column() {
        // Ruled number grid at x=200..600; line numbers at x=50 and labels at x=80
        // aligned across many rows (BEA-style exterior stub).
        let xs_v = vec![200.0_f32, 300.0, 400.0, 500.0, 600.0];
        let mut runs = Vec::new();
        for row in 0..12 {
            let y = 400.0 - 14.0 * row as f32;
            runs.push(mk_run(50.0, y - 4.0, &format!("{row}")));
            runs.push(mk_run(80.0, y - 4.0, "label"));
            for k in 0..4 {
                runs.push(mk_run(210.0 + 100.0 * k as f32, y - 4.0, "1.0"));
            }
        }
        let expanded = expand_xs_exterior_text_cols(&xs_v, &runs, 410.0, 200.0, 3.0);
        assert!(
            expanded.len() > xs_v.len(),
            "expected left exterior expansion, got {expanded:?}"
        );
        assert!(
            expanded[0] < 200.0,
            "outer left should be left of frame: {expanded:?}"
        );
        let ncols = expanded.len().saturating_sub(1);
        assert!(ncols >= 6, "line+label+4 data → ≥6 cols, got {ncols} xs={expanded:?}");
    }

    #[test]
    fn densify_x_subdivides_every_other_v() {
        // Full H implied by multi-row text; V only every other column (step-2).
        // True 10 cols at pitch 40: V at 0,80,160,240,320,400 (6 lines → 5 gaps).
        // Text left-edges at 2 + 40*k for k=0..10 across many rows.
        let xs_v = vec![0.0_f32, 80.0, 160.0, 240.0, 320.0, 400.0];
        let mut runs = Vec::new();
        for row in 0..12 {
            let y = 200.0 - 14.0 * row as f32;
            for k in 0..10 {
                let x = 2.0 + 40.0 * k as f32;
                runs.push(mk_run(x, y - 4.0, "c"));
            }
        }
        let (densified, synth) = densify_x_from_text_cols(&xs_v, &runs, 210.0, 20.0, 3.0);
        let ncols = densified.len().saturating_sub(1);
        assert_eq!(
            ncols, 10,
            "expected 10 cols from partial-V densify, got {ncols} xs={densified:?} synth={synth:?}"
        );
        assert!(
            !synth.is_empty(),
            "expected synthetic V separators, got none"
        );
    }

    #[test]
    fn densify_y_includes_sparse_single_cell_rows() {
        // Partial H every 5 body lines; most rows multi-col but a few key-only.
        // H at 700, 640, 580 (outer + mid) → 2 large gaps holding 5 rows each.
        let y_h = vec![700.0_f32, 640.0, 580.0];
        let mut runs = Vec::new();
        // 10 body rows, centers 694, 682, … 586 (step 12).
        for i in 0..10 {
            let y = 694.0 - 12.0 * i as f32;
            // Key column always present.
            runs.push(mk_run(40.0, y - 4.0, &format!("R{i:02}")));
            // Sparse multi-col: skip i=2 and i=7 (single-cell only).
            if i != 2 && i != 7 {
                runs.push(mk_run(100.0, y - 4.0, "v"));
                if i % 3 == 0 {
                    runs.push(mk_run(160.0, y - 4.0, "w"));
                }
            }
        }
        let (densified, synth) = densify_y_from_text_bands(&y_h, &runs, 30.0, 200.0, 3.0);
        let nrows = densified.len().saturating_sub(1);
        assert_eq!(
            nrows, 10,
            "sparse single-cell rows must densify, got {nrows} ys={densified:?} synth={synth:?}"
        );
        assert!(
            !synth.is_empty(),
            "expected synthetic H separators, got none"
        );
    }

    #[test]
    fn densify_x_noop_when_full_v_matches_text() {
        // Full V with multi-token cells: primary + second word left-edges that
        // *do* align across rows (SKU + short label) but cluster near the cell
        // left — span ≪ gap, so must not densify (painted/SKU regression).
        let xs_v = vec![0.0_f32, 50.0, 100.0, 150.0, 200.0];
        let mut runs = Vec::new();
        for row in 0..8 {
            let y = 160.0 - 16.0 * row as f32;
            for x in [5.0_f32, 55.0, 105.0, 155.0] {
                runs.push(mk_run(x, y - 4.0, "sku"));
                // Second token ~14pt into the cell (aligned, multi-row support).
                runs.push(mk_run(x + 14.0, y - 4.0, "desc"));
            }
        }
        let (densified, synth) = densify_x_from_text_cols(&xs_v, &runs, 170.0, 20.0, 3.0);
        assert_eq!(
            densified.len(),
            xs_v.len(),
            "full-V multi-token must not densify: xs={densified:?}"
        );
        assert!(synth.is_empty());
    }

    #[test]
    fn densify_y_dense_every_row_h_no_over_split() {
        // Full H (every row ruled) with multi-col text — must not invent extra rows.
        let y_h: Vec<f32> = (0..6).map(|i| 100.0 - 12.0 * i as f32).collect();
        let mut runs = Vec::new();
        for i in 0..5 {
            let y = 94.0 - 12.0 * i as f32;
            for xi in [10.0_f32, 50.0, 90.0] {
                runs.push(mk_run(xi, y - 4.0, "x"));
            }
        }
        let (densified, synth) = densify_y_from_text_bands(&y_h, &runs, 0.0, 120.0, 3.0);
        assert_eq!(
            densified.len(),
            y_h.len(),
            "every-row H must not over-split ys={densified:?}"
        );
        assert!(synth.is_empty(), "synth={synth:?}");
    }

    #[test]
    fn densify_y_rejects_single_col_prose_stack() {
        // Ruled frame with multi-col header only + long single-col body → no densify
        // from prose lines (multi not majority of bands).
        let y_h = vec![200.0_f32, 100.0];
        let mut runs = Vec::new();
        // One multi-col header band.
        for xi in [20.0_f32, 80.0, 140.0] {
            runs.push(mk_run(xi, 190.0, "H"));
        }
        // 8 single-col prose lines inside the gap.
        for i in 0..8 {
            let y = 175.0 - 8.0 * i as f32;
            runs.push(mk_run(20.0, y, "prose"));
        }
        let (densified, synth) = densify_y_from_text_bands(&y_h, &runs, 10.0, 180.0, 3.0);
        assert_eq!(
            densified.len(),
            y_h.len(),
            "single-col prose must not densify ys={densified:?} synth={synth:?}"
        );
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
                    row: r as u32,
                    col: c as u32,
                    rowspan: 1,
                    colspan: 1,
                    bbox: Rect {
                        x0: c as f32 * 20.0,
                        y0: 100.0 - r as f32 * 10.0,
                        x1: (c as f32 + 1.0) * 20.0,
                        y1: 110.0 - r as f32 * 10.0,
                    },
                    text: (*text).into(),
                    is_header: r == 0,
                    confidence: 1.0,
                });
            }
        }
        let mut table = Table {
            bbox: Rect {
                x0: 0.0,
                y0: 50.0,
                x1: 100.0,
                y1: 120.0,
            },
            page: 0,
            method: TableMethod::Lattice,
            confidence: 1.0,
            rows: 5,
            cols: 5,
            cells,
            header_rows: 1,
            continued_from_previous_page: false,
            continued_to_next_page: false,
            logical_table_id: None,
            strategy_provenance: vec![],
            notes: vec![],
            edge_score: 1.0,
            fill_rate: 0.8,
            weak_edges: false,
        joint_count: 0,
        };
        strip_trailing_footer_totals(&mut table);
        assert_eq!(table.rows, 3, "stripped totals rows");
        assert!(table
            .notes
            .iter()
            .any(|n| n.contains("footer_totals_stripped")));
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
        let tabs = detect_ruled_tables(0, &runs, &rules, &opts, &[]);
        assert!(!tabs.is_empty(), "expected lattice table");
        assert!(tabs[0].rows >= 2 && tabs[0].cols >= 2);
    }
}
