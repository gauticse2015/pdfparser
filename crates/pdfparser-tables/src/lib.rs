//! Multi-strategy PDF table extraction (lattice, stream, hybrid, stitch, FP control).
#![deny(missing_docs)]

mod form;
mod geom;
mod hybrid;
mod lattice;
mod options;
mod split;
mod stitch;
mod stream;
mod types;

pub use form::scrub_document_table_fps;
pub use lattice::detect_lattice_tables;
pub use options::{TableModeSet, TableOptions, TablePreset};
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
    if !opts.detect_tables {
        return Vec::new();
    }

    let mut cands = Vec::new();

    if opts.modes.lattice {
        cands.extend(detect_lattice_tables(page_index, runs, rules, opts));
    }

    // Strong lattice: typed quality (confidence + not weak_edges), not string notes.
    let strong_lattice_bboxes: Vec<pdfparser_ir::Rect> = cands
        .iter()
        .filter(|t| {
            t.method == TableMethod::Lattice
                && t.cols >= 2
                && t.rows >= 2
                && t.confidence >= opts.strong_lattice_min_conf
                && !t.weak_edges
        })
        .map(|t| t.bbox)
        .collect();
    let has_strong_lattice = !strong_lattice_bboxes.is_empty();
    if opts.modes.hybrid {
        let hybrid = detect_hybrid_tables(page_index, runs, rules, opts);
        if !has_strong_lattice {
            cands.extend(hybrid);
        } else {
            for h in hybrid {
                let overlaps_strong = strong_lattice_bboxes.iter().any(|&kb| {
                    region_overlap(kb, h.bbox) >= 0.40 || geom::iou(kb, h.bbox) >= 0.35
                });
                if !overlaps_strong {
                    cands.push(h);
                }
            }
        }
    }

    if opts.modes.stream {
        let stream_cands = detect_stream_tables(page_index, runs, opts);
        for mut s in stream_cands {
            let under_ruled = cands.iter().any(|k| {
                matches!(k.method, TableMethod::Lattice | TableMethod::Hybrid)
                    && region_overlap(k.bbox, s.bbox) >= 0.45
            });
            if under_ruled {
                s.confidence *= 0.55;
                s.notes.push("demoted_under_ruled".into());
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
    let mut kept = nms(cands, min_conf);
    kept.retain(|t| match t.method {
        TableMethod::Stream => t.confidence >= opts.min_confidence_stream,
        _ => t.confidence >= opts.min_table_confidence * 0.85 || t.confidence >= 0.52,
    });
    kept.truncate(opts.max_tables_per_page as usize);
    kept
}

/// Detect tables for all pages; optional stitch and over-seg scrub.
///
/// `page_heights[i]` should be media-box height in user units for page `i`.
pub fn detect_tables_document(
    pages: &[(u32, &[TextRun], &[RuleSegment])],
    page_heights: &[f32],
    opts: &TableOptions,
) -> (Vec<Vec<Table>>, Vec<Table>) {
    let mut page_tables: Vec<Vec<Table>> = pages
        .iter()
        .map(|(idx, runs, rules)| detect_tables_page(*idx, runs, rules, opts))
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

fn nms(mut cands: Vec<Table>, min_conf: f32) -> Vec<Table> {
    cands.retain(|t| t.confidence >= min_conf * 0.80);
    // Prefer method rank, then composite quality (confidence + fill + edges).
    // Do not prefer smaller tables or hard cell-count cutoffs.
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

/// Composite ranking score for NMS (higher is better).
fn quality_score(t: &Table) -> f32 {
    let edge = if t.edge_score > 0.0 {
        t.edge_score
    } else {
        0.5 // unknown (stream) — neutral
    };
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
