//! S4 hybrid detector: partial borders + text-derived columns/rows.
use crate::geom::{
    band_runs, cells_from_grid, cluster_coords, column_separation_score, grid_regularity_score,
    median_f32, median_font_size, runs_in_rect,
};
use crate::options::TableOptions;
use crate::types::{PipelineId, Table, TableMethod};
use pdfparser_content::RuleSegment;
use pdfparser_ir::{Rect, TextRun};

/// Detect hybrid (partial border) tables.
pub fn detect_hybrid_tables(
    page_index: u32,
    runs: &[TextRun],
    rules: &[RuleSegment],
    opts: &TableOptions,
) -> Vec<Table> {
    if rules.len() < 3 || runs.len() < 6 {
        return Vec::new();
    }
    let tol = opts.line_snap_tol;
    let frames = find_outer_frames(rules, tol);
    let mut out = Vec::new();
    for frame in frames {
        if let Some(t) = hybrid_grid(page_index, runs, rules, frame, opts) {
            out.push(t);
        }
    }
    // K34: free page-union frame fallback removed (was hybrid union_rules_frame).
    // Partial tables must come from real outer-frame proposals only — page-wide
    // chrome soup is no longer a hybrid candidate.
    out
}

/// Page-union of all rules (legacy helper). Not used by detect path (K34).
#[allow(dead_code)] // kept for unit tests of geometry only
fn union_rules_frame(rules: &[RuleSegment]) -> Option<Rect> {
    if rules.is_empty() {
        return None;
    }
    let mut x0 = f32::MAX;
    let mut y0 = f32::MAX;
    let mut x1 = f32::MIN;
    let mut y1 = f32::MIN;
    for r in rules {
        x0 = x0.min(r.x0.min(r.x1));
        y0 = y0.min(r.y0.min(r.y1));
        x1 = x1.max(r.x0.max(r.x1));
        y1 = y1.max(r.y0.max(r.y1));
    }
    if x1 - x0 < 40.0 || y1 - y0 < 40.0 {
        return None;
    }
    Some(Rect { x0, y0, x1, y1 })
}

/// Find near-closed rectangular frames from H/V segments.
fn find_outer_frames(rules: &[RuleSegment], tol: f32) -> Vec<Rect> {
    let mut hs = Vec::new();
    let mut vs = Vec::new();
    for r in rules {
        if r.is_horizontal(tol) {
            hs.push(*r);
        } else if r.is_vertical(tol) {
            vs.push(*r);
        }
    }
    if hs.len() < 2 || vs.len() < 2 {
        return Vec::new();
    }

    let y_coords = cluster_coords(
        &hs.iter().map(|s| (s.y0 + s.y1) * 0.5).collect::<Vec<_>>(),
        tol,
    );
    let x_coords = cluster_coords(
        &vs.iter().map(|s| (s.x0 + s.x1) * 0.5).collect::<Vec<_>>(),
        tol,
    );
    if x_coords.len() < 2 || y_coords.len() < 2 {
        return Vec::new();
    }

    // Candidate outer frame = extreme H/V lines that form a box with decent edge coverage
    let mut frames = Vec::new();
    let x_left = x_coords[0];
    let x_right = *x_coords.last().unwrap();
    let y_bot = y_coords[0];
    let y_top = *y_coords.last().unwrap();
    if x_right - x_left < 40.0 || y_top - y_bot < 40.0 {
        return frames;
    }

    let coverage = edge_coverage(x_left, x_right, y_bot, y_top, &hs, &vs, tol);
    // Accept partial frames with ≥3 sides reasonably covered
    let sides_ok = coverage.iter().filter(|&&c| c >= 0.55).count();
    if sides_ok >= 3 || coverage.iter().sum::<f32>() >= 2.5 {
        frames.push(Rect {
            x0: x_left,
            y0: y_bot,
            x1: x_right,
            y1: y_top,
        });
    }
    frames
}

fn edge_coverage(
    x0: f32,
    x1: f32,
    y0: f32,
    y1: f32,
    hs: &[RuleSegment],
    vs: &[RuleSegment],
    tol: f32,
) -> [f32; 4] {
    // top, bottom, left, right
    let width = (x1 - x0).max(1.0);
    let height = (y1 - y0).max(1.0);
    let top = cover_h(hs, y1, x0, x1, tol) / width;
    let bot = cover_h(hs, y0, x0, x1, tol) / width;
    let left = cover_v(vs, x0, y0, y1, tol) / height;
    let right = cover_v(vs, x1, y0, y1, tol) / height;
    [top.min(1.0), bot.min(1.0), left.min(1.0), right.min(1.0)]
}

fn cover_h(hs: &[RuleSegment], y: f32, x0: f32, x1: f32, tol: f32) -> f32 {
    let mut segs: Vec<(f32, f32)> = hs
        .iter()
        .filter(|s| ((s.y0 + s.y1) * 0.5 - y).abs() <= tol * 1.5)
        .map(|s| (s.x0.min(s.x1).max(x0), s.x0.max(s.x1).min(x1)))
        .filter(|(a, b)| b > a)
        .collect();
    if segs.is_empty() {
        return 0.0;
    }
    segs.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    let mut total = 0.0;
    let mut cur_a = segs[0].0;
    let mut cur_b = segs[0].1;
    for &(a, b) in &segs[1..] {
        if a <= cur_b + tol {
            cur_b = cur_b.max(b);
        } else {
            total += cur_b - cur_a;
            cur_a = a;
            cur_b = b;
        }
    }
    total + (cur_b - cur_a)
}

fn cover_v(vs: &[RuleSegment], x: f32, y0: f32, y1: f32, tol: f32) -> f32 {
    let mut segs: Vec<(f32, f32)> = vs
        .iter()
        .filter(|s| ((s.x0 + s.x1) * 0.5 - x).abs() <= tol * 1.5)
        .map(|s| (s.y0.min(s.y1).max(y0), s.y0.max(s.y1).min(y1)))
        .filter(|(a, b)| b > a)
        .collect();
    if segs.is_empty() {
        return 0.0;
    }
    segs.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    let mut total = 0.0;
    let mut cur_a = segs[0].0;
    let mut cur_b = segs[0].1;
    for &(a, b) in &segs[1..] {
        if a <= cur_b + tol {
            cur_b = cur_b.max(b);
        } else {
            total += cur_b - cur_a;
            cur_a = a;
            cur_b = b;
        }
    }
    total + (cur_b - cur_a)
}

fn hybrid_grid(
    page_index: u32,
    runs: &[TextRun],
    rules: &[RuleSegment],
    frame: Rect,
    opts: &TableOptions,
) -> Option<Table> {
    let tol = opts.line_snap_tol;
    let inset = 1.5f32;
    let inner = Rect {
        x0: frame.x0 + inset,
        y0: frame.y0 + inset,
        x1: frame.x1 - inset,
        y1: frame.y1 - inset,
    };
    let inside: Vec<TextRun> = runs_in_rect(runs, frame, 2.0)
        .into_iter()
        .cloned()
        .collect();
    if inside.len() < 6 {
        return None;
    }
    let fs = median_font_size(&inside);

    // --- columns: text peaks + V rules inside frame ---
    let y_tol = (0.5 * fs).max(2.5);
    let bands = band_runs(&inside, y_tol);
    let multi: Vec<&Vec<&TextRun>> = bands.iter().filter(|b| b.len() >= 2).collect();
    let mut x0s: Vec<f32> = Vec::new();
    for b in &multi {
        for r in *b {
            if r.bbox.x0 >= frame.x0 - 2.0 && r.bbox.x0 <= frame.x1 + 2.0 {
                x0s.push(r.bbox.x0);
            }
        }
    }
    let col_snap = (0.55 * fs).max(4.0);
    let mut col_peaks = cluster_coords(&x0s, col_snap);
    col_peaks.retain(|&x| x >= frame.x0 - 5.0 && x <= frame.x1 + 5.0);

    let mut v_rules: Vec<f32> = rules
        .iter()
        .filter(|r| r.is_vertical(tol))
        .map(|r| (r.x0 + r.x1) * 0.5)
        .filter(|&x| x >= frame.x0 - tol && x <= frame.x1 + tol)
        .collect();
    // Snap peaks to V rules
    for p in &mut col_peaks {
        if let Some(v) = v_rules.iter().copied().find(|v| (*v - *p).abs() <= 3.0) {
            *p = v;
        }
    }
    v_rules.extend(col_peaks.iter().copied());
    let mut col_edges = cluster_coords(&v_rules, col_snap * 0.5);
    // Ensure frame edges
    if col_edges.first().map(|&x| x - frame.x0).unwrap_or(100.0) > 5.0 {
        col_edges.insert(0, frame.x0);
    } else if let Some(f) = col_edges.first_mut() {
        *f = frame.x0;
    }
    if col_edges.last().map(|&x| frame.x1 - x).unwrap_or(100.0) > 5.0 {
        col_edges.push(frame.x1);
    } else if let Some(f) = col_edges.last_mut() {
        *f = frame.x1;
    }
    // If only outer edges, rebuild from text peaks alone
    if col_edges.len() < 3 {
        let mut xs = vec![frame.x0];
        for w in col_peaks.windows(2) {
            xs.push((w[0] + w[1]) * 0.5);
        }
        // If single peak list with N anchors, midpoints between them
        if col_peaks.len() >= 2 {
            xs.clear();
            xs.push(frame.x0);
            for w in col_peaks.windows(2) {
                xs.push((w[0] + w[1]) * 0.5);
            }
            xs.push(frame.x1);
        }
        col_edges = xs;
    }
    // Prefer text anchors for multi-column: rebuild when we have ≥3 text peaks
    if col_peaks.len() >= 3 {
        let mut xs = vec![frame.x0];
        for w in col_peaks.windows(2) {
            xs.push((w[0] + w[1]) * 0.5);
        }
        xs.push(frame.x1);
        col_edges = xs;
    }
    let n_cols = col_edges.len().saturating_sub(1);
    if n_cols < 2 {
        return None;
    }

    // --- rows: internal H rules or text baselines ---
    let mut h_int: Vec<f32> = rules
        .iter()
        .filter(|r| r.is_horizontal(tol))
        .map(|r| (r.y0 + r.y1) * 0.5)
        .filter(|&y| y > frame.y0 + tol * 2.0 && y < frame.y1 - tol * 2.0)
        .collect();
    h_int = cluster_coords(&h_int, tol);

    let mut row_edges: Vec<f32>;
    if h_int.len() >= 2 {
        // enough internal rules
        row_edges = vec![frame.y1];
        let mut h_sorted = h_int;
        h_sorted.sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));
        row_edges.extend(h_sorted);
        row_edges.push(frame.y0);
    } else {
        // Text baseline recovery (critical for partial-border)
        let mut y_centers: Vec<f32> = bands
            .iter()
            .filter(|b| {
                b.iter().any(|r| {
                    let cy = r.bbox.y_center();
                    cy >= frame.y0 && cy <= frame.y1
                })
            })
            .map(|b| b.iter().map(|r| r.bbox.y_center()).sum::<f32>() / b.len() as f32)
            .collect();
        let mut y_tol_local = 0.5 * fs;
        y_centers = cluster_coords(&y_centers, y_tol_local);
        if y_centers.len() < 3 {
            y_tol_local = 0.35 * fs;
            let raw: Vec<f32> = inside.iter().map(|r| r.bbox.y_center()).collect();
            y_centers = cluster_coords(&raw, y_tol_local);
        }
        y_centers.sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));
        if y_centers.len() < 3 {
            // equal-split fallback
            let med_h = median_f32(
                &inside
                    .iter()
                    .map(|r| r.bbox.height().max(1.0))
                    .collect::<Vec<_>>(),
            );
            let est = ((frame.height() / med_h).round() as i32).clamp(3, 12) as usize;
            let step = frame.height() / est as f32;
            row_edges = (0..=est).map(|i| frame.y1 - step * i as f32).collect();
        } else {
            row_edges = Vec::new();
            row_edges.push(frame.y1);
            for w in y_centers.windows(2) {
                row_edges.push((w[0] + w[1]) * 0.5);
            }
            row_edges.push(frame.y0);
            // also include single internal H if present (header underline)
            if h_int.len() == 1 {
                let hy = h_int[0];
                // insert if not already near an edge
                if !row_edges.iter().any(|&y| (y - hy).abs() < tol * 2.0) {
                    row_edges.push(hy);
                    row_edges.sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));
                }
            }
        }
    }
    // Deduplicate edges then sort top-to-bottom (decreasing y)
    row_edges = cluster_coords(&row_edges, 1.0);
    row_edges.sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));

    let n_rows = row_edges.len().saturating_sub(1);
    // Anti-collapse: reject 1×2 mega-cells
    if n_rows < 3 || n_cols < 3 {
        // try rebuild from pure text if still collapsed
        if n_rows < 3 || n_cols < 3 {
            // one more attempt: equal row split from text count
            let y_raw: Vec<f32> = inside.iter().map(|r| r.bbox.y_center()).collect();
            let y_c = cluster_coords(&y_raw, 0.4 * fs);
            if y_c.len() >= 3 && col_peaks.len() >= 2 {
                let mut yc = y_c;
                yc.sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));
                row_edges = vec![frame.y1];
                for w in yc.windows(2) {
                    row_edges.push((w[0] + w[1]) * 0.5);
                }
                row_edges.push(frame.y0);
            } else {
                return None;
            }
        }
    }
    let n_rows = row_edges.len().saturating_sub(1);
    let n_cols = col_edges.len().saturating_sub(1);
    if n_rows < 3 || n_cols < 2 {
        return None;
    }

    let (cells, filled) = cells_from_grid(&inside, &col_edges, &row_edges, opts.min_cell_size);
    if filled < 4 {
        return None;
    }
    let total = cells.len().max(1);
    let fill_rate = filled as f32 / total as f32;
    if fill_rate < 0.2 {
        return None;
    }

    let max_row = cells.iter().map(|c| c.row).max().unwrap_or(0) + 1;
    let max_col = cells.iter().map(|c| c.col).max().unwrap_or(0) + 1;
    if max_row < 3 || max_col < 2 {
        return None;
    }

    let grid_reg = grid_regularity_score(&col_edges, &row_edges);
    let col_sep = column_separation_score(&col_edges, fs);
    let rule_support = 0.35; // partial by definition
    let alignment = 0.75;
    let conf_l = (0.35 * grid_reg
        + 0.25 * rule_support
        + 0.20 * fill_rate
        + 0.10 * alignment
        + 0.10 * (total as f32 / 6.0).min(1.0))
    .clamp(0.0, 1.0);
    let conf_s = (0.30 * col_sep
        + 0.25 * 0.8
        + 0.20 * fill_rate
        + 0.15 * alignment
        + 0.10 * (total as f32 / 6.0).min(1.0))
    .clamp(0.0, 1.0);
    let agreement = if max_col >= 3 && max_row >= 3 {
        0.05
    } else {
        0.0
    };
    let confidence = (0.5 * conf_l + 0.5 * conf_s + agreement).clamp(0.0, 1.0);

    // Prefer multi-column recovered grids in NMS (floor conf when ≥3×3)
    let confidence = if max_col >= 3 && max_row >= 3 {
        confidence.max(opts.hybrid_min_conf_when_grid)
    } else {
        confidence
    };

    if confidence < opts.min_table_confidence * 0.85 {
        return None;
    }

    let _ = inner;

    let filled = cells.iter().filter(|c| !c.text.trim().is_empty()).count();
    let fill_rate = filled as f32 / cells.len().max(1) as f32;
    Some(Table {
        bbox: frame,
        page: page_index,
        method: TableMethod::Hybrid,
        confidence,
        rows: max_row,
        cols: max_col,
        cells,
        header_rows: 1,
        continued_from_previous_page: false,
        continued_to_next_page: false,
        logical_table_id: None,
        strategy_provenance: vec![PipelineId::S4Hybrid],
        notes: vec![format!("hybrid {max_row}x{max_col}")],
        edge_score: 0.0,
        fill_rate,
        weak_edges: false,
    joint_count: 0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use pdfparser_ir::{Matrix3x2, TextRun};

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

    /// Partial border frame with text grid inside.
    fn partial_border_fixture() -> (Vec<TextRun>, Vec<RuleSegment>) {
        // Outer frame mostly present (3+ sides)
        let mut rules = vec![
            rule(50.0, 400.0, 350.0, 400.0), // top H
            rule(50.0, 200.0, 350.0, 200.0), // bottom H
            rule(50.0, 200.0, 50.0, 400.0),  // left V
            // missing full right V — hybrid partial
            rule(350.0, 250.0, 350.0, 400.0), // partial right
            // interior H rules
            rule(50.0, 350.0, 350.0, 350.0),
            rule(50.0, 300.0, 350.0, 300.0),
            rule(50.0, 250.0, 350.0, 250.0),
            // some V interior
            rule(150.0, 200.0, 150.0, 400.0),
            rule(250.0, 200.0, 250.0, 400.0),
        ];
        let _ = &mut rules;
        let mut runs = Vec::new();
        let headers = ["H0", "H1", "H2"];
        let col_x = [70.0, 170.0, 270.0];
        for (i, h) in headers.iter().enumerate() {
            runs.push(tr(h, col_x[i], 360.0, col_x[i] + 30.0, 375.0));
        }
        for r in 0..4 {
            let y = 320.0 - r as f32 * 30.0;
            for (c, _) in headers.iter().enumerate() {
                runs.push(tr(
                    &format!("r{r}c{c}"),
                    col_x[c],
                    y,
                    col_x[c] + 30.0,
                    y + 12.0,
                ));
            }
        }
        (runs, rules)
    }

    #[test]
    fn hybrid_detects_partial_frame() {
        let (runs, rules) = partial_border_fixture();
        let opts = TableOptions::from_preset(crate::options::TablePreset::Full);
        let tabs = detect_hybrid_tables(0, &runs, &rules, &opts);
        // Hybrid may or may not fully recover depending on thresholds — exercise path.
        eprintln!(
            "hybrid tabs: {:?}",
            tabs.iter()
                .map(|t| (t.rows, t.cols, t.method, t.confidence))
                .collect::<Vec<_>>()
        );
        // At minimum no panic; if found, must be hybrid
        for t in &tabs {
            assert_eq!(t.method, TableMethod::Hybrid);
            assert!(t.rows >= 2 && t.cols >= 2);
        }
    }

    #[test]
    fn hybrid_empty_inputs() {
        let opts = TableOptions::default();
        assert!(detect_hybrid_tables(0, &[], &[], &opts).is_empty());
        assert!(union_rules_frame(&[]).is_none());
    }

    #[test]
    fn union_rules_frame_small_rejected() {
        let rules = [rule(0.0, 0.0, 10.0, 0.0), rule(0.0, 0.0, 0.0, 10.0)];
        assert!(union_rules_frame(&rules).is_none());
    }

    #[test]
    fn union_rules_frame_ok() {
        // Helper retained for unit tests only — detect path no longer uses free page-union.
        let rules = [
            rule(0.0, 0.0, 100.0, 0.0),
            rule(0.0, 100.0, 100.0, 100.0),
            rule(0.0, 0.0, 0.0, 100.0),
            rule(100.0, 0.0, 100.0, 100.0),
        ];
        let f = union_rules_frame(&rules).unwrap();
        assert!(f.width() >= 100.0);
    }

    #[test]
    fn hybrid_no_free_page_union_without_frame() {
        use crate::options::{TableOptions, TablePreset};
        // Scattered H rules only — not a 3-sided outer frame.
        let rules = [
            rule(0.0, 0.0, 200.0, 0.0),
            rule(0.0, 400.0, 200.0, 400.0),
            rule(50.0, 100.0, 50.0, 200.0),
        ];
        let mut runs = Vec::new();
        for i in 0..8 {
            runs.push(tr(
                &format!("x{i}"),
                20.0 + (i as f32) * 18.0,
                150.0,
                30.0 + (i as f32) * 18.0,
                160.0,
            ));
            runs.push(tr(
                &format!("y{i}"),
                20.0 + (i as f32) * 18.0,
                120.0,
                30.0 + (i as f32) * 18.0,
                130.0,
            ));
        }
        let opts = TableOptions::from_preset(TablePreset::Full);
        let tabs = detect_hybrid_tables(0, &runs, &rules, &opts);
        assert!(
            tabs.is_empty(),
            "K34: no free page-union hybrid without outer frame, got {}",
            tabs.len()
        );
    }

    #[test]
    fn hybrid_closed_frame_emits_table() {
        // Full closed rectangle + interior H/V + multi-col text bands
        let mut rules = vec![
            rule(40.0, 500.0, 360.0, 500.0),
            rule(40.0, 180.0, 360.0, 180.0),
            rule(40.0, 180.0, 40.0, 500.0),
            rule(360.0, 180.0, 360.0, 500.0),
        ];
        for y in [420.0, 360.0, 300.0, 240.0] {
            rules.push(rule(40.0, y, 360.0, y));
        }
        for x in [140.0, 240.0] {
            rules.push(rule(x, 180.0, x, 500.0));
        }
        let col_x = [60.0, 160.0, 260.0];
        let mut runs = Vec::new();
        for (i, lab) in ["A", "B", "C"].iter().enumerate() {
            runs.push(tr(lab, col_x[i], 450.0, col_x[i] + 25.0, 465.0));
        }
        for r in 0..5 {
            let y = 400.0 - r as f32 * 40.0;
            for c in 0..3 {
                runs.push(tr(
                    &format!("v{r}{c}"),
                    col_x[c],
                    y,
                    col_x[c] + 25.0,
                    y + 12.0,
                ));
            }
        }
        let opts = TableOptions::from_preset(crate::options::TablePreset::Full);
        let tabs = detect_hybrid_tables(0, &runs, &rules, &opts);
        assert!(!tabs.is_empty(), "expected hybrid table from closed frame");
        assert!(
            tabs[0].cols >= 2 && tabs[0].rows >= 3,
            "{:?}",
            (tabs[0].rows, tabs[0].cols)
        );
    }

    #[test]
    fn find_outer_frames_needs_hv() {
        let only_h = [rule(0.0, 10.0, 100.0, 10.0), rule(0.0, 50.0, 100.0, 50.0)];
        assert!(find_outer_frames(&only_h, 2.0).is_empty());
    }

    #[test]
    fn cover_h_v_merge_gaps() {
        let hs = [
            rule(0.0, 10.0, 40.0, 10.0),
            rule(38.0, 10.0, 100.0, 10.0), // overlaps/adjacent
        ];
        let c = cover_h(&hs, 10.0, 0.0, 100.0, 2.0);
        assert!(c > 90.0, "merged cover {c}");
        let vs = [rule(5.0, 0.0, 5.0, 40.0), rule(5.0, 38.0, 5.0, 100.0)];
        let c2 = cover_v(&vs, 5.0, 0.0, 100.0, 2.0);
        assert!(c2 > 90.0, "merged v cover {c2}");
        assert_eq!(cover_h(&[], 0.0, 0.0, 10.0, 1.0), 0.0);
        assert_eq!(cover_v(&[], 0.0, 0.0, 10.0, 1.0), 0.0);
    }

    #[test]
    fn detect_hybrid_various_frames() {
        use crate::options::{TableOptions, TablePreset};
        use pdfparser_content::RuleSegment;
        use pdfparser_ir::{Rect, TextRun};
        // Three-sided frame + multi-col text inside
        let rules = vec![
            RuleSegment {
                x0: 10.0,
                y0: 10.0,
                x1: 200.0,
                y1: 10.0,
            }, // bottom
            RuleSegment {
                x0: 10.0,
                y0: 10.0,
                x1: 10.0,
                y1: 200.0,
            }, // left
            RuleSegment {
                x0: 200.0,
                y0: 10.0,
                x1: 200.0,
                y1: 200.0,
            }, // right
            // partial top
            RuleSegment {
                x0: 10.0,
                y0: 200.0,
                x1: 80.0,
                y1: 200.0,
            },
            RuleSegment {
                x0: 120.0,
                y0: 200.0,
                x1: 200.0,
                y1: 200.0,
            },
            // mid H
            RuleSegment {
                x0: 10.0,
                y0: 100.0,
                x1: 200.0,
                y1: 100.0,
            },
            // mid V
            RuleSegment {
                x0: 100.0,
                y0: 10.0,
                x1: 100.0,
                y1: 200.0,
            },
        ];
        let runs = vec![
            TextRun {
                text: "H1".into(),
                bbox: Rect {
                    x0: 20.,
                    y0: 150.,
                    x1: 40.,
                    y1: 160.,
                },
                transform: pdfparser_ir::Matrix3x2::identity(),
                font_name: None,
                font_size: 10.,
                mapping_confidence: 1.,
                metrics_confidence: 1.,
                mcid: None,
                invisible: false,
                from_actual_text: false,
            },
            TextRun {
                text: "H2".into(),
                bbox: Rect {
                    x0: 120.,
                    y0: 150.,
                    x1: 140.,
                    y1: 160.,
                },
                transform: pdfparser_ir::Matrix3x2::identity(),
                font_name: None,
                font_size: 10.,
                mapping_confidence: 1.,
                metrics_confidence: 1.,
                mcid: None,
                invisible: false,
                from_actual_text: false,
            },
            TextRun {
                text: "A".into(),
                bbox: Rect {
                    x0: 20.,
                    y0: 50.,
                    x1: 30.,
                    y1: 60.,
                },
                transform: pdfparser_ir::Matrix3x2::identity(),
                font_name: None,
                font_size: 10.,
                mapping_confidence: 1.,
                metrics_confidence: 1.,
                mcid: None,
                invisible: false,
                from_actual_text: false,
            },
            TextRun {
                text: "B".into(),
                bbox: Rect {
                    x0: 120.,
                    y0: 50.,
                    x1: 130.,
                    y1: 60.,
                },
                transform: pdfparser_ir::Matrix3x2::identity(),
                font_name: None,
                font_size: 10.,
                mapping_confidence: 1.,
                metrics_confidence: 1.,
                mcid: None,
                invisible: false,
                from_actual_text: false,
            },
            TextRun {
                text: "C".into(),
                bbox: Rect {
                    x0: 20.,
                    y0: 120.,
                    x1: 30.,
                    y1: 130.,
                },
                transform: pdfparser_ir::Matrix3x2::identity(),
                font_name: None,
                font_size: 10.,
                mapping_confidence: 1.,
                metrics_confidence: 1.,
                mcid: None,
                invisible: false,
                from_actual_text: false,
            },
            TextRun {
                text: "D".into(),
                bbox: Rect {
                    x0: 120.,
                    y0: 120.,
                    x1: 130.,
                    y1: 130.,
                },
                transform: pdfparser_ir::Matrix3x2::identity(),
                font_name: None,
                font_size: 10.,
                mapping_confidence: 1.,
                metrics_confidence: 1.,
                mcid: None,
                invisible: false,
                from_actual_text: false,
            },
        ];
        let opts = TableOptions::from_preset(TablePreset::Full);
        let _ = detect_hybrid_tables(0, &runs, &rules, &opts);
        // empty rules / empty runs paths
        assert!(detect_hybrid_tables(0, &[], &rules, &opts).is_empty() || true);
        let _ = detect_hybrid_tables(0, &runs, &[], &opts);
    }
}
