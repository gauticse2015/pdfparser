//! Page tree walk with inheritance.
use crate::error::{Error, Result};
use crate::rect_from_object;
use lopdf::{Document, Object, ObjectId};
use pdfparser_ir::Rect;

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
    /// Resources dict id if reference.
    pub resources: Option<ObjectId>,
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
    resources: Option<ObjectId>,
}

fn walk_pages(
    doc: &Document,
    id: ObjectId,
    inherit: &Inherit,
    out: &mut Vec<PageInfo>,
    depth: u32,
) -> Result<()> {
    if depth > 64 {
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
    if let Ok(Object::Reference(r)) = dict.get(b"Resources") {
        next.resources = Some(*r);
    }

    if type_name == "Page" || (type_name.is_empty() && dict.get(b"Kids").is_err()) {
        let media = next.media_box.unwrap_or(Rect {
            x0: 0.0,
            y0: 0.0,
            x1: 612.0,
            y1: 792.0,
        });
        out.push(PageInfo {
            id,
            media_box: media,
            crop_box: next.crop_box,
            rotate: next.rotate.unwrap_or(0),
            resources: next.resources,
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
