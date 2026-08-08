//! Span merge and dense cell emit for ruled lattices.
#![allow(clippy::needless_range_loop)]

use super::joints::Edges;
use crate::types::TableCell;
use pdfparser_ir::Rect;

pub(crate) struct RawCell {
    pub(crate) bbox: Rect,
    pub(crate) text: String,
    pub(crate) edges: Edges,
    pub(crate) active: bool,
    pub(crate) colspan: u32,
    pub(crate) rowspan: u32,
}

/// Merge adjacent empty/open-edge cells into colspan/rowspan masters.
pub(crate) fn merge_spans_dense(grid: &mut [Vec<RawCell>]) {
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

/// Emit a full rectangular cell matrix: active masters keep text + spans;
/// covered (inactive) slots are empty 1×1 cells for structure/gold alignment.
pub(crate) fn emit_cells_dense(grid: &[Vec<RawCell>]) -> (Vec<TableCell>, u32, u32) {
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
                is_header: r == 0 || (r == 1 && !cell.text.trim().is_empty() && r < 2),
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
