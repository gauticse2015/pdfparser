//! Full-page render via external CLI tools (PR3 spike).
//!
//! Tries, in order: `pdftoppm` (poppler), `mutool` (mupdf), `gs` (ghostscript).
//! No native library link — optional process isolation. Fail-soft if none present.

use super::render::{PageRenderer, ProviderError, RenderSafety};
use crate::raster::RasterPage;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

/// Renders a PDF page to grayscale using host CLI tools.
#[derive(Debug, Clone)]
pub struct ExternalCliPageRenderer {
    /// Absolute path to the PDF file.
    pub pdf_path: PathBuf,
    /// Page width in PDF user units (for RasterPage mapping).
    pub page_width: f32,
    /// Page height in PDF user units.
    pub page_height: f32,
    /// Working directory for temp files (defaults to std::env::temp_dir).
    pub temp_dir: PathBuf,
}

impl ExternalCliPageRenderer {
    /// Create a renderer for one PDF path and page size.
    pub fn new(pdf_path: impl Into<PathBuf>, page_width: f32, page_height: f32) -> Self {
        Self {
            pdf_path: pdf_path.into(),
            page_width: page_width.max(1.0),
            page_height: page_height.max(1.0),
            temp_dir: std::env::temp_dir(),
        }
    }

    /// Which tool would be used, if any.
    pub fn detect_tool() -> Option<&'static str> {
        ["pdftoppm", "mutool", "gs"]
            .into_iter()
            .find(|&tool| which(tool))
    }
}

fn which(bin: &str) -> bool {
    Command::new("which")
        .arg(bin)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

impl PageRenderer for ExternalCliPageRenderer {
    fn render_gray(
        &self,
        page_index: u32,
        dpi: u32,
        safety: &RenderSafety,
    ) -> Result<RasterPage, ProviderError> {
        let dpi = dpi.min(safety.max_dpi).max(36);
        let w_in = self.page_width / 72.0;
        let h_in = self.page_height / 72.0;
        if !safety.allows_page(w_in, h_in, dpi) {
            return Err(ProviderError {
                message: format!("page exceeds RenderSafety at dpi={dpi}"),
            });
        }
        if !self.pdf_path.is_file() {
            return Err(ProviderError {
                message: format!("pdf not found: {}", self.pdf_path.display()),
            });
        }
        let tool = ExternalCliPageRenderer::detect_tool().ok_or_else(|| ProviderError {
            message: "no full-page render CLI (install pdftoppm, mutool, or gs)".into(),
        })?;

        let stamp = Instant::now();
        let prefix = self.temp_dir.join(format!(
            "pdfparser_render_{}_{}",
            std::process::id(),
            page_index
        ));
        let png = render_with_tool(tool, &self.pdf_path, page_index, dpi, &prefix)?;
        if stamp.elapsed() > Duration::from_millis(safety.timeout_ms) {
            let _ = std::fs::remove_file(&png);
            return Err(ProviderError {
                message: format!("render exceeded timeout_ms={}", safety.timeout_ms),
            });
        }
        let page = load_png_as_raster_page(&png, self.page_width, self.page_height)?;
        let _ = std::fs::remove_file(&png);
        // pdftoppm may write prefix-1.png etc — best-effort cleanup
        if let Some(parent) = prefix.parent() {
            if let Ok(rd) = std::fs::read_dir(parent) {
                for e in rd.flatten() {
                    let n = e.file_name().to_string_lossy().into_owned();
                    if n.starts_with(
                        prefix
                            .file_name()
                            .map(|s| s.to_string_lossy())
                            .unwrap_or_default()
                            .as_ref(),
                    ) {
                        let _ = std::fs::remove_file(e.path());
                    }
                }
            }
        }
        Ok(page)
    }
}

fn render_with_tool(
    tool: &str,
    pdf: &Path,
    page_index: u32,
    dpi: u32,
    prefix: &Path,
) -> Result<PathBuf, ProviderError> {
    let page_1based = page_index + 1;
    match tool {
        "pdftoppm" => {
            let status = Command::new("pdftoppm")
                .args([
                    "-gray",
                    "-png",
                    "-r",
                    &dpi.to_string(),
                    "-f",
                    &page_1based.to_string(),
                    "-l",
                    &page_1based.to_string(),
                    "-singlefile",
                ])
                .arg(pdf)
                .arg(prefix)
                .status()
                .map_err(|e| ProviderError {
                    message: format!("pdftoppm spawn: {e}"),
                })?;
            if !status.success() {
                return Err(ProviderError {
                    message: format!("pdftoppm failed: {status}"),
                });
            }
            let png = prefix.with_extension("png");
            if png.is_file() {
                Ok(png)
            } else {
                Err(ProviderError {
                    message: format!("pdftoppm output missing: {}", png.display()),
                })
            }
        }
        "mutool" => {
            let out = prefix.with_extension("png");
            let status = Command::new("mutool")
                .args([
                    "draw",
                    "-F",
                    "png",
                    "-c",
                    "gray",
                    "-r",
                    &dpi.to_string(),
                    "-o",
                ])
                .arg(&out)
                .arg(pdf)
                .arg(page_1based.to_string())
                .status()
                .map_err(|e| ProviderError {
                    message: format!("mutool spawn: {e}"),
                })?;
            if !status.success() || !out.is_file() {
                return Err(ProviderError {
                    message: format!("mutool draw failed: {status}"),
                });
            }
            Ok(out)
        }
        "gs" => {
            let out = prefix.with_extension("png");
            let status = Command::new("gs")
                .args([
                    "-dSAFER",
                    "-dBATCH",
                    "-dNOPAUSE",
                    "-sDEVICE=pnggray",
                    &format!("-r{dpi}"),
                    &format!("-dFirstPage={page_1based}"),
                    &format!("-dLastPage={page_1based}"),
                    &format!("-sOutputFile={}", out.display()),
                ])
                .arg(pdf)
                .status()
                .map_err(|e| ProviderError {
                    message: format!("gs spawn: {e}"),
                })?;
            if !status.success() || !out.is_file() {
                return Err(ProviderError {
                    message: format!("gs failed: {status}"),
                });
            }
            Ok(out)
        }
        other => Err(ProviderError {
            message: format!("unknown tool {other}"),
        }),
    }
}

fn load_png_as_raster_page(
    path: &Path,
    page_width: f32,
    page_height: f32,
) -> Result<RasterPage, ProviderError> {
    let img = image::open(path).map_err(|e| ProviderError {
        message: format!("open png: {e}"),
    })?;
    let gray = img.to_luma8();
    let (w, h) = gray.dimensions();
    if w == 0 || h == 0 {
        return Err(ProviderError {
            message: "empty render".into(),
        });
    }
    Ok(RasterPage {
        width: w as usize,
        height: h as usize,
        pixels: gray.into_raw(),
        scale_x: page_width / w as f32,
        scale_y: page_height / h as f32,
        origin_x: 0.0,
        origin_y: 0.0,
        // PNG row 0 is top; PDF y grows up.
        y_down_pixels: true,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_tool_does_not_panic() {
        let _ = ExternalCliPageRenderer::detect_tool();
    }

    #[test]
    fn missing_pdf_errors() {
        let r = ExternalCliPageRenderer::new("/no/such/file.pdf", 612.0, 792.0);
        let err = r
            .render_gray(0, 72, &RenderSafety::default())
            .expect_err("missing pdf");
        assert!(err.message.contains("not found") || err.message.contains("CLI"));
    }

    /// Live render when a host CLI tool is present (pdftoppm/mutool/gs).
    /// Skips cleanly when none installed — pure-Rust CI stays green.
    #[test]
    fn live_render_soft_gold_when_tool_present() {
        if ExternalCliPageRenderer::detect_tool().is_none() {
            return;
        }
        // Prefer soft-gold real lattice; fall back to synthetic lattice.
        let candidates = [
            "benchmark/corpus/real/35_real_camelot_fuel.pdf",
            "benchmark/corpus/06_table_lattice.pdf",
        ];
        let mut pdf: Option<PathBuf> = None;
        for c in candidates {
            let p = PathBuf::from(c);
            if p.is_file() {
                pdf = Some(p);
                break;
            }
            // tests may run from crate dir
            let alt = PathBuf::from("../..").join(c);
            if alt.is_file() {
                pdf = Some(alt);
                break;
            }
        }
        let Some(pdf) = pdf else {
            return;
        };
        let r = ExternalCliPageRenderer::new(&pdf, 612.0, 792.0);
        let page = r
            .render_gray(0, 72, &RenderSafety::default())
            .expect("pdftoppm/mutool/gs should render soft-gold PDF");
        assert!(page.width > 10 && page.height > 10);
        assert_eq!(page.pixels.len(), page.width * page.height);
        assert!(page.y_down_pixels);
    }
}
