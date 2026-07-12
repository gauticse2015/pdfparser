//! Page gray render trait for full-page line sensing (PR3 scaffold).

use crate::raster::RasterPage;
use std::fmt;

/// Error from an optional render backend.
#[derive(Debug, Clone)]
pub struct ProviderError {
    /// Human-readable message.
    pub message: String,
}

impl fmt::Display for ProviderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "render provider: {}", self.message)
    }
}

impl std::error::Error for ProviderError {}

/// Hard caps for full-page render (design `RenderSafety`).
#[derive(Debug, Clone, Copy)]
pub struct RenderSafety {
    /// Max DPI (default 200).
    pub max_dpi: u32,
    /// Max width*height pixels (default 40e6).
    pub max_pixels: u64,
    /// Timeout per page in ms (default 5000).
    pub timeout_ms: u64,
    /// Max pages rendered per document call (default 50).
    pub max_pages_rendered: u32,
}

impl Default for RenderSafety {
    fn default() -> Self {
        Self {
            max_dpi: 200,
            max_pixels: 40_000_000,
            timeout_ms: 5000,
            max_pages_rendered: 50,
        }
    }
}

impl RenderSafety {
    /// Whether a page at `dpi` with size in inches is within caps.
    pub fn allows_page(&self, width_in: f32, height_in: f32, dpi: u32) -> bool {
        if dpi > self.max_dpi || dpi == 0 {
            return false;
        }
        let w = (width_in.max(0.0) * dpi as f32) as u64;
        let h = (height_in.max(0.0) * dpi as f32) as u64;
        w.saturating_mul(h) <= self.max_pixels
    }
}

/// Optional full-page gray renderer (Tier 1).
///
/// Implement with pdfium/skia/etc. behind cargo feature `full-page-render`.
/// Default builds use [`NullPageRenderer`] (always errors → fail-soft).
pub trait PageRenderer: Send + Sync {
    /// Render page to grayscale raster in page space.
    fn render_gray(
        &self,
        page_index: u32,
        dpi: u32,
        safety: &RenderSafety,
    ) -> Result<RasterPage, ProviderError>;
}

/// No-op renderer: always fails so extract continues with vector+embedded only.
#[derive(Debug, Default, Clone, Copy)]
pub struct NullPageRenderer;

impl PageRenderer for NullPageRenderer {
    fn render_gray(
        &self,
        _page_index: u32,
        _dpi: u32,
        _safety: &RenderSafety,
    ) -> Result<RasterPage, ProviderError> {
        Err(ProviderError {
            message: "full-page render not compiled (enable feature full-page-render + backend)"
                .into(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_safety_caps() {
        let s = RenderSafety::default();
        assert!(s.allows_page(8.5, 11.0, 150));
        assert!(!s.allows_page(8.5, 11.0, 300)); // > max_dpi 200
        assert!(!s.allows_page(100.0, 100.0, 200)); // huge pixels
    }

    #[test]
    fn null_renderer_fails_soft() {
        let r = NullPageRenderer;
        assert!(r.render_gray(0, 150, &RenderSafety::default()).is_err());
    }
}
