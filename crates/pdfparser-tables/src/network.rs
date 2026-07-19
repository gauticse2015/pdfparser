//! Network-class borderless tables (textline + column alignments).
//!
//! # Table-area engine v1 (PR5)
//!
//! Production borderless path:
//! 1. Build textlines (baseline bands)
//! 2. Keep multi-column lines only for structure (single-col prose never forms areas)
//! 3. **Propose table areas** ([`propose_table_areas`]):
//!    - **Hard gap** = 3× soft (font-scaled). Always splits; never re-merged.
//!    - **Soft gap**: split only when neighboring column schemas diverge
//!      (equal count + left-edge bipartite match).
//!    - Re-merge adjacent same-schema regions; bridge short note islands
//!      only when the island-span gap is still **below** hard.
//! 4. Per-area column anchors (support-filtered left edges)
//! 5. One row per multi-col textline (drop non-grid note lines)
//! 6. Reject non-table areas (prose lists, optional narrow low-numeric bands)
//!
//! **No full-page mega-table fallback.** If area proposal yields no viable
//! region (or every region fails gates), return empty — do not invent a table
//! from single-col prose or by fusing all multi lines into one page-wide area.

use crate::geom::{bbox_of_cells, cluster_coords, median_font_size};
use crate::options::TableOptions;
use crate::types::{PipelineId, Table, TableCell, TableMethod};
use pdfparser_ir::{Rect, TextRun};

/// Detect borderless tables via textline network structure + table-area engine.
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
    let y_tol = (0.65 * fs)
        .max(2.5)
        .max(0.45 * median_row_pitch)
        .min((0.9 * median_row_pitch).max(fs));
    let lines = build_textlines(&body, y_tol);
    let multi: Vec<&TextLine> = lines.iter().filter(|l| l.multi).collect();
    if multi.len() < opts.stream_min_body_bands.max(3) as usize {
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
    let soft_floor = (opts.stream_region_gap_font_mult * fs * 0.35)
        .max(opts.stream_region_gap_min * 0.5)
        .max(fs * 1.1);
    let soft_gap = (multi_pitch * 1.6).max(soft_floor);
    // Hard: large enough that normal body pitch never crosses it; still
    // separates distinct stacked tables with multi-row blank bands.
    let hard_gap = (multi_pitch * 6.0).max(soft_gap * 2.5).max(fs * 4.0);
    // Area proposal *before* per-region build — page-global filter collapses
    // multi-table pages into one skeleton if applied first. Section notes are
    // dropped per-area inside build_table_from_lines.
    let regions = propose_table_areas(&multi, soft_gap, hard_gap, fs);
    let min_multi = opts.stream_min_body_bands.max(3) as usize;
    let mut out = Vec::new();
    for region in regions {
        if region.len() < min_multi {
            continue;
        }
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
        // Numbered footnote only (e.g. "1. See note below"). Long free-text
        // trailers are left intact — they may be real body rows.
        i > 0 && i <= 3 && matches!(bytes.get(i), Some(b'.') | Some(b')'))
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
    mut tabs: Vec<crate::types::Table>,
    max_gap: f32,
) -> Vec<crate::types::Table> {
    use crate::types::{Table, TableMethod};
    if tabs.len() <= 1 {
        return tabs;
    }
    // Top-first (higher y0 first).
    tabs.sort_by(|a, b| {
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
    });
    let mut out: Vec<Table> = Vec::new();
    for t in tabs {
        if out.is_empty() {
            out.push(t);
            continue;
        }
        let prev = out.last().unwrap();
        let same_page = prev.page == t.page;
        let both_stream = matches!(prev.method, TableMethod::Stream | TableMethod::DenseNumeric)
            && matches!(t.method, TableMethod::Stream | TableMethod::DenseNumeric);
        let same_cols = prev.cols == t.cols && prev.cols >= 3;
        // Vertical stack: prev above t (prev.y0 >= t.y1 in PDF coords, gap small).
        let gap = prev.bbox.y0 - t.bbox.y1;
        let x_overlap = {
            let x0 = prev.bbox.x0.max(t.bbox.x0);
            let x1 = prev.bbox.x1.min(t.bbox.x1);
            let w = (x1 - x0).max(0.0);
            let min_w = prev.bbox.width().min(t.bbox.width()).max(1.0);
            w / min_w
        };
        // Width ratio: avoid gluing a wide financial block to a narrow sidebar
        // table (false stack merges of distinct grids).
        let w_ratio = {
            let pw = prev.bbox.width().max(1.0);
            let tw = t.bbox.width().max(1.0);
            (pw / tw).max(tw / pw)
        };
        if same_page
            && both_stream
            && same_cols
            && gap >= -2.0
            && gap <= max_gap * 1.5
            && x_overlap >= 0.55
            && w_ratio <= 1.35
            && prev.rows + t.rows <= 120
        {
            let mut merged = prev.clone();
            // Append body rows of t (skip near-duplicate header if identical to prev header).
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
            let start_row = if skip_header { 1u32 } else { 0u32 };
            let row_off = prev.rows;
            for cell in &t.cells {
                if cell.row < start_row {
                    continue;
                }
                let mut nc = cell.clone();
                nc.row = cell.row - start_row + row_off;
                merged.cells.push(nc);
            }
            let added = t.rows.saturating_sub(start_row);
            merged.rows = row_off + added;
            merged.bbox.x0 = prev.bbox.x0.min(t.bbox.x0);
            merged.bbox.y0 = t.bbox.y0.min(prev.bbox.y0);
            merged.bbox.x1 = prev.bbox.x1.max(t.bbox.x1);
            merged.bbox.y1 = prev.bbox.y1.max(t.bbox.y1);
            merged.confidence = prev.confidence.max(t.confidence) * 0.98;
            merged
                .notes
                .push(format!("stream_stack_merge +{added}rows"));
            *out.last_mut().unwrap() = merged;
        } else {
            out.push(t);
        }
    }
    out
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

struct TextLine {
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

/// Single-run body rows that glue a text label to dense numerics without
/// whitespace (`"Andhra Pradesh48.1140.45-3.26…"`). Common in RBI/Excel PDF
/// exports; still a multi-column table row for area proposal + char-x split.
///
/// Also BEA NIPA: `"1Gross domestic product (GDP)5.81.92.5…"` where the first
/// seam is often `)` + digit rather than letter + digit.
fn looks_glued_tabular(s: &str) -> bool {
    let t = s.trim();
    if t.len() < 10 {
        return false;
    }
    let digits = t.chars().filter(|c| c.is_ascii_digit()).count();
    let alpha = t.chars().filter(|c| c.is_alphabetic()).count();
    if digits < 6 || alpha < 3 {
        return false;
    }
    // Letter or closing paren/bracket immediately followed by a digit.
    let mut seam = false;
    let mut prev_alpha = false;
    let mut prev_close = false;
    for ch in t.chars() {
        if ch.is_alphabetic() {
            prev_alpha = true;
            prev_close = false;
        } else if ch.is_ascii_digit() {
            if prev_alpha || prev_close {
                seam = true;
                break;
            }
            prev_alpha = false;
            prev_close = false;
        } else if ch.is_whitespace() {
            prev_alpha = false;
            prev_close = false;
        } else if ch == ')' || ch == ']' {
            prev_close = true;
            prev_alpha = false;
        } else if ch != '.' && ch != '\'' && ch != '-' {
            prev_alpha = false;
            prev_close = false;
        }
        // keep prev_alpha across hyphen/apostrophe inside names (O'Brien, Jean-Luc)
    }
    // Dense digit packing: numbers dominate the non-alpha tail.
    seam && digits * 3 >= t.len().saturating_sub(alpha)
}

/// Glued pure-numeric stream: many financial tokens, little/no alpha
/// (`"11.30.32.1-2.1-8.651.73.2…"`). Phase-4 NIPA right-hand number blocks.
fn looks_glued_numeric_stream(s: &str) -> bool {
    let t = s.trim();
    if t.len() < 12 {
        return false;
    }
    let digits = t.chars().filter(|c| c.is_ascii_digit()).count();
    let alpha = t.chars().filter(|c| c.is_alphabetic()).count();
    if digits < 10 || alpha > digits / 4 {
        return false;
    }
    // At least 4 tokenizable financial numbers.
    tokenize_numeric_tail(t).len() >= 4
}

/// NIPA placeholder / leader-dot rows: `14Change in private inventories.....`
/// (line# + label + dots, almost no numeric values). Must stay in the grid so
/// row topology matches gold (inventory / net-exports blank rows).
fn looks_nipa_placeholder_row(s: &str) -> bool {
    let t = s.trim();
    if t.len() < 16 {
        return false;
    }
    let dots = t.chars().filter(|&c| c == '.' || c == '·').count();
    if dots < 12 {
        return false;
    }
    let chars: Vec<char> = t.chars().collect();
    if !chars[0].is_ascii_digit() {
        return false;
    }
    let mut j = 0usize;
    while j < chars.len() && chars[j].is_ascii_digit() {
        j += 1;
    }
    j < chars.len() && chars[j].is_alphabetic() && j <= 3
}

/// Section banner rows in NIPA tables (`Addenda:`, `Current-dollar measures:`).
fn looks_nipa_section_header(s: &str) -> bool {
    let t = s.trim();
    if t.len() < 6 || t.len() > 48 {
        return false;
    }
    let lower = t.to_ascii_lowercase();
    lower.starts_with("addenda")
        || lower.starts_with("current-dollar")
        || lower.starts_with("current dollar")
        || (lower.ends_with(':')
            && t.chars().filter(|c| c.is_alphabetic()).count() >= 4
            && t.chars().filter(|c| c.is_ascii_digit()).count() <= 2)
}

/// Field count for a glued tabular / numeric-only line (label + numbers).
fn glued_field_count(text: &str) -> usize {
    let t = text.trim();
    if t.is_empty() {
        return 0;
    }
    if looks_glued_tabular(t) {
        // Match fill_row_glued_tabular field layout: optional line# + label + nums.
        let chars: Vec<char> = t.chars().collect();
        let mut body_start = 0usize;
        let mut fields = 1usize; // label at least
        if !chars.is_empty() && chars[0].is_ascii_digit() {
            let mut j = 0usize;
            while j < chars.len() && chars[j].is_ascii_digit() {
                j += 1;
            }
            if j < chars.len() && chars[j].is_alphabetic() && j <= 3 {
                fields = 2; // line number + label
                body_start = j;
            }
        }
        let body: String = chars[body_start..].iter().collect();
        let body_chars: Vec<char> = body.chars().collect();
        let mut si = 0usize;
        for (i, &ch) in body_chars.iter().enumerate() {
            if ch.is_ascii_digit() {
                si = i;
                if i > 0 && (body_chars[i - 1].is_alphabetic() || body_chars[i - 1] == ')') {
                    si = i;
                }
                break;
            }
        }
        let tail: String = body_chars[si..].iter().collect();
        let n = tokenize_numeric_tail_signed(&tail).len();
        return fields + n;
    }
    if looks_glued_numeric_stream(t) {
        return tokenize_numeric_tail(t).len().max(1);
    }
    0
}

/// Equal-width column anchors when all body rows are glued single-run (NIPA).
fn synthesize_cols_from_glued_tokens(lines: &[&TextLine], fs: f32) -> Option<Vec<f32>> {
    let mut max_fields = 0usize;
    let mut x0 = f32::INFINITY;
    let mut x1 = f32::NEG_INFINITY;
    let mut glued_rows = 0usize;
    for line in lines {
        if line.runs.len() != 1 {
            continue;
        }
        let r = &line.runs[0];
        let t = r.text.trim();
        if t.is_empty() {
            continue;
        }
        let n = glued_field_count(t);
        if n >= 3 {
            glued_rows += 1;
            max_fields = max_fields.max(n);
            x0 = x0.min(r.bbox.x0);
            x1 = x1.max(r.bbox.x1);
        }
    }
    // Need enough glued rows and a wide x-span.
    if glued_rows < 3 || max_fields < 3 || !x0.is_finite() || x1 - x0 < fs * 8.0 {
        return None;
    }
    // Cap synthetic columns to product max (~20 quarters; stay under max_cols).
    let ncols = max_fields.clamp(3, 24);
    let width = (x1 - x0).max(fs * 8.0);
    let step = width / ncols as f32;
    let mut anchors = Vec::with_capacity(ncols);
    for i in 0..ncols {
        anchors.push(x0 + (i as f32 + 0.5) * step);
    }
    Some(anchors)
}

/// Cluster left-edge tolerance (~¾ em, floor at min cell-ish scale).
fn left_cluster_tol(fs: f32) -> f32 {
    (0.75 * fs).max(3.0)
}

/// Raw area split on ordered multi-col lines (top→bottom).
///
/// Hard-gap branch is unconditional: even identical column schemas become
/// separate areas when the vertical separation is ≥ `hard_gap` (3× soft).
fn split_multi_regions<'a>(
    multi: &[&'a TextLine],
    soft_gap: f32,
    hard_gap: f32,
    fs: f32,
) -> Vec<Vec<&'a TextLine>> {
    if multi.is_empty() {
        return Vec::new();
    }
    let tol = left_cluster_tol(fs);
    // Ensure hard is strictly larger than soft so the three bands are distinct.
    let hard_gap = hard_gap.max(soft_gap * 3.0);
    let mut regions = Vec::new();
    let mut cur: Vec<&TextLine> = vec![multi[0]];
    for i in 0..multi.len() - 1 {
        let gap = (multi[i].y - multi[i + 1].y).abs();
        let split = if gap >= hard_gap {
            // Hard gap: always open a new table area.
            true
        } else if gap > soft_gap {
            // Soft gap: keep only when neighboring windows share the same
            // column count and left-edge layout (section note → continue).
            let a0 = i.saturating_sub(3);
            let a = &multi[a0..=i];
            let b1 = (i + 1 + 3).min(multi.len() - 1);
            let b = &multi[i + 1..=b1];
            let sa = region_col_lefts_supported(a, fs);
            let sb = region_col_lefts_supported(b, fs);
            !schemas_compatible(&sa, &sb, tol)
        } else {
            false
        };
        if split {
            regions.push(std::mem::take(&mut cur));
            cur = vec![multi[i + 1]];
        } else {
            cur.push(multi[i + 1]);
        }
    }
    if !cur.is_empty() {
        regions.push(cur);
    }
    regions
}

/// All left-edges clustered (no support filter) — used only as fallback.
fn region_col_lefts(lines: &[&TextLine], fs: f32) -> Vec<f32> {
    let mut lefts: Vec<f32> = Vec::new();
    for line in lines {
        for r in &line.runs {
            if r.text.trim().is_empty() {
                continue;
            }
            lefts.push(r.bbox.x0);
        }
    }
    let x_tol = left_cluster_tol(fs);
    let mut xs = cluster_coords(&lefts, x_tol);
    xs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    xs
}

/// Column left-edges preferring multi-run lines (headers / true multi-cell rows).
///
/// Glued single-run body rows share one left edge; using them for support
/// filtering collapses the skeleton to col0. Rich lines carry the real schema.
fn region_col_lefts_prefer_rich(lines: &[&TextLine], fs: f32) -> Vec<f32> {
    let rich: Vec<&TextLine> = lines
        .iter()
        .copied()
        .filter(|l| l.runs.iter().filter(|r| !r.text.trim().is_empty()).count() >= 3)
        .collect();
    if rich.len() >= 2 {
        let a = region_col_lefts_supported(&rich, fs);
        if a.len() >= 3 {
            return a;
        }
        let b = region_col_lefts(&rich, fs);
        if b.len() >= 3 {
            return b;
        }
    }
    region_col_lefts_supported(lines, fs)
}

/// Split a glued label+numeric row into column cells.
///
/// 1. Leading alphabetic label (state/entity name) → col 0.
/// 2. Tail tokenized as financial-style numbers (≤2 decimal places) and
///    lone `-` missing markers, assigned left→right into remaining columns.
///
/// Proportional char-x fails on proportional fonts (`Andhr|a Prade|sh48…`);
/// digit-aware tokenization matches RBI/Excel stream exports.
fn fill_row_glued_tabular(text: &str, row: &mut [String]) {
    let ncols = row.len();
    if ncols == 0 {
        return;
    }
    for cell in row.iter_mut() {
        cell.clear();
    }
    let t = text.trim();
    if t.is_empty() {
        return;
    }
    let chars: Vec<char> = t.chars().collect();
    // NIPA/BEA: optional leading line number glued to the label (`1Gross…`).
    // Digits immediately followed by a letter are a line index, not a value.
    let mut line_num: Option<String> = None;
    let mut body_start = 0usize;
    if !chars.is_empty() && chars[0].is_ascii_digit() {
        let mut j = 0usize;
        while j < chars.len() && chars[j].is_ascii_digit() {
            j += 1;
        }
        if j < chars.len() && chars[j].is_alphabetic() && j <= 3 {
            line_num = Some(chars[..j].iter().collect());
            body_start = j;
        }
    }
    let body: String = chars[body_start..].iter().collect();
    let body_chars: Vec<char> = body.chars().collect();

    // Seam: first digit after alphabetic label (or after `)` for NIPA titles).
    // `Structures-3.2` → seam at `-` so the signed value is not eaten into the label.
    let mut seam = None;
    for (i, &ch) in body_chars.iter().enumerate() {
        if ch.is_ascii_digit() {
            if i > 0 && body_chars[i - 1] == '-' {
                // Signed number after label (`Structures-3.2`) or mid-stream.
                seam = Some(i - 1);
            } else if i > 0 && (body_chars[i - 1].is_alphabetic() || body_chars[i - 1] == ')') {
                seam = Some(i);
            } else {
                seam = Some(i);
            }
            break;
        }
    }
    let Some(si) = seam else {
        let mut col = 0usize;
        if let Some(ln) = line_num {
            row[0] = ln;
            col = 1.min(ncols.saturating_sub(1));
        }
        if col < ncols {
            row[col] = body.trim().to_string();
        }
        return;
    };
    let label: String = body_chars[..si]
        .iter()
        .collect::<String>()
        .trim()
        .to_string();
    let tail: String = body_chars[si..].iter().collect();
    if ncols == 1 {
        row[0] = t.to_string();
        return;
    }
    // BEA NIPA (`1Gross…5.8-5.3`): line# + label + true signed 1-decimal rates.
    // RBI/Excel (`Andhra Pradesh48.11-3.26`): label + 2-decimal + missing `-`.
    let tokens = if line_num.is_some() {
        tokenize_numeric_tail_signed(&tail)
    } else {
        tokenize_numeric_tail(&tail)
    };
    let mut col = 0usize;
    if let Some(ln) = line_num {
        if col < ncols {
            row[col] = ln;
            col += 1;
        }
    }
    if label.is_empty() {
        for tok in tokens {
            if col < ncols {
                row[col] = tok;
                col += 1;
            } else {
                let last = ncols - 1;
                if !row[last].is_empty() {
                    row[last].push(' ');
                }
                row[last].push_str(&tok);
            }
        }
        return;
    }
    // Expand glued header labels (StatesTotal → States, Total).
    let label_parts = split_glued_header_label(&label);
    for part in &label_parts {
        if col < ncols {
            row[col] = part.clone();
            col += 1;
        }
    }
    if label_parts.is_empty() && col < ncols {
        row[col] = label;
        col += 1;
    }
    for tok in tokens {
        if col < ncols {
            row[col] = tok;
            col += 1;
        } else {
            let last = ncols - 1;
            if !row[last].is_empty() {
                row[last].push(' ');
            }
            row[last].push_str(&tok);
        }
    }
}

/// Expand known/CamelCase glued headers into empty right-neighbor cells.
fn expand_glued_headers_in_row(row: &mut [String]) {
    let ncols = row.len();
    let mut c = 0usize;
    while c < ncols {
        let parts = split_glued_header_label(row[c].trim());
        if parts.len() < 2 {
            c += 1;
            continue;
        }
        // Need empty cells to the right for extra parts.
        let need = parts.len() - 1;
        let mut empty_right = 0usize;
        for j in (c + 1)..ncols {
            if row[j].trim().is_empty() {
                empty_right += 1;
            } else {
                break;
            }
        }
        if empty_right < need {
            c += 1;
            continue;
        }
        for (i, part) in parts.iter().enumerate() {
            let dest = c + i;
            if dest < ncols {
                row[dest] = part.clone();
            }
        }
        c += parts.len();
    }
}

/// Split common glued header compounds without spaces.
fn split_glued_header_label(label: &str) -> Vec<String> {
    let t = label.trim();
    if t.is_empty() {
        return vec![String::new()];
    }
    // Known financial-stream compounds (RBI / liabilities tables).
    let known = [
        ("StatesTotal", &["States", "Total"][..]),
        ("NSSFWMA", &["NSSF", "WMA"][..]),
        ("MarketNSSFWMA", &["Market", "NSSF", "WMA"][..]),
        ("Market NSSFWMA", &["Market", "NSSF", "WMA"][..]),
        ("MarketNSSF", &["Market", "NSSF"][..]),
        ("MarketLoans", &["Market", "Loans"][..]),
    ];
    for (k, parts) in known {
        if t.eq_ignore_ascii_case(k) {
            return parts.iter().map(|s| (*s).to_string()).collect();
        }
    }
    // CamelCase / letter→Upper split: "StatesTotal" → ["States","Total"]
    let chars: Vec<char> = t.chars().collect();
    if chars.len() >= 6 {
        let mut parts = Vec::new();
        let mut start = 0usize;
        for i in 1..chars.len() {
            if chars[i].is_uppercase()
                && chars[i - 1].is_lowercase()
                && i + 1 < chars.len()
                && chars[i + 1].is_lowercase()
            {
                parts.push(chars[start..i].iter().collect::<String>());
                start = i;
            }
        }
        if start > 0 {
            parts.push(chars[start..].iter().collect::<String>());
            if parts.len() >= 2 && parts.iter().all(|p| p.len() >= 2) {
                return parts;
            }
        }
    }
    vec![t.to_string()]
}

/// Tokenize glued numeric tails: numbers with ≤2 decimals, and lone `-`.
///
/// RBI/Excel mode: `-` before a digit is a *missing* field then an unsigned
/// number (`40.45-3.26` → `-` + `3.26`). Prefer [`tokenize_numeric_tail_signed`]
/// for BEA NIPA true negatives.
fn tokenize_numeric_tail(tail: &str) -> Vec<String> {
    let chars: Vec<char> = tail.chars().collect();
    let mut i = 0usize;
    let mut out = Vec::new();
    while i < chars.len() {
        if chars[i] == '-' {
            // RBI/Excel glued streams use `-` as *missing* between fields more often
            // than true negatives (`40.45-3.26` → blank then `3.26`). Emit missing
            // marker then parse the unsigned number.
            if i + 1 < chars.len() && chars[i + 1].is_ascii_digit() {
                out.push("-".into());
                let (tok, ni) = parse_fin_number(&chars, i + 1);
                out.push(tok);
                i = ni;
            } else {
                out.push("-".into());
                i += 1;
            }
        } else if chars[i].is_ascii_digit() {
            let (tok, ni) = parse_fin_number(&chars, i);
            out.push(tok);
            i = ni;
        } else {
            i += 1;
        }
    }
    out
}

/// Tokenize with true signed numbers (BEA NIPA: `5.8-5.3` → `5.8`, `-5.3`).
///
/// NIPA percent tables use a single decimal place; limiting decimals avoids
/// eating the next integer (`-28.034.8` → `-28.0` + `34.8`, not `-28.03`).
fn tokenize_numeric_tail_signed(tail: &str) -> Vec<String> {
    let chars: Vec<char> = tail.chars().collect();
    let mut i = 0usize;
    let mut out = Vec::new();
    while i < chars.len() {
        if chars[i] == '-' && i + 1 < chars.len() && chars[i + 1].is_ascii_digit() {
            let (tok, ni) = parse_fin_number_decimals(&chars, i, 1);
            out.push(tok);
            i = ni;
        } else if chars[i].is_ascii_digit() {
            let (tok, ni) = parse_fin_number_decimals(&chars, i, 1);
            out.push(tok);
            i = ni;
        } else {
            i += 1;
        }
    }
    out
}

/// Parse one financial number starting at `start` (optional leading `-`).
///
/// At most two decimal digits. Prefer one decimal when the second digit is
/// clearly the integer start of the next dotted number (`4.42.62` → `4.4` +
/// `2.62`, while `48.1140.45` → `48.11` + `40.45`).
fn parse_fin_number(chars: &[char], start: usize) -> (String, usize) {
    parse_fin_number_decimals(chars, start, 2)
}

fn parse_fin_number_decimals(chars: &[char], start: usize, max_decimals: usize) -> (String, usize) {
    let mut i = start;
    let mut s = String::new();
    if i < chars.len() && chars[i] == '-' {
        s.push('-');
        i += 1;
    }
    while i < chars.len() && chars[i].is_ascii_digit() {
        s.push(chars[i]);
        i += 1;
    }
    if max_decimals == 0 || i >= chars.len() || chars[i] != '.' {
        return (s, i);
    }
    s.push('.');
    i += 1;
    let mut taken = 0usize;
    while taken < max_decimals && i < chars.len() && chars[i].is_ascii_digit() {
        if taken == 1 && max_decimals >= 2 {
            // Optional second decimal. Skip when clearly the integer start of
            // the next dotted number: `4.42.62` → `4.4`+`2.62`.
            let next_is_dot = i + 1 < chars.len() && chars[i + 1] == '.';
            if next_is_dot {
                break;
            }
        }
        s.push(chars[i]);
        i += 1;
        taken += 1;
    }
    (s, i)
}

/// Left-edge anchors that appear on multiple rows (rejects one-off jitter phantoms).
fn region_col_lefts_supported(lines: &[&TextLine], fs: f32) -> Vec<f32> {
    if lines.is_empty() {
        return Vec::new();
    }
    let x_tol = left_cluster_tol(fs);
    let raw = region_col_lefts(lines, fs);
    if raw.len() < 2 {
        return raw;
    }
    // Multi-row support: appear on at least ~⅓ of lines (geometric majority of
    // a third of the region — scales with height, no absolute cap).
    let min_support = ((lines.len() + 2) / 3).max(2);
    let hit_tol = x_tol;
    let mut supported: Vec<(f32, usize)> = Vec::new();
    for &cx in &raw {
        let hits = lines
            .iter()
            .filter(|line| {
                line.runs
                    .iter()
                    .any(|r| !r.text.trim().is_empty() && (r.bbox.x0 - cx).abs() <= hit_tol)
            })
            .count();
        if hits >= min_support {
            supported.push((cx, hits));
        }
    }
    if supported.len() < 2 {
        return raw;
    }
    supported.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    let cols: Vec<f32> = supported.iter().map(|(c, _)| *c).collect();
    let collapsed = collapse_near_cols(&cols, lines, x_tol);
    if collapsed.len() >= 2 {
        collapsed
    } else {
        cols
    }
}

/// Exact equal-length schema (legacy strict path).
fn same_schema(a: &[f32], b: &[f32], tol: f32) -> bool {
    if a.len() != b.len() || a.len() < 2 {
        return false;
    }
    a.iter().zip(b.iter()).all(|(x, y)| (*x - *y).abs() <= tol)
}

/// Soft schema match for same column count with jittered left-edges.
/// Different column counts ⇒ different tables (never soft-merge 3-col with 4-col).
fn schemas_compatible(a: &[f32], b: &[f32], tol: f32) -> bool {
    if a.len() < 2 || b.len() < 2 || a.len() != b.len() {
        return false;
    }
    if same_schema(a, b, tol) {
        return true;
    }
    // Equal count: majority of anchors bipartite-match (mild x-jitter).
    let mut used = vec![false; b.len()];
    let mut matched = 0usize;
    for &ax in a {
        let mut best: Option<(usize, f32)> = None;
        for (i, &bx) in b.iter().enumerate() {
            if used[i] {
                continue;
            }
            let d = (ax - bx).abs();
            if d <= tol && best.map_or(true, |(_, bd)| d < bd) {
                best = Some((i, d));
            }
        }
        if let Some((i, _)) = best {
            used[i] = true;
            matched += 1;
        }
    }
    matched * 2 >= a.len() && matched >= 2
}

/// Vertical gap between the last line of `a` and first line of `b` (top→bottom order).
fn region_gap(a: &[&TextLine], b: &[&TextLine]) -> f32 {
    match (a.last(), b.first()) {
        (Some(x), Some(y)) => (x.y - y.y).abs(),
        _ => 0.0,
    }
}

/// Re-merge adjacent regions with the same column skeleton, and bridge short
/// incompatible islands (section-note multi-col lines between table halves).
/// Never merge across a hard vertical gap.
fn merge_same_schema_regions<'a>(
    regions: Vec<Vec<&'a TextLine>>,
    fs: f32,
    hard_gap: f32,
) -> Vec<Vec<&'a TextLine>> {
    if regions.len() <= 1 {
        return regions;
    }
    let tol = left_cluster_tol(fs);

    let adjacent_merge = |regs: Vec<Vec<&'a TextLine>>| -> Vec<Vec<&'a TextLine>> {
        let mut out: Vec<Vec<&TextLine>> = Vec::new();
        for reg in regs {
            if out.is_empty() {
                out.push(reg);
                continue;
            }
            let prev = out.last().unwrap();
            if region_gap(prev, &reg) >= hard_gap {
                out.push(reg);
                continue;
            }
            let sa = region_col_lefts_supported(prev, fs);
            let sb = region_col_lefts_supported(&reg, fs);
            if schemas_compatible(&sa, &sb, tol) {
                out.last_mut().unwrap().extend(reg);
            } else {
                out.push(reg);
            }
        }
        out
    };

    let mut out = adjacent_merge(regions);

    // Bridge A | island | C when island is smaller than a min body table and
    // A/C share schema (section-note multi-col lines between halves).
    let max_island = 3usize; // below stream_min_body_bands default floor
    for _ in 0..8 {
        if out.len() < 3 {
            break;
        }
        let mut next: Vec<Vec<&TextLine>> = Vec::new();
        let mut i = 0;
        let mut changed = false;
        while i < out.len() {
            if i + 2 < out.len() && out[i + 1].len() <= max_island {
                let gap_ac = region_gap(&out[i], &out[i + 2]);
                if gap_ac < hard_gap {
                    let sa = region_col_lefts_supported(&out[i], fs);
                    let sc = region_col_lefts_supported(&out[i + 2], fs);
                    if schemas_compatible(&sa, &sc, tol) {
                        let mut merged = std::mem::take(&mut out[i]);
                        merged.extend(std::mem::take(&mut out[i + 1]));
                        merged.extend(std::mem::take(&mut out[i + 2]));
                        next.push(merged);
                        i += 3;
                        changed = true;
                        continue;
                    }
                }
            }
            next.push(std::mem::take(&mut out[i]));
            i += 1;
        }
        out = adjacent_merge(next);
        if !changed {
            break;
        }
    }
    out
}

/// Merge column anchors closer than half median pitch (jitter double-peaks).
fn collapse_near_cols(cols: &[f32], lines: &[&TextLine], x_tol: f32) -> Vec<f32> {
    if cols.len() < 3 {
        return cols.to_vec();
    }
    let mut gaps: Vec<f32> = cols
        .windows(2)
        .map(|w| w[1] - w[0])
        .filter(|g| *g > 1.0)
        .collect();
    if gaps.is_empty() {
        return cols.to_vec();
    }
    gaps.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let med_gap = gaps[gaps.len() / 2].max(x_tol * 2.0);
    let merge_dist = (0.5 * med_gap).max(x_tol);

    let support = |cx: f32| -> usize {
        lines
            .iter()
            .filter(|line| {
                line.runs
                    .iter()
                    .any(|r| !r.text.trim().is_empty() && (r.bbox.x0 - cx).abs() <= x_tol * 1.2)
            })
            .count()
    };

    let mut out: Vec<(f32, usize)> = cols.iter().map(|&c| (c, support(c))).collect();
    let mut changed = true;
    while changed && out.len() >= 3 {
        changed = false;
        let mut next: Vec<(f32, usize)> = Vec::with_capacity(out.len());
        let mut i = 0;
        while i < out.len() {
            if i + 1 < out.len() && (out[i + 1].0 - out[i].0) <= merge_dist {
                // Keep the stronger-supported anchor (or average if tied).
                let (c0, s0) = out[i];
                let (c1, s1) = out[i + 1];
                let (c, s) = if s0 > s1 {
                    (c0, s0 + s1)
                } else if s1 > s0 {
                    (c1, s0 + s1)
                } else {
                    ((c0 + c1) * 0.5, s0 + s1)
                };
                next.push((c, s));
                i += 2;
                changed = true;
            } else {
                next.push(out[i]);
                i += 1;
            }
        }
        out = next;
    }
    out.into_iter().map(|(c, _)| c).collect()
}

fn assign_col(r: &TextRun, anchors: &[f32], xs: &[f32], ncols: usize, hit_tol: f32) -> usize {
    // Snap left edge to nearest anchor when close.
    let mut best_a: Option<(usize, f32)> = None;
    for (i, &a) in anchors.iter().enumerate() {
        let d = (r.bbox.x0 - a).abs();
        if d <= hit_tol {
            if best_a.map_or(true, |(_, bd)| d < bd) {
                best_a = Some((i, d));
            }
        }
    }
    if let Some((i, _)) = best_a {
        return i.min(ncols - 1);
    }
    let cx = (r.bbox.x0 + r.bbox.x1) * 0.5;
    let mut col = ncols - 1;
    for c in 0..ncols {
        if cx >= xs[c] && cx < xs[c + 1] {
            col = c;
            break;
        }
    }
    col
}

fn build_table_from_lines(
    page_index: u32,
    lines: &[&TextLine],
    opts: &TableOptions,
    fs: f32,
    page_width: f32,
) -> Option<Table> {
    if lines.len() < opts.stream_min_body_bands.max(3) as usize {
        return None;
    }

    let x_tol = left_cluster_tol(fs);
    // Prefer anchors from multi-run lines (headers / true multi-cell rows).
    // Glued single-run body rows all share one left edge and would otherwise
    // starve support-filtered anchors down to a single column.
    let mut supported = region_col_lefts_prefer_rich(lines, fs);
    if supported.len() < 2 {
        supported = region_col_lefts_supported(lines, fs);
    }
    if supported.len() < 2 {
        supported = region_col_lefts(lines, fs);
    }
    // Phase-4: NIPA/BEA-style pages paint each body row as one glued run
    // (label+numbers or pure number stream). Left-edge schema may be 1-col or
    // a weak few-col header skeleton; invent equal-width anchors from the max
    // tokenized field count when that is clearly richer.
    let mut synthetic_glued_cols = false;
    if let Some(syn) = synthesize_cols_from_glued_tokens(lines, fs) {
        if supported.len() < 2 || syn.len() >= supported.len().saturating_add(4) {
            supported = syn;
            synthetic_glued_cols = true;
        }
    }
    if supported.len() < 2 {
        return None;
    }

    // Drop multi-col lines that poorly align with the region's column skeleton
    // (section-note mini-grids, list markers). Keeps real body + header rows.
    // Glued single-run tabular rows are kept (they carry body data).
    let hit_tol = x_tol * 1.25;
    let grid_lines: Vec<&TextLine> = lines
        .iter()
        .copied()
        .filter(|line| {
            let n = line
                .runs
                .iter()
                .filter(|r| !r.text.trim().is_empty())
                .count();
            if n < 2 {
                return n == 1
                    && line
                        .runs
                        .first()
                        .map(|r| {
                            looks_glued_tabular(&r.text)
                                || looks_glued_numeric_stream(&r.text)
                                || looks_nipa_placeholder_row(&r.text)
                                || looks_nipa_section_header(&r.text)
                        })
                        .unwrap_or(false);
            }
            // With synthetic equal-width anchors, skip geometry alignment filter
            // (all glued rows share the same left edge).
            if synthetic_glued_cols {
                return true;
            }
            let aligned = line
                .runs
                .iter()
                .filter(|r| {
                    !r.text.trim().is_empty()
                        && supported
                            .iter()
                            .any(|&cx| (r.bbox.x0 - cx).abs() <= hit_tol)
                })
                .count();
            // Majority of cells land on region anchors.
            aligned >= 2 && aligned * 2 >= n
        })
        .collect();
    let use_lines: &[&TextLine] = if grid_lines.len() >= opts.stream_min_body_bands.max(3) as usize
    {
        &grid_lines
    } else {
        lines
    };

    // Recompute anchors on cleaned lines (rich lines first), unless we already
    // synthesized equal-width columns for a glued-only region.
    if !synthetic_glued_cols {
        supported = region_col_lefts_prefer_rich(use_lines, fs);
        if supported.len() < 2 {
            supported = region_col_lefts_supported(use_lines, fs);
        }
        if supported.len() < 2 {
            supported = region_col_lefts(use_lines, fs);
        }
        if supported.len() < 2 {
            if let Some(syn) = synthesize_cols_from_glued_tokens(use_lines, fs) {
                supported = syn;
                synthetic_glued_cols = true;
            }
        } else if let Some(syn) = synthesize_cols_from_glued_tokens(use_lines, fs) {
            if syn.len() >= supported.len().saturating_add(4) {
                supported = syn;
                synthetic_glued_cols = true;
            }
        }
    }
    if supported.len() < 2 {
        return None;
    }

    // Collapse residual near-duplicate anchors (post-jitter split clusters).
    // Skip for synthetic equal-width glued columns — collapse uses run left-edges
    // and would re-fuse every synthetic anchor to a single left edge.
    if !synthetic_glued_cols {
        supported = collapse_near_cols(&supported, use_lines, x_tol);
    }
    if supported.len() < 2 {
        return None;
    }

    let mut rights: Vec<f32> = Vec::new();
    for line in use_lines {
        for r in &line.runs {
            rights.push(r.bbox.x1);
        }
    }
    let page_right = rights.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    let mut xs = vec![supported[0] - 1.0];
    for w in supported.windows(2) {
        xs.push((w[0] + w[1]) * 0.5);
    }
    xs.push(page_right.max(*supported.last().unwrap() + fs * 4.0) + 1.0);
    xs = cluster_coords(&xs, 1.0);
    xs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let ncols = xs.len().saturating_sub(1);
    if ncols < 2 || ncols as u32 > opts.lattice_max_cols {
        return None;
    }

    let mut nrows = use_lines.len();
    if nrows as u32 > opts.lattice_max_rows {
        return None;
    }

    let centers: Vec<f32> = use_lines.iter().map(|l| l.y).collect();
    let mut ys = Vec::with_capacity(nrows + 1);
    ys.push(centers[0] + fs * 0.7);
    for w in centers.windows(2) {
        ys.push((w[0] + w[1]) * 0.5);
    }
    ys.push(centers[nrows - 1] - fs * 0.7);

    let mut grid: Vec<Vec<String>> = vec![vec![String::new(); ncols]; nrows];
    let mut bboxes: Vec<Vec<Rect>> = vec![
        vec![
            Rect {
                x0: 0.0,
                y0: 0.0,
                x1: 0.0,
                y1: 0.0
            };
            ncols
        ];
        nrows
    ];

    for (ri, line) in use_lines.iter().enumerate() {
        let y1 = ys[ri].max(ys[ri + 1]);
        let y0 = ys[ri].min(ys[ri + 1]);
        // Single wide TJ string painted as one run: split whitespace tokens
        // left-to-right across columns (stream/export tables). Glued
        // label+numeric rows use proportional char-x binning into xs edges.
        // NIPA: glued body often arrives as one long run + a right-margin line#
        // as a second run. Join runs (skip far-right 1–3 digit markers) and
        // token-fill when the joined text is glued tabular.
        if ncols >= 3 && !line.runs.is_empty() {
            let mut sorted: Vec<&TextRun> = line.runs.iter().collect();
            sorted.sort_by(|a, b| {
                a.bbox
                    .x0
                    .partial_cmp(&b.bbox.x0)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            let x_left = sorted
                .iter()
                .find(|r| !r.text.trim().is_empty())
                .map(|r| r.bbox.x0)
                .unwrap_or(0.0);
            // Content runs excluding NIPA right-margin line# markers.
            let content: Vec<&TextRun> = sorted
                .iter()
                .copied()
                .filter(|r| {
                    let t = r.text.trim();
                    if t.is_empty() {
                        return false;
                    }
                    let pure_line = t.chars().all(|c| c.is_ascii_digit()) && t.len() <= 3;
                    !(pure_line && r.bbox.x0 > x_left + 200.0)
                })
                .collect();
            // CRITICAL: multi-run geometry rows (campaign donors, etc.) must NOT
            // be concatenated and force-filled via NIPA glued tokenizer.
            // Joining "MENA"+"JUAN" without spaces invents false letter+digit seams
            // and destroys column assignment (real-track regression 0.94→0.45).
            // Glued path only when ≤1 content run, or synthetic_glued_cols region.
            let use_glued_path = content.len() <= 1 || synthetic_glued_cols;
            if use_glued_path {
                let mut joined = String::new();
                for r in &content {
                    let t = r.text.trim();
                    if t.is_empty() {
                        continue;
                    }
                    // Space-join multi content only for synthetic glued pages
                    // (true multi-run should not reach here with content.len()>1
                    // unless synthetic_glued_cols).
                    if !joined.is_empty() && content.len() > 1 {
                        joined.push(' ');
                    }
                    joined.push_str(t);
                }
                if looks_glued_tabular(&joined)
                    || looks_glued_numeric_stream(&joined)
                    || looks_nipa_placeholder_row(&joined)
                {
                    fill_row_glued_tabular(&joined, &mut grid[ri]);
                    for c in 0..ncols {
                        bboxes[ri][c] = Rect {
                            x0: xs[c],
                            y0,
                            x1: xs[c + 1],
                            y1,
                        };
                    }
                    continue;
                }
                if looks_nipa_section_header(&joined) {
                    // Section banner occupies the label column only.
                    if ncols > 1 {
                        grid[ri][1] = joined;
                    } else {
                        grid[ri][0] = joined;
                    }
                    for c in 0..ncols {
                        bboxes[ri][c] = Rect {
                            x0: xs[c],
                            y0,
                            x1: xs[c + 1],
                            y1,
                        };
                    }
                    continue;
                }
                // Longest single run alone (partial half-line still better than
                // geometry binning of a truncated TJ) — only for true single-run.
                if content.len() == 1 {
                    let run = content[0];
                    if (looks_glued_tabular(&run.text) || looks_glued_numeric_stream(&run.text))
                        && run.text.len() >= 16
                    {
                        fill_row_glued_tabular(&run.text, &mut grid[ri]);
                        for c in 0..ncols {
                            bboxes[ri][c] = Rect {
                                x0: xs[c],
                                y0,
                                x1: xs[c + 1],
                                y1,
                            };
                        }
                        continue;
                    }
                }
            }
        }
        if line.runs.len() == 1 {
            let run = &line.runs[0];
            let tokens: Vec<&str> = run
                .text
                .split_whitespace()
                .filter(|t| !t.is_empty())
                .collect();
            if tokens.len() >= ncols && ncols >= 2 {
                for (ti, tok) in tokens.iter().enumerate() {
                    if ti >= ncols {
                        let col = ncols - 1;
                        if !grid[ri][col].is_empty() {
                            grid[ri][col].push(' ');
                        }
                        grid[ri][col].push_str(tok);
                    } else {
                        grid[ri][ti] = (*tok).to_string();
                    }
                }
                for c in 0..ncols {
                    bboxes[ri][c] = Rect {
                        x0: xs[c],
                        y0,
                        x1: xs[c + 1],
                        y1,
                    };
                }
                continue;
            }
        }
        for r in &line.runs {
            let t = r.text.trim();
            if t.is_empty() {
                continue;
            }
            // Prefer snap-to-anchor when left edge is near a column; else center bin.
            let col = assign_col(r, &supported, &xs, ncols, hit_tol);
            if !grid[ri][col].is_empty() {
                grid[ri][col].push(' ');
            }
            grid[ri][col].push_str(t);
        }
        for c in 0..ncols {
            bboxes[ri][c] = Rect {
                x0: xs[c],
                y0,
                x1: xs[c + 1],
                y1,
            };
        }
    }

    // Expand glued header tokens into empty neighbor cells (StatesTotal, NSSFWMA).
    for row in &mut grid {
        expand_glued_headers_in_row(row);
    }

    // Drop leading caption / unit rows (TABLE 125, (Contd.), `Billion) that
    // inflate liabilities-style stream tables above the real header band.
    while nrows > 6 {
        let joined = grid[0]
            .iter()
            .map(|c| c.trim())
            .filter(|c| !c.is_empty())
            .collect::<Vec<_>>()
            .join(" ")
            .to_ascii_lowercase();
        let caption = joined.contains("table ")
            || joined.contains("contd")
            || joined.contains("billion")
            || joined.contains("(`")
            || joined.contains("( `");
        if !caption {
            break;
        }
        grid.remove(0);
        bboxes.remove(0);
        nrows -= 1;
    }

    // Strong reject: 2-col prose bait (word lists, numbered lists).
    if ncols == 2 {
        let mut alpha_pairs = 0u32;
        let mut rows_ne = 0u32;
        let mut numish = 0u32;
        let mut list_marker = 0u32;
        let mut long_right = 0u32;
        for row in &grid {
            let a = row[0].trim();
            let b = row[1].trim();
            if a.is_empty() && b.is_empty() {
                continue;
            }
            rows_ne += 1;
            let dig = a.chars().filter(|c| c.is_ascii_digit()).count()
                + b.chars().filter(|c| c.is_ascii_digit()).count();
            if dig >= 1 {
                numish += 1;
            }
            let a_alpha = a.chars().any(|c| c.is_alphabetic());
            let b_alpha = b.chars().any(|c| c.is_alphabetic());
            if a_alpha && b_alpha && dig == 0 {
                alpha_pairs += 1;
            }
            // "1." / "(a)" / "•" style markers in col0
            let marker = {
                let t = a.trim_end_matches(|c: char| c == '.' || c == ')' || c == ':');
                let t = t.trim_start_matches('(');
                (t.chars().all(|c| c.is_ascii_digit()) && !t.is_empty() && t.len() <= 3)
                    || (t.len() == 1 && t.chars().next().unwrap().is_ascii_alphabetic())
            };
            if marker {
                list_marker += 1;
            }
            if b.chars().count() >= 28 {
                long_right += 1;
            }
        }
        if rows_ne >= 4
            && (alpha_pairs as f32) / (rows_ne as f32) >= 0.60
            && (numish as f32) / (rows_ne as f32) < 0.20
        {
            return None;
        }
        // Numbered / lettered prose list: short marker col + long prose col.
        if rows_ne >= 4
            && (list_marker as f32) / (rows_ne as f32) >= 0.70
            && (long_right as f32) / (rows_ne as f32) >= 0.50
        {
            return None;
        }
    }

    let mean_chars = {
        let mut n = 0u32;
        let mut ch = 0u32;
        for row in &grid {
            for c in row {
                if c.is_empty() {
                    continue;
                }
                n += 1;
                ch += c.chars().count() as u32;
            }
        }
        if n == 0 {
            0.0
        } else {
            ch as f32 / n as f32
        }
    };
    // Numeric density for non-table area gates.
    let num_dens = {
        let mut n = 0u32;
        let mut dig = 0u32;
        for row in &grid {
            for c in row {
                if c.is_empty() {
                    continue;
                }
                n += 1;
                if c.chars().any(|ch| ch.is_ascii_digit()) {
                    dig += 1;
                }
            }
        }
        if n == 0 {
            0.0
        } else {
            dig as f32 / n as f32
        }
    };

    // Prose bait: long cells + low digit density. Classic 2-col lists, and also
    // multi-col paragraph grids (function words split across invented columns).
    if mean_chars >= opts.stream_max_prose_mean_chars && ncols <= 2 {
        return None;
    }
    if mean_chars >= opts.stream_max_prose_mean_chars * 0.70
        && num_dens < 0.28
        && ncols >= 3
        && nrows <= 12
    {
        return None;
    }
    // Short-token multi-col prose: function words / punctuation shards with
    // sparse numbers (prose mentions "Table 6.1" but is not a data grid).
    if ncols >= 4 && nrows <= 10 && num_dens < 0.35 && mean_chars < 22.0 {
        let mut short_tokens = 0u32;
        let mut tokens = 0u32;
        for row in &grid {
            for c in row {
                let t = c.trim();
                if t.is_empty() {
                    continue;
                }
                tokens += 1;
                if t.chars().count() <= 8 {
                    short_tokens += 1;
                }
            }
        }
        if tokens >= 8 && short_tokens as f32 >= tokens as f32 * 0.50 {
            return None;
        }
    }

    let mut cells: Vec<TableCell> = Vec::new();
    let mut filled = 0u32;
    for r in 0..nrows {
        for c in 0..ncols {
            let text = grid[r][c].clone();
            if !text.is_empty() {
                filled += 1;
            }
            cells.push(TableCell {
                row: r as u32,
                col: c as u32,
                rowspan: 1,
                colspan: 1,
                bbox: bboxes[r][c],
                text,
                is_header: r == 0,
                confidence: 0.85,
            });
        }
    }
    if filled < 4 {
        return None;
    }
    let fill_rate = filled as f32 / (nrows * ncols) as f32;
    if fill_rate < 0.15 && filled < 8 {
        return None;
    }

    let bbox = bbox_of_cells(&cells);

    // Reject very narrow multi-col bands relative to page width when the area
    // looks non-tabular (low numeric density). Only when page width is
    // estimable from body runs and clearly wider than the candidate band —
    // pure synthetic grids (page_width ≈ table span) are unaffected.
    if page_width > 50.0 {
        let x_span = (bbox.x1 - bbox.x0).max(0.0);
        if x_span > 0.0 && x_span < 0.15 * page_width && num_dens < 0.20 && ncols >= 2 {
            return None;
        }
    }
    let conf = (0.55
        + 0.25 * fill_rate.min(1.0)
        + 0.10 * (ncols as f32 / 6.0).min(1.0)
        + 0.10 * (nrows as f32 / 20.0).min(1.0))
    .clamp(0.0, 0.95);
    if conf < opts.min_confidence_stream {
        return None;
    }

    Some(Table {
        bbox,
        page: page_index,
        method: TableMethod::Stream,
        confidence: conf,
        rows: nrows as u32,
        cols: ncols as u32,
        cells,
        header_rows: 1,
        continued_from_previous_page: false,
        continued_to_next_page: false,
        logical_table_id: None,
        strategy_provenance: vec![PipelineId::S5Network],
        notes: vec![format!("network {nrows}x{ncols}")],
        edge_score: 0.0,
        fill_rate,
        weak_edges: false,
        joint_count: 0,
        text_row_recovery: false,
        text_col_recovery: false,
        multitable_stream_recovery: false,
        stream_vs_overwide_hybrid: false,
    })
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
        use pdfparser_ir::{Rect, TextRun};
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
        opts.min_confidence_stream = 0.45;
        opts.min_table_confidence = 0.45;
        let tabs = detect_network_tables(0, &runs, &opts);
        assert!(!tabs.is_empty(), "expected NIPA glued table, got 0");
        assert!(tabs[0].cols >= 4, "cols={}", tabs[0].cols);
        assert!(tabs[0].rows >= 5, "rows={}", tabs[0].rows);
    }
}
