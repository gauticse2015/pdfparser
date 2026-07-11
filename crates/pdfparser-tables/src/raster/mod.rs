//! Raster line sensing for ruled tables (Camelot-class production pipeline).
//!
//! Pipeline (see [`morph`] for algorithm detail):
//! 1. Grayscale ROI (embedded Image XObject, CTM-mapped)
//! 2. Adaptive threshold → binary ink
//! 3. Morph close (dashed) + multi-scale H/V open
//! 4. RLE extract ∪ projection-profile full-span peaks
//! 5. Span filter + joint-graph + spacing regularity gate
//! 6. Map to page space → [`RuleSegment`]s for the lattice engine
//!
//! Pure geometry/image processing — no PDF I/O. Callers supply pixels + affine.

mod morph;

pub use morph::{
    detect_line_segments, gray_from_rgb, gray_from_rgba, RasterConfig, RasterPage, RasterRule,
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
        assert!(segs.is_empty(), "noise must not produce rules, got {}", segs.len());
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
        eprintln!("cfg k={} min_seg={} close={}", cfg.h_kernel, cfg.min_seg_px, cfg.close_kernel);
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
