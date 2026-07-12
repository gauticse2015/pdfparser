//! Page evidence layers for the table engine (Engine V2).
//!
//! Decouples sensing inputs from builders for diagnostics and exclusive routing.

mod line;
mod page;

pub use line::{LineEvidence, LineSourceKind, OrientedSeg};
pub use page::{
    EvidenceDiagnostics, MethodMix, PageEvidence, ProposalOrigin, RegionKind, RegionProposal,
};

use pdfparser_content::RuleSegment;
use pdfparser_ir::TextRun;

use crate::raster::RasterPage;

/// Build page evidence from the same inputs the product orchestrator consumes.
///
/// Coordinates are expected in post-`/Rotate` page space (see extract).
pub fn page_evidence_from_inputs(
    page_index: u32,
    page_width: f32,
    page_height: f32,
    runs: &[TextRun],
    rules: &[RuleSegment],
    raster_pages: &[RasterPage],
) -> PageEvidence {
    let lines = line::from_rule_segments(rules);
    let diagnostics = EvidenceDiagnostics {
        vector_rule_count: rules.len() as u32,
        raster_page_count: raster_pages.len() as u32,
        text_run_count: runs.len() as u32,
        h_seg_count: lines.count_h(1.5) as u32,
        v_seg_count: lines.count_v(1.5) as u32,
        ..EvidenceDiagnostics::default()
    };

    PageEvidence {
        page_index,
        page_width,
        page_height,
        runs: runs.to_vec(),
        lines,
        raster_pages: raster_pages.to_vec(),
        proposals: Vec::new(),
        diagnostics,
    }
}
