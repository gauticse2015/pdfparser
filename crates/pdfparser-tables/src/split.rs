//! Side-by-side anti over-segmentation: split fused tables on empty gutters.
use crate::geom::bbox_of_cells;
use crate::options::TableOptions;
use crate::types::{PipelineId, Table, TableCell, TableMethod};
use pdfparser_ir::TextRun;

/// Split tables that fuse two side-by-side grids via an empty gutter column.
pub fn split_side_by_side(tables: Vec<Table>, runs: &[TextRun], opts: &TableOptions) -> Vec<Table> {
    let mut out = Vec::new();
    for t in tables {
        if let Some((left, right)) = try_split_gutter(&t, runs, opts) {
            // Phase 14: carving a 2-col strip off a wide multi-col lattice is
            // almost always a false gutter (sparse/empty interior columns).
            // Prefer the larger multi-col piece; if both are strips, keep original.
            if t.cols >= 6 && (left.cols <= 2 || right.cols <= 2) {
                let larger = if right.cols > left.cols
                    || (right.cols == left.cols && right.rows >= left.rows)
                {
                    right
                } else {
                    left
                };
                if larger.cols >= 3 && larger.rows >= 2 {
                    out.push(larger);
                } else {
                    out.push(t);
                }
                continue;
            }
            out.push(left);
            out.push(right);
        } else {
            out.push(t);
        }
    }
    out
}

fn try_split_gutter(t: &Table, _runs: &[TextRun], opts: &TableOptions) -> Option<(Table, Table)> {
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

    // Wide multi-col data tables often have 1–2 sparse interior columns
    // (optional fields). That is not a dual-table gutter. Only consider split
    // when sparse interior columns are a large share of the grid (true fuse).
    let ncols = t.cols as usize;
    let sparse_interior = (1..ncols.saturating_sub(1))
        .filter(|&col| {
            let fill = if col_total[col] == 0 {
                0.0
            } else {
                col_fill[col] as f32 / col_total[col] as f32
            };
            fill <= 0.15
        })
        .count();
    if ncols >= 6 && sparse_interior <= 2 {
        return None;
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
        edge_score: t.edge_score,
        fill_rate: t.fill_rate,
        weak_edges: t.weak_edges,
        joint_count: t.joint_count,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use pdfparser_ir::Rect;

    fn cell(row: u32, col: u32, text: &str, x0: f32, x1: f32) -> TableCell {
        TableCell {
            row,
            col,
            rowspan: 1,
            colspan: 1,
            bbox: Rect {
                x0,
                y0: 100.0 - row as f32 * 15.0,
                x1,
                y1: 112.0 - row as f32 * 15.0,
            },
            text: text.into(),
            is_header: row == 0,
            confidence: 0.9,
        }
    }

    /// 2×2 grids fused with empty gutter col 2: cols 0,1 | empty | 3,4
    fn fused_table() -> Table {
        let mut cells = Vec::new();
        // left 3 rows × 2 cols
        for r in 0..3u32 {
            cells.push(cell(r, 0, &format!("L{r}a"), 0.0, 40.0));
            cells.push(cell(r, 1, &format!("L{r}b"), 40.0, 80.0));
            cells.push(cell(r, 2, "", 80.0, 140.0)); // wide empty gutter
            cells.push(cell(r, 3, &format!("R{r}a"), 140.0, 180.0));
            cells.push(cell(r, 4, &format!("R{r}b"), 180.0, 220.0));
        }
        // clear gutter texts
        for c in &mut cells {
            if c.col == 2 {
                c.text.clear();
            }
        }
        Table {
            bbox: Rect {
                x0: 0.0,
                y0: 55.0,
                x1: 220.0,
                y1: 112.0,
            },
            page: 0,
            method: TableMethod::Lattice,
            confidence: 0.9,
            rows: 3,
            cols: 5,
            cells,
            header_rows: 1,
            continued_from_previous_page: false,
            continued_to_next_page: false,
            logical_table_id: None,
            strategy_provenance: vec![PipelineId::S2Lattice],
            notes: vec![],
            edge_score: 0.9,
            fill_rate: 0.8,
            weak_edges: false,
        joint_count: 0,
        }
    }

    #[test]
    fn split_side_by_side_gutter() {
        let t = fused_table();
        let opts = TableOptions {
            detect_tables: true,
            side_by_side_split: true,
            min_gutter_gap: 15.0,
            min_gutter_vs_col: 0.6,
            ..TableOptions::default()
        };
        let out = split_side_by_side(vec![t], &[], &opts);
        assert_eq!(
            out.len(),
            2,
            "shapes {:?}",
            out.iter().map(|x| (x.rows, x.cols)).collect::<Vec<_>>()
        );
        assert!(out.iter().all(|x| x.cols == 2));
        assert!(out
            .iter()
            .all(|x| x.strategy_provenance.contains(&PipelineId::P4SideBySide)));
    }

    #[test]
    fn no_split_stream() {
        let mut t = fused_table();
        t.method = TableMethod::Stream;
        let opts = TableOptions::default();
        let out = split_side_by_side(vec![t], &[], &opts);
        assert_eq!(out.len(), 1);
    }

    #[test]
    fn no_split_narrow() {
        let t = Table {
            bbox: Rect::zero(),
            page: 0,
            method: TableMethod::Lattice,
            confidence: 0.9,
            rows: 2,
            cols: 2,
            cells: vec![
                cell(0, 0, "a", 0.0, 10.0),
                cell(0, 1, "b", 10.0, 20.0),
                cell(1, 0, "c", 0.0, 10.0),
                cell(1, 1, "d", 10.0, 20.0),
            ],
            header_rows: 1,
            continued_from_previous_page: false,
            continued_to_next_page: false,
            logical_table_id: None,
            strategy_provenance: vec![],
            notes: vec![],
            edge_score: 0.9,
            fill_rate: 1.0,
            weak_edges: false,
        joint_count: 0,
        };
        let out = split_side_by_side(vec![t], &[], &TableOptions::default());
        assert_eq!(out.len(), 1);
    }
}


#[cfg(test)]
mod phase14_gutter {
    use super::*;
    use pdfparser_ir::Rect;

    fn lattice_grid(rows: u32, cols: u32) -> Table {
        let mut cells = Vec::new();
        for r in 0..rows {
            for c in 0..cols {
                let empty = c == 2 && r > 0; // sparse col 2 as fake gutter
                cells.push(TableCell {
                    row: r,
                    col: c,
                    rowspan: 1,
                    colspan: 1,
                    text: if empty { String::new() } else { format!("{r},{c}") },
                    bbox: Rect {
                        x0: c as f32 * 40.0,
                        y0: r as f32 * 12.0,
                        x1: c as f32 * 40.0 + 35.0,
                        y1: r as f32 * 12.0 + 10.0,
                    },
                    is_header: false,
                    confidence: 0.9,
                });
            }
        }
        Table {
            bbox: Rect { x0: 0.0, y0: 0.0, x1: cols as f32 * 40.0, y1: rows as f32 * 12.0 },
            page: 0,
            method: TableMethod::Lattice,
            confidence: 0.9,
            rows,
            cols,
            cells,
            header_rows: 0,
            continued_from_previous_page: false,
            continued_to_next_page: false,
            logical_table_id: None,
            strategy_provenance: vec![],
            notes: vec![],
            edge_score: 0.9,
            fill_rate: 0.7,
            weak_edges: false,
        joint_count: 0,
        }
    }

    #[test]
    fn refuse_split_wide_into_two_col_strip() {
        let opts = TableOptions::default();
        let t = lattice_grid(5, 10);
        // May or may not propose gutter; if it does, our guard must refuse leaving 2-col strip
        if let Some((l, r)) = try_split_gutter(&t, &[], &opts) {
            assert!(
                !(t.cols >= 6 && (l.cols <= 2 || r.cols <= 2)),
                "must not emit 2-col strip from wide grid"
            );
        }
    }
}
