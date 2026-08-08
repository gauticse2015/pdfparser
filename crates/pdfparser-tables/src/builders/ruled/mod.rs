//! Ruled table builder (S2 lattice): multi-region ruled grids + text assign.
#![allow(clippy::needless_range_loop)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::collapsible_if)]

mod cells;
mod emit;
mod joints;

pub use cells::trim_empty_border_rows_cols;

use crate::geom::{assign_runs_exclusive, bbox_of_cells, cluster_coords, grid_regularity_score};
use crate::options::TableOptions;
use crate::types::{PipelineId, Table, TableMethod};
use cells::{redistribute_row_tokens, strip_trailing_footer_totals};
use emit::{emit_cells_dense, merge_spans_dense, RawCell};
use joints::{
    cluster_line_components, coalesce_h, coalesce_v, edge_flags, filter_joint_supported_coords,
    segments_cross_hv, suppress_text_baseline_h_rules, HSeg, VSeg,
};
use pdfparser_content::RuleSegment;
use pdfparser_ir::{Rect, TextRun};

use super::densify::{
    collapse_overdense_h_from_text, collapse_sparse_interior_columns, collapse_thin_gaps,
    densify_x_from_text_cols, densify_y_from_text_bands, expand_xs_exterior_text_cols,
    multi_col_band_centers, should_run_text_densify,
};

/// Detect ruled (lattice) tables on a page (may emit multiple).
pub fn detect_ruled_tables(
    page_index: u32,
    runs: &[TextRun],
    rules: &[RuleSegment],
    opts: &TableOptions,
    raster_pages: &[crate::RasterPage],
) -> Vec<Table> {
    let tol = opts.advanced.line_snap_tol;
    let min_cell = opts.advanced.min_cell_size;
    let min_seg = opts.advanced.lattice_min_seg_len;
    let joint_gap = opts.advanced.lattice_joint_gap;
    let min_joints = opts.advanced.lattice_min_joints.max(1) as usize;

    // Count axis-aligned vector rules after min_seg (ignore short junk).
    let vector_hv_count = rules
        .iter()
        .filter(|r| r.len() >= min_seg && (r.is_horizontal(tol) || r.is_vertical(tol)))
        .count();

    // Merge raster-derived rules when enabled (image-painted / scanned grids).
    // Production morph already applies joint-graph + regularity gates so charts
    // and deco images do not inject phantom lattices.
    let mut rule_buf: Vec<RuleSegment> = rules.to_vec();
    let mut used_raster = false;
    if opts.advanced.raster_line_detect && !raster_pages.is_empty() {
        // K28: stamp existing vector rules into raster ink before morph (combined).
        use crate::raster::{config_for_raster_page, merge_rules, rules_from_raster_combined};
        let mut raster_rules = Vec::new();
        for rp in raster_pages {
            // Skip tiny icons / logos — not table images.
            if rp.width < 40 || rp.height < 40 {
                continue;
            }
            let cfg = config_for_raster_page(
                rp,
                opts.advanced.raster_adaptive_radius,
                opts.advanced.raster_adaptive_bias,
                opts.advanced.raster_min_kernel,
                opts.advanced.raster_min_seg_px,
                opts.advanced.raster_merge_gap_px,
                opts.advanced.raster_pos_snap_px,
            );
            // Combined path: vector stamp ∪ morph (PR4c production wire).
            // Contour seeds for router region ownership are built in the
            // orchestrator finalize path — do not recompute them here.
            raster_rules.extend(rules_from_raster_combined(rp, rules, &cfg));
        }
        if !raster_rules.is_empty() {
            used_raster = true;
            rule_buf = merge_rules(&rule_buf, &raster_rules, tol.max(1.0));
        }
    }
    // K29: drop H rules that track many text baselines (full-page / raster false underlines).
    if used_raster && !runs.is_empty() {
        rule_buf = suppress_text_baseline_h_rules(&rule_buf, runs, tol);
    }
    let rules = rule_buf.as_slice();
    // Pure image-table pages (few axis-aligned vector rules) may keep empty cells.
    // Mixed pages with a real vector lattice keep normal fill gates.
    let raster_primary = used_raster && vector_hv_count < 4;

    let mut h_segs: Vec<HSeg> = Vec::new();
    let mut v_segs: Vec<VSeg> = Vec::new();
    for r in rules {
        if r.len() < min_seg {
            continue;
        }
        if r.is_horizontal(tol) {
            let y = (r.y0 + r.y1) * 0.5;
            h_segs.push(HSeg {
                y,
                x0: r.x0.min(r.x1),
                x1: r.x0.max(r.x1),
            });
        } else if r.is_vertical(tol) {
            let x = (r.x0 + r.x1) * 0.5;
            v_segs.push(VSeg {
                x,
                y0: r.y0.min(r.y1),
                y1: r.y0.max(r.y1),
            });
        }
    }

    if h_segs.len() < 2 || v_segs.len() < 2 {
        return Vec::new();
    }

    h_segs = coalesce_h(&h_segs, tol);
    v_segs = coalesce_v(&v_segs, tol);

    // Single joint model: expand segments by joint_gap; pass snap tol separately.
    let clusters = cluster_line_components(&h_segs, &v_segs, tol, joint_gap, min_joints);
    let multi_component = clusters.len() > 1;

    let mut tables = Vec::new();
    for (hi, vi, joints) in &clusters {
        if let Some(mut t) = table_from_component(
            page_index,
            runs,
            &h_segs,
            &v_segs,
            hi,
            vi,
            joints,
            opts,
            min_cell,
            tol,
            used_raster,
            raster_primary,
        ) {
            if used_raster && (t.fill_rate < 0.10 || raster_primary) {
                t.strategy_provenance.push(PipelineId::S6RasterLines);
                t.notes.push("raster_lines".into());
            }
            tables.push(t);
        }
    }

    // Global snap only when we did not already see multiple joint-rich components.
    // Multi-CC failure must not re-fuse into a page-wide mega-grid.
    if tables.is_empty() && !multi_component {
        if let Some(mut t) = table_from_global_snap(
            page_index,
            runs,
            &h_segs,
            &v_segs,
            opts,
            min_cell,
            tol,
            used_raster,
            raster_primary,
        ) {
            if used_raster && (t.fill_rate < 0.10 || raster_primary) {
                t.strategy_provenance.push(PipelineId::S6RasterLines);
                t.notes.push("raster_lines".into());
            }
            tables.push(t);
        }
    }

    for t in &mut tables {
        if !used_raster || t.fill_rate > 0.05 {
            strip_trailing_footer_totals(t);
        }
        // Drop fully empty leading/trailing rows and empty outer columns that
        // are not part of the data span (decorative frame chrome).
        // Never trim pure image lattices: all cells are text-empty, so border
        // trim would collapse the schema to nothing.
        if !(used_raster && t.fill_rate < 0.05) {
            trim_empty_border_rows_cols(t);
        }
    }

    tables.sort_by(|a, b| {
        b.bbox
            .y1
            .partial_cmp(&a.bbox.y1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| {
                a.bbox
                    .x0
                    .partial_cmp(&b.bbox.x0)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
    });
    tables
}

fn table_from_component(
    page_index: u32,
    runs: &[TextRun],
    h_segs: &[HSeg],
    v_segs: &[VSeg],
    h_idx: &[usize],
    v_idx: &[usize],
    joints: &[(f32, f32)],
    opts: &TableOptions,
    min_cell: f32,
    tol: f32,
    used_raster: bool,
    raster_primary: bool,
) -> Option<Table> {
    // Anchors: joints + line coordinates of segments in this component only.
    // Do NOT inject H endpoints into xs or V endpoints into ys.
    let mut xs: Vec<f32> = joints.iter().map(|p| p.0).collect();
    let mut ys: Vec<f32> = joints.iter().map(|p| p.1).collect();
    for &i in v_idx {
        xs.push(v_segs[i].x);
    }
    for &i in h_idx {
        ys.push(h_segs[i].y);
    }

    xs = cluster_coords(&xs, tol);
    ys = cluster_coords(&ys, tol);
    if xs.len() < 3 || ys.len() < 3 {
        return None;
    }

    // Vertical lines (columns): strict joint count + span — drops short phantom ticks.
    // Horizontal lines (rows): joint count only (or looser span) — multi-level headers often
    // have short H rules only under sub-columns (Act/Bud), which must be kept for structure.
    let min_jpl = opts.advanced.lattice_min_joints_per_line.max(1) as usize;
    let tun = &opts.advanced.tuning;
    // Raster lines often have incomplete joint spans at image edges — use looser span.
    let (v_span, h_span) = if used_raster {
        (
            tun.lattice_raster_v_span_frac,
            tun.lattice_raster_h_span_frac,
        )
    } else {
        (tun.lattice_v_span_frac, tun.lattice_h_span_frac)
    };
    xs = filter_joint_supported_coords(&xs, joints, tol, true, min_jpl, v_span);
    ys = filter_joint_supported_coords(&ys, joints, tol, false, min_jpl, h_span);
    // Recover long H rules that joint-span filter dropped (partial joints on
    // dashed/short-tick corners). Only when joint-filtered H is clearly
    // under-dense vs physical long H segments — avoids re-introducing
    // double-rules on already-dense grids.
    {
        let x_lo = xs.iter().copied().fold(f32::INFINITY, f32::min);
        let x_hi = xs.iter().copied().fold(f32::NEG_INFINITY, f32::max);
        let width = (x_hi - x_lo).abs().max(1.0);
        let long_h: Vec<f32> = h_idx
            .iter()
            .map(|&i| h_segs[i])
            .filter(|h| (h.x1 - h.x0).abs() >= width * tun.lattice_long_h_width_frac)
            .map(|h| h.y)
            .collect();
        let long_clustered = cluster_coords(&long_h, tol);
        if long_clustered.len() as f32 >= ys.len() as f32 * tun.lattice_long_h_recover_ratio
            && long_clustered.len() > ys.len()
        {
            let mut merged = ys.clone();
            merged.extend(long_clustered);
            ys = cluster_coords(&merged, tol);
        }
    }
    if xs.len() < 3 || ys.len() < 3 {
        return None;
    }

    // Drop thin gaps → dense retained line sets (renumbered).
    xs = collapse_thin_gaps(&xs, min_cell);
    let mut y_ttb = collapse_thin_gaps(&ys, min_cell);
    y_ttb.sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));

    // Ruled anchors *before* text densify — joint density / conf use these so
    // synthetic lines do not understate structure quality.
    let xs_ruled = xs.clone();
    let ys_ruled = y_ttb.clone();

    // Sparse intermediate V rules (full H, V every Nth column) under-count
    // columns vs multi-row text left-edges. Densify X after joint filter +
    // thin-gap collapse, before building the cell grid.
    let mut synthetic_v_xs: Vec<f32> = Vec::new();
    let mut text_col_recovery = false;
    let mut synthetic_h_ys: Vec<f32> = Vec::new();
    let mut text_row_recovery = false;
    if should_run_text_densify(opts) {
        let y_hi = y_ttb.iter().copied().fold(f32::NEG_INFINITY, f32::max);
        let y_lo = y_ttb.iter().copied().fold(f32::INFINITY, f32::min);
        // Stub/line-number columns often sit just left of a ruled number grid.
        // Expand frame X at most carefully before left-edge densify.
        let dens_params = densify_params_from_tuning(&opts.advanced.tuning);
        let xs_exp = expand_xs_exterior_text_cols(&xs, runs, y_hi, y_lo, min_cell, &dens_params);
        if xs_exp.len() > xs.len() {
            let before_c = xs.len().saturating_sub(1);
            let after_c = xs_exp.len().saturating_sub(1);
            // Narrow frames: allow at most +1 exterior stub column (avoids
            // multi-word left-edge soup inventing several phantom cols).
            let max_extra = opts.advanced.tuning.densify_x_narrow_max_extra as usize;
            if !(before_c <= 3 && after_c > before_c + max_extra) {
                xs = xs_exp;
                text_col_recovery = true;
            }
        }
        let (x_densified, synth) =
            densify_x_from_text_cols(&xs, runs, y_hi, y_lo, min_cell, &dens_params);
        // Reject densify explosions (left-edge soup → many phantom cols).
        let dens_cols = x_densified.len().saturating_sub(1);
        let base_cols = xs.len().saturating_sub(1).max(1);
        let tun = &opts.advanced.tuning;
        let exploded_x = dens_cols > tun.densify_x_explode_abs_cols as usize
            && dens_cols as f32
                > (base_cols as f32) * tun.densify_x_explode_growth_factor
                    + tun.densify_x_explode_growth_add;
        // Narrow grids: at most +N synthetic V (multi-word false densify).
        let narrow_x_explode =
            base_cols <= 3 && dens_cols > base_cols + tun.densify_x_narrow_max_extra as usize;
        if !exploded_x
            && !narrow_x_explode
            && x_densified.len() as u32 <= opts.advanced.lattice_max_cols + 1
            && x_densified.len() > xs.len()
        {
            xs = x_densified;
            synthetic_v_xs = synth;
            text_col_recovery = true;
        }

        // False underlines / double rules: H anchors ≫ multi-col text bands → rebuild.
        // Never skip densify after this: a false overdense collapse left under-rowed
        // grids permanently stuck when densify was gated off.
        if opts.advanced.lattice_collapse_overdense_h {
            // Prefer tuning factor (document profiles) when it differs from advanced default.
            let overdense_factor = opts.advanced.tuning.lattice_overdense_h_factor;
            if let Some((y_new, synth)) = collapse_overdense_h_from_text(
                &y_ttb,
                runs,
                xs[0],
                *xs.last().unwrap_or(&xs[0]),
                min_cell,
                overdense_factor,
            ) {
                y_ttb = y_new;
                synthetic_h_ys = synth;
                text_row_recovery = true;
            }
        }

        // Sparse intermediate H rules under-count rows vs text bands (multi-col or
        // regular single-run body). Always attempt densify when under-dense.
        //
        // Skip Y densify on multi-line prose grids: rich V skeleton + few H
        // rules + low numeric density → H already marks true rows; densify
        // would shred wrapped cell text into phantom rows. Statistical grids
        // are digit-heavy and need densify. Thresholds: `opts.advanced.tuning`.
        let v_cols_now = xs.len().saturating_sub(1);
        let h_rows_now = y_ttb.len().saturating_sub(1);
        let numeric_frac = {
            let mut ne = 0u32;
            let mut num = 0u32;
            for r in runs {
                let t = r.text.trim();
                if t.is_empty() {
                    continue;
                }
                ne += 1;
                if t.chars().any(|c| c.is_ascii_digit()) {
                    num += 1;
                }
            }
            if ne == 0 {
                0.0
            } else {
                num as f32 / ne as f32
            }
        };
        let tun = &opts.advanced.tuning;
        let mut skip_y_densify = v_cols_now >= tun.densify_y_skip_min_v_cols as usize
            && h_rows_now <= tun.densify_y_skip_max_h_rows as usize
            && h_rows_now >= tun.densify_y_skip_min_h_rows as usize
            && numeric_frac < tun.densify_y_skip_numeric_frac;
        // Sparse-H + rich multi-col text: do not skip densify (under-row recovery).
        // Geometric: multi-col bands ≥ sparse_h_force_mult × H body rows.
        if skip_y_densify && h_rows_now <= tun.densify_y_skip_max_h_rows as usize {
            let multi_probe =
                multi_col_band_centers(runs, xs[0], *xs.last().unwrap_or(&xs[0]), &y_ttb, min_cell);
            let need = ((h_rows_now as f32) * tun.densify_y_sparse_h_force_mult)
                .ceil()
                .max(3.0) as usize;
            if multi_probe.len() >= need {
                skip_y_densify = false;
            }
        }
        if !skip_y_densify {
            let y_before = y_ttb.clone();
            let (y_densified, synth) = densify_y_from_text_bands(
                &y_ttb,
                runs,
                xs[0],
                *xs.last().unwrap_or(&xs[0]),
                min_cell,
                &dens_params,
            );
            if y_densified.len() as u32 > opts.advanced.lattice_max_rows + 1 {
                // Too many inferred rows — keep pre-densify anchors.
                y_ttb = y_before;
            } else if y_densified.len() > y_before.len() {
                let before_rows = y_before.len().saturating_sub(1);
                let after_rows = y_densified.len().saturating_sub(1);
                // Growth policy (TableTuning — document profiles; no corpus branches):
                // - Small recovery: +≤delta and growth ≤ small_growth_max → keep.
                // - Mid wrap explode: growth in (lo, hi] → reject.
                // - Large growth only if multi-col text bands support row count.
                let growth = after_rows as f32 / (before_rows.max(1) as f32);
                let small_recovery = after_rows
                    <= before_rows + tun.densify_y_small_delta_max as usize
                    && growth <= tun.densify_y_small_growth_max;
                let multi_bands = multi_col_band_centers(
                    runs,
                    xs[0],
                    *xs.last().unwrap_or(&xs[0]),
                    &y_densified,
                    min_cell,
                );
                let multi_n = multi_bands.len().max(1);
                let band_cap = (multi_n as f32 * tun.densify_y_max_rows_vs_multi_band
                    + tun.densify_y_multi_band_slack)
                    .ceil() as usize;
                // Mid-range growth: reject wrap explosions (default).
                // When multi-col bands tightly support after_rows (after ≈ multi_n
                // within tuning cap), allow densify in the mid band (under-row recovery).
                let tight_support = multi_n >= 3
                    && after_rows <= band_cap
                    && (after_rows as f32) >= multi_n as f32 * 0.75
                    && (after_rows as i32 - multi_n as i32).unsigned_abs() as usize
                        <= tun.densify_y_multi_band_slack as usize + 1;
                let wrap_explode = !small_recovery
                    && !tight_support
                    && before_rows >= tun.densify_y_explode_min_before as usize
                    && after_rows > before_rows + tun.densify_y_small_delta_max as usize
                    && growth
                        > tun
                            .densify_y_explode_growth_lo
                            .max(tun.densify_y_small_growth_max)
                    && growth <= tun.densify_y_explode_growth_hi;
                // Reject severe over-row densify when multi bands cannot support rows
                // (e.g. 4 H → 16 densify with only ~4 multi bands).
                let severe_over_row = !small_recovery
                    && !tight_support
                    && multi_n >= 2
                    && after_rows > before_rows
                    && after_rows > band_cap
                    && growth > tun.densify_y_explode_growth_hi;
                if wrap_explode || severe_over_row {
                    y_ttb = y_before;
                } else {
                    y_ttb = y_densified;
                    synthetic_h_ys = synth;
                    text_row_recovery = true;
                }
            }
        }
    }

    let nrows = y_ttb.len().saturating_sub(1);
    let ncols = xs.len().saturating_sub(1);
    if nrows < 2 || ncols < 2 {
        return None;
    }
    if nrows as u32 > opts.advanced.lattice_max_rows
        || ncols as u32 > opts.advanced.lattice_max_cols
    {
        return None;
    }

    let mut h_local: Vec<HSeg> = h_idx.iter().map(|&i| h_segs[i]).collect();
    // Virtual H rules at text-inferred separators so rowspan merge does not
    // re-collapse densified rows, and edge completeness stays meaningful.
    if text_row_recovery {
        let x0 = xs[0];
        let x1 = *xs.last().unwrap_or(&x0);
        for &y in &synthetic_h_ys {
            h_local.push(HSeg { y, x0, x1 });
        }
    }
    let mut v_local: Vec<VSeg> = v_idx.iter().map(|&i| v_segs[i]).collect();
    // Virtual V rules at text-inferred column separators (partial-V densify).
    if text_col_recovery {
        let y_top = y_ttb.iter().copied().fold(f32::NEG_INFINITY, f32::max);
        let y_bot = y_ttb.iter().copied().fold(f32::INFINITY, f32::min);
        for &x in &synthetic_v_xs {
            v_local.push(VSeg {
                x,
                y0: y_bot,
                y1: y_top,
            });
        }
    }
    let cover_frac = opts.advanced.lattice_edge_cover_frac;

    // Dense nrows×ncols cells (geometry first; text via exclusive assignment).
    let mut grid: Vec<Vec<RawCell>> = Vec::with_capacity(nrows);
    let mut edge_hits = 0u32;
    let mut edge_total = 0u32;
    let mut flat_bboxes: Vec<Rect> = Vec::with_capacity(nrows * ncols);

    for row in 0..nrows {
        let y_top = y_ttb[row];
        let y_bot = y_ttb[row + 1];
        let (y1, y0) = if y_top >= y_bot {
            (y_top, y_bot)
        } else {
            (y_bot, y_top)
        };
        let mut row_cells = Vec::with_capacity(ncols);
        for col in 0..ncols {
            let x0 = xs[col];
            let x1 = xs[col + 1];
            let bbox = Rect { x0, y0, x1, y1 };
            let edges = edge_flags(bbox, &h_local, &v_local, tol, cover_frac);
            edge_total += 4;
            if edges.left {
                edge_hits += 1;
            }
            if edges.right {
                edge_hits += 1;
            }
            if edges.top {
                edge_hits += 1;
            }
            if edges.bottom {
                edge_hits += 1;
            }
            flat_bboxes.push(bbox);
            row_cells.push(RawCell {
                bbox,
                text: String::new(),
                edges,
                active: true,
                colspan: 1,
                rowspan: 1,
            });
        }
        grid.push(row_cells);
    }

    // One run → one cell (avoids boundary double-counts that block colspan).
    // Wide multi-col runs are split in assign_runs_exclusive when bboxes span.
    //
    // Do **not** redistribute here: `merge_spans_dense` re-binds runs onto
    // master cells and would wipe any pre-span spill. Redistribute once after
    // the final exclusive assign (below).
    let texts = assign_runs_exclusive(runs, &flat_bboxes);
    let mut filled = 0usize;
    for (i, text) in texts.into_iter().enumerate() {
        let r = i / ncols;
        let c = i % ncols;
        if !text.trim().is_empty() {
            filled += 1;
        }
        grid[r][c].text = text;
    }

    let total = (nrows * ncols).max(1);
    let fill_rate = filled as f32 / total as f32;
    // Empty cells only for pure image-table pages (raster_primary): text is ink.
    // On mixed pages, decorative image grids must pass normal fill gates.
    let allow_empty = raster_primary
        && opts.advanced.raster_allow_empty_cells
        && nrows >= 2
        && ncols >= 2
        && joints.len() >= 4;
    if !allow_empty {
        if fill_rate < opts.advanced.lattice_min_fill_rate && filled < 2 {
            return None;
        }
        let empty_frac = 1.0 - fill_rate;
        if empty_frac >= opts.advanced.lattice_empty_frac_reject
            && filled < opts.advanced.lattice_min_filled_cells as usize
        {
            return None;
        }
    }

    merge_spans_dense(&mut grid);

    // Re-bind runs onto active masters only (union bboxes after span growth).
    // Covered slots stay empty — ICDAR-style blanks under col/row spans.
    {
        let mut master_idx: Vec<(usize, usize)> = Vec::new();
        let mut master_boxes: Vec<Rect> = Vec::new();
        for (r, row) in grid.iter().enumerate() {
            for (c, cell) in row.iter().enumerate() {
                if cell.active {
                    master_idx.push((r, c));
                    master_boxes.push(cell.bbox);
                }
            }
        }
        let texts = assign_runs_exclusive(runs, &master_boxes);
        for ((r, c), text) in master_idx.into_iter().zip(texts) {
            grid[r][c].text = text;
        }
        for row in grid.iter_mut() {
            for cell in row.iter_mut() {
                if !cell.active {
                    cell.text.clear();
                }
            }
        }
        // Span re-assign can re-dump multi-token lines into one master; spill
        // again so year/number grids keep per-column tokens.
        redistribute_row_tokens(&mut grid);
    }

    // Dense emission: masters carry colspan/rowspan; covered slots stay empty
    // under spans (text at top-left of span).
    let (cells, max_row, max_col) = emit_cells_dense(&grid);
    // Drop completely empty interior columns after densify can invent gutters.
    // Never collapse pure image lattices (all cells empty → would shred schema).
    let (cells, max_row, max_col) = if raster_primary && opts.advanced.raster_allow_empty_cells {
        (cells, max_row, max_col)
    } else {
        collapse_sparse_interior_columns(cells, max_row, max_col)
    };
    if cells.is_empty() || max_row < 2 || max_col < 2 {
        return None;
    }

    // Tiny chrome (caption 2×2, empty form labels): not data tables.
    let filled_final = cells.iter().filter(|c| !c.text.trim().is_empty()).count();
    if max_row <= opts.advanced.lattice_min_side_for_tiny_reject
        && max_col <= opts.advanced.lattice_min_side_for_tiny_reject
        && filled_final <= opts.advanced.lattice_tiny_max_filled as usize
    {
        // Allow if cells carry substantial text (real tiny data table)
        let mean_chars = cells
            .iter()
            .filter(|c| !c.text.trim().is_empty())
            .map(|c| c.text.trim().chars().count())
            .sum::<usize>() as f32
            / filled_final.max(1) as f32;
        if mean_chars < 12.0 {
            return None;
        }
    }

    let bbox = bbox_of_cells(&cells);
    let area = bbox.width().max(0.0) * bbox.height().max(0.0);
    if area < opts.advanced.lattice_min_table_area {
        return None;
    }
    // Regularity / joint density vs *ruled* anchors (pre-densify) so synthetic
    // text-inferred lines do not understate structure quality.
    let grid_regularity = grid_regularity_score(&xs_ruled, &ys_ruled);
    let edge_score = if edge_total == 0 {
        0.0
    } else {
        edge_hits as f32 / edge_total as f32
    };
    let expected_joints = (xs_ruled.len() * ys_ruled.len()) as f32;
    let joint_density = if expected_joints < 1.0 {
        0.0
    } else {
        (joints.len() as f32 / expected_joints).min(1.0)
    };

    // Structure-only (empty) tables: weight edges/joints/regularity higher than fill.
    let conf = if fill_rate < 0.05 && (used_raster || raster_primary) {
        (0.40 * grid_regularity
            + 0.35 * edge_score
            + 0.15 * joint_density
            + 0.10 * (cells.len() as f32 / 6.0).min(1.0))
        .clamp(0.0, 1.0)
    } else {
        (0.30 * grid_regularity
            + 0.25 * edge_score
            + 0.20 * fill_rate
            + 0.15 * joint_density
            + 0.10 * (cells.len() as f32 / 6.0).min(1.0))
        .clamp(0.0, 1.0)
    };

    // Empty raster lattices: require non-weak edges and some joint density.
    if used_raster && fill_rate < 0.05 {
        if edge_score < opts.advanced.lattice_weak_edge_threshold || joint_density < 0.25 {
            return None;
        }
    }

    let weak_edges = edge_score < opts.advanced.lattice_weak_edge_threshold;
    let mut notes = vec![format!(
        "lattice_cc joints={} h={} v={} xs={} ys={} edge={edge_score:.2}",
        joints.len(),
        h_idx.len(),
        v_idx.len(),
        xs.len(),
        y_ttb.len()
    )];
    if text_row_recovery {
        notes.push(format!(
            "text_row_recovery synthetic_h={}",
            synthetic_h_ys.len()
        ));
    }
    if text_col_recovery {
        notes.push(format!(
            "text_col_recovery synthetic_v={}",
            synthetic_v_xs.len()
        ));
    }
    if cells.iter().any(|c| c.colspan > 1 || c.rowspan > 1) {
        notes.push("spans_merged".into());
    }

    Some(Table {
        bbox,
        page: page_index,
        method: TableMethod::Lattice,
        confidence: conf,
        rows: max_row,
        cols: max_col,
        cells,
        header_rows: 1,
        continued_from_previous_page: false,
        continued_to_next_page: false,
        logical_table_id: None,
        strategy_provenance: vec![PipelineId::S2Lattice],
        notes,
        edge_score,
        fill_rate,
        weak_edges,
        joint_count: joints.len() as u32,
        text_row_recovery,
        text_col_recovery,
        multitable_stream_recovery: false,
        stream_vs_overwide_hybrid: false,
    })
}

/// Build densify params from caller-overridable tuning (shared defaults).
fn densify_params_from_tuning(tun: &crate::TableTuning) -> super::densify::DensifyParams {
    super::densify::DensifyParams {
        pitch_cv_max: tun.densify_pitch_cv_max,
        exterior_pad_frac: tun.densify_x_exterior_pad_frac,
        short_token_chars: tun.densify_x_short_token_chars as usize,
    }
}

fn table_from_global_snap(
    page_index: u32,
    runs: &[TextRun],
    h_segs: &[HSeg],
    v_segs: &[VSeg],
    opts: &TableOptions,
    min_cell: f32,
    tol: f32,
    used_raster: bool,
    raster_primary: bool,
) -> Option<Table> {
    let joint_gap = opts.advanced.lattice_joint_gap;
    let min_joints = opts.advanced.lattice_min_joints.max(1) as usize;

    let xs = cluster_coords(&v_segs.iter().map(|s| s.x).collect::<Vec<_>>(), tol);
    let ys = cluster_coords(&h_segs.iter().map(|s| s.y).collect::<Vec<_>>(), tol);
    if xs.len() < 3 || ys.len() < 3 {
        return None;
    }

    // Joints only where both an H and V segment actually cover the crossing
    // (with joint_gap), not a full Cartesian product of all line coords.
    let mut joints = Vec::new();
    for h in h_segs {
        for v in v_segs {
            if let Some(pt) = segments_cross_hv(h, v, tol, joint_gap) {
                joints.push(pt);
            }
        }
    }
    if joints.len() < min_joints {
        return None;
    }

    let h_idx: Vec<usize> = (0..h_segs.len()).collect();
    let v_idx: Vec<usize> = (0..v_segs.len()).collect();
    let mut t = table_from_component(
        page_index,
        runs,
        h_segs,
        v_segs,
        &h_idx,
        &v_idx,
        &joints,
        opts,
        min_cell,
        tol,
        used_raster,
        raster_primary,
    )?;
    t.notes.push("lattice_global_fallback".into());
    Some(t)
}

#[cfg(test)]
mod tests {
    use super::super::densify::DensifyParams;
    use super::cells::{split_glued_numeric, tokenize_cell};
    use super::*;

    #[test]
    fn split_glued_numeric_fast_path_and_glued() {
        // Ordinary single number: leave intact (fast path).
        assert_eq!(split_glued_numeric("1,234.5"), vec!["1,234.5".to_string()]);
        assert_eq!(split_glued_numeric("804"), vec!["804".to_string()]);
        // Multi-comma glued US numbers: split into tokens.
        let glued = split_glued_numeric("804,006671,330");
        assert!(
            glued.len() >= 2,
            "expected multi-token split, got {glued:?}"
        );
        // Non-numeric: unchanged.
        assert_eq!(split_glued_numeric("Total"), vec!["Total".to_string()]);
    }

    #[test]
    fn tokenize_cell_splits_glued_numbers() {
        let toks = tokenize_cell("804,006671,330636,903");
        assert!(
            toks.len() >= 2,
            "glued census-style numbers should tokenize, got {toks:?}"
        );
    }

    #[test]
    fn two_disjoint_grids_two_components() {
        let mut h = Vec::new();
        let mut v = Vec::new();
        for y in [0.0_f32, 50.0, 100.0] {
            h.push(HSeg {
                y,
                x0: 0.0,
                x1: 100.0,
            });
            h.push(HSeg {
                y,
                x0: 200.0,
                x1: 300.0,
            });
        }
        for x in [0.0_f32, 50.0, 100.0] {
            v.push(VSeg {
                x,
                y0: 0.0,
                y1: 100.0,
            });
        }
        for x in [200.0_f32, 250.0, 300.0] {
            v.push(VSeg {
                x,
                y0: 0.0,
                y1: 100.0,
            });
        }
        let clusters = cluster_line_components(&h, &v, 2.0, 3.5, 4);
        assert!(
            clusters.len() >= 2,
            "expected ≥2 components, got {}",
            clusters.len()
        );
    }

    #[test]
    fn collapse_thin_keeps_span() {
        let xs = vec![0.0, 1.0, 50.0, 100.0]; // 1.0 is thin after 0
        let out = collapse_thin_gaps(&xs, 3.0);
        assert!(out.len() >= 3, "{out:?}");
        assert!((out[0] - 0.0).abs() < 1e-3);
        assert!((out.last().copied().unwrap() - 100.0).abs() < 1e-3);
    }

    #[test]
    fn joint_filter_drops_singleton_and_short_span() {
        // Full V lines at 0,50,100; singleton at 25; short mid-span at 75 (two joints only mid-y)
        let mut joints = vec![];
        for x in [0.0_f32, 50.0, 100.0] {
            for y in [0.0_f32, 50.0, 100.0] {
                joints.push((x, y));
            }
        }
        joints.push((25.0, 50.0)); // singleton
        joints.push((75.0, 40.0));
        joints.push((75.0, 60.0)); // short span vs global 100
        let coords = vec![0.0, 25.0, 50.0, 75.0, 100.0];
        let kept = filter_joint_supported_coords(&coords, &joints, 2.0, true, 2, 0.45);
        assert!(
            !kept.iter().any(|&x| (x - 25.0).abs() < 1.0),
            "singleton dropped: {kept:?}"
        );
        assert!(
            !kept.iter().any(|&x| (x - 75.0).abs() < 1.0),
            "short-span phantom dropped: {kept:?}"
        );
        assert_eq!(kept.len(), 3, "{kept:?}");
    }

    fn mk_run(x0: f32, y0: f32, text: &str) -> TextRun {
        TextRun {
            text: text.into(),
            bbox: Rect {
                x0,
                y0,
                x1: x0 + 20.0,
                y1: y0 + 8.0,
            },
            transform: pdfparser_ir::Matrix3x2::identity(),
            font_name: None,
            font_size: 8.0,
            mapping_confidence: 1.0,
            metrics_confidence: 1.0,
            mcid: None,
            invisible: false,
            from_actual_text: false,
        }
    }

    #[test]
    fn densify_y_subdivides_sparse_h_gaps() {
        // 5 H lines (4 gaps) but 12 multi-col text bands — classic partial body H.
        // Gaps of 3 bands each between H at 700, 652, 604, 556, 508 (every 48pt).
        let y_h = vec![700.0_f32, 652.0, 604.0, 556.0, 508.0];
        let mut runs = Vec::new();
        // 12 row centers from 692 down by 16
        for i in 0..12 {
            let y = 692.0 - 16.0 * i as f32;
            for (xi, label) in [(40.0, "A"), (90.0, "B"), (140.0, "C")].iter() {
                runs.push(mk_run(*xi, y - 4.0, label));
            }
        }
        let (densified, synth) =
            densify_y_from_text_bands(&y_h, &runs, 30.0, 180.0, 3.0, &DensifyParams::default());
        let nrows = densified.len().saturating_sub(1);
        assert_eq!(
            nrows, 12,
            "expected 12 rows from text densify, got {nrows} ys={densified:?} synth={synth:?}"
        );
        assert!(
            !synth.is_empty(),
            "expected synthetic H separators, got none"
        );
    }

    #[test]
    fn densify_y_noop_when_h_matches_text() {
        // One multi-col band per H gap → no densify.
        let y_h = vec![100.0_f32, 80.0, 60.0, 40.0];
        let mut runs = Vec::new();
        for y in [90.0_f32, 70.0, 50.0] {
            for xi in [10.0_f32, 50.0, 90.0] {
                runs.push(mk_run(xi, y - 4.0, "x"));
            }
        }
        let (densified, synth) =
            densify_y_from_text_bands(&y_h, &runs, 0.0, 120.0, 3.0, &DensifyParams::default());
        assert_eq!(densified.len(), y_h.len(), "ys={densified:?}");
        assert!(synth.is_empty());
    }

    #[test]
    fn expand_xs_adds_left_stub_column() {
        // Ruled number grid at x=200..600; line numbers at x=50 and labels at x=80
        // aligned across many rows (BEA-style exterior stub).
        let xs_v = vec![200.0_f32, 300.0, 400.0, 500.0, 600.0];
        let mut runs = Vec::new();
        for row in 0..12 {
            let y = 400.0 - 14.0 * row as f32;
            runs.push(mk_run(50.0, y - 4.0, &format!("{row}")));
            runs.push(mk_run(80.0, y - 4.0, "label"));
            for k in 0..4 {
                runs.push(mk_run(210.0 + 100.0 * k as f32, y - 4.0, "1.0"));
            }
        }
        let expanded = expand_xs_exterior_text_cols(
            &xs_v,
            &runs,
            410.0,
            200.0,
            3.0,
            &DensifyParams::default(),
        );
        assert!(
            expanded.len() > xs_v.len(),
            "expected left exterior expansion, got {expanded:?}"
        );
        assert!(
            expanded[0] < 200.0,
            "outer left should be left of frame: {expanded:?}"
        );
        let ncols = expanded.len().saturating_sub(1);
        assert!(
            ncols >= 6,
            "line+label+4 data → ≥6 cols, got {ncols} xs={expanded:?}"
        );
    }

    #[test]
    fn densify_x_subdivides_every_other_v() {
        // Full H implied by multi-row text; V only every other column (step-2).
        // True 10 cols at pitch 40: V at 0,80,160,240,320,400 (6 lines → 5 gaps).
        // Text left-edges at 2 + 40*k for k=0..10 across many rows.
        let xs_v = vec![0.0_f32, 80.0, 160.0, 240.0, 320.0, 400.0];
        let mut runs = Vec::new();
        for row in 0..12 {
            let y = 200.0 - 14.0 * row as f32;
            for k in 0..10 {
                let x = 2.0 + 40.0 * k as f32;
                runs.push(mk_run(x, y - 4.0, "c"));
            }
        }
        let (densified, synth) =
            densify_x_from_text_cols(&xs_v, &runs, 210.0, 20.0, 3.0, &DensifyParams::default());
        let ncols = densified.len().saturating_sub(1);
        assert_eq!(
            ncols, 10,
            "expected 10 cols from partial-V densify, got {ncols} xs={densified:?} synth={synth:?}"
        );
        assert!(
            !synth.is_empty(),
            "expected synthetic V separators, got none"
        );
    }

    #[test]
    fn densify_y_includes_sparse_single_cell_rows() {
        // Partial H every 5 body lines; most rows multi-col but a few key-only.
        // H at 700, 640, 580 (outer + mid) → 2 large gaps holding 5 rows each.
        let y_h = vec![700.0_f32, 640.0, 580.0];
        let mut runs = Vec::new();
        // 10 body rows, centers 694, 682, … 586 (step 12).
        for i in 0..10 {
            let y = 694.0 - 12.0 * i as f32;
            // Key column always present.
            runs.push(mk_run(40.0, y - 4.0, &format!("R{i:02}")));
            // Sparse multi-col: skip i=2 and i=7 (single-cell only).
            if i != 2 && i != 7 {
                runs.push(mk_run(100.0, y - 4.0, "v"));
                if i % 3 == 0 {
                    runs.push(mk_run(160.0, y - 4.0, "w"));
                }
            }
        }
        let (densified, synth) =
            densify_y_from_text_bands(&y_h, &runs, 30.0, 200.0, 3.0, &DensifyParams::default());
        let nrows = densified.len().saturating_sub(1);
        assert_eq!(
            nrows, 10,
            "sparse single-cell rows must densify, got {nrows} ys={densified:?} synth={synth:?}"
        );
        assert!(
            !synth.is_empty(),
            "expected synthetic H separators, got none"
        );
    }

    #[test]
    fn densify_x_noop_when_full_v_matches_text() {
        // Full V with multi-token cells: primary + second word left-edges that
        // *do* align across rows (SKU + short label) but cluster near the cell
        // left — span ≪ gap, so must not densify (painted/SKU regression).
        let xs_v = vec![0.0_f32, 50.0, 100.0, 150.0, 200.0];
        let mut runs = Vec::new();
        for row in 0..8 {
            let y = 160.0 - 16.0 * row as f32;
            for x in [5.0_f32, 55.0, 105.0, 155.0] {
                runs.push(mk_run(x, y - 4.0, "sku"));
                // Second token ~14pt into the cell (aligned, multi-row support).
                runs.push(mk_run(x + 14.0, y - 4.0, "desc"));
            }
        }
        let (densified, synth) =
            densify_x_from_text_cols(&xs_v, &runs, 170.0, 20.0, 3.0, &DensifyParams::default());
        assert_eq!(
            densified.len(),
            xs_v.len(),
            "full-V multi-token must not densify: xs={densified:?}"
        );
        assert!(synth.is_empty());
    }

    #[test]
    fn densify_y_dense_every_row_h_no_over_split() {
        // Full H (every row ruled) with multi-col text — must not invent extra rows.
        let y_h: Vec<f32> = (0..6).map(|i| 100.0 - 12.0 * i as f32).collect();
        let mut runs = Vec::new();
        for i in 0..5 {
            let y = 94.0 - 12.0 * i as f32;
            for xi in [10.0_f32, 50.0, 90.0] {
                runs.push(mk_run(xi, y - 4.0, "x"));
            }
        }
        let (densified, synth) =
            densify_y_from_text_bands(&y_h, &runs, 0.0, 120.0, 3.0, &DensifyParams::default());
        assert_eq!(
            densified.len(),
            y_h.len(),
            "every-row H must not over-split ys={densified:?}"
        );
        assert!(synth.is_empty(), "synth={synth:?}");
    }

    #[test]
    fn densify_y_rejects_single_col_prose_stack() {
        // Ruled frame with multi-col header only + long single-col body → no densify
        // from prose lines (multi not majority of bands).
        let y_h = vec![200.0_f32, 100.0];
        let mut runs = Vec::new();
        // One multi-col header band.
        for xi in [20.0_f32, 80.0, 140.0] {
            runs.push(mk_run(xi, 190.0, "H"));
        }
        // 8 single-col prose lines inside the gap.
        for i in 0..8 {
            let y = 175.0 - 8.0 * i as f32;
            runs.push(mk_run(20.0, y, "prose"));
        }
        let (densified, synth) =
            densify_y_from_text_bands(&y_h, &runs, 10.0, 180.0, 3.0, &DensifyParams::default());
        assert_eq!(
            densified.len(),
            y_h.len(),
            "single-col prose must not densify ys={densified:?} synth={synth:?}"
        );
        assert!(synth.is_empty());
    }
    #[test]
    fn strip_footer_totals_on_invoice_grid() {
        use crate::types::{Table, TableCell, TableMethod};
        use pdfparser_ir::Rect;
        let mut cells = Vec::new();
        let rows = [
            vec!["SKU", "Description", "Qty", "Unit", "Amount"],
            vec!["SKU-A", "Svc A", "1", "10", "10"],
            vec!["SKU-B", "Svc B", "2", "5", "10"],
            vec!["", "Subtotal", "", "", "20"],
            vec!["", "Total", "", "", "20"],
        ];
        for (r, row) in rows.iter().enumerate() {
            for (c, text) in row.iter().enumerate() {
                cells.push(TableCell {
                    row: r as u32,
                    col: c as u32,
                    rowspan: 1,
                    colspan: 1,
                    bbox: Rect {
                        x0: c as f32 * 20.0,
                        y0: 100.0 - r as f32 * 10.0,
                        x1: (c as f32 + 1.0) * 20.0,
                        y1: 110.0 - r as f32 * 10.0,
                    },
                    text: (*text).into(),
                    is_header: r == 0,
                    confidence: 1.0,
                });
            }
        }
        let mut table = Table {
            bbox: Rect {
                x0: 0.0,
                y0: 50.0,
                x1: 100.0,
                y1: 120.0,
            },
            page: 0,
            method: TableMethod::Lattice,
            confidence: 1.0,
            rows: 5,
            cols: 5,
            cells,
            header_rows: 1,
            continued_from_previous_page: false,
            continued_to_next_page: false,
            logical_table_id: None,
            strategy_provenance: vec![],
            notes: vec![],
            edge_score: 1.0,
            fill_rate: 0.8,
            weak_edges: false,
            joint_count: 0,
            text_row_recovery: false,
            text_col_recovery: false,
            multitable_stream_recovery: false,
            stream_vs_overwide_hybrid: false,
        };
        strip_trailing_footer_totals(&mut table);
        assert_eq!(table.rows, 3, "stripped totals rows");
        assert!(table
            .notes
            .iter()
            .any(|n| n.contains("footer_totals_stripped")));
    }

    #[test]
    fn detect_lattice_full_stroke_grid() {
        use crate::options::{TableOptions, TablePreset};
        use pdfparser_content::RuleSegment;
        let mut rules = Vec::new();
        for y in [0.0_f32, 40.0, 80.0, 120.0] {
            rules.push(RuleSegment {
                x0: 0.0,
                y0: y,
                x1: 100.0,
                y1: y,
            });
        }
        for x in [0.0_f32, 50.0, 100.0] {
            rules.push(RuleSegment {
                x0: x,
                y0: 0.0,
                x1: x,
                y1: 120.0,
            });
        }
        let mut runs = Vec::new();
        let labels = [["A", "B"], ["C", "D"], ["E", "F"]];
        for (r, row) in labels.iter().enumerate() {
            for (c, lab) in row.iter().enumerate() {
                let x0 = 5.0 + c as f32 * 50.0;
                let y0 = 90.0 - r as f32 * 40.0;
                runs.push(mk_run(x0, y0, lab));
            }
        }
        let opts = TableOptions::from_preset(TablePreset::Full);
        let tabs = detect_ruled_tables(0, &runs, &rules, &opts, &[]);
        assert!(!tabs.is_empty(), "expected lattice table");
        assert!(tabs[0].rows >= 2 && tabs[0].cols >= 2);
    }
}
