//! Detector stage traits (OCP: new extractors plug in without editing callers).
use crate::hybrid::detect_hybrid_tables;
use crate::network::detect_network_tables;
use crate::options::TableOptions;
use crate::raster::RasterPage;
use crate::stream::detect_stream_tables;
use crate::types::Table;
use pdfparser_content::RuleSegment;
use pdfparser_ir::TextRun;

/// One page-level table detector.
pub trait TableDetector {
    /// Stable telemetry name (`lattice`, `hybrid`, `network`, `stream`).
    fn name(&self) -> &'static str;

    /// Run this detector. Unused inputs may be ignored (stream has no rules).
    fn detect(
        &self,
        page_index: u32,
        runs: &[TextRun],
        rules: &[RuleSegment],
        opts: &TableOptions,
        raster_pages: &[RasterPage],
        page_size: Option<(f32, f32)>,
    ) -> Vec<Table>;
}

/// Ruled / lattice detector.
#[derive(Debug, Clone, Copy, Default)]
pub struct LatticeDetector;

impl TableDetector for LatticeDetector {
    fn name(&self) -> &'static str {
        "lattice"
    }

    fn detect(
        &self,
        page_index: u32,
        runs: &[TextRun],
        rules: &[RuleSegment],
        opts: &TableOptions,
        raster_pages: &[RasterPage],
        _page_size: Option<(f32, f32)>,
    ) -> Vec<Table> {
        crate::builders::ruled::detect_ruled_tables(page_index, runs, rules, opts, raster_pages)
    }
}

/// Partial-border hybrid detector.
#[derive(Debug, Clone, Copy, Default)]
pub struct HybridDetector;

impl TableDetector for HybridDetector {
    fn name(&self) -> &'static str {
        "hybrid"
    }

    fn detect(
        &self,
        page_index: u32,
        runs: &[TextRun],
        rules: &[RuleSegment],
        opts: &TableOptions,
        _raster_pages: &[RasterPage],
        _page_size: Option<(f32, f32)>,
    ) -> Vec<Table> {
        detect_hybrid_tables(page_index, runs, rules, opts)
    }
}

/// Production borderless (network) detector.
#[derive(Debug, Clone, Copy, Default)]
pub struct NetworkDetector;

impl TableDetector for NetworkDetector {
    fn name(&self) -> &'static str {
        "network"
    }

    fn detect(
        &self,
        page_index: u32,
        runs: &[TextRun],
        _rules: &[RuleSegment],
        opts: &TableOptions,
        _raster_pages: &[RasterPage],
        _page_size: Option<(f32, f32)>,
    ) -> Vec<Table> {
        detect_network_tables(page_index, runs, opts)
    }
}

/// Classic whitespace stream detector (experimental / LatticeStream only).
#[derive(Debug, Clone, Copy, Default)]
pub struct StreamDetector;

impl TableDetector for StreamDetector {
    fn name(&self) -> &'static str {
        "stream"
    }

    fn detect(
        &self,
        page_index: u32,
        runs: &[TextRun],
        _rules: &[RuleSegment],
        opts: &TableOptions,
        _raster_pages: &[RasterPage],
        _page_size: Option<(f32, f32)>,
    ) -> Vec<Table> {
        detect_stream_tables(page_index, runs, opts)
    }
}

/// Exclusive Engine V2 AutoRouter (proposals in → kept proposals out).
pub trait TableRouter {
    /// Route and filter region proposals.
    fn route(
        &self,
        proposals: Vec<crate::evidence::RegionProposal>,
        median_line_gap: f32,
        policy: &crate::policy::ProposalPolicy,
    ) -> Vec<crate::evidence::RegionProposal>;
}

/// Product exclusive router.
#[derive(Debug, Clone, Copy, Default)]
pub struct ExclusiveAutoRouter;

impl TableRouter for ExclusiveAutoRouter {
    fn route(
        &self,
        proposals: Vec<crate::evidence::RegionProposal>,
        median_line_gap: f32,
        policy: &crate::policy::ProposalPolicy,
    ) -> Vec<crate::evidence::RegionProposal> {
        crate::router::route_proposals(proposals, median_line_gap, policy)
    }
}
