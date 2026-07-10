//! Public options.
use pdfparser_core::ResourceLimits;
use pdfparser_tables::TableOptions;

/// Open options.
#[derive(Debug, Clone, Default)]
pub struct OpenOptions {
    /// Resource limits.
    pub limits: ResourceLimits,
}

/// Text extraction options.
#[derive(Debug, Clone)]
pub struct TextOptions {
    /// Sort into reading order (multi-column aware).
    pub sort_reading_order: bool,
    /// Insert spaces between runs by gap heuristic.
    pub insert_spaces: bool,
    /// Apply page /Rotate to geometry before ordering (R8).
    pub apply_page_rotate: bool,
    /// Include invisible text (Tr=3).
    pub include_invisible: bool,
}

impl Default for TextOptions {
    fn default() -> Self {
        Self {
            sort_reading_order: true,
            insert_spaces: true,
            apply_page_rotate: true,
            include_invisible: true,
        }
    }
}

/// Full document extract options (Phase T).
#[derive(Debug, Clone, Default)]
pub struct ExtractOptions {
    /// Text options.
    pub text: TextOptions,
    /// Table options (default: detect off).
    pub tables: TableOptions,
}
