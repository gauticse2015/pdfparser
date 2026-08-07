//! Shared vertical stack-merge for same-column table fragments.
//!
//! Lattice (Engine V2) and network stream both rejoin header/body splits that
//! the detector emitted as two tables. One implementation keeps the geometry
//! policy in one place (DRY / ISP).

use crate::types::{Table, TableMethod};

/// Geometry thresholds for a stack merge pass.
#[derive(Debug, Clone, Copy)]
pub(crate) struct StackMergePolicy {
    /// Minimum column count on both fragments.
    pub min_cols: u32,
    /// Inclusive max combined row count after merge.
    pub max_total_rows: u32,
    /// Minimum fractional x-overlap (intersection / min width).
    pub min_x_overlap: f32,
    /// Max width ratio (wider/narrower); larger ⇒ distinct side-by-side grids.
    pub max_width_ratio: f32,
    /// Allow a small negative gap (slight bbox overlap).
    pub gap_lo: f32,
    /// Diagnostic note prefix (`lattice_stack_merge` / `stream_stack_merge`).
    pub note_prefix: &'static str,
    /// Copy typed text-recovery flags from the lower fragment (lattice).
    pub copy_text_recovery: bool,
}

/// Both fragments are lattice.
pub(crate) fn methods_ok_lattice(a: &Table, b: &Table) -> bool {
    a.method == TableMethod::Lattice && b.method == TableMethod::Lattice
}

/// Both fragments are borderless (stream / dense-numeric).
pub(crate) fn methods_ok_stream(a: &Table, b: &Table) -> bool {
    matches!(a.method, TableMethod::Stream | TableMethod::DenseNumeric)
        && matches!(b.method, TableMethod::Stream | TableMethod::DenseNumeric)
}

/// Merge vertically stacked same-column tables (PDF y-up, top-first).
///
/// `max_gap_for(prev)` supplies the per-pair gap ceiling (row-height based for
/// lattice, font-based for stream).
pub(crate) fn merge_stacked_same_col(
    mut tabs: Vec<Table>,
    methods_ok: impl Fn(&Table, &Table) -> bool,
    max_gap_for: impl Fn(&Table) -> f32,
    policy: StackMergePolicy,
) -> Vec<Table> {
    if tabs.len() <= 1 {
        return tabs;
    }
    tabs.sort_by(|a, b| {
        a.page.cmp(&b.page).then_with(|| {
            b.bbox
                .y1
                .partial_cmp(&a.bbox.y1)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| {
                    a.bbox
                        .x0
                        .partial_cmp(&b.bbox.x0)
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
        })
    });
    let mut out: Vec<Table> = Vec::new();
    for t in tabs {
        if out.is_empty() {
            out.push(t);
            continue;
        }
        let prev = out.last().unwrap();
        let same_page = prev.page == t.page;
        let same_cols = prev.cols == t.cols && prev.cols >= policy.min_cols;
        let gap = prev.bbox.y0 - t.bbox.y1;
        let x0 = prev.bbox.x0.max(t.bbox.x0);
        let x1 = prev.bbox.x1.min(t.bbox.x1);
        let ov = (x1 - x0).max(0.0) / prev.bbox.width().min(t.bbox.width()).max(1.0);
        let w_ratio = {
            let pw = prev.bbox.width().max(1.0);
            let tw = t.bbox.width().max(1.0);
            (pw / tw).max(tw / pw)
        };
        let max_gap = max_gap_for(prev);
        if methods_ok(prev, &t)
            && same_page
            && same_cols
            && gap >= policy.gap_lo
            && gap <= max_gap
            && ov >= policy.min_x_overlap
            && w_ratio <= policy.max_width_ratio
            && prev.rows + t.rows <= policy.max_total_rows
        {
            let merged = merge_pair(prev, &t, policy);
            *out.last_mut().unwrap() = merged;
        } else {
            out.push(t);
        }
    }
    out
}

fn merge_pair(prev: &Table, t: &Table, policy: StackMergePolicy) -> Table {
    let mut merged = prev.clone();
    let skip_header = t.rows >= 1
        && prev.rows >= 1
        && (0..prev.cols as usize).all(|c| {
            let a = prev
                .cells
                .iter()
                .find(|cell| cell.row == 0 && cell.col == c as u32)
                .map(|cell| cell.text.trim())
                .unwrap_or("");
            let b = t
                .cells
                .iter()
                .find(|cell| cell.row == 0 && cell.col == c as u32)
                .map(|cell| cell.text.trim())
                .unwrap_or("");
            !a.is_empty() && a.eq_ignore_ascii_case(b)
        });
    let start = if skip_header { 1u32 } else { 0u32 };
    let off = prev.rows;
    for cell in &t.cells {
        if cell.row < start {
            continue;
        }
        let mut nc = cell.clone();
        nc.row = cell.row - start + off;
        merged.cells.push(nc);
    }
    let added = t.rows.saturating_sub(start);
    merged.rows = off + added;
    merged.bbox.x0 = prev.bbox.x0.min(t.bbox.x0);
    merged.bbox.y0 = t.bbox.y0.min(prev.bbox.y0);
    merged.bbox.x1 = prev.bbox.x1.max(t.bbox.x1);
    merged.bbox.y1 = prev.bbox.y1.max(t.bbox.y1);
    merged.confidence = prev.confidence.max(t.confidence) * 0.98;
    if policy.copy_text_recovery {
        merged.text_row_recovery = prev.text_row_recovery || t.text_row_recovery;
        merged.text_col_recovery = prev.text_col_recovery || t.text_col_recovery;
    }
    merged
        .notes
        .push(format!("{} +{added}rows", policy.note_prefix));
    merged
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{TableCell, TableMethod};
    use pdfparser_ir::Rect;

    fn tab(method: TableMethod, cols: u32, rows: u32, y0: f32, y1: f32) -> Table {
        let mut cells = Vec::new();
        for r in 0..rows {
            for c in 0..cols {
                cells.push(TableCell {
                    row: r,
                    col: c,
                    rowspan: 1,
                    colspan: 1,
                    bbox: Rect::zero(),
                    text: if r == 0 {
                        format!("H{c}")
                    } else {
                        format!("r{r}c{c}")
                    },
                    is_header: r == 0,
                    confidence: 1.0,
                });
            }
        }
        let mut t = Table::fixture(method, rows, cols, cells, 0.9);
        t.bbox = Rect {
            x0: 10.0,
            y0,
            x1: 110.0,
            y1,
        };
        t
    }

    #[test]
    fn merges_stacked_lattice_skipping_dup_header() {
        let a = tab(TableMethod::Lattice, 3, 2, 80.0, 120.0);
        let b = tab(TableMethod::Lattice, 3, 3, 20.0, 70.0);
        let out = merge_stacked_same_col(
            vec![a, b],
            methods_ok_lattice,
            |_| 40.0,
            StackMergePolicy {
                min_cols: 2,
                max_total_rows: 100,
                min_x_overlap: 0.70,
                max_width_ratio: 1.25,
                gap_lo: -2.0,
                note_prefix: "lattice_stack_merge",
                copy_text_recovery: true,
            },
        );
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].rows, 4); // 2 + (3-1 header)
    }
}
