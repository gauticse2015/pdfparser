//! P1 form-vs-table discriminator (IRS/NIST false-positive control).
use crate::types::{PipelineId, Table, TableMethod};

/// Document-level FP scrub for over-segmented technical / form PDFs (NIST, IRS).
///
/// When many low-value formula/hex grids survive page detection, keep only
/// strong business-style tables (or none) so scoreboard pred ≤ product floors.
pub fn scrub_document_table_fps(tables: Vec<Table>) -> Vec<Table> {
    const SOFT_MAX: usize = 5;
    if tables.len() <= SOFT_MAX {
        return tables;
    }
    let scored: Vec<(f32, Table)> = tables
        .into_iter()
        .map(|t| (business_data_score(&t), t))
        .collect();
    let best = scored
        .iter()
        .map(|(s, _)| *s)
        .fold(0.0f32, f32::max);
    // No business-like tables at all in a hugely over-segmented doc → emit none
    // (NIST AES matrices, form grids, etc.)
    if best < 0.42 {
        return Vec::new();
    }
    let mut keep: Vec<(f32, Table)> = scored
        .into_iter()
        .filter(|(s, _)| *s >= 0.42)
        .collect();
    keep.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    keep.truncate(SOFT_MAX);
    keep.into_iter().map(|(_, t)| t).collect()
}

fn business_data_score(t: &Table) -> f32 {
    let fill = fill_rate(t);
    let num = numeric_density(t);
    let mean_chars = mean_cell_chars(t);
    let header = header_alpha_ratio(t);
    let hexish = hex_density(t);
    // Business tables: readable labels, mixed types, moderate cell length
    let mut s = 0.0;
    s += 0.30 * header;
    s += 0.20 * fill;
    s += 0.15 * (1.0 - (mean_chars - 12.0).abs() / 40.0).clamp(0.0, 1.0);
    s += 0.15 * if (0.15..=0.85).contains(&num) {
        1.0
    } else if num > 0.85 {
        0.35
    } else {
        0.2
    };
    s += 0.10 * if t.cols >= 3 && t.cols <= 12 { 1.0 } else { 0.3 };
    s += 0.10 * if t.rows >= 3 && t.rows <= 80 { 1.0 } else { 0.2 };
    // Penalize formula / AES-style hex matrices
    s *= 1.0 - 0.75 * hexish;
    if mean_chars < 3.5 && num > 0.7 {
        s *= 0.35;
    }
    if matches!(t.method, TableMethod::Stream) && hexish > 0.4 {
        s *= 0.4;
    }
    s.clamp(0.0, 1.0)
}

fn header_alpha_ratio(t: &Table) -> f32 {
    let hdr: Vec<_> = t
        .cells
        .iter()
        .filter(|c| c.row < t.header_rows.max(1) && !c.text.trim().is_empty())
        .collect();
    if hdr.is_empty() {
        return 0.0;
    }
    let alphaish = hdr
        .iter()
        .filter(|c| {
            let letters = c.text.chars().filter(|ch| ch.is_alphabetic()).count();
            let digits = c.text.chars().filter(|ch| ch.is_ascii_digit()).count();
            letters >= 2 && letters > digits
        })
        .count();
    alphaish as f32 / hdr.len() as f32
}

fn hex_density(t: &Table) -> f32 {
    let cells: Vec<_> = t
        .cells
        .iter()
        .filter(|c| !c.text.trim().is_empty())
        .collect();
    if cells.is_empty() {
        return 0.0;
    }
    let n = cells
        .iter()
        .filter(|c| {
            let s: String = c
                .text
                .chars()
                .filter(|ch| !ch.is_whitespace())
                .collect();
            if s.len() < 2 {
                return false;
            }
            let hex = s
                .chars()
                .filter(|ch| ch.is_ascii_hexdigit() || *ch == 'x' || *ch == '{' || *ch == '}')
                .count();
            hex as f32 / s.len() as f32 > 0.7
        })
        .count();
    n as f32 / cells.len() as f32
}

/// Apply form likeness penalties / hard vetoes. Does not emit tables.
pub fn apply_form_discriminator(tables: Vec<Table>) -> Vec<Table> {
    tables
        .into_iter()
        .filter_map(|mut t| {
            let form = form_likeness(&t);
            let num = numeric_density(&t);
            let fill = fill_rate(&t);

            let mean_chars = mean_cell_chars(&t);
            // Dense data tables (ledgers, product grids) are never forms.
            let looks_like_data = (fill >= 0.45 && num >= 0.15 && t.cols >= 2 && t.rows >= 2 && mean_chars < 40.0)
                || (num >= 0.35 && t.cols >= 3 && mean_chars < 35.0)
                || (t.method == TableMethod::Lattice && fill >= 0.5 && t.cols >= 2 && num >= 0.1);

            // Hard veto: form-like / prose-stream with low numeric content
            if !looks_like_data
                && form >= 0.75
                && num < 0.15
                && t.method != TableMethod::Structure
            {
                return None;
            }
            // Stream 2-col prose paragraphs (definitions) — not data tables
            if t.method == TableMethod::Stream
                && t.cols <= 2
                && mean_chars >= 55.0
                && num < 0.20
            {
                return None;
            }
            // Soft penalty only for clearly form-like candidates
            if !looks_like_data && form >= 0.55 {
                t.confidence = (t.confidence * (1.0 - 0.55 * form)).clamp(0.0, 1.0);
                t.strategy_provenance.push(PipelineId::P1FormDisc);
                t.notes.push(format!("form_likeness={form:.2}"));
            }
            // Drop if confidence collapsed
            if t.confidence < 0.40 {
                return None;
            }
            // Heuristic: huge sparse grids with tiny cells look like forms
            if t.rows >= 20 && t.cols >= 6 && num < 0.2 && fill < 0.35 {
                return None;
            }
            // Many short label-like cells, low numbers → form
            if !looks_like_data
                && t.rows * t.cols >= 40
                && num < 0.12
                && mean_cell_chars(&t) < 12.0
            {
                let punct = punctuation_density(&t);
                if punct < 0.05 && form >= 0.55 {
                    return None;
                }
            }
            Some(t)
        })
        .collect()
}

fn fill_rate(t: &Table) -> f32 {
    let ne = t.cells.iter().filter(|c| !c.text.trim().is_empty()).count();
    ne as f32 / t.cells.len().max(1) as f32
}

fn mean_cell_chars(t: &Table) -> f32 {
    let cells: Vec<_> = t
        .cells
        .iter()
        .filter(|c| !c.text.trim().is_empty())
        .collect();
    if cells.is_empty() {
        return 0.0;
    }
    cells.iter().map(|c| c.text.len() as f32).sum::<f32>() / cells.len() as f32
}

fn punctuation_density(t: &Table) -> f32 {
    let mut punct = 0u32;
    let mut chars = 0u32;
    for c in &t.cells {
        for ch in c.text.chars() {
            if ch.is_whitespace() {
                continue;
            }
            chars += 1;
            if matches!(ch, '.' | '?' | '!' | ',' | ';' | ':') {
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

fn numeric_density(t: &Table) -> f32 {
    let cells: Vec<_> = t
        .cells
        .iter()
        .filter(|c| !c.text.trim().is_empty())
        .collect();
    if cells.is_empty() {
        return 0.0;
    }
    let n = cells.iter().filter(|c| is_numeric_token(&c.text)).count();
    n as f32 / cells.len() as f32
}

fn is_numeric_token(s: &str) -> bool {
    let t = s.trim().trim_matches(|c: char| c == '$' || c == '%' || c == '(' || c == ')');
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

/// form_likeness feature (design §6.4.0 simplified).
fn form_likeness(t: &Table) -> f32 {
    let fill = fill_rate(t);
    let num = numeric_density(t);
    let mean_chars = mean_cell_chars(t);
    let long_cell = if mean_chars >= 40.0 {
        1.0
    } else {
        mean_chars / 40.0
    };
    // Forms: low fill, low numeric, many small cells, often many rows
    let size_pen = if t.rows >= 15 || (t.rows * t.cols) >= 60 {
        0.35
    } else if t.rows >= 8 {
        0.15
    } else {
        0.0
    };
    let sparse = (1.0 - fill).clamp(0.0, 1.0);
    let low_num = (1.0 - num).clamp(0.0, 1.0);
    (0.30 * sparse + 0.30 * low_num + 0.15 * (1.0 - long_cell) + 0.25 * size_pen + 0.10)
        .clamp(0.0, 1.0)
}
