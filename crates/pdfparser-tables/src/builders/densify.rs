//! Text densify / thin-gap / empty-column helpers for ruled lattice grids.
//!
//! Extracted from the ruled builder so densify policy stays reviewable and
//! can be gated via [`crate::TableOptions::lattice_text_densify`].
//!
//! Thresholds that often need document-type tuning flow through
//! [`DensifyParams`] (sourced from [`crate::TableTuning`]).

use crate::geom::cluster_coords;
use crate::types::TableCell;
use pdfparser_ir::TextRun;

/// Subset of [`crate::TableTuning`] consumed by densify helpers.
#[derive(Debug, Clone, Copy)]
pub(crate) struct DensifyParams {
    /// Max CV of text-band pitch for regular densify (default 0.45).
    pub pitch_cv_max: f32,
    /// Exterior expand pad as fraction of frame width (default 0.55).
    pub exterior_pad_frac: f32,
    /// Max char count for short densify-X token candidates (default 14).
    pub short_token_chars: usize,
}

impl Default for DensifyParams {
    fn default() -> Self {
        let d = crate::TableTuning::default();
        Self {
            pitch_cv_max: d.densify_pitch_cv_max,
            exterior_pad_frac: d.densify_x_exterior_pad_frac,
            short_token_chars: d.densify_x_short_token_chars as usize,
        }
    }
}

/// of rows, then blocked densify (`text_row_recovery` short-circuit).
pub(crate) fn collapse_overdense_h_from_text(
    y_ttb: &[f32],
    runs: &[TextRun],
    frame_x0: f32,
    frame_x1: f32,
    min_cell: f32,
    overdense_factor: f32,
) -> Option<(Vec<f32>, Vec<f32>)> {
    if y_ttb.len() < 4 {
        return None;
    }
    let multi = multi_col_band_centers(runs, frame_x0, frame_x1, y_ttb, min_cell);
    if multi.len() < 3 {
        return None;
    }
    // All non-empty text bands (incl. single-run). Dense numeric grids often
    // have one TextRun per cell — multi-col alone under-represents body rows.
    let all = all_text_band_centers(runs, frame_x0, frame_x1, y_ttb, min_cell);
    // Only treat as underline noise when multi-col bands dominate text structure.
    // If single-run body bands are the majority, H density likely matches real rows.
    if all.len() >= multi.len().saturating_mul(2) {
        return None;
    }
    let h_rows = y_ttb.len().saturating_sub(1);
    // Compare H density to multi-col bands only when they represent structure.
    if (h_rows as f32) < multi.len() as f32 * overdense_factor.max(1.15) {
        return None;
    }
    // Extra guard: if all-band count is near H row count, grid is consistent — keep H.
    if !all.is_empty() && (h_rows as f32) <= all.len() as f32 * 1.25 + 2.0 {
        return None;
    }
    // Outer frame from existing H extremes; separators between consecutive text bands.
    let y_top = y_ttb.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    let y_bot = y_ttb.iter().copied().fold(f32::INFINITY, f32::min);
    let mut anchors = vec![y_top];
    let mut synth = Vec::new();
    for w in multi.windows(2) {
        let mid = (w[0] + w[1]) * 0.5;
        if (anchors.last().copied().unwrap_or(y_top) - mid).abs() >= min_cell * 0.8 {
            anchors.push(mid);
            synth.push(mid);
        }
    }
    anchors.push(y_bot);
    anchors.sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));
    let collapsed = collapse_thin_gaps(&anchors, min_cell * 0.85);
    let mut collapsed = collapsed;
    collapsed.sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));
    if collapsed.len().saturating_sub(1) < 2 {
        return None;
    }
    // Must reduce row count toward text bands.
    if collapsed.len().saturating_sub(1) >= h_rows {
        return None;
    }
    Some((collapsed, synth))
}

/// Multi-col text band centers (top-to-bottom) inside frame defined by y_ttb extremes.
fn multi_col_band_centers(
    runs: &[TextRun],
    frame_x0: f32,
    frame_x1: f32,
    y_ttb: &[f32],
    min_cell: f32,
) -> Vec<f32> {
    text_band_centers(
        runs, frame_x0, frame_x1, y_ttb, min_cell, /* multi_only */ true,
    )
}

/// All non-empty text band centers (single-run body rows included).
fn all_text_band_centers(
    runs: &[TextRun],
    frame_x0: f32,
    frame_x1: f32,
    y_ttb: &[f32],
    min_cell: f32,
) -> Vec<f32> {
    text_band_centers(
        runs, frame_x0, frame_x1, y_ttb, min_cell, /* multi_only */ false,
    )
}

/// Band Y centers inside the ruled frame.
///
/// `multi_only`: require ≥2 runs per band (column evidence). When false, any
/// non-empty band counts — needed for dense numeric grids with one run/cell.
fn text_band_centers(
    runs: &[TextRun],
    frame_x0: f32,
    frame_x1: f32,
    y_ttb: &[f32],
    min_cell: f32,
    multi_only: bool,
) -> Vec<f32> {
    let y_top = y_ttb.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    let y_bot = y_ttb.iter().copied().fold(f32::INFINITY, f32::min);
    if !y_top.is_finite() || !y_bot.is_finite() {
        return Vec::new();
    }
    let pad = 1.0f32;
    let inside: Vec<&TextRun> = runs
        .iter()
        .filter(|r| {
            if r.text.trim().is_empty() {
                return false;
            }
            let cx = (r.bbox.x0 + r.bbox.x1) * 0.5;
            let cy = r.bbox.y_center();
            cx >= frame_x0 - pad && cx <= frame_x1 + pad && cy <= y_top + pad && cy >= y_bot - pad
        })
        .collect();
    if inside.len() < 4 {
        return Vec::new();
    }
    let fs = {
        let mut v: Vec<f32> = inside
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
    let y_tol = (0.45 * fs).max(2.0);
    let mut items = inside;
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
    let mut bands: Vec<Vec<&TextRun>> = Vec::new();
    for r in items {
        if let Some(band) = bands.last_mut() {
            let by = band.iter().map(|x| x.bbox.y_center()).sum::<f32>() / band.len() as f32;
            if (r.bbox.y_center() - by).abs() <= y_tol {
                band.push(r);
                continue;
            }
        }
        bands.push(vec![r]);
    }
    let mut centers: Vec<f32> = bands
        .iter()
        .filter(|b| {
            if multi_only {
                b.len() >= 2
            } else {
                !b.is_empty()
            }
        })
        .map(|b| b.iter().map(|r| r.bbox.y_center()).sum::<f32>() / b.len() as f32)
        .filter(|&c| c < y_top - min_cell * 0.15 && c > y_bot + min_cell * 0.15)
        .collect();
    centers = cluster_coords(&centers, y_tol * 0.6);
    centers.sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));
    centers
}

/// Densify missing vertical anchors from multi-row text left-edges.
///
/// Geometric rule only:
/// Expand ruled X anchors when multi-row text columns sit just outside the
/// lattice frame (stub labels / line numbers on statistical tables).
///
/// Geometric rule: left-edges that hit ≥⅓ of table body bands, lie within
/// ~0.55× frame width of the nearest frame edge, and are not already inside
/// an existing cell gap of the ruled skeleton.
pub(crate) fn expand_xs_exterior_text_cols(
    xs: &[f32],
    runs: &[TextRun],
    frame_y_top: f32,
    frame_y_bot: f32,
    min_cell: f32,
    params: &DensifyParams,
) -> Vec<f32> {
    if xs.len() < 2 {
        return xs.to_vec();
    }
    let mut xs_sorted = xs.to_vec();
    xs_sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let x0 = xs_sorted[0];
    let x1 = *xs_sorted.last().unwrap();
    let frame_w = (x1 - x0).abs().max(1.0);
    let y_hi = frame_y_top.max(frame_y_bot);
    let y_lo = frame_y_top.min(frame_y_bot);
    let y_pad = min_cell.max(2.0);
    // Search a bit outside the frame for stub columns.
    let search_pad =
        (frame_w * params.exterior_pad_frac).clamp(min_cell * 4.0, frame_w.max(min_cell * 8.0));

    let inside: Vec<&TextRun> = runs
        .iter()
        .filter(|r| {
            if r.text.trim().is_empty() {
                return false;
            }
            let cy = r.bbox.y_center();
            cy <= y_hi + y_pad && cy >= y_lo - y_pad
        })
        .collect();
    if inside.len() < 6 {
        return xs.to_vec();
    }
    let fs = {
        let mut v: Vec<f32> = inside
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
    let x_tol = (0.5 * fs).max(min_cell);
    let y_tol = (0.5 * fs).max(2.0);
    let v_cols = xs_sorted.len().saturating_sub(1);
    // Prefer left-edges of short/numeric body cells (BEA line stubs, labels).
    // Sparse-V frames (2–3 cols) also accept short alpha stubs (label columns
    // outside the ruled number grid) — geometric, not corpus-specific.
    let mut lefts: Vec<f32> = inside
        .iter()
        .filter(|r| {
            let t = r.text.trim();
            if t.is_empty() {
                return false;
            }
            let dig = t.chars().any(|c| c.is_ascii_digit());
            let short = t.chars().count() <= params.short_token_chars;
            let alpha_stub = v_cols <= 3
                && short
                && t.chars().any(|c| c.is_ascii_alphabetic())
                && t.chars().count() <= 10;
            dig || short || alpha_stub
        })
        .map(|r| r.bbox.x0)
        .collect();
    if lefts.len() < 4 {
        lefts = inside.iter().map(|r| r.bbox.x0).collect();
    }
    lefts = cluster_coords(&lefts, x_tol);
    let mut band_ys: Vec<f32> = inside.iter().map(|r| r.bbox.y_center()).collect();
    band_ys = cluster_coords(&band_ys, y_tol);
    if band_ys.len() < 4 {
        return xs.to_vec();
    }
    // Sparse-V: lower hit floor so a consistent stub col (3–4 rows) still expands.
    let min_hits = if v_cols <= 3 {
        ((band_ys.len() + 3) / 4).max(2)
    } else {
        ((band_ys.len() + 2) / 3).max(2)
    };
    let mut exterior: Vec<f32> = Vec::new();
    for &cand in &lefts {
        let left_of = cand < x0 - min_cell * 0.5;
        let right_of = cand > x1 + min_cell * 0.5;
        if !left_of && !right_of {
            continue;
        }
        let dist = if left_of { x0 - cand } else { cand - x1 };
        if dist > search_pad || dist < min_cell * 0.35 {
            continue;
        }
        let rows_hit = band_ys
            .iter()
            .filter(|&&by| {
                inside.iter().any(|r| {
                    (r.bbox.y_center() - by).abs() <= y_tol && (r.bbox.x0 - cand).abs() <= x_tol
                })
            })
            .count();
        if rows_hit >= min_hits {
            exterior.push(cand);
        }
    }
    if exterior.is_empty() {
        return xs.to_vec();
    }
    exterior = cluster_coords(&exterior, x_tol);
    exterior.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    let n_orig = xs_sorted.len();
    let mut out = xs_sorted;
    // Prepend left exterior columns with separators just left of each left-edge.
    let mut left_ext: Vec<f32> = exterior.iter().copied().filter(|&c| c < x0).collect();
    left_ext.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    if !left_ext.is_empty() {
        let mut new_left = Vec::new();
        // Outer left edge: a bit left of first stub.
        let outer = left_ext[0] - min_cell.max(fs * 0.8);
        new_left.push(outer);
        for w in left_ext.windows(2) {
            // Separator between consecutive exterior cols.
            let mid = (w[0] + w[1]) * 0.5;
            new_left.push(mid);
        }
        // Separator between last exterior and old frame left.
        let last = *left_ext.last().unwrap();
        let join = (last + x0) * 0.5;
        if join > outer + min_cell && x0 - join >= min_cell * 0.5 {
            new_left.push(join);
        }
        new_left.extend(out.iter().copied());
        out = new_left;
    }
    // Append right exterior columns.
    let mut right_ext: Vec<f32> = exterior.iter().copied().filter(|&c| c > x1).collect();
    right_ext.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    if !right_ext.is_empty() {
        let first_r = right_ext[0];
        let join = (x1 + first_r) * 0.5;
        if join > x1 + min_cell * 0.25 && !out.iter().any(|&x| (x - join).abs() < min_cell * 0.5) {
            out.push(join);
        }
        for w in right_ext.windows(2) {
            let mid = (w[0] + w[1]) * 0.5;
            if mid > *out.last().unwrap() + min_cell {
                out.push(mid);
            }
        }
        let last = *right_ext.last().unwrap();
        let outer = last + min_cell.max(fs * 0.8);
        if outer > *out.last().unwrap() + min_cell * 0.5 {
            out.push(outer);
        }
    }
    out = cluster_coords(&out, min_cell * 0.35);
    out.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    if out.len() <= n_orig {
        return xs.to_vec();
    }
    out
}

/// 1. Cluster left-edges that recur across many text bands (true columns).
/// 2. Fire only when those columns **outnumber** ruled V gaps (partial-V).
/// 3. Inside each V gap, if ≥2 text columns **span a majority of the gap**,
///    insert separators at midpoints between consecutive text left-edges.
///
/// Full-V multi-token cells fail (2): second-word lefts sit near the primary
/// left and do not outnumber V gaps after multi-row filtering.
pub(crate) fn densify_x_from_text_cols(
    xs: &[f32],
    runs: &[TextRun],
    frame_y_top: f32,
    frame_y_bot: f32,
    min_cell: f32,
    params: &DensifyParams,
) -> (Vec<f32>, Vec<f32>) {
    if xs.len() < 2 {
        return (xs.to_vec(), Vec::new());
    }

    let x0 = xs[0];
    let x1 = *xs.last().unwrap_or(&x0);
    let y_hi = frame_y_top.max(frame_y_bot);
    let y_lo = frame_y_top.min(frame_y_bot);
    let pad = min_cell.max(1.0);
    let inside: Vec<&TextRun> = runs
        .iter()
        .filter(|r| {
            if r.text.trim().is_empty() {
                return false;
            }
            let cx = (r.bbox.x0 + r.bbox.x1) * 0.5;
            let cy = r.bbox.y_center();
            cx >= x0 - pad && cx <= x1 + pad && cy <= y_hi + pad && cy >= y_lo - pad
        })
        .collect();
    if inside.len() < 4 {
        return (xs.to_vec(), Vec::new());
    }
    let fs = {
        let mut v: Vec<f32> = inside
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
    let x_tol = (0.5 * fs).max(min_cell);
    let y_tol = (0.5 * fs).max(2.0);
    // Prefer left-edges of short/numeric body cells.
    let mut lefts: Vec<f32> = inside
        .iter()
        .filter(|r| {
            let t = r.text.trim();
            if t.is_empty() {
                return false;
            }
            let dig = t.chars().any(|c| c.is_ascii_digit());
            let short = t.chars().count() <= params.short_token_chars;
            dig || short
        })
        .map(|r| r.bbox.x0)
        .collect();
    if lefts.len() < 4 {
        lefts = inside.iter().map(|r| r.bbox.x0).collect();
    }
    lefts = cluster_coords(&lefts, x_tol);
    let mut band_ys: Vec<f32> = inside.iter().map(|r| r.bbox.y_center()).collect();
    band_ys = cluster_coords(&band_ys, y_tol);
    if band_ys.len() < 2 {
        return (xs.to_vec(), Vec::new());
    }
    let v_cols_probe = xs.len().saturating_sub(1);
    // Outer-frame: weaker multi-row support for statistical grids.
    let min_hits = if v_cols_probe <= 3 {
        ((band_ys.len() + 6) / 8).max(2)
    } else {
        ((band_ys.len() + 2) / 3).max(2)
    };
    let mut col_xs: Vec<f32> = Vec::new();
    // Seed: left-edges. Sparse-V only: complete-number lefts inside multi-number
    // Multi-number left fractions only on sparse outer frames.
    let mut cands = lefts.clone();
    if v_cols_probe <= 3 {
        for r in &inside {
            let fracs = multi_number_left_fracs(&r.text);
            if fracs.len() < 2 {
                continue;
            }
            let span = (r.bbox.x1 - r.bbox.x0).max(1.0);
            for &frac in &fracs {
                cands.push(r.bbox.x0 + frac * span);
            }
        }
    }
    cands = cluster_coords(&cands, x_tol * 0.65);
    for &cand in &cands {
        if (cand - x0).abs() < min_cell * 0.12 || (cand - x1).abs() < min_cell * 0.12 {
            continue;
        }
        if cand < x0 - min_cell || cand > x1 + min_cell {
            continue;
        }
        let rows_hit = band_ys
            .iter()
            .filter(|&&by| {
                inside.iter().any(|r| {
                    if (r.bbox.y_center() - by).abs() > y_tol {
                        return false;
                    }
                    let near_left = (r.bbox.x0 - cand).abs() <= x_tol;
                    if near_left {
                        return true;
                    }
                    // Sparse-V only: multi-number run with a number left near cand.
                    if v_cols_probe > 3 {
                        return false;
                    }
                    let fracs = multi_number_left_fracs(&r.text);
                    if fracs.len() < 2 {
                        return false;
                    }
                    let span = (r.bbox.x1 - r.bbox.x0).max(1.0);
                    fracs.iter().any(|&f| {
                        let lx = r.bbox.x0 + f * span;
                        (lx - cand).abs() <= x_tol
                    })
                })
            })
            .count();
        if rows_hit >= min_hits {
            col_xs.push(cand);
        }
    }
    col_xs = cluster_coords(&col_xs, x_tol * 0.65);
    col_xs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    if v_cols_probe <= 3 && col_xs.len() > 11 {
        col_xs = cluster_coords(&col_xs, x_tol * 1.0);
        col_xs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    }

    let v_cols = xs.len().saturating_sub(1);
    if v_cols < 1 || col_xs.len() <= v_cols {
        return (xs.to_vec(), Vec::new());
    }
    // strong_partial only for sparse outer-frame (few ruled V columns).
    let strong_partial =
        col_xs.len() >= v_cols.saturating_mul(2) && col_xs.len() >= 4 && v_cols <= 3;

    let mut xs_sorted = xs.to_vec();
    xs_sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    let mut under_ruled = 0u32;
    for w in xs_sorted.windows(2) {
        let g0 = w[0];
        let g1 = w[1];
        let gap_w = (g1 - g0).abs();
        if gap_w < min_cell * 2.0 {
            continue;
        }
        let mut in_gap: Vec<f32> = col_xs
            .iter()
            .copied()
            .filter(|&c| c > g0 + min_cell * 0.25 && c < g1 - min_cell * 0.25)
            .collect();
        if in_gap.len() < 2 {
            continue;
        }
        in_gap.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let span = in_gap.last().copied().unwrap_or(0.0) - in_gap[0];
        if span >= gap_w * 0.5 {
            under_ruled += 1;
        }
    }
    if !strong_partial {
        let surplus = col_xs.len().saturating_sub(v_cols);
        let need = if surplus >= 2 && v_cols <= 3 {
            1u32
        } else {
            (v_cols as u32).div_ceil(2).max(1)
        };
        if under_ruled < need {
            return (xs.to_vec(), Vec::new());
        }
    }

    let mut out = vec![xs_sorted[0]];
    let mut synthetic = Vec::new();
    for w in xs_sorted.windows(2) {
        let g0 = w[0];
        let g1 = w[1];
        let gap_w = (g1 - g0).abs();
        let mut in_gap: Vec<f32> = col_xs
            .iter()
            .copied()
            .filter(|&c| c > g0 + min_cell * 0.25 && c < g1 - min_cell * 0.25)
            .collect();
        in_gap.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let span_ok =
            in_gap.len() >= 2 && (in_gap.last().copied().unwrap_or(0.0) - in_gap[0]) >= gap_w * 0.5;
        if span_ok {
            for i in 1..in_gap.len() {
                let pitch = in_gap[i] - in_gap[i - 1];
                let inset = (pitch * 0.12).clamp(min_cell * 0.15, min_cell.max(1.0));
                let sep = in_gap[i] - inset;
                let prev = *out.last().unwrap();
                if sep > prev + min_cell && g1 - sep >= min_cell {
                    out.push(sep);
                    synthetic.push(sep);
                }
            }
        }
        out.push(g1);
    }
    let mut collapsed = collapse_thin_gaps(&out, min_cell);
    collapsed.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let synthetic: Vec<f32> = collapsed
        .iter()
        .copied()
        .filter(|&x| !xs_sorted.iter().any(|&vx| (vx - x).abs() <= min_cell * 0.5))
        .collect();
    if collapsed.len() <= xs_sorted.len() {
        return (xs.to_vec(), Vec::new());
    }
    (collapsed, synthetic)
}

/// True when token is a pure number (digits + optional commas/sign/percent/parens).
fn is_numberish_token(s: &str) -> bool {
    let t = s
        .trim()
        .trim_start_matches('(')
        .trim_end_matches(')')
        .trim_end_matches('%');
    if t.is_empty() {
        return false;
    }
    let mut dig = false;
    for c in t.chars() {
        if c.is_ascii_digit() {
            dig = true;
            continue;
        }
        if matches!(c, ',' | '.' | '-' | '+' | '–' | '—') {
            continue;
        }
        return false;
    }
    dig
}

/// Left-edge fractions [0,1) of complete numbers inside a text run.
///
/// Merges spaced decimals common in census TJ dumps (`11,062 .6` → one number)
/// and skips leader-dot tokens (`. . . .`). Used for sparse-V multi-number densify.
fn multi_number_left_fracs(text: &str) -> Vec<f32> {
    let chars: Vec<char> = text.chars().collect();
    let n = chars.len();
    if n == 0 {
        return Vec::new();
    }
    // Whitespace-separated tokens as (start, end exclusive, string).
    let mut tokens: Vec<(usize, usize, String)> = Vec::new();
    let mut i = 0usize;
    while i < n {
        while i < n && chars[i].is_whitespace() {
            i += 1;
        }
        if i >= n {
            break;
        }
        let start = i;
        while i < n && !chars[i].is_whitespace() {
            i += 1;
        }
        let s: String = chars[start..i].iter().collect();
        tokens.push((start, i, s));
    }
    let mut fracs = Vec::new();
    let mut ti = 0usize;
    while ti < tokens.len() {
        let (start, _end, ref s) = tokens[ti];
        // Leader dots / ellipsis gutters — not numbers.
        if s.chars().all(|c| matches!(c, '.' | '·' | '…' | '•')) {
            ti += 1;
            continue;
        }
        if !is_numberish_token(s) {
            ti += 1;
            continue;
        }
        // Merge spaced decimal: "11,062" + ".6"  or  "11,062" + "." + "6"
        let has_frac = s.contains('.')
            && s.split_once('.')
                .map(|(_, r)| r.chars().any(|c| c.is_ascii_digit()))
                .unwrap_or(false);
        let mut t = ti;
        if !has_frac && t + 1 < tokens.len() {
            let n1 = tokens[t + 1].2.as_str();
            if n1.starts_with('.') && n1.chars().any(|c| c.is_ascii_digit()) {
                t += 1;
            } else if (n1 == "." || n1 == "·")
                && t + 2 < tokens.len()
                && tokens[t + 2].2.chars().all(|c| c.is_ascii_digit())
            {
                t += 2;
            }
        }
        fracs.push(start as f32 / n as f32);
        ti = t + 1;
    }
    fracs
}

/// Median absolute neighbor gap of top-to-bottom centers (0 if <2 gaps).
fn median_band_pitch(centers_ttb: &[f32]) -> f32 {
    if centers_ttb.len() < 2 {
        return 0.0;
    }
    let mut gaps: Vec<f32> = centers_ttb
        .windows(2)
        .map(|w| (w[0] - w[1]).abs())
        .filter(|&g| g > 0.5)
        .collect();
    if gaps.is_empty() {
        return 0.0;
    }
    gaps.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    gaps[gaps.len() / 2]
}

/// True when consecutive band centers have stable vertical pitch (table-like).
///
/// Geometric rule: median neighbor gap ≥ min_cell/2, and coefficient of
/// variation of gaps ≤ `pitch_cv_max` (loose enough for header+body pitch
/// change, tight enough to reject irregular prose).
fn text_bands_regular_pitch(centers_ttb: &[f32], min_cell: f32, pitch_cv_max: f32) -> bool {
    if centers_ttb.len() < 4 {
        return false;
    }
    let mut gaps: Vec<f32> = centers_ttb
        .windows(2)
        .map(|w| (w[0] - w[1]).abs())
        .filter(|&g| g > 0.5)
        .collect();
    if gaps.len() < 3 {
        return false;
    }
    gaps.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let med = gaps[gaps.len() / 2];
    if med < (min_cell * 0.5).max(2.0) {
        return false;
    }
    let mean = gaps.iter().sum::<f32>() / gaps.len() as f32;
    if mean < 1e-3 {
        return false;
    }
    let var = gaps.iter().map(|g| (g - mean) * (g - mean)).sum::<f32>() / gaps.len() as f32;
    let cv = (var.sqrt()) / mean;
    cv <= pitch_cv_max
}

/// When H rules are sparse relative to multi-column text bands, insert Y
/// separators at midpoints between consecutive text bands *inside* each H gap.
///
/// Returns densified top-to-bottom Y anchors and the Y coordinates of any
/// newly inserted (synthetic) separators. Full grids (one multi-col band per
/// H gap) are unchanged.
pub(crate) fn densify_y_from_text_bands(
    y_ttb: &[f32],
    runs: &[TextRun],
    frame_x0: f32,
    frame_x1: f32,
    min_cell: f32,
    params: &DensifyParams,
) -> (Vec<f32>, Vec<f32>) {
    if y_ttb.len() < 2 {
        return (y_ttb.to_vec(), Vec::new());
    }
    let y_top = y_ttb.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    let y_bot = y_ttb.iter().copied().fold(f32::INFINITY, f32::min);
    if !y_top.is_finite() || !y_bot.is_finite() || y_top - y_bot < min_cell * 2.0 {
        return (y_ttb.to_vec(), Vec::new());
    }

    // Text strictly inside the ruled frame (pad slightly for centers on edges).
    let pad = 1.0f32;
    let inside: Vec<&TextRun> = runs
        .iter()
        .filter(|r| {
            if r.text.trim().is_empty() {
                return false;
            }
            let cx = (r.bbox.x0 + r.bbox.x1) * 0.5;
            let cy = r.bbox.y_center();
            cx >= frame_x0 - pad && cx <= frame_x1 + pad && cy <= y_top + pad && cy >= y_bot - pad
        })
        .collect();
    if inside.len() < 4 {
        return (y_ttb.to_vec(), Vec::new());
    }

    let fs = {
        let mut v: Vec<f32> = inside
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
    let y_tol = (0.45 * fs).max(2.0);

    // Band by Y without requiring a contiguous TextRun slice.
    let mut items = inside;
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
    let mut bands: Vec<Vec<&TextRun>> = Vec::new();
    for r in items {
        if let Some(band) = bands.last_mut() {
            let by = band.iter().map(|x| x.bbox.y_center()).sum::<f32>() / band.len() as f32;
            if (r.bbox.y_center() - by).abs() <= y_tol {
                band.push(r);
                continue;
            }
        }
        bands.push(vec![r]);
    }

    // Band centers: multi-col bands gate densify (proves table structure).
    // Single-run bands are *also* used for subdivision once gated — sparse
    // fill often leaves key-column-only rows (1 cell) that still mark a real
    // body line. Pure single-column prose never passes the multi-col gate.
    let y_lo_int = y_bot + min_cell * 0.25;
    let y_hi_int = y_top - min_cell * 0.25;
    let frame_w = (frame_x1 - frame_x0).abs().max(1.0);
    let mut multi_centers: Vec<f32> = Vec::new();
    let mut all_centers: Vec<f32> = Vec::new();
    for b in &bands {
        if b.is_empty() {
            continue;
        }
        let c = b.iter().map(|r| r.bbox.y_center()).sum::<f32>() / b.len() as f32;
        if c >= y_hi_int || c <= y_lo_int {
            continue;
        }
        all_centers.push(c);
        if b.len() >= 2 {
            // Phase-3: only count *wide* multi-run bands as body rows.
            // Wrapped words in one column (disease names) form multi-run
            // bands with small x-span and must not densify rows.
            let bx0 = b.iter().map(|r| r.bbox.x0).fold(f32::INFINITY, f32::min);
            let bx1 = b
                .iter()
                .map(|r| r.bbox.x1)
                .fold(f32::NEG_INFINITY, f32::max);
            if (bx1 - bx0) >= frame_w * 0.28 {
                multi_centers.push(c);
            }
        }
    }
    multi_centers = cluster_coords(&multi_centers, y_tol * 0.6);
    multi_centers.sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));
    all_centers = cluster_coords(&all_centers, y_tol * 0.6);
    all_centers.sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));

    let h_rows = y_ttb.len().saturating_sub(1);

    // Gate A — multi-col text *clearly* outnumbers H-derived rows (partial H).
    // Require ~2× H rows so rowspan / multi-line headers (modest band excess)
    // do not invent phantom rows. Classic sparse-H health tables have
    // multi_centers ≫ h_rows (e.g. 50 vs 4).
    //
    // Phase-3 pitch split:
    // - multi_pitch (multi-run bands only): true body rows for census/BEA/NICS
    // - wrap-heavy disease cells: multi_pitch ~1em → block densify; keep sparse H
    let multi_pitch = median_band_pitch(&multi_centers);
    let all_pitch = median_band_pitch(&all_centers);
    // Multi-col body pitch is the authority when enough samples exist.
    let body_pitch = if multi_centers.len() >= 5 {
        multi_pitch
    } else {
        multi_pitch.max(all_pitch)
    };
    // Table-scale: dense statistical bodies can sit near ~1.0–1.1em pitch
    // (census fs=6, pitch=6.5). Wrap lines in multi-line cells are typically
    // < ~1.0em of the dominant body font. Use ~1.02em as the cut.
    let body_scale_ok = body_pitch >= (fs * 1.02).max(min_cell * 0.6);
    let multi_gate = multi_centers.len() >= 4
        && (multi_centers.len() as f32) >= (h_rows as f32) * 2.0 + 2.0
        && body_scale_ok;
    // Gate B — regular-pitch body with substantial multi-col evidence.
    // Rejects pure prose (1 multi-col header + single-col stack: multi/all ≪ ¼).
    let regular_pitch = text_bands_regular_pitch(&all_centers, min_cell, params.pitch_cv_max);
    let multi_share_ok = !all_centers.is_empty()
        && (multi_centers.len() as f32) >= (all_centers.len() as f32) * 0.25
        && multi_centers.len() >= 2;
    // all_gate: only when multi body pitch is table-scale (not wrap-dominated).
    let all_gate = multi_share_ok
        && all_centers.len() >= 4
        && (all_centers.len() as f32) >= (h_rows as f32) * 2.0 + 2.0
        && regular_pitch
        && body_scale_ok
        && multi_pitch >= (fs * 1.05).max(min_cell * 0.65);
    // Gate C — outer-frame / few-H statistical grids: many body bands, table pitch,
    // even if multi/all share is modest (sparse fill). Disease wrap fails pitch.
    let sparse_h_gate = h_rows <= 5
        && all_centers.len() >= (h_rows * 5).max(12)
        && body_scale_ok
        && (multi_centers.len() >= 3 || multi_share_ok);
    // Gate D — small ruled tables (2–3 H rows) missing a few body bands.
    // multi_centers clearly exceeds H but not 2× (so multi_gate fails). Cap
    // growth later so we only recover +2..+4 rows (age-cohort class grids).
    let small_under_gate = (2..=3).contains(&h_rows)
        && multi_centers.len() >= h_rows + 2
        && multi_centers.len() <= h_rows + 4
        && multi_centers.len() >= 4
        && body_scale_ok
        && multi_share_ok
        && multi_pitch >= (fs * 1.1).max(min_cell * 0.75);

    if !multi_gate && !all_gate && !sparse_h_gate && !small_under_gate {
        return (y_ttb.to_vec(), Vec::new());
    }
    let small_under_only = small_under_gate && !multi_gate && !all_gate && !sparse_h_gate;
    let multi_n = multi_centers.len();

    // Band source:
    // - Prefer multi when it is dense enough (avoids wrap fragments).
    // - Use all_centers for sparse-H / dense numeric when multi under-samples
    //   single-cell body rows (census/NICS) AND multi_pitch is table-scale
    //   (blocks disease wrap where multi_pitch ~1em).
    let band_centers = if sparse_h_gate || all_gate {
        if multi_pitch >= (fs * 1.05).max(min_cell * 0.65) && !all_centers.is_empty() {
            all_centers
        } else if multi_centers.len() >= 4 {
            multi_centers
        } else {
            all_centers
        }
    } else if multi_centers.len() >= 4 {
        multi_centers
    } else {
        all_centers
    };

    let mut out: Vec<f32> = Vec::with_capacity(band_centers.len() + y_ttb.len());
    let mut synthetic: Vec<f32> = Vec::new();
    out.push(y_ttb[0]);

    for w in y_ttb.windows(2) {
        let gap_top = w[0];
        let gap_bot = w[1];
        if gap_top <= gap_bot {
            out.push(gap_bot);
            continue;
        }
        // Text band centers strictly inside this H gap.
        let mut in_gap: Vec<f32> = band_centers
            .iter()
            .copied()
            .filter(|&c| c < gap_top - 0.5 && c > gap_bot + 0.5)
            .collect();
        in_gap.sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));

        // Need ≥3 bands inside a gap before inventing separators. Two bands
        // (e.g. multi-line header wrapped once) is normal in a full-H grid and
        // must not create phantom rows. Sparse body H leaves many bands/gap.
        // Require table-scale pitch between bands (wrapped lines are ~1em).
        let min_in_gap = 3;
        if in_gap.len() >= min_in_gap {
            // Separators only between table-scale bands; wrap lines (~≤1em) skip.
            let min_band_pitch = (min_cell.max(3.0)).max(fs * 1.02);
            for pair in in_gap.windows(2) {
                let band_gap = (pair[0] - pair[1]).abs();
                if band_gap < min_band_pitch {
                    // Wrapped lines of the same logical row — no separator.
                    continue;
                }
                let mid = (pair[0] + pair[1]) * 0.5;
                let prev = *out.last().unwrap();
                // Keep clear of existing anchors and the gap floor.
                if (prev - mid).abs() >= min_cell && (mid - gap_bot).abs() >= min_cell {
                    // Avoid near-duplicates of real H lines already in out / next.
                    if (mid - gap_top).abs() >= min_cell {
                        out.push(mid);
                        synthetic.push(mid);
                    }
                }
            }
        }
        out.push(gap_bot);
    }

    // Collapse any accidental thin pairs; re-sort top-to-bottom.
    let mut collapsed = collapse_thin_gaps(&out, min_cell);
    collapsed.sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));

    // Recompute synthetic as densified anchors not near original H lines.
    let synthetic: Vec<f32> = collapsed
        .iter()
        .copied()
        .filter(|&y| !y_ttb.iter().any(|&hy| (hy - y).abs() <= min_cell * 0.5))
        .collect();

    if collapsed.len() <= y_ttb.len() {
        return (y_ttb.to_vec(), Vec::new());
    }
    // Phase-3 safety: refuse densify that shreds multi-line cells inside a
    // few tall ruled rows (disease outbreak grids). Signal: huge row growth
    // with body pitch still near 1em (wrap), not a sparse-H statistical body
    // where pitch is still table-scale relative to font.
    let dens_rows = collapsed.len().saturating_sub(1);
    if dens_rows > h_rows.saturating_mul(3).max(h_rows + 6)
        && body_pitch > 0.0
        && body_pitch < fs * 1.08
        && h_rows <= 6
    {
        return (y_ttb.to_vec(), Vec::new());
    }
    // Small-under only recovers a few missing body rows — never explode.
    if small_under_only && dens_rows > h_rows + 4 {
        return (y_ttb.to_vec(), Vec::new());
    }
    if small_under_only && dens_rows > multi_n.max(h_rows + 2) {
        return (y_ttb.to_vec(), Vec::new());
    }
    (collapsed, synthetic)
}

/// Merge consecutive coordinates whose gap is below min_cell (keep outer of each pair).
pub(crate) fn collapse_thin_gaps(coords: &[f32], min_cell: f32) -> Vec<f32> {
    if coords.is_empty() {
        return Vec::new();
    }
    let mut v = coords.to_vec();
    v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let mut out = vec![v[0]];
    for &c in &v[1..] {
        let prev = *out.last().unwrap();
        if (c - prev).abs() < min_cell {
            // Absorb into previous (keep first); for outer max use last of cluster later
            // Prefer keeping both endpoints of the grid: skip interior thin lines.
            continue;
        }
        out.push(c);
    }
    // Ensure we kept the last coordinate if it was skipped as thin mid-pair
    let last = *v.last().unwrap();
    if out.last().map(|x| (*x - last).abs() > 1e-3).unwrap_or(true) {
        // If last is close to final out, replace; else push
        if let Some(p) = out.last_mut() {
            if (last - *p).abs() < min_cell {
                *p = last;
            } else {
                out.push(last);
            }
        }
    }
    out
}

/// Drop interior columns that are almost entirely empty (densify / exterior-stub
/// artifacts). Keeps first and last column always; requires ≥4 columns.
///
/// Never drops a column that participates in a multi-col span (structure safety).
pub(crate) fn collapse_sparse_interior_columns(
    cells: Vec<TableCell>,
    nrows: u32,
    ncols: u32,
) -> (Vec<TableCell>, u32, u32) {
    if ncols < 4 || nrows < 2 || cells.is_empty() {
        return (cells, nrows, ncols);
    }
    let nc = ncols as usize;
    let mut keep = vec![true; nc];
    for c in 1..nc.saturating_sub(1) {
        let filled = cells
            .iter()
            .filter(|cell| cell.col == c as u32 && !cell.text.trim().is_empty())
            .count();
        // Only drop *completely* empty interior columns (densify gutters).
        // Sparse but real data columns must stay.
        if filled == 0 {
            keep[c] = false;
        }
    }
    if keep.iter().all(|&k| k) {
        return (cells, nrows, ncols);
    }
    // Map old col → new col
    let mut map = vec![None; nc];
    let mut new_c = 0u32;
    for (c, &k) in keep.iter().enumerate() {
        if k {
            map[c] = Some(new_c);
            new_c += 1;
        }
    }
    if new_c < 2 {
        return (cells, nrows, ncols);
    }
    let mut out = Vec::with_capacity(cells.len());
    for mut cell in cells {
        let oc = cell.col as usize;
        if oc >= nc || !keep[oc] {
            continue;
        }
        // Shrink colspan to surviving columns in the original span range.
        // Do **not** keep empty densify gutters just because a header span
        // crossed them (would reintroduce densify gutters).
        if cell.colspan > 1 {
            let end = (oc + cell.colspan as usize).min(nc);
            let kept_span = (oc..end).filter(|&i| keep[i]).count() as u32;
            cell.colspan = kept_span.max(1);
        }
        cell.col = map[oc].unwrap_or(0);
        out.push(cell);
    }
    (out, nrows, new_c)
}

#[cfg(test)]
mod densify_y_debug {
    use super::*;
    use pdfparser_ir::{Rect, TextRun};

    fn run(text: &str, x0: f32, y0: f32, x1: f32, y1: f32, fs: f32) -> TextRun {
        TextRun {
            text: text.into(),
            bbox: Rect { x0, y0, x1, y1 },
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

    #[test]
    fn multi_number_merges_spaced_decimals() {
        let fr = multi_number_left_fracs("11,062 .6 1,540 .0 9,522 .6");
        assert_eq!(fr.len(), 3, "spaced decimals must merge: {fr:?}");
        // First number at start
        assert!(fr[0] < 0.05, "first at left: {fr:?}");
        // Three distinct anchors
        assert!(fr[1] > fr[0] + 0.1 && fr[2] > fr[1] + 0.1, "{fr:?}");
    }

    #[test]
    fn multi_number_skips_leader_dots() {
        let fr = multi_number_left_fracs("Violent crime  . . . . . . . 467 .9 69 .1");
        assert_eq!(fr.len(), 2, "leaders skipped, two body nums: {fr:?}");
    }

    #[test]
    fn densify_y_sparse_h_many_body_rows() {
        // 3 H lines (2 gaps) + 12 multi-col body bands at table pitch.
        let y_ttb = vec![400.0_f32, 200.0, 0.0]; // top to bottom
        let mut runs = Vec::new();
        let fs = 8.0;
        for i in 0..12 {
            let y = 380.0 - i as f32 * 14.0; // pitch 14 > 1.15*8
            runs.push(run("a", 10.0, y, 30.0, y + 8.0, fs));
            runs.push(run("b", 50.0, y, 70.0, y + 8.0, fs));
            runs.push(run("1", 90.0, y, 110.0, y + 8.0, fs));
        }
        let (out, synth) =
            densify_y_from_text_bands(&y_ttb, &runs, 0.0, 120.0, 6.0, &DensifyParams::default());
        assert!(
            out.len() > y_ttb.len(),
            "should densify sparse H: ys={} synth={}",
            out.len(),
            synth.len()
        );
    }

    #[test]
    fn densify_y_rejects_wrap_line_pitch() {
        // sparse H + many wrap bands at ~1em pitch must NOT densify.
        let y_ttb = vec![400.0_f32, 200.0, 0.0];
        let mut runs = Vec::new();
        let fs = 10.0;
        for i in 0..20 {
            let y = 380.0 - i as f32 * 9.0; // pitch 9 < 1.15*10
            runs.push(run("word", 10.0, y, 80.0, y + 8.0, fs));
            runs.push(run("more", 90.0, y, 160.0, y + 8.0, fs));
        }
        let (out, synth) =
            densify_y_from_text_bands(&y_ttb, &runs, 0.0, 200.0, 6.0, &DensifyParams::default());
        assert_eq!(
            out.len(),
            y_ttb.len(),
            "wrap pitch must not densify synth={}",
            synth.len()
        );
    }
}
