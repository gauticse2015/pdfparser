//! S3 stream detector: whitespace-aligned columns without ruling lines.
use crate::geom::{
    band_runs, bbox_of_cells, cells_from_grid, cluster_coords, column_separation_score,
    median_font_size, row_consistency_score,
};
use crate::options::TableOptions;
use crate::types::{PipelineId, Table, TableMethod};
use pdfparser_ir::TextRun;

/// Detect stream (borderless) tables from text geometry alone.
pub fn detect_stream_tables(page_index: u32, runs: &[TextRun], opts: &TableOptions) -> Vec<Table> {
    if runs.len() < 6 {
        return Vec::new();
    }

    // Prefer body-sized runs (drop large titles).
    let fs_all = median_font_size(runs);
    let body: Vec<&TextRun> = runs
        .iter()
        .filter(|r| !r.text.trim().is_empty() && r.font_size <= fs_all * 1.3 + 0.5)
        .collect();
    if body.len() < 6 {
        return Vec::new();
    }
    let owned: Vec<TextRun> = body.iter().map(|r| (*r).clone()).collect();
    detect_stream_region(page_index, &owned, opts, None)
}

/// Stream detection restricted to an optional x-span (used by hybrid / multi-table).
pub fn detect_stream_region(
    page_index: u32,
    runs: &[TextRun],
    opts: &TableOptions,
    x_clip: Option<(f32, f32)>,
) -> Vec<Table> {
    let runs: Vec<&TextRun> = runs
        .iter()
        .filter(|r| {
            if r.text.trim().is_empty() {
                return false;
            }
            if let Some((x0, x1)) = x_clip {
                let cx = (r.bbox.x0 + r.bbox.x1) * 0.5;
                cx >= x0 - 2.0 && cx <= x1 + 2.0
            } else {
                true
            }
        })
        .collect();
    if runs.len() < 6 {
        return Vec::new();
    }

    let fs = median_font_size(
        &runs
            .iter()
            .map(|r| (*r).clone())
            .collect::<Vec<_>>(),
    );
    let y_tol = (0.55 * fs).max(3.0);
    let owned: Vec<TextRun> = runs.iter().map(|r| (*r).clone()).collect();
    let bands = band_runs(&owned, y_tol);

    // Multi-column bands only for column discovery
    let multi: Vec<&Vec<&TextRun>> = bands.iter().filter(|b| b.len() >= 2).collect();
    if multi.len() < 3 {
        return Vec::new();
    }

    // Column anchors from left edges on multi-run bands
    let mut x0s: Vec<f32> = Vec::new();
    for b in &multi {
        for r in *b {
            x0s.push(r.bbox.x0);
        }
    }
    let col_snap = (0.55 * fs).max(4.0);
    let mut col_anchors = cluster_coords(&x0s, col_snap);
    if col_anchors.len() < 2 {
        return Vec::new();
    }
    // Drop rare columns (< 40% of multi bands)
    col_anchors.retain(|&x| {
        let hits = multi
            .iter()
            .filter(|b| b.iter().any(|r| (r.bbox.x0 - x).abs() <= col_snap * 1.5))
            .count();
        hits as f32 >= (multi.len() as f32 * 0.35)
    });
    if col_anchors.len() < 2 {
        return Vec::new();
    }
    col_anchors.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    // Also try gap-midpoint peaks between words on multi bands
    let mut gap_mids: Vec<f32> = Vec::new();
    for b in &multi {
        for w in b.windows(2) {
            let gap = w[1].bbox.x0 - w[0].bbox.x1;
            if gap > 2.0 * fs * 0.35 {
                gap_mids.push((w[0].bbox.x1 + w[1].bbox.x0) * 0.5);
            }
        }
    }
    // Prefer left-edge anchors; gap mids refine if they align
    let mut xs = Vec::new();
    let left_pad = col_anchors[0] - fs * 0.3;
    xs.push(if let Some((x0, _)) = x_clip {
        x0.min(left_pad)
    } else {
        left_pad
    });
    for w in col_anchors.windows(2) {
        // boundary between columns = midpoint of previous rightmost content and next anchor
        // or simple midpoint of anchors
        let mid = (w[0] + w[1]) * 0.5;
        // snap to nearest gap peak if close
        let snapped = gap_mids
            .iter()
            .copied()
            .filter(|&g| (g - mid).abs() < col_snap * 2.0)
            .fold(None, |best: Option<f32>, g| match best {
                None => Some(g),
                Some(b) => {
                    if (g - mid).abs() < (b - mid).abs() {
                        Some(g)
                    } else {
                        Some(b)
                    }
                }
            })
            .unwrap_or(mid);
        xs.push(snapped);
    }
    let last = *col_anchors.last().unwrap();
    let right_extent = owned
        .iter()
        .filter(|r| (r.bbox.x0 - last).abs() <= col_snap * 3.0 || r.bbox.x0 >= last - col_snap)
        .map(|r| r.bbox.x1)
        .fold(last + fs * 4.0, f32::max);
    let right = if let Some((_, x1)) = x_clip {
        x1.max(right_extent)
    } else {
        right_extent
    };
    xs.push(right);
    xs = cluster_coords(&xs, 1.0);

    // Row centers: multi-column bands only (drop title/prose single lines)
    let x_lo = xs[0];
    let x_hi = *xs.last().unwrap();
    let mut body_bands: Vec<&Vec<&TextRun>> = multi
        .iter()
        .copied()
        .filter(|b| {
            let hits = b
                .iter()
                .filter(|r| {
                    col_anchors
                        .iter()
                        .any(|&a| (r.bbox.x0 - a).abs() <= col_snap * 1.5)
                })
                .count();
            hits >= 2
                && b.iter().any(|r| {
                    let cx = (r.bbox.x0 + r.bbox.x1) * 0.5;
                    cx >= x_lo - 2.0 && cx <= x_hi + 2.0
                })
        })
        .collect();
    if body_bands.len() < 3 {
        return Vec::new();
    }
    // Keep only the largest contiguous y-cluster of multi-col bands (drop far titles)
    body_bands.sort_by(|a, b| {
        let ya = a.iter().map(|r| r.bbox.y_center()).sum::<f32>() / a.len() as f32;
        let yb = b.iter().map(|r| r.bbox.y_center()).sum::<f32>() / b.len() as f32;
        yb.partial_cmp(&ya).unwrap_or(std::cmp::Ordering::Equal)
    });
    // Find densest vertical group (gap < 3*fs between successive bands)
    let centers: Vec<f32> = body_bands
        .iter()
        .map(|b| b.iter().map(|r| r.bbox.y_center()).sum::<f32>() / b.len() as f32)
        .collect();
    let mut best_range = (0usize, body_bands.len());
    let mut i = 0;
    while i < centers.len() {
        let mut j = i + 1;
        while j < centers.len() && (centers[j - 1] - centers[j]).abs() < fs * 3.5 {
            j += 1;
        }
        if j - i > best_range.1 - best_range.0 {
            best_range = (i, j);
        }
        i = j.max(i + 1);
    }
    let (bi, bj) = best_range;
    if bj - bi < 3 {
        return Vec::new();
    }
    let mut y_centers: Vec<f32> = centers[bi..bj].to_vec();
    y_centers = cluster_coords(&y_centers, y_tol);
    y_centers.sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));
    if y_centers.len() < 3 {
        return Vec::new();
    }

    let mut ys = Vec::new();
    let top = y_centers[0] + fs * 0.65;
    ys.push(top);
    for w in y_centers.windows(2) {
        ys.push((w[0] + w[1]) * 0.5);
    }
    let bot = y_centers[y_centers.len() - 1] - fs * 0.65;
    ys.push(bot);

    let min_cell = opts.min_cell_size;
    let (cells, filled) = cells_from_grid(&owned, &xs, &ys, min_cell);
    if cells.is_empty() || filled < 4 {
        return Vec::new();
    }
    let total = cells.len().max(1);
    let fill_rate = filled as f32 / total as f32;
    if fill_rate < 0.25 {
        return Vec::new();
    }

    let mut row_fill = vec![0u32; ys.len().saturating_sub(1)];
    for c in &cells {
        if !c.text.trim().is_empty() && (c.row as usize) < row_fill.len() {
            row_fill[c.row as usize] += 1;
        }
    }
    let multi_col_rows = row_fill.iter().filter(|&&n| n >= 2).count();
    if multi_col_rows < 3 {
        return Vec::new();
    }

    let max_row = cells.iter().map(|c| c.row).max().unwrap_or(0) + 1;
    let max_col = cells.iter().map(|c| c.col).max().unwrap_or(0) + 1;
    if max_col < 2 || max_row < 3 {
        return Vec::new();
    }

    // Prose / document-layout reject (NIST, papers, forms)
    let col_sep = column_separation_score(&xs, fs);
    if col_sep < 0.30 {
        return Vec::new();
    }
    let punct = punctuation_density(&cells);
    let alignment = 0.8f32;
    let mean_chars = mean_nonempty_chars(&cells);
    let num_dens = numeric_density(&cells);
    // Long paragraph cells in narrow 2-col layouts → prose (NIST definitions)
    if mean_chars >= 70.0 && max_col <= 2 && num_dens < 0.25 {
        return Vec::new();
    }
    // Numbered-list / definition layout (1. foo, 2. bar) only for 2-col
    if max_col <= 2 && looks_like_numbered_list(&cells) && num_dens < 0.2 && mean_chars > 40.0
    {
        return Vec::new();
    }
    // TOC-ish: many dots, long cells, few columns
    if punct > 0.12 && mean_chars > 40.0 && max_col <= 3 && num_dens < 0.15 {
        return Vec::new();
    }
    if punct > 0.12 && alignment < 0.55 {
        return Vec::new();
    }
    // Sparse wide multi-col stream with mostly empty cells is TOC/layout junk
    if max_col >= 5 && fill_rate < 0.35 && num_dens < 0.12 {
        return Vec::new();
    }

    let row_cons = row_consistency_score(&row_fill);
    let confidence = (0.30 * col_sep
        + 0.25 * row_cons
        + 0.20 * fill_rate
        + 0.15 * alignment
        + 0.10 * (total as f32 / 6.0).min(1.0))
    .clamp(0.0, 1.0);
    // Soft penalize borderline prose without hard reject (document scrub handles NIST)
    let confidence = if mean_chars > 55.0 && num_dens < 0.15 && max_col <= 2 {
        confidence * 0.65
    } else {
        confidence
    };

    if confidence < opts.min_confidence_stream {
        return Vec::new();
    }

    let bbox = bbox_of_cells(&cells);
    vec![Table {
        bbox,
        page: page_index,
        method: TableMethod::Stream,
        confidence,
        rows: max_row,
        cols: max_col,
        cells,
        header_rows: 1,
        continued_from_previous_page: false,
        continued_to_next_page: false,
        logical_table_id: None,
        strategy_provenance: vec![PipelineId::S3Stream],
        notes: vec![format!("stream_cols={max_col} rows={max_row}")],
    }]
}

fn punctuation_density(cells: &[crate::types::TableCell]) -> f32 {
    let mut punct = 0u32;
    let mut chars = 0u32;
    for c in cells {
        for ch in c.text.chars() {
            if ch.is_whitespace() {
                continue;
            }
            chars += 1;
            if matches!(ch, '.' | '?' | '!' | ',' | ';' | ':' | '·' | '…') {
                punct += 1;
            }
        }
    }
    if chars == 0 {
        0.0
    } else {
        punct as f32 / chars as f32
    }
}

fn mean_nonempty_chars(cells: &[crate::types::TableCell]) -> f32 {
    let mut n = 0u32;
    let mut sum = 0u32;
    for c in cells {
        let t = c.text.trim();
        if t.is_empty() {
            continue;
        }
        n += 1;
        sum += t.chars().count() as u32;
    }
    if n == 0 {
        0.0
    } else {
        sum as f32 / n as f32
    }
}

fn numeric_density(cells: &[crate::types::TableCell]) -> f32 {
    let mut ne = 0u32;
    let mut num = 0u32;
    for c in cells {
        let t = c.text.trim();
        if t.is_empty() {
            continue;
        }
        ne += 1;
        if is_numericish(t) {
            num += 1;
        }
    }
    if ne == 0 {
        0.0
    } else {
        num as f32 / ne as f32
    }
}

fn is_numericish(s: &str) -> bool {
    let t = s
        .trim()
        .trim_matches(|c: char| c == '$' || c == '%' || c == '(' || c == ')' || c == ',');
    if t.is_empty() {
        return false;
    }
    let digits = t.chars().filter(|c| c.is_ascii_digit()).count();
    let alpha = t.chars().filter(|c| c.is_alphabetic()).count();
    digits >= 1 && digits >= alpha
}

fn looks_like_numbered_list(cells: &[crate::types::TableCell]) -> bool {
    // First column mostly "1." / "2." / "a)" style markers
    let first_col: Vec<&str> = cells
        .iter()
        .filter(|c| c.col == 0 && !c.text.trim().is_empty())
        .map(|c| c.text.trim())
        .collect();
    if first_col.len() < 3 {
        return false;
    }
    let markers = first_col
        .iter()
        .filter(|s| {
            let s = s.trim();
            if s.len() > 6 {
                return false;
            }
            let digits = s.chars().filter(|c| c.is_ascii_digit()).count();
            digits >= 1
                && s.chars()
                    .all(|c| c.is_ascii_digit() || matches!(c, '.' | ')' | '(' | '-' | ' '))
        })
        .count();
    markers as f32 / first_col.len() as f32 >= 0.5
}
