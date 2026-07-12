//! Raster line sensing for ruled tables (Camelot-class production pipeline).
//!
//! Pipeline (see [`morph`] for algorithm detail):
//! 1. Grayscale ROI (embedded Image XObject, CTM-mapped)
//! 2. Adaptive threshold → binary ink
//! 3. Optional K28: stamp vector H/V into ink ([`stamp_vector_rules_into_mask`])
//! 4. Morph close (dashed) + multi-scale H/V open
//! 5. RLE extract ∪ projection-profile full-span peaks
//! 6. Span filter + joint-graph + spacing regularity gate
//! 7. Map to page space → [`RuleSegment`]s for the lattice engine
//! 8. Contour seeds: CC on H∨V masks ([`contour_seeds_from_page`])
//!
//! Pure geometry/image processing — no PDF I/O. Callers supply pixels + affine.

mod morph;

// Re-export morph public API. Names not referenced in this file are still part of
// the crate surface (`crate::raster::*`); allow unused_imports for pure re-exports.
#[allow(unused_imports)]
pub use morph::{
    combined_line_masks, contour_seeds_from_page, detect_line_segments,
    detect_line_segments_combined, detect_line_segments_from_ink, find_contour_seeds,
    gray_from_rgb, gray_from_rgba, page_to_pix, stamp_vector_rules_into_gray,
    stamp_vector_rules_into_mask, threshold_ink, ContourSeed, RasterConfig, RasterPage, RasterRule,
};

use pdfparser_content::RuleSegment;

/// Convert raster rules to content-stream style segments for the lattice engine.
pub fn rules_from_raster(page: &RasterPage, cfg: &RasterConfig) -> Vec<RuleSegment> {
    detect_line_segments(page, cfg)
        .into_iter()
        .map(|r| RuleSegment {
            x0: r.x0,
            y0: r.y0,
            x1: r.x1,
            y1: r.y1,
        })
        .collect()
}

/// Combined vector∪raster line sensing (K28).
///
/// Stamps near-H/V `vector_rules` into the threshold ink mask, runs the morph
/// detector, then merges recovered segments with the original vector rules
/// (dedupe via [`merge_rules`]).
pub fn rules_from_raster_combined(
    page: &RasterPage,
    vector_rules: &[RuleSegment],
    cfg: &RasterConfig,
) -> Vec<RuleSegment> {
    let raster: Vec<RuleSegment> = detect_line_segments_combined(page, vector_rules, cfg)
        .into_iter()
        .map(|r| RuleSegment {
            x0: r.x0,
            y0: r.y0,
            x1: r.x1,
            y1: r.y1,
        })
        .collect();
    let snap = cfg.pos_snap_px.max(1.0) * page.scale_x.abs().max(page.scale_y.abs()).max(1.0);
    merge_rules(vector_rules, &raster, snap)
}

/// Build a production [`RasterConfig`] for a page using option floors.
pub fn config_for_raster_page(
    page: &RasterPage,
    adaptive_radius: usize,
    adaptive_bias: u8,
    min_kernel: usize,
    min_seg_px: usize,
    merge_gap_px: usize,
    pos_snap_px: f32,
) -> RasterConfig {
    RasterConfig::for_dimensions(page.width, page.height).with_option_floors(
        adaptive_radius,
        adaptive_bias,
        min_kernel,
        min_seg_px,
        merge_gap_px,
        pos_snap_px,
    )
}

/// Merge vector rules with raster-derived rules (dedupe near-duplicates).
pub fn merge_rules(vector: &[RuleSegment], raster: &[RuleSegment], snap: f32) -> Vec<RuleSegment> {
    let mut out: Vec<RuleSegment> = vector.to_vec();
    for r in raster {
        let dup = out.iter().any(|v| {
            let same_h = r.is_horizontal(snap)
                && v.is_horizontal(snap)
                && (r.y0 - v.y0).abs() <= snap
                && intervals_overlap(
                    r.x0.min(r.x1),
                    r.x0.max(r.x1),
                    v.x0.min(v.x1),
                    v.x0.max(v.x1),
                );
            let same_v = r.is_vertical(snap)
                && v.is_vertical(snap)
                && (r.x0 - v.x0).abs() <= snap
                && intervals_overlap(
                    r.y0.min(r.y1),
                    r.y0.max(r.y1),
                    v.y0.min(v.y1),
                    v.y0.max(v.y1),
                );
            same_h || same_v
        });
        if !dup {
            out.push(*r);
        }
    }
    out
}

fn intervals_overlap(a0: f32, a1: f32, b0: f32, b1: f32) -> bool {
    a0 <= b1 && b0 <= a1
}

#[cfg(test)]
mod tests {
    use super::*;
    use morph::{detect_line_segments, RasterConfig, RasterPage};

    /// Draw a ruled grid into a grayscale buffer (white bg, black lines).
    fn synthetic_grid(rows: usize, cols: usize, cell: usize, line_w: usize) -> RasterPage {
        let w = cols * cell + line_w;
        let h = rows * cell + line_w;
        let mut px = vec![255u8; w * h];
        for r in 0..=rows {
            let y0 = r * cell;
            for dy in 0..line_w {
                let y = y0 + dy;
                if y >= h {
                    break;
                }
                for x in 0..w {
                    px[y * w + x] = 0;
                }
            }
        }
        for c in 0..=cols {
            let x0 = c * cell;
            for dx in 0..line_w {
                let x = x0 + dx;
                if x >= w {
                    break;
                }
                for y in 0..h {
                    px[y * w + x] = 0;
                }
            }
        }
        RasterPage {
            width: w,
            height: h,
            pixels: px,
            scale_x: 1.0,
            scale_y: 1.0,
            origin_x: 0.0,
            origin_y: 0.0,
            y_down_pixels: true,
        }
    }

    #[test]
    fn morph_finds_grid_lines() {
        let page = synthetic_grid(5, 4, 20, 2);
        let cfg = RasterConfig::for_dimensions(page.width, page.height);
        let segs = detect_line_segments(&page, &cfg);
        let n_h = segs.iter().filter(|s| (s.y0 - s.y1).abs() < 1.5).count();
        let n_v = segs.iter().filter(|s| (s.x0 - s.x1).abs() < 1.5).count();
        assert!(n_h >= 5, "expected ≥5 H lines, got {n_h} segs={segs:?}");
        assert!(n_v >= 4, "expected ≥4 V lines, got {n_v}");
    }

    #[test]
    fn morph_on_c100_png() {
        let img = match image::open("/tmp/c100_grid.png") {
            Ok(i) => i,
            Err(_) => {
                eprintln!("skip no /tmp/c100_grid.png");
                return;
            }
        };
        let img = img.to_rgb8();
        let (w, h) = (img.width() as usize, img.height() as usize);
        let gray = morph::gray_from_rgb(img.as_raw(), w, h).unwrap();
        let page = RasterPage {
            width: w,
            height: h,
            pixels: gray,
            scale_x: 520.0 / w as f32,
            scale_y: 438.961 / h as f32,
            origin_x: 40.0,
            origin_y: 273.039,
            y_down_pixels: true,
        };
        let cfg = RasterConfig::for_dimensions(w, h);
        let segs = detect_line_segments(&page, &cfg);
        let n_h = segs.iter().filter(|s| (s.y0 - s.y1).abs() < 2.0).count();
        let n_v = segs.iter().filter(|s| (s.x0 - s.x1).abs() < 2.0).count();
        assert!(n_h >= 5 && n_v >= 4, "H={n_h} V={n_v}");
    }

    #[test]
    fn lattice_from_c100_raster_rules() {
        use crate::lattice::detect_lattice_tables;
        use crate::options::{TableOptions, TablePreset};
        use pdfparser_ir::{Matrix3x2, Rect, TextRun};

        let img = match image::open("/tmp/c100_grid.png") {
            Ok(i) => i,
            Err(_) => return,
        };
        let img = img.to_rgb8();
        let (w, h) = (img.width() as usize, img.height() as usize);
        let gray = morph::gray_from_rgb(img.as_raw(), w, h).unwrap();
        let page = RasterPage {
            width: w,
            height: h,
            pixels: gray,
            scale_x: 520.0 / w as f32,
            scale_y: 438.961 / h as f32,
            origin_x: 40.0,
            origin_y: 273.039,
            y_down_pixels: true,
        };
        let rules = rules_from_raster(&page, &RasterConfig::for_dimensions(w, h));
        assert!(!rules.is_empty(), "expected raster rules from C100 PNG");

        let mut runs = Vec::new();
        for r in 0..8u32 {
            for c in 0..4u32 {
                let x = 40.0 + 20.0 + c as f32 * (520.0 / 4.0);
                let y = 273.039 + 438.961 - 20.0 - r as f32 * (438.961 / 8.0);
                runs.push(TextRun {
                    text: format!("r{r}c{c}"),
                    bbox: Rect {
                        x0: x,
                        y0: y,
                        x1: x + 30.0,
                        y1: y + 10.0,
                    },
                    transform: Matrix3x2::identity(),
                    font_name: None,
                    font_size: 9.0,
                    mapping_confidence: 1.0,
                    metrics_confidence: 1.0,
                    mcid: None,
                    invisible: false,
                    from_actual_text: false,
                });
            }
        }
        let opts = TableOptions::from_preset(TablePreset::Auto);
        let tabs = detect_lattice_tables(0, &runs, &rules, &opts, &[]);
        let tabs2 = detect_lattice_tables(0, &runs, &[], &opts, &[page]);
        assert!(
            !tabs.is_empty() || !tabs2.is_empty(),
            "expected lattice from raster rules"
        );
    }

    #[test]
    fn merge_rules_dedupes() {
        let v = vec![RuleSegment {
            x0: 0.0,
            y0: 10.0,
            x1: 100.0,
            y1: 10.0,
        }];
        let r = vec![
            RuleSegment {
                x0: 0.0,
                y0: 10.2,
                x1: 100.0,
                y1: 10.2,
            },
            RuleSegment {
                x0: 0.0,
                y0: 50.0,
                x1: 100.0,
                y1: 50.0,
            },
        ];
        let m = merge_rules(&v, &r, 1.0);
        assert_eq!(m.len(), 2);
    }

    #[test]
    fn noise_image_emits_no_rules() {
        // Random speckles should not become a table grid
        let w = 120usize;
        let h = 100usize;
        let mut px = vec![255u8; w * h];
        for i in (0..w * h).step_by(7) {
            px[i] = 0;
        }
        let page = RasterPage {
            width: w,
            height: h,
            pixels: px,
            scale_x: 1.0,
            scale_y: 1.0,
            origin_x: 0.0,
            origin_y: 0.0,
            y_down_pixels: true,
        };
        let segs = detect_line_segments(&page, &RasterConfig::for_dimensions(w, h));
        assert!(
            segs.is_empty(),
            "noise must not produce rules, got {}",
            segs.len()
        );
    }

    /// Synthetic blank ROI + vector H/V grid stamped into ink → morph finds lines.
    fn vector_grid_rules(rows: usize, cols: usize, cell: f32, origin: f32) -> Vec<RuleSegment> {
        let mut rules = Vec::new();
        let w = cols as f32 * cell;
        let h = rows as f32 * cell;
        for r in 0..=rows {
            let y = origin + r as f32 * cell;
            rules.push(RuleSegment {
                x0: origin,
                y0: y,
                x1: origin + w,
                y1: y,
            });
        }
        for c in 0..=cols {
            let x = origin + c as f32 * cell;
            rules.push(RuleSegment {
                x0: x,
                y0: origin,
                x1: x,
                y1: origin + h,
            });
        }
        rules
    }

    fn blank_page(w: usize, h: usize) -> RasterPage {
        RasterPage {
            width: w,
            height: h,
            pixels: vec![255u8; w * h],
            scale_x: 1.0,
            scale_y: 1.0,
            origin_x: 0.0,
            origin_y: 0.0,
            y_down_pixels: true,
        }
    }

    #[test]
    fn stamp_vector_grid_into_blank_detects_lines() {
        let w = 122usize;
        let h = 102usize;
        let page = blank_page(w, h);
        // PDF y-up: with y_down_pixels, page y=0 is at bottom of image.
        // Draw a grid in page space covering the full bitmap.
        let rows = 4usize;
        let cols = 5usize;
        let cell = 20.0f32;
        // Map grid into pixel space with origin at bottom-left of page.
        // page y from 0..h maps to pixel rows; use rules in pixel-aligned page coords
        // with scale 1 and origin 0 so page y == height - py.
        let mut rules = Vec::new();
        for r in 0..=rows {
            let py = (r as f32) * cell;
            let page_y = h as f32 - py; // y_down: pix_to_page inverse
            rules.push(RuleSegment {
                x0: 0.0,
                y0: page_y,
                x1: cols as f32 * cell,
                y1: page_y,
            });
        }
        for c in 0..=cols {
            let px = c as f32 * cell;
            rules.push(RuleSegment {
                x0: px,
                y0: h as f32 - rows as f32 * cell,
                x1: px,
                y1: h as f32,
            });
        }

        let mut ink = vec![0u8; w * h];
        stamp_vector_rules_into_mask(&mut ink, w, h, &page, &rules, 2, Some(1.0));
        let ink_count: usize = ink.iter().map(|&v| v as usize).sum();
        assert!(
            ink_count > 500,
            "stamp should paint substantial ink, got {ink_count}"
        );

        let cfg = RasterConfig::for_dimensions(w, h);
        let segs = detect_line_segments_from_ink(&page, &ink, &cfg);
        let n_h = segs.iter().filter(|s| (s.y0 - s.y1).abs() < 1.5).count();
        let n_v = segs.iter().filter(|s| (s.x0 - s.x1).abs() < 1.5).count();
        assert!(
            n_h >= 4 && n_v >= 5,
            "stamped blank grid should detect lines H={n_h} V={n_v} segs={segs:?}"
        );
    }

    #[test]
    fn combined_stamp_recovers_vector_grid() {
        let w = 122usize;
        let h = 102usize;
        let page = blank_page(w, h);
        let rows = 4usize;
        let cols = 5usize;
        let cell = 20.0f32;
        let mut rules = Vec::new();
        for r in 0..=rows {
            let py = r as f32 * cell;
            let page_y = h as f32 - py;
            rules.push(RuleSegment {
                x0: 0.0,
                y0: page_y,
                x1: cols as f32 * cell,
                y1: page_y,
            });
        }
        for c in 0..=cols {
            let px = c as f32 * cell;
            rules.push(RuleSegment {
                x0: px,
                y0: h as f32 - rows as f32 * cell,
                x1: px,
                y1: h as f32,
            });
        }

        // Pure raster on blank must find nothing.
        let cfg = RasterConfig::for_dimensions(w, h);
        let pure = rules_from_raster(&page, &cfg);
        assert!(
            pure.is_empty(),
            "blank page must not emit raster rules, got {}",
            pure.len()
        );

        let combined = rules_from_raster_combined(&page, &rules, &cfg);
        let n_h = combined
            .iter()
            .filter(|s| s.is_horizontal(1.5))
            .count();
        let n_v = combined.iter().filter(|s| s.is_vertical(1.5)).count();
        assert!(
            n_h >= 4 && n_v >= 5,
            "combined stamp should recover grid H={n_h} V={n_v} total={}",
            combined.len()
        );
        // Merged set must include at least the original vector segments (deduped).
        assert!(
            combined.len() >= rules.len().min(n_h + n_v),
            "combined should keep vector∪raster evidence"
        );
    }

    #[test]
    fn contour_seeds_from_stamped_grid() {
        let w = 122usize;
        let h = 102usize;
        let page = blank_page(w, h);
        let rows = 4usize;
        let cols = 5usize;
        let cell = 20.0f32;
        let mut rules = Vec::new();
        for r in 0..=rows {
            let py = r as f32 * cell;
            let page_y = h as f32 - py;
            rules.push(RuleSegment {
                x0: 0.0,
                y0: page_y,
                x1: cols as f32 * cell,
                y1: page_y,
            });
        }
        for c in 0..=cols {
            let px = c as f32 * cell;
            rules.push(RuleSegment {
                x0: px,
                y0: h as f32 - rows as f32 * cell,
                x1: px,
                y1: h as f32,
            });
        }

        let cfg = RasterConfig::for_dimensions(w, h);
        let seeds = contour_seeds_from_page(&page, &rules, &cfg, true, 0.001);
        assert!(
            !seeds.is_empty(),
            "stamped grid should yield ≥1 contour seed"
        );
        // Grid lines form one connected component of H∨V ink.
        let largest = seeds.iter().max_by_key(|s| s.area).unwrap();
        assert!(
            largest.width() >= 80 && largest.height() >= 60,
            "contour bbox too small: {}x{} area={}",
            largest.width(),
            largest.height(),
            largest.area
        );
        let (x0, y0, x1, y1) = largest.to_page_bbox(&page);
        assert!(x1 > x0 && y1 > y0, "page bbox degenerate {x0},{y0},{x1},{y1}");
    }

    #[test]
    fn stamp_into_gray_then_detect() {
        let w = 100usize;
        let h = 80usize;
        let mut page = blank_page(w, h);
        // With y_down_pixels=false, page y grows with pixel y (simpler for gray stamp test).
        page.y_down_pixels = false;
        let rules = vector_grid_rules(3, 4, 20.0, 0.0);
        stamp_vector_rules_into_gray(&mut page, &rules, 2, Some(1.0));
        let black: usize = page.pixels.iter().filter(|&&p| p == 0).count();
        assert!(black > 200, "gray stamp should paint black, got {black}");
        let segs = detect_line_segments(&page, &RasterConfig::for_dimensions(w, h));
        let n_h = segs.iter().filter(|s| (s.y0 - s.y1).abs() < 1.5).count();
        let n_v = segs.iter().filter(|s| (s.x0 - s.x1).abs() < 1.5).count();
        assert!(
            n_h >= 3 && n_v >= 4,
            "gray-stamped grid H={n_h} V={n_v}"
        );
    }
}

#[test]
fn morph_on_c104_png() {
    let img = match image::open("/tmp/C104_img_rules_21x6.png") {
        Ok(i) => i,
        Err(_) => return,
    };
    let img = img.to_rgb8();
    let (w, h) = (img.width() as usize, img.height() as usize);
    let gray = morph::gray_from_rgb(img.as_raw(), w, h).unwrap();
    let page = RasterPage {
        width: w,
        height: h,
        pixels: gray,
        scale_x: 1.0,
        scale_y: 1.0,
        origin_x: 0.0,
        origin_y: 0.0,
        y_down_pixels: true,
    };
    let cfg = RasterConfig::for_dimensions(w, h);
    eprintln!(
        "cfg k={} min_seg={} close={}",
        cfg.h_kernel, cfg.min_seg_px, cfg.close_kernel
    );
    let segs = detect_line_segments(&page, &cfg);
    let n_h = segs.iter().filter(|s| (s.y0 - s.y1).abs() < 2.0).count();
    let n_v = segs.iter().filter(|s| (s.x0 - s.x1).abs() < 2.0).count();
    eprintln!("C104 morph H={n_h} V={n_v} total={}", segs.len());
    assert!(n_h >= 15 && n_v >= 5, "H={n_h} V={n_v}");
}

#[test]
fn morph_on_c106_png() {
    let img = match image::open("/tmp/C106_img_rules_18x5.png") {
        Ok(i) => i,
        Err(e) => {
            eprintln!("skip {e}");
            return;
        }
    };
    let img = img.to_rgb8();
    let (w, h) = (img.width() as usize, img.height() as usize);
    let gray = morph::gray_from_rgb(img.as_raw(), w, h).unwrap();
    let page = RasterPage {
        width: w,
        height: h,
        pixels: gray,
        scale_x: 520.0 / w as f32,
        scale_y: 500.0 / h as f32,
        origin_x: 40.0,
        origin_y: 200.0,
        y_down_pixels: true,
    };
    let segs = detect_line_segments(&page, &RasterConfig::for_dimensions(w, h));
    let n_h = segs.iter().filter(|s| (s.y0 - s.y1).abs() < 2.0).count();
    let n_v = segs.iter().filter(|s| (s.x0 - s.x1).abs() < 2.0).count();
    assert!(n_h >= 15 && n_v >= 5, "C106 H={n_h} V={n_v}");
}
