//! PDF table extraction: lattice (ruled) + hybrid (partial) + network (borderless).
//!
//! Production path (Auto/Full):
//! 1. Lattice on vector rules (incl. thin-fill painted rules)
//! 2. Hybrid only outside strong lattice
//! 3. Network borderless only outside strong lattice (no dual soup)
#![deny(missing_docs)]

mod form;
mod geom;
mod hybrid;
pub mod builders;
mod lattice;
mod network;
mod options;
mod raster;
mod split;
mod stitch;
mod stream;
mod types;

pub use form::scrub_document_table_fps;
pub use lattice::detect_lattice_tables;
pub use network::detect_network_tables;
pub use options::{TableModeSet, TableOptions, TablePreset};
pub use raster::{
    config_for_raster_page, gray_from_rgb, gray_from_rgba, merge_rules, rules_from_raster,
    RasterConfig, RasterPage, RasterRule,
};
pub use stitch::{materialize_stitched, stitch_document};
pub use stream::detect_stream_tables;
pub use types::{PipelineId, Table, TableCell, TableMethod};

use form::apply_form_discriminator;
use hybrid::detect_hybrid_tables;
use pdfparser_content::RuleSegment;
use pdfparser_ir::TextRun;
use split::split_side_by_side;

/// Whether the table engine is available.
pub fn tables_available() -> bool {
    true
}

/// Detect tables on a single page from text runs + rule segments.
pub fn detect_tables_page(
    page_index: u32,
    runs: &[TextRun],
    rules: &[RuleSegment],
    opts: &TableOptions,
) -> Vec<Table> {
    detect_tables_page_with_raster(page_index, runs, rules, opts, &[])
}

/// Detect tables with optional raster page bitmaps (embedded images / renders).
///
/// When `opts.raster_line_detect` is true and `raster_pages` is non-empty, line
/// segments are recovered via morphology and merged into the lattice rule set.
pub fn detect_tables_page_with_raster(
    page_index: u32,
    runs: &[TextRun],
    rules: &[RuleSegment],
    opts: &TableOptions,
    raster_pages: &[RasterPage],
) -> Vec<Table> {
    if !opts.detect_tables {
        return Vec::new();
    }

    let mut cands = Vec::new();

    if opts.modes.lattice {
        cands.extend(detect_lattice_tables(
            page_index,
            runs,
            rules,
            opts,
            raster_pages,
        ));
    }

    let strong_lattice_bboxes: Vec<pdfparser_ir::Rect> = cands
        .iter()
        .filter(|t| is_strong_lattice(t, opts))
        .map(|t| t.bbox)
        .collect();
    let has_strong_lattice = !strong_lattice_bboxes.is_empty();

    if opts.modes.hybrid {
        let hybrid = detect_hybrid_tables(page_index, runs, rules, opts);
        if !has_strong_lattice {
            cands.extend(hybrid);
        } else {
            for h in hybrid {
                if !overlaps_any(h.bbox, &strong_lattice_bboxes) {
                    cands.push(h);
                }
            }
        }
    }

    if opts.modes.stream {
        // Production borderless path = network (textline + alignments), not classic stream soup.
        let borderless = detect_network_tables(page_index, runs, opts);
        for mut s in borderless {
            if opts.exclusive_under_strong_lattice && has_strong_lattice {
                if overlaps_any(s.bbox, &strong_lattice_bboxes) {
                    continue;
                }
            } else if has_strong_lattice && overlaps_any(s.bbox, &strong_lattice_bboxes) {
                s.confidence *= 0.50;
                s.notes.push("demoted_under_lattice".into());
            }
            // Weak 2-col alpha lists next to a real grid are stream FPs.
            if has_strong_lattice && s.cols == 2 && stream_numeric_density(&s) < 0.10 {
                s.confidence *= 0.40;
                s.notes.push("demoted_weak_2col".into());
            }
            cands.push(s);
        }
    }

    if opts.side_by_side_split {
        cands = split_side_by_side(cands, runs, opts);
    }
    if opts.form_discriminator {
        cands = apply_form_discriminator(cands, opts);
    }

    let min_conf = opts.min_confidence_stream.min(opts.min_table_confidence);
    let mut kept = nms(cands, min_conf, opts.nms_containment_frac);
    kept.retain(|t| match t.method {
        TableMethod::Stream => t.confidence >= opts.min_confidence_stream,
        _ => t.confidence >= opts.min_table_confidence,
    });
    kept.truncate(opts.max_tables_per_page as usize);
    kept
}

fn is_strong_lattice(t: &Table, opts: &TableOptions) -> bool {
    t.method == TableMethod::Lattice
        && t.cols >= 2
        && t.rows >= 2
        && t.confidence >= opts.strong_lattice_min_conf
        && !t.weak_edges
}

fn overlaps_any(bbox: pdfparser_ir::Rect, regions: &[pdfparser_ir::Rect]) -> bool {
    regions
        .iter()
        .any(|&kb| region_overlap(kb, bbox) >= 0.40 || geom::iou(kb, bbox) >= 0.35)
}

/// Detect tables for all pages; optional stitch and over-seg scrub.
///
/// This entry point has no raster bitmaps (runs + rules only). Image-line
/// sensing is a no-op here — use [`detect_tables_document_with_raster`] or the
/// `pdfparser` façade `document_tables` for embedded-image grids.
pub fn detect_tables_document(
    pages: &[(u32, &[TextRun], &[RuleSegment])],
    page_heights: &[f32],
    opts: &TableOptions,
) -> (Vec<Vec<Table>>, Vec<Table>) {
    let mut page_tables: Vec<Vec<Table>> = pages
        .iter()
        .map(|(idx, runs, rules)| detect_tables_page_with_raster(*idx, runs, rules, opts, &[]))
        .collect();

    if opts.stitch_multipage {
        stitch_document(&mut page_tables, page_heights, opts);
    }

    let mut logical = if opts.stitch_multipage {
        materialize_stitched(&page_tables)
    } else {
        page_tables.iter().flatten().cloned().collect()
    };
    if opts.form_discriminator {
        logical = scrub_document_table_fps(logical, opts);
    }
    (page_tables, logical)
}

/// Document-level detect with per-page raster bitmaps for line sensing.
pub fn detect_tables_document_with_raster(
    pages: &[(u32, &[TextRun], &[RuleSegment], &[RasterPage])],
    page_heights: &[f32],
    opts: &TableOptions,
) -> (Vec<Vec<Table>>, Vec<Table>) {
    let mut page_tables: Vec<Vec<Table>> = pages
        .iter()
        .map(|(idx, runs, rules, rasters)| {
            detect_tables_page_with_raster(*idx, runs, rules, opts, rasters)
        })
        .collect();

    if opts.stitch_multipage {
        stitch_document(&mut page_tables, page_heights, opts);
    }

    let mut logical = if opts.stitch_multipage {
        materialize_stitched(&page_tables)
    } else {
        page_tables.iter().flatten().cloned().collect()
    };
    if opts.form_discriminator {
        logical = scrub_document_table_fps(logical, opts);
    }
    (page_tables, logical)
}

fn nms(mut cands: Vec<Table>, min_conf: f32, containment_frac: f32) -> Vec<Table> {
    // Align with final retain: do not admit candidates below product min conf.
    cands.retain(|t| t.confidence >= min_conf);
    cands.sort_by(|a, b| {
        method_rank(b.method)
            .cmp(&method_rank(a.method))
            .then_with(|| {
                quality_score(b)
                    .partial_cmp(&quality_score(a))
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
    });
    let mut out: Vec<Table> = Vec::new();
    for c in cands {
        if out
            .iter()
            .any(|k| containment_ratio(c.bbox, k.bbox) >= containment_frac)
        {
            continue;
        }
        let c_rank = method_rank(c.method);
        out.retain(|k| {
            if method_rank(k.method) > c_rank {
                return true;
            }
            containment_ratio(k.bbox, c.bbox) < containment_frac
        });
        let overlaps = out.iter().any(|k| {
            let ov = region_overlap(k.bbox, c.bbox);
            ov >= 0.28 || geom::iou(k.bbox, c.bbox) >= 0.35
        });
        if !overlaps {
            out.push(c);
        }
    }
    out
}

fn containment_ratio(inner: pdfparser_ir::Rect, outer: pdfparser_ir::Rect) -> f32 {
    let x0 = inner.x0.max(outer.x0);
    let y0 = inner.y0.max(outer.y0);
    let x1 = inner.x1.min(outer.x1);
    let y1 = inner.y1.min(outer.y1);
    let w = (x1 - x0).max(0.0);
    let h = (y1 - y0).max(0.0);
    let inter = w * h;
    let area = (inner.width() * inner.height()).max(1.0);
    inter / area
}

fn quality_score(t: &Table) -> f32 {
    let edge = if t.edge_score > 0.0 { t.edge_score } else { 0.5 };
    let fill = if t.fill_rate > 0.0 { t.fill_rate } else { 0.5 };
    let weak_pen = if t.weak_edges { 0.85 } else { 1.0 };
    (0.55 * t.confidence + 0.25 * fill + 0.20 * edge) * weak_pen
}

fn region_overlap(a: pdfparser_ir::Rect, b: pdfparser_ir::Rect) -> f32 {
    let x0 = a.x0.max(b.x0);
    let y0 = a.y0.max(b.y0);
    let x1 = a.x1.min(b.x1);
    let y1 = a.y1.min(b.y1);
    let w = (x1 - x0).max(0.0);
    let h = (y1 - y0).max(0.0);
    let inter = w * h;
    if inter <= 0.0 {
        return 0.0;
    }
    let aa = (a.width() * a.height()).max(1.0);
    let ba = (b.width() * b.height()).max(1.0);
    inter / aa.min(ba)
}

fn method_rank(m: TableMethod) -> u8 {
    match m {
        TableMethod::Structure => 5,
        TableMethod::Lattice => 4,
        TableMethod::Hybrid => 3,
        TableMethod::Stream => 1,
        TableMethod::DenseNumeric => 2,
        _ => 0,
    }
}

fn stream_numeric_density(t: &Table) -> f32 {
    let mut ne = 0u32;
    let mut num = 0u32;
    for c in &t.cells {
        let s = c.text.trim();
        if s.is_empty() {
            continue;
        }
        ne += 1;
        let t = s.trim_matches(|ch: char| {
            ch == '$' || ch == '%' || ch == '(' || ch == ')' || ch == ','
        });
        if t.is_empty() {
            continue;
        }
        let digits = t.chars().filter(|ch| ch.is_ascii_digit()).count();
        let alpha = t.chars().filter(|ch| ch.is_alphabetic()).count();
        if digits >= 1 && digits >= alpha {
            num += 1;
        }
    }
    if ne == 0 {
        0.0
    } else {
        num as f32 / ne as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pdfparser_content::RuleSegment;
    use pdfparser_ir::{Matrix3x2, Rect, TextRun};

    fn tr(text: &str, x0: f32, y0: f32, x1: f32, y1: f32) -> TextRun {
        TextRun {
            text: text.into(),
            bbox: Rect { x0, y0, x1, y1 },
            transform: Matrix3x2::identity(),
            font_name: None,
            font_size: 10.0,
            mapping_confidence: 1.0,
            metrics_confidence: 1.0,
            mcid: None,
            invisible: false,
            from_actual_text: false,
        }
    }

    fn rule(x0: f32, y0: f32, x1: f32, y1: f32) -> RuleSegment {
        RuleSegment { x0, y0, x1, y1 }
    }

    fn lattice_grid(
        x0: f32,
        y0: f32,
        rows: u32,
        cols: u32,
        cell_w: f32,
        cell_h: f32,
    ) -> (Vec<TextRun>, Vec<RuleSegment>) {
        let mut runs = Vec::new();
        let mut rules = Vec::new();
        let x1 = x0 + cols as f32 * cell_w;
        let y1 = y0 + rows as f32 * cell_h;
        for r in 0..=rows {
            let y = y0 + r as f32 * cell_h;
            rules.push(rule(x0, y, x1, y));
        }
        for c in 0..=cols {
            let x = x0 + c as f32 * cell_w;
            rules.push(rule(x, y0, x, y1));
        }
        for r in 0..rows {
            for c in 0..cols {
                let cx0 = x0 + c as f32 * cell_w + 4.0;
                let top_y0 = y1 - (r as f32 + 1.0) * cell_h + 4.0;
                let top_y1 = top_y0 + 10.0;
                runs.push(tr(
                    &format!("r{r}c{c}"),
                    cx0,
                    top_y0,
                    cx0 + 20.0,
                    top_y1,
                ));
            }
        }
        (runs, rules)
    }

    #[test]
    fn tables_available_true() {
        assert!(tables_available());
    }

    #[test]
    fn detect_off_returns_empty() {
        let (runs, rules) = lattice_grid(50.0, 200.0, 4, 3, 60.0, 20.0);
        assert!(detect_tables_page(0, &runs, &rules, &TableOptions::default()).is_empty());
    }

    #[test]
    fn detect_lattice_page() {
        let (runs, rules) = lattice_grid(50.0, 200.0, 5, 4, 70.0, 22.0);
        let opts = TableOptions::from_preset(TablePreset::Full);
        let tabs = detect_tables_page(0, &runs, &rules, &opts);
        assert!(!tabs.is_empty());
        assert!(matches!(tabs[0].method, TableMethod::Lattice | TableMethod::Hybrid));
        assert!(tabs[0].rows >= 3 && tabs[0].cols >= 3);
    }

    #[test]
    fn auto_finds_ruled_grid() {
        let (runs, rules) = lattice_grid(50.0, 200.0, 5, 4, 70.0, 22.0);
        let opts = TableOptions::from_preset(TablePreset::Auto);
        let tabs = detect_tables_page(0, &runs, &rules, &opts);
        assert!(!tabs.is_empty());
        assert!(tabs.iter().any(|t| t.method == TableMethod::Lattice));
    }

    #[test]
    fn presets() {
        assert!(!TableOptions::from_preset(TablePreset::Off).detect_tables);
        assert!(TableOptions::from_preset(TablePreset::LatticeOnly).modes.lattice);
        assert!(TableOptions::from_preset(TablePreset::Full).modes.hybrid);
        assert!(TableOptions::from_preset(TablePreset::Auto).exclusive_under_strong_lattice);
        assert_eq!(
            TableOptions::from_preset(TablePreset::Auto).modes.lattice,
            TableOptions::from_preset(TablePreset::Full).modes.lattice
        );
    }
}
