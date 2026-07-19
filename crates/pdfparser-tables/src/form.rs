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
    s += 0.15
        * if (0.15..=0.85).contains(&num) {
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

            // Data tables are not only numeric: SKU / code / id grids are fully
            // filled ruled lattices with short alphanumeric cells and num≈0.
            // Do not require numeric_density for strong lattice candidates.
            let strong_lattice = t.method == TableMethod::Lattice
                && fill >= 0.50
                && t.cols >= 2
                && t.rows >= 3
                && !t.weak_edges;

            // Wide high-edge lattices with sparse/empty cells (rowspans, comment
            // columns) are real multi-col data tables — not form chrome.
            // Disease-outbreak / multi-table pages (Phase 14) hit fill < 0.5.
            let wide_ruled_data = t.method == TableMethod::Lattice
                && t.cols >= 6
                && t.rows >= 3
                && !t.weak_edges
                && t.edge_score >= 0.80
                && fill >= 0.18
                && (num >= 0.08 || mean_chars < 80.0);

            // Raster-recovered ruled grids may have zero text (ink is in the image).
            let is_raster = t.is_from_raster();
            let raster_lattice = t.method == TableMethod::Lattice
                && is_raster
                && t.cols >= 2
                && t.rows >= 2
                && !t.weak_edges;

            // Dense multi-col streams (names+ids+amounts) often have low pure-numeric
            // density because of dates/IDs — still real data tables.
            let dense_stream_data =
                matches!(t.method, TableMethod::Stream | TableMethod::DenseNumeric)
                    && fill >= 0.55
                    && t.cols >= 4
                    && t.rows >= 6
                    && mean_chars < 48.0
                    && t.confidence >= 0.55;

            let looks_like_data = (fill >= 0.45
                && num >= 0.15
                && t.cols >= 2
                && t.rows >= 2
                && mean_chars < 40.0)
                || (num >= 0.35 && t.cols >= 3 && mean_chars < 35.0)
                || (t.method == TableMethod::Lattice && fill >= 0.5 && t.cols >= 2 && num >= 0.1)
                || strong_lattice
                || wide_ruled_data
                || raster_lattice
                || dense_stream_data;

            // Phase-1: harder form veto (was 0.75) — chrome/forms over-detect.
            if !looks_like_data && form >= 0.52 && num < 0.20 && t.method != TableMethod::Structure
            {
                return None;
            }
            // Stream / borderless: prose and form-like grids hard-drop earlier.
            if t.method == TableMethod::Stream {
                // Dense multi-col data grids (campaign donors, liabilities, ACS):
                // high fill, moderate mean length — keep even when large.
                let dense_data_grid = t.cols >= 4
                    && t.rows >= 6
                    && fill >= 0.55
                    && mean_chars < 48.0
                    && t.confidence >= 0.55;
                // Structured: short tables OR large dense grids (rows>25 used to fail).
                let structured = t.cols >= 3
                    && t.rows >= 4
                    && fill >= 0.35
                    && (t.rows <= 25 || dense_data_grid || (fill >= 0.60 && t.cols >= 4));
                // Huge form worksheets (Schedule C style) — not dense data tables.
                // Phase-2: do not hard-drop high-fill multi-col streams solely on size.
                if t.rows >= 20 && t.cols >= 6 && form >= 0.35 && !dense_data_grid {
                    // Sparse/label form sheets still drop; keep filled grids.
                    if fill < 0.50 || num < 0.08 {
                        return None;
                    }
                }
                if t.rows * t.cols >= 200 && form >= 0.30 && !dense_data_grid {
                    if fill < 0.50 || (num < 0.10 && mean_chars > 30.0) {
                        return None;
                    }
                }
                // Paragraph prose (narrow + long cells)
                if mean_chars >= opts.stream_max_prose_mean_chars * 0.50
                    && num < 0.22
                    && t.cols <= 3
                {
                    return None;
                }
                // Form-likeness + low numeric (not multi-col structured grids)
                if form >= 0.48 && num < 0.20 && !structured && !dense_data_grid {
                    return None;
                }
                if form >= 0.55 && num < 0.25 && t.cols <= 3 {
                    return None;
                }
                // Tiny low-fill stream fragments on form pages
                if fill < 0.40 && t.rows <= 5 && num < 0.30 && t.cols <= 3 {
                    return None;
                }
                // IRS / label-value streams
                if t.cols <= 2 && t.rows <= 12 && num < 0.15 && mean_chars > 8.0 {
                    return None;
                }
                // Notice / standard metadata grids / IRS form worksheets
                let lower = t
                    .cells
                    .iter()
                    .take(40)
                    .map(|c| c.text.to_ascii_lowercase())
                    .collect::<Vec<_>>()
                    .join(" ");
                let irs_or_notice = lower.contains("name of standard")
                    || lower.contains("withdrawn")
                    || lower.contains("warning notice")
                    || lower.contains("social security")
                    || lower.contains("employer id")
                    || lower.contains("department of the treasury")
                    || lower.contains("omb no.")
                    || lower.contains("irs use only")
                    || lower.contains("accounting method")
                    || lower.contains("principal business or profession")
                    || lower.contains("business address")
                    || lower.contains("schedule c")
                    || lower.contains("schedule d")
                    || lower.contains("profit or loss from business")
                    || lower.contains("employer identification")
                    || lower.contains("proceeds (sales price)")
                    || lower.contains("cost (or other basis)")
                    || lower.contains("adjustments to gain or loss")
                    || lower.contains("capital gain or (loss)")
                    || lower.contains("form 1099-b")
                    || lower.contains("form 1099")
                    || lower.contains("short-term transactions")
                    || lower.contains("long-term transactions")
                    || lower.contains("totals for all short-term")
                    || lower.contains("totals for all long-term");
                if irs_or_notice && (num < 0.45 || t.rows <= 16 || fill < 0.70) {
                    return None;
                }
            }
            // Lattice Schedule D / tax capital-gains form fragments (sparse ruled)
            if t.method == TableMethod::Lattice {
                let lower = t
                    .cells
                    .iter()
                    .take(30)
                    .map(|c| c.text.to_ascii_lowercase())
                    .collect::<Vec<_>>()
                    .join(" ");
                if (lower.contains("proceeds (sales price)")
                    || lower.contains("cost (or other basis)")
                    || lower.contains("adjustments to gain or loss")
                    || lower.contains("schedule d"))
                    && num < 0.40
                {
                    return None;
                }
            }
            // Lattice chrome / form sheets (IRS Schedule C style): high form
            // likeness + sparse fill + low numeric. Keep dense filled lattices
            // (nested outer forms with full cells still pass looks_like_data or
            // have fill high enough).
            if t.method == TableMethod::Lattice && !looks_like_data {
                if fill < 0.22 && num < 0.10 && t.rows * t.cols >= 8 {
                    return None;
                }
                // Form worksheets: many blank cells, label text, form score high
                if form >= 0.60 && fill < 0.50 && num < 0.18 && t.cols >= 4 {
                    return None;
                }
            }
            if !looks_like_data && form >= 0.50 {
                t.confidence = (t.confidence * (1.0 - 0.60 * form)).clamp(0.0, 1.0);
                t.strategy_provenance.push(PipelineId::P1FormDisc);
                t.notes.push(format!("form_likeness={form:.2}"));
            }
            if t.confidence < min_drop.max(0.45) {
                return None;
            }
            // Huge empty non-raster grids with no numeric signal → form/layout junk.
            if !raster_lattice
                && fill < 0.20
                && num < 0.15
                && t.rows * t.cols >= 40
                && t.method == TableMethod::Lattice
                && t.weak_edges
            {
                return None;
            }
            // Dense short-token veto: aimed at empty form chrome / code dumps
            // mistaken as stream grids. Never apply to strong filled lattices
            // (SKU tables, id matrices) — those are real data.
            if !looks_like_data
                && !strong_lattice
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
    // Size alone must not mark a dense multi-row data grid as form-like
    // (campaign-donor / liability class tables are 30–60 rows of real data).
    // Apply size penalty only when the grid is also sparse/empty-ish.
    let size_pen = if fill >= 0.55 && t.cols >= 3 {
        0.0
    } else if t.rows >= 15 || (t.rows * t.cols) >= 60 {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{PipelineId, TableCell};
    use pdfparser_ir::Rect;

    fn cell(row: u32, col: u32, text: &str) -> TableCell {
        TableCell {
            row,
            col,
            rowspan: 1,
            colspan: 1,
            bbox: Rect {
                x0: col as f32 * 40.0,
                y0: 100.0 - row as f32 * 12.0,
                x1: (col as f32 + 1.0) * 40.0,
                y1: 112.0 - row as f32 * 12.0,
            },
            text: text.into(),
            is_header: row == 0,
            confidence: 0.9,
        }
    }

    fn grid(method: TableMethod, rows: u32, cols: u32, texts: &[&str]) -> Table {
        assert_eq!(texts.len(), (rows * cols) as usize);
        let mut cells = Vec::new();
        for r in 0..rows {
            for c in 0..cols {
                let i = (r * cols + c) as usize;
                cells.push(cell(r, c, texts[i]));
            }
        }
        Table {
            bbox: Rect {
                x0: 0.0,
                y0: 0.0,
                x1: cols as f32 * 40.0,
                y1: rows as f32 * 12.0,
            },
            page: 0,
            method,
            confidence: 0.85,
            rows,
            cols,
            cells,
            header_rows: 1,
            continued_from_previous_page: false,
            continued_to_next_page: false,
            logical_table_id: None,
            strategy_provenance: vec![PipelineId::S2Lattice],
            notes: vec![],
            edge_score: 0.9,
            fill_rate: 0.9,
            weak_edges: false,
            joint_count: 0,
            text_row_recovery: false,
            text_col_recovery: false,
            multitable_stream_recovery: false,
            stream_vs_overwide_hybrid: false,
        }
    }

    #[test]
    fn form_disc_keeps_numeric_data_grid() {
        let t = grid(
            TableMethod::Lattice,
            3,
            3,
            &["A", "B", "C", "1", "2", "3", "4", "5", "6"],
        );
        let mut opts = TableOptions::default();
        opts.detect_tables = true;
        opts.form_discriminator = true;
        let out = apply_form_discriminator(vec![t], &opts);
        assert_eq!(out.len(), 1);
    }

    #[test]
    fn form_disc_keeps_large_dense_stream_grid() {
        // Campaign-donor class: many rows, multi-col, high fill — not IRS form.
        let cols = 7u32;
        let rows = 24u32;
        let mut texts = Vec::new();
        for r in 0..rows {
            for c in 0..cols {
                if r == 0 {
                    texts.push(format!("H{c}"));
                } else if c >= 4 {
                    texts.push(format!("{}", 1000 + r * 10 + c));
                } else {
                    texts.push(format!("Name{r}_{c}"));
                }
            }
        }
        let refs: Vec<&str> = texts.iter().map(|s| s.as_str()).collect();
        let mut t = grid(TableMethod::Stream, rows, cols, &refs);
        t.fill_rate = 1.0;
        t.confidence = 0.95;
        let mut opts = TableOptions::default();
        opts.detect_tables = true;
        opts.min_table_confidence = 0.55;
        opts.form_discriminator = true;
        opts.min_confidence_stream = 0.62;
        let out = apply_form_discriminator(vec![t], &opts);
        assert_eq!(
            out.len(),
            1,
            "dense multi-col stream must survive form disc"
        );
    }

    #[test]
    fn form_disc_drops_sparse_form_chrome() {
        // Sparse, non-numeric, form-like labels → veto
        let mut t = grid(
            TableMethod::Stream,
            4,
            2,
            &["Name:", "", "Address:", "", "Phone:", "", "Email:", ""],
        );
        t.confidence = 0.7;
        t.fill_rate = 0.3;
        // Make cells long-ish labels with empty partners
        t.cells[0].text = "Full legal name of applicant".into();
        t.cells[2].text = "Street address line one".into();
        t.cells[4].text = "Daytime phone number".into();
        t.cells[6].text = "Email address for contact".into();
        let mut opts = TableOptions::default();
        opts.detect_tables = true;
        opts.min_table_confidence = 0.55;
        opts.form_discriminator = true;
        let out = apply_form_discriminator(vec![t], &opts);
        // Form-like sparse stream should be dropped or heavily demoted out
        assert!(
            out.is_empty() || out[0].confidence < 0.55,
            "form chrome should not survive: {:?}",
            out.iter().map(|x| x.confidence).collect::<Vec<_>>()
        );
    }

    #[test]
    fn form_disc_drops_long_prose_stream() {
        let long =
            "This is a long paragraph of prose that should not look like a data table cell at all.";
        let mut t = grid(
            TableMethod::Stream,
            3,
            2,
            &[long, long, long, long, long, long],
        );
        t.fill_rate = 1.0;
        let mut opts = TableOptions::default();
        opts.detect_tables = true;
        opts.form_discriminator = true;
        opts.stream_max_prose_mean_chars = 70.0;
        let out = apply_form_discriminator(vec![t], &opts);
        assert!(out.is_empty(), "prose stream must be vetoed");
    }

    #[test]
    fn form_disc_keeps_strong_lattice_sku() {
        // SKU-like alphanumeric grid with low pure-numeric density
        let t = grid(
            TableMethod::Lattice,
            4,
            3,
            &[
                "SKU", "Desc", "Qty", "A1", "Widget", "2", "B2", "Gadget", "1", "C3", "Bolt", "5",
            ],
        );
        let mut opts = TableOptions::default();
        opts.detect_tables = true;
        opts.form_discriminator = true;
        let out = apply_form_discriminator(vec![t], &opts);
        assert_eq!(out.len(), 1, "SKU lattice is data");
    }

    #[test]
    fn scrub_overseg_drops_junk_when_many() {
        let good = grid(
            TableMethod::Lattice,
            4,
            4,
            &[
                "Metric", "Q1", "Q2", "Q3", "Rev", "10", "20", "30", "Cost", "5", "6", "7", "NI",
                "5", "14", "23",
            ],
        );
        let mut junks = Vec::new();
        for i in 0..10 {
            let mut j = grid(TableMethod::Stream, 2, 2, &["ab", "cd", "ef", "gh"]);
            j.confidence = 0.5;
            j.fill_rate = 0.2;
            j.notes.push(format!("junk{i}"));
            junks.push(j);
        }
        let mut all = junks;
        all.push(good);
        let mut opts = TableOptions::default();
        opts.detect_tables = true;
        opts.form_discriminator = true;
        opts.overseg_trigger = 4;
        opts.max_logical_tables = 4;
        opts.min_data_table_score = 0.42;
        let out = scrub_document_table_fps(all, &opts);
        assert!(!out.is_empty(), "should keep at least the data table");
        assert!(out.len() <= 4, "soft cap under overseg, got {}", out.len());
        let blob: String = out
            .iter()
            .flat_map(|t| t.cells.iter().map(|c| c.text.clone()))
            .collect::<Vec<_>>()
            .join(" ");
        assert!(
            blob.contains("Metric") || blob.contains("Rev"),
            "data lost: {blob}"
        );
    }

    #[test]
    fn scrub_noop_when_few_tables() {
        let t = grid(
            TableMethod::Lattice,
            3,
            3,
            &["A", "B", "C", "1", "2", "3", "4", "5", "6"],
        );
        let mut opts = TableOptions::default();
        opts.overseg_trigger = 8;
        let out = scrub_document_table_fps(vec![t.clone()], &opts);
        assert_eq!(out.len(), 1);
    }

    #[test]
    fn scrub_empty_input() {
        let opts = TableOptions::default();
        assert!(scrub_document_table_fps(vec![], &opts).is_empty());
    }

    #[test]
    fn scrub_drops_captionish_under_pressure() {
        let mut cap = grid(
            TableMethod::Stream,
            2,
            2,
            &["Figure 1 Overview", "x", "y", "z"],
        );
        cap.cells[0].text = "Figure 1 Overview of results".into();
        let mut good = grid(
            TableMethod::Lattice,
            4,
            3,
            &[
                "Name", "Val", "Unit", "A", "1", "kg", "B", "2", "kg", "C", "3", "kg",
            ],
        );
        good.fill_rate = 1.0;
        let mut many = vec![cap];
        for _ in 0..8 {
            let mut j = grid(TableMethod::Stream, 2, 6, &["a"; 12]);
            // long mean chars, low num → TOC-ish
            for c in &mut j.cells {
                c.text = "section heading text here".into();
            }
            many.push(j);
        }
        many.push(good);
        let mut opts = TableOptions::default();
        opts.overseg_trigger = 3;
        opts.max_logical_tables = 2;
        opts.min_data_table_score = 0.42;
        let out = scrub_document_table_fps(many, &opts);
        // Caption-like should not dominate; at most soft_cap kept
        assert!(out.len() <= 2);
        for t in &out {
            let first: String = t
                .cells
                .iter()
                .filter(|c| c.row == 0)
                .map(|c| c.text.clone())
                .collect::<Vec<_>>()
                .join(" ");
            assert!(
                !first.to_ascii_lowercase().starts_with("figure "),
                "caption survived: {first}"
            );
        }
    }

    #[test]
    fn form_likeness_and_hex_paths() {
        // Hex-matrix style cells
        let mut t = grid(
            TableMethod::Stream,
            3,
            3,
            &[
                "0xAB", "0xCD", "0xEF", "0x12", "0x34", "0x56", "0x78", "0x9A", "0xBC",
            ],
        );
        t.fill_rate = 1.0;
        let mut opts = TableOptions::default();
        opts.detect_tables = true;
        opts.form_discriminator = true;
        let _ = apply_form_discriminator(vec![t], &opts);
        // Code-like
        let code = grid(
            TableMethod::Stream,
            3,
            3,
            &[
                "a=1", "b->c", "fn()", "x[i]", "{y}", "z;", "a|b", "c`d", "e\\f",
            ],
        );
        let _ = apply_form_discriminator(vec![code], &opts);
    }
}
