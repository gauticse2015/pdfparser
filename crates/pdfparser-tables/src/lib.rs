//! Table extraction engine — Phase V: lattice + stream + hybrid + stitch + FP control.
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

    // S2 Lattice first — highest trust for ruled grids
    if opts.modes.lattice {
        cands.extend(detect_lattice_tables(page_index, runs, rules, opts));
    }

    let has_strong_lattice = cands.iter().any(|t| {
        t.method == TableMethod::Lattice && t.cols >= 2 && t.rows >= 2 && t.confidence >= 0.65
    });

    // S4 Hybrid only when lattice did not already recover a strong grid
    // (still allow hybrid for partial-border pages where lattice rejects cols<2)
    if opts.modes.hybrid && !has_strong_lattice {
        cands.extend(detect_hybrid_tables(page_index, runs, rules, opts));
    }

    // S3 Stream — always candidate; NMS suppresses under ruled tables
    if opts.modes.stream {
        let stream_cands = detect_stream_tables(page_index, runs, opts);
        // Demote stream that heavily overlaps a lattice/hybrid candidate
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

    // P4 side-by-side split (on lattice/hybrid over-merges)
    if opts.side_by_side_split {
        cands = split_side_by_side(cands, runs);
    }

    // P1 form discriminator (penalties / veto)
    if opts.form_discriminator {
        cands = apply_form_discriminator(cands);
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

/// Detect tables for all pages and optionally stitch multi-page logical tables.
pub fn detect_tables_document(
    pages: &[(u32, &[TextRun], &[RuleSegment])],
    opts: &TableOptions,
) -> (Vec<Vec<Table>>, Vec<Table>) {
    let mut page_tables: Vec<Vec<Table>> = pages
        .iter()
        .map(|(idx, runs, rules)| detect_tables_page(*idx, runs, rules, opts))
        .collect();

    if opts.stitch_multipage {
        stitch_document(&mut page_tables, opts);
    }

    let mut logical = if opts.stitch_multipage {
        materialize_stitched(&page_tables)
    } else {
        page_tables.iter().flatten().cloned().collect()
    };
    if opts.form_discriminator {
        logical = scrub_document_table_fps(logical);
    }
    (page_tables, logical)
}

fn nms(mut cands: Vec<Table>, min_conf: f32) -> Vec<Table> {
    cands.retain(|t| t.confidence >= min_conf * 0.80);
    cands.sort_by(|a, b| {
        method_rank(b.method)
            .cmp(&method_rank(a.method))
            .then_with(|| {
                b.confidence
                    .partial_cmp(&a.confidence)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .then_with(|| {
                let sa = a.rows * a.cols;
                let sb = b.rows * b.cols;
                sb.cmp(&sa)
            })
    });
    let mut out: Vec<Table> = Vec::new();
    for c in cands {
        let overlaps = out.iter().any(|k| {
            let ov = region_overlap(k.bbox, c.bbox);
            // Tight IoU OR substantial containment of either box
            ov >= 0.28 || geom::iou(k.bbox, c.bbox) >= 0.35
        });
        if !overlaps {
            out.push(c);
        }
    }
    out
}

/// Overlap as intersection / min(area_a, area_b) — catches nested / near-duplicate regions.
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
