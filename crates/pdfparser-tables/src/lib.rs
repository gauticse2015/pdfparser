//! Table extraction engine — Phase U: lattice (S2) + cell assign (R9) + NMS/confidence.
#![deny(missing_docs)]

mod lattice;
mod options;
mod types;

pub use lattice::detect_lattice_tables;
pub use options::{TableModeSet, TableOptions, TablePreset};
pub use types::{PipelineId, Table, TableCell, TableMethod};

use pdfparser_content::RuleSegment;
use pdfparser_ir::TextRun;

/// Whether the table engine is available (Phase U+).
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

    // S2 Lattice
    if opts.modes.lattice {
        cands.extend(detect_lattice_tables(page_index, runs, rules, opts));
    }

    // S1 Structure — Phase U stub (no structure map yet)
    // S3/S4 — Phase V

    // NMS by bbox IoU
    let mut kept = nms(cands, opts.min_table_confidence, 0.5);
    kept.truncate(opts.max_tables_per_page as usize);
    kept
}

fn nms(mut cands: Vec<Table>, min_conf: f32, iou_thresh: f32) -> Vec<Table> {
    cands.retain(|t| t.confidence >= min_conf);
    cands.sort_by(|a, b| {
        b.confidence
            .partial_cmp(&a.confidence)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| method_rank(b.method).cmp(&method_rank(a.method)))
    });
    let mut out: Vec<Table> = Vec::new();
    for c in cands {
        let overlaps = out.iter().any(|k| iou(k.bbox, c.bbox) >= iou_thresh);
        if !overlaps {
            out.push(c);
        }
    }
    out
}

fn method_rank(m: TableMethod) -> u8 {
    match m {
        TableMethod::Structure => 4,
        TableMethod::Hybrid => 3,
        TableMethod::Lattice => 2,
        TableMethod::Stream => 1,
        _ => 0,
    }
}

fn iou(a: pdfparser_ir::Rect, b: pdfparser_ir::Rect) -> f32 {
    let x0 = a.x0.max(b.x0);
    let y0 = a.y0.max(b.y0);
    let x1 = a.x1.min(b.x1);
    let y1 = a.y1.min(b.y1);
    let w = (x1 - x0).max(0.0);
    let h = (y1 - y0).max(0.0);
    let inter = w * h;
    let ua = a.width() * a.height() + b.width() * b.height() - inter;
    if ua <= 0.0 {
        0.0
    } else {
        inter / ua
    }
}
