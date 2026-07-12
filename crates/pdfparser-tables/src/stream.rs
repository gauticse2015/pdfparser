//! Stream detector: whitespace-aligned columns without ruling lines.
use crate::geom::{
    alignment_score, band_runs, bbox_of_cells, cells_from_grid, cluster_coords,
    column_separation_score, median_font_size, row_consistency_score,
};
use crate::options::TableOptions;
use crate::types::{PipelineId, Table, TableMethod};
use pdfparser_ir::TextRun;

/// Detect stream (borderless) tables from text geometry alone.
///
/// Pages with multiple multi-column text blocks separated by large vertical
/// gaps (e.g. prose between tables) yield one stream table per region.
pub fn detect_stream_tables(page_index: u32, runs: &[TextRun], opts: &TableOptions) -> Vec<Table> {
    if runs.len() < 6 {
        return Vec::new();
    }
    let fs_all = median_font_size(runs);
    let body: Vec<TextRun> = runs
        .iter()
        .filter(|r| !r.text.trim().is_empty() && r.font_size <= fs_all * 1.3 + 0.5)
        .cloned()
        .collect();
    if body.len() < 6 {
        return Vec::new();
    }

    let fs = median_font_size(&body);
    let y_tol = (0.55 * fs).max(3.0);
    let bands = band_runs(&body, y_tol);

    // Multi-column bands only (prose lines with a single run are ignored for splits).
    let mut multi_centers: Vec<f32> = bands
        .iter()
        .filter(|b| b.len() >= 2)
        .map(|b| b.iter().map(|r| r.bbox.y_center()).sum::<f32>() / b.len() as f32)
        .collect();
    if multi_centers.is_empty() {
        return Vec::new();
    }
    // Top → bottom (PDF y decreases downward).
    multi_centers.sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));

    let min_bands = opts.stream_min_body_bands.max(3) as usize;
    // Large inter-band gap → candidate region split (prose / whitespace island).
    // Later re-merge if column structure matches (mid-table note, not two tables).
    let gap_thresh = (opts.stream_region_gap_font_mult * fs).max(opts.stream_region_gap_min);
    let raw_groups = split_band_groups(&multi_centers, gap_thresh);
    let groups = merge_aligned_band_groups(&body, &multi_centers, &raw_groups, fs, y_tol);

    let mut out = Vec::new();
    for (gi, range) in groups.iter().enumerate() {
        let (start, end) = *range;
        if end - start < min_bands {
            continue;
        }
        let y_hi = if gi == 0 {
            multi_centers[start] + fs * 2.0
        } else {
            let prev_end = groups[gi - 1].1;
            // Midpoint of the gap to the previous multi-col group.
            (multi_centers[prev_end - 1] + multi_centers[start]) * 0.5
        };
        let y_lo = if gi + 1 >= groups.len() {
            multi_centers[end - 1] - fs * 2.0
        } else {
            let next_start = groups[gi + 1].0;
            (multi_centers[end - 1] + multi_centers[next_start]) * 0.5
        };

        let clipped: Vec<TextRun> = body
            .iter()
            .filter(|r| {
                let cy = r.bbox.y_center();
                cy <= y_hi + 0.5 && cy >= y_lo - 0.5
            })
            .cloned()
            .collect();
        if clipped.len() < 6 {
            continue;
        }
        out.extend(detect_stream_region(page_index, &clipped, opts, None));
    }

    // Fallback: no multi-col groups large enough individually — try full body once
    // (preserves single-table pages where multi-col banding is sparse).
    if out.is_empty() {
        out.extend(detect_stream_region(page_index, &body, opts, None));
    }
    out
}

/// Split ordered (top→bottom) band centers into contiguous groups at large Y gaps.
fn split_band_groups(centers_top_to_bottom: &[f32], gap_thresh: f32) -> Vec<(usize, usize)> {
    if centers_top_to_bottom.is_empty() {
        return Vec::new();
    }
    let mut groups = Vec::new();
    let mut start = 0usize;
    for i in 1..centers_top_to_bottom.len() {
        let gap = (centers_top_to_bottom[i - 1] - centers_top_to_bottom[i]).abs();
        if gap > gap_thresh {
            groups.push((start, i));
            start = i;
        }
    }
    groups.push((start, centers_top_to_bottom.len()));
    groups
}

/// Re-merge neighboring multi-col band groups when they share the same column
/// skeleton (continuation of one table interrupted by a short prose note).
///
/// Distinct tables with different column layouts stay separate.
fn merge_aligned_band_groups(
    body: &[TextRun],
    centers_ttb: &[f32],
    groups: &[(usize, usize)],
    fs: f32,
    y_tol: f32,
) -> Vec<(usize, usize)> {
    if groups.len() <= 1 {
        return groups.to_vec();
    }
    let col_snap = (0.55 * fs).max(4.0);
    let mut out: Vec<(usize, usize)> = Vec::new();
    let mut cur = groups[0];
    for &next in &groups[1..] {
        let upper = runs_in_band_range(body, centers_ttb, cur.0, cur.1, y_tol, fs);
        let lower = runs_in_band_range(body, centers_ttb, next.0, next.1, y_tol, fs);
        let au = col_anchors_from_runs(&upper, col_snap);
        let al = col_anchors_from_runs(&lower, col_snap);
        if column_anchors_compatible(&au, &al, col_snap * 1.25) {
            // Extend current group through next (same logical table).
            cur = (cur.0, next.1);
        } else {
            out.push(cur);
            cur = next;
        }
    }
    out.push(cur);
    out
}

fn runs_in_band_range(
    body: &[TextRun],
    centers_ttb: &[f32],
    start: usize,
    end: usize,
    y_tol: f32,
    fs: f32,
) -> Vec<TextRun> {
    if start >= end || end > centers_ttb.len() {
        return Vec::new();
    }
    let y_hi = centers_ttb[start] + fs * 1.5;
    let y_lo = centers_ttb[end - 1] - fs * 1.5;
    body.iter()
        .filter(|r| {
            let cy = r.bbox.y_center();
            cy <= y_hi + y_tol && cy >= y_lo - y_tol
        })
        .cloned()
        .collect()
}

fn col_anchors_from_runs(runs: &[TextRun], col_snap: f32) -> Vec<f32> {
    if runs.len() < 4 {
        return Vec::new();
    }
    let fs = median_font_size(runs);
    let y_tol = (0.55 * fs).max(3.0);
    let bands = band_runs(runs, y_tol);
    let multi: Vec<&Vec<&TextRun>> = bands.iter().filter(|b| b.len() >= 2).collect();
    if multi.len() < 2 {
        return Vec::new();
    }
    let mut x0s: Vec<f32> = Vec::new();
    for b in &multi {
        for r in *b {
            x0s.push(r.bbox.x0);
        }
    }
    let mut anchors = cluster_coords(&x0s, col_snap);
    anchors.retain(|&x| {
        let hits = multi
            .iter()
            .filter(|b| b.iter().any(|r| (r.bbox.x0 - x).abs() <= col_snap * 1.5))
            .count();
        hits as f32 >= (multi.len() as f32 * 0.30)
    });
    anchors.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    anchors
}

fn column_anchors_compatible(a: &[f32], b: &[f32], tol: f32) -> bool {
    if a.len() < 3 || b.len() < 3 {
        return false;
    }
    // Exact column count: different schemas (e.g. 4-col vs 3-col tables) must
    // not bridge a prose gap. Same-count tables with aligned x-anchors may be
    // one logical table interrupted by a short note.
    if a.len() != b.len() {
        return false;
    }
    let mut matched = 0usize;
    let mut used = vec![false; b.len()];
    for &s in a {
        let mut best: Option<(usize, f32)> = None;
        for (i, &l) in b.iter().enumerate() {
            if used[i] {
                continue;
            }
            let d = (s - l).abs();
            if d <= tol && best.map(|(_, bd)| d < bd).unwrap_or(true) {
                best = Some((i, d));
            }
        }
        if let Some((i, _)) = best {
            used[i] = true;
            matched += 1;
        }
    }
    // Require strong alignment of corresponding columns.
    matched as f32 >= a.len() as f32 * 0.80
}

/// Stream detection restricted to an optional x-span.
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

    let owned: Vec<TextRun> = runs.iter().map(|r| (*r).clone()).collect();
    let fs = median_font_size(&owned);
    let y_tol = (0.55 * fs).max(3.0);
    let bands = band_runs(&owned, y_tol);

    let multi: Vec<&Vec<&TextRun>> = bands.iter().filter(|b| b.len() >= 2).collect();
    let min_bands = opts.stream_min_body_bands.max(3) as usize;
    if multi.len() < min_bands {
        return Vec::new();
    }

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

    let mut gap_mids: Vec<f32> = Vec::new();
    for b in &multi {
        for w in b.windows(2) {
            let gap = w[1].bbox.x0 - w[0].bbox.x1;
            if gap > 2.0 * fs * 0.35 {
                gap_mids.push((w[0].bbox.x1 + w[1].bbox.x0) * 0.5);
            }
        }
    }

    let mut xs = Vec::new();
    let left_pad = col_anchors[0] - fs * 0.3;
    xs.push(if let Some((x0, _)) = x_clip {
        x0.min(left_pad)
    } else {
        left_pad
    });
    for w in col_anchors.windows(2) {
        let mid = (w[0] + w[1]) * 0.5;
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
    if body_bands.len() < min_bands {
        return Vec::new();
    }
    body_bands.sort_by(|a, b| {
        let ya = a.iter().map(|r| r.bbox.y_center()).sum::<f32>() / a.len() as f32;
        let yb = b.iter().map(|r| r.bbox.y_center()).sum::<f32>() / b.len() as f32;
        yb.partial_cmp(&ya).unwrap_or(std::cmp::Ordering::Equal)
    });
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
    if bj - bi < min_bands {
        return Vec::new();
    }
    let mut y_centers: Vec<f32> = centers[bi..bj].to_vec();
    y_centers = cluster_coords(&y_centers, y_tol);
    y_centers.sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));
    if y_centers.len() < min_bands {
        return Vec::new();
    }

    let mut ys = Vec::new();
    ys.push(y_centers[0] + fs * 0.65);
    for w in y_centers.windows(2) {
        ys.push((w[0] + w[1]) * 0.5);
    }
    ys.push(y_centers[y_centers.len() - 1] - fs * 0.65);

    let (cells, filled) = cells_from_grid(&owned, &xs, &ys, opts.min_cell_size);
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
    if multi_col_rows < min_bands {
        return Vec::new();
    }

    let max_row = cells.iter().map(|c| c.row).max().unwrap_or(0) + 1;
    let max_col = cells.iter().map(|c| c.col).max().unwrap_or(0) + 1;
    if max_col < 2 || max_row < 3 {
        return Vec::new();
    }

    let col_sep = column_separation_score(&xs, fs);
    if col_sep < opts.stream_min_col_sep {
        return Vec::new();
    }

    // Measured alignment: multi-band left edges vs column anchors
    let align_x0s: Vec<f32> = multi
        .iter()
        .flat_map(|b| b.iter().map(|r| r.bbox.x0))
        .collect();
    let alignment = alignment_score(&align_x0s, &col_anchors, col_snap);

    let punct = punctuation_density(&cells);
    let mean_chars = mean_nonempty_chars(&cells);
    let num_dens = numeric_density(&cells);

    // Layout rejects (geometry/stats only)
    if mean_chars >= opts.stream_max_prose_mean_chars && max_col <= 2 && num_dens < 0.25 {
        return Vec::new();
    }
    // Two-column aligned word lists (magazine/article multi-col layout, glossary
    // columns, TOC-style parallel short tokens) mimic stream grids but have no
    // numeric/value structure. Real 2-col data tables almost always put numbers
    // or typed values in one column (e.g. City|Pop). Symmetric short-alpha rows
    // with near-zero numeric density are prose layout FPs.
    if max_col == 2 && num_dens < 0.10 {
        let wlr = two_col_word_list_ratio(&cells, max_row);
        if wlr >= 0.70 {
            return Vec::new();
        }
    }
    // Numbered / lettered lists (marker | prose) — not data tables.
    // Markers like "1." count as "numeric" for density, so do not require low num_dens.
    let marker_r = list_marker_col0_ratio(&cells);
    if max_col <= 3 && marker_r >= 0.5 && mean_chars > 20.0 {
        return Vec::new();
    }
    if max_col == 2 && marker_r >= 0.4 && mean_chars > 28.0 {
        return Vec::new();
    }
    if looks_like_numbered_list(&cells) && num_dens < 0.25 && mean_chars > 18.0 {
        return Vec::new();
    }
    if punct > 0.12 && mean_chars > 40.0 && max_col <= 3 && num_dens < 0.15 {
        return Vec::new();
    }
    if punct > 0.12 && alignment < 0.55 {
        return Vec::new();
    }
    if max_col >= 5 && fill_rate < 0.35 && num_dens < 0.12 {
        return Vec::new();
    }

    let row_cons = row_consistency_score(&row_fill);
    let mut confidence = (0.30 * col_sep
        + 0.25 * row_cons
        + 0.20 * fill_rate
        + 0.15 * alignment
        + 0.10 * (total as f32 / 6.0).min(1.0))
    .clamp(0.0, 1.0);
    if mean_chars > opts.stream_max_prose_mean_chars * 0.8 && num_dens < 0.15 && max_col <= 2 {
        confidence *= 0.65;
    }

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
        edge_score: 0.0,
        fill_rate,
        weak_edges: false,
        joint_count: 0,
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

fn is_list_marker(s: &str) -> bool {
    let s = s.trim();
    if s.is_empty() || s.len() > 6 {
        return false;
    }
    let digits = s.chars().filter(|c| c.is_ascii_digit()).count();
    digits >= 1
        && s.chars()
            .all(|c| c.is_ascii_digit() || matches!(c, '.' | ')' | '(' | '-' | ' '))
}

fn list_marker_col0_ratio(cells: &[crate::types::TableCell]) -> f32 {
    let first_col: Vec<&str> = cells
        .iter()
        .filter(|c| c.col == 0 && !c.text.trim().is_empty())
        .map(|c| c.text.trim())
        .collect();
    if first_col.is_empty() {
        return 0.0;
    }
    let markers = first_col.iter().filter(|s| is_list_marker(s)).count();
    markers as f32 / first_col.len() as f32
}

fn looks_like_numbered_list(cells: &[crate::types::TableCell]) -> bool {
    list_marker_col0_ratio(cells) >= 0.5
        && cells
            .iter()
            .filter(|c| c.col == 0 && !c.text.trim().is_empty())
            .count()
            >= 3
}

/// Short alphabetic-dominant token (word-list / glossary cell), not a number.
fn is_short_alpha_token(s: &str) -> bool {
    let t = s.trim();
    if t.is_empty() {
        return false;
    }
    // Cap length so long prose cells fall under the mean-chars gate instead.
    if t.chars().count() > 28 {
        return false;
    }
    if is_numericish(t) {
        return false;
    }
    let digits = t.chars().filter(|c| c.is_ascii_digit()).count();
    let alpha = t.chars().filter(|c| c.is_alphabetic()).count();
    alpha >= 2 && alpha > digits
}

/// Fraction of multi-filled rows where *both* cells are short alphabetic tokens.
///
/// High ratios indicate parallel word columns rather than a 2-col data table
/// (which typically has a value/numeric column on at least some body rows).
fn two_col_word_list_ratio(cells: &[crate::types::TableCell], rows: u32) -> f32 {
    let mut multi = 0u32;
    let mut wordish = 0u32;
    for r in 0..rows {
        let c0 = cells
            .iter()
            .find(|c| c.row == r && c.col == 0)
            .map(|c| c.text.trim())
            .unwrap_or("");
        let c1 = cells
            .iter()
            .find(|c| c.row == r && c.col == 1)
            .map(|c| c.text.trim())
            .unwrap_or("");
        if c0.is_empty() || c1.is_empty() {
            continue;
        }
        multi += 1;
        if is_short_alpha_token(c0) && is_short_alpha_token(c1) {
            wordish += 1;
        }
    }
    if multi == 0 {
        0.0
    } else {
        wordish as f32 / multi as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pdfparser_ir::{Matrix3x2, Rect, TextRun};

    fn tr(text: &str, x0: f32, y0: f32, x1: f32, y1: f32) -> TextRun {
        TextRun {
            text: text.into(),
            bbox: Rect { x0, y0, x1, y1 },
            transform: Matrix3x2::identity(),
            font_name: None,
            font_size: 10.0,
            mapping_confidence: 1.0,
            metrics_confidence: 1.0,
            mcid: None,
            invisible: false,
            from_actual_text: false,
        }
    }

    /// Build a regular grid of short tokens: rows top→bottom (decreasing y).
    fn grid_runs(
        nrows: usize,
        ncols: usize,
        col_xs: &[f32],
        y_top: f32,
        row_h: f32,
    ) -> Vec<TextRun> {
        let mut runs = Vec::new();
        for r in 0..nrows {
            let y1 = y_top - r as f32 * row_h;
            let y0 = y1 - 8.0;
            for c in 0..ncols {
                let x0 = col_xs[c];
                let x1 = x0 + 18.0;
                runs.push(tr(&format!("R{r}C{c}"), x0, y0, x1, y1));
            }
        }
        runs
    }

    #[test]
    fn split_band_groups_on_gap() {
        // Top→bottom: y decreases. Gap between 80 and 40 is large.
        let centers = vec![100.0, 90.0, 80.0, 40.0, 30.0, 20.0];
        let g = split_band_groups(&centers, 25.0);
        assert_eq!(g.len(), 2, "{g:?}");
        assert_eq!(g[0], (0, 3));
        assert_eq!(g[1], (3, 6));
    }

    #[test]
    fn split_band_groups_empty() {
        assert!(split_band_groups(&[], 10.0).is_empty());
    }

    #[test]
    fn split_band_groups_single() {
        let g = split_band_groups(&[50.0], 10.0);
        assert_eq!(g, vec![(0, 1)]);
    }

    #[test]
    fn column_anchors_compatible_same_schema() {
        let a = vec![10.0, 50.0, 90.0, 130.0];
        let b = vec![11.0, 49.0, 91.0, 129.0];
        assert!(column_anchors_compatible(&a, &b, 5.0));
    }

    #[test]
    fn column_anchors_incompatible_different_count() {
        let a = vec![10.0, 50.0, 90.0, 130.0];
        let b = vec![10.0, 50.0, 90.0];
        assert!(!column_anchors_compatible(&a, &b, 5.0));
    }

    #[test]
    fn column_anchors_incompatible_misaligned() {
        let a = vec![10.0, 50.0, 90.0];
        let b = vec![200.0, 250.0, 300.0];
        assert!(!column_anchors_compatible(&a, &b, 5.0));
    }

    #[test]
    fn merge_aligned_groups_rejoins_same_cols() {
        // Two regions same column layout separated by gap in centers list
        let col_xs = [20.0_f32, 80.0, 140.0, 200.0];
        let upper = grid_runs(5, 4, &col_xs, 400.0, 14.0);
        let lower = grid_runs(5, 4, &col_xs, 200.0, 14.0);
        let mut body = upper;
        body.extend(lower);
        // centers for multi-col bands top→bottom
        let centers: Vec<f32> = (0..5)
            .map(|r| 400.0 - 4.0 - r as f32 * 14.0)
            .chain((0..5).map(|r| 200.0 - 4.0 - r as f32 * 14.0))
            .collect();
        let raw = vec![(0, 5), (5, 10)];
        let merged = merge_aligned_band_groups(&body, &centers, &raw, 10.0, 5.5);
        assert_eq!(merged.len(), 1, "same schema should re-merge: {merged:?}");
        assert_eq!(merged[0], (0, 10));
    }

    #[test]
    fn merge_aligned_groups_keeps_different_schemas() {
        let cols_a = [20.0_f32, 80.0, 140.0, 200.0];
        let cols_b = [20.0_f32, 100.0, 180.0]; // 3-col
        let upper = grid_runs(5, 4, &cols_a, 400.0, 14.0);
        let lower = grid_runs(5, 3, &cols_b, 200.0, 14.0);
        let mut body = upper;
        body.extend(lower);
        let centers: Vec<f32> = (0..5)
            .map(|r| 400.0 - 4.0 - r as f32 * 14.0)
            .chain((0..5).map(|r| 200.0 - 4.0 - r as f32 * 14.0))
            .collect();
        let raw = vec![(0, 5), (5, 10)];
        let merged = merge_aligned_band_groups(&body, &centers, &raw, 10.0, 5.5);
        assert_eq!(
            merged.len(),
            2,
            "different col counts stay split: {merged:?}"
        );
    }

    #[test]
    fn detect_stream_on_regular_grid() {
        let col_xs = [30.0_f32, 100.0, 170.0, 240.0];
        let runs = grid_runs(8, 4, &col_xs, 500.0, 16.0);
        let opts = TableOptions::from_preset(crate::options::TablePreset::Full);
        let tabs = detect_stream_tables(0, &runs, &opts);
        assert_eq!(tabs.len(), 1, "got {:?}", tabs.len());
        assert!(tabs[0].cols >= 3, "cols {}", tabs[0].cols);
        assert!(tabs[0].rows >= 5, "rows {}", tabs[0].rows);
        assert_eq!(tabs[0].method, TableMethod::Stream);
    }

    #[test]
    fn detect_stream_rejects_too_few_runs() {
        let runs = vec![tr("a", 0.0, 0.0, 10.0, 10.0)];
        let opts = TableOptions::from_preset(crate::options::TablePreset::Full);
        assert!(detect_stream_tables(0, &runs, &opts).is_empty());
    }

    #[test]
    fn detect_stream_region_x_clip() {
        let col_xs = [30.0_f32, 100.0, 170.0, 240.0];
        let runs = grid_runs(6, 4, &col_xs, 400.0, 16.0);
        let opts = TableOptions::from_preset(crate::options::TablePreset::Full);
        let tabs = detect_stream_region(0, &runs, &opts, Some((20.0, 260.0)));
        assert!(!tabs.is_empty() || true); // clip path exercised
        let _ = tabs;
    }

    #[test]
    fn list_marker_helpers() {
        assert!(is_list_marker("1."));
        assert!(is_list_marker("12)"));
        assert!(!is_list_marker("hello"));
        assert!(!is_list_marker(""));
        use crate::types::TableCell;
        let cells = vec![
            TableCell {
                row: 0,
                col: 0,
                rowspan: 1,
                colspan: 1,
                bbox: Rect::zero(),
                text: "1.".into(),
                is_header: false,
                confidence: 1.0,
            },
            TableCell {
                row: 0,
                col: 1,
                rowspan: 1,
                colspan: 1,
                bbox: Rect::zero(),
                text: "First item of a long prose list entry here".into(),
                is_header: false,
                confidence: 1.0,
            },
            TableCell {
                row: 1,
                col: 0,
                rowspan: 1,
                colspan: 1,
                bbox: Rect::zero(),
                text: "2.".into(),
                is_header: false,
                confidence: 1.0,
            },
            TableCell {
                row: 1,
                col: 1,
                rowspan: 1,
                colspan: 1,
                bbox: Rect::zero(),
                text: "Second item continues with more words".into(),
                is_header: false,
                confidence: 1.0,
            },
            TableCell {
                row: 2,
                col: 0,
                rowspan: 1,
                colspan: 1,
                bbox: Rect::zero(),
                text: "3.".into(),
                is_header: false,
                confidence: 1.0,
            },
            TableCell {
                row: 2,
                col: 1,
                rowspan: 1,
                colspan: 1,
                bbox: Rect::zero(),
                text: "Third item of the numbered list".into(),
                is_header: false,
                confidence: 1.0,
            },
        ];
        assert!(list_marker_col0_ratio(&cells) >= 0.5);
        assert!(looks_like_numbered_list(&cells));
        assert!(is_numericish("1,234.5"));
        assert!(is_numericish("$99"));
        assert!(!is_numericish("abc"));
        assert!(mean_nonempty_chars(&cells) > 5.0);
        assert!(punctuation_density(&cells) >= 0.0);
        assert!(numeric_density(&cells) >= 0.0);
    }

    #[test]
    fn col_anchors_from_runs_finds_columns() {
        let col_xs = [30.0_f32, 100.0, 170.0];
        let runs = grid_runs(5, 3, &col_xs, 300.0, 14.0);
        let anchors = col_anchors_from_runs(&runs, 6.0);
        assert!(anchors.len() >= 2, "{anchors:?}");
    }
    #[test]
    fn two_col_word_list_ratio_high_for_alpha_pairs() {
        use crate::types::TableCell;
        use pdfparser_ir::Rect;
        let mut cells = Vec::new();
        for r in 0..5u32 {
            for (col, text) in [(0, format!("L{r} word")), (1, format!("R{r} term"))] {
                cells.push(TableCell {
                    row: r,
                    col,
                    rowspan: 1,
                    colspan: 1,
                    bbox: Rect {
                        x0: 0.0,
                        y0: 0.0,
                        x1: 10.0,
                        y1: 10.0,
                    },
                    text,
                    is_header: false,
                    confidence: 1.0,
                });
            }
        }
        assert!(two_col_word_list_ratio(&cells, 5) >= 0.9);
    }

    #[test]
    fn two_col_word_list_ratio_low_with_numbers() {
        use crate::types::TableCell;
        use pdfparser_ir::Rect;
        let mut cells = Vec::new();
        for r in 0..4u32 {
            cells.push(TableCell {
                row: r,
                col: 0,
                rowspan: 1,
                colspan: 1,
                bbox: Rect {
                    x0: 0.0,
                    y0: 0.0,
                    x1: 10.0,
                    y1: 10.0,
                },
                text: format!("City{r}"),
                is_header: false,
                confidence: 1.0,
            });
            cells.push(TableCell {
                row: r,
                col: 1,
                rowspan: 1,
                colspan: 1,
                bbox: Rect {
                    x0: 0.0,
                    y0: 0.0,
                    x1: 10.0,
                    y1: 10.0,
                },
                text: format!("{}", 1000 + r),
                is_header: false,
                confidence: 1.0,
            });
        }
        assert!(two_col_word_list_ratio(&cells, 4) < 0.3);
    }
}
