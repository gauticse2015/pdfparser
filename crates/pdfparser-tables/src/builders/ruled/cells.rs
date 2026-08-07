//! Cell tokenize, span merge, footer strip, empty-border trim.
#![allow(clippy::needless_range_loop)]
#![allow(clippy::collapsible_if)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::manual_div_ceil)]
#![allow(clippy::if_same_then_else)]
#![allow(clippy::bool_comparison)]

use super::joints::Edges;
use crate::geom::bbox_of_cells;
use crate::types::{Table, TableCell};
use pdfparser_ir::Rect;

pub(crate) fn redistribute_row_tokens(grid: &mut [Vec<RawCell>]) {
    for row in grid.iter_mut() {
        let ncols = row.len();
        if ncols < 2 {
            continue;
        }
        let nonempty: Vec<usize> = row
            .iter()
            .enumerate()
            .filter(|(_, c)| c.active && !c.text.trim().is_empty())
            .map(|(i, _)| i)
            .collect();
        // --- Path A: single multi-token cell dumps into full/empty row ---
        if nonempty.len() == 1 {
            let src = nonempty[0];
            let tokens: Vec<String> = tokenize_cell(&row[src].text);
            // Full-row tabular dumps only: token count ≈ column count (not 2-token
            // headers like "FY24 LABEL" that must stay in one cell for colspan).
            if tokens.len() >= ncols.saturating_sub(1)
                && tokens.len() <= ncols
                && token_majority_numeric(&tokens)
            {
                let empty_n = row.iter().filter(|c| c.text.trim().is_empty()).count();
                if empty_n + 1 >= tokens.len() {
                    let (start, end) = if tokens.len() == ncols {
                        (0, ncols)
                    } else {
                        let need = tokens.len();
                        let mut s = src.saturating_sub(need.saturating_sub(1) / 2);
                        if s + need > ncols {
                            s = ncols - need;
                        }
                        (s, s + need)
                    };
                    if (start..end).all(|c| c == src || row[c].text.trim().is_empty()) {
                        row[src].text.clear();
                        for (i, tok) in tokens.into_iter().enumerate() {
                            let c = start + i;
                            if c < ncols {
                                row[c].text = tok;
                            }
                        }
                        continue;
                    }
                }
            }
        }

        // --- Path A2: 2-col label|threshold rows with all text dumped in col0 ---
        // "Low-income Less than 50" + empty col1 → split at first threshold phrase.
        if ncols == 2
            && nonempty.len() == 1
            && nonempty[0] == 0
            && row[1].active
            && row[1].text.trim().is_empty()
        {
            let src = row[0].text.trim();
            if let Some((left, right)) = split_label_threshold(src) {
                row[0].text = left;
                row[1].text = right;
                continue;
            }
        }

        // --- Path B: spill multi-numeric tokens right into empty neighbors ---
        // Walk left→right so cascading spills fill a sparse body row.
        for src in 0..ncols {
            if !row[src].active || row[src].text.trim().is_empty() {
                continue;
            }
            let tokens = tokenize_cell(&row[src].text);
            if tokens.len() < 2 || !token_majority_numeric(&tokens) {
                continue;
            }
            // Count contiguous empty active cells to the right.
            let mut right_empty = 0usize;
            for c in (src + 1)..ncols {
                if !row[c].active {
                    break;
                }
                if row[c].text.trim().is_empty() {
                    right_empty += 1;
                } else {
                    break;
                }
            }
            // Need room for tokens beyond the first (kept in src).
            if right_empty + 1 >= tokens.len() {
                // Place one token per column starting at src.
                row[src].text = tokens[0].clone();
                for (i, tok) in tokens.iter().skip(1).enumerate() {
                    let c = src + 1 + i;
                    if c < ncols && row[c].text.trim().is_empty() {
                        row[c].text = tok.clone();
                    }
                }
                continue;
            }
            // Path B2: glued year/number dump landed mid-row with empties on
            // *both* sides (label | · | · | 19901992… | · | ·). Fill empty
            // data columns left→right after the last non-empty label cell.
            let empty_slots: Vec<usize> = (0..ncols)
                .filter(|&c| c != src && row[c].active && row[c].text.trim().is_empty())
                .collect();
            if empty_slots.len() + 1 < tokens.len() {
                continue;
            }
            // Prefer slots from first empty at/after a leading label block.
            let mut label_end = 0usize;
            while label_end < ncols
                && row[label_end].active
                && !row[label_end].text.trim().is_empty()
                && label_end != src
            {
                label_end += 1;
            }
            let mut targets: Vec<usize> = empty_slots
                .iter()
                .copied()
                .filter(|&c| c >= label_end)
                .collect();
            if !targets.contains(&src) {
                targets.push(src);
                targets.sort_unstable();
            }
            if targets.len() < tokens.len() {
                // Include empties left of src if still short.
                targets = empty_slots.clone();
                if !targets.contains(&src) {
                    targets.push(src);
                    targets.sort_unstable();
                }
            }
            if targets.len() < tokens.len() {
                continue;
            }
            row[src].text.clear();
            for (i, tok) in tokens.into_iter().enumerate() {
                if i < targets.len() {
                    row[targets[i]].text = tok;
                }
            }
        }
    }
}

/// Split "Low-income Less than 50" / "Upper-income 120 or more" into label | rest.
pub(crate) fn split_label_threshold(text: &str) -> Option<(String, String)> {
    let t = text.trim();
    if t.len() < 8 {
        return None;
    }
    // Prefer known threshold phrase starts — earliest match wins so
    // "At least 50 and less than 80" splits at "At least", not mid-phrase.
    const MARKERS: &[&str] = &[
        " Less than ",
        " less than ",
        " At least ",
        " at least ",
        " More than ",
        " more than ",
        " Greater than ",
        " greater than ",
        " or more",
        " or less",
    ];
    let mut best: Option<(usize, &'static str)> = None;
    for m in MARKERS {
        if let Some(idx) = t.find(m) {
            if idx >= 3 {
                best = match best {
                    Some((bi, _)) if bi <= idx => best,
                    _ => Some((idx, m)),
                };
            }
        }
    }
    if let Some((idx, m)) = best {
        let left = t[..idx].trim();
        let right = t[idx..].trim();
        if !left.is_empty() && !right.is_empty() {
            // For " 120 or more" style, marker is suffix — find number start
            if m.trim_start().starts_with("or ") {
                let bytes = t.as_bytes();
                let mut i = idx;
                while i > 0 && bytes[i - 1].is_ascii_whitespace() {
                    i -= 1;
                }
                let end_num = i;
                while i > 0
                    && (bytes[i - 1].is_ascii_digit()
                        || bytes[i - 1] == b','
                        || bytes[i - 1] == b'.')
                {
                    i -= 1;
                }
                if i < end_num && i >= 3 {
                    let left = t[..i].trim();
                    let right = t[i..].trim();
                    if !left.is_empty() && right.chars().any(|c| c.is_ascii_digit()) {
                        return Some((left.to_string(), right.to_string()));
                    }
                }
            } else {
                return Some((left.to_string(), right.to_string()));
            }
        }
    }
    // Fallback: first multi-digit number starts the value half.
    let bytes = t.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i].is_ascii_digit() {
            // require ≥2 digits in a row
            let start = i;
            while i < bytes.len()
                && (bytes[i].is_ascii_digit() || bytes[i] == b',' || bytes[i] == b'.')
            {
                i += 1;
            }
            if i - start >= 2 && start >= 4 {
                let left = t[..start].trim();
                let right = t[start..].trim();
                if left.chars().any(|c| c.is_ascii_alphabetic()) && !right.is_empty() {
                    return Some((left.to_string(), right.to_string()));
                }
            }
        } else {
            i += 1;
        }
    }
    None
}

pub(crate) fn tokenize_cell(text: &str) -> Vec<String> {
    let t = text.trim();
    // Glued 4-digit years with no whitespace: "1990199219931994" → years.
    // Geometric tabular pattern (ICDAR year-header rows), not corpus-specific.
    if t.len() >= 8 && t.len() % 4 == 0 && t.chars().all(|c| c.is_ascii_digit()) {
        let years: Vec<String> = t
            .as_bytes()
            .chunks(4)
            .map(|c| String::from_utf8_lossy(c).into_owned())
            .collect();
        if years.iter().all(|y| {
            y.parse::<u32>()
                .ok()
                .map(|n| (1900..=2100).contains(&n))
                .unwrap_or(false)
        }) {
            return years;
        }
    }
    // Glued decimals like "0.3230.2720.2650.290" (no spaces). Cap frac at 3
    // digits when more digits follow (start of next number).
    if t.contains('.')
        && t.chars().all(|c| c.is_ascii_digit() || c == '.')
        && t.matches('.').count() >= 2
    {
        let b = t.as_bytes();
        let mut simple: Vec<String> = Vec::new();
        let mut i = 0usize;
        let mut ok = true;
        while i < b.len() {
            let start = i;
            if !b[i].is_ascii_digit() {
                ok = false;
                break;
            }
            while i < b.len() && b[i].is_ascii_digit() {
                i += 1;
            }
            if i >= b.len() || b[i] != b'.' {
                ok = false;
                break;
            }
            i += 1;
            let frac_start = i;
            while i < b.len() && b[i].is_ascii_digit() {
                i += 1;
                // After 3 frac digits, if another digit remains, it starts the
                // next number ("0.3230.272" → 0.323 | 0.272).
                if i - frac_start >= 3 && i < b.len() && b[i].is_ascii_digit() {
                    break;
                }
            }
            if i == frac_start {
                ok = false;
                break;
            }
            simple.push(t[start..i].to_string());
        }
        if ok && simple.len() >= 2 {
            return simple;
        }
    }
    // Join "11,062 . 6" / "11,062. 6" spaced decimals into one token.
    let parts: Vec<&str> = text.split_whitespace().filter(|t| !t.is_empty()).collect();
    let mut tokens: Vec<String> = Vec::new();
    let mut i = 0;
    while i < parts.len() {
        // pattern: NUM . FRAC
        if i + 2 < parts.len()
            && parts[i].chars().any(|c| c.is_ascii_digit())
            && parts[i + 1] == "."
            && parts[i + 2].chars().all(|c| c.is_ascii_digit())
        {
            tokens.push(format!("{}.{}", parts[i], parts[i + 2]));
            i += 3;
            continue;
        }
        // pattern: NUM. FRAC (dot stuck to num already split wrong)
        if i + 1 < parts.len()
            && parts[i].ends_with('.')
            && parts[i].chars().any(|c| c.is_ascii_digit())
            && parts[i + 1].chars().all(|c| c.is_ascii_digit())
        {
            tokens.push(format!("{}{}", parts[i], parts[i + 1]));
            i += 2;
            continue;
        }
        // pattern: lone ". FRAC" after previous number token → append decimal
        if i + 1 < parts.len()
            && parts[i] == "."
            && parts[i + 1].chars().all(|c| c.is_ascii_digit())
            && !tokens.is_empty()
            && tokens.last().unwrap().chars().any(|c| c.is_ascii_digit())
            && !tokens.last().unwrap().contains('.')
        {
            let prev = tokens.pop().unwrap();
            tokens.push(format!("{prev}.{}", parts[i + 1]));
            i += 2;
            continue;
        }
        // Phase-4: glued numbers without spaces ("804,006671,330636,903")
        // split into comma-grouped numeric tokens.
        let glued = split_glued_numeric(parts[i]);
        if glued.len() > 1 {
            tokens.extend(glued);
        } else {
            tokens.push(parts[i].to_string());
        }
        i += 1;
    }
    // Merge residual "N" + ".N" fragments left as separate tokens.
    let mut merged = Vec::new();
    let mut j = 0;
    while j < tokens.len() {
        if j + 1 < tokens.len()
            && tokens[j].chars().any(|c| c.is_ascii_digit())
            && !tokens[j].contains('.')
            && tokens[j + 1].starts_with('.')
            && tokens[j + 1].chars().skip(1).all(|c| c.is_ascii_digit())
        {
            merged.push(format!("{}{}", tokens[j], tokens[j + 1]));
            j += 2;
        } else {
            merged.push(tokens[j].clone());
            j += 1;
        }
    }
    merged
}

/// Split runs of US-style numbers jammed together without whitespace.
pub(crate) fn split_glued_numeric(s: &str) -> Vec<String> {
    // Fast path: no digits → leave as-is.
    if !s.bytes().any(|b| b.is_ascii_digit()) {
        return vec![s.to_string()];
    }
    // Fast path: at most one thousands comma and no alphabetic junk — ordinary
    // single number ("1,234.5") or plain digits; glued cases have ≥2 commas
    // without separators ("804,006671,330") or multi-group digit runs.
    let comma_n = s.chars().filter(|c| *c == ',').count();
    if comma_n <= 1 && !s.chars().any(|c| c.is_ascii_alphabetic()) {
        // Still may be glued without commas: "804006671330" is rare; comma-glued
        // multi-numbers always have ≥2 commas. Single-comma / no-comma: one token.
        return vec![s.to_string()];
    }
    // Match digit groups possibly with commas/decimals: 1,234.5 or 1234
    let mut out = Vec::new();
    let mut i = 0;
    let chars: Vec<char> = s.chars().collect();
    while i < chars.len() {
        // skip junk
        if !chars[i].is_ascii_digit() && chars[i] != '-' && chars[i] != '+' {
            // keep non-number as own token if starts here
            if out.is_empty() {
                return vec![s.to_string()];
            }
            break;
        }
        let start = i;
        if chars[i] == '-' || chars[i] == '+' {
            i += 1;
        }
        if i >= chars.len() || !chars[i].is_ascii_digit() {
            break;
        }
        // integer part with optional thousands commas
        while i < chars.len() {
            if chars[i].is_ascii_digit() {
                i += 1;
            } else if chars[i] == ','
                && i + 3 < chars.len()
                && chars[i + 1].is_ascii_digit()
                && chars[i + 2].is_ascii_digit()
                && chars[i + 3].is_ascii_digit()
            {
                // only consume comma when next is exactly 3 digits (thousands)
                // but glued next number may start after 3 digits: 804,006671
                // take comma+3digits as part of current number, then if more digits continue new number
                i += 1; // comma
                let mut digs = 0;
                while i < chars.len() && chars[i].is_ascii_digit() && digs < 3 {
                    i += 1;
                    digs += 1;
                }
                if digs == 3 {
                    // if more digits follow without comma, new number starts
                    if i < chars.len() && chars[i].is_ascii_digit() {
                        break; // current number ends; don't consume extra digits
                    }
                    continue;
                } else {
                    break;
                }
            } else {
                break;
            }
        }
        // optional decimal
        if i < chars.len() && chars[i] == '.' {
            i += 1;
            while i < chars.len() && chars[i].is_ascii_digit() {
                i += 1;
            }
        }
        if i > start {
            out.push(chars[start..i].iter().collect());
        } else {
            break;
        }
    }
    if out.len() <= 1 {
        vec![s.to_string()]
    } else {
        out
    }
}

pub(crate) fn token_majority_numeric(tokens: &[String]) -> bool {
    if tokens.is_empty() {
        return false;
    }
    let data_like = tokens
        .iter()
        .filter(|t| t.chars().any(|c| c.is_ascii_digit()))
        .count();
    data_like * 2 >= tokens.len()
}

// ─── Invoice footer / totals row post-process ────────────────────────────────

/// Strip trailing Subtotal/Tax/Total footer rows from an invoice line-item lattice.
///
/// Many invoices draw totals inside the same ruled grid as SKU lines. Lattice
/// geometry correctly finds the full grid; product/gold want the items body only
/// (header + line items). Only runs when the table looks like line items and the
/// trailing block has totals keywords — financial metric grids (Revenue/…) are
/// left intact.
/// Drop fully-empty leading/trailing rows and fully-empty outer columns.
///
/// Ruled grids often include blank frame rows (above header / below body) or
/// gutter columns with no text. Never drops interior empty rows/cols (those may
/// be structural blanks). A single empty outer column on wide grids (≥5 cols)
/// is retained when it is the only edge empty (possible notes/placeholder col).
pub fn trim_empty_border_rows_cols(table: &mut Table) {
    let nrows = table.rows as usize;
    let ncols = table.cols as usize;
    if nrows < 2 || ncols < 2 || table.cells.is_empty() {
        return;
    }
    let mut grid: Vec<Vec<bool>> = vec![vec![false; ncols]; nrows];
    for c in &table.cells {
        let r = c.row as usize;
        let col = c.col as usize;
        if r < nrows && col < ncols && !c.text.trim().is_empty() {
            grid[r][col] = true;
        }
    }
    let row_empty = |r: usize| -> bool { grid[r].iter().all(|&f| !f) };
    let col_empty = |c: usize| -> bool { (0..nrows).all(|r| !grid[r][c]) };

    let mut r0 = 0usize;
    while r0 < nrows && row_empty(r0) {
        r0 += 1;
    }
    let mut r1 = nrows;
    while r1 > r0 && row_empty(r1 - 1) {
        r1 -= 1;
    }
    let mut c0 = 0usize;
    while c0 < ncols && col_empty(c0) {
        c0 += 1;
    }
    let mut c1 = ncols;
    while c1 > c0 && col_empty(c1 - 1) {
        c1 -= 1;
    }
    // Keep a single empty outer column on multi-col grids only when the rest of
    // the span still has content on both sides of that gutter (structural blank
    // / notes column). Do not invent columns; only refuse to strip an existing
    // empty edge that sits next to filled data.
    if ncols >= 5 {
        if c0 == 1 && c1 == ncols {
            c0 = 0;
        }
        if c0 == 0 && c1 == ncols - 1 {
            c1 = ncols;
        }
    }
    // Keep ≥2×2 after trim; require some actual trim.
    if r1 - r0 < 2 || c1 - c0 < 2 {
        return;
    }
    if r0 == 0 && r1 == nrows && c0 == 0 && c1 == ncols {
        return;
    }

    let mut new_cells = Vec::with_capacity(table.cells.len());
    for mut cell in table.cells.drain(..) {
        let r = cell.row as usize;
        let c = cell.col as usize;
        if r < r0 || r >= r1 || c < c0 || c >= c1 {
            continue;
        }
        // Clamp span into surviving window.
        let max_rowspan = (r1 - r) as u32;
        let max_colspan = (c1 - c) as u32;
        cell.row = (r - r0) as u32;
        cell.col = (c - c0) as u32;
        if cell.rowspan > max_rowspan {
            cell.rowspan = max_rowspan.max(1);
        }
        if cell.colspan > max_colspan {
            cell.colspan = max_colspan.max(1);
        }
        new_cells.push(cell);
    }
    if new_cells.is_empty() {
        return;
    }
    let new_rows = (r1 - r0) as u32;
    let new_cols = (c1 - c0) as u32;
    table.cells = new_cells;
    table.rows = new_rows;
    table.cols = new_cols;
    table.bbox = bbox_of_cells(&table.cells);
    let filled = table
        .cells
        .iter()
        .filter(|c| !c.text.trim().is_empty())
        .count();
    let total = (new_rows as usize).saturating_mul(new_cols as usize).max(1);
    table.fill_rate = filled as f32 / total as f32;
    table.notes.push(format!(
        "trim_empty_border r0={r0} r1={r1} c0={c0} c1={c1} -> {new_rows}x{new_cols}"
    ));
}

pub(crate) fn strip_trailing_footer_totals(table: &mut Table) {
    let nrows = table.rows as usize;
    let ncols = table.cols as usize;
    if nrows < 3 || ncols < 2 || table.cells.is_empty() {
        return;
    }

    let mut grid: Vec<Vec<String>> = vec![vec![String::new(); ncols]; nrows];
    for c in &table.cells {
        let r = c.row as usize;
        let col = c.col as usize;
        if r < nrows && col < ncols {
            // Prefer first non-empty if duplicates (span placeholders).
            if grid[r][col].trim().is_empty() && !c.text.trim().is_empty() {
                grid[r][col] = c.text.clone();
            } else if grid[r][col].is_empty() {
                grid[r][col] = c.text.clone();
            }
        }
    }

    let header = &grid[0];
    let body = &grid[1..];
    if !looks_like_invoice_line_items(header, body) {
        return;
    }

    // Walk up from the bottom while rows look like totals footers.
    let mut cut = nrows;
    while cut > 1 {
        let r = cut - 1;
        if is_footer_totals_row(&grid[r]) {
            cut -= 1;
        } else {
            break;
        }
    }
    // Keep header + ≥1 body row; require at least one strip.
    if cut < 2 || cut >= nrows {
        return;
    }
    // Safety: stripped block must carry an explicit totals keyword.
    let stripped_has_kw = (cut..nrows).any(|r| row_has_totals_keyword(&grid[r]));
    if !stripped_has_kw {
        return;
    }

    let n_stripped = nrows - cut;
    table.cells.retain(|c| (c.row as usize) < cut);
    table.rows = cut as u32;
    if !table.cells.is_empty() {
        table.bbox = bbox_of_cells(&table.cells);
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
        .push(format!("footer_totals_stripped n={n_stripped}"));
}

pub(crate) fn looks_like_invoice_line_items(header: &[String], body: &[Vec<String>]) -> bool {
    let mut hits = 0u32;
    for cell in header {
        let t = cell.trim().to_lowercase();
        if t.is_empty() {
            continue;
        }
        if matches!(
            t.as_str(),
            "sku"
                | "qty"
                | "quantity"
                | "description"
                | "unit"
                | "amount"
                | "price"
                | "item"
                | "total"
                | "line"
                | "#"
                | "no"
                | "no."
                | "part"
                | "code"
                | "product"
                | "desc"
                | "cost"
        ) || t == "unit price"
            || t == "line total"
            || t == "item #"
            || t == "part no"
            || t == "part no."
            || t.contains("sku")
            || t == "qty."
        {
            hits += 1;
        }
    }
    if hits >= 2 {
        return true;
    }
    if body.is_empty() {
        return false;
    }
    let skuish = body
        .iter()
        .filter(|r| {
            let c0 = r.first().map(|s| s.trim()).unwrap_or("");
            let c1 = r.get(1).map(|s| s.trim()).unwrap_or("");
            is_line_item_id(c0) || is_line_item_id(c1)
        })
        .count();
    // Body-only path: need SKU-like IDs AND money-like amounts so statistical
    // grids with numeric col-0 indices never look like invoices.
    let moneyish = body
        .iter()
        .filter(|r| {
            r.iter().any(|c| {
                let t = c.trim();
                t.contains('$')
                    || t.contains('€')
                    || t.contains('£')
                    || (t.contains('.')
                        && t.chars().filter(|ch| ch.is_ascii_digit()).count() >= 3
                        && t.chars().all(|ch| {
                            ch.is_ascii_digit() || ch == '.' || ch == ',' || ch == '-' || ch == ' '
                        }))
            })
        })
        .count();
    skuish * 2 >= body.len() && moneyish * 2 >= body.len()
}

pub(crate) fn is_line_item_id(s: &str) -> bool {
    if s.is_empty() || cell_is_totals_label(s) {
        return false;
    }
    // Pure digits: product/line codes are typically 3–6 digits. Single-digit
    // statistical row indices (0..N reclassification tables) must NOT look like
    // invoice SKUs or footer-strip kills legitimate Total rows on ICDAR grids.
    if s.chars().all(|c| c.is_ascii_digit()) && (3..=6).contains(&s.len()) {
        return true;
    }
    let upper = s.to_ascii_uppercase();
    if upper.starts_with("SKU") {
        return true;
    }
    let has_digit = s.chars().any(|c| c.is_ascii_digit());
    let has_alpha = s.chars().any(|c| c.is_ascii_alphabetic());
    has_digit && has_alpha && s.len() <= 16
}

pub(crate) fn is_footer_totals_row(cells: &[String]) -> bool {
    let filled: Vec<&str> = cells
        .iter()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();
    if filled.is_empty() {
        return false;
    }
    if !row_has_totals_keyword(cells) {
        return false;
    }
    let n = cells.len().max(1);
    let sparse = filled.len() as f32 / n as f32 <= 0.55;
    let first = cells.first().map(|s| s.trim()).unwrap_or("");
    let first_empty = first.is_empty();
    let first_totals = cell_is_totals_label(first);
    let left_half = (n + 1) / 2;
    let left_empty = cells
        .iter()
        .take(left_half)
        .filter(|c| c.trim().is_empty())
        .count();
    let left_mostly_empty = left_empty as f32 / left_half.max(1) as f32 >= 0.5;
    // Dense "Total" summary rows (age-cohort / category totals with values in
    // every column) must stay — only sparse / left-empty invoice footers strip.
    // first_totals alone used to kill those statistical totals.
    if first_totals {
        return sparse || first_empty || left_mostly_empty;
    }
    sparse || first_empty || left_mostly_empty
}

pub(crate) fn row_has_totals_keyword(cells: &[String]) -> bool {
    cells.iter().any(|c| cell_is_totals_label(c))
}

/// True when cell text is a totals/footer label (Subtotal, Tax, Amount Due, …).
pub(crate) fn cell_is_totals_label(s: &str) -> bool {
    let t = s
        .trim()
        .to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if t.is_empty() {
        return false;
    }
    const PHRASES: &[&str] = &[
        "subtotal",
        "sub total",
        "grand total",
        "amount due",
        "balance due",
        "total due",
        "net due",
        "sales tax",
        "amount payable",
        "balance payable",
        "total amount",
        "invoice total",
        "order total",
        "net total",
        "tax total",
    ];
    for p in PHRASES {
        if t.contains(p) {
            return true;
        }
    }
    // Tax as primary label — not "tax rate" notes on metric rows.
    if (t.starts_with("tax") && !t.starts_with("tax rate"))
        || t == "vat"
        || t.starts_with("vat ")
        || t == "gst"
        || t.starts_with("gst ")
    {
        return true;
    }
    // "Total" / "Total TOKEN_…" / short "… total" labels.
    if t == "total" || t.starts_with("total ") {
        return true;
    }
    if t.ends_with(" total") && t.len() < 40 {
        return true;
    }
    false
}

pub(crate) struct RawCell {
    pub(crate) bbox: Rect,
    pub(crate) text: String,
    pub(crate) edges: Edges,
    pub(crate) active: bool,
    pub(crate) colspan: u32,
    pub(crate) rowspan: u32,
}
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

/// Drop interior columns that are almost entirely empty (densify / exterior-stub
/// artifacts). Keeps first and last column always; requires ≥4 columns.
///
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
