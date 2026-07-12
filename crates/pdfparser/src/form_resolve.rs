//! Form XObject resolver for content-VM expansion (PR2a / K19).
//!
//! Walks page (and nested Form) resources for `/Subtype /Form` XObjects and
//! supplies decoded content streams + `/Matrix` to
//! [`pdfparser_content::interpret_page_with_resolver`].

use lopdf::{Dictionary, Object, ObjectId as LopdfId};
use pdfparser_content::{FormContentResolver, FormXObject};
use pdfparser_core::{Error, PdfDocument, Result};
use pdfparser_ir::{Matrix3x2, ObjectId, Rect};
use std::collections::HashMap;

/// Document-backed Form resolver with a resource scope stack.
pub struct DocFormResolver<'a> {
    doc: &'a PdfDocument,
    /// Stack of XObject name → object id maps. Top is the innermost scope.
    xobject_stack: Vec<HashMap<String, LopdfId>>,
}

impl<'a> DocFormResolver<'a> {
    /// Build a resolver for one page's initial resource scope.
    pub fn for_page(doc: &'a PdfDocument, page_index: usize) -> Result<Self> {
        let map = page_xobject_map(doc, page_index)?;
        Ok(Self {
            doc,
            xobject_stack: vec![map],
        })
    }

    fn lookup(&self, name: &str) -> Option<LopdfId> {
        for scope in self.xobject_stack.iter().rev() {
            if let Some(id) = scope.get(name) {
                return Some(*id);
            }
            // Soft match by suffix / prefix (same as raster_images).
            if let Some((_, id)) = scope
                .iter()
                .find(|(k, _)| k.ends_with(name) || name.ends_with(k.as_str()))
            {
                return Some(*id);
            }
        }
        None
    }
}

impl FormContentResolver for DocFormResolver<'_> {
    fn resolve_form(&mut self, name: &str) -> Option<FormXObject> {
        let id = self.lookup(name)?;
        let (subtype, matrix, b_box) = self
            .doc
            .with_doc(|d| {
                let obj = d.get_object(id).ok()?;
                let dict = match obj {
                    Object::Stream(s) => &s.dict,
                    Object::Dictionary(d) => d,
                    _ => return None,
                };
                let subtype = dict
                    .get(b"Subtype")
                    .ok()
                    .and_then(|o| o.as_name_str().ok())
                    .unwrap_or("")
                    .to_string();
                let matrix = dict
                    .get(b"Matrix")
                    .ok()
                    .and_then(parse_matrix)
                    .unwrap_or_else(Matrix3x2::identity);
                let b_box = dict.get(b"BBox").ok().and_then(parse_bbox);
                Some((subtype, matrix, b_box))
            })
            .ok()??;

        if subtype != "Form" {
            return None;
        }

        let stream = self.doc.decode_stream_id(id).ok()?;
        Some(FormXObject {
            id: ObjectId {
                num: id.0,
                gen: id.1,
            },
            stream,
            matrix,
            b_box,
        })
    }

    fn enter_form(&mut self, form: &FormXObject) {
        let lopdf_id: LopdfId = (form.id.num, form.id.gen);
        let map = self
            .doc
            .with_doc(|d| form_xobject_map(d, lopdf_id))
            .unwrap_or_default();
        self.xobject_stack.push(map);
    }

    fn leave_form(&mut self) {
        if self.xobject_stack.len() > 1 {
            self.xobject_stack.pop();
        }
    }
}

fn page_xobject_map(doc: &PdfDocument, page_index: usize) -> Result<HashMap<String, LopdfId>> {
    let page = doc.pages.get(page_index).ok_or(Error::PageOutOfRange {
        index: page_index as u32,
    })?;
    doc.with_doc(|d| {
        let mut map = HashMap::new();
        if let Ok(page_dict) = d.get_dictionary(page.id) {
            collect_xobjects(d, page_dict, &mut map);
        }
        if let Some(res_id) = page.resources {
            if let Ok(res) = d.get_dictionary(res_id) {
                collect_xobjects_from_res(d, res, &mut map);
            }
        }
        if let Ok((Some(res), _)) = d.get_page_resources(page.id) {
            collect_xobjects_from_res(d, res, &mut map);
        }
        map
    })
}

fn form_xobject_map(doc: &lopdf::Document, form_id: LopdfId) -> HashMap<String, LopdfId> {
    let mut map = HashMap::new();
    let dict = match doc.get_object(form_id).ok() {
        Some(Object::Stream(s)) => &s.dict,
        Some(Object::Dictionary(d)) => d,
        _ => return map,
    };
    match dict.get(b"Resources") {
        Ok(Object::Dictionary(res)) => collect_xobjects_from_res(doc, res, &mut map),
        Ok(Object::Reference(rid)) => {
            if let Ok(res) = doc.get_dictionary(*rid) {
                collect_xobjects_from_res(doc, res, &mut map);
            }
        }
        _ => {}
    }
    map
}

fn collect_xobjects(
    doc: &lopdf::Document,
    page_dict: &Dictionary,
    map: &mut HashMap<String, LopdfId>,
) {
    match page_dict.get(b"Resources") {
        Ok(Object::Dictionary(res)) => collect_xobjects_from_res(doc, res, map),
        Ok(Object::Reference(id)) => {
            if let Ok(res) = doc.get_dictionary(*id) {
                collect_xobjects_from_res(doc, res, map);
            }
        }
        _ => {}
    }
}

/// Collect immediate XObject name → id entries (no nested Form flatten).
fn collect_xobjects_from_res(
    doc: &lopdf::Document,
    res: &Dictionary,
    map: &mut HashMap<String, LopdfId>,
) {
    let xobj = match res.get(b"XObject") {
        Ok(Object::Dictionary(d)) => d,
        Ok(Object::Reference(id)) => match doc.get_dictionary(*id) {
            Ok(d) => d,
            Err(_) => return,
        },
        _ => return,
    };
    for (name_bytes, obj) in xobj.iter() {
        let name = String::from_utf8_lossy(name_bytes).into_owned();
        if let Object::Reference(id) = obj {
            map.insert(name, *id);
        }
    }
}

fn parse_matrix(obj: &Object) -> Option<Matrix3x2> {
    let arr = match obj {
        Object::Array(a) => a,
        _ => return None,
    };
    if arr.len() < 6 {
        return None;
    }
    let mut m = [0.0f32; 6];
    for (i, o) in arr.iter().take(6).enumerate() {
        m[i] = match o {
            Object::Integer(n) => *n as f32,
            Object::Real(n) => *n,
            _ => return None,
        };
    }
    Some(Matrix3x2 { m })
}

fn parse_bbox(obj: &Object) -> Option<Rect> {
    pdfparser_core::rect_from_object(obj)
}
