//! S2 lattice detector: ruled grids from stroked segments + R9 cell text assign.
use crate::options::TableOptions;
use crate::types::{PipelineId, Table, TableCell, TableMethod};
use pdfparser_content::RuleSegment;
use pdfparser_ir::{Rect, TextRun};

/// Detect lattice tables on a page.
pub fn detect_lattice_tables(
    page_index: u32,
    runs: &[TextRun],
    rules: &[RuleSegment],
    opts: &TableOptions,
) -> Vec<Table> {
    let tol = opts.line_snap_tol;
    let min_cell = opts.min_cell_size;

    let mut hs: Vec<f32> = Vec::new();
    let mut vs: Vec<f32> = Vec::new();
    let mut h_segs: Vec<RuleSegment> = Vec::new();
    let mut v_segs: Vec<RuleSegment> = Vec::new();

    for r in rules {
        if r.is_horizontal(tol) {
            let y = (r.y0 + r.y1) * 0.5;
            hs.push(y);
            h_segs.push(*r);
        } else if r.is_vertical(tol) {
            let x = (r.x0 + r.x1) * 0.5;
            vs.push(x);
            v_segs.push(*r);
        }
    }

    let ys = cluster_coords(&hs, tol);
    let xs = cluster_coords(&vs, tol);

    if xs.len() < 2 || ys.len() < 2 {
        return Vec::new();
    }

    // Optional: find connected components / largest rectangular grid region.
    // Phase U: use full snapped line set if it forms a coherent grid with text.
    let mut tables = Vec::new();

    // Filter xs/ys to those that participate in intersections (length-supported).
    let xs = filter_lines_with_support(&xs, &ys, &v_segs, &h_segs, tol, true);
    let ys = filter_lines_with_support(&ys, &xs, &h_segs, &v_segs, tol, false);

    if xs.len() < 2 || ys.len() < 2 {
        return Vec::new();
    }

    // Rows top-to-bottom: PDF y-up so reverse y for row index 0 at top
    let mut y_top_to_bottom = ys.clone();
    y_top_to_bottom.sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));
    // fix Equal
    y_top_to_bottom.sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));

    let nrows = y_top_to_bottom.len() - 1;
    let ncols = xs.len() - 1;
    // Require a real grid (cols>=2, rows>=2). Outer-only frames are hybrid/S4 territory.
    if nrows < 2 || ncols < 2 {
        return Vec::new();
    }
    // Cap absurd grids
    if nrows > 80 || ncols > 40 {
        return Vec::new();
    }

    let mut cells = Vec::new();
    let mut filled = 0usize;
    for row in 0..nrows {
        let y_top = y_top_to_bottom[row];
        let y_bot = y_top_to_bottom[row + 1];
        // ensure top > bot in y-up
        let (y1, y0) = if y_top >= y_bot {
            (y_top, y_bot)
        } else {
            (y_bot, y_top)
        };
        if (y1 - y0) < min_cell {
            continue;
        }
        for col in 0..ncols {
            let x0 = xs[col];
            let x1 = xs[col + 1];
            if (x1 - x0) < min_cell {
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

    if cells.is_empty() {
        return Vec::new();
    }

    let total = cells.len().max(1);
    let fill_rate = filled as f32 / total as f32;
    // Require some text inside the grid (avoid empty decorative boxes)
    if fill_rate < 0.05 && filled < 2 {
        return Vec::new();
    }

    let bbox = cells
        .iter()
        .map(|c| c.bbox)
        .reduce(|a, b| a.union(b))
        .unwrap_or(Rect::zero());

    let grid_regularity = grid_regularity_score(&xs, &y_top_to_bottom);
    let rule_support = 0.85; // lattice by construction
    let alignment = 0.8;
    // Volume 2 lattice weights (R19)
    let confidence = (0.35 * grid_regularity
        + 0.25 * rule_support
        + 0.20 * fill_rate
        + 0.10 * alignment
        + 0.10 * (total as f32 / 6.0).min(1.0))
    .clamp(0.0, 1.0);

    // Actual row count from cells
    let max_row = cells.iter().map(|c| c.row).max().unwrap_or(0) + 1;
    let max_col = cells.iter().map(|c| c.col).max().unwrap_or(0) + 1;

    tables.push(Table {
        bbox,
        page: page_index,
        method: TableMethod::Lattice,
        confidence,
        rows: max_row,
        cols: max_col,
        cells,
        header_rows: 1,
        continued_from_previous_page: false,
        continued_to_next_page: false,
        logical_table_id: None,
        strategy_provenance: vec![PipelineId::S2Lattice],
        notes: vec![format!("lattice_lines xs={} ys={}", xs.len(), ys.len())],
    });

    tables
}

fn cluster_coords(vals: &[f32], tol: f32) -> Vec<f32> {
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

fn filter_lines_with_support(
    primary: &[f32],
    _cross: &[f32],
    segs: &[RuleSegment],
    _orth_segs: &[RuleSegment],
    tol: f32,
    vertical: bool,
) -> Vec<f32> {
    // Keep lines that have at least one segment near the coordinate with decent length
    primary
        .iter()
        .copied()
        .filter(|&c| {
            segs.iter().any(|s| {
                let coord = if vertical {
                    (s.x0 + s.x1) * 0.5
                } else {
                    (s.y0 + s.y1) * 0.5
                };
                (coord - c).abs() <= tol && s.len() >= 5.0
            })
        })
        .collect()
}

fn assign_text(runs: &[TextRun], cell: Rect) -> String {
    // R9: geometry assignment — center of run inside cell (with small inset tolerance)
    let pad = 0.5f32;
    let mut parts: Vec<(f32, f32, &str)> = Vec::new(); // y, x, text for reading order inside cell
    for r in runs {
        if r.text.trim().is_empty() {
            continue;
        }
        let cx = (r.bbox.x0 + r.bbox.x1) * 0.5;
        let cy = (r.bbox.y0 + r.bbox.y1) * 0.5;
        if cx >= cell.x0 - pad && cx <= cell.x1 + pad && cy >= cell.y0 - pad && cy <= cell.y1 + pad
        {
            parts.push((cy, cx, r.text.as_str()));
        }
    }
    // top-to-bottom, left-to-right
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

fn grid_regularity_score(xs: &[f32], ys: &[f32]) -> f32 {
    fn cv_gaps(coords: &[f32]) -> f32 {
        if coords.len() < 3 {
            return 0.0;
        }
        let gaps: Vec<f32> = coords.windows(2).map(|w| (w[1] - w[0]).abs()).collect();
        if gaps.is_empty() {
            return 0.0;
        }
        let mean = gaps.iter().sum::<f32>() / gaps.len() as f32;
        if mean < 1e-3 {
            return 1.0;
        }
        let var = gaps.iter().map(|g| (g - mean).powi(2)).sum::<f32>() / gaps.len() as f32;
        let std = var.sqrt();
        (std / mean).min(2.0)
    }
    let cv = 0.5 * (cv_gaps(xs) + cv_gaps(ys));
    (1.0 - cv * 0.5).clamp(0.0, 1.0)
}
