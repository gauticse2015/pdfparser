//! Form-vs-table discriminator and document-level over-segmentation scrub.
use crate::options::TableOptions;
use crate::types::{PipelineId, Table, TableMethod};

/// Document-level scrub for over-segmented non-data grids.
///
/// When candidate count exceeds `overseg_trigger`, keep only high-scoring
/// data-like tables (headers, mixed types, moderate cell length, low hex/formula
/// density). Purely feature-based; no document identity.
pub fn scrub_document_table_fps(tables: Vec<Table>, opts: &TableOptions) -> Vec<Table> {
    if tables.is_empty() {
        return tables;
    }
    let trigger = opts.overseg_trigger.max(1) as usize;
    if tables.len() <= trigger {
        return tables;
    }

    let scored: Vec<(f32, Table)> = tables
        .into_iter()
        .map(|t| (data_table_score(&t), t))
        .collect();

    // Stricter bar under over-segmentation pressure
    let min_score = (opts.min_data_table_score + 0.12).min(0.85);
    let mut keep: Vec<(f32, Table)> = scored
        .into_iter()
        .filter(|(s, t)| *s >= min_score && is_plausible_data_table(t))
        .collect();
    keep.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

    // If nothing looks like a data table, emit none
    if keep.is_empty() {
        return Vec::new();
    }

    // Soft max under over-seg: prefer quality over volume
    let soft_cap = opts.overseg_trigger.max(1) as usize;
    let hard_cap = if opts.max_logical_tables == 0 {
        soft_cap
    } else {
        (opts.max_logical_tables as usize).min(soft_cap)
    };
    keep.truncate(hard_cap);
    keep.into_iter().map(|(_, t)| t).collect()
}

/// Extra geometric/content gates used only under over-segmentation.
fn is_plausible_data_table(t: &Table) -> bool {
    let mean = mean_cell_chars(t);
    let hex = hex_density(t);
    let hdr = header_alpha_ratio(t);
    let num = numeric_density(t);
    let code = code_like_density(t);
    if hex >= 0.40 {
        return false;
    }
    if code >= 0.28 {
        return false;
    }
    // Long prose in narrow layouts
    if mean >= 50.0 && t.cols <= 3 {
        return false;
    }
    // Dense short-token matrices without mixed cell lengths
    if mean < 8.0 && t.cols >= 5 {
        return false;
    }
    // Prefer tables with at least some alphabetic headers
    if hdr < 0.25 && t.cols >= 3 {
        return false;
    }
    // Caption / figure label often masquerades as a grid on dense pages
    if looks_like_caption_table(t) {
        return false;
    }
    // Very low numeric + medium mean on wide grids → TOC/layout junk under over-seg
    if t.cols >= 6 && num < 0.12 && mean > 18.0 {
        return false;
    }
    true
}

fn code_like_density(t: &Table) -> f32 {
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
            let s = c.text.as_str();
            let code_chars = s
                .chars()
                .filter(|ch| matches!(ch, '=' | '[' | ']' | '{' | '}' | '\\' | '`' | '|' | ';'))
                .count();
            let total = s.chars().filter(|ch| !ch.is_whitespace()).count().max(1);
            code_chars as f32 / total as f32 > 0.08
                || s.contains("==")
                || s.contains("->")
                || s.contains("()")
        })
        .count();
    n as f32 / cells.len() as f32
}

fn looks_like_caption_table(t: &Table) -> bool {
    let first: String = t
        .cells
        .iter()
        .filter(|c| c.row == 0 && !c.text.trim().is_empty())
        .map(|c| c.text.trim().to_ascii_lowercase())
        .collect::<Vec<_>>()
        .join(" ");
    first.starts_with("figure ")
        || first.starts_with("fig.")
        || first.starts_with("table ")
        || first.starts_with("listing ")
}

/// Score how much a candidate looks like a data table (0..1).
fn data_table_score(t: &Table) -> f32 {
    let fill = fill_rate(t);
    let num = numeric_density(t);
    let mean_chars = mean_cell_chars(t);
    let header = header_alpha_ratio(t);
    let hexish = hex_density(t);
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
    s += 0.10 * if (3..=12).contains(&t.cols) { 1.0 } else { 0.3 };
    s += 0.10 * if (3..=80).contains(&t.rows) { 1.0 } else { 0.2 };
    // Penalize formula / hex-matrix style cells
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
            let s: String = c.text.chars().filter(|ch| !ch.is_whitespace()).collect();
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

/// Apply form-likeness penalties / hard vetoes (page-level).
pub fn apply_form_discriminator(tables: Vec<Table>, opts: &TableOptions) -> Vec<Table> {
    let min_drop = opts.min_table_confidence * 0.7;
    tables
        .into_iter()
        .filter_map(|mut t| {
            let form = form_likeness(&t);
            let num = numeric_density(&t);
            let fill = fill_rate(&t);
            let mean_chars = mean_cell_chars(&t);

            let looks_like_data = (fill >= 0.45
                && num >= 0.15
                && t.cols >= 2
                && t.rows >= 2
                && mean_chars < 40.0)
                || (num >= 0.35 && t.cols >= 3 && mean_chars < 35.0)
                || (t.method == TableMethod::Lattice
                    && fill >= 0.5
                    && t.cols >= 2
                    && num >= 0.1);

            if !looks_like_data
                && form >= 0.75
                && num < 0.15
                && t.method != TableMethod::Structure
            {
                return None;
            }
            // Narrow stream with long paragraph cells → prose layout, not a grid
            if t.method == TableMethod::Stream
                && t.cols <= 2
                && mean_chars >= opts.stream_max_prose_mean_chars * 0.8
                && num < 0.20
            {
                return None;
            }
            if !looks_like_data && form >= 0.55 {
                t.confidence = (t.confidence * (1.0 - 0.55 * form)).clamp(0.0, 1.0);
                t.strategy_provenance.push(PipelineId::P1FormDisc);
                t.notes.push(format!("form_likeness={form:.2}"));
            }
            if t.confidence < min_drop.max(0.40) {
                return None;
            }
            if t.rows >= 20 && t.cols >= 6 && num < 0.2 && fill < 0.35 {
                return None;
            }
            if !looks_like_data
                && t.rows * t.cols >= 40
                && num < 0.12
                && mean_chars < 12.0
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

fn form_likeness(t: &Table) -> f32 {
    let fill = fill_rate(t);
    let num = numeric_density(t);
    let mean_chars = mean_cell_chars(t);
    let long_cell = if mean_chars >= 40.0 {
        1.0
    } else {
        mean_chars / 40.0
    };
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
