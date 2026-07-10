//! Shared geometry helpers for table detectors.
use pdfparser_ir::{Rect, TextRun};

/// Cluster 1D coordinates with snap tolerance.
pub fn cluster_coords(vals: &[f32], tol: f32) -> Vec<f32> {
    if vals.is_empty() {
        return Vec::new();
    }
    let mut v = vals.to_vec();
    v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let mut out = Vec::new();
    let mut cur = v[0];
    let mut sum = v[0];
    let mut n = 1u32;
    for &x in &v[1..] {
        if (x - cur).abs() <= tol {
            sum += x;
            n += 1;
            cur = sum / n as f32;
        } else {
            out.push(cur);
            cur = x;
            sum = x;
            n = 1;
        }
    }
    out.push(cur);
    out
}

/// Group text runs into horizontal bands (top-to-bottom).
pub fn band_runs(runs: &[TextRun], y_tol: f32) -> Vec<Vec<&TextRun>> {
    let mut items: Vec<&TextRun> = runs
        .iter()
        .filter(|r| !r.text.trim().is_empty())
        .collect();
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
    for b in &mut bands {
        b.sort_by(|a, c| {
            a.bbox
                .x0
                .partial_cmp(&c.bbox.x0)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
    }
    bands
}

/// R9: assign text runs into a cell rectangle.
pub fn assign_text(runs: &[TextRun], cell: Rect) -> String {
    let pad = 1.0f32;
    let mut parts: Vec<(f32, f32, &str)> = Vec::new();
    for r in runs {
        if r.text.trim().is_empty() {
            continue;
        }
        let cx = (r.bbox.x0 + r.bbox.x1) * 0.5;
        let cy = (r.bbox.y0 + r.bbox.y1) * 0.5;
        if cx >= cell.x0 - pad
            && cx <= cell.x1 + pad
            && cy >= cell.y0 - pad
            && cy <= cell.y1 + pad
        {
            parts.push((cy, cx, r.text.as_str()));
        }
    }
    parts.sort_by(|a, b| {
        b.0.partial_cmp(&a.0)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
    });
    let mut out = String::new();
    for (i, (_, _, t)) in parts.iter().enumerate() {
        if i > 0 {
            let prev_y = parts[i - 1].0;
            let y = parts[i].0;
            if (prev_y - y).abs() > 2.0 {
                // Mid-token line wrap (no separator) vs multi-line cell content
                let cont = out
                    .chars()
                    .last()
                    .map(|c| c.is_alphanumeric() || c == '_')
                    .unwrap_or(false)
                    && t.chars()
                        .next()
                        .map(|c| c.is_alphanumeric() || c == '_')
                        .unwrap_or(false);
                if !cont {
                    out.push('\n');
                }
            } else if !out.ends_with(' ') && !t.starts_with(' ') {
                out.push(' ');
            }
        }
        out.push_str(t);
    }
    out.trim().to_string()
}

/// Median font size helper.
pub fn median_font_size(runs: &[TextRun]) -> f32 {
    let mut v: Vec<f32> = runs
        .iter()
        .map(|r| r.font_size)
        .filter(|s| *s > 0.0)
        .collect();
    if v.is_empty() {
        return 10.0;
    }
    v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    v[v.len() / 2]
}

/// Median of a float slice.
pub fn median_f32(vals: &[f32]) -> f32 {
    if vals.is_empty() {
        return 0.0;
    }
    let mut v = vals.to_vec();
    v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    v[v.len() / 2]
}

/// Grid regularity from column/row positions.
pub fn grid_regularity_score(xs: &[f32], ys: &[f32]) -> f32 {
    fn cv_gaps(coords: &[f32]) -> f32 {
        if coords.len() < 3 {
            return 0.0;
        }
        let gaps: Vec<f32> = coords.windows(2).map(|w| (w[1] - w[0]).abs()).collect();
        let mean = gaps.iter().sum::<f32>() / gaps.len() as f32;
        if mean < 1e-3 {
            return 1.0;
        }
        let var = gaps.iter().map(|g| (g - mean).powi(2)).sum::<f32>() / gaps.len() as f32;
        (var.sqrt() / mean).min(2.0)
    }
    let cv = 0.5 * (cv_gaps(xs) + cv_gaps(ys));
    (1.0 - cv * 0.5).clamp(0.0, 1.0)
}

/// Build table cells from x and y boundaries (y top-to-bottom decreasing).
pub fn cells_from_grid(
    runs: &[TextRun],
    xs: &[f32],
    y_top_to_bottom: &[f32],
    min_cell: f32,
) -> (Vec<crate::types::TableCell>, usize) {
    use crate::types::TableCell;
    let mut cells = Vec::new();
    let mut filled = 0usize;
    if xs.len() < 2 || y_top_to_bottom.len() < 2 {
        return (cells, 0);
    }
    let nrows = y_top_to_bottom.len() - 1;
    let ncols = xs.len() - 1;
    for row in 0..nrows {
        let ya = y_top_to_bottom[row];
        let yb = y_top_to_bottom[row + 1];
        let (y1, y0) = if ya >= yb { (ya, yb) } else { (yb, ya) };
        if y1 - y0 < min_cell {
            continue;
        }
        for col in 0..ncols {
            let x0 = xs[col];
            let x1 = xs[col + 1];
            if x1 - x0 < min_cell {
                continue;
            }
            let bbox = Rect { x0, y0, x1, y1 };
            let text = assign_text(runs, bbox);
            if !text.trim().is_empty() {
                filled += 1;
            }
            cells.push(TableCell {
                row: row as u32,
                col: col as u32,
                rowspan: 1,
                colspan: 1,
                bbox,
                text,
                is_header: row == 0,
                confidence: 0.9,
            });
        }
    }
    (cells, filled)
}

/// Bounding box of cells.
pub fn bbox_of_cells(cells: &[crate::types::TableCell]) -> Rect {
    cells
        .iter()
        .map(|c| c.bbox)
        .reduce(|a, b| a.union(b))
        .unwrap_or(Rect::zero())
}

/// Column separation score from edge list.
pub fn column_separation_score(xs: &[f32], fs: f32) -> f32 {
    if xs.len() < 3 {
        return 0.5;
    }
    let gaps: Vec<f32> = xs.windows(2).map(|w| (w[1] - w[0]).abs()).collect();
    let mean = gaps.iter().sum::<f32>() / gaps.len() as f32;
    let target = fs * 3.0;
    (mean / target).min(1.0)
}

/// Row fill consistency (lower CV → higher score).
pub fn row_consistency_score(row_fill: &[u32]) -> f32 {
    if row_fill.is_empty() {
        return 0.0;
    }
    let mean = row_fill.iter().sum::<u32>() as f32 / row_fill.len() as f32;
    if mean < 1e-3 {
        return 0.0;
    }
    let var = row_fill
        .iter()
        .map(|&n| (n as f32 - mean).powi(2))
        .sum::<f32>()
        / row_fill.len() as f32;
    let cv = var.sqrt() / mean;
    (1.0 - cv.min(1.5) / 1.5).clamp(0.0, 1.0)
}

/// Runs whose centers fall inside a rect (with pad).
pub fn runs_in_rect(runs: &[TextRun], r: Rect, pad: f32) -> Vec<&TextRun> {
    runs.iter()
        .filter(|t| {
            if t.text.trim().is_empty() {
                return false;
            }
            let cx = (t.bbox.x0 + t.bbox.x1) * 0.5;
            let cy = (t.bbox.y0 + t.bbox.y1) * 0.5;
            cx >= r.x0 - pad && cx <= r.x1 + pad && cy >= r.y0 - pad && cy <= r.y1 + pad
        })
        .collect()
}

/// IoU of two rects.
pub fn iou(a: Rect, b: Rect) -> f32 {
    let x0 = a.x0.max(b.x0);
    let y0 = a.y0.max(b.y0);
    let x1 = a.x1.min(b.x1);
    let y1 = a.y1.min(b.y1);
    let w = (x1 - x0).max(0.0);
    let h = (y1 - y0).max(0.0);
    let inter = w * h;
    let ua = a.width() * a.height() + b.width() * b.height() - inter;
    if ua <= 0.0 {
        0.0
    } else {
        inter / ua
    }
}
