//! Public table IR.
use pdfparser_ir::Rect;
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// Strategy provenance tags.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum PipelineId {
    /// Structure.
    S1Structure,
    /// Lattice.
    S2Lattice,
    /// Stream.
    S3Stream,
    /// Hybrid.
    S4Hybrid,
    /// Form discriminator.
    P1FormDisc,
    /// Dense numeric.
    P2DenseNumeric,
    /// Overflow cells.
    P3OverflowCell,
    /// Side-by-side / anti over-seg.
    P4SideBySide,
    /// Superscript recovery.
    P5Superscript,
    /// Multi-page stitch.
    D1Stitch,
}

/// Detection method.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum TableMethod {
    /// Tagged structure.
    Structure,
    /// Ruled lattice.
    Lattice,
    /// Whitespace stream.
    Stream,
    /// Hybrid.
    Hybrid,
    /// Dense numeric refine.
    DenseNumeric,
    /// Superscript recovery.
    SuperscriptRecovered,
    /// Form-like (rarely emitted).
    FormLayout,
}

/// One table cell.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct TableCell {
    /// Row index (0 = top).
    pub row: u32,
    /// Column index (0 = left).
    pub col: u32,
    /// Row span.
    pub rowspan: u32,
    /// Col span.
    pub colspan: u32,
    /// Geometry.
    pub bbox: Rect,
    /// Cell text (R9 geometry assign).
    pub text: String,
    /// Header row flag.
    pub is_header: bool,
    /// Per-cell confidence.
    pub confidence: f32,
}

/// Extracted table.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Table {
    /// Bounding box.
    pub bbox: Rect,
    /// Page index (0-based).
    pub page: u32,
    /// Method.
    pub method: TableMethod,
    /// Confidence 0..1.
    pub confidence: f32,
    /// Row count.
    pub rows: u32,
    /// Column count.
    pub cols: u32,
    /// Cells (sparse OK; typically full grid).
    pub cells: Vec<TableCell>,
    /// Header row count.
    pub header_rows: u32,
    /// Multi-page flags.
    pub continued_from_previous_page: bool,
    /// Multi-page flags.
    pub continued_to_next_page: bool,
    /// Logical table id after D1 stitch (shared across fragments).
    pub logical_table_id: Option<u32>,
    /// Strategy tags.
    pub strategy_provenance: Vec<PipelineId>,
    /// Notes.
    pub notes: Vec<String>,
}
