//! Morphological H/V line detection — production raster line sensing.
//!
//! # Algorithm (Camelot-class morphology + projection + grid validation)
//!
//! Per embedded image / render ROI:
//!
//! 1. **Preprocess** — validate buffer; auto-invert dark backgrounds; despeckle
//! 2. **Adaptive threshold** — integral-image local mean − bias → binary ink
//! 3. **Gap bridge** — morphological close with a small SE (dashed / broken rules)
//! 4. **Directional open** — long 1×k / k×1 kernels suppress text & noise, keep rules
//! 5. **Multi-scale open** — longer kernel union for thick rules
//! 6. **Run extraction** — RLE with collinear merge + position snap
//! 7. **Projection profiles** — full-span H/V peaks (recovers lines AA/text fragment)
//! 8. **Span filter** — drop short cell underlines / text strokes
//! 9. **Joint graph filter** — drop segments that do not cross orthogonal rules
//! 10. **Regularity gate** — quasi-uniform spacing; reject chart axes / noise
//! 11. **Page-space map** — pixel → user space via origin/scale (CTM unit-square)
//!
//! Designed for born-digital image-painted grids and clean scans. Not OCR.

/// Grayscale page or image region in raster space.
#[derive(Debug, Clone)]
pub struct RasterPage {
    /// Width in pixels.
    pub width: usize,
    /// Height in pixels.
    pub height: usize,
    /// Row-major luminance 0=black … 255=white.
    pub pixels: Vec<u8>,
    /// Page units per pixel (x).
    pub scale_x: f32,
    /// Page units per pixel (y).
    pub scale_y: f32,
    /// Page-space origin of pixel (0,0) (min corner after CTM).
    pub origin_x: f32,
    /// Page-space origin of pixel (0,0) (bottom of image in PDF y-up).
    pub origin_y: f32,
    /// If true, pixel row 0 is top of image (standard bitmap); PDF y grows up.
    pub y_down_pixels: bool,
}

/// Tunables for morphology (sane defaults for ~72–200 DPI table images).
#[derive(Debug, Clone)]
pub struct RasterConfig {
    /// Local window half-size for adaptive threshold (pixels).
    pub adaptive_radius: usize,
    /// Subtracted from local mean (darker → more ink). Typical 8–15.
    pub adaptive_bias: u8,
    /// Horizontal kernel length for open (pixels). ~ page_width / 40.
    pub h_kernel: usize,
    /// Vertical kernel length for open (pixels).
    pub v_kernel: usize,
    /// Min segment length in pixels to emit.
    pub min_seg_px: usize,
    /// Merge collinear runs within this gap (pixels).
    pub merge_gap_px: usize,
    /// Snap tolerance when clustering line positions (pixels).
    pub pos_snap_px: f32,
    /// Small SE size for morphological close (bridge dashed gaps). 0 = off.
    pub close_kernel: usize,
    /// Second-scale open kernel multiplier (0 = single scale). Typical 2.
    pub multi_scale_factor: usize,
    /// Min orthogonal crossings a segment must have to survive joint filter.
    pub min_crossings: usize,
    /// Min H and V lines after filtering to accept the ROI as a table grid.
    pub min_grid_lines: usize,
    /// Min H×V crossing pairs required for the ROI.
    pub min_total_joints: usize,
    /// Auto-invert when mean luminance is below this (dark paper / inverted scans).
    pub invert_mean_below: u8,
    /// Despeckle: remove ink pixels with fewer than this many ink neighbors (0–8).
    /// 0 disables. Typical 2.
    pub despeckle_min_neighbors: u8,
    /// When false, skip joint-graph / regularity gates (debug / soft recovery).
    pub enforce_grid_gate: bool,
}

impl Default for RasterConfig {
    fn default() -> Self {
        Self {
            adaptive_radius: 8,
            adaptive_bias: 10,
            h_kernel: 25,
            v_kernel: 25,
            min_seg_px: 12,
            merge_gap_px: 3,
            pos_snap_px: 1.5,
            close_kernel: 3,
            multi_scale_factor: 2,
            min_crossings: 2,
            min_grid_lines: 3,
            min_total_joints: 6,
            invert_mean_below: 90,
            despeckle_min_neighbors: 2,
            enforce_grid_gate: true,
        }
    }
}

impl RasterConfig {
    /// Build config scaled to image dimensions (production defaults).
    pub fn for_dimensions(width: usize, height: usize) -> Self {
        let dim = width.max(height) as f32;
        let k = ((dim / 35.0).round() as usize).clamp(15, 80);
        let min_seg = ((dim / 50.0).round() as usize).clamp(8, 40);
        Self {
            adaptive_radius: 6,
            adaptive_bias: 12,
            h_kernel: k,
            v_kernel: k,
            min_seg_px: min_seg,
            merge_gap_px: 4.max(min_seg / 8),
            pos_snap_px: 2.0,
            close_kernel: 3,
            multi_scale_factor: 2,
            min_crossings: 2,
            min_grid_lines: 3,
            min_total_joints: 6,
            invert_mean_below: 90,
            despeckle_min_neighbors: 2,
            enforce_grid_gate: true,
        }
    }

    /// Override from [`crate::options::TableOptions`]-style knobs.
    pub fn with_option_floors(
        mut self,
        adaptive_radius: usize,
        adaptive_bias: u8,
        min_kernel: usize,
        min_seg_px: usize,
        merge_gap_px: usize,
        pos_snap_px: f32,
    ) -> Self {
        self.adaptive_radius = adaptive_radius;
        self.adaptive_bias = adaptive_bias;
        self.h_kernel = self.h_kernel.max(min_kernel);
        self.v_kernel = self.v_kernel.max(min_kernel);
        self.min_seg_px = self.min_seg_px.max(min_seg_px);
        self.merge_gap_px = merge_gap_px;
        self.pos_snap_px = pos_snap_px;
        self
    }
}

/// Axis-aligned rule in page user space.
#[derive(Debug, Clone, Copy)]
pub struct RasterRule {
    /// Start x (page space).
    pub x0: f32,
    /// Start y (page space).
    pub y0: f32,
    /// End x (page space).
    pub x1: f32,
    /// End y (page space).
    pub y1: f32,
}

// ─── Pixel-space segments (pre page map) ────────────────────────────────────

#[derive(Clone, Copy, Debug)]
struct PixH {
    y: f32,
    x0: f32,
    x1: f32,
}

#[derive(Clone, Copy, Debug)]
struct PixV {
    x: f32,
    y0: f32,
    y1: f32,
}

/// Detect H/V line segments from a grayscale page.
pub fn detect_line_segments(page: &RasterPage, cfg: &RasterConfig) -> Vec<RasterRule> {
    let Some(ink) = threshold_ink(page, cfg) else {
        return Vec::new();
    };
    detect_line_segments_from_ink(page, &ink, cfg)
}

/// Detect H/V lines after OR-ing near-axis vector rules into the ink mask (K28).
///
/// Pipeline: adaptive threshold → stamp vector H/V → morph close + directional
/// open → RLE / projection → joint + regularity gates → page-space rules.
pub fn detect_line_segments_combined(
    page: &RasterPage,
    vector_rules: &[pdfparser_content::RuleSegment],
    cfg: &RasterConfig,
) -> Vec<RasterRule> {
    let Some(mut ink) = threshold_ink(page, cfg) else {
        return Vec::new();
    };
    stamp_vector_rules_into_mask(
        &mut ink,
        page.width,
        page.height,
        page,
        vector_rules,
        2,
        None,
    );
    detect_line_segments_from_ink(page, &ink, cfg)
}

/// Build binary ink (1 = dark) from grayscale via invert/despeckle/threshold.
///
/// Does **not** morph-close; callers may stamp vectors then close inside
/// [`detect_line_segments_from_ink`].
pub fn threshold_ink(page: &RasterPage, cfg: &RasterConfig) -> Option<Vec<u8>> {
    if page.width < 8 || page.height < 8 || page.pixels.len() < page.width * page.height {
        return None;
    }
    let mut pixels = page.pixels[..page.width * page.height].to_vec();
    maybe_invert(&mut pixels, cfg.invert_mean_below);
    if cfg.despeckle_min_neighbors > 0 {
        despeckle_luma(
            &mut pixels,
            page.width,
            page.height,
            cfg.despeckle_min_neighbors,
        );
    }
    Some(adaptive_threshold(
        &pixels,
        page.width,
        page.height,
        cfg,
    ))
}

/// Map page-space point to continuous pixel coordinates (row 0 = top when
/// [`RasterPage::y_down_pixels`]).
pub fn page_to_pix(page: &RasterPage, x: f32, y: f32) -> (f32, f32) {
    let sx = if page.scale_x.abs() < 1e-9 {
        1.0
    } else {
        page.scale_x
    };
    let sy = if page.scale_y.abs() < 1e-9 {
        1.0
    } else {
        page.scale_y
    };
    let px = (x - page.origin_x) / sx;
    let py = if page.y_down_pixels {
        page.height as f32 - (y - page.origin_y) / sy
    } else {
        (y - page.origin_y) / sy
    };
    (px, py)
}

/// Stamp near-horizontal / near-vertical vector rules into a binary ink mask.
///
/// Each H/V segment is painted as `stroke_px` thick ink (`1`) in the pixel space
/// of `page` (origin/scale/y orientation). Non-axis-aligned segments are skipped.
/// `axis_tol_page` defaults to `max(1.0, 2 * max(|scale_x|, |scale_y|))`.
pub fn stamp_vector_rules_into_mask(
    ink: &mut [u8],
    width: usize,
    height: usize,
    page: &RasterPage,
    rules: &[pdfparser_content::RuleSegment],
    stroke_px: usize,
    axis_tol_page: Option<f32>,
) {
    if ink.len() < width * height || width == 0 || height == 0 {
        return;
    }
    let tol = axis_tol_page.unwrap_or_else(|| {
        let s = page.scale_x.abs().max(page.scale_y.abs()).max(1e-6);
        (2.0 * s).max(1.0)
    });
    let half = stroke_px.max(1).saturating_sub(1) / 2;
    let half_hi = stroke_px.max(1) / 2;

    for r in rules {
        if r.is_horizontal(tol) {
            let (px0, py0) = page_to_pix(page, r.x0, r.y0);
            let (px1, py1) = page_to_pix(page, r.x1, r.y1);
            let y = ((py0 + py1) * 0.5).round() as i32;
            let x_a = px0.min(px1).floor() as i32;
            let x_b = px0.max(px1).ceil() as i32;
            for dy in -(half as i32)..=(half_hi as i32) {
                let yy = y + dy;
                if yy < 0 || yy >= height as i32 {
                    continue;
                }
                let row = yy as usize * width;
                for x in x_a..=x_b {
                    if x >= 0 && x < width as i32 {
                        ink[row + x as usize] = 1;
                    }
                }
            }
        } else if r.is_vertical(tol) {
            let (px0, py0) = page_to_pix(page, r.x0, r.y0);
            let (px1, py1) = page_to_pix(page, r.x1, r.y1);
            let x = ((px0 + px1) * 0.5).round() as i32;
            let y_a = py0.min(py1).floor() as i32;
            let y_b = py0.max(py1).ceil() as i32;
            for dx in -(half as i32)..=(half_hi as i32) {
                let xx = x + dx;
                if xx < 0 || xx >= width as i32 {
                    continue;
                }
                for y in y_a..=y_b {
                    if y >= 0 && y < height as i32 {
                        ink[y as usize * width + xx as usize] = 1;
                    }
                }
            }
        }
    }
}

/// Stamp near-H/V vector rules as black strokes into a grayscale [`RasterPage`].
///
/// Useful for synthetic tests and callers that prefer pre-threshold painting.
/// Stroke value is `0` (ink) on a typical white (`255`) page.
#[allow(dead_code)] // exercised in raster unit tests via re-export
pub fn stamp_vector_rules_into_gray(
    page: &mut RasterPage,
    rules: &[pdfparser_content::RuleSegment],
    stroke_px: usize,
    axis_tol_page: Option<f32>,
) {
    let n = page.width.saturating_mul(page.height);
    if page.pixels.len() < n || n == 0 {
        return;
    }
    // Reuse binary stamp into a temp mask, then darken gray where mask is ink.
    let mut ink = vec![0u8; n];
    stamp_vector_rules_into_mask(
        &mut ink,
        page.width,
        page.height,
        page,
        rules,
        stroke_px,
        axis_tol_page,
    );
    for i in 0..n {
        if ink[i] != 0 {
            page.pixels[i] = 0;
        }
    }
}

/// Pixel-space axis-aligned contour seed (connected component of line ink).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContourSeed {
    /// Inclusive min x (pixels).
    pub px0: u32,
    /// Inclusive min y (pixels).
    pub py0: u32,
    /// Exclusive max x (pixels).
    pub px1: u32,
    /// Exclusive max y (pixels).
    pub py1: u32,
    /// Number of ink pixels in the component.
    pub area: usize,
}

impl ContourSeed {
    /// Width in pixels.
    #[allow(dead_code)] // used by diagnostics / external callers
    pub fn width(&self) -> u32 {
        self.px1.saturating_sub(self.px0)
    }
    /// Height in pixels.
    #[allow(dead_code)]
    pub fn height(&self) -> u32 {
        self.py1.saturating_sub(self.py0)
    }

    /// Map seed AABB to page-space corners `(x0,y0,x1,y1)` (min/max).
    pub fn to_page_bbox(&self, page: &RasterPage) -> (f32, f32, f32, f32) {
        let (x0, y0) = pix_to_page(page, self.px0 as f32, self.py0 as f32);
        let (x1, y1) = pix_to_page(
            page,
            self.px1.saturating_sub(1) as f32,
            self.py1.saturating_sub(1) as f32,
        );
        (x0.min(x1), y0.min(y1), x0.max(x1), y0.max(y1))
    }
}

/// Build directional H/V line masks from threshold ink (optional vector stamp).
///
/// Returns `(h_mask, v_mask, ink_after_close)` for contour / joint consumers.
pub fn combined_line_masks(
    page: &RasterPage,
    vector_rules: &[pdfparser_content::RuleSegment],
    cfg: &RasterConfig,
    stamp: bool,
) -> Option<(Vec<u8>, Vec<u8>, Vec<u8>)> {
    let mut ink = threshold_ink(page, cfg)?;
    if stamp && !vector_rules.is_empty() {
        stamp_vector_rules_into_mask(
            &mut ink,
            page.width,
            page.height,
            page,
            vector_rules,
            2,
            None,
        );
    }
    let ink = if cfg.close_kernel >= 2 {
        morph_close(&ink, page.width, page.height, cfg.close_kernel)
    } else {
        ink
    };
    let h_kernel = cfg.h_kernel.max(3);
    let v_kernel = cfg.v_kernel.max(3);
    let mut h_mask = morph_open_horizontal(&ink, page.width, page.height, h_kernel);
    let mut v_mask = morph_open_vertical(&ink, page.width, page.height, v_kernel);
    if cfg.multi_scale_factor >= 2 {
        let hk2 = (h_kernel * cfg.multi_scale_factor).min(page.width.max(3));
        let vk2 = (v_kernel * cfg.multi_scale_factor).min(page.height.max(3));
        if hk2 > h_kernel + 4 {
            let h2 = morph_open_horizontal(&ink, page.width, page.height, hk2);
            or_masks(&mut h_mask, &h2);
        }
        if vk2 > v_kernel + 4 {
            let v2 = morph_open_vertical(&ink, page.width, page.height, vk2);
            or_masks(&mut v_mask, &v2);
        }
    }
    Some((h_mask, v_mask, ink))
}

/// Connected-component AABBs on a binary mask (`!= 0` = foreground).
///
/// 4-connected flood fill; components with `area < min_area` are dropped.
pub fn find_contour_seeds(
    mask: &[u8],
    width: usize,
    height: usize,
    min_area: usize,
) -> Vec<ContourSeed> {
    if width == 0 || height == 0 || mask.len() < width * height {
        return Vec::new();
    }
    let mut seen = vec![false; width * height];
    let mut seeds = Vec::new();
    let mut stack = Vec::new();

    for y0 in 0..height {
        for x0 in 0..width {
            let i0 = y0 * width + x0;
            if mask[i0] == 0 || seen[i0] {
                continue;
            }
            stack.clear();
            stack.push((x0, y0));
            seen[i0] = true;
            let mut area = 0usize;
            let mut min_x = x0;
            let mut max_x = x0;
            let mut min_y = y0;
            let mut max_y = y0;
            while let Some((x, y)) = stack.pop() {
                area += 1;
                min_x = min_x.min(x);
                max_x = max_x.max(x);
                min_y = min_y.min(y);
                max_y = max_y.max(y);
                for (nx, ny) in [
                    (x.wrapping_sub(1), y),
                    (x + 1, y),
                    (x, y.wrapping_sub(1)),
                    (x, y + 1),
                ] {
                    if nx >= width || ny >= height {
                        continue;
                    }
                    let j = ny * width + nx;
                    if mask[j] != 0 && !seen[j] {
                        seen[j] = true;
                        stack.push((nx, ny));
                    }
                }
            }
            if area >= min_area {
                seeds.push(ContourSeed {
                    px0: min_x as u32,
                    py0: min_y as u32,
                    px1: (max_x + 1) as u32,
                    py1: (max_y + 1) as u32,
                    area,
                });
            }
        }
    }
    seeds
}

/// Contour region seeds from H∨V morph masks (K6), with optional K28 stamp.
///
/// `min_area_frac` is relative to `width * height` (clamped to at least 16 px).
pub fn contour_seeds_from_page(
    page: &RasterPage,
    vector_rules: &[pdfparser_content::RuleSegment],
    cfg: &RasterConfig,
    stamp: bool,
    min_area_frac: f32,
) -> Vec<ContourSeed> {
    let Some((h_mask, v_mask, _)) = combined_line_masks(page, vector_rules, cfg, stamp) else {
        return Vec::new();
    };
    let mut union = h_mask;
    or_masks(&mut union, &v_mask);
    let area = page.width.saturating_mul(page.height) as f32;
    let min_area = ((area * min_area_frac.max(0.0)).round() as usize).max(16);
    find_contour_seeds(&union, page.width, page.height, min_area)
}

/// Detect H/V line segments from a pre-thresholded binary ink mask.
///
/// Applies morph close, directional open, RLE, projection, and grid gates.
pub fn detect_line_segments_from_ink(
    page: &RasterPage,
    ink: &[u8],
    cfg: &RasterConfig,
) -> Vec<RasterRule> {
    if page.width < 8 || page.height < 8 || ink.len() < page.width * page.height {
        return Vec::new();
    }

    let ink = if cfg.close_kernel >= 2 {
        morph_close(ink, page.width, page.height, cfg.close_kernel)
    } else {
        ink.to_vec()
    };

    let h_kernel = cfg.h_kernel.max(3);
    let v_kernel = cfg.v_kernel.max(3);
    let mut h_mask = morph_open_horizontal(&ink, page.width, page.height, h_kernel);
    let mut v_mask = morph_open_vertical(&ink, page.width, page.height, v_kernel);

    // Multi-scale: longer kernels capture thick/long rules; union with primary.
    if cfg.multi_scale_factor >= 2 {
        let hk2 = (h_kernel * cfg.multi_scale_factor).min(page.width.max(3));
        let vk2 = (v_kernel * cfg.multi_scale_factor).min(page.height.max(3));
        if hk2 > h_kernel + 4 {
            let h2 = morph_open_horizontal(&ink, page.width, page.height, hk2);
            or_masks(&mut h_mask, &h2);
        }
        if vk2 > v_kernel + 4 {
            let v2 = morph_open_vertical(&ink, page.width, page.height, vk2);
            or_masks(&mut v_mask, &v2);
        }
    }

    // Morph RLE on directional masks (good for clean vector-like renders).
    let mut h_segs = extract_h_pix(
        &h_mask,
        page.width,
        page.height,
        cfg.min_seg_px,
        cfg.merge_gap_px,
        cfg.pos_snap_px,
    );
    let mut v_segs = extract_v_pix(
        &v_mask,
        page.width,
        page.height,
        cfg.min_seg_px,
        cfg.merge_gap_px,
        cfg.pos_snap_px,
    );

    // Projection-profile lines on the ink mask: full-span rules even when text
    // or AA breaks morph runs into short pieces (common in image-painted grids).
    let proj_h = projection_h_lines(&ink, page.width, page.height, cfg.pos_snap_px);
    let proj_v = projection_v_lines(&ink, page.width, page.height, cfg.pos_snap_px);
    h_segs.extend(proj_h);
    v_segs.extend(proj_v);
    h_segs = snap_merge_h(&h_segs, cfg.pos_snap_px.max(2.0) * 1.5);
    v_segs = snap_merge_v(&v_segs, cfg.pos_snap_px.max(2.0) * 1.5);

    // Span filter: full table rules span most of the image; cell underlines /
    // text strokes are short.
    h_segs = filter_by_span_h(&h_segs, page.width);
    v_segs = filter_by_span_v(&v_segs, page.height);

    // Joint graph: table rules cross; chart axes / deco rarely form dense joints.
    if cfg.enforce_grid_gate {
        let (h_keep, v_keep, joints) = joint_filter(
            &h_segs,
            &v_segs,
            cfg.min_crossings,
            cfg.pos_snap_px.max(1.0) + 1.0,
        );
        h_segs = h_keep;
        v_segs = v_keep;

        if h_segs.len() < cfg.min_grid_lines
            || v_segs.len() < cfg.min_grid_lines
            || joints < cfg.min_total_joints
        {
            return Vec::new();
        }

        // Collapse near-duplicate line positions (double-pixel rules, AA pairs).
        h_segs = snap_merge_h(&h_segs, cfg.pos_snap_px.max(2.0) * 1.5);
        v_segs = snap_merge_v(&v_segs, cfg.pos_snap_px.max(2.0) * 1.5);

        // Regularity gate: true grids have quasi-uniform line spacing.
        if !passes_regularity(&h_segs, &v_segs) {
            // Soft recovery: drop short outliers by re-filtering at higher span
            // threshold then re-check (text underlines that barely passed).
            h_segs = filter_by_span_frac(&h_segs, page.width, 0.70);
            v_segs = filter_by_span_frac_v(&v_segs, page.height, 0.70);
            let (h2, v2, j2) = joint_filter(
                &h_segs,
                &v_segs,
                cfg.min_crossings,
                cfg.pos_snap_px.max(1.0) + 1.0,
            );
            h_segs = snap_merge_h(&h2, cfg.pos_snap_px.max(2.0) * 1.5);
            v_segs = snap_merge_v(&v2, cfg.pos_snap_px.max(2.0) * 1.5);
            if h_segs.len() < cfg.min_grid_lines
                || v_segs.len() < cfg.min_grid_lines
                || j2 < cfg.min_total_joints
                || !passes_regularity(&h_segs, &v_segs)
            {
                return Vec::new();
            }
        }
    }

    let mut out = Vec::with_capacity(h_segs.len() + v_segs.len());
    for s in h_segs {
        let (px0, py0) = pix_to_page(page, s.x0, s.y);
        let (px1, py1) = pix_to_page(page, s.x1, s.y);
        let y_mid = (py0 + py1) * 0.5;
        out.push(RasterRule {
            x0: px0.min(px1),
            y0: y_mid,
            x1: px0.max(px1),
            y1: y_mid,
        });
    }
    for s in v_segs {
        let (px0, py0) = pix_to_page(page, s.x, s.y0);
        let (px1, py1) = pix_to_page(page, s.x, s.y1);
        let x_mid = (px0 + px1) * 0.5;
        out.push(RasterRule {
            x0: x_mid,
            y0: py0.min(py1),
            x1: x_mid,
            y1: py0.max(py1),
        });
    }
    out
}

/// Build grayscale from packed RGB (3 bytes/pixel).
pub fn gray_from_rgb(rgb: &[u8], width: usize, height: usize) -> Option<Vec<u8>> {
    if rgb.len() < width * height * 3 {
        return None;
    }
    let mut g = vec![0u8; width * height];
    for i in 0..width * height {
        let r = rgb[i * 3] as u32;
        let gg = rgb[i * 3 + 1] as u32;
        let b = rgb[i * 3 + 2] as u32;
        // Rec. 601 luma
        g[i] = ((r * 30 + gg * 59 + b * 11) / 100) as u8;
    }
    Some(g)
}

/// Build grayscale from packed RGBA (4 bytes/pixel).
pub fn gray_from_rgba(rgba: &[u8], width: usize, height: usize) -> Option<Vec<u8>> {
    if rgba.len() < width * height * 4 {
        return None;
    }
    let mut g = vec![0u8; width * height];
    for i in 0..width * height {
        let r = rgba[i * 4] as u32;
        let gg = rgba[i * 4 + 1] as u32;
        let b = rgba[i * 4 + 2] as u32;
        let a = rgba[i * 4 + 3] as u32;
        let luma = (r * 30 + gg * 59 + b * 11) / 100;
        // Composite on white
        g[i] = ((luma * a + 255 * (255 - a)) / 255) as u8;
    }
    Some(g)
}

// ─── Preprocess ─────────────────────────────────────────────────────────────

fn maybe_invert(pixels: &mut [u8], mean_below: u8) {
    if pixels.is_empty() || mean_below == 0 {
        return;
    }
    let sum: u64 = pixels.iter().map(|&p| p as u64).sum();
    let mean = (sum / pixels.len() as u64) as u8;
    if mean < mean_below {
        for p in pixels.iter_mut() {
            *p = 255 - *p;
        }
    }
}

/// Remove isolated dark speckles on a white-ish page (operates on luma, pre-threshold).
fn despeckle_luma(pixels: &mut [u8], w: usize, h: usize, min_neighbors: u8) {
    // Only suppress very dark isolated pixels; leave line ink alone.
    let dark_thr = 80u8;
    let mut kill = Vec::new();
    for y in 1..h.saturating_sub(1) {
        for x in 1..w.saturating_sub(1) {
            let i = y * w + x;
            if pixels[i] > dark_thr {
                continue;
            }
            let mut n = 0u8;
            for dy in -1i32..=1 {
                for dx in -1i32..=1 {
                    if dx == 0 && dy == 0 {
                        continue;
                    }
                    let j = ((y as i32 + dy) as usize) * w + (x as i32 + dx) as usize;
                    if pixels[j] <= dark_thr {
                        n += 1;
                    }
                }
            }
            if n < min_neighbors {
                kill.push(i);
            }
        }
    }
    for i in kill {
        pixels[i] = 255;
    }
}

// ─── Threshold ──────────────────────────────────────────────────────────────

fn adaptive_threshold(pixels: &[u8], w: usize, h: usize, cfg: &RasterConfig) -> Vec<u8> {
    let r = cfg.adaptive_radius.max(1);
    let bias = cfg.adaptive_bias as i32;
    // Integral image for O(1) local means
    let mut integ = vec![0u32; (w + 1) * (h + 1)];
    for y in 0..h {
        let mut row_sum = 0u32;
        for x in 0..w {
            row_sum += pixels[y * w + x] as u32;
            integ[(y + 1) * (w + 1) + (x + 1)] = integ[y * (w + 1) + (x + 1)] + row_sum;
        }
    }
    let mut out = vec![0u8; w * h];
    for y in 0..h {
        let y0 = y.saturating_sub(r);
        let y1 = (y + r + 1).min(h);
        for x in 0..w {
            let x0 = x.saturating_sub(r);
            let x1 = (x + r + 1).min(w);
            let area = ((x1 - x0) * (y1 - y0)) as u32;
            let sum = integ[y1 * (w + 1) + x1] + integ[y0 * (w + 1) + x0]
                - integ[y0 * (w + 1) + x1]
                - integ[y1 * (w + 1) + x0];
            let mean = (sum / area.max(1)) as i32;
            let px = pixels[y * w + x] as i32;
            out[y * w + x] = if px < mean - bias { 1 } else { 0 };
        }
    }
    out
}

// ─── Morphology ─────────────────────────────────────────────────────────────

fn morph_close(ink: &[u8], w: usize, h: usize, k: usize) -> Vec<u8> {
    // Close = dilate then erode; bridges small gaps in dashed rules.
    let d = dilate_square(ink, w, h, k);
    erode_square(&d, w, h, k)
}

fn morph_open_horizontal(ink: &[u8], w: usize, h: usize, k: usize) -> Vec<u8> {
    let k = k.max(1);
    let eroded = erode_h(ink, w, h, k);
    dilate_h(&eroded, w, h, k)
}

fn morph_open_vertical(ink: &[u8], w: usize, h: usize, k: usize) -> Vec<u8> {
    let k = k.max(1);
    let eroded = erode_v(ink, w, h, k);
    dilate_v(&eroded, w, h, k)
}

fn or_masks(dst: &mut [u8], src: &[u8]) {
    let n = dst.len().min(src.len());
    for i in 0..n {
        if src[i] != 0 {
            dst[i] = 1;
        }
    }
}

fn erode_square(src: &[u8], w: usize, h: usize, k: usize) -> Vec<u8> {
    let half = k / 2;
    let mut out = vec![0u8; w * h];
    for y in 0..h {
        for x in 0..w {
            let mut ok = true;
            'outer: for dy in 0..k {
                for dx in 0..k {
                    let yy = y + dy;
                    let xx = x + dx;
                    if yy < half || xx < half || yy - half >= h || xx - half >= w {
                        ok = false;
                        break 'outer;
                    }
                    if src[(yy - half) * w + (xx - half)] == 0 {
                        ok = false;
                        break 'outer;
                    }
                }
            }
            out[y * w + x] = if ok { 1 } else { 0 };
        }
    }
    out
}

fn dilate_square(src: &[u8], w: usize, h: usize, k: usize) -> Vec<u8> {
    let half = k / 2;
    let mut out = vec![0u8; w * h];
    for y in 0..h {
        for x in 0..w {
            if src[y * w + x] == 0 {
                continue;
            }
            for dy in 0..k {
                for dx in 0..k {
                    let yy = y + dy;
                    let xx = x + dx;
                    if yy < half || xx < half {
                        continue;
                    }
                    let sy = yy - half;
                    let sx = xx - half;
                    if sy < h && sx < w {
                        out[sy * w + sx] = 1;
                    }
                }
            }
        }
    }
    out
}

fn erode_h(src: &[u8], w: usize, h: usize, k: usize) -> Vec<u8> {
    let half = k / 2;
    let mut out = vec![0u8; w * h];
    for y in 0..h {
        // Sliding window: count ink in kernel; O(w) per row with prefix.
        let row = &src[y * w..(y + 1) * w];
        let mut pref = vec![0u32; w + 1];
        for x in 0..w {
            pref[x + 1] = pref[x] + row[x] as u32;
        }
        for x in 0..w {
            let x0 = x.saturating_sub(half);
            let x1 = (x + half + 1).min(w);
            // Full kernel must fit for strict erode equivalence with earlier impl
            if x < half || x + half >= w {
                out[y * w + x] = 0;
                continue;
            }
            let sum = pref[x1] - pref[x0];
            out[y * w + x] = if sum == k as u32 { 1 } else { 0 };
        }
    }
    out
}

fn dilate_h(src: &[u8], w: usize, h: usize, k: usize) -> Vec<u8> {
    let half = k / 2;
    let mut out = vec![0u8; w * h];
    for y in 0..h {
        for x in 0..w {
            if src[y * w + x] == 0 {
                continue;
            }
            let x0 = x.saturating_sub(half);
            let x1 = (x + half + 1).min(w);
            for sx in x0..x1 {
                out[y * w + sx] = 1;
            }
        }
    }
    out
}

fn erode_v(src: &[u8], w: usize, h: usize, k: usize) -> Vec<u8> {
    let half = k / 2;
    let mut out = vec![0u8; w * h];
    for x in 0..w {
        let mut pref = vec![0u32; h + 1];
        for y in 0..h {
            pref[y + 1] = pref[y] + src[y * w + x] as u32;
        }
        for y in 0..h {
            if y < half || y + half >= h {
                out[y * w + x] = 0;
                continue;
            }
            let y0 = y - half;
            let y1 = y + half + 1;
            let sum = pref[y1] - pref[y0];
            out[y * w + x] = if sum == k as u32 { 1 } else { 0 };
        }
    }
    out
}

fn dilate_v(src: &[u8], w: usize, h: usize, k: usize) -> Vec<u8> {
    let half = k / 2;
    let mut out = vec![0u8; w * h];
    for y in 0..h {
        for x in 0..w {
            if src[y * w + x] == 0 {
                continue;
            }
            let y0 = y.saturating_sub(half);
            let y1 = (y + half + 1).min(h);
            for sy in y0..y1 {
                out[sy * w + x] = 1;
            }
        }
    }
    out
}

// ─── Run extraction (pixel space) ───────────────────────────────────────────

fn extract_h_pix(
    mask: &[u8],
    w: usize,
    h: usize,
    min_len: usize,
    merge_gap: usize,
    pos_snap: f32,
) -> Vec<PixH> {
    let mut raw: Vec<(f32, f32, f32)> = Vec::new(); // y, x0, x1
    for y in 0..h {
        let mut x = 0usize;
        while x < w {
            if mask[y * w + x] == 0 {
                x += 1;
                continue;
            }
            let x0 = x;
            while x < w && mask[y * w + x] != 0 {
                x += 1;
            }
            let x1 = x;
            if x1 - x0 >= min_len {
                raw.push((y as f32 + 0.5, x0 as f32, x1 as f32));
            }
        }
    }
    raw.sort_by(|a, b| {
        a.0.partial_cmp(&b.0)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
    });
    let mut clusters: Vec<(f32, Vec<(f32, f32)>)> = Vec::new();
    for (y, x0, x1) in raw {
        if let Some((cy, ranges)) = clusters.last_mut() {
            if (y - *cy).abs() <= pos_snap {
                ranges.push((x0, x1));
                *cy = (*cy + y) * 0.5;
                continue;
            }
        }
        clusters.push((y, vec![(x0, x1)]));
    }
    let mut out = Vec::new();
    for (y, mut ranges) in clusters {
        ranges.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
        let mut merged: Vec<(f32, f32)> = Vec::new();
        for (x0, x1) in ranges {
            if let Some((_, last_x1)) = merged.last_mut() {
                if x0 <= *last_x1 + merge_gap as f32 {
                    *last_x1 = (*last_x1).max(x1);
                    continue;
                }
            }
            merged.push((x0, x1));
        }
        for (x0, x1) in merged {
            if (x1 - x0) < min_len as f32 {
                continue;
            }
            out.push(PixH { y, x0, x1 });
        }
    }
    out
}

fn extract_v_pix(
    mask: &[u8],
    w: usize,
    h: usize,
    min_len: usize,
    merge_gap: usize,
    pos_snap: f32,
) -> Vec<PixV> {
    let mut raw: Vec<(f32, f32, f32)> = Vec::new(); // x, y0, y1
    for x in 0..w {
        let mut y = 0usize;
        while y < h {
            if mask[y * w + x] == 0 {
                y += 1;
                continue;
            }
            let y0 = y;
            while y < h && mask[y * w + x] != 0 {
                y += 1;
            }
            let y1 = y;
            if y1 - y0 >= min_len {
                raw.push((x as f32 + 0.5, y0 as f32, y1 as f32));
            }
        }
    }
    raw.sort_by(|a, b| {
        a.0.partial_cmp(&b.0)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
    });
    let mut clusters: Vec<(f32, Vec<(f32, f32)>)> = Vec::new();
    for (x, y0, y1) in raw {
        if let Some((cx, ranges)) = clusters.last_mut() {
            if (x - *cx).abs() <= pos_snap {
                ranges.push((y0, y1));
                *cx = (*cx + x) * 0.5;
                continue;
            }
        }
        clusters.push((x, vec![(y0, y1)]));
    }
    let mut out = Vec::new();
    for (x, mut ranges) in clusters {
        ranges.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
        let mut merged: Vec<(f32, f32)> = Vec::new();
        for (y0, y1) in ranges {
            if let Some((_, last_y1)) = merged.last_mut() {
                if y0 <= *last_y1 + merge_gap as f32 {
                    *last_y1 = (*last_y1).max(y1);
                    continue;
                }
            }
            merged.push((y0, y1));
        }
        for (y0, y1) in merged {
            if (y1 - y0) < min_len as f32 {
                continue;
            }
            out.push(PixV { x, y0, y1 });
        }
    }
    out
}

// ─── Projection profiles (full-span line peaks) ─────────────────────────────

/// Rows whose ink fraction exceeds a high threshold → full-width H rules.
fn projection_h_lines(ink: &[u8], w: usize, h: usize, pos_snap: f32) -> Vec<PixH> {
    if w == 0 || h == 0 {
        return Vec::new();
    }
    let mut row_frac = vec![0.0f32; h];
    for y in 0..h {
        let mut s = 0u32;
        for x in 0..w {
            s += ink[y * w + x] as u32;
        }
        row_frac[y] = s as f32 / w as f32;
    }
    // Peak rows: very high ink density (solid rules). Lower thr picks up chart
    // gridlines and text bands; 0.72 keeps born-digital table rules (~0.9+).
    let thr = 0.72f32;
    let mut peaks: Vec<f32> = Vec::new();
    let mut y = 0usize;
    while y < h {
        if row_frac[y] < thr {
            y += 1;
            continue;
        }
        let mut wsum = 0.0f32;
        let mut mass = 0.0f32;
        while y < h && row_frac[y] >= thr * 0.85 {
            wsum += y as f32 * row_frac[y];
            mass += row_frac[y];
            y += 1;
        }
        if mass > 0.0 {
            peaks.push(wsum / mass);
        }
    }
    // Drop peaks that are too close (AA double-thick)
    peaks = collapse_peaks(peaks, pos_snap.max(1.5));
    peaks
        .into_iter()
        .map(|py| PixH {
            y: py,
            x0: 0.0,
            x1: w as f32,
        })
        .collect()
}

fn projection_v_lines(ink: &[u8], w: usize, h: usize, pos_snap: f32) -> Vec<PixV> {
    if w == 0 || h == 0 {
        return Vec::new();
    }
    let mut col_frac = vec![0.0f32; w];
    for x in 0..w {
        let mut s = 0u32;
        for y in 0..h {
            s += ink[y * w + x] as u32;
        }
        col_frac[x] = s as f32 / h as f32;
    }
    let thr = 0.72f32;
    let mut peaks: Vec<f32> = Vec::new();
    let mut x = 0usize;
    while x < w {
        if col_frac[x] < thr {
            x += 1;
            continue;
        }
        let mut wsum = 0.0f32;
        let mut mass = 0.0f32;
        while x < w && col_frac[x] >= thr * 0.85 {
            wsum += x as f32 * col_frac[x];
            mass += col_frac[x];
            x += 1;
        }
        if mass > 0.0 {
            peaks.push(wsum / mass);
        }
    }
    peaks = collapse_peaks(peaks, pos_snap.max(1.5));
    peaks
        .into_iter()
        .map(|px| PixV {
            x: px,
            y0: 0.0,
            y1: h as f32,
        })
        .collect()
}

fn collapse_peaks(mut peaks: Vec<f32>, snap: f32) -> Vec<f32> {
    if peaks.is_empty() {
        return peaks;
    }
    peaks.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let mut out = vec![peaks[0]];
    for &p in &peaks[1..] {
        if p - out.last().unwrap() <= snap {
            let last = out.last_mut().unwrap();
            *last = (*last + p) * 0.5;
        } else {
            out.push(p);
        }
    }
    out
}

// ─── Span / snap cleanup ────────────────────────────────────────────────────

fn filter_by_span_h(segs: &[PixH], img_w: usize) -> Vec<PixH> {
    if segs.is_empty() {
        return Vec::new();
    }
    let max_len = segs.iter().map(|s| s.x1 - s.x0).fold(0.0f32, f32::max);
    // Prefer near-full-width rules. Partial cell underlines are much shorter.
    let floor = (max_len * 0.88).max(img_w as f32 * 0.65).min(max_len);
    segs.iter()
        .filter(|s| (s.x1 - s.x0) >= floor * 0.98)
        .copied()
        .collect()
}

fn filter_by_span_v(segs: &[PixV], img_h: usize) -> Vec<PixV> {
    if segs.is_empty() {
        return Vec::new();
    }
    let max_len = segs.iter().map(|s| s.y1 - s.y0).fold(0.0f32, f32::max);
    let floor = (max_len * 0.88).max(img_h as f32 * 0.65).min(max_len);
    segs.iter()
        .filter(|s| (s.y1 - s.y0) >= floor * 0.98)
        .copied()
        .collect()
}

fn filter_by_span_frac(segs: &[PixH], img_w: usize, frac: f32) -> Vec<PixH> {
    let floor = img_w as f32 * frac;
    segs.iter()
        .filter(|s| (s.x1 - s.x0) >= floor)
        .copied()
        .collect()
}

fn filter_by_span_frac_v(segs: &[PixV], img_h: usize, frac: f32) -> Vec<PixV> {
    let floor = img_h as f32 * frac;
    segs.iter()
        .filter(|s| (s.y1 - s.y0) >= floor)
        .copied()
        .collect()
}

fn snap_merge_h(segs: &[PixH], snap: f32) -> Vec<PixH> {
    if segs.is_empty() {
        return Vec::new();
    }
    let mut sorted = segs.to_vec();
    sorted.sort_by(|a, b| a.y.partial_cmp(&b.y).unwrap_or(std::cmp::Ordering::Equal));
    let mut out: Vec<PixH> = Vec::new();
    for s in sorted {
        if let Some(last) = out.last_mut() {
            if (s.y - last.y).abs() <= snap {
                last.y = (last.y + s.y) * 0.5;
                last.x0 = last.x0.min(s.x0);
                last.x1 = last.x1.max(s.x1);
                continue;
            }
        }
        out.push(s);
    }
    out
}

fn snap_merge_v(segs: &[PixV], snap: f32) -> Vec<PixV> {
    if segs.is_empty() {
        return Vec::new();
    }
    let mut sorted = segs.to_vec();
    sorted.sort_by(|a, b| a.x.partial_cmp(&b.x).unwrap_or(std::cmp::Ordering::Equal));
    let mut out: Vec<PixV> = Vec::new();
    for s in sorted {
        if let Some(last) = out.last_mut() {
            if (s.x - last.x).abs() <= snap {
                last.x = (last.x + s.x) * 0.5;
                last.y0 = last.y0.min(s.y0);
                last.y1 = last.y1.max(s.y1);
                continue;
            }
        }
        out.push(s);
    }
    out
}

// ─── Joint graph + regularity ───────────────────────────────────────────────

fn joint_filter(
    h: &[PixH],
    v: &[PixV],
    min_cross: usize,
    tol: f32,
) -> (Vec<PixH>, Vec<PixV>, usize) {
    if h.is_empty() || v.is_empty() {
        return (Vec::new(), Vec::new(), 0);
    }
    let mut h_counts = vec![0usize; h.len()];
    let mut v_counts = vec![0usize; v.len()];
    for (i, hs) in h.iter().enumerate() {
        for (j, vs) in v.iter().enumerate() {
            // Crossing: V.x in H span, H.y in V span (with tol for thickness)
            let in_x = vs.x >= hs.x0 - tol && vs.x <= hs.x1 + tol;
            let in_y = hs.y >= vs.y0 - tol && hs.y <= vs.y1 + tol;
            if in_x && in_y {
                h_counts[i] += 1;
                v_counts[j] += 1;
            }
        }
    }
    let h_keep: Vec<PixH> = h
        .iter()
        .zip(h_counts.iter())
        .filter(|(_, &c)| c >= min_cross)
        .map(|(s, _)| *s)
        .collect();
    let v_keep: Vec<PixV> = v
        .iter()
        .zip(v_counts.iter())
        .filter(|(_, &c)| c >= min_cross)
        .map(|(s, _)| *s)
        .collect();

    // Re-count joints on kept set
    let mut joints2 = 0usize;
    for hs in &h_keep {
        for vs in &v_keep {
            let in_x = vs.x >= hs.x0 - tol && vs.x <= hs.x1 + tol;
            let in_y = hs.y >= vs.y0 - tol && hs.y <= vs.y1 + tol;
            if in_x && in_y {
                joints2 += 1;
            }
        }
    }
    (h_keep, v_keep, joints2)
}

/// Reject irregular deco (single axis + ticks) that survived joint filter.
fn passes_regularity(h: &[PixH], v: &[PixV]) -> bool {
    if h.len() < 3 || v.len() < 3 {
        return false;
    }
    // Span coverage: longest H should cover most of the V extent
    let min_vx = v.iter().map(|s| s.x).fold(f32::INFINITY, f32::min);
    let max_vx = v.iter().map(|s| s.x).fold(f32::NEG_INFINITY, f32::max);
    let min_hy = h.iter().map(|s| s.y).fold(f32::INFINITY, f32::min);
    let max_hy = h.iter().map(|s| s.y).fold(f32::NEG_INFINITY, f32::max);
    let v_span = (max_vx - min_vx).max(1.0);
    let h_span = (max_hy - min_hy).max(1.0);

    let max_h_len = h.iter().map(|s| s.x1 - s.x0).fold(0.0f32, f32::max);
    let max_v_len = v.iter().map(|s| s.y1 - s.y0).fold(0.0f32, f32::max);

    // Longest rules must span a meaningful fraction of the grid frame
    if max_h_len < v_span * 0.45 || max_v_len < h_span * 0.45 {
        return false;
    }

    // Spacing CV: true grids have moderately regular gaps (not random ticks)
    let h_ok = spacing_reasonable(h.iter().map(|s| s.y).collect());
    let v_ok = spacing_reasonable(v.iter().map(|s| s.x).collect());
    h_ok && v_ok
}

fn spacing_reasonable(mut coords: Vec<f32>) -> bool {
    if coords.len() < 3 {
        return true;
    }
    coords.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    // Dedup near-identical
    let mut uniq = vec![coords[0]];
    for &c in &coords[1..] {
        if c - uniq.last().unwrap() > 1.0 {
            uniq.push(c);
        }
    }
    if uniq.len() < 3 {
        return true;
    }
    let gaps: Vec<f32> = uniq.windows(2).map(|w| w[1] - w[0]).collect();
    let mean = gaps.iter().sum::<f32>() / gaps.len() as f32;
    if mean < 1.0 {
        return false;
    }
    let var = gaps
        .iter()
        .map(|g| {
            let d = g - mean;
            d * d
        })
        .sum::<f32>()
        / gaps.len() as f32;
    let cv = (var.sqrt()) / mean;
    // Allow irregular multi-header tables (CV up to ~1.2) but reject pure noise
    cv < 1.35
}

fn pix_to_page(page: &RasterPage, px: f32, py: f32) -> (f32, f32) {
    let x = page.origin_x + px * page.scale_x;
    let y = if page.y_down_pixels {
        page.origin_y + (page.height as f32 - py) * page.scale_y
    } else {
        page.origin_y + py * page.scale_y
    };
    (x, y)
}

#[cfg(test)]
mod unit_tests {
    use super::*;

    fn grid_page(rows: usize, cols: usize, cell: usize, line_w: usize) -> RasterPage {
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
    fn synthetic_grid_detected() {
        let page = grid_page(5, 4, 24, 2);
        let segs = detect_line_segments(
            &page,
            &RasterConfig::for_dimensions(page.width, page.height),
        );
        let n_h = segs.iter().filter(|s| (s.y0 - s.y1).abs() < 1.5).count();
        let n_v = segs.iter().filter(|s| (s.x0 - s.x1).abs() < 1.5).count();
        assert!(n_h >= 5, "H={n_h} segs={}", segs.len());
        assert!(n_v >= 4, "V={n_v}");
    }

    #[test]
    fn chart_axes_rejected() {
        // Two axes + a few tick marks — not a table grid
        let w = 200usize;
        let h = 150usize;
        let mut px = vec![255u8; w * h];
        // bottom H axis
        for x in 20..180 {
            for dy in 0..2 {
                px[(h - 20 + dy) * w + x] = 0;
            }
        }
        // left V axis
        for y in 20..h - 20 {
            for dx in 0..2 {
                px[y * w + 20 + dx] = 0;
            }
        }
        // ticks
        for t in 0..5 {
            let x = 20 + t * 30;
            for y in h - 25..h - 18 {
                px[y * w + x] = 0;
            }
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
            "chart axes must not emit table rules, got {}",
            segs.len()
        );
    }

    #[test]
    fn dashed_h_lines_bridged() {
        // Grid with dashed H lines (gaps of 2px every 8px)
        let rows = 4usize;
        let cols = 3usize;
        let cell = 30usize;
        let w = cols * cell + 2;
        let h = rows * cell + 2;
        let mut px = vec![255u8; w * h];
        for r in 0..=rows {
            let y = r * cell;
            for x in 0..w {
                // dash: draw 6, skip 2
                if x % 8 < 6 {
                    px[y * w + x] = 0;
                    if y + 1 < h {
                        px[(y + 1) * w + x] = 0;
                    }
                }
            }
        }
        for c in 0..=cols {
            let x = c * cell;
            for y in 0..h {
                px[y * w + x] = 0;
                if x + 1 < w {
                    px[y * w + x + 1] = 0;
                }
            }
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
        let mut cfg = RasterConfig::for_dimensions(w, h);
        cfg.close_kernel = 3;
        cfg.min_seg_px = 10;
        let segs = detect_line_segments(&page, &cfg);
        let n_h = segs.iter().filter(|s| (s.y0 - s.y1).abs() < 1.5).count();
        assert!(
            n_h >= 3,
            "dashed H should bridge, H={n_h} total={}",
            segs.len()
        );
    }
}
