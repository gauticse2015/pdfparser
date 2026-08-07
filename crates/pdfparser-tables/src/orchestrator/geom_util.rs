//! Shared bbox / ranking helpers for orchestrator stages.
use crate::geom;
use crate::types::{Table, TableMethod};

pub(crate) fn overlaps_any(bbox: pdfparser_ir::Rect, regions: &[pdfparser_ir::Rect]) -> bool {
    regions
        .iter()
        .any(|&kb| region_overlap(kb, bbox) >= 0.40 || geom::iou(kb, bbox) >= 0.35)
}
pub(crate) fn containment_ratio(inner: pdfparser_ir::Rect, outer: pdfparser_ir::Rect) -> f32 {
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

pub(crate) fn quality_score(t: &Table) -> f32 {
    let edge = if t.edge_score > 0.0 {
        t.edge_score
    } else {
        0.5
    };
    let fill = if t.fill_rate > 0.0 { t.fill_rate } else { 0.5 };
    let weak_pen = if t.weak_edges { 0.85 } else { 1.0 };
    (0.55 * t.confidence + 0.25 * fill + 0.20 * edge) * weak_pen
}

pub(crate) fn region_overlap(a: pdfparser_ir::Rect, b: pdfparser_ir::Rect) -> f32 {
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

pub(crate) fn method_rank(m: TableMethod) -> u8 {
    match m {
        TableMethod::Structure => 5,
        TableMethod::Lattice => 4,
        TableMethod::Hybrid => 3,
        TableMethod::Stream => 1,
        TableMethod::DenseNumeric => 2,
        _ => 0,
    }
}
