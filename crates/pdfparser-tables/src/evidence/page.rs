//! Page-level evidence bundle and diagnostics.

use super::line::LineEvidence;
use crate::raster::RasterPage;
use crate::types::TableMethod;
use pdfparser_ir::{Rect, TextRun};
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// Kind of region proposal (Engine V2 router).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum RegionKind {
    /// Joint-rich ruled contour.
    RuledContour,
    /// Partial rules + text.
    PartialRuled,
    /// Borderless text alignment region.
    BorderlessText,
    /// Residual (rare second pass).
    Residual,
}

/// Where a region proposal came from (do not overload score fields for this).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum ProposalOrigin {
    /// Built from a detector candidate table.
    #[default]
    Detector,
    /// Raster/vector contour seed (region hint; may not emit a table alone).
    ContourSeed,
}

/// A candidate table region (may overlap before partition).
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct RegionProposal {
    /// Kind.
    pub kind: RegionKind,
    /// Bounding box in page space.
    pub bbox: Rect,
    /// Line score 0..1 (see design AutoRouter).
    pub line_score: f32,
    /// Text structure score 0..1.
    pub text_score: f32,
    /// Joint count for ruled proposals (real joints when known; 0 = unknown).
    pub joint_count: u32,
    /// Area / page area.
    pub area_frac: f32,
    /// Whitespace / empty-cell estimate 0..1 (ruled chrome gate).
    ///
    /// Default `0.0` when unknown (does not reject). Router rejects ruled when
    /// `whitespace_est >= ProposalPolicy::whitespace_reject`.
    #[cfg_attr(feature = "serde", serde(default))]
    pub whitespace_est: f32,
    /// Provenance of this proposal.
    #[cfg_attr(feature = "serde", serde(default))]
    pub origin: ProposalOrigin,
    /// Indices into the detector candidate list this proposal was built from.
    ///
    /// After K26 merge this may list multiple sources; emit picks the best
    /// unused candidate (identity-based, not loose bbox match).
    #[cfg_attr(feature = "serde", serde(default))]
    pub source_indices: Vec<usize>,
}

/// Per-page method mix counts (telemetry).
#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct MethodMix {
    /// Lattice tables emitted.
    pub lattice: u32,
    /// Hybrid / partial tables.
    pub hybrid: u32,
    /// Stream / network / borderless tables.
    pub stream: u32,
    /// Other methods.
    pub other: u32,
}

impl MethodMix {
    /// Aggregate from tables.
    pub fn from_tables(tables: &[crate::types::Table]) -> Self {
        let mut m = Self::default();
        for t in tables {
            match t.method {
                TableMethod::Lattice => m.lattice += 1,
                TableMethod::Hybrid => m.hybrid += 1,
                TableMethod::Stream => m.stream += 1,
                _ => m.other += 1,
            }
        }
        m
    }

    /// Total tables.
    pub fn total(&self) -> u32 {
        self.lattice + self.hybrid + self.stream + self.other
    }
}

/// Sensing / routing diagnostics for shadow dumps (not control flow).
#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct EvidenceDiagnostics {
    /// Vector RuleSegment count before raster merge.
    pub vector_rule_count: u32,
    /// Embedded/full-page raster pages provided.
    pub raster_page_count: u32,
    /// Text runs on page.
    pub text_run_count: u32,
    /// Near-horizontal segments in LineEvidence.
    pub h_seg_count: u32,
    /// Near-vertical segments.
    pub v_seg_count: u32,
    /// Whether any lattice-strength table was emitted.
    pub strong_lattice_fired: bool,
    /// Method mix of final page tables.
    pub method_mix: MethodMix,
    /// Engine path used: "legacy" or "engine_v2".
    pub engine_path: String,
    /// Free-form notes.
    pub notes: Vec<String>,
}

/// Full page evidence for table detection.
#[derive(Debug, Clone)]
pub struct PageEvidence {
    /// 0-based page index.
    pub page_index: u32,
    /// Page width (user units, post-rotate).
    pub page_width: f32,
    /// Page height (user units, post-rotate).
    pub page_height: f32,
    /// Text runs.
    pub runs: Vec<TextRun>,
    /// Unified lines.
    pub lines: LineEvidence,
    /// Raster pages available for morph (embedded and/or full-page).
    pub raster_pages: Vec<RasterPage>,
    /// Region proposals (filled by router; empty under legacy).
    pub proposals: Vec<RegionProposal>,
    /// Diagnostics.
    pub diagnostics: EvidenceDiagnostics,
}
