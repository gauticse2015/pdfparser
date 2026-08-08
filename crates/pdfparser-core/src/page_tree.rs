//! Page tree walk with inheritance.
use crate::error::{Error, Result};
use crate::limits::hard_max;
use crate::rect_from_object;
use lopdf::{Dictionary, Document, Object, ObjectId};
use pdfparser_ir::Rect;

/// Fallback MediaBox when the page tree omits `/MediaBox` (US Letter).
const DEFAULT_MEDIA_WIDTH: f32 = 612.0;
const DEFAULT_MEDIA_HEIGHT: f32 = 792.0;

/// Page `/Resources` as stored on [`PageInfo`] (A2.14).
///
/// Indirect references still inherit down the page tree. Inline dictionaries
/// are snapshotted (page-local or inherited from a parent `/Pages` node) and
/// never replace a nearer `/Resources` reference.
#[derive(Debug, Clone, Default)]
pub enum PageResources {
    /// No `/Resources` on this page or an ancestor.
    #[default]
    None,
    /// Indirect `/Resources` dictionary (page-local or inherited).
    Reference(ObjectId),
    /// Owned snapshot of an inline `/Resources` dictionary.
    Inline(Dictionary),
}

impl PageResources {
    /// Indirect dict id when `/Resources` is a reference.
    pub fn as_reference(&self) -> Option<ObjectId> {
        match self {
            Self::Reference(id) => Some(*id),
            _ => None,
        }
    }

    /// Borrow the inline snapshot when `/Resources` is a dictionary.
    pub fn as_inline(&self) -> Option<&Dictionary> {
        match self {
            Self::Inline(dict) => Some(dict),
            _ => None,
        }
    }
}

/// One page summary.
#[derive(Debug, Clone)]
pub struct PageInfo {
    /// Page object id.
    pub id: ObjectId,
    /// MediaBox.
    pub media_box: Rect,
    /// CropBox.
    pub crop_box: Option<Rect>,
    /// /Rotate.
    pub rotate: i32,
    /// `/Resources`: inherited/local reference, or owned inline snapshot.
    pub resources: PageResources,
}

/// Flattened page list.
#[derive(Debug, Clone)]
pub struct PageTree {
    pages: Vec<PageInfo>,
}

impl PageTree {
    /// Build from document catalog.
    pub fn from_document(doc: &Document) -> Result<Self> {
        let root = doc
            .catalog()
            .map_err(|e| Error::Syntax(format!("catalog: {e}")))?;
        let pages_ref = root
            .get(b"Pages")
            .map_err(|e| Error::Syntax(format!("Pages: {e}")))?;
        let pages_id = match pages_ref {
            Object::Reference(r) => *r,
            _ => return Err(Error::Syntax("Pages not a reference".into())),
        };
        let mut pages = Vec::new();
        let inherit = Inherit::default();
        walk_pages(doc, pages_id, &inherit, &mut pages, 0)?;
        Ok(Self { pages })
    }

    /// Length.
    pub fn len(&self) -> usize {
        self.pages.len()
    }

    /// Empty.
    pub fn is_empty(&self) -> bool {
        self.pages.is_empty()
    }

    /// Get page.
    pub fn get(&self, index: usize) -> Option<&PageInfo> {
        self.pages.get(index)
    }

    /// Iter.
    pub fn iter(&self) -> impl Iterator<Item = &PageInfo> {
        self.pages.iter()
    }
}

#[derive(Clone, Default)]
struct Inherit {
    media_box: Option<Rect>,
    crop_box: Option<Rect>,
    rotate: Option<i32>,
    /// Inherited indirect `/Resources` (unchanged reference path).
    resources: Option<ObjectId>,
    /// Inherited inline `/Resources` snapshot; cleared by a nearer reference.
    inline_resources: Option<Dictionary>,
}

fn walk_pages(
    doc: &Document,
    id: ObjectId,
    inherit: &Inherit,
    out: &mut Vec<PageInfo>,
    depth: u32,
) -> Result<()> {
    if depth > hard_max::MAX_NESTING_DEPTH {
        return Err(Error::LimitExceeded {
            kind: crate::limits::LimitKind::NestingDepth,
        });
    }
    let dict = doc
        .get_dictionary(id)
        .map_err(|e| Error::Syntax(e.to_string()))?;
    let type_name = dict
        .get(b"Type")
        .ok()
        .and_then(|o| match o {
            Object::Name(n) => Some(String::from_utf8_lossy(n).into_owned()),
            _ => None,
        })
        .unwrap_or_default();

    let mut next = inherit.clone();
    if let Ok(obj) = dict.get(b"MediaBox") {
        next.media_box = rect_from_object(obj);
    }
    if let Ok(obj) = dict.get(b"CropBox") {
        next.crop_box = rect_from_object(obj);
    }
    if let Ok(Object::Integer(r)) = dict.get(b"Rotate") {
        next.rotate = Some(*r as i32);
    }
    match dict.get(b"Resources") {
        Ok(Object::Reference(r)) => {
            next.resources = Some(*r);
            next.inline_resources = None;
        }
        Ok(Object::Dictionary(d)) => {
            next.inline_resources = Some(d.clone());
            // Nearest `/Resources` wins: do not keep a farther ancestor's ref.
            next.resources = None;
        }
        _ => {}
    }

    if type_name == "Page" || (type_name.is_empty() && dict.get(b"Kids").is_err()) {
        let media = next.media_box.unwrap_or(Rect {
            x0: 0.0,
            y0: 0.0,
            x1: DEFAULT_MEDIA_WIDTH,
            y1: DEFAULT_MEDIA_HEIGHT,
        });
        let resources = if let Some(res_id) = next.resources {
            PageResources::Reference(res_id)
        } else if let Some(inline) = next.inline_resources {
            PageResources::Inline(inline)
        } else {
            PageResources::None
        };
        out.push(PageInfo {
            id,
            media_box: media,
            crop_box: next.crop_box,
            rotate: next.rotate.unwrap_or(0),
            resources,
        });
        return Ok(());
    }

    if let Ok(Object::Array(kids)) = dict.get(b"Kids") {
        for kid in kids {
            if let Object::Reference(kid_id) = kid {
                walk_pages(doc, *kid_id, &next, out, depth + 1)?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{PdfDocument, ResourceLimits};

    fn assemble_pdf(objects: &[&str]) -> Vec<u8> {
        let mut body = String::from("%PDF-1.4\n");
        let mut offsets = Vec::with_capacity(objects.len());
        for obj in objects {
            offsets.push(body.len());
            body.push_str(obj);
            if !obj.ends_with('\n') {
                body.push('\n');
            }
        }
        let xref_pos = body.len();
        let n = objects.len() + 1;
        body.push_str(&format!("xref\n0 {n}\n0000000000 65535 f \n"));
        for off in offsets {
            body.push_str(&format!("{off:010} 00000 n \n"));
        }
        body.push_str(&format!(
            "trailer\n<< /Size {n} /Root 1 0 R >>\nstartxref\n{xref_pos}\n%%EOF\n"
        ));
        body.into_bytes()
    }

    fn load_pages(objects: &[&str]) -> PageTree {
        let bytes = assemble_pdf(objects);
        let doc = PdfDocument::from_bytes(&bytes, ResourceLimits::default()).unwrap();
        doc.pages.clone()
    }

    #[test]
    fn stores_inline_page_resources() {
        let pages = load_pages(&[
            "1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n",
            "2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n",
            "3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] \
             /Resources << /Font << /F1 4 0 R >> >> >>\nendobj\n",
            "4 0 obj\n<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>\nendobj\n",
        ]);
        let page = pages.get(0).unwrap();
        let inline = page
            .resources
            .as_inline()
            .expect("page-local inline /Resources");
        assert!(inline.get(b"Font").is_ok());
        assert!(page.resources.as_reference().is_none());
    }

    #[test]
    fn inherits_resources_reference() {
        let pages = load_pages(&[
            "1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n",
            "2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 /Resources 4 0 R >>\nendobj\n",
            "3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] >>\nendobj\n",
            "4 0 obj\n<< /Font << /F1 5 0 R >> >>\nendobj\n",
            "5 0 obj\n<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>\nendobj\n",
        ]);
        let page = pages.get(0).unwrap();
        assert_eq!(page.resources.as_reference(), Some((4, 0)));
        assert!(page.resources.as_inline().is_none());
    }

    #[test]
    fn inherits_inline_pages_resources() {
        let pages = load_pages(&[
            "1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n",
            "2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 \
             /Resources << /Font << /F1 4 0 R >> >> >>\nendobj\n",
            "3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] >>\nendobj\n",
            "4 0 obj\n<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>\nendobj\n",
        ]);
        let page = pages.get(0).unwrap();
        let inline = page
            .resources
            .as_inline()
            .expect("inherited inline /Resources from /Pages");
        assert!(inline.get(b"Font").is_ok());
        assert!(page.resources.as_reference().is_none());
    }

    #[test]
    fn page_inline_resources_override_inherited_reference() {
        let pages = load_pages(&[
            "1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n",
            "2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 /Resources 4 0 R >>\nendobj\n",
            "3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] \
             /Resources << /XObject << >> >> >>\nendobj\n",
            "4 0 obj\n<< /Font << /F1 5 0 R >> >>\nendobj\n",
            "5 0 obj\n<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>\nendobj\n",
        ]);
        let page = pages.get(0).unwrap();
        let inline = page.resources.as_inline().expect("page inline wins");
        assert!(inline.get(b"XObject").is_ok());
        assert!(page.resources.as_reference().is_none());
    }

    #[test]
    fn page_resources_reference_overrides_inherited_inline() {
        let pages = load_pages(&[
            "1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n",
            "2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 \
             /Resources << /Font << /F1 5 0 R >> >> >>\nendobj\n",
            "3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] \
             /Resources 4 0 R >>\nendobj\n",
            "4 0 obj\n<< /XObject << >> >>\nendobj\n",
            "5 0 obj\n<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>\nendobj\n",
        ]);
        let page = pages.get(0).unwrap();
        assert_eq!(page.resources.as_reference(), Some((4, 0)));
        assert!(page.resources.as_inline().is_none());
    }
}
