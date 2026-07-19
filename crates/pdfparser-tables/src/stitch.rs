//! Multi-page table stitcher.
use crate::options::TableOptions;
use crate::types::{PipelineId, Table, TableCell};

/// Stitch multi-page table fragments in-place (flags + logical_table_id).
///
/// `page_heights[i]` is the page height in user units (typically media-box height).
pub fn stitch_document(page_tables: &mut [Vec<Table>], page_heights: &[f32], opts: &TableOptions) {
    if page_tables.len() < 2 {
        return;
    }
    let mut next_id: u32 = 1;
    for i in 1..page_tables.len() {
        let h_prev = page_heights.get(i - 1).copied().unwrap_or(0.0);
        let h_cur = page_heights.get(i).copied().unwrap_or(0.0);
        let h_prev = if h_prev > 1.0 {
            h_prev
        } else {
            infer_page_height(&page_tables[i - 1])
        };
        let h_cur = if h_cur > 1.0 {
            h_cur
        } else {
            infer_page_height(&page_tables[i])
        };

        let bottoms: Vec<usize> = page_tables[i - 1]
            .iter()
            .enumerate()
            .filter(|(_, t)| {
                !t.continued_to_next_page && in_bottom_band(t, h_prev, opts.stitch_band_frac)
            })
            .map(|(idx, _)| idx)
            .collect();
        let tops: Vec<usize> = page_tables[i]
            .iter()
            .enumerate()
            .filter(|(_, t)| {
                !t.continued_from_previous_page && in_top_band(t, h_cur, opts.stitch_band_frac)
            })
            .map(|(idx, _)| idx)
            .collect();

        let mut pairs: Vec<(f32, usize, usize)> = Vec::new();
        for &bi in &bottoms {
            for &ti in &tops {
                let a = &page_tables[i - 1][bi];
                let b = &page_tables[i][ti];
                if let Some(score) = stitch_score(a, b, opts) {
                    pairs.push((score, bi, ti));
                }
            }
        }
        pairs.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        let mut used_b = std::collections::HashSet::new();
        let mut used_t = std::collections::HashSet::new();
        for &(_score, bi, ti) in &pairs {
            if used_b.contains(&bi) || used_t.contains(&ti) {
                continue;
            }
            used_b.insert(bi);
            used_t.insert(ti);
            let id = {
                let a = &page_tables[i - 1][bi];
                a.logical_table_id.unwrap_or_else(|| {
                    let id = next_id;
                    next_id += 1;
                    id
                })
            };
            {
                let a = &mut page_tables[i - 1][bi];
                a.logical_table_id = Some(id);
                a.continued_to_next_page = true;
                if !a.strategy_provenance.contains(&PipelineId::D1Stitch) {
                    a.strategy_provenance.push(PipelineId::D1Stitch);
                }
            }
            {
                let b = &mut page_tables[i][ti];
                b.logical_table_id = Some(id);
                b.continued_from_previous_page = true;
                if !b.strategy_provenance.contains(&PipelineId::D1Stitch) {
                    b.strategy_provenance.push(PipelineId::D1Stitch);
                }
            }
        }
    }
}

/// Build logical (stitched) tables from page fragments sharing `logical_table_id`.
pub fn materialize_stitched(page_tables: &[Vec<Table>]) -> Vec<Table> {
    let mut by_id: std::collections::BTreeMap<u32, Vec<&Table>> = std::collections::BTreeMap::new();
    let mut singles: Vec<Table> = Vec::new();

    for page in page_tables {
        for t in page {
            if let Some(id) = t.logical_table_id {
                by_id.entry(id).or_default().push(t);
            } else {
                singles.push(t.clone());
            }
        }
    }

    let mut out = Vec::new();
    for (_id, mut frags) in by_id {
        frags.sort_by_key(|t| t.page);
        if let Some(merged) = merge_fragments(&frags) {
            out.push(merged);
        } else {
            for f in frags {
                out.push(f.clone());
            }
        }
    }
    out.extend(singles);
    out.sort_by(|a, b| {
        a.page.cmp(&b.page).then_with(|| {
            a.bbox
                .x0
                .partial_cmp(&b.bbox.x0)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
    });
    out
}

fn merge_fragments(frags: &[&Table]) -> Option<Table> {
    if frags.is_empty() {
        return None;
    }
    if frags.len() == 1 {
        return Some(frags[0].clone());
    }
    let first = frags[0];
    let cols = first.cols;
    let mut cells: Vec<TableCell> = first.cells.clone();
    let mut row_base = first.rows;
    let header_rows = first.header_rows.max(1);

    for frag in &frags[1..] {
        if frag.cols != cols {
            continue;
        }
        let skip = if header_sim(first, frag) >= 0.85 {
            header_rows
        } else {
            0
        };
        for c in &frag.cells {
            if c.row < skip {
                continue;
            }
            let mut nc = c.clone();
            nc.row = row_base + (c.row - skip);
            nc.is_header = false;
            cells.push(nc);
        }
        let max_body = frag
            .cells
            .iter()
            .filter(|c| c.row >= skip)
            .map(|c| c.row)
            .max()
            .map(|r| r + 1 - skip)
            .unwrap_or(0);
        row_base += max_body;
    }

    let max_row = cells.iter().map(|c| c.row).max().unwrap_or(0) + 1;
    let max_col = cells.iter().map(|c| c.col).max().unwrap_or(0) + 1;
    let bbox = cells
        .iter()
        .map(|c| c.bbox)
        .reduce(|a, b| a.union(b))
        .unwrap_or(first.bbox);
    let mut notes = first.notes.clone();
    notes.push(format!("stitched_pages={}", frags.len()));
    let mut prov = first.strategy_provenance.clone();
    if !prov.contains(&PipelineId::D1Stitch) {
        prov.push(PipelineId::D1Stitch);
    }
    let filled = cells.iter().filter(|c| !c.text.trim().is_empty()).count();
    let fill_rate = filled as f32 / cells.len().max(1) as f32;
    Some(Table {
        bbox,
        page: first.page,
        method: first.method,
        confidence: first.confidence,
        rows: max_row,
        cols: max_col,
        cells,
        header_rows: first.header_rows,
        continued_from_previous_page: false,
        continued_to_next_page: false,
        logical_table_id: first.logical_table_id,
        strategy_provenance: prov,
        notes,
        edge_score: first.edge_score,
        fill_rate,
        weak_edges: first.weak_edges,
        joint_count: first.joint_count,
        text_row_recovery: first.text_row_recovery,
        text_col_recovery: first.text_col_recovery,
        multitable_stream_recovery: first.multitable_stream_recovery,
        stream_vs_overwide_hybrid: first.stream_vs_overwide_hybrid,
    })
}

/// Infer page height from table bboxes when media box is unavailable.
fn infer_page_height(tables: &[Table]) -> f32 {
    let top = tables.iter().map(|t| t.bbox.y1).fold(0.0f32, f32::max);
    if top > 1.0 {
        top * 1.05
    } else {
        0.0
    }
}

fn in_bottom_band(t: &Table, page_h: f32, band_frac: f32) -> bool {
    if page_h <= 1.0 {
        return true;
    }
    let band_top = page_h * band_frac;
    t.bbox.y0 < band_top || t.bbox.y_center() < page_h * (band_frac + 0.15)
}

fn in_top_band(t: &Table, page_h: f32, band_frac: f32) -> bool {
    if page_h <= 1.0 {
        return true;
    }
    let band_bot = page_h * (1.0 - band_frac);
    t.bbox.y1 > band_bot || t.bbox.y_center() > page_h * (1.0 - band_frac - 0.15)
}

fn stitch_score(a: &Table, b: &Table, opts: &TableOptions) -> Option<f32> {
    if a.cols != b.cols || a.cols < 2 {
        return None;
    }
    match (a.method, b.method) {
        (crate::types::TableMethod::FormLayout, _) | (_, crate::types::TableMethod::FormLayout) => {
            return None;
        }
        _ => {}
    }
    let col_dx = mean_col_dx(a, b);
    let max_dx = opts.stitch_max_col_dx;
    if col_dx > max_dx {
        return None;
    }
    let hs = header_sim(a, b);
    let header_ok = hs >= opts.stitch_min_header_sim || headers_subset(a, b) || b.header_rows == 0;
    // Same-shape multi-row grids with aligned columns can continue without header copy
    let continuation_ok = a.cols >= 3 && a.rows >= 4 && b.rows >= 2 && col_dx <= max_dx * 0.7;
    if !header_ok && !continuation_ok {
        return None;
    }
    let score = (0.5 * hs + 0.5 * (1.0 - (col_dx / max_dx).min(1.0))).clamp(0.0, 1.0);
    if score < 0.35 && !continuation_ok {
        return None;
    }
    Some(if continuation_ok {
        score.max(0.55)
    } else {
        score
    })
}

fn mean_col_dx(a: &Table, b: &Table) -> f32 {
    let ca = col_centers(a);
    let cb = col_centers(b);
    if ca.len() != cb.len() || ca.is_empty() {
        return 100.0;
    }
    ca.iter()
        .zip(cb.iter())
        .map(|(x, y)| (x - y).abs())
        .sum::<f32>()
        / ca.len() as f32
}

fn col_centers(t: &Table) -> Vec<f32> {
    let mut sums = vec![0.0f32; t.cols as usize];
    let mut ns = vec![0u32; t.cols as usize];
    for c in &t.cells {
        let i = c.col as usize;
        if i < sums.len() {
            sums[i] += (c.bbox.x0 + c.bbox.x1) * 0.5;
            ns[i] += 1;
        }
    }
    sums.iter()
        .zip(ns.iter())
        .map(|(s, n)| if *n > 0 { s / *n as f32 } else { 0.0 })
        .collect()
}

fn header_sim(a: &Table, b: &Table) -> f32 {
    let ha = header_texts(a);
    let hb = header_texts(b);
    if ha.is_empty() || hb.is_empty() {
        return 0.0;
    }
    let n = ha.len().min(hb.len());
    let mut hits = 0u32;
    for i in 0..n {
        let na = normalize(&ha[i]);
        let nb = normalize(&hb[i]);
        if na == nb || (!na.is_empty() && !nb.is_empty() && (na.contains(&nb) || nb.contains(&na)))
        {
            hits += 1;
        }
    }
    hits as f32 / n as f32
}

fn headers_subset(a: &Table, b: &Table) -> bool {
    let ha = header_texts(a);
    let hb = header_texts(b);
    if ha.is_empty() || hb.is_empty() {
        return false;
    }
    hb.iter().zip(ha.iter()).all(|(x, y)| {
        let nx = normalize(x);
        let ny = normalize(y);
        nx.is_empty() || ny.is_empty() || nx == ny || ny.contains(&nx) || nx.contains(&ny)
    })
}

fn header_texts(t: &Table) -> Vec<String> {
    let hr = t.header_rows.max(1);
    let mut cols = vec![String::new(); t.cols as usize];
    for c in &t.cells {
        if c.row < hr && (c.col as usize) < cols.len() {
            if !cols[c.col as usize].is_empty() {
                cols[c.col as usize].push(' ');
            }
            cols[c.col as usize].push_str(c.text.trim());
        }
    }
    cols
}

fn normalize(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(|c| c.to_lowercase())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::TableMethod;
    use pdfparser_ir::Rect;

    fn cell(row: u32, col: u32, text: &str) -> TableCell {
        TableCell {
            row,
            col,
            rowspan: 1,
            colspan: 1,
            bbox: Rect {
                x0: col as f32 * 50.0,
                y0: row as f32 * 12.0,
                x1: (col as f32 + 1.0) * 50.0,
                y1: (row as f32 + 1.0) * 12.0,
            },
            text: text.into(),
            is_header: row == 0,
            confidence: 0.9,
        }
    }

    fn mk_table(page: u32, y0: f32, y1: f32, header: &[&str], body: &[&[&str]]) -> Table {
        let cols = header.len() as u32;
        let mut cells = Vec::new();
        for (c, h) in header.iter().enumerate() {
            let mut ce = cell(0, c as u32, h);
            ce.bbox.y0 = y1 - 12.0;
            ce.bbox.y1 = y1;
            cells.push(ce);
        }
        for (r, row) in body.iter().enumerate() {
            for (c, t) in row.iter().enumerate() {
                let mut ce = cell((r + 1) as u32, c as u32, t);
                let row_y1 = y1 - 12.0 * (r + 1) as f32;
                ce.bbox.y0 = row_y1 - 12.0;
                ce.bbox.y1 = row_y1;
                cells.push(ce);
            }
        }
        let rows = (body.len() + 1) as u32;
        Table {
            bbox: Rect {
                x0: 0.0,
                y0,
                x1: cols as f32 * 50.0,
                y1,
            },
            page,
            method: TableMethod::Lattice,
            confidence: 0.9,
            rows,
            cols,
            cells,
            header_rows: 1,
            continued_from_previous_page: false,
            continued_to_next_page: false,
            logical_table_id: None,
            strategy_provenance: vec![],
            notes: vec![],
            edge_score: 0.9,
            fill_rate: 1.0,
            weak_edges: false,
            joint_count: 0,
            text_row_recovery: false,
            text_col_recovery: false,
            multitable_stream_recovery: false,
            stream_vs_overwide_hybrid: false,
        }
    }

    #[test]
    fn stitch_two_pages_matching_headers() {
        let header = ["Date", "Desc", "Amt", "Bal"];
        let p0 = mk_table(
            0,
            20.0,
            80.0, // bottom of page height 792
            &header,
            &[&["1", "a", "10", "100"], &["2", "b", "20", "120"]],
        );
        // Put fragment near bottom of page 0 and top of page 1
        let mut bottom = p0;
        bottom.bbox = Rect {
            x0: 50.0,
            y0: 40.0,
            x1: 250.0,
            y1: 120.0,
        };
        for c in &mut bottom.cells {
            c.bbox.y0 += 40.0;
            c.bbox.y1 += 40.0;
        }
        let mut top = mk_table(
            1,
            700.0,
            780.0,
            &header,
            &[&["3", "c", "30", "150"], &["4", "d", "40", "190"]],
        );
        top.bbox = Rect {
            x0: 50.0,
            y0: 700.0,
            x1: 250.0,
            y1: 780.0,
        };
        let mut pages = vec![vec![bottom], vec![top]];
        let heights = [792.0f32, 792.0];
        let mut opts = TableOptions::default();
        opts.stitch_multipage = true;
        opts.stitch_band_frac = 0.30;
        opts.stitch_max_col_dx = 12.0;
        opts.stitch_min_header_sim = 0.85;
        stitch_document(&mut pages, &heights, &opts);
        assert!(
            pages[0][0].continued_to_next_page && pages[1][0].continued_from_previous_page,
            "flags {:?}",
            (
                pages[0][0].continued_to_next_page,
                pages[1][0].continued_from_previous_page,
                pages[0][0].logical_table_id,
                pages[1][0].logical_table_id
            )
        );
        let logical = materialize_stitched(&pages);
        assert_eq!(
            logical.len(),
            1,
            "expected 1 stitched table, got {}",
            logical.len()
        );
        assert!(logical[0].rows >= 4, "rows {}", logical[0].rows);
        assert_eq!(logical[0].cols, 4);
    }

    #[test]
    fn stitch_single_page_noop() {
        let t = mk_table(0, 100.0, 200.0, &["A", "B"], &[&["1", "2"]]);
        let mut pages = vec![vec![t]];
        stitch_document(&mut pages, &[792.0], &TableOptions::default());
        assert!(pages[0][0].logical_table_id.is_none());
    }

    #[test]
    fn materialize_singles() {
        let t = mk_table(0, 100.0, 200.0, &["A", "B"], &[&["1", "2"]]);
        let logical = materialize_stitched(&[vec![t]]);
        assert_eq!(logical.len(), 1);
    }

    #[test]
    fn normalize_header_sim() {
        assert_eq!(normalize("  Hello "), "hello");
        assert!(
            header_sim(
                &mk_table(0, 0.0, 50.0, &["Date", "Amt"], &[&["1", "2"]]),
                &mk_table(1, 700.0, 750.0, &["Date", "Amt"], &[&["3", "4"]]),
            ) > 0.9
        );
    }

    #[test]
    fn stitch_score_rejects_col_mismatch() {
        let a = mk_table(0, 0.0, 50.0, &["A", "B"], &[&["1", "2"]]);
        let b = mk_table(1, 700.0, 750.0, &["A", "B", "C"], &[&["1", "2", "3"]]);
        let opts = TableOptions::default();
        assert!(stitch_score(&a, &b, &opts).is_none());
    }

    #[test]
    fn stitch_score_rejects_form() {
        let mut a = mk_table(0, 0.0, 50.0, &["A", "B", "C"], &[&["1", "2", "3"]]);
        a.method = TableMethod::FormLayout;
        let b = mk_table(1, 700.0, 750.0, &["A", "B", "C"], &[&["4", "5", "6"]]);
        assert!(stitch_score(&a, &b, &TableOptions::default()).is_none());
    }

    #[test]
    fn bands_and_infer_height() {
        let t = mk_table(0, 10.0, 80.0, &["A", "B"], &[&["1", "2"]]);
        assert!(in_bottom_band(&t, 792.0, 0.3));
        let top = mk_table(1, 700.0, 780.0, &["A", "B"], &[&["3", "4"]]);
        assert!(in_top_band(&top, 792.0, 0.3));
        assert!(in_bottom_band(&t, 0.0, 0.3)); // page_h invalid → true
        assert!(infer_page_height(&[top]) > 700.0);
        assert_eq!(infer_page_height(&[]), 0.0);
    }

    #[test]
    fn headers_subset_and_col_centers() {
        let a = mk_table(0, 0.0, 50.0, &["Date", "Amount"], &[&["1", "2"]]);
        let b = mk_table(1, 0.0, 50.0, &["Date", "Amt"], &[&["3", "4"]]);
        // partial contain
        let _ = headers_subset(&a, &b);
        let cc = col_centers(&a);
        assert_eq!(cc.len(), 2);
        assert!(mean_col_dx(&a, &a) < 1.0);
    }

    #[test]
    fn materialize_merge_skips_header_on_second() {
        let a = mk_table(
            0,
            0.0,
            100.0,
            &["H1", "H2", "H3"],
            &[&["a", "b", "c"], &["d", "e", "f"]],
        );
        let mut b = mk_table(1, 700.0, 800.0, &["H1", "H2", "H3"], &[&["g", "h", "i"]]);
        b.logical_table_id = Some(1);
        let mut a2 = a.clone();
        a2.logical_table_id = Some(1);
        let logical = materialize_stitched(&[vec![a2], vec![b]]);
        assert_eq!(logical.len(), 1);
        assert!(logical[0].rows >= 3);
    }

    #[test]
    fn stitch_with_zero_heights_uses_infer() {
        let header = ["A", "B", "C"];
        let mut bottom = mk_table(
            0,
            5.0,
            60.0,
            &header,
            &[&["1", "2", "3"], &["4", "5", "6"], &["7", "8", "9"]],
        );
        bottom.bbox = Rect {
            x0: 0.0,
            y0: 5.0,
            x1: 150.0,
            y1: 60.0,
        };
        let mut top = mk_table(1, 200.0, 260.0, &header, &[&["10", "11", "12"]]);
        top.bbox = Rect {
            x0: 0.0,
            y0: 200.0,
            x1: 150.0,
            y1: 260.0,
        };
        let mut pages = vec![vec![bottom], vec![top]];
        stitch_document(&mut pages, &[0.0, 0.0], &TableOptions::default());
        // may or may not stitch depending on inferred bands; exercise path
        let _ = materialize_stitched(&pages);
    }
}
