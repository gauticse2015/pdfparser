//! Cell-content statistics shared by form, stream, and orchestrator gates.
//!
//! **Kept** as a standalone module (not folded into options). One implementation
//! of fill / numeric / punctuation / mean-length so detectors cannot drift
//! (ISP: consumers take [`CellStats`], not the whole options bag).

use crate::types::Table;

/// Precomputed content signals for a candidate table.
#[derive(Debug, Clone, Copy)]
pub struct CellStats {
    /// Non-empty cell fraction over the cell list (0..1).
    pub fill_rate: f32,
    /// Fraction of non-empty cells that look numeric.
    pub numeric_density: f32,
    /// Mean character count of non-empty cells.
    pub mean_chars: f32,
    /// Sentence-punctuation density over non-whitespace chars.
    pub punctuation_density: f32,
    /// Count of non-empty cells.
    pub filled: usize,
}

impl CellStats {
    /// Compute stats from a table's cell list.
    pub fn from_table(t: &Table) -> Self {
        let mut filled_n = 0usize;
        let mut numeric_n = 0usize;
        let mut char_sum = 0usize;
        let mut punct = 0u32;
        let mut non_ws = 0u32;
        for c in &t.cells {
            let text = c.text.as_str();
            let trim = text.trim();
            if trim.is_empty() {
                continue;
            }
            filled_n += 1;
            char_sum += trim.chars().count();
            if is_numeric_token(trim) {
                numeric_n += 1;
            }
            for ch in text.chars() {
                if ch.is_whitespace() {
                    continue;
                }
                non_ws += 1;
                if matches!(ch, '.' | '?' | '!' | ',' | ';' | ':') {
                    punct += 1;
                }
            }
        }
        let denom = t.cells.len().max(1) as f32;
        let filled_f = filled_n.max(1) as f32;
        Self {
            fill_rate: filled_n as f32 / denom,
            numeric_density: if filled_n == 0 {
                0.0
            } else {
                numeric_n as f32 / filled_f
            },
            mean_chars: if filled_n == 0 {
                0.0
            } else {
                char_sum as f32 / filled_f
            },
            punctuation_density: if non_ws == 0 {
                0.0
            } else {
                punct as f32 / non_ws as f32
            },
            filled: filled_n,
        }
    }
}

/// Token looks like a number / currency / percent (shared detector heuristic).
pub fn is_numeric_token(s: &str) -> bool {
    let t = s
        .trim()
        .trim_matches(|c: char| c == '$' || c == '%' || c == '(' || c == ')');
    if t.is_empty() {
        return false;
    }
    let mut has_digit = false;
    for ch in t.chars() {
        if ch.is_ascii_digit() {
            has_digit = true;
        } else if !matches!(ch, '.' | ',' | '-' | '+' | ' ') {
            return false;
        }
    }
    has_digit
}

/// Looser numeric cell: mixed labels like `GDP 5.8` still count (digits ≥ alpha).
pub fn is_numeric_token_loose(s: &str) -> bool {
    let t = s
        .trim()
        .trim_matches(|ch: char| ch == '$' || ch == '%' || ch == '(' || ch == ')' || ch == ',');
    if t.is_empty() {
        return false;
    }
    let digits = t.chars().filter(|ch| ch.is_ascii_digit()).count();
    let alpha = t.chars().filter(|ch| ch.is_alphabetic()).count();
    digits >= 1 && digits >= alpha
}

impl CellStats {
    /// Digit-vs-alpha numeric density (ownership / chrome gates).
    pub fn loose_numeric_density(t: &Table) -> f32 {
        let mut ne = 0u32;
        let mut num = 0u32;
        for c in &t.cells {
            let s = c.text.trim();
            if s.is_empty() {
                continue;
            }
            ne += 1;
            if is_numeric_token_loose(s) {
                num += 1;
            }
        }
        if ne == 0 {
            0.0
        } else {
            num as f32 / ne as f32
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Table, TableCell, TableMethod};
    use pdfparser_ir::Rect;

    fn t(cells: &[&str], cols: u32) -> Table {
        let rows = (cells.len() as u32).div_ceil(cols.max(1));
        Table {
            bbox: Rect {
                x0: 0.0,
                y0: 0.0,
                x1: 100.0,
                y1: 100.0,
            },
            page: 0,
            method: TableMethod::Stream,
            confidence: 0.8,
            rows,
            cols,
            cells: cells
                .iter()
                .enumerate()
                .map(|(i, s)| TableCell {
                    row: i as u32 / cols.max(1),
                    col: i as u32 % cols.max(1),
                    rowspan: 1,
                    colspan: 1,
                    bbox: Rect::zero(),
                    text: (*s).into(),
                    is_header: false,
                    confidence: 1.0,
                })
                .collect(),
            header_rows: 1,
            continued_from_previous_page: false,
            continued_to_next_page: false,
            logical_table_id: None,
            strategy_provenance: vec![],
            notes: vec![],
            edge_score: 0.0,
            fill_rate: 0.0,
            weak_edges: false,
            joint_count: 0,
            text_row_recovery: false,
            text_col_recovery: false,
            multitable_stream_recovery: false,
            stream_vs_overwide_hybrid: false,
        }
    }

    #[test]
    fn numeric_and_fill() {
        let s = CellStats::from_table(&t(&["A", "12", "3.4", "$5"], 2));
        assert_eq!(s.filled, 4);
        assert!((s.numeric_density - 0.75).abs() < 1e-5);
        assert!(s.mean_chars > 1.0);
    }
}
