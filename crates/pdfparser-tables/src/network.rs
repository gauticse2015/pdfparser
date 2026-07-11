//! Network-class borderless tables (textline + column alignments).
//!
//! Production borderless path:
//! 1. Build textlines (baseline bands)
//! 2. Keep multi-column lines only for structure
//! 3. Split regions on large vertical gaps between multi-col lines
//! 4. Project column edges from left alignments
//! 5. One row per multi-col textline

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

    // Split multi-col runs on large gaps, then re-merge neighbors that share
    // the same column skeleton (one table interrupted by a short note).
    let gap_thresh = (opts.stream_region_gap_font_mult * fs).max(opts.stream_region_gap_min);
    let raw = split_multi_regions(&multi, gap_thresh);
    let regions = merge_same_schema_regions(raw, fs);
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

/// `multi` ordered top→bottom; split when consecutive multi-line Y gap exceeds thresh.
fn split_multi_regions<'a>(multi: &[&'a TextLine], gap_thresh: f32) -> Vec<Vec<&'a TextLine>> {
    if multi.is_empty() {
        return Vec::new();
    }
    let mut regions = Vec::new();
    let mut cur: Vec<&TextLine> = vec![multi[0]];
    for w in multi.windows(2) {
        let gap = (w[0].y - w[1].y).abs();
        if gap > gap_thresh {
            regions.push(std::mem::take(&mut cur));
            cur = vec![w[1]];
        } else {
            cur.push(w[1]);
        }
    }
    if !cur.is_empty() {
        regions.push(cur);
    }
    regions
}

fn region_col_lefts(lines: &[&TextLine], fs: f32) -> Vec<f32> {
    let mut lefts: Vec<f32> = Vec::new();
    for line in lines {
        for r in &line.runs {
            lefts.push(r.bbox.x0);
        }
    }
    let x_tol = (0.55 * fs).max(3.0);
    let mut xs = cluster_coords(&lefts, x_tol);
    xs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    xs
}

fn same_schema(a: &[f32], b: &[f32], tol: f32) -> bool {
    if a.len() != b.len() || a.len() < 2 {
        return false;
    }
    a.iter()
        .zip(b.iter())
        .all(|(x, y)| (*x - *y).abs() <= tol)
}

/// Re-merge adjacent regions with matching column left-edges (continuation table).
fn merge_same_schema_regions<'a>(
    regions: Vec<Vec<&'a TextLine>>,
    fs: f32,
) -> Vec<Vec<&'a TextLine>> {
    if regions.len() <= 1 {
        return regions;
    }
    let tol = (0.8 * fs).max(4.0);
    let mut out: Vec<Vec<&TextLine>> = Vec::new();
    for reg in regions {
        if out.is_empty() {
            out.push(reg);
            continue;
        }
        let prev = out.last().unwrap();
        let sa = region_col_lefts(prev, fs);
        let sb = region_col_lefts(&reg, fs);
        if same_schema(&sa, &sb, tol) {
            out.last_mut().unwrap().extend(reg);
        } else {
            out.push(reg);
        }
    }
    out
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

    let mut lefts: Vec<f32> = Vec::new();
    let mut rights: Vec<f32> = Vec::new();
    for line in lines {
        for r in &line.runs {
            lefts.push(r.bbox.x0);
            rights.push(r.bbox.x1);
        }
    }
    let x_tol = (0.55 * fs).max(3.0);
    let mut col_lefts = cluster_coords(&lefts, x_tol);
    col_lefts.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    if col_lefts.len() < 2 {
        return None;
    }

    let min_support = (lines.len() / 4).max(2);
    let mut supported: Vec<f32> = Vec::new();
    for &cx in &col_lefts {
        let hits = lines
            .iter()
            .filter(|line| {
                line.runs
                    .iter()
                    .any(|r| (r.bbox.x0 - cx).abs() <= x_tol * 1.2)
            })
            .count();
        if hits >= min_support {
            supported.push(cx);
        }
    }
    if supported.len() < 2 {
        supported = col_lefts;
    }
    supported = cluster_coords(&supported, x_tol * 0.8);
    supported.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    if supported.len() < 2 {
        return None;
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

    let nrows = lines.len();
    if nrows as u32 > opts.lattice_max_rows {
        return None;
    }

    let centers: Vec<f32> = lines.iter().map(|l| l.y).collect();
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

    for (ri, line) in lines.iter().enumerate() {
        let y1 = ys[ri].max(ys[ri + 1]);
        let y0 = ys[ri].min(ys[ri + 1]);
        for r in &line.runs {
            let cx = (r.bbox.x0 + r.bbox.x1) * 0.5;
            let mut col = ncols - 1;
            for c in 0..ncols {
                if cx >= xs[c] && cx < xs[c + 1] {
                    col = c;
                    break;
                }
            }
            if !grid[ri][col].is_empty() {
                grid[ri][col].push(' ');
            }
            grid[ri][col].push_str(r.text.trim());
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
