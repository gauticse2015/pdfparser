//! Layout: R8 rotation normalize, space insertion, reading order (Phase T).
#![deny(missing_docs)]

use pdfparser_ir::{Point, Rect, TextRun};

/// Apply page /Rotate to a point (R8 frozen matrices).
pub fn rotate_point(p: Point, rotate: i32, media: Rect) -> Point {
    let r = rotate.rem_euclid(360);
    let x = p.x - media.x0;
    let y = p.y - media.y0;
    let w = media.width();
    let h = media.height();
    match r {
        90 => Point {
            x: media.x0 + y,
            y: media.y0 + (w - x),
        },
        180 => Point {
            x: media.x0 + (w - x),
            y: media.y0 + (h - y),
        },
        270 => Point {
            x: media.x0 + (h - y),
            y: media.y0 + x,
        },
        _ => Point {
            x: media.x0 + x,
            y: media.y0 + y,
        },
    }
}

/// Transform rect by page rotate.
pub fn to_upright_rect(r: Rect, page_rotate: i32, media: Rect) -> Rect {
    Rect::from_points(
        r.corners()
            .into_iter()
            .map(|p| rotate_point(p, page_rotate, media)),
    )
}

/// Transform runs into upright space when rotate != 0.
pub fn apply_page_rotate_to_runs(runs: &mut [TextRun], rotate: i32, media: Rect) {
    if rotate.rem_euclid(360) == 0 {
        return;
    }
    for run in runs.iter_mut() {
        run.bbox = to_upright_rect(run.bbox, rotate, media);
    }
}

/// Insert spaces between runs on same line when gap is large.
pub fn insert_spaces(runs: &[TextRun]) -> Vec<TextRun> {
    if runs.is_empty() {
        return Vec::new();
    }
    // Work on sorted copy by geometry
    let mut indexed: Vec<(usize, &TextRun)> = runs.iter().enumerate().collect();
    // group into bands
    let median_fs = median(runs.iter().map(|r| r.font_size).collect());
    let y_tol = 0.25 * median_fs.max(1.0);

    indexed.sort_by(|a, b| {
        b.1.bbox
            .y_center()
            .partial_cmp(&a.1.bbox.y_center())
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| {
                a.1.bbox
                    .x0
                    .partial_cmp(&b.1.bbox.x0)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
    });

    let mut out: Vec<TextRun> = Vec::new();
    let mut band: Vec<&TextRun> = Vec::new();
    let mut band_y = indexed[0].1.bbox.y_center();

    let flush_band = |band: &mut Vec<&TextRun>, out: &mut Vec<TextRun>, median_fs: f32| {
        if band.is_empty() {
            return;
        }
        band.sort_by(|a, b| {
            a.bbox
                .x0
                .partial_cmp(&b.bbox.x0)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        let space_w = 0.25 * median_fs.max(1.0);
        // Gaps larger than this are column gutters, not word spaces.
        let max_word_gap = median_fs.max(1.0) * 3.0;
        for (i, run) in band.iter().enumerate() {
            if i > 0 {
                let prev = band[i - 1];
                let gap = run.bbox.x0 - prev.bbox.x1;
                if gap > space_w * 0.5
                    && gap <= max_word_gap
                    && !prev.text.ends_with(' ')
                    && !run.text.starts_with(' ')
                {
                    let mut space = (*run).clone();
                    space.text = " ".into();
                    // Keep space bbox local (do not span gutters).
                    let mid = (prev.bbox.x1 + run.bbox.x0) * 0.5;
                    space.bbox = Rect {
                        x0: mid - space_w * 0.25,
                        y0: prev.bbox.y0.min(run.bbox.y0),
                        x1: mid + space_w * 0.25,
                        y1: prev.bbox.y1.max(run.bbox.y1),
                    };
                    out.push(space);
                }
            }
            out.push((*run).clone());
        }
        band.clear();
    };

    for (_, run) in indexed {
        if (run.bbox.y_center() - band_y).abs() <= y_tol {
            band.push(run);
        } else {
            flush_band(&mut band, &mut out, median_fs);
            band_y = run.bbox.y_center();
            band.push(run);
        }
    }
    flush_band(&mut band, &mut out, median_fs);
    out
}

/// Build plain text in reading order (multi-column aware).
pub fn reading_order_text(runs: &[TextRun]) -> String {
    if runs.is_empty() {
        return String::new();
    }
    let median_fs = median(runs.iter().map(|r| r.font_size).collect()).max(1.0);
    let y_tol = 0.25 * median_fs;
    let median_space = 0.25 * median_fs;
    let g_col = 1.5 * median_space;
    let w_min = 3.0 * median_fs;

    // lines: cluster by y
    let mut items: Vec<&TextRun> = runs
        .iter()
        .filter(|r| !r.text.trim().is_empty() || r.text == " ")
        .collect();
    // exclude pure space-only from column detect but keep in assembly
    items.sort_by(|a, b| {
        b.bbox
            .y_center()
            .partial_cmp(&a.bbox.y_center())
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(
                a.bbox
                    .x0
                    .partial_cmp(&b.bbox.x0)
                    .unwrap_or(std::cmp::Ordering::Equal),
            )
    });

    #[derive(Clone)]
    struct Line {
        y: f32,
        runs: Vec<TextRun>,
        x0: f32,
        x1: f32,
    }

    let mut lines: Vec<Line> = Vec::new();
    for run in items {
        if run.text == " " {
            // attach space to previous run on line if any
            if let Some(line) = lines.last_mut() {
                if (line.y - run.bbox.y_center()).abs() <= y_tol {
                    line.runs.push(run.clone());
                    line.x1 = line.x1.max(run.bbox.x1);
                    continue;
                }
            }
        }
        if let Some(line) = lines.last_mut() {
            if (line.y - run.bbox.y_center()).abs() <= y_tol {
                // Merge only when the run continues to the RIGHT with a modest gap.
                // Large positive gap => column gutter; large negative => other column.
                let gap = run.bbox.x0 - line.x1;
                if gap >= -0.5 * median_fs && gap < g_col * 2.0 {
                    line.runs.push(run.clone());
                    line.x0 = line.x0.min(run.bbox.x0);
                    line.x1 = line.x1.max(run.bbox.x1);
                    continue;
                }
            }
        }
        lines.push(Line {
            y: run.bbox.y_center(),
            runs: vec![run.clone()],
            x0: run.bbox.x0,
            x1: run.bbox.x1,
        });
    }

    for line in &mut lines {
        line.runs.sort_by(|a, b| {
            a.bbox
                .x0
                .partial_cmp(&b.bbox.x0)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
    }

    // Detect two-column layout: look for vertical gutter
    let page_x0 = lines.iter().map(|l| l.x0).fold(f32::INFINITY, f32::min);
    let page_x1 = lines.iter().map(|l| l.x1).fold(f32::NEG_INFINITY, f32::max);
    let mid = (page_x0 + page_x1) * 0.5;

    // Count lines mostly left vs right
    let mut left_lines = 0;
    let mut right_lines = 0;
    for l in &lines {
        let cx = (l.x0 + l.x1) * 0.5;
        let w = l.x1 - l.x0;
        if w >= (page_x1 - page_x0) * 0.8 {
            continue; // spanning
        }
        if cx < mid - g_col {
            left_lines += 1;
        } else if cx > mid + g_col {
            right_lines += 1;
        }
    }

    let multi =
        left_lines >= 2 && right_lines >= 2 && (mid - page_x0) >= w_min && (page_x1 - mid) >= w_min;

    let line_text =
        |l: &Line| -> String { l.runs.iter().map(|r| r.text.as_str()).collect::<String>() };

    if !multi {
        // single column: top to bottom
        let mut out = String::new();
        for (i, l) in lines.iter().enumerate() {
            if i > 0 {
                out.push('\n');
            }
            out.push_str(&line_text(l));
        }
        return out;
    }

    // multi-column: spanning first, then left column top-bottom, then right
    let mut spanning = Vec::new();
    let mut left = Vec::new();
    let mut right = Vec::new();
    let full_w = (page_x1 - page_x0).max(1.0);
    for l in &lines {
        let w = l.x1 - l.x0;
        if w >= full_w * 0.80 {
            spanning.push(l);
            continue;
        }
        let cx = (l.x0 + l.x1) * 0.5;
        if cx <= mid {
            left.push(l);
        } else {
            right.push(l);
        }
    }

    let mut out = String::new();
    let mut first = true;
    for l in spanning.into_iter().chain(left).chain(right) {
        if !first {
            out.push('\n');
        }
        first = false;
        out.push_str(&line_text(l));
    }
    out
}

/// Paint-order text join (fallback).
pub fn paint_order_text(runs: &[TextRun]) -> String {
    runs.iter()
        .map(|r| r.text.as_str())
        .collect::<Vec<_>>()
        .join("")
}

fn median(mut v: Vec<f32>) -> f32 {
    if v.is_empty() {
        return 12.0;
    }
    v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    v[v.len() / 2]
}

#[cfg(test)]
mod tests {
    use super::*;
    use pdfparser_ir::Matrix3x2;

    #[test]
    fn rotate_90_roundtrip_corners() {
        let media = Rect {
            x0: 0.0,
            y0: 0.0,
            x1: 100.0,
            y1: 200.0,
        };
        let p = Point { x: 10.0, y: 20.0 };
        let p1 = rotate_point(p, 90, media);
        let p2 = rotate_point(p1, 90, media);
        let p3 = rotate_point(p2, 90, media);
        let p4 = rotate_point(p3, 90, media);
        assert!((p4.x - p.x).abs() < 1e-2);
        assert!((p4.y - p.y).abs() < 1e-2);
    }

    #[test]
    fn multi_column_order() {
        let mk = |t: &str, x0: f32, y: f32| TextRun {
            text: t.into(),
            bbox: Rect {
                x0,
                y0: y,
                x1: x0 + 50.0,
                y1: y + 10.0,
            },
            transform: Matrix3x2::identity(),
            font_name: None,
            font_size: 10.0,
            mapping_confidence: 1.0,
            metrics_confidence: 1.0,
            mcid: None,
            invisible: false,
            from_actual_text: false,
        };
        // left column higher y first then lower; right similar
        let runs = vec![
            mk("LEFT_COL_START", 72.0, 700.0),
            mk("LEFT_COL_END", 72.0, 600.0),
            mk("RIGHT_COL_START", 320.0, 700.0),
            mk("RIGHT_COL_END", 320.0, 600.0),
        ];
        let text = reading_order_text(&runs);
        let l0 = text.find("LEFT_COL_START").unwrap();
        let l1 = text.find("LEFT_COL_END").unwrap();
        let r0 = text.find("RIGHT_COL_START").unwrap();
        let r1 = text.find("RIGHT_COL_END").unwrap();
        assert!(l0 < l1 && l1 < r0 && r0 < r1, "order was: {text}");
    }
}
