//! Candidate prefer / demote / suppress passes.
use super::geom_util::{containment_ratio, region_overlap};
use crate::geom;
use crate::types::{Table, TableMethod};

pub(crate) fn should_suppress_stream_under_lattices(
    stream_bbox: pdfparser_ir::Rect,
    lattices: &[pdfparser_ir::Rect],
) -> bool {
    let s_area = (stream_bbox.width() * stream_bbox.height()).max(1.0);
    for &lat in lattices {
        let overlap = region_overlap(stream_bbox, lat);
        let iou = geom::iou(stream_bbox, lat);
        if overlap < 0.40 && iou < 0.35 {
            continue;
        }
        let l_area = (lat.width() * lat.height()).max(1.0);
        // Stream mostly contained in lattice → suppress.
        if containment_ratio(stream_bbox, lat) >= 0.55 {
            return true;
        }
        // Stream substantially larger than lattice corner → keep stream.
        if s_area > l_area * 2.0 {
            continue;
        }
        // Comparable size + overlap → suppress (classic exclusive).
        return true;
    }
    false
}

pub(crate) fn prefer_lattice_over_overlapping_hybrid(mut cands: Vec<Table>) -> Vec<Table> {
    let lattices: Vec<pdfparser_ir::Rect> = cands
        .iter()
        .filter(|t| t.method == TableMethod::Lattice)
        .map(|t| t.bbox)
        .collect();
    if lattices.is_empty() {
        return cands;
    }
    cands.retain(|t| {
        if t.method != TableMethod::Hybrid {
            return true;
        }
        // Drop hybrid if it largely overlaps any lattice (lattice is preferred for ruled).
        !lattices.iter().any(|&lb| {
            containment_ratio(t.bbox, lb) >= 0.50
                || containment_ratio(lb, t.bbox) >= 0.50
                || geom::iou(t.bbox, lb) >= 0.40
        })
    });
    cands
}

/// Drop sparse over-wide hybrid (or weak lattice) when a high-fill multi-col
/// stream/network already covers the same region.
///
/// Hybrid line-sensing on borderless Quartz/Tabula PDFs invents many empty
/// gutter columns (e.g. 56×27) while network recovers the true schema (56×7).
/// Method-rank NMS would otherwise keep Hybrid over Stream.
pub(crate) fn prefer_stream_over_sparse_hybrid(mut cands: Vec<Table>) -> Vec<Table> {
    if cands.len() < 2 {
        return cands;
    }
    let strong_streams: Vec<(pdfparser_ir::Rect, u32, f32, f32)> = cands
        .iter()
        .filter(|t| {
            matches!(t.method, TableMethod::Stream | TableMethod::DenseNumeric)
                && t.cols >= 3
                && t.rows >= 4
                && t.confidence >= 0.65
                && t.fill_rate >= 0.55
        })
        .map(|t| (t.bbox, t.cols, t.confidence, t.fill_rate))
        .collect();
    if strong_streams.is_empty() {
        return cands;
    }
    cands.retain(|t| {
        if !matches!(t.method, TableMethod::Hybrid | TableMethod::Lattice) {
            return true;
        }
        !strong_streams.iter().any(|&(sb, sc, sconf, sfill)| {
            let overlap = region_overlap(t.bbox, sb) >= 0.40 || geom::iou(t.bbox, sb) >= 0.30;
            if !overlap {
                return false;
            }
            let over_wide = (t.cols as f32) >= (sc as f32) * 1.5 + 1.0;
            let sparse = t.fill_rate > 0.0 && t.fill_rate + 0.12 < sfill;
            let weaker = t.confidence + 0.05 < sconf;
            // Drop only when stream is clearly the better schema for the region.
            (over_wide || sparse) && (weaker || sfill >= 0.70)
        })
    });
    cands
}

/// Drop or demote lattice tables that look like vertical column-group slices
/// when a wider multi-column stream/network table **overlaps the same region**.
///
/// Motivating case: census Table 324 upper stream + overlapping 2-col lattice
/// strip (over-detect). Prefer the wider multi-col table.
///
/// Phase 15: do **not** drop a vertically disjoint 2-col lattice (e.g. Table 325
/// lower on the page) just because an upper wide stream exists.
pub(crate) fn demote_lattice_column_slices(mut cands: Vec<Table>) -> Vec<Table> {
    if cands.len() < 2 {
        return cands;
    }
    let wide_streams: Vec<pdfparser_ir::Rect> = cands
        .iter()
        .filter(|t| {
            matches!(t.method, TableMethod::Stream | TableMethod::DenseNumeric)
                && t.cols >= 4
                && t.rows >= 4
                && t.confidence >= 0.55
        })
        .map(|t| t.bbox)
        .collect();
    if wide_streams.is_empty() {
        // Still demote tiny corners vs large multi-col lattices on-page.
        let has_large = cands.iter().any(|t| t.cols >= 4 && t.rows >= 3);
        if has_large {
            for t in &mut cands {
                if t.method == TableMethod::Lattice && t.cols <= 2 && t.rows <= 4 {
                    t.confidence *= 0.45;
                    t.notes.push("demoted_tiny_lattice_corner".into());
                }
            }
        }
        return cands;
    }

    cands.retain(|t| {
        if t.method != TableMethod::Lattice {
            return true;
        }
        let skinny = t.cols <= 2 && t.rows >= 8;
        if !skinny {
            return true;
        }
        // Only drop if this skinny lattice overlaps a wide stream (same region).
        let overlaps_wide = wide_streams
            .iter()
            .any(|&wb| region_overlap(t.bbox, wb) >= 0.25 || geom::iou(t.bbox, wb) >= 0.20);
        if overlaps_wide {
            return false;
        }
        true
    });
    // Soft demote remaining overlapping 2-col lattices.
    for t in &mut cands {
        if t.method == TableMethod::Lattice && t.cols <= 2 && t.rows >= 8 {
            let overlaps_wide = wide_streams
                .iter()
                .any(|&wb| region_overlap(t.bbox, wb) >= 0.25 || geom::iou(t.bbox, wb) >= 0.20);
            if overlaps_wide {
                t.confidence *= 0.55;
                t.notes.push("demoted_lattice_column_slice".into());
            }
        }
    }
    // Only demote tiny lattice corners against a much larger multi-col **stream**
    // (not hybrid — hybrid often re-detects the same ruled region at 3–4 cols and
    // must not erase a valid 3×2 lattice; sensing 95).
    let large_streams: Vec<(pdfparser_ir::Rect, f32)> = cands
        .iter()
        .filter(|t| {
            matches!(t.method, TableMethod::Stream | TableMethod::DenseNumeric)
                && t.cols >= 4
                && t.rows >= 3
        })
        .map(|t| (t.bbox, (t.bbox.width() * t.bbox.height()).max(1.0)))
        .collect();
    if !large_streams.is_empty() {
        for t in &mut cands {
            if t.method == TableMethod::Lattice && t.cols <= 2 && t.rows <= 4 {
                let t_area = (t.bbox.width() * t.bbox.height()).max(1.0);
                let overlaps_large = large_streams.iter().any(|&(lb, la)| {
                    la >= t_area * 2.0
                        && (region_overlap(t.bbox, lb) >= 0.25 || geom::iou(t.bbox, lb) >= 0.20)
                });
                if overlaps_large {
                    t.confidence *= 0.45;
                    t.notes.push("demoted_tiny_lattice_corner".into());
                }
            }
        }
    }
    cands
}
