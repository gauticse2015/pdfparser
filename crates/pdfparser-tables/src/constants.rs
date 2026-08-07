//! Named geometry / page constants shared across detectors and policy.
//!
//! Keep magic numbers that appear in more than one module here so letter-page
//! stand-ins and axis-align tolerances cannot drift.

/// US Letter page area (user units²) used when real media-box is unknown.
pub const LETTER_PAGE_AREA: f32 = 612.0 * 792.0;

/// Default letter width (user units).
pub const LETTER_PAGE_WIDTH: f32 = 612.0;

/// Default letter height (user units).
pub const LETTER_PAGE_HEIGHT: f32 = 792.0;

/// Fallback median text-line gap when page stats are unavailable (K26).
pub const ROUTER_MEDIAN_LINE_GAP: f32 = 12.0;

/// Minimum width for a 2-column lattice to count as "strong" (not a corner strip).
pub const STRONG_LATTICE_2COL_MIN_WIDTH: f32 = 140.0;

/// Minimum rows for a strong 2-column lattice.
pub const STRONG_LATTICE_2COL_MIN_ROWS: u32 = 4;
