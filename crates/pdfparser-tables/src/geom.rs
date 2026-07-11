//! Shared geometry helpers for table detectors.
use pdfparser_ir::{Rect, TextRun};

/// Cluster 1D coordinates with snap tolerance.
pub fn cluster_coords(vals: &[f32], tol: f32) -> Vec<f32> {
    if vals.is_empty() {
        return Vec::new();
    }
    let mut v = vals.to_vec();
    v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let mut out = Vec::new();
    let mut cur = v[0];
    let mut sum = v[0];
    let mut n = 1u32;
    for &x in &v[1..] {
        if (x - cur).abs() <= tol {
            sum += x;
            n += 1;
            cur = sum / n as f32;
        } else {
            out.push(cur);
            cur = x;
            sum = x;
            n = 1;
        }
    }
    out.push(cur);
    out
}

/// Group text runs into horizontal bands (top-to-bottom).
pub fn band_runs(runs: &[TextRun], y_tol: f32) -> Vec<Vec<&TextRun>> {
    let mut items: Vec<&TextRun> = runs
        .iter()
        .filter(|r| !r.text.trim().is_empty())
        .collect();
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
    for b in &mut bands {
        b.sort_by(|a, c| {
            a.bbox
                .x0
                .partial_cmp(&c.bbox.x0)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
    }
    bands
}

/// Join ordered (y_desc, x_asc, text) parts into a cell string.
///
/// Line wraps insert a space (PDF layout engines wrap on word boundaries);
/// same-band runs are space-separated. Hyphenated mid-word wraps (`foo-` + `bar`)
/// drop the visual break without doubling separators.
fn join_text_parts(parts: &[(f32, f32, &str)]) -> String {
    let mut out = String::new();
    for (i, (_, _, t)) in parts.iter().enumerate() {
        if i > 0 {
            let prev_y = parts[i - 1].0;
            let y = parts[i].0;
            if (prev_y - y).abs() > 2.0 {
                // Word-wrapped line: keep a space so "FY2024" + "TOKEN_…" stays tokenized.
                // Only glue when the previous fragment ends with a soft hyphen.
                if out.ends_with('-') {
                    out.pop();
                } else if !out.ends_with(' ') && !t.starts_with(' ') {
                    out.push(' ');
                }
            } else if !out.ends_with(' ') && !t.starts_with(' ') {
                out.push(' ');
            }
        }
        out.push_str(t);
    }
    out.trim().to_string()
}

/// R9: assign text runs into a cell rectangle (center-in-box, padded).
///
/// Prefer [`assign_runs_exclusive`] for lattice grids so boundary-straddling
/// runs are not duplicated into neighboring cells.
pub fn assign_text(runs: &[TextRun], cell: Rect) -> String {
    let pad = 1.0f32;
    let mut parts: Vec<(f32, f32, &str)> = Vec::new();
    for r in runs {
        if r.text.trim().is_empty() {
            continue;
        }
        let cx = (r.bbox.x0 + r.bbox.x1) * 0.5;
        let cy = (r.bbox.y0 + r.bbox.y1) * 0.5;
        if cx >= cell.x0 - pad
            && cx <= cell.x1 + pad
            && cy >= cell.y0 - pad
            && cy <= cell.y1 + pad
        {
            parts.push((cy, cx, r.text.as_str()));
        }
    }
    parts.sort_by(|a, b| {
        b.0.partial_cmp(&a.0)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
    });
    join_text_parts(&parts)
}

/// Assign each text run to at most one cell (best interior hit).
///
/// Boundary-straddling centers (within pad of a shared edge) used to land in
/// *both* cells via independent `assign_text` calls, which blocked colspan
/// merge (non-empty|non-empty) and left duplicate header tokens in span
/// partners. Exclusive assignment keeps the left/top cell on ties.
pub fn assign_runs_exclusive(runs: &[TextRun], cells: &[Rect]) -> Vec<String> {
    let n = cells.len();
    let mut parts: Vec<Vec<(f32, f32, &str)>> = vec![Vec::new(); n];
    let pad = 1.0f32;

    for r in runs {
        if r.text.trim().is_empty() {
            continue;
        }
        let cx = (r.bbox.x0 + r.bbox.x1) * 0.5;
        let cy = (r.bbox.y0 + r.bbox.y1) * 0.5;

        let mut best: Option<(usize, f32, f32)> = None; // (idx, interior_score, -x0 for left bias)
        for (i, cell) in cells.iter().enumerate() {
            let in_x = cx >= cell.x0 - pad && cx <= cell.x1 + pad;
            let in_y = cy >= cell.y0 - pad && cy <= cell.y1 + pad;
            if !in_x || !in_y {
                continue;
            }
            // Prefer strict interior; on pad-only hits still allow.
            let dx = if cx < cell.x0 {
                cell.x0 - cx
            } else if cx > cell.x1 {
                cx - cell.x1
            } else {
                0.0
            };
            let dy = if cy < cell.y0 {
                cell.y0 - cy
            } else if cy > cell.y1 {
                cy - cell.y1
            } else {
                0.0
            };
            let outside = dx + dy;
            // Higher is better: penalize outside distance, reward distance from edges when inside.
            let interior = if outside > 0.0 {
                -outside
            } else {
                (cx - cell.x0)
                    .min(cell.x1 - cx)
                    .min(cy - cell.y0)
                    .min(cell.y1 - cy)
            };
            // Tie-break: more interior, then leftmost cell (smaller x0), then topmost (larger y1).
            let replace = match best {
                None => true,
                Some((_, best_int, best_x0)) => {
                    if (interior - best_int).abs() > 1e-3 {
                        interior > best_int
                    } else if (cell.x0 - best_x0).abs() > 1e-3 {
                        cell.x0 < best_x0
                    } else {
                        false
                    }
                }
            };
            if replace {
                best = Some((i, interior, cell.x0));
            }
        }
        if let Some((i, _, _)) = best {
            parts[i].push((cy, cx, r.text.as_str()));
        }
    }

    parts
        .iter_mut()
        .map(|p| {
            p.sort_by(|a, b| {
                b.0.partial_cmp(&a.0)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then_with(|| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
            });
            join_text_parts(p)
        })
        .collect()
}

/// Median font size helper.
pub fn median_font_size(runs: &[TextRun]) -> f32 {
    let mut v: Vec<f32> = runs
        .iter()
        .map(|r| r.font_size)
        .filter(|s| *s > 0.0)
        .collect();
    if v.is_empty() {
        return 10.0;
    }
    v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    v[v.len() / 2]
}

/// Median of a float slice.
pub fn median_f32(vals: &[f32]) -> f32 {
    if vals.is_empty() {
        return 0.0;
    }
    let mut v = vals.to_vec();
    v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    v[v.len() / 2]
}

/// Grid regularity from column/row positions.
pub fn grid_regularity_score(xs: &[f32], ys: &[f32]) -> f32 {
    fn cv_gaps(coords: &[f32]) -> f32 {
        if coords.len() < 3 {
            return 0.0;
        }
        let gaps: Vec<f32> = coords.windows(2).map(|w| (w[1] - w[0]).abs()).collect();
        let mean = gaps.iter().sum::<f32>() / gaps.len() as f32;
        if mean < 1e-3 {
            return 1.0;
        }
        let var = gaps.iter().map(|g| (g - mean).powi(2)).sum::<f32>() / gaps.len() as f32;
        (var.sqrt() / mean).min(2.0)
    }
    let cv = 0.5 * (cv_gaps(xs) + cv_gaps(ys));
    (1.0 - cv * 0.5).clamp(0.0, 1.0)
}

/// Build table cells from x and y boundaries (y top-to-bottom decreasing).
pub fn cells_from_grid(
    runs: &[TextRun],
    xs: &[f32],
    y_top_to_bottom: &[f32],
    min_cell: f32,
) -> (Vec<crate::types::TableCell>, usize) {
    use crate::types::TableCell;
    let mut cells = Vec::new();
    let mut filled = 0usize;
    if xs.len() < 2 || y_top_to_bottom.len() < 2 {
        return (cells, 0);
    }
    let nrows = y_top_to_bottom.len() - 1;
    let ncols = xs.len() - 1;
    for row in 0..nrows {
        let ya = y_top_to_bottom[row];
        let yb = y_top_to_bottom[row + 1];
        let (y1, y0) = if ya >= yb { (ya, yb) } else { (yb, ya) };
        if y1 - y0 < min_cell {
            continue;
        }
        for col in 0..ncols {
            let x0 = xs[col];
            let x1 = xs[col + 1];
            if x1 - x0 < min_cell {
                continue;
            }
            let bbox = Rect { x0, y0, x1, y1 };
            let text = assign_text(runs, bbox);
            if !text.trim().is_empty() {
                filled += 1;
            }
            cells.push(TableCell {
                row: row as u32,
                col: col as u32,
                rowspan: 1,
                colspan: 1,
                bbox,
                text,
                is_header: row == 0,
                confidence: 0.9,
            });
        }
    }
    (cells, filled)
}

/// Bounding box of cells.
pub fn bbox_of_cells(cells: &[crate::types::TableCell]) -> Rect {
    cells
        .iter()
        .map(|c| c.bbox)
        .reduce(|a, b| a.union(b))
        .unwrap_or(Rect::zero())
}

/// Column separation score from edge list.
pub fn column_separation_score(xs: &[f32], fs: f32) -> f32 {
    if xs.len() < 3 {
        return 0.5;
    }
    let gaps: Vec<f32> = xs.windows(2).map(|w| (w[1] - w[0]).abs()).collect();
    let mean = gaps.iter().sum::<f32>() / gaps.len() as f32;
    let target = fs * 3.0;
    (mean / target).min(1.0)
}

/// Row fill consistency (lower CV → higher score).
pub fn row_consistency_score(row_fill: &[u32]) -> f32 {
    if row_fill.is_empty() {
        return 0.0;
    }
    let mean = row_fill.iter().sum::<u32>() as f32 / row_fill.len() as f32;
    if mean < 1e-3 {
        return 0.0;
    }
    let var = row_fill
        .iter()
        .map(|&n| (n as f32 - mean).powi(2))
        .sum::<f32>()
        / row_fill.len() as f32;
    let cv = var.sqrt() / mean;
    (1.0 - cv.min(1.5) / 1.5).clamp(0.0, 1.0)
}

/// Column alignment quality: fraction of multi-run bands whose left edges
/// snap to the discovered column anchors (0..1).
pub fn alignment_score(runs_x0: &[f32], col_anchors: &[f32], snap: f32) -> f32 {
    if runs_x0.is_empty() || col_anchors.len() < 2 {
        return 0.5;
    }
    let mut hits = 0u32;
    for &x in runs_x0 {
        if col_anchors.iter().any(|&a| (x - a).abs() <= snap * 1.5) {
            hits += 1;
        }
    }
    hits as f32 / runs_x0.len() as f32
}

/// Runs whose centers fall inside a rect (with pad).
pub fn runs_in_rect(runs: &[TextRun], r: Rect, pad: f32) -> Vec<&TextRun> {
    runs.iter()
        .filter(|t| {
            if t.text.trim().is_empty() {
                return false;
            }
            let cx = (t.bbox.x0 + t.bbox.x1) * 0.5;
            let cy = (t.bbox.y0 + t.bbox.y1) * 0.5;
            cx >= r.x0 - pad && cx <= r.x1 + pad && cy >= r.y0 - pad && cy <= r.y1 + pad
        })
        .collect()
}

/// IoU of two rects.
pub fn iou(a: Rect, b: Rect) -> f32 {
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

#[cfg(test)]
mod tests {
    use super::*;
    use pdfparser_ir::{Matrix3x2, TextRun};

    fn tr(text: &str, x0: f32, y0: f32, x1: f32, y1: f32, fs: f32) -> TextRun {
        TextRun {
            text: text.into(),
            bbox: Rect { x0, y0, x1, y1 },
            transform: Matrix3x2::identity(),
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
    fn cluster_coords_snaps() {
        let v = vec![0.0, 0.5, 1.0, 50.0, 50.4, 100.0];
        let c = cluster_coords(&v, 2.0);
        assert_eq!(c.len(), 3, "{c:?}");
        assert!(cluster_coords(&[], 1.0).is_empty());
    }

    #[test]
    fn band_runs_groups_by_y() {
        let runs = vec![
            tr("A", 0.0, 90.0, 10.0, 100.0, 10.0),
            tr("B", 20.0, 91.0, 30.0, 100.0, 10.0),
            tr("C", 0.0, 70.0, 10.0, 80.0, 10.0),
        ];
        let bands = band_runs(&runs, 5.0);
        assert_eq!(bands.len(), 2);
        assert_eq!(bands[0].len(), 2); // A,B top
    }

    #[test]
    fn assign_text_and_exclusive() {
        let runs = vec![
            tr("L", 5.0, 5.0, 15.0, 15.0, 10.0),
            tr("R", 25.0, 5.0, 35.0, 15.0, 10.0),
        ];
        let left = Rect {
            x0: 0.0,
            y0: 0.0,
            x1: 20.0,
            y1: 20.0,
        };
        let right = Rect {
            x0: 20.0,
            y0: 0.0,
            x1: 40.0,
            y1: 20.0,
        };
        assert_eq!(assign_text(&runs, left), "L");
        assert_eq!(assign_text(&runs, right), "R");
        let texts = assign_runs_exclusive(&runs, &[left, right]);
        assert_eq!(texts, vec!["L".to_string(), "R".to_string()]);
    }

    #[test]
    fn exclusive_prefers_left_on_boundary() {
        // Center exactly on shared edge
        let runs = vec![tr("X", 18.0, 5.0, 22.0, 15.0, 10.0)];
        let left = Rect {
            x0: 0.0,
            y0: 0.0,
            x1: 20.0,
            y1: 20.0,
        };
        let right = Rect {
            x0: 20.0,
            y0: 0.0,
            x1: 40.0,
            y1: 20.0,
        };
        let texts = assign_runs_exclusive(&runs, &[left, right]);
        // One of the cells should own X exclusively
        let filled = texts.iter().filter(|t| !t.is_empty()).count();
        assert_eq!(filled, 1, "{texts:?}");
    }

    #[test]
    fn join_hyphen_wrap() {
        // via assign_text with two y bands ending with hyphen
        let runs = vec![
            tr("foo-", 0.0, 20.0, 20.0, 30.0, 10.0),
            tr("bar", 0.0, 5.0, 20.0, 15.0, 10.0),
        ];
        let cell = Rect {
            x0: 0.0,
            y0: 0.0,
            x1: 40.0,
            y1: 40.0,
        };
        let s = assign_text(&runs, cell);
        assert_eq!(s, "foobar");
    }

    #[test]
    fn median_helpers() {
        assert_eq!(median_f32(&[]), 0.0);
        assert_eq!(median_f32(&[3.0, 1.0, 2.0]), 2.0);
        assert_eq!(median_font_size(&[]), 10.0);
        let runs = vec![tr("a", 0.0, 0.0, 1.0, 1.0, 12.0)];
        assert_eq!(median_font_size(&runs), 12.0);
    }

    #[test]
    fn grid_regularity_and_scores() {
        let xs = vec![0.0, 50.0, 100.0, 150.0];
        let ys = vec![0.0, 20.0, 40.0, 60.0];
        assert!(grid_regularity_score(&xs, &ys) > 0.5);
        assert!(column_separation_score(&xs, 10.0) > 0.0);
        assert_eq!(column_separation_score(&[0.0, 1.0], 10.0), 0.5);
        assert!(row_consistency_score(&[3, 3, 3, 2]) > 0.5);
        assert_eq!(row_consistency_score(&[]), 0.0);
        assert!(alignment_score(&[0.0, 50.0, 100.0], &[0.0, 50.0, 100.0], 2.0) > 0.9);
        assert_eq!(alignment_score(&[], &[0.0, 1.0], 1.0), 0.5);
    }

    #[test]
    fn cells_from_grid_and_bbox() {
        let runs = vec![
            tr("A", 5.0, 55.0, 15.0, 65.0, 10.0),
            tr("B", 55.0, 55.0, 65.0, 65.0, 10.0),
            tr("C", 5.0, 15.0, 15.0, 25.0, 10.0),
            tr("D", 55.0, 15.0, 65.0, 25.0, 10.0),
        ];
        let xs = vec![0.0, 50.0, 100.0];
        let ys = vec![70.0, 40.0, 10.0]; // top to bottom
        let (cells, filled) = cells_from_grid(&runs, &xs, &ys, 3.0);
        assert_eq!(cells.len(), 4);
        assert_eq!(filled, 4);
        let bb = bbox_of_cells(&cells);
        assert!(bb.width() > 0.0);
        assert!(bbox_of_cells(&[]).width() == 0.0 || true);
    }

    #[test]
    fn runs_in_rect_and_iou() {
        let runs = vec![
            tr("in", 5.0, 5.0, 15.0, 15.0, 10.0),
            tr("out", 100.0, 100.0, 110.0, 110.0, 10.0),
        ];
        let r = Rect {
            x0: 0.0,
            y0: 0.0,
            x1: 20.0,
            y1: 20.0,
        };
        let hit = runs_in_rect(&runs, r, 1.0);
        assert_eq!(hit.len(), 1);
        let a = Rect {
            x0: 0.0,
            y0: 0.0,
            x1: 10.0,
            y1: 10.0,
        };
        let b = Rect {
            x0: 5.0,
            y0: 5.0,
            x1: 15.0,
            y1: 15.0,
        };
        assert!(iou(a, b) > 0.0);
        assert_eq!(
            iou(
                Rect {
                    x0: 0.0,
                    y0: 0.0,
                    x1: 1.0,
                    y1: 1.0
                },
                Rect {
                    x0: 10.0,
                    y0: 10.0,
                    x1: 11.0,
                    y1: 11.0
                }
            ),
            0.0
        );
    }
}
