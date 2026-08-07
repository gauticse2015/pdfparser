//! Build a [`crate::types::Table`] from network textlines.
#![allow(clippy::needless_range_loop)]
#![allow(clippy::collapsible_if)]
#![allow(clippy::manual_pattern_char_comparison)]

use super::glued::*;
use super::TextLine;
use crate::geom::{bbox_of_cells, cluster_coords};
use crate::options::TableOptions;
use crate::types::{PipelineId, Table, TableCell, TableMethod};
use pdfparser_ir::{Rect, TextRun};

pub(crate) fn assign_col(
    r: &TextRun,
    anchors: &[f32],
    xs: &[f32],
    ncols: usize,
    hit_tol: f32,
) -> usize {
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

pub(crate) fn build_table_from_lines(
    page_index: u32,
    lines: &[&TextLine],
    opts: &TableOptions,
    fs: f32,
    page_width: f32,
) -> Option<Table> {
    if lines.len() < opts.advanced.stream_min_body_bands.max(3) as usize {
        return None;
    }

    let x_tol = left_cluster_tol(fs);
    // Prefer anchors from multi-run lines (headers / true multi-cell rows).
    // Glued single-run body rows all share one left edge and would otherwise
    // starve support-filtered anchors down to a single column.
    let mut supported = region_col_lefts_prefer_rich(lines, fs);
    if supported.len() < 2 {
        supported = region_col_lefts_supported(lines, fs);
    }
    if supported.len() < 2 {
        supported = region_col_lefts(lines, fs);
    }
    // Phase-4: NIPA/BEA-style pages paint each body row as one glued run
    // (label+numbers or pure number stream). Left-edge schema may be 1-col or
    // a weak few-col header skeleton; invent equal-width anchors from the max
    // tokenized field count when that is clearly richer.
    let mut synthetic_glued_cols = false;
    if let Some(syn) = synthesize_cols_from_glued_tokens(lines, fs) {
        if supported.len() < 2 || syn.len() >= supported.len().saturating_add(4) {
            supported = syn;
            synthetic_glued_cols = true;
        }
    }
    if supported.len() < 2 {
        return None;
    }

    // Drop multi-col lines that poorly align with the region's column skeleton
    // (section-note mini-grids, list markers). Keeps real body + header rows.
    // Glued single-run tabular rows are kept (they carry body data).
    // Single-run whitespace-token rows whose token count matches the column
    // skeleton are kept (stream headers painted as one TJ) — but only near
    // the region top, to avoid prose sentences mid-body becoming rows.
    let hit_tol = x_tol * 1.25;
    let n_anch = supported.len().max(2);
    // Region top y: highest multi-run (or any multi-token) line in the region.
    let region_top_y = lines.iter().map(|l| l.y).fold(f32::NEG_INFINITY, f32::max);
    let header_band = (fs * 2.5).max(hit_tol * 3.0);
    let grid_lines: Vec<&TextLine> = lines
        .iter()
        .copied()
        .filter(|line| {
            let n = line
                .runs
                .iter()
                .filter(|r| !r.text.trim().is_empty())
                .count();
            let near_top = line.y >= region_top_y - header_band;
            if n < 2 {
                return n == 1
                    && line
                        .runs
                        .first()
                        .map(|r| {
                            let t = r.text.as_str();
                            if looks_glued_tabular(t)
                                || looks_glued_numeric_stream(t)
                                || looks_nipa_placeholder_row(t)
                                || looks_nipa_section_header(t)
                            {
                                return true;
                            }
                            // Header band: single-run whitespace multi-col header.
                            // Multi-word labels often tokenize to > ncols
                            // ("Foreign turnover (FFr bn.) % of Total…").
                            if !near_top {
                                return false;
                            }
                            let tokens = t.split_whitespace().filter(|x| !x.is_empty()).count();
                            if tokens < 3 || tokens + 1 < n_anch {
                                return false;
                            }
                            if tokens <= n_anch + 2 {
                                return true;
                            }
                            let alpha = t.chars().filter(|c| c.is_alphabetic()).count();
                            let digits = t.chars().filter(|c| c.is_ascii_digit()).count();
                            alpha >= 10 && alpha > digits.saturating_mul(2)
                        })
                        .unwrap_or(false);
            }
            // With synthetic equal-width anchors, skip geometry alignment filter
            // (all glued rows share the same left edge).
            if synthetic_glued_cols {
                return true;
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
            if aligned >= 2 && aligned * 2 >= n {
                return true;
            }
            // Header band: keep multi-run lines near region top with weak
            // alignment. Require aligned≥1 so bare title/prose bands (fixture
            // 07) stay out, while label headers that touch ≥1 body anchor keep.
            near_top && n >= 2 && n <= n_anch.max(3) + 2 && aligned >= 1
        })
        .collect();
    let use_lines: &[&TextLine] =
        if grid_lines.len() >= opts.advanced.stream_min_body_bands.max(3) as usize {
            &grid_lines
        } else {
            lines
        };

    // Recompute anchors on cleaned lines (rich lines first), unless we already
    // synthesized equal-width columns for a glued-only region.
    if !synthetic_glued_cols {
        supported = region_col_lefts_prefer_rich(use_lines, fs);
        if supported.len() < 2 {
            supported = region_col_lefts_supported(use_lines, fs);
        }
        if supported.len() < 2 {
            supported = region_col_lefts(use_lines, fs);
        }
        if supported.len() < 2 {
            if let Some(syn) = synthesize_cols_from_glued_tokens(use_lines, fs) {
                supported = syn;
                synthetic_glued_cols = true;
            }
        } else if let Some(syn) = synthesize_cols_from_glued_tokens(use_lines, fs) {
            if syn.len() >= supported.len().saturating_add(4) {
                supported = syn;
                synthetic_glued_cols = true;
            }
        }
    }
    if supported.len() < 2 {
        return None;
    }

    // Collapse residual near-duplicate anchors (post-jitter split clusters).
    // Skip for synthetic equal-width glued columns — collapse uses run left-edges
    // and would re-fuse every synthetic anchor to a single left edge.
    if !synthetic_glued_cols {
        supported = collapse_near_cols(&supported, use_lines, x_tol);
    }
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
    if ncols < 2 || ncols as u32 > opts.advanced.lattice_max_cols {
        return None;
    }

    let mut nrows = use_lines.len();
    if nrows as u32 > opts.advanced.lattice_max_rows {
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
        // Single wide TJ string painted as one run: split whitespace tokens
        // left-to-right across columns (stream/export tables). Glued
        // label+numeric rows use proportional char-x binning into xs edges.
        // NIPA: glued body often arrives as one long run + a right-margin line#
        // as a second run. Join runs (skip far-right 1–3 digit markers) and
        // token-fill when the joined text is glued tabular.
        if ncols >= 3 && !line.runs.is_empty() {
            let mut sorted: Vec<&TextRun> = line.runs.iter().collect();
            sorted.sort_by(|a, b| {
                a.bbox
                    .x0
                    .partial_cmp(&b.bbox.x0)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            let x_left = sorted
                .iter()
                .find(|r| !r.text.trim().is_empty())
                .map(|r| r.bbox.x0)
                .unwrap_or(0.0);
            // Content runs excluding NIPA right-margin line# markers.
            let content: Vec<&TextRun> = sorted
                .iter()
                .copied()
                .filter(|r| {
                    let t = r.text.trim();
                    if t.is_empty() {
                        return false;
                    }
                    let pure_line = t.chars().all(|c| c.is_ascii_digit()) && t.len() <= 3;
                    !(pure_line && r.bbox.x0 > x_left + 200.0)
                })
                .collect();
            // CRITICAL: multi-run geometry rows (campaign donors, etc.) must NOT
            // be concatenated and force-filled via NIPA glued tokenizer.
            // Joining "MENA"+"JUAN" without spaces invents false letter+digit seams
            // and destroys column assignment (real-track regression 0.94→0.45).
            // Glued path only when ≤1 content run, or synthetic_glued_cols region.
            let use_glued_path = content.len() <= 1 || synthetic_glued_cols;
            if use_glued_path {
                let mut joined = String::new();
                for r in &content {
                    let t = r.text.trim();
                    if t.is_empty() {
                        continue;
                    }
                    // Space-join multi content only for synthetic glued pages
                    // (true multi-run should not reach here with content.len()>1
                    // unless synthetic_glued_cols).
                    if !joined.is_empty() && content.len() > 1 {
                        joined.push(' ');
                    }
                    joined.push_str(t);
                }
                if looks_glued_tabular(&joined)
                    || looks_glued_numeric_stream(&joined)
                    || looks_nipa_placeholder_row(&joined)
                {
                    fill_row_glued_tabular(&joined, &mut grid[ri]);
                    for c in 0..ncols {
                        bboxes[ri][c] = Rect {
                            x0: xs[c],
                            y0,
                            x1: xs[c + 1],
                            y1,
                        };
                    }
                    continue;
                }
                if looks_nipa_section_header(&joined) {
                    // Section banner occupies the label column only.
                    if ncols > 1 {
                        grid[ri][1] = joined;
                    } else {
                        grid[ri][0] = joined;
                    }
                    for c in 0..ncols {
                        bboxes[ri][c] = Rect {
                            x0: xs[c],
                            y0,
                            x1: xs[c + 1],
                            y1,
                        };
                    }
                    continue;
                }
                // Longest single run alone (partial half-line still better than
                // geometry binning of a truncated TJ) — only for true single-run.
                if content.len() == 1 {
                    let run = content[0];
                    if (looks_glued_tabular(&run.text) || looks_glued_numeric_stream(&run.text))
                        && run.text.len() >= 16
                    {
                        fill_row_glued_tabular(&run.text, &mut grid[ri]);
                        for c in 0..ncols {
                            bboxes[ri][c] = Rect {
                                x0: xs[c],
                                y0,
                                x1: xs[c + 1],
                                y1,
                            };
                        }
                        continue;
                    }
                }
            }
        }
        if line.runs.len() == 1 {
            let run = &line.runs[0];
            let tokens: Vec<&str> = run
                .text
                .split_whitespace()
                .filter(|t| !t.is_empty())
                .collect();
            if tokens.len() >= ncols && ncols >= 2 {
                // 1:1 token→col when counts match; otherwise bin tokens by
                // proportional char-x within the run bbox (multi-word headers
                // must not place one token per column).
                if tokens.len() == ncols {
                    for (ti, tok) in tokens.iter().enumerate() {
                        grid[ri][ti] = (*tok).to_string();
                    }
                } else {
                    fill_row_whitespace_by_x(run, &tokens, &mut grid[ri], &xs);
                }
                for c in 0..ncols {
                    bboxes[ri][c] = Rect {
                        x0: xs[c],
                        y0,
                        x1: xs[c + 1],
                        y1,
                    };
                }
                continue;
            }
        }
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

    // Expand glued header tokens into empty neighbor cells (StatesTotal, NSSFWMA).
    for row in &mut grid {
        expand_glued_headers_in_row(row);
    }

    // Drop leading caption / unit rows (TABLE 125, (Contd.), `Billion) that
    // inflate liabilities-style stream tables above the real header band.
    while nrows > 6 {
        let joined = grid[0]
            .iter()
            .map(|c| c.trim())
            .filter(|c| !c.is_empty())
            .collect::<Vec<_>>()
            .join(" ")
            .to_ascii_lowercase();
        let caption = joined.contains("table ")
            || joined.contains("contd")
            || joined.contains("billion")
            || joined.contains("(`")
            || joined.contains("( `");
        if !caption {
            break;
        }
        grid.remove(0);
        bboxes.remove(0);
        nrows -= 1;
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
                let t = a.trim_end_matches(['.', ')', ':']);
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
    // Numeric density for non-table area gates.
    let num_dens = {
        let mut n = 0u32;
        let mut dig = 0u32;
        for row in &grid {
            for c in row {
                if c.is_empty() {
                    continue;
                }
                n += 1;
                if c.chars().any(|ch| ch.is_ascii_digit()) {
                    dig += 1;
                }
            }
        }
        if n == 0 {
            0.0
        } else {
            dig as f32 / n as f32
        }
    };

    // Prose bait: long cells + low digit density. Classic 2-col lists, and also
    // multi-col paragraph grids (function words split across invented columns).
    if mean_chars >= opts.advanced.stream_max_prose_mean_chars && ncols <= 2 {
        return None;
    }
    if mean_chars >= opts.advanced.stream_max_prose_mean_chars * 0.70
        && num_dens < 0.28
        && ncols >= 3
        && nrows <= 12
    {
        return None;
    }
    // Short-token multi-col prose: function words / punctuation shards with
    // almost no numbers (prose mentions "Table 6.1" but is not a data grid).
    // Keep genuine small data tables (e.g. Name/Role/Office/Salary) that have
    // short label cells but a real numeric column (num_dens typically ≥0.20).
    if ncols >= 4 && nrows <= 10 && num_dens < 0.15 && mean_chars < 18.0 {
        let mut short_tokens = 0u32;
        let mut tokens = 0u32;
        for row in &grid {
            for c in row {
                let t = c.trim();
                if t.is_empty() {
                    continue;
                }
                tokens += 1;
                if t.chars().count() <= 8 {
                    short_tokens += 1;
                }
            }
        }
        if tokens >= 8 && short_tokens as f32 >= tokens as f32 * 0.55 {
            return None;
        }
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

    // Reject very narrow multi-col bands relative to page width when the area
    // looks non-tabular (low numeric density). Only when page width is
    // estimable from body runs and clearly wider than the candidate band —
    // pure synthetic grids (page_width ≈ table span) are unaffected.
    if page_width > 50.0 {
        let x_span = (bbox.x1 - bbox.x0).max(0.0);
        if x_span > 0.0 && x_span < 0.15 * page_width && num_dens < 0.20 && ncols >= 2 {
            return None;
        }
    }
    let conf = (0.55
        + 0.25 * fill_rate.min(1.0)
        + 0.10 * (ncols as f32 / 6.0).min(1.0)
        + 0.10 * (nrows as f32 / 20.0).min(1.0))
    .clamp(0.0, 0.95);
    if conf < opts.advanced.min_confidence_stream {
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
        joint_count: 0,
        text_row_recovery: false,
        text_col_recovery: false,
        multitable_stream_recovery: false,
        stream_vs_overwide_hybrid: false,
    })
}
