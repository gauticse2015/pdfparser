//! D1 multi-page table stitcher.
use crate::options::TableOptions;
use crate::types::{PipelineId, Table, TableCell};

/// Stitch multi-page table fragments in-place (sets flags + logical_table_id).
pub fn stitch_document(page_tables: &mut [Vec<Table>], _opts: &TableOptions) {
    if page_tables.len() < 2 {
        return;
    }
    let mut next_id: u32 = 1;
    for i in 1..page_tables.len() {
        // Collect candidate indices (can't hold mut refs across pages easily)
        let bottoms: Vec<usize> = page_tables[i - 1]
            .iter()
            .enumerate()
            .filter(|(_, t)| {
                !t.continued_to_next_page && in_bottom_band(t, page_height_hint(t))
            })
            .map(|(idx, _)| idx)
            .collect();
        let tops: Vec<usize> = page_tables[i]
            .iter()
            .enumerate()
            .filter(|(_, t)| {
                !t.continued_from_previous_page && in_top_band(t, page_height_hint(t))
            })
            .map(|(idx, _)| idx)
            .collect();

        let mut pairs: Vec<(f32, usize, usize)> = Vec::new();
        for &bi in &bottoms {
            for &ti in &tops {
                let a = &page_tables[i - 1][bi];
                let b = &page_tables[i][ti];
                if let Some(score) = stitch_score(a, b) {
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
                if let Some(id) = a.logical_table_id {
                    id
                } else {
                    let id = next_id;
                    next_id += 1;
                    id
                }
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

/// Build logical (stitched) tables for scoreboard adapter.
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
    // stable order by page then x
    out.sort_by(|a, b| {
        a.page
            .cmp(&b.page)
            .then_with(|| {
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
        // Drop repeated header rows when similar
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
    })
}

fn page_height_hint(t: &Table) -> f32 {
    // Approximate letter height if bbox is mid-page; use 792 as default US Letter
    let top = t.bbox.y1;
    if top > 700.0 {
        top + 50.0
    } else {
        792.0
    }
}

fn in_bottom_band(t: &Table, page_h: f32) -> bool {
    // bottom 30% of page (PDF y-up: low y)
    let band_top = page_h * 0.30;
    t.bbox.y0 < band_top || t.bbox.y_center() < page_h * 0.45
}

fn in_top_band(t: &Table, page_h: f32) -> bool {
    let band_bot = page_h * 0.70;
    t.bbox.y1 > band_bot || t.bbox.y_center() > page_h * 0.55
}

fn stitch_score(a: &Table, b: &Table) -> Option<f32> {
    if a.cols != b.cols || a.cols < 2 {
        return None;
    }
    // methods compatible
    match (a.method, b.method) {
        (crate::types::TableMethod::FormLayout, _) => return None,
        (_, crate::types::TableMethod::FormLayout) => return None,
        _ => {}
    }
    let col_dx = mean_col_dx(a, b);
    if col_dx > 3.0 {
        // allow slightly looser for bank statements
        if col_dx > 12.0 {
            return None;
        }
    }
    let hs = header_sim(a, b);
    // Match if headers similar OR b looks like continuation (header matches or body-only)
    let header_ok = hs >= 0.85 || headers_subset(a, b) || (b.header_rows == 0);
    // Bank ledgers: same col count + alignment is enough when both have many rows
    let ledger_ok = a.cols >= 4 && a.rows >= 5 && b.rows >= 3 && col_dx <= 8.0;
    if !header_ok && !ledger_ok {
        return None;
    }
    let score = (0.5 * hs + 0.5 * (1.0 - (col_dx / 12.0).min(1.0))).clamp(0.0, 1.0);
    if score < 0.35 && !ledger_ok {
        return None;
    }
    Some(score.max(if ledger_ok { 0.6 } else { score }))
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
        if na == nb
            || (!na.is_empty()
                && !nb.is_empty()
                && (na.contains(&nb) || nb.contains(&na)))
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
