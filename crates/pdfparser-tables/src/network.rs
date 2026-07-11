//! Network-class borderless tables (textline + column alignments).
//!
//! Production borderless path:
//! 1. Build textlines (baseline bands)
//! 2. Keep multi-column lines only for structure
//! 3. Region split: hard gap (3× soft) always splits; soft gap splits when
//!    neighboring column schemas differ (equal count + left-edge match)
//! 4. Re-merge adjacent same-schema regions; bridge short note islands
//! 5. Per-region column anchors (support-filtered left edges)
//! 6. One row per multi-col textline (drop non-grid note lines)

use crate::geom::{bbox_of_cells, cluster_coords, median_font_size};
use crate::options::TableOptions;
use crate::types::{PipelineId, Table, TableCell, TableMethod};
use pdfparser_ir::{Rect, TextRun};

/// Detect borderless tables via textline network structure.
pub fn detect_network_tables(page_index: u32, runs: &[TextRun], opts: &TableOptions) -> Vec<Table> {
    if runs.len() < 6 {
        return Vec::new();
    }
    let fs_all = median_font_size(runs);
    let body: Vec<&TextRun> = runs
        .iter()
        .filter(|r| !r.text.trim().is_empty() && r.font_size <= fs_all * 1.35 + 0.5)
        .collect();
    if body.len() < 6 {
        return Vec::new();
    }

    let fs = {
        let mut v: Vec<f32> = body.iter().map(|r| r.font_size).filter(|s| *s > 0.0).collect();
        if v.is_empty() {
            10.0
        } else {
            v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            v[v.len() / 2]
        }
    };
    let y_tol = (0.50 * fs).max(2.5);
    let lines = build_textlines(&body, y_tol);
    let multi: Vec<&TextLine> = lines.iter().filter(|l| l.multi).collect();
    if multi.len() < opts.stream_min_body_bands.max(3) as usize {
        return Vec::new();
    }

    // Soft gap: section notes / short prose between same-schema bands.
    // Hard gap: 3× soft (font-scaled) — always separate regions this far apart.
    let soft_gap = (opts.stream_region_gap_font_mult * fs).max(opts.stream_region_gap_min);
    let hard_gap = soft_gap * 3.0;
    // Split on schema/gap first (page-global filter *before* split collapses
    // multi-table pages into one skeleton). Section notes are dropped per-region
    // inside build_table_from_lines.
    let raw = split_multi_regions(&multi, soft_gap, hard_gap, fs);
    let regions = merge_same_schema_regions(raw, fs, hard_gap);
    let min_multi = opts.stream_min_body_bands.max(3) as usize;
    let mut out = Vec::new();
    for region in regions {
        if region.len() < min_multi {
            continue;
        }
        if let Some(t) = build_table_from_lines(page_index, &region, opts, fs) {
            out.push(t);
        }
    }
    out
}

struct TextLine {
    y: f32,
    runs: Vec<TextRun>,
    multi: bool,
}

fn build_textlines(body: &[&TextRun], y_tol: f32) -> Vec<TextLine> {
    let mut items: Vec<&TextRun> = body.to_vec();
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
    let mut lines: Vec<TextLine> = Vec::new();
    for r in items {
        if let Some(line) = lines.last_mut() {
            if (r.bbox.y_center() - line.y).abs() <= y_tol {
                line.runs.push((*r).clone());
                line.y = line.runs.iter().map(|x| x.bbox.y_center()).sum::<f32>()
                    / line.runs.len() as f32;
                line.multi = line.runs.len() >= 2;
                continue;
            }
        }
        lines.push(TextLine {
            y: r.bbox.y_center(),
            runs: vec![(*r).clone()],
            multi: false,
        });
    }
    for line in &mut lines {
        line.runs
            .sort_by(|a, b| a.bbox.x0.partial_cmp(&b.bbox.x0).unwrap_or(std::cmp::Ordering::Equal));
        line.multi = line.runs.len() >= 2;
    }
    lines
}

/// Cluster left-edge tolerance (~¾ em, floor at min cell-ish scale).
fn left_cluster_tol(fs: f32) -> f32 {
    (0.75 * fs).max(3.0)
}

/// `multi` ordered top→bottom.
///
/// - gap ≤ soft: always keep
/// - soft < gap < hard: split when neighboring schemas diverge
/// - gap ≥ hard: always split (distinct multi-table separation)
fn split_multi_regions<'a>(
    multi: &[&'a TextLine],
    soft_gap: f32,
    hard_gap: f32,
    fs: f32,
) -> Vec<Vec<&'a TextLine>> {
    if multi.is_empty() {
        return Vec::new();
    }
    let tol = left_cluster_tol(fs);
    let mut regions = Vec::new();
    let mut cur: Vec<&TextLine> = vec![multi[0]];
    for i in 0..multi.len() - 1 {
        let gap = (multi[i].y - multi[i + 1].y).abs();
        let split = if gap >= hard_gap {
            true
        } else if gap > soft_gap {
            // Keep only when neighboring windows share the same column count
            // and left-edge layout (section note → same schema continues).
            let a0 = i.saturating_sub(3);
            let a = &multi[a0..=i];
            let b1 = (i + 1 + 3).min(multi.len() - 1);
            let b = &multi[i + 1..=b1];
            let sa = region_col_lefts_supported(a, fs);
            let sb = region_col_lefts_supported(b, fs);
            !schemas_compatible(&sa, &sb, tol)
        } else {
            false
        };
        if split {
            regions.push(std::mem::take(&mut cur));
            cur = vec![multi[i + 1]];
        } else {
            cur.push(multi[i + 1]);
        }
    }
    if !cur.is_empty() {
        regions.push(cur);
    }
    regions
}

/// All left-edges clustered (no support filter) — used only as fallback.
fn region_col_lefts(lines: &[&TextLine], fs: f32) -> Vec<f32> {
    let mut lefts: Vec<f32> = Vec::new();
    for line in lines {
        for r in &line.runs {
            if r.text.trim().is_empty() {
                continue;
            }
            lefts.push(r.bbox.x0);
        }
    }
    let x_tol = left_cluster_tol(fs);
    let mut xs = cluster_coords(&lefts, x_tol);
    xs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    xs
}

/// Left-edge anchors that appear on multiple rows (rejects one-off jitter phantoms).
fn region_col_lefts_supported(lines: &[&TextLine], fs: f32) -> Vec<f32> {
    if lines.is_empty() {
        return Vec::new();
    }
    let x_tol = left_cluster_tol(fs);
    let raw = region_col_lefts(lines, fs);
    if raw.len() < 2 {
        return raw;
    }
    // Multi-row support: appear on at least ~⅓ of lines (geometric majority of
    // a third of the region — scales with height, no absolute cap).
    let min_support = ((lines.len() + 2) / 3).max(2);
    let hit_tol = x_tol;
    let mut supported: Vec<(f32, usize)> = Vec::new();
    for &cx in &raw {
        let hits = lines
            .iter()
            .filter(|line| {
                line.runs
                    .iter()
                    .any(|r| !r.text.trim().is_empty() && (r.bbox.x0 - cx).abs() <= hit_tol)
            })
            .count();
        if hits >= min_support {
            supported.push((cx, hits));
        }
    }
    if supported.len() < 2 {
        return raw;
    }
    supported.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    let cols: Vec<f32> = supported.iter().map(|(c, _)| *c).collect();
    let collapsed = collapse_near_cols(&cols, lines, x_tol);
    if collapsed.len() >= 2 {
        collapsed
    } else {
        cols
    }
}

/// Exact equal-length schema (legacy strict path).
fn same_schema(a: &[f32], b: &[f32], tol: f32) -> bool {
    if a.len() != b.len() || a.len() < 2 {
        return false;
    }
    a.iter()
        .zip(b.iter())
        .all(|(x, y)| (*x - *y).abs() <= tol)
}

/// Soft schema match for same column count with jittered left-edges.
/// Different column counts ⇒ different tables (never soft-merge 3-col with 4-col).
fn schemas_compatible(a: &[f32], b: &[f32], tol: f32) -> bool {
    if a.len() < 2 || b.len() < 2 || a.len() != b.len() {
        return false;
    }
    if same_schema(a, b, tol) {
        return true;
    }
    // Equal count: majority of anchors bipartite-match (mild x-jitter).
    let mut used = vec![false; b.len()];
    let mut matched = 0usize;
    for &ax in a {
        let mut best: Option<(usize, f32)> = None;
        for (i, &bx) in b.iter().enumerate() {
            if used[i] {
                continue;
            }
            let d = (ax - bx).abs();
            if d <= tol && best.map_or(true, |(_, bd)| d < bd) {
                best = Some((i, d));
            }
        }
        if let Some((i, _)) = best {
            used[i] = true;
            matched += 1;
        }
    }
    matched * 2 >= a.len() && matched >= 2
}

/// Vertical gap between the last line of `a` and first line of `b` (top→bottom order).
fn region_gap(a: &[&TextLine], b: &[&TextLine]) -> f32 {
    match (a.last(), b.first()) {
        (Some(x), Some(y)) => (x.y - y.y).abs(),
        _ => 0.0,
    }
}

/// Re-merge adjacent regions with the same column skeleton, and bridge short
/// incompatible islands (section-note multi-col lines between table halves).
/// Never merge across a hard vertical gap.
fn merge_same_schema_regions<'a>(
    regions: Vec<Vec<&'a TextLine>>,
    fs: f32,
    hard_gap: f32,
) -> Vec<Vec<&'a TextLine>> {
    if regions.len() <= 1 {
        return regions;
    }
    let tol = left_cluster_tol(fs);

    let adjacent_merge = |regs: Vec<Vec<&'a TextLine>>| -> Vec<Vec<&'a TextLine>> {
        let mut out: Vec<Vec<&TextLine>> = Vec::new();
        for reg in regs {
            if out.is_empty() {
                out.push(reg);
                continue;
            }
            let prev = out.last().unwrap();
            if region_gap(prev, &reg) >= hard_gap {
                out.push(reg);
                continue;
            }
            let sa = region_col_lefts_supported(prev, fs);
            let sb = region_col_lefts_supported(&reg, fs);
            if schemas_compatible(&sa, &sb, tol) {
                out.last_mut().unwrap().extend(reg);
            } else {
                out.push(reg);
            }
        }
        out
    };

    let mut out = adjacent_merge(regions);

    // Bridge A | island | C when island is smaller than a min body table and
    // A/C share schema (section-note multi-col lines between halves).
    let max_island = 3usize; // below stream_min_body_bands default floor
    for _ in 0..8 {
        if out.len() < 3 {
            break;
        }
        let mut next: Vec<Vec<&TextLine>> = Vec::new();
        let mut i = 0;
        let mut changed = false;
        while i < out.len() {
            if i + 2 < out.len() && out[i + 1].len() <= max_island {
                let gap_ac = region_gap(&out[i], &out[i + 2]);
                if gap_ac < hard_gap {
                    let sa = region_col_lefts_supported(&out[i], fs);
                    let sc = region_col_lefts_supported(&out[i + 2], fs);
                    if schemas_compatible(&sa, &sc, tol) {
                        let mut merged = std::mem::take(&mut out[i]);
                        merged.extend(std::mem::take(&mut out[i + 1]));
                        merged.extend(std::mem::take(&mut out[i + 2]));
                        next.push(merged);
                        i += 3;
                        changed = true;
                        continue;
                    }
                }
            }
            next.push(std::mem::take(&mut out[i]));
            i += 1;
        }
        out = adjacent_merge(next);
        if !changed {
            break;
        }
    }
    out
}

/// Merge column anchors closer than half median pitch (jitter double-peaks).
fn collapse_near_cols(cols: &[f32], lines: &[&TextLine], x_tol: f32) -> Vec<f32> {
    if cols.len() < 3 {
        return cols.to_vec();
    }
    let mut gaps: Vec<f32> = cols.windows(2).map(|w| w[1] - w[0]).filter(|g| *g > 1.0).collect();
    if gaps.is_empty() {
        return cols.to_vec();
    }
    gaps.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let med_gap = gaps[gaps.len() / 2].max(x_tol * 2.0);
    let merge_dist = (0.5 * med_gap).max(x_tol);

    let support = |cx: f32| -> usize {
        lines
            .iter()
            .filter(|line| {
                line.runs
                    .iter()
                    .any(|r| !r.text.trim().is_empty() && (r.bbox.x0 - cx).abs() <= x_tol * 1.2)
            })
            .count()
    };

    let mut out: Vec<(f32, usize)> = cols.iter().map(|&c| (c, support(c))).collect();
    let mut changed = true;
    while changed && out.len() >= 3 {
        changed = false;
        let mut next: Vec<(f32, usize)> = Vec::with_capacity(out.len());
        let mut i = 0;
        while i < out.len() {
            if i + 1 < out.len() && (out[i + 1].0 - out[i].0) <= merge_dist {
                // Keep the stronger-supported anchor (or average if tied).
                let (c0, s0) = out[i];
                let (c1, s1) = out[i + 1];
                let (c, s) = if s0 > s1 {
                    (c0, s0 + s1)
                } else if s1 > s0 {
                    (c1, s0 + s1)
                } else {
                    ((c0 + c1) * 0.5, s0 + s1)
                };
                next.push((c, s));
                i += 2;
                changed = true;
            } else {
                next.push(out[i]);
                i += 1;
            }
        }
        out = next;
    }
    out.into_iter().map(|(c, _)| c).collect()
}

fn assign_col(r: &TextRun, anchors: &[f32], xs: &[f32], ncols: usize, hit_tol: f32) -> usize {
    // Snap left edge to nearest anchor when close.
    let mut best_a: Option<(usize, f32)> = None;
    for (i, &a) in anchors.iter().enumerate() {
        let d = (r.bbox.x0 - a).abs();
        if d <= hit_tol {
            if best_a.map_or(true, |(_, bd)| d < bd) {
                best_a = Some((i, d));
            }
        }
    }
    if let Some((i, _)) = best_a {
        return i.min(ncols - 1);
    }
    let cx = (r.bbox.x0 + r.bbox.x1) * 0.5;
    let mut col = ncols - 1;
    for c in 0..ncols {
        if cx >= xs[c] && cx < xs[c + 1] {
            col = c;
            break;
        }
    }
    col
}

fn build_table_from_lines(
    page_index: u32,
    lines: &[&TextLine],
    opts: &TableOptions,
    fs: f32,
) -> Option<Table> {
    if lines.len() < opts.stream_min_body_bands.max(3) as usize {
        return None;
    }

    let x_tol = left_cluster_tol(fs);
    // First-pass anchors from support-filtered left edges (jitter-tolerant).
    let mut supported = region_col_lefts_supported(lines, fs);
    if supported.len() < 2 {
        supported = region_col_lefts(lines, fs);
    }
    if supported.len() < 2 {
        return None;
    }

    // Drop multi-col lines that poorly align with the region's column skeleton
    // (section-note mini-grids, list markers). Keeps real body + header rows.
    let hit_tol = x_tol * 1.25;
    let grid_lines: Vec<&TextLine> = lines
        .iter()
        .copied()
        .filter(|line| {
            let n = line
                .runs
                .iter()
                .filter(|r| !r.text.trim().is_empty())
                .count();
            if n < 2 {
                return false;
            }
            let aligned = line
                .runs
                .iter()
                .filter(|r| {
                    !r.text.trim().is_empty()
                        && supported
                            .iter()
                            .any(|&cx| (r.bbox.x0 - cx).abs() <= hit_tol)
                })
                .count();
            // Majority of cells land on region anchors.
            aligned >= 2 && aligned * 2 >= n
        })
        .collect();
    let use_lines: &[&TextLine] = if grid_lines.len() >= opts.stream_min_body_bands.max(3) as usize
    {
        &grid_lines
    } else {
        lines
    };

    // Recompute anchors on cleaned lines for tighter columns.
    supported = region_col_lefts_supported(use_lines, fs);
    if supported.len() < 2 {
        supported = region_col_lefts(use_lines, fs);
    }
    if supported.len() < 2 {
        return None;
    }

    // Collapse residual near-duplicate anchors (post-jitter split clusters).
    supported = collapse_near_cols(&supported, use_lines, x_tol);
    if supported.len() < 2 {
        return None;
    }

    let mut rights: Vec<f32> = Vec::new();
    for line in use_lines {
        for r in &line.runs {
            rights.push(r.bbox.x1);
        }
    }
    let page_right = rights.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    let mut xs = vec![supported[0] - 1.0];
    for w in supported.windows(2) {
        xs.push((w[0] + w[1]) * 0.5);
    }
    xs.push(page_right.max(*supported.last().unwrap() + fs * 4.0) + 1.0);
    xs = cluster_coords(&xs, 1.0);
    xs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let ncols = xs.len().saturating_sub(1);
    if ncols < 2 || ncols as u32 > opts.lattice_max_cols {
        return None;
    }

    let nrows = use_lines.len();
    if nrows as u32 > opts.lattice_max_rows {
        return None;
    }

    let centers: Vec<f32> = use_lines.iter().map(|l| l.y).collect();
    let mut ys = Vec::with_capacity(nrows + 1);
    ys.push(centers[0] + fs * 0.7);
    for w in centers.windows(2) {
        ys.push((w[0] + w[1]) * 0.5);
    }
    ys.push(centers[nrows - 1] - fs * 0.7);

    let mut grid: Vec<Vec<String>> = vec![vec![String::new(); ncols]; nrows];
    let mut bboxes: Vec<Vec<Rect>> = vec![
        vec![
            Rect {
                x0: 0.0,
                y0: 0.0,
                x1: 0.0,
                y1: 0.0
            };
            ncols
        ];
        nrows
    ];

    for (ri, line) in use_lines.iter().enumerate() {
        let y1 = ys[ri].max(ys[ri + 1]);
        let y0 = ys[ri].min(ys[ri + 1]);
        for r in &line.runs {
            let t = r.text.trim();
            if t.is_empty() {
                continue;
            }
            // Prefer snap-to-anchor when left edge is near a column; else center bin.
            let col = assign_col(r, &supported, &xs, ncols, hit_tol);
            if !grid[ri][col].is_empty() {
                grid[ri][col].push(' ');
            }
            grid[ri][col].push_str(t);
        }
        for c in 0..ncols {
            bboxes[ri][c] = Rect {
                x0: xs[c],
                y0,
                x1: xs[c + 1],
                y1,
            };
        }
    }

    // Strong reject: 2-col prose bait (word lists, numbered lists).
    if ncols == 2 {
        let mut alpha_pairs = 0u32;
        let mut rows_ne = 0u32;
        let mut numish = 0u32;
        let mut list_marker = 0u32;
        let mut long_right = 0u32;
        for row in &grid {
            let a = row[0].trim();
            let b = row[1].trim();
            if a.is_empty() && b.is_empty() {
                continue;
            }
            rows_ne += 1;
            let dig = a.chars().filter(|c| c.is_ascii_digit()).count()
                + b.chars().filter(|c| c.is_ascii_digit()).count();
            if dig >= 1 {
                numish += 1;
            }
            let a_alpha = a.chars().any(|c| c.is_alphabetic());
            let b_alpha = b.chars().any(|c| c.is_alphabetic());
            if a_alpha && b_alpha && dig == 0 {
                alpha_pairs += 1;
            }
            // "1." / "(a)" / "•" style markers in col0
            let marker = {
                let t = a.trim_end_matches(|c: char| c == '.' || c == ')' || c == ':');
                let t = t.trim_start_matches('(');
                (t.chars().all(|c| c.is_ascii_digit()) && !t.is_empty() && t.len() <= 3)
                    || (t.len() == 1 && t.chars().next().unwrap().is_ascii_alphabetic())
            };
            if marker {
                list_marker += 1;
            }
            if b.chars().count() >= 28 {
                long_right += 1;
            }
        }
        if rows_ne >= 4
            && (alpha_pairs as f32) / (rows_ne as f32) >= 0.60
            && (numish as f32) / (rows_ne as f32) < 0.20
        {
            return None;
        }
        // Numbered / lettered prose list: short marker col + long prose col.
        if rows_ne >= 4
            && (list_marker as f32) / (rows_ne as f32) >= 0.70
            && (long_right as f32) / (rows_ne as f32) >= 0.50
        {
            return None;
        }
    }

    let mean_chars = {
        let mut n = 0u32;
        let mut ch = 0u32;
        for row in &grid {
            for c in row {
                if c.is_empty() {
                    continue;
                }
                n += 1;
                ch += c.chars().count() as u32;
            }
        }
        if n == 0 {
            0.0
        } else {
            ch as f32 / n as f32
        }
    };
    if mean_chars >= opts.stream_max_prose_mean_chars && ncols <= 2 {
        return None;
    }

    let mut cells: Vec<TableCell> = Vec::new();
    let mut filled = 0u32;
    for r in 0..nrows {
        for c in 0..ncols {
            let text = grid[r][c].clone();
            if !text.is_empty() {
                filled += 1;
            }
            cells.push(TableCell {
                row: r as u32,
                col: c as u32,
                rowspan: 1,
                colspan: 1,
                bbox: bboxes[r][c],
                text,
                is_header: r == 0,
                confidence: 0.85,
            });
        }
    }
    if filled < 4 {
        return None;
    }
    let fill_rate = filled as f32 / (nrows * ncols) as f32;
    if fill_rate < 0.15 && filled < 8 {
        return None;
    }

    let bbox = bbox_of_cells(&cells);
    let conf = (0.55 + 0.25 * fill_rate.min(1.0) + 0.10 * (ncols as f32 / 6.0).min(1.0)
        + 0.10 * (nrows as f32 / 20.0).min(1.0))
    .clamp(0.0, 0.95);
    if conf < opts.min_confidence_stream {
        return None;
    }

    Some(Table {
        bbox,
        page: page_index,
        method: TableMethod::Stream,
        confidence: conf,
        rows: nrows as u32,
        cols: ncols as u32,
        cells,
        header_rows: 1,
        continued_from_previous_page: false,
        continued_to_next_page: false,
        logical_table_id: None,
        strategy_provenance: vec![PipelineId::S5Network],
        notes: vec![format!("network {nrows}x{ncols}")],
        edge_score: 0.0,
        fill_rate,
        weak_edges: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::options::TablePreset;
    use pdfparser_ir::Matrix3x2;

    fn grid_runs(rows: u32, cols: u32) -> Vec<TextRun> {
        let mut runs = Vec::new();
        for r in 0..rows {
            for c in 0..cols {
                runs.push(TextRun {
                    text: format!("r{r}c{c}"),
                    bbox: Rect {
                        x0: 30.0 + c as f32 * 50.0,
                        y0: 700.0 - r as f32 * 12.0,
                        x1: 45.0 + c as f32 * 50.0,
                        y1: 710.0 - r as f32 * 12.0,
                    },
                    transform: Matrix3x2::identity(),
                    font_name: None,
                    font_size: 9.0,
                    mapping_confidence: 1.0,
                    metrics_confidence: 1.0,
                    mcid: None,
                    invisible: false,
                    from_actual_text: false,
                });
            }
        }
        runs
    }

    #[test]
    fn network_large_borderless() {
        let runs = grid_runs(25, 5);
        let opts = TableOptions::from_preset(TablePreset::Auto);
        let tabs = detect_network_tables(0, &runs, &opts);
        assert!(!tabs.is_empty());
        assert!(tabs[0].rows >= 20, "rows={}", tabs[0].rows);
        assert_eq!(tabs[0].cols, 5);
    }

    /// Large irregular borderless grid with mid-page section-note islands + mild x jitter.
    /// Must stay one table (not fragment into header-slices).
    #[test]
    fn network_irregular_grid_section_gap_stays_one() {
        let mut runs = Vec::new();
        let cols = 8u32;
        let body_rows = 36u32;
        let xs: Vec<f32> = (0..cols).map(|c| 30.0 + c as f32 * 48.0).collect();
        let mut y = 740.0_f32;
        // header
        for (c, &x) in xs.iter().enumerate() {
            runs.push(TextRun {
                text: format!("H{c}"),
                bbox: Rect {
                    x0: x,
                    y0: y,
                    x1: x + 20.0,
                    y1: y + 8.0,
                },
                transform: Matrix3x2::identity(),
                font_name: None,
                font_size: 7.0,
                mapping_confidence: 1.0,
                metrics_confidence: 1.0,
                mcid: None,
                invisible: false,
                from_actual_text: false,
            });
        }
        y -= 12.0;
        for r in 0..body_rows {
            // Section note island every 10 body rows (different column schema).
            if r > 0 && r % 10 == 0 {
                y -= 8.0;
                runs.push(TextRun {
                    text: format!("=== Section {} notes ===", r / 10),
                    bbox: Rect {
                        x0: 30.0,
                        y0: y,
                        x1: 200.0,
                        y1: y + 7.0,
                    },
                    transform: Matrix3x2::identity(),
                    font_name: None,
                    font_size: 6.0,
                    mapping_confidence: 1.0,
                    metrics_confidence: 1.0,
                    mcid: None,
                    invisible: false,
                    from_actual_text: false,
                });
                y -= 10.0;
                // Mini multi-col note with different x anchors (must not fork regions).
                for (k, x) in [30.0_f32, 70.0, 110.0, 150.0, 190.0].iter().enumerate() {
                    runs.push(TextRun {
                        text: format!("note{k}"),
                        bbox: Rect {
                            x0: *x,
                            y0: y,
                            x1: *x + 18.0,
                            y1: y + 7.0,
                        },
                        transform: Matrix3x2::identity(),
                        font_name: None,
                        font_size: 6.0,
                        mapping_confidence: 1.0,
                        metrics_confidence: 1.0,
                        mcid: None,
                        invisible: false,
                        from_actual_text: false,
                    });
                }
                y -= 12.0;
            }
            for (c, &x) in xs.iter().enumerate() {
                // Mild jitter + occasional large offset (ICDAR-class).
                let jx = if (r + c as u32) % 11 == 0 {
                    14.0
                } else if (r * 3 + c as u32) % 5 == 0 {
                    -2.5
                } else if (r + c as u32) % 3 == 0 {
                    2.0
                } else {
                    0.0
                };
                // Sparse empties.
                if c > 0 && (r * 7 + c as u32) % 13 == 0 {
                    continue;
                }
                runs.push(TextRun {
                    text: format!("r{r}c{c}"),
                    bbox: Rect {
                        x0: x + jx,
                        y0: y,
                        x1: x + jx + 18.0,
                        y1: y + 7.0,
                    },
                    transform: Matrix3x2::identity(),
                    font_name: None,
                    font_size: 6.0,
                    mapping_confidence: 1.0,
                    metrics_confidence: 1.0,
                    mcid: None,
                    invisible: false,
                    from_actual_text: false,
                });
            }
            y -= if r % 4 == 0 { 11.5 } else { 9.5 };
        }
        let opts = TableOptions::from_preset(TablePreset::Auto);
        let tabs = detect_network_tables(0, &runs, &opts);
        assert_eq!(
            tabs.len(),
            1,
            "expected 1 table, got {} shapes={:?}",
            tabs.len(),
            tabs.iter().map(|t| (t.rows, t.cols)).collect::<Vec<_>>()
        );
        let t = &tabs[0];
        // ~1 header + 36 body; section notes dropped. Allow small slack.
        assert!(
            t.rows >= 30 && t.rows <= 40,
            "rows should cover body, got {}",
            t.rows
        );
        assert!(
            (7..=9).contains(&t.cols),
            "cols should be ~8 despite jitter, got {}",
            t.cols
        );
    }

    /// Two distinct borderless tables with a large vertical gap + different
    /// column layouts must not merge (hard split + schema identity).
    #[test]
    fn network_hard_gap_keeps_two_tables() {
        let mut runs = Vec::new();
        // Table A: 5×3 at top-left
        for r in 0..5u32 {
            for c in 0..3u32 {
                runs.push(TextRun {
                    text: format!("A{r}{c}"),
                    bbox: Rect {
                        x0: 40.0 + c as f32 * 60.0,
                        y0: 700.0 - r as f32 * 12.0,
                        x1: 55.0 + c as f32 * 60.0,
                        y1: 710.0 - r as f32 * 12.0,
                    },
                    transform: Matrix3x2::identity(),
                    font_name: None,
                    font_size: 9.0,
                    mapping_confidence: 1.0,
                    metrics_confidence: 1.0,
                    mcid: None,
                    invisible: false,
                    from_actual_text: false,
                });
            }
        }
        // Table B: 6×2 lower-right, different x anchors, gap ≫ 2× soft_gap
        for r in 0..6u32 {
            for c in 0..2u32 {
                runs.push(TextRun {
                    text: format!("B{r}{c}"),
                    bbox: Rect {
                        x0: 320.0 + c as f32 * 80.0,
                        y0: 400.0 - r as f32 * 12.0,
                        x1: 340.0 + c as f32 * 80.0,
                        y1: 410.0 - r as f32 * 12.0,
                    },
                    transform: Matrix3x2::identity(),
                    font_name: None,
                    font_size: 9.0,
                    mapping_confidence: 1.0,
                    metrics_confidence: 1.0,
                    mcid: None,
                    invisible: false,
                    from_actual_text: false,
                });
            }
        }
        let opts = TableOptions::from_preset(TablePreset::Auto);
        let tabs = detect_network_tables(0, &runs, &opts);
        assert!(
            tabs.len() >= 2,
            "hard gap + different schema must keep 2 tables, got {:?}",
            tabs.iter().map(|t| (t.rows, t.cols)).collect::<Vec<_>>()
        );
    }

    #[test]
    fn network_rejects_numbered_prose_list() {
        let mut runs = Vec::new();
        for i in 0..8 {
            runs.push(TextRun {
                text: format!("{}.", i + 1),
                bbox: Rect {
                    x0: 40.0,
                    y0: 700.0 - i as f32 * 14.0,
                    x1: 55.0,
                    y1: 710.0 - i as f32 * 14.0,
                },
                transform: Matrix3x2::identity(),
                font_name: None,
                font_size: 10.0,
                mapping_confidence: 1.0,
                metrics_confidence: 1.0,
                mcid: None,
                invisible: false,
                from_actual_text: false,
            });
            runs.push(TextRun {
                text: format!(
                    "Long prose discussion point number {i} elaborates methodology further"
                ),
                bbox: Rect {
                    x0: 70.0,
                    y0: 700.0 - i as f32 * 14.0,
                    x1: 320.0,
                    y1: 710.0 - i as f32 * 14.0,
                },
                transform: Matrix3x2::identity(),
                font_name: None,
                font_size: 10.0,
                mapping_confidence: 1.0,
                metrics_confidence: 1.0,
                mcid: None,
                invisible: false,
                from_actual_text: false,
            });
        }
        let opts = TableOptions::from_preset(TablePreset::Auto);
        let tabs = detect_network_tables(0, &runs, &opts);
        assert!(
            tabs.is_empty(),
            "numbered prose list must not be a table: {:?}",
            tabs.iter().map(|t| (t.rows, t.cols)).collect::<Vec<_>>()
        );
    }
}

