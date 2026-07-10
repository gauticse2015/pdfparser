//! Stable-ish IR types for pdfparser (Phase T freeze subset).
#![deny(missing_docs)]

use std::fmt;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// Schema version for exported extract documents.
pub const SCHEMA_VERSION: u32 = 1;

/// PDF object identifier (num, generation).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct ObjectId {
    /// Object number.
    pub num: u32,
    /// Generation.
    pub gen: u16,
}

/// 2D point in user space (or upright export space).
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Point {
    /// X coordinate.
    pub x: f32,
    /// Y coordinate.
    pub y: f32,
}

/// Axis-aligned rectangle.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Rect {
    /// Left.
    pub x0: f32,
    /// Bottom (PDF y-up).
    pub y0: f32,
    /// Right.
    pub x1: f32,
    /// Top.
    pub y1: f32,
}

impl Rect {
    /// Empty rect at origin.
    pub fn zero() -> Self {
        Self {
            x0: 0.0,
            y0: 0.0,
            x1: 0.0,
            y1: 0.0,
        }
    }

    /// Width.
    pub fn width(&self) -> f32 {
        (self.x1 - self.x0).abs()
    }

    /// Height.
    pub fn height(&self) -> f32 {
        (self.y1 - self.y0).abs()
    }

    /// Center Y.
    pub fn y_center(&self) -> f32 {
        (self.y0 + self.y1) * 0.5
    }

    /// Union with another rect.
    pub fn union(self, o: Rect) -> Rect {
        Rect {
            x0: self.x0.min(o.x0),
            y0: self.y0.min(o.y0),
            x1: self.x1.max(o.x1),
            y1: self.y1.max(o.y1),
        }
    }

    /// Corners in order BL, BR, TR, TL.
    pub fn corners(self) -> [Point; 4] {
        [
            Point {
                x: self.x0,
                y: self.y0,
            },
            Point {
                x: self.x1,
                y: self.y0,
            },
            Point {
                x: self.x1,
                y: self.y1,
            },
            Point {
                x: self.x0,
                y: self.y1,
            },
        ]
    }

    /// Bounding rect from points.
    pub fn from_points(pts: impl IntoIterator<Item = Point>) -> Rect {
        let mut iter = pts.into_iter();
        let Some(first) = iter.next() else {
            return Rect::zero();
        };
        let mut r = Rect {
            x0: first.x,
            y0: first.y,
            x1: first.x,
            y1: first.y,
        };
        for p in iter {
            r.x0 = r.x0.min(p.x);
            r.y0 = r.y0.min(p.y);
            r.x1 = r.x1.max(p.x);
            r.y1 = r.y1.max(p.y);
        }
        r
    }
}

/// Affine transform [a b c d e f] as PDF / Tm / CTM.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Matrix3x2 {
    /// Components a,b,c,d,e,f.
    pub m: [f32; 6],
}

impl Default for Matrix3x2 {
    fn default() -> Self {
        Self::identity()
    }
}

impl Matrix3x2 {
    /// Identity.
    pub fn identity() -> Self {
        Self {
            m: [1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
        }
    }

    /// Multiply self * other (apply other first, then self) — PDF concat order for `cm`.
    pub fn concat(self, other: Matrix3x2) -> Matrix3x2 {
        let a = self.m;
        let b = other.m;
        Matrix3x2 {
            m: [
                a[0] * b[0] + a[1] * b[2],
                a[0] * b[1] + a[1] * b[3],
                a[2] * b[0] + a[3] * b[2],
                a[2] * b[1] + a[3] * b[3],
                a[4] * b[0] + a[5] * b[2] + b[4],
                a[4] * b[1] + a[5] * b[3] + b[5],
            ],
        }
    }

    /// Transform point (row vector * matrix).
    pub fn apply(&self, x: f32, y: f32) -> Point {
        let m = self.m;
        Point {
            x: m[0] * x + m[2] * y + m[4],
            y: m[1] * x + m[3] * y + m[5],
        }
    }
}

/// Stable warning codes for extract consumers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum WarningCode {
    /// Unknown content operator skipped.
    UnknownOperator,
    /// Missing ToUnicode / encoding gap.
    MissingToUnicode,
    /// Glyph mapped to U+FFFD.
    UnknownGlyph,
    /// Reading order fell back to paint order.
    ReadingOrderFallbackPaint,
    /// Page skipped in recoverable mode.
    PageSkipped,
    /// Unsupported feature.
    Unsupported,
    /// Encryption encountered.
    Encryption,
    /// Soft limit / partial content.
    LimitSoft,
    /// Other.
    Other,
}

impl fmt::Display for WarningCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

/// Extract warning.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct ExtractWarning {
    /// Code.
    pub code: WarningCode,
    /// Optional page index (0-based).
    pub page: Option<u32>,
    /// Human message.
    pub message: String,
    /// Recoverable flag.
    pub recoverable: bool,
}

/// Positioned text run (paint-order IR core).
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct TextRun {
    /// Unicode text.
    pub text: String,
    /// Axis-aligned bbox.
    pub bbox: Rect,
    /// CTM * Tm at run start.
    pub transform: Matrix3x2,
    /// Font resource name if known.
    pub font_name: Option<String>,
    /// Font size approximation in user space.
    pub font_size: f32,
    /// Unicode mapping confidence 0..1.
    pub mapping_confidence: f32,
    /// Geometry/metrics confidence 0..1.
    pub metrics_confidence: f32,
    /// Optional MCID.
    pub mcid: Option<u32>,
    /// Text rendered invisible (Tr=3).
    pub invisible: bool,
    /// From ActualText.
    pub from_actual_text: bool,
}

/// Page element (Phase T: text only in practice).
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(tag = "type", rename_all = "snake_case"))]
pub enum Element {
    /// Text run.
    Text(TextRun),
}

/// Document metadata snapshot.
#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct DocumentMetadata {
    /// Title.
    pub title: Option<String>,
    /// Author.
    pub author: Option<String>,
    /// Producer.
    pub producer: Option<String>,
    /// PDF version string.
    pub pdf_version: Option<String>,
    /// Page count.
    pub page_count: u32,
}

/// One extracted page (Phase T text focus).
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct ExtractedPage {
    /// 0-based index.
    pub index: u32,
    /// Media box.
    pub media_box: Rect,
    /// Crop box if different.
    pub crop_box: Option<Rect>,
    /// Page /Rotate.
    pub rotate: i32,
    /// Plain text (reading order when requested).
    pub text: String,
    /// Paint-order elements.
    pub elements: Vec<Element>,
    /// Per-page warnings.
    pub warnings: Vec<ExtractWarning>,
}

/// Full extract document.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct ExtractedDocument {
    /// Schema version.
    pub schema_version: u32,
    /// Metadata.
    pub metadata: DocumentMetadata,
    /// Pages.
    pub pages: Vec<ExtractedPage>,
    /// Warnings.
    pub warnings: Vec<ExtractWarning>,
    /// Partial flag.
    pub partial: bool,
}
