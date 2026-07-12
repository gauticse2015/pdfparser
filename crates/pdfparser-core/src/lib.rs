//! Core PDF load path: open document, page tree, stream decode under limits.
#![deny(missing_docs)]

mod error;
mod filters;
mod limits;
mod objects;
mod page_tree;

pub use error::{Error, Result};
pub use filters::decode_stream_data;
pub use limits::{hard_max, LimitKind, ResourceGovernor, ResourceLimits};
pub use objects::{extract_objects, DocumentObjects, FormField, ImageObject, LinkAnnotation};
pub use page_tree::{PageInfo, PageTree};

use lopdf::{Document as LopdfDocument, Object, ObjectId as LopdfId};
use pdfparser_ir::Rect;
use std::path::Path;
use std::sync::Mutex;

/// Opened PDF document (encoded access + page tree).
pub struct PdfDocument {
    inner: Mutex<LopdfDocument>,
    /// Governor.
    pub governor: ResourceGovernor,
    /// Page tree.
    pub pages: PageTree,
    /// Trailer encrypt present.
    pub encrypted: bool,
    /// PDF version.
    pub version: String,
}

impl PdfDocument {
    /// Open from path. Encrypted files return Error::Encryption (K15).
    pub fn open(path: impl AsRef<Path>, limits: ResourceLimits) -> Result<Self> {
        let data = std::fs::read(path.as_ref()).map_err(Error::Io)?;
        Self::from_bytes(&data, limits)
    }

    /// Open from bytes.
    pub fn from_bytes(data: &[u8], limits: ResourceLimits) -> Result<Self> {
        if data.len() as u64 > limits.max_file_bytes {
            return Err(Error::LimitExceeded {
                kind: LimitKind::FileSize,
            });
        }
        let governor = ResourceGovernor::new(limits);
        let doc = LopdfDocument::load_mem(data).map_err(|e| Error::Syntax(e.to_string()))?;

        // Encryption gate (K15): any Encrypt in trailer
        let encrypted = doc
            .trailer
            .get(b"Encrypt")
            .ok()
            .map(|_| true)
            .unwrap_or(false);
        if encrypted {
            return Err(Error::Encryption);
        }

        let version = doc.version.clone();
        let pages = PageTree::from_document(&doc)?;

        Ok(Self {
            inner: Mutex::new(doc),
            governor,
            pages,
            encrypted: false,
            version,
        })
    }

    /// Page count.
    pub fn page_count(&self) -> u32 {
        self.pages.len() as u32
    }

    /// Extract images, URI links, AcroForm fields, and outline titles.
    pub fn objects(&self) -> Result<DocumentObjects> {
        let doc = self
            .inner
            .lock()
            .map_err(|_| Error::Internal("lock".into()))?;
        extract_objects(&doc, &self.pages)
    }

    /// Metadata from Info dict.
    pub fn info_string(&self, key: &[u8]) -> Option<String> {
        let doc = self.inner.lock().ok()?;
        let info = doc.trailer.get(b"Info").ok()?;
        let id = match info {
            Object::Reference(r) => *r,
            _ => return None,
        };
        let dict = doc.get_dictionary(id).ok()?;
        let obj = dict.get(key).ok()?;
        object_to_string(obj)
    }

    /// Decode a content stream for page index (0-based).
    pub fn page_content_bytes(&self, page_index: usize) -> Result<Vec<u8>> {
        let page = self.pages.get(page_index).ok_or(Error::PageOutOfRange {
            index: page_index as u32,
        })?;
        let doc = self
            .inner
            .lock()
            .map_err(|_| Error::Internal("lock".into()))?;
        let mut out = Vec::new();
        collect_contents(&doc, &page.id, &mut out, &self.governor)?;
        Ok(out)
    }

    /// Font resources dict object id for page (merged Resources/Font).
    pub fn page_font_map(&self, page_index: usize) -> Result<Vec<(String, LopdfId)>> {
        let page = self.pages.get(page_index).ok_or(Error::PageOutOfRange {
            index: page_index as u32,
        })?;
        let doc = self
            .inner
            .lock()
            .map_err(|_| Error::Internal("lock".into()))?;
        let mut fonts = Vec::new();
        if let Some(res_id) = page.resources {
            if let Ok(res) = doc.get_dictionary(res_id) {
                if let Ok(Object::Dictionary(font_dict)) = res.get(b"Font") {
                    for (name, obj) in font_dict.iter() {
                        let name = String::from_utf8_lossy(name).into_owned();
                        if let Object::Reference(id) = obj {
                            fonts.push((name, *id));
                        }
                    }
                } else if let Ok(Object::Reference(fr)) = res.get(b"Font") {
                    if let Ok(font_dict) = doc.get_dictionary(*fr) {
                        for (name, obj) in font_dict.iter() {
                            let name = String::from_utf8_lossy(name).into_owned();
                            if let Object::Reference(id) = obj {
                                fonts.push((name, *id));
                            }
                        }
                    }
                }
            }
        }
        // Also try page dict Resources
        if fonts.is_empty() {
            if let Ok(page_dict) = doc.get_dictionary(page.id) {
                if let Ok(Object::Dictionary(res)) = page_dict.get(b"Resources") {
                    if let Ok(Object::Dictionary(font_dict)) = res.get(b"Font") {
                        for (name, obj) in font_dict.iter() {
                            let name = String::from_utf8_lossy(name).into_owned();
                            if let Object::Reference(id) = obj {
                                fonts.push((name, *id));
                            }
                        }
                    }
                } else if let Ok(Object::Reference(res_ref)) = page_dict.get(b"Resources") {
                    if let Ok(res) = doc.get_dictionary(*res_ref) {
                        if let Ok(Object::Dictionary(font_dict)) = res.get(b"Font") {
                            for (name, obj) in font_dict.iter() {
                                let name = String::from_utf8_lossy(name).into_owned();
                                if let Object::Reference(id) = obj {
                                    fonts.push((name, *id));
                                }
                            }
                        } else if let Ok(Object::Reference(fr)) = res.get(b"Font") {
                            if let Ok(font_dict) = doc.get_dictionary(*fr) {
                                for (name, obj) in font_dict.iter() {
                                    let name = String::from_utf8_lossy(name).into_owned();
                                    if let Object::Reference(id) = obj {
                                        fonts.push((name, *id));
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        Ok(fonts)
    }

    /// Access raw lopdf document for font loading.
    pub fn with_doc<R>(&self, f: impl FnOnce(&LopdfDocument) -> R) -> Result<R> {
        let doc = self
            .inner
            .lock()
            .map_err(|_| Error::Internal("lock".into()))?;
        Ok(f(&doc))
    }

    /// Decode stream by object id under governor.
    pub fn decode_stream_id(&self, id: LopdfId) -> Result<Vec<u8>> {
        let doc = self
            .inner
            .lock()
            .map_err(|_| Error::Internal("lock".into()))?;
        let mut out = Vec::new();
        append_content_object(&doc, Object::Reference(id), &mut out, &self.governor)?;
        Ok(out)
    }
}

fn object_to_string(obj: &Object) -> Option<String> {
    match obj {
        Object::String(s, _) => Some(String::from_utf8_lossy(s).into_owned()),
        Object::Name(n) => Some(String::from_utf8_lossy(n).into_owned()),
        _ => None,
    }
}

fn collect_contents(
    doc: &LopdfDocument,
    page_id: &LopdfId,
    out: &mut Vec<u8>,
    gov: &ResourceGovernor,
) -> Result<()> {
    let page = doc
        .get_dictionary(*page_id)
        .map_err(|e| Error::Syntax(e.to_string()))?;
    match page.get(b"Contents") {
        Ok(Object::Reference(id)) => {
            append_content_object(doc, Object::Reference(*id), out, gov)?;
        }
        Ok(Object::Array(arr)) => {
            for obj in arr {
                append_content_object(doc, obj.clone(), out, gov)?;
            }
        }
        Ok(Object::Stream(stream)) => {
            let bytes = filters::decode_stream_data(&stream.dict, &stream.content, gov)?;
            out.extend_from_slice(&bytes);
        }
        _ => {}
    }
    Ok(())
}

/// Decode a Contents entry (stream, ref→stream, ref→array of streams, or array).
fn append_content_object(
    doc: &LopdfDocument,
    obj: Object,
    out: &mut Vec<u8>,
    gov: &ResourceGovernor,
) -> Result<()> {
    match obj {
        Object::Stream(stream) => {
            let bytes = filters::decode_stream_data(&stream.dict, &stream.content, gov)?;
            out.extend_from_slice(&bytes);
            out.push(b'\n');
        }
        Object::Reference(id) => {
            let resolved = doc
                .get_object(id)
                .map_err(|e| Error::Syntax(e.to_string()))?
                .clone();
            // Contents may be an indirect array of content streams.
            match resolved {
                Object::Array(arr) => {
                    for item in arr {
                        append_content_object(doc, item, out, gov)?;
                    }
                }
                other => append_content_object(doc, other, out, gov)?,
            }
        }
        Object::Array(arr) => {
            for item in arr {
                append_content_object(doc, item, out, gov)?;
            }
        }
        other => {
            return Err(Error::Syntax(format!(
                "expected stream, got {}",
                object_kind(&other)
            )));
        }
    }
    Ok(())
}

fn object_kind(obj: &Object) -> &'static str {
    match obj {
        Object::Null => "null",
        Object::Boolean(_) => "bool",
        Object::Integer(_) => "int",
        Object::Real(_) => "real",
        Object::Name(_) => "name",
        Object::String(_, _) => "string",
        Object::Array(_) => "array",
        Object::Dictionary(_) => "dictionary",
        Object::Stream(_) => "stream",
        Object::Reference(_) => "reference",
    }
}

/// Convert lopdf rectangle array to Rect.
pub fn rect_from_object(obj: &Object) -> Option<Rect> {
    let arr = match obj {
        Object::Array(a) => a,
        _ => return None,
    };
    if arr.len() < 4 {
        return None;
    }
    let nums: Vec<f32> = arr
        .iter()
        .take(4)
        .filter_map(|o| match o {
            Object::Integer(i) => Some(*i as f32),
            Object::Real(r) => Some(*r),
            _ => None,
        })
        .collect();
    if nums.len() < 4 {
        return None;
    }
    Some(Rect {
        x0: nums[0].min(nums[2]),
        y0: nums[1].min(nums[3]),
        x1: nums[0].max(nums[2]),
        y1: nums[1].max(nums[3]),
    })
}
