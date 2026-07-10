//! Side-by-side anti over-segmentation: split fused tables on empty gutters.
use crate::geom::bbox_of_cells;
use crate::options::TableOptions;
use crate::types::{PipelineId, Table, TableCell, TableMethod};
use pdfparser_ir::TextRun;

/// Split tables that fuse two side-by-side grids via an empty gutter column.
pub fn split_side_by_side(
    tables: Vec<Table>,
    runs: &[TextRun],
    opts: &TableOptions,
) -> Vec<Table> {
    let mut out = Vec::new();
    for t in tables {
        if let Some((left, right)) = try_split_gutter(&t, runs, opts) {
            out.push(left);
            out.push(right);
        } else {
            out.push(t);
        }
    }
    out
}

fn try_split_gutter(
    t: &Table,
    _runs: &[TextRun],
    opts: &TableOptions,
) -> Option<(Table, Table)> {
    // Only split ruled lattices (stream/hybrid over-width handled by NMS).
    if t.cols < 4 || t.rows < 2 {
        return None;
    }
    if !matches!(t.method, TableMethod::Lattice) {
        return None;
    }

    let mut col_fill = vec![0u32; t.cols as usize];
    let mut col_total = vec![0u32; t.cols as usize];
    for c in &t.cells {
        if (c.col as usize) < col_fill.len() {
            col_total[c.col as usize] += 1;
            if !c.text.trim().is_empty() {
                col_fill[c.col as usize] += 1;
            }
        }
    }

    let mut gutter: Option<usize> = None;
    for col in 1..(t.cols as usize - 1) {
        let fill = if col_total[col] == 0 {
            0.0
        } else {
            col_fill[col] as f32 / col_total[col] as f32
        };
        if fill > 0.15 {
            continue;
        }
        let left_ok = (0..col).any(|c| col_fill[c] >= 2);
        let right_ok = ((col + 1)..t.cols as usize).any(|c| col_fill[c] >= 2);
        if left_ok && right_ok {
            gutter = match gutter {
                None => Some(col),
                Some(g) => {
                    if col_width(t, col) > col_width(t, g) {
                        Some(col)
                    } else {
                        Some(g)
                    }
                }
            };
        }
    }
    let g = gutter?;

    let left_x1 = t
        .cells
        .iter()
        .filter(|c| (c.col as usize) < g && !c.text.trim().is_empty())
        .map(|c| c.bbox.x1)
        .fold(f32::MIN, f32::max);
    let right_x0 = t
        .cells
        .iter()
        .filter(|c| (c.col as usize) > g && !c.text.trim().is_empty())
        .map(|c| c.bbox.x0)
        .fold(f32::MAX, f32::min);
    if left_x1 == f32::MIN || right_x0 == f32::MAX {
        return None;
    }
    let gap = right_x0 - left_x1;
    let mut ws: Vec<f32> = (0..t.cols as usize)
        .filter(|&c| c != g)
        .map(|c| col_width(t, c))
        .filter(|&w| w > 1.0)
        .collect();
    ws.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let median_w = if ws.is_empty() {
        opts.min_gutter_gap
    } else {
        ws[ws.len() / 2]
    };
    if gap < opts.min_gutter_gap || gap < median_w * opts.min_gutter_vs_col {
        return None;
    }

    let left = extract_col_range(t, 0, g)?;
    let right = extract_col_range(t, g + 1, t.cols as usize)?;
    if left.cols >= 2 && right.cols >= 2 && left.rows >= 2 && right.rows >= 2 {
        Some((left, right))
    } else {
        None
    }
}

fn col_width(t: &Table, col: usize) -> f32 {
    t.cells
        .iter()
        .filter(|c| c.col as usize == col)
        .map(|c| c.bbox.width())
        .fold(0.0, f32::max)
}

fn extract_col_range(t: &Table, col0: usize, col1: usize) -> Option<Table> {
    if col1 <= col0 {
        return None;
    }
    let mut cells: Vec<TableCell> = t
        .cells
        .iter()
        .filter(|c| {
            let col = c.col as usize;
            col >= col0 && col < col1
        })
        .map(|c| {
            let mut nc = c.clone();
            nc.col = c.col - col0 as u32;
            nc
        })
        .collect();
    if cells.is_empty() {
        return None;
    }
    let max_row = cells.iter().map(|c| c.row).max().unwrap_or(0);
    let mut keep_rows = vec![false; (max_row + 1) as usize];
    for c in &cells {
        if !c.text.trim().is_empty() {
            keep_rows[c.row as usize] = true;
        }
    }
    let mut row_map = vec![None; keep_rows.len()];
    let mut nr = 0u32;
    for (i, &k) in keep_rows.iter().enumerate() {
        if k {
            row_map[i] = Some(nr);
            nr += 1;
        }
    }
    cells.retain(|c| row_map[c.row as usize].is_some());
    for c in &mut cells {
        c.row = row_map[c.row as usize].unwrap();
        c.is_header = c.row == 0;
    }
    if nr < 2 {
        return None;
    }
    let max_col = cells.iter().map(|c| c.col).max().unwrap_or(0) + 1;
    let bbox = bbox_of_cells(&cells);
    let mut notes = t.notes.clone();
    notes.push(format!("split_gutter cols {col0}..{col1}"));
    let mut prov = t.strategy_provenance.clone();
    if !prov.contains(&PipelineId::P4SideBySide) {
        prov.push(PipelineId::P4SideBySide);
    }
    Some(Table {
        bbox,
        page: t.page,
        method: t.method,
        confidence: t.confidence,
        rows: nr,
        cols: max_col,
        cells,
        header_rows: 1,
        continued_from_previous_page: false,
        continued_to_next_page: false,
        logical_table_id: None,
        strategy_provenance: prov,
        notes,
    })
}
