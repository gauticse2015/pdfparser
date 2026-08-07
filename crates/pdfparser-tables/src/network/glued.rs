//! Glued-token / NIPA / schema helpers for borderless network tables.
#![allow(clippy::if_same_then_else)]
#![allow(clippy::manual_div_ceil)]
#![allow(clippy::collapsible_if)]
#![allow(clippy::needless_range_loop)]

use super::TextLine;
use crate::geom::cluster_coords;
use pdfparser_ir::TextRun;

/// Single-run body rows that glue a text label to dense numerics without
/// whitespace (`"Andhra Pradesh48.1140.45-3.26…"`). Common in RBI/Excel PDF
/// exports; still a multi-column table row for area proposal + char-x split.
///
/// Also BEA NIPA: `"1Gross domestic product (GDP)5.81.92.5…"` where the first
/// seam is often `)` + digit rather than letter + digit.
pub(crate) fn looks_glued_tabular(s: &str) -> bool {
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
pub(crate) fn looks_glued_numeric_stream(s: &str) -> bool {
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
pub(crate) fn looks_nipa_placeholder_row(s: &str) -> bool {
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
pub(crate) fn looks_nipa_section_header(s: &str) -> bool {
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
pub(crate) fn glued_field_count(text: &str) -> usize {
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
pub(crate) fn synthesize_cols_from_glued_tokens(lines: &[&TextLine], fs: f32) -> Option<Vec<f32>> {
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
pub(crate) fn left_cluster_tol(fs: f32) -> f32 {
    (0.75 * fs).max(3.0)
}

/// Raw area split on ordered multi-col lines (top→bottom).
///
/// Hard-gap branch is unconditional: even identical column schemas become
/// separate areas when the vertical separation is ≥ `hard_gap` (3× soft).
pub(crate) fn split_multi_regions<'a>(
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
pub(crate) fn region_col_lefts(lines: &[&TextLine], fs: f32) -> Vec<f32> {
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
pub(crate) fn region_col_lefts_prefer_rich(lines: &[&TextLine], fs: f32) -> Vec<f32> {
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
pub(crate) fn fill_row_glued_tabular(text: &str, row: &mut [String]) {
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

/// Assign whitespace tokens to columns by proportional char-x inside a single
/// text run's bbox. Used when token count ≠ ncols (multi-word header cells).
pub(crate) fn fill_row_whitespace_by_x(
    run: &TextRun,
    tokens: &[&str],
    row: &mut [String],
    xs: &[f32],
) {
    let ncols = row.len();
    if ncols == 0 || xs.len() < 2 {
        return;
    }
    for cell in row.iter_mut() {
        cell.clear();
    }
    let full = run.text.as_str();
    let x0 = run.bbox.x0;
    let x1 = run.bbox.x1;
    let width = (x1 - x0).max(1.0);
    let nchars = full.chars().count().max(1) as f32;
    let mut search_from = 0usize;
    for tok in tokens {
        let Some(rel) = full[search_from..].find(tok) else {
            continue;
        };
        let start = search_from + rel;
        let mid = start + tok.chars().count() / 2;
        let x = x0 + (mid as f32 / nchars) * width;
        let mut col = ncols - 1;
        for c in 0..ncols {
            if x >= xs[c] && (c + 1 >= ncols || x < xs[c + 1]) {
                col = c;
                break;
            }
        }
        if !row[col].is_empty() {
            row[col].push(' ');
        }
        row[col].push_str(tok);
        search_from = start + tok.len();
    }
}

/// Expand known/CamelCase glued headers into empty right-neighbor cells.
pub(crate) fn expand_glued_headers_in_row(row: &mut [String]) {
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
pub(crate) fn split_glued_header_label(label: &str) -> Vec<String> {
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
pub(crate) fn tokenize_numeric_tail(tail: &str) -> Vec<String> {
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
pub(crate) fn tokenize_numeric_tail_signed(tail: &str) -> Vec<String> {
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
pub(crate) fn parse_fin_number(chars: &[char], start: usize) -> (String, usize) {
    parse_fin_number_decimals(chars, start, 2)
}

pub(crate) fn parse_fin_number_decimals(
    chars: &[char],
    start: usize,
    max_decimals: usize,
) -> (String, usize) {
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
pub(crate) fn region_col_lefts_supported(lines: &[&TextLine], fs: f32) -> Vec<f32> {
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
pub(crate) fn same_schema(a: &[f32], b: &[f32], tol: f32) -> bool {
    if a.len() != b.len() || a.len() < 2 {
        return false;
    }
    a.iter().zip(b.iter()).all(|(x, y)| (*x - *y).abs() <= tol)
}

/// Soft schema match for same column count with jittered left-edges.
/// Different column counts ⇒ different tables (never soft-merge 3-col with 4-col).
pub(crate) fn schemas_compatible(a: &[f32], b: &[f32], tol: f32) -> bool {
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
pub(crate) fn region_gap(a: &[&TextLine], b: &[&TextLine]) -> f32 {
    match (a.last(), b.first()) {
        (Some(x), Some(y)) => (x.y - y.y).abs(),
        _ => 0.0,
    }
}

/// Re-merge adjacent regions with the same column skeleton, and bridge short
/// incompatible islands (section-note multi-col lines between table halves).
/// Never merge across a hard vertical gap.
pub(crate) fn merge_same_schema_regions<'a>(
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
pub(crate) fn collapse_near_cols(cols: &[f32], lines: &[&TextLine], x_tol: f32) -> Vec<f32> {
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
