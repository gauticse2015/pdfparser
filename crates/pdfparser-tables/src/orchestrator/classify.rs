//! Page-flavor classification: strong/solid lattice, chrome FP, borderless precision.
use super::geom_util::containment_ratio;
use crate::constants::{STRONG_LATTICE_2COL_MIN_ROWS, STRONG_LATTICE_2COL_MIN_WIDTH};
use crate::geom;
use crate::lexicon::{
    cell_blob, is_irs_header_blob, phrase_hits, NOTICE_METADATA_PHRASES, TAX_FORM_PHRASES,
};
use crate::options::TableOptions;
use crate::stats::CellStats;
use crate::types::{Table, TableMethod};

pub(crate) fn is_strong_lattice(t: &Table, opts: &TableOptions) -> bool {
    if t.method != TableMethod::Lattice
        || t.rows < 2
        || t.confidence < opts.advanced.strong_lattice_min_conf
        || t.weak_edges
    {
        return false;
    }
    if t.cols >= 3 {
        return true;
    }
    // 2-col lattices: strong only if wide enough to be a real side-by-side
    // table, not a partial corner fragment (disease table left strip ~100u).
    // Side-by-side stress fixture tables are ~150–170u wide.
    t.cols == 2
        && t.bbox.width() >= STRONG_LATTICE_2COL_MIN_WIDTH
        && t.rows >= STRONG_LATTICE_2COL_MIN_ROWS
}

/// Solid ruled table for page ownership (slightly looser than strong_lattice).
///
/// Phase-1 V3: if any such table exists, borderless detectors stay off.
pub(crate) fn is_solid_ruled_table(t: &Table, opts: &TableOptions) -> bool {
    if t.method != TableMethod::Lattice || t.weak_edges {
        return false;
    }
    if t.rows < 2 || t.cols < 2 {
        return false;
    }
    if t.confidence < opts.min_table_confidence.min(0.55) {
        return false;
    }
    // Reject near-empty chrome frames (page borders / form rules).
    let empty_frac = lattice_empty_frac(t);
    if empty_frac >= 0.90 {
        return false;
    }
    // Tall few-col lattices grown mostly by text_row densify (NIPA page-rule soup
    // → 47×3) must NOT own the page and kill multi-col stream recovery (gold ~50×22).
    // Thresholds from tuning so document-type overrides stay consistent.
    let heavy_y_densify = t.text_row_recovery;
    if t.cols <= opts.advanced.tuning.solid_lattice_stream_safe_max_cols
        && t.rows >= opts.advanced.tuning.solid_lattice_stream_safe_min_rows
        && heavy_y_densify
    {
        return false;
    }
    t.cols >= 2 && t.rows >= 2
}

pub(crate) fn lattice_empty_frac(t: &Table) -> f32 {
    let n = t.rows.saturating_mul(t.cols).max(1) as f32;
    if t.cells.is_empty() {
        return 0.5;
    }
    let empty = t.cells.iter().filter(|c| c.text.trim().is_empty()).count() as f32;
    // Prefer schema size when cell list matches grid; else use list length.
    let denom = if t.cells.len() as u32 >= t.rows.saturating_mul(t.cols) {
        n
    } else {
        t.cells.len().max(1) as f32
    };
    empty / denom
}

/// Borderless/stream gates.
///
/// Merge vertically stacked Lattice tables with equal column counts when the
/// gap is modest and X-overlap is high. Targets multi-region CC shred that
pub(crate) fn borderless_passes_precision(
    t: &Table,
    opts: &TableOptions,
    recall_mode: bool,
) -> bool {
    if t.rows < 3 || t.cols < 2 {
        return false;
    }
    // Camelot-class: need enough cells to be a real table (≥6 filled-ish).
    let filled = t.cells.iter().filter(|c| !c.text.trim().is_empty()).count();
    if filled < 6 && t.rows.saturating_mul(t.cols) < 12 {
        return false;
    }
    let mean = stream_mean_cell_chars(t);
    let num = stream_numeric_density(t);
    let structure_ok = t.cols >= 3 && t.rows >= 4 && filled >= 8;

    // Notice / form metadata grids (NIST withdrawn style) — always reject.
    if looks_like_notice_metadata(t) {
        return false;
    }
    // Tax form field snippets — always reject.
    if looks_like_tax_form_fields(t) {
        return false;
    }

    if recall_mode {
        let dense_numeric = num >= 0.30 && t.cols >= 3 && t.rows >= 4;
        // High-fill multi-col grids (campaign donors 56×7, liabilities ~30×10)
        // are real borderless data tables — not form worksheets.
        let dense_data_grid = t.cols >= 4
            && t.rows >= 6
            && filled >= 24
            && (t.fill_rate >= 0.55
                || filled as f32 / (t.rows.saturating_mul(t.cols).max(1) as f32) >= 0.55)
            && mean < 48.0;
        // Soft conf floor for structured / dense-numeric grids.
        let conf_floor = if dense_numeric || dense_data_grid {
            0.48
        } else if structure_ok {
            (opts.advanced.min_confidence_stream * 0.85).min(0.55)
        } else {
            opts.advanced.min_confidence_stream
        };
        if t.confidence < conf_floor {
            return false;
        }
        // Cap giant *form-like* networks (IRS worksheets): large + sparse/low-fill
        // OR giant with weak numeric and long label cells. Dense high-fill grids pass.
        if t.rows >= 20 && t.cols >= 6 && !dense_data_grid && !dense_numeric {
            return false;
        }
        if t.rows.saturating_mul(t.cols) >= 200 && !dense_data_grid && !dense_numeric {
            // Still allow medium-fill multi-col with strong conf (network class).
            if !(t.cols >= 5 && t.fill_rate >= 0.45 && t.confidence >= 0.75 && mean < 40.0) {
                return false;
            }
        }
        // IRS header strip (OMB / Department of the Treasury)
        if looks_like_irs_header_strip(t) {
            return false;
        }
        // Prose reject: still drop paragraph bands, but allow medium mean when
        // multi-col + some numeric OR large grid.
        if t.cols <= 2 && mean >= opts.advanced.stream_max_prose_mean_chars * 0.50 && num < 0.20 {
            return false;
        }
        if mean >= opts.advanced.stream_max_prose_mean_chars * 0.90
            && num < 0.12
            && !structure_ok
            && !dense_numeric
            && !dense_data_grid
        {
            return false;
        }
        // Weak 2-col alpha lists
        if t.cols == 2 && num < 0.12 && mean > 14.0 {
            return false;
        }
        return true;
    }

    // Precision mode (default / under ruled co-existence checks)
    if mean >= opts.advanced.stream_max_prose_mean_chars * 0.55 && num < 0.25 {
        return false;
    }
    if mean >= 28.0 && num < 0.15 {
        return false;
    }
    if t.confidence < opts.advanced.min_confidence_stream {
        return false;
    }
    if t.cols == 2 && num < 0.12 && mean > 12.0 {
        return false;
    }
    true
}

pub(crate) fn looks_like_tax_form_fields(t: &Table) -> bool {
    let blob = cell_blob(t.cells.iter().map(|c| c.text.as_str()), usize::MAX);
    let tax_hits = phrase_hits(&blob, TAX_FORM_PHRASES);
    let num = stream_numeric_density(t);
    (tax_hits >= 1 && num < 0.45) || (tax_hits >= 2 && num < 0.55)
}

pub(crate) fn looks_like_irs_header_strip(t: &Table) -> bool {
    let blob = cell_blob(t.cells.iter().map(|c| c.text.as_str()), 12);
    is_irs_header_blob(&blob) && t.rows <= 8
}

/// Phase-2: strong multi-col numeric stream that coexists with lattice (census).
pub(crate) fn is_multitable_stream_recovery(s: &Table, lattices: &[pdfparser_ir::Rect]) -> bool {
    if s.cols < 4 || s.rows < 6 {
        return false;
    }
    if stream_numeric_density(s) < 0.28 {
        return false;
    }
    if s.confidence < 0.65 {
        return false;
    }
    // Require meaningful size (avoid thin FP bands next to lattices).
    if s.bbox.width() < 120.0 || s.bbox.height() < 80.0 {
        return false;
    }
    // Must not substantially overlap any lattice.
    for &lat in lattices {
        if geom::iou(s.bbox, lat) >= 0.18 {
            return false;
        }
        if containment_ratio(s.bbox, lat) >= 0.30 {
            return false;
        }
        if containment_ratio(lat, s.bbox) >= 0.40 {
            return false;
        }
        // Same vertical band / stacked: require clear y-separation or x-separation
        let y_overlap = (s.bbox.y1.min(lat.y1) - s.bbox.y0.max(lat.y0)).max(0.0);
        let x_overlap = (s.bbox.x1.min(lat.x1) - s.bbox.x0.max(lat.x0)).max(0.0);
        if y_overlap > 0.5 * s.bbox.height().min(lat.height())
            && x_overlap > 0.5 * s.bbox.width().min(lat.width())
        {
            return false;
        }
    }
    true
}

pub(crate) fn looks_like_notice_metadata(t: &Table) -> bool {
    let mut hits = 0u32;
    for c in t.cells.iter().take(24) {
        let s = c.text.to_ascii_lowercase();
        if NOTICE_METADATA_PHRASES.iter().any(|p| s.contains(p))
            || s.starts_with("1.")
            || s.starts_with("2.")
        {
            hits += 1;
        }
    }
    hits >= 2 && stream_numeric_density(t) < 0.25
}

pub(crate) fn stream_mean_cell_chars(t: &Table) -> f32 {
    CellStats::from_table(t).mean_chars
}

/// Page-border / form-box lattices that are not data tables (Phase-1).
///
/// Conservative: only drop near-empty chrome. Do **not** drop filled label
/// lattices (outer nested forms) or multi-col report tables.
pub(crate) fn is_chrome_lattice_fp(t: &Table) -> bool {
    if t.method != TableMethod::Lattice {
        return false;
    }
    // Image-painted grids legitimately have zero extractable text (ink is in
    // the bitmap). Never treat raster lattices as empty chrome frames.
    if t.is_from_raster() {
        return false;
    }
    let empty = lattice_empty_frac(t);
    let num = stream_numeric_density(t);
    let mean = stream_mean_cell_chars(t);
    let filled = t.cells.iter().filter(|c| !c.text.trim().is_empty()).count();
    // Near-empty ruled frame (page border / empty checkbox grid)
    if empty >= 0.88 && num < 0.12 && filled <= 6 {
        return true;
    }
    // Tiny empty-ish box
    if t.rows <= 3 && t.cols <= 3 && filled <= 2 && num < 0.15 {
        return true;
    }
    // 2-col prose notice (NIST withdrawn) — long alpha cells, no numbers
    if t.cols == 2 && mean >= 28.0 && num < 0.12 && t.rows >= 8 {
        return true;
    }
    // Sparse form worksheet: high empty + weak edges (IRS Schedule C style).
    // Line numbers inflate numeric_density — also check dotted leaders.
    if empty >= 0.55 && t.cols >= 6 && (t.weak_edges || t.edge_score < 0.55) {
        return true;
    }
    // Dotted leader forms (". . .") with weak lattice edges
    let dotted = t
        .cells
        .iter()
        .filter(|c| c.text.contains(". .") || c.text.matches('.').count() >= 3)
        .count();
    if dotted >= 4 && t.weak_edges && empty >= 0.40 {
        return true;
    }
    // Tax form field snippets (SSN / EIN / "enter code" / Schedule D capital gains)
    let blob = t
        .cells
        .iter()
        .map(|c| c.text.to_ascii_lowercase())
        .collect::<Vec<_>>()
        .join(" ");
    if (blob.contains("social security")
        || blob.contains("employer id")
        || blob.contains("enter code from instructions")
        || blob.contains("(ssn)")
        || blob.contains("(ein)")
        || blob.contains("proceeds (sales price)")
        || blob.contains("cost (or other basis)")
        || blob.contains("adjustments to gain or loss")
        || blob.contains("schedule d")
        || blob.contains("capital gain or (loss)"))
        && num < 0.40
    {
        return true;
    }
    false
}

/// Digit-vs-alpha heuristic (looser than [`crate::stats::is_numeric_token`]):
/// mixed labels like "GDP 5.8" still count as numeric cells for ownership.
pub(crate) fn stream_numeric_density(t: &Table) -> f32 {
    CellStats::loose_numeric_density(t)
}
