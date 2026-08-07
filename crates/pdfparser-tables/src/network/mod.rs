//! Network-class borderless tables (textline + column alignments).
//!
//! Production borderless path: textlines → table areas → per-area grid.
#![allow(clippy::too_many_arguments)]

mod build;
mod glued;

pub(crate) use build::build_table_from_lines;
pub(crate) use glued::*;

use crate::geom::median_font_size;
use crate::options::TableOptions;
use crate::types::Table;
#[cfg(test)]
use pdfparser_ir::Rect;
use pdfparser_ir::TextRun;

/// Detect borderless tables via textline network + table-area engine.
pub fn detect_network_tables(page_index: u32, runs: &[TextRun], opts: &TableOptions) -> Vec<Table> {
    if runs.len() < 6 {
        return Vec::new();
    }
    let fs_all = median_font_size(runs);
    let body: Vec<&TextRun> = runs
        .iter()
        .filter(|r| !r.text.trim().is_empty() && r.font_size <= fs_all * 1.35 + 0.5)
        .collect();
    if body.len() < 6 {
        return Vec::new();
    }

    let fs = {
        let mut v: Vec<f32> = body
            .iter()
            .map(|r| r.font_size)
            .filter(|s| *s > 0.0)
            .collect();
        if v.is_empty() {
            10.0
        } else {
            v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            v[v.len() / 2]
        }
    };
    // Page x-extent for narrow-band reject (from full body, not per-region).
    let page_width = estimate_page_width(&body);
    // Vertical band tol: at least ~⅔ em, but also a fraction of the median
    // body y-pitch so cells of one logical row that jitter by a few points
    // still coalesce (common in stream/export PDFs).
    let y_centers: Vec<f32> = {
        let mut v: Vec<f32> = body.iter().map(|r| r.bbox.y_center()).collect();
        v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        v
    };
    let median_row_pitch = {
        // Ignore sub-em micro-gaps (within-row glyph jitter); keep row-scale gaps.
        let mut gaps: Vec<f32> = y_centers
            .windows(2)
            .map(|w| w[1] - w[0])
            .filter(|&g| g > fs * 0.45 && g < fs * 8.0)
            .collect();
        if gaps.is_empty() {
            fs * 1.2
        } else {
            gaps.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            gaps[gaps.len() / 2]
        }
    };
    // Band tol: merge cells of one logical row (jitter < ~half row pitch).
    // Multipliers from TableTuning (document profiles).
    let tun = &opts.advanced.tuning;
    let y_tol = (tun.stream_y_tol_font_mult * fs)
        .max(2.5)
        .max(tun.stream_y_tol_pitch_mult * median_row_pitch)
        .min((0.9 * median_row_pitch).max(fs));
    let lines = build_textlines(&body, y_tol);
    let multi: Vec<&TextLine> = lines.iter().filter(|l| l.multi).collect();
    if multi.len() < opts.advanced.stream_min_body_bands.max(3) as usize {
        return Vec::new();
    }

    // Soft/hard gaps primarily from observed multi-line pitch so dense stream
    // tables (row pitch ≪ 4×fs) never hard-split mid-body. Font mult is a
    // floor for sparse layouts, not a ceiling that forces mid-table cuts.
    let multi_pitch = {
        let mut gaps: Vec<f32> = multi
            .windows(2)
            .map(|w| (w[0].y - w[1].y).abs())
            .filter(|&g| g > 0.5)
            .collect();
        if gaps.is_empty() {
            fs * 1.5
        } else {
            gaps.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            gaps[gaps.len() / 2]
        }
    };
    let soft_floor = (opts.advanced.stream_region_gap_font_mult * fs * 0.35)
        .max(opts.advanced.stream_region_gap_min * 0.5)
        .max(fs * 1.1);
    let soft_gap = (multi_pitch * tun.stream_soft_gap_pitch_mult).max(soft_floor);
    // Hard: large enough that normal body pitch never crosses it; still
    // separates distinct stacked tables with multi-row blank bands.
    let hard_gap = (multi_pitch * tun.stream_hard_gap_pitch_mult)
        .max(soft_gap * 2.5)
        .max(fs * 4.0);
    // Area proposal *before* per-region build — page-global filter collapses
    // multi-table pages into one skeleton if applied first. Section notes are
    // dropped per-area inside build_table_from_lines.
    let regions = propose_table_areas(&multi, soft_gap, hard_gap, fs);
    let min_multi = opts.advanced.stream_min_body_bands.max(3) as usize;
    let mut out = Vec::new();
    for region in regions {
        if region.len() < min_multi {
            continue;
        }
        // Note: orphan header attach (pull multi lines from above soft_gap) was
        // tried and rejected — it fused multi-region stream fixtures (59) and
        // inflated stream_table_07. Single-run multi-word headers are recovered
        // inside build_table_from_lines near-top keep instead.
        if let Some(mut t) = build_table_from_lines(page_index, &region, opts, fs, page_width) {
            strip_trailing_stream_footnotes(&mut t);
            crate::builders::ruled::trim_empty_border_rows_cols(&mut t);
            out.push(t);
        }
    }
    // Stack-merge same-col stream fragments split by mid-table note islands when
    // the gap is modest and column counts match (keeps multi-page shreds as one
    // logical stream table without inventing rows).
    out = merge_stacked_same_col_stream(out, hard_gap.max(fs * 8.0).max(soft_gap * 3.0));
    out
}

/// Drop trailing stream rows that are numbered footnotes, not data.
///
/// Pattern: last row has text only in col 0, starts with `N.` / `N)` list marker,
/// and data columns are empty. Does not invent or pad rows for count metrics.
fn strip_trailing_stream_footnotes(table: &mut crate::types::Table) {
    use crate::types::TableMethod;
    if !matches!(
        table.method,
        TableMethod::Stream | TableMethod::DenseNumeric
    ) {
        return;
    }
    let nrows = table.rows as usize;
    let ncols = table.cols as usize;
    if nrows < 5 || ncols < 2 || table.cells.is_empty() {
        return;
    }
    let mut grid: Vec<Vec<String>> = vec![vec![String::new(); ncols]; nrows];
    for c in &table.cells {
        let r = c.row as usize;
        let col = c.col as usize;
        if r < nrows && col < ncols && grid[r][col].is_empty() {
            grid[r][col] = c.text.clone();
        }
    }
    let is_trailing_note_row = |row: &[String]| -> bool {
        let c0 = row.first().map(|s| s.trim()).unwrap_or("");
        if c0.is_empty() {
            return false;
        }
        let data_filled = row.iter().skip(1).filter(|c| !c.trim().is_empty()).count();
        if data_filled > 0 {
            return false;
        }
        // "1. only countries…" / "2) note"
        let bytes = c0.as_bytes();
        let mut i = 0;
        while i < bytes.len() && bytes[i].is_ascii_digit() {
            i += 1;
        }
        // Numbered footnote (e.g. "1. See note below").
        if i > 0 && i <= 3 && matches!(bytes.get(i), Some(b'.') | Some(b')')) {
            return true;
        }
        // Long prose-only trailer with no data cells (stream foot notes like
        // "S. Typhimurium, representing 81% of all…"). Short labels stay.
        let alpha = c0.chars().filter(|c| c.is_alphabetic()).count();
        let digits = c0.chars().filter(|c| c.is_ascii_digit()).count();
        c0.chars().count() >= 28 && alpha >= 16 && alpha > digits.saturating_mul(2)
    };
    let mut cut = nrows;
    while cut > 3 {
        if is_trailing_note_row(&grid[cut - 1]) {
            cut -= 1;
        } else {
            break;
        }
    }
    if cut >= nrows || cut < 3 {
        return;
    }
    let n_stripped = nrows - cut;
    table.cells.retain(|c| (c.row as usize) < cut);
    table.rows = cut as u32;
    if !table.cells.is_empty() {
        table.bbox = crate::geom::bbox_of_cells(&table.cells);
    }
    let filled = table
        .cells
        .iter()
        .filter(|c| !c.text.trim().is_empty())
        .count();
    let total = (table.rows as usize)
        .saturating_mul(table.cols as usize)
        .max(1);
    table.fill_rate = filled as f32 / total as f32;
    table
        .notes
        .push(format!("stream_footnote_stripped n={n_stripped}"));
}

/// Merge vertically stacked Stream tables with equal column counts when the
/// gap between bboxes is modest. Preserves top→bottom order (PDF y descending).
fn merge_stacked_same_col_stream(
    tabs: Vec<crate::types::Table>,
    max_gap: f32,
) -> Vec<crate::types::Table> {
    crate::stack_merge::merge_stacked_same_col(
        tabs,
        crate::stack_merge::methods_ok_stream,
        move |_| max_gap * 1.5,
        crate::stack_merge::StackMergePolicy {
            min_cols: 3,
            max_total_rows: 120,
            min_x_overlap: 0.55,
            max_width_ratio: 1.35,
            gap_lo: -2.0,
            note_prefix: "stream_stack_merge",
            copy_text_recovery: false,
        },
    )
}

/// Horizontal span of body runs — used as a page-width estimate for area gates.
fn estimate_page_width(body: &[&TextRun]) -> f32 {
    let mut x0 = f32::INFINITY;
    let mut x1 = f32::NEG_INFINITY;
    for r in body {
        x0 = x0.min(r.bbox.x0);
        x1 = x1.max(r.bbox.x1);
    }
    if x0.is_finite() && x1.is_finite() && x1 > x0 {
        x1 - x0
    } else {
        0.0
    }
}

/// Propose table areas from multi-col textlines ordered top→bottom.
///
/// # Split policy (v1)
/// | Gap band | Action |
/// |----------|--------|
/// | `gap ≤ soft` | Keep in same area |
/// | `soft < gap < hard` | Split iff neighboring column schemas are incompatible |
/// | `gap ≥ hard` (= 3× soft) | **Always** split |
///
/// After raw split: re-merge adjacent same-schema areas and bridge short
/// note islands, but **never** across a hard gap.
fn propose_table_areas<'a>(
    multi: &[&'a TextLine],
    soft_gap: f32,
    hard_gap: f32,
    fs: f32,
) -> Vec<Vec<&'a TextLine>> {
    debug_assert!(
        hard_gap >= soft_gap * 2.99,
        "hard_gap must be 3× soft for table-area v1"
    );
    let raw = split_multi_regions(multi, soft_gap, hard_gap, fs);
    merge_same_schema_regions(raw, fs, hard_gap)
}

pub(crate) struct TextLine {
    y: f32,
    runs: Vec<TextRun>,
    multi: bool,
}

fn build_textlines(body: &[&TextRun], y_tol: f32) -> Vec<TextLine> {
    let mut items: Vec<&TextRun> = body.to_vec();
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
    let mut lines: Vec<TextLine> = Vec::new();
    for r in items {
        if let Some(line) = lines.last_mut() {
            if (r.bbox.y_center() - line.y).abs() <= y_tol {
                line.runs.push((*r).clone());
                line.y = line.runs.iter().map(|x| x.bbox.y_center()).sum::<f32>()
                    / line.runs.len() as f32;
                line.multi = line.runs.len() >= 2;
                continue;
            }
        }
        lines.push(TextLine {
            y: r.bbox.y_center(),
            runs: vec![(*r).clone()],
            multi: false,
        });
    }
    for line in &mut lines {
        line.runs.sort_by(|a, b| {
            a.bbox
                .x0
                .partial_cmp(&b.bbox.x0)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        // Multi-col: ≥2 runs, whitespace-token TJ row, or glued label+numerics
        // (RBI/state tables paint a whole data row as one text object without spaces).
        let token_multi = line.runs.len() == 1
            && line.runs[0]
                .text
                .split_whitespace()
                .filter(|t| !t.is_empty())
                .count()
                >= 3;
        let glued_multi = line.runs.len() == 1 && looks_glued_tabular(&line.runs[0].text);
        // Pure glued numeric stream (NIPA: "11.30.32.1-2.1-8.6…") — no label.
        let glued_numeric_only =
            line.runs.len() == 1 && looks_glued_numeric_stream(&line.runs[0].text);
        let nipa_struct = line.runs.len() == 1
            && (looks_nipa_placeholder_row(&line.runs[0].text)
                || looks_nipa_section_header(&line.runs[0].text));
        line.multi =
            line.runs.len() >= 2 || token_multi || glued_multi || glued_numeric_only || nipa_struct;
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::options::TablePreset;
    use pdfparser_ir::Matrix3x2;

    fn grid_runs(rows: u32, cols: u32) -> Vec<TextRun> {
        let mut runs = Vec::new();
        for r in 0..rows {
            for c in 0..cols {
                runs.push(TextRun {
                    text: format!("r{r}c{c}"),
                    bbox: Rect {
                        x0: 30.0 + c as f32 * 50.0,
                        y0: 700.0 - r as f32 * 12.0,
                        x1: 45.0 + c as f32 * 50.0,
                        y1: 710.0 - r as f32 * 12.0,
                    },
                    transform: Matrix3x2::identity(),
                    font_name: None,
                    font_size: 9.0,
                    mapping_confidence: 1.0,
                    metrics_confidence: 1.0,
                    mcid: None,
                    invisible: false,
                    from_actual_text: false,
                });
            }
        }
        runs
    }

    #[test]
    fn network_large_borderless() {
        let runs = grid_runs(25, 5);
        let opts = TableOptions::from_preset(TablePreset::Auto);
        let tabs = detect_network_tables(0, &runs, &opts);
        assert!(!tabs.is_empty());
        assert!(tabs[0].rows >= 20, "rows={}", tabs[0].rows);
        assert_eq!(tabs[0].cols, 5);
    }

    #[test]
    fn looks_glued_tabular_detects_rbi_style() {
        assert!(looks_glued_tabular(
            "Andhra Pradesh48.1140.45-3.264.42.62-0.91-0.25"
        ));
        assert!(looks_glued_tabular(
            "Assam12.6910.02-2.410.260.08--0.060.010.24"
        ));
        assert!(!looks_glued_tabular("plain prose without numbers here"));
        assert!(!looks_glued_tabular("only 12 digits 34 56"));
    }

    #[test]
    fn glued_numeric_tail_two_decimal_split() {
        let toks = tokenize_numeric_tail("48.1140.45-3.264.42.62-0.91-0.25");
        assert_eq!(toks[0], "48.11");
        assert_eq!(toks[1], "40.45");
        // Dash before digit is missing marker + unsigned number.
        assert!(toks.iter().any(|t| t == "-"));
        assert!(toks.iter().any(|t| t == "3.26" || t == "3.2"));
        let mut row = vec![String::new(); 11];
        fill_row_glued_tabular("Andhra Pradesh48.1140.45-3.264.42.62-0.91-0.25", &mut row);
        assert_eq!(row[0], "Andhra Pradesh");
        assert_eq!(row[1], "48.11");
        assert_eq!(row[2], "40.45");
        // No line# → RBI missing-marker mode.
        assert_eq!(row[3], "-");
    }

    #[test]
    fn fill_row_nipa_line_number_and_signed_values() {
        let mut row = vec![String::new(); 22];
        fill_row_glued_tabular(
            "1Gross domestic product (GDP)5.81.92.5-5.3-28.034.84.25.26.23.37.0-2.0-0.62.72.62.22.14.93.3",
            &mut row,
        );
        assert_eq!(row[0], "1");
        assert!(
            row[1].starts_with("Gross domestic product"),
            "label={}",
            row[1]
        );
        assert_eq!(row[2], "5.8");
        assert_eq!(row[3], "1.9");
        assert_eq!(row[4], "2.5");
        assert_eq!(row[5], "-5.3");
        assert_eq!(row[6], "-28.0");
        // ~19 numbers after label → last annual-ish token present
        let n_filled = row.iter().filter(|c| !c.trim().is_empty()).count();
        assert!(
            n_filled >= 18,
            "expected dense NIPA row fill, got {n_filled} cells: {row:?}"
        );
    }

    /// Header multi-run schema + glued single-run body (RBI liabilities style).
    #[test]
    fn network_glued_body_rows_recover_table() {
        let mut runs = Vec::new();
        let xs = [40.0_f32, 120.0, 180.0, 240.0, 300.0, 360.0];
        // multi-run header
        for (i, &x) in xs.iter().enumerate() {
            runs.push(TextRun {
                text: format!("H{i}"),
                bbox: Rect {
                    x0: x,
                    y0: 700.0,
                    x1: x + 30.0,
                    y1: 710.0,
                },
                transform: Matrix3x2::identity(),
                font_name: None,
                font_size: 9.0,
                mapping_confidence: 1.0,
                metrics_confidence: 1.0,
                mcid: None,
                invisible: false,
                from_actual_text: false,
            });
        }
        // glued body rows spanning full width
        let bodies = [
            "Alpha State12.3456.78-1.232.34-0.50",
            "Beta Region9.8765.43-2.101.00-0.25",
            "Gamma Place3.2109.87-0.504.56-1.10",
            "Delta Land8.0012.34-3.000.50-0.10",
            "Epsilon Bay1.112.22-0.333.33-0.44",
            "Zeta Coast5.556.66-0.777.77-0.88",
        ];
        for (ri, body) in bodies.iter().enumerate() {
            let y = 680.0 - ri as f32 * 14.0;
            runs.push(TextRun {
                text: body.to_string(),
                bbox: Rect {
                    x0: 40.0,
                    y0: y,
                    x1: 400.0,
                    y1: y + 10.0,
                },
                transform: Matrix3x2::identity(),
                font_name: None,
                font_size: 9.0,
                mapping_confidence: 1.0,
                metrics_confidence: 1.0,
                mcid: None,
                invisible: false,
                from_actual_text: false,
            });
        }
        let opts = TableOptions::from_preset(TablePreset::Auto);
        let tabs = detect_network_tables(0, &runs, &opts);
        assert!(!tabs.is_empty(), "expected glued-body network table");
        let t = &tabs[0];
        assert!(t.rows >= 6, "rows={}", t.rows);
        assert!(t.cols >= 4, "cols={}", t.cols);
        // first body cell should keep alphabetic label prefix
        let labels: Vec<String> = t
            .cells
            .iter()
            .filter(|c| c.col == 0 && !c.is_header)
            .map(|c| c.text.clone())
            .collect();
        assert!(
            labels
                .iter()
                .any(|s| s.contains("Alpha") || s.contains("Beta")),
            "labels={labels:?}"
        );
    }

    /// Large irregular borderless grid with mid-page section-note islands + mild x jitter.
    /// Must stay one table (not fragment into header-slices).
    #[test]
    fn network_irregular_grid_section_gap_stays_one() {
        let mut runs = Vec::new();
        let cols = 8u32;
        let body_rows = 36u32;
        let xs: Vec<f32> = (0..cols).map(|c| 30.0 + c as f32 * 48.0).collect();
        let mut y = 740.0_f32;
        // header
        for (c, &x) in xs.iter().enumerate() {
            runs.push(TextRun {
                text: format!("H{c}"),
                bbox: Rect {
                    x0: x,
                    y0: y,
                    x1: x + 20.0,
                    y1: y + 8.0,
                },
                transform: Matrix3x2::identity(),
                font_name: None,
                font_size: 7.0,
                mapping_confidence: 1.0,
                metrics_confidence: 1.0,
                mcid: None,
                invisible: false,
                from_actual_text: false,
            });
        }
        y -= 12.0;
        for r in 0..body_rows {
            // Section note island every 10 body rows (different column schema).
            if r > 0 && r % 10 == 0 {
                y -= 8.0;
                runs.push(TextRun {
                    text: format!("=== Section {} notes ===", r / 10),
                    bbox: Rect {
                        x0: 30.0,
                        y0: y,
                        x1: 200.0,
                        y1: y + 7.0,
                    },
                    transform: Matrix3x2::identity(),
                    font_name: None,
                    font_size: 6.0,
                    mapping_confidence: 1.0,
                    metrics_confidence: 1.0,
                    mcid: None,
                    invisible: false,
                    from_actual_text: false,
                });
                y -= 10.0;
                // Mini multi-col note with different x anchors (must not fork regions).
                for (k, x) in [30.0_f32, 70.0, 110.0, 150.0, 190.0].iter().enumerate() {
                    runs.push(TextRun {
                        text: format!("note{k}"),
                        bbox: Rect {
                            x0: *x,
                            y0: y,
                            x1: *x + 18.0,
                            y1: y + 7.0,
                        },
                        transform: Matrix3x2::identity(),
                        font_name: None,
                        font_size: 6.0,
                        mapping_confidence: 1.0,
                        metrics_confidence: 1.0,
                        mcid: None,
                        invisible: false,
                        from_actual_text: false,
                    });
                }
                y -= 12.0;
            }
            for (c, &x) in xs.iter().enumerate() {
                // Mild jitter + occasional large offset (ICDAR-class).
                let jx = if (r + c as u32) % 11 == 0 {
                    14.0
                } else if (r * 3 + c as u32) % 5 == 0 {
                    -2.5
                } else if (r + c as u32) % 3 == 0 {
                    2.0
                } else {
                    0.0
                };
                // Sparse empties.
                if c > 0 && (r * 7 + c as u32) % 13 == 0 {
                    continue;
                }
                runs.push(TextRun {
                    text: format!("r{r}c{c}"),
                    bbox: Rect {
                        x0: x + jx,
                        y0: y,
                        x1: x + jx + 18.0,
                        y1: y + 7.0,
                    },
                    transform: Matrix3x2::identity(),
                    font_name: None,
                    font_size: 6.0,
                    mapping_confidence: 1.0,
                    metrics_confidence: 1.0,
                    mcid: None,
                    invisible: false,
                    from_actual_text: false,
                });
            }
            y -= if r % 4 == 0 { 11.5 } else { 9.5 };
        }
        let opts = TableOptions::from_preset(TablePreset::Auto);
        let tabs = detect_network_tables(0, &runs, &opts);
        assert_eq!(
            tabs.len(),
            1,
            "expected 1 table, got {} shapes={:?}",
            tabs.len(),
            tabs.iter().map(|t| (t.rows, t.cols)).collect::<Vec<_>>()
        );
        let t = &tabs[0];
        // ~1 header + 36 body; section notes dropped. Allow small slack.
        assert!(
            t.rows >= 30 && t.rows <= 40,
            "rows should cover body, got {}",
            t.rows
        );
        assert!(
            (7..=9).contains(&t.cols),
            "cols should be ~8 despite jitter, got {}",
            t.cols
        );
    }

    /// Two distinct borderless tables with a large vertical gap + different
    /// column layouts must not merge (hard split + schema identity).
    #[test]
    fn network_hard_gap_keeps_two_tables() {
        let mut runs = Vec::new();
        // Table A: 5×3 at top-left
        for r in 0..5u32 {
            for c in 0..3u32 {
                runs.push(TextRun {
                    text: format!("A{r}{c}"),
                    bbox: Rect {
                        x0: 40.0 + c as f32 * 60.0,
                        y0: 700.0 - r as f32 * 12.0,
                        x1: 55.0 + c as f32 * 60.0,
                        y1: 710.0 - r as f32 * 12.0,
                    },
                    transform: Matrix3x2::identity(),
                    font_name: None,
                    font_size: 9.0,
                    mapping_confidence: 1.0,
                    metrics_confidence: 1.0,
                    mcid: None,
                    invisible: false,
                    from_actual_text: false,
                });
            }
        }
        // Table B: 6×2 lower-right, different x anchors, gap ≫ 2× soft_gap
        for r in 0..6u32 {
            for c in 0..2u32 {
                runs.push(TextRun {
                    text: format!("B{r}{c}"),
                    bbox: Rect {
                        x0: 320.0 + c as f32 * 80.0,
                        y0: 400.0 - r as f32 * 12.0,
                        x1: 340.0 + c as f32 * 80.0,
                        y1: 410.0 - r as f32 * 12.0,
                    },
                    transform: Matrix3x2::identity(),
                    font_name: None,
                    font_size: 9.0,
                    mapping_confidence: 1.0,
                    metrics_confidence: 1.0,
                    mcid: None,
                    invisible: false,
                    from_actual_text: false,
                });
            }
        }
        let opts = TableOptions::from_preset(TablePreset::Auto);
        let tabs = detect_network_tables(0, &runs, &opts);
        assert!(
            tabs.len() >= 2,
            "hard gap + different schema must keep 2 tables, got {:?}",
            tabs.iter().map(|t| (t.rows, t.cols)).collect::<Vec<_>>()
        );
    }

    #[test]
    fn network_rejects_numbered_prose_list() {
        let mut runs = Vec::new();
        for i in 0..8 {
            runs.push(TextRun {
                text: format!("{}.", i + 1),
                bbox: Rect {
                    x0: 40.0,
                    y0: 700.0 - i as f32 * 14.0,
                    x1: 55.0,
                    y1: 710.0 - i as f32 * 14.0,
                },
                transform: Matrix3x2::identity(),
                font_name: None,
                font_size: 10.0,
                mapping_confidence: 1.0,
                metrics_confidence: 1.0,
                mcid: None,
                invisible: false,
                from_actual_text: false,
            });
            runs.push(TextRun {
                text: format!(
                    "Long prose discussion point number {i} elaborates methodology further"
                ),
                bbox: Rect {
                    x0: 70.0,
                    y0: 700.0 - i as f32 * 14.0,
                    x1: 320.0,
                    y1: 710.0 - i as f32 * 14.0,
                },
                transform: Matrix3x2::identity(),
                font_name: None,
                font_size: 10.0,
                mapping_confidence: 1.0,
                metrics_confidence: 1.0,
                mcid: None,
                invisible: false,
                from_actual_text: false,
            });
        }
        let opts = TableOptions::from_preset(TablePreset::Auto);
        let tabs = detect_network_tables(0, &runs, &opts);
        assert!(
            tabs.is_empty(),
            "numbered prose list must not be a table: {:?}",
            tabs.iter().map(|t| (t.rows, t.cols)).collect::<Vec<_>>()
        );
    }

    fn push_grid(
        runs: &mut Vec<TextRun>,
        rows: u32,
        cols: u32,
        x0: f32,
        y_top: f32,
        x_pitch: f32,
        y_pitch: f32,
        prefix: &str,
        fs: f32,
    ) {
        for r in 0..rows {
            for c in 0..cols {
                let x = x0 + c as f32 * x_pitch;
                let y = y_top - r as f32 * y_pitch;
                runs.push(TextRun {
                    text: format!("{prefix}{r}{c}"),
                    bbox: Rect {
                        x0: x,
                        y0: y,
                        x1: x + 18.0,
                        y1: y + 8.0,
                    },
                    transform: Matrix3x2::identity(),
                    font_name: None,
                    font_size: fs,
                    mapping_confidence: 1.0,
                    metrics_confidence: 1.0,
                    mcid: None,
                    invisible: false,
                    from_actual_text: false,
                });
            }
        }
    }

    /// Table-area hard gap (3× soft) always splits — even when both grids share
    /// the same column schema. Verifies area proposal does not re-merge.
    #[test]
    fn area_hard_gap_splits_two_tables() {
        let mut runs = Vec::new();
        let fs = 9.0_f32;
        // soft ≈ max(4*9, 24)=36; hard = 108. Place second grid ≥120 below first.
        // Table A: y_top=700, 5 rows × pitch 12 → last row y=652
        push_grid(&mut runs, 5, 3, 40.0, 700.0, 55.0, 12.0, "A", fs);
        // Table B: same x anchors/schema, y_top=500 → gap from 652→500 = 152 ≥ hard
        push_grid(&mut runs, 5, 3, 40.0, 500.0, 55.0, 12.0, "B", fs);
        let opts = TableOptions::from_preset(TablePreset::Auto);
        let tabs = detect_network_tables(0, &runs, &opts);
        assert_eq!(
            tabs.len(),
            2,
            "hard gap must yield 2 table areas (same schema), got shapes={:?}",
            tabs.iter().map(|t| (t.rows, t.cols)).collect::<Vec<_>>()
        );
        for t in &tabs {
            assert!(t.rows >= 3, "rows={}", t.rows);
            assert_eq!(t.cols, 3);
        }
    }

    /// Soft gap with incompatible column schemas must open two areas
    /// (different col counts never soft-merge).
    #[test]
    fn schema_incompatible_soft_gap_splits() {
        let mut runs = Vec::new();
        let fs = 9.0_f32;
        // soft=36, hard=108. Soft-band gap ~50–80 between last A and first B.
        // A: 3-col left edges 40/100/160
        push_grid(&mut runs, 5, 3, 40.0, 700.0, 60.0, 12.0, "A", fs);
        // last A y ≈ 700-48=652; B y_top=600 → gap≈52 (soft band)
        // B: 4-col left edges 40/85/130/175 — different schema
        push_grid(&mut runs, 5, 4, 40.0, 600.0, 45.0, 12.0, "B", fs);
        let opts = TableOptions::from_preset(TablePreset::Auto);
        let tabs = detect_network_tables(0, &runs, &opts);
        assert!(
            tabs.len() >= 2,
            "schema-incompatible soft gap must split areas, got shapes={:?}",
            tabs.iter().map(|t| (t.rows, t.cols)).collect::<Vec<_>>()
        );
        let cols: Vec<u32> = tabs.iter().map(|t| t.cols).collect();
        assert!(
            cols.contains(&3) && cols.contains(&4),
            "expected both 3-col and 4-col areas, cols={cols:?}"
        );
    }

    /// Single-column prose alone never proposes a table area (no multi-col
    /// lines → empty; no mega-fallback invents a page table).
    #[test]
    fn area_no_mega_fallback_from_single_col_prose() {
        let mut runs = Vec::new();
        for i in 0..12 {
            runs.push(TextRun {
                text: format!("Paragraph line {i} of flowing single-column prose without columns."),
                bbox: Rect {
                    x0: 50.0,
                    y0: 700.0 - i as f32 * 14.0,
                    x1: 400.0,
                    y1: 712.0 - i as f32 * 14.0,
                },
                transform: Matrix3x2::identity(),
                font_name: None,
                font_size: 10.0,
                mapping_confidence: 1.0,
                metrics_confidence: 1.0,
                mcid: None,
                invisible: false,
                from_actual_text: false,
            });
        }
        let opts = TableOptions::from_preset(TablePreset::Auto);
        let tabs = detect_network_tables(0, &runs, &opts);
        assert!(
            tabs.is_empty(),
            "single-col prose must not invent a mega-table: {:?}",
            tabs.iter().map(|t| (t.rows, t.cols)).collect::<Vec<_>>()
        );
    }

    #[test]
    fn nipa_glued_region_detects_multi_col() {
        use crate::options::{TableOptions, TablePreset};
        #[cfg(test)]
        use pdfparser_ir::Rect;
        use pdfparser_ir::TextRun;
        fn run(text: &str, x0: f32, y0: f32, x1: f32, fs: f32) -> TextRun {
            TextRun {
                text: text.into(),
                bbox: Rect {
                    x0,
                    y0,
                    x1,
                    y1: y0 + fs,
                },
                transform: pdfparser_ir::Matrix3x2::identity(),
                font_name: None,
                font_size: fs,
                mapping_confidence: 1.0,
                metrics_confidence: 1.0,
                mcid: None,
                invisible: false,
                from_actual_text: false,
            }
        }
        for text in [
            "1Gross domestic product (GDP)5.81.92.5-5.3-28.034.84.25.26.23.37.0-2.0",
            "3Goods11.30.32.1-2.1-8.651.73.216.514.7-8.55.6-1.2-0.3",
        ] {
            assert!(looks_glued_tabular(text), "glued? {}", text);
            assert!(
                glued_field_count(text) >= 4,
                "fields {}",
                glued_field_count(text)
            );
        }
        let mut runs = Vec::new();
        let fs = 8.0;
        let rows = [
            "1Gross domestic product (GDP)5.81.92.5-5.3-28.034.84.25.26.23.37.0-2.0",
            "2Personal consumption expenditures8.42.52.2-6.4-30.240.55.68.913.6",
            "3Goods11.30.32.1-2.1-8.651.73.216.514.7-8.55.6-1.2-0.3",
            "4Durable goods16.7-0.34.3-16.6-0.2100.75.528.414.3-23.111.1",
            "5Nondurable goods8.50.60.96.1-12.530.81.810.114.81.12.6-2.7",
            "6Services6.93.72.3-8.4-38.735.16.85.513.09.33.20.63.2",
            "7Gross private domestic investment8.74.8-1.2-9.9-46.498.913.2",
        ];
        for (i, text) in rows.iter().enumerate() {
            let y = 600.0 - i as f32 * 12.0;
            runs.push(run(text, 40.0, y, 900.0, fs));
        }
        assert!(looks_glued_tabular(rows[0]), "glued row0");
        let mut opts = TableOptions::from_preset(TablePreset::Auto);
        opts.advanced.min_confidence_stream = 0.45;
        opts.min_table_confidence = 0.45;
        let tabs = detect_network_tables(0, &runs, &opts);
        assert!(!tabs.is_empty(), "expected NIPA glued table, got 0");
        assert!(tabs[0].cols >= 4, "cols={}", tabs[0].cols);
        assert!(tabs[0].rows >= 5, "rows={}", tabs[0].rows);
    }
}
