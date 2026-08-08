//! Page evidence layers for the table engine (Engine V2).
//!
//! Decouples sensing inputs from builders for diagnostics and exclusive routing.
//! P1.2: borrowed [`PageEvidence`] on the diagnostics path only. Product Auto/Fast
//! detect must not construct evidence until P3.2.

mod line;
mod page;

pub use line::{LineEvidence, LineSourceKind, OrientedSeg};
pub use page::{
    EvidenceDiagnostics, MethodMix, PageEvidence, PageEvidenceOwned, ProposalOrigin, RegionKind,
    RegionProposal,
};

use pdfparser_content::RuleSegment;
use pdfparser_ir::TextRun;

use crate::raster::RasterPage;

/// Build borrowed page evidence from the same inputs the product orchestrator consumes.
///
/// Diagnostics wrapper only (P1.2). Does not clone runs or raster pixels.
/// Coordinates are expected in post-`/Rotate` page space (see extract).
pub fn page_evidence_from_inputs<'a>(
    page_index: u32,
    page_width: f32,
    page_height: f32,
    runs: &'a [TextRun],
    rules: &'a [RuleSegment],
    raster_pages: &'a [RasterPage],
) -> PageEvidence<'a> {
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
        runs,
        rules,
        lines,
        raster_pages,
        proposals: Vec::new(),
        diagnostics,
    }
}

/// Clone [`PageEvidence`] for dump. Returns `None` unless both flags are set.
///
/// Product detect must not call this. CLI `--dump-evidence` sets
/// `shadow_diagnostics` and requests dump.
pub fn page_evidence_owned_if_dump(
    evidence: &PageEvidence<'_>,
    shadow_diagnostics: bool,
    dump: bool,
) -> Option<PageEvidenceOwned> {
    if shadow_diagnostics && dump {
        Some(evidence.to_owned_dump())
    } else {
        None
    }
}
