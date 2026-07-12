//! Unified line evidence (vector rules + future raster provenance).

use pdfparser_content::RuleSegment;
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// Provenance of a line segment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum LineSourceKind {
    /// Content-stream stroke (`S`/`s`/…).
    VectorStroke,
    /// Thin filled rect treated as a rule.
    ThinFill,
    /// Morphology on an embedded Image XObject.
    RasterEmbedded,
    /// Morphology on a full-page render (optional feature).
    RasterFullPage,
    /// Expanded Form XObject content (future PR2).
    FormExpanded,
}

/// Axis-aligned segment in page space with provenance.
#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct OrientedSeg {
    /// Start x.
    pub x0: f32,
    /// Start y.
    pub y0: f32,
    /// End x.
    pub x1: f32,
    /// End y.
    pub y1: f32,
    /// Source kind.
    pub source: LineSourceKind,
}

impl OrientedSeg {
    /// Near-horizontal under `tol`.
    pub fn is_horizontal(&self, tol: f32) -> bool {
        (self.y0 - self.y1).abs() <= tol
    }

    /// Near-vertical under `tol`.
    pub fn is_vertical(&self, tol: f32) -> bool {
        (self.x0 - self.x1).abs() <= tol
    }

    /// Euclidean length.
    pub fn len(&self) -> f32 {
        let dx = self.x1 - self.x0;
        let dy = self.y1 - self.y0;
        (dx * dx + dy * dy).sqrt()
    }

    /// Convert to content-stream rule segment (drops provenance).
    pub fn to_rule_segment(&self) -> RuleSegment {
        RuleSegment {
            x0: self.x0,
            y0: self.y0,
            x1: self.x1,
            y1: self.y1,
        }
    }
}

/// Collection of oriented segments for one page.
#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct LineEvidence {
    /// Segments.
    pub segs: Vec<OrientedSeg>,
}

impl LineEvidence {
    /// Iterate segments.
    pub fn iter(&self) -> impl Iterator<Item = &OrientedSeg> {
        self.segs.iter()
    }

    /// Count near-H segments.
    pub fn count_h(&self, tol: f32) -> usize {
        self.segs.iter().filter(|s| s.is_horizontal(tol)).count()
    }

    /// Count near-V segments.
    pub fn count_v(&self, tol: f32) -> usize {
        self.segs.iter().filter(|s| s.is_vertical(tol)).count()
    }

    /// Build from legacy `RuleSegment` list (vector provenance unknown → stroke).
    pub fn from_rules(rules: &[RuleSegment]) -> Self {
        from_rule_segments(rules)
    }

    /// Export as rule segments for legacy lattice/hybrid.
    pub fn to_rule_segments(&self) -> Vec<RuleSegment> {
        self.segs.iter().map(OrientedSeg::to_rule_segment).collect()
    }
}

/// Map content-stream rules into evidence (default provenance = VectorStroke).
pub fn from_rule_segments(rules: &[RuleSegment]) -> LineEvidence {
    LineEvidence {
        segs: rules
            .iter()
            .map(|r| OrientedSeg {
                x0: r.x0,
                y0: r.y0,
                x1: r.x1,
                y1: r.y1,
                source: LineSourceKind::VectorStroke,
            })
            .collect(),
    }
}
