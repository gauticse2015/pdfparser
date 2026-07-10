//! Document object extraction: images, links, AcroForm fields, outline titles.
//!
//! Generic ISO 32000 walks — no corpus-specific logic.
use crate::error::Result;
use crate::page_tree::PageTree;
use lopdf::{Dictionary, Document, Object, ObjectId};
use std::collections::HashSet;

/// Image XObject summary (metadata only; pixels not decoded).
#[derive(Debug, Clone)]
pub struct ImageObject {
    /// Resource name if known.
    pub name: Option<String>,
    /// Width in samples when present.
    pub width: Option<u32>,
    /// Height in samples when present.
    pub height: Option<u32>,
    /// Page index (0-based).
    pub page: u32,
}

/// URI hyperlink from a Link annotation.
#[derive(Debug, Clone)]
pub struct LinkAnnotation {
    /// Destination URI.
    pub uri: String,
    /// Page index.
    pub page: u32,
}

/// AcroForm field (name [+ optional value]).
#[derive(Debug, Clone)]
pub struct FormField {
    /// Fully qualified or partial field name.
    pub name: String,
    /// Optional current value as string.
    pub value: Option<String>,
}

/// Extracted document objects summary.
#[derive(Debug, Clone, Default)]
pub struct DocumentObjects {
    /// Images found on pages.
    pub images: Vec<ImageObject>,
    /// URI links.
    pub links: Vec<LinkAnnotation>,
    /// Form field names (and values when present).
    pub form_fields: Vec<FormField>,
    /// Outline / bookmark titles in document order (flattened).
    pub outline_titles: Vec<String>,
}

impl DocumentObjects {
    /// Image count for scoreboard.
    pub fn image_count(&self) -> usize {
        self.images.len()
    }

    /// Link URIs.
    pub fn link_uris(&self) -> Vec<String> {
        self.links.iter().map(|l| l.uri.clone()).collect()
    }

    /// Form field names (and `name=value` when value present — metrics use name before `=`).
    pub fn form_field_labels(&self) -> Vec<String> {
        self.form_fields
            .iter()
            .map(|f| match &f.value {
                Some(v) if !v.is_empty() => format!("{}={}", f.name, v),
                _ => f.name.clone(),
            })
            .collect()
    }
}

/// Extract objects from an open lopdf document + page tree.
pub fn extract_objects(doc: &Document, pages: &PageTree) -> Result<DocumentObjects> {
    Ok(DocumentObjects {
        images: collect_images(doc, pages),
        links: collect_links(doc, pages),
        form_fields: collect_form_fields(doc),
        outline_titles: collect_outline_titles(doc),
    })
}

fn collect_images(doc: &Document, pages: &PageTree) -> Vec<ImageObject> {
    let mut images = Vec::new();
    let mut seen: HashSet<(u32, u16, u32)> = HashSet::new(); // object id + page
    for (page_idx, page) in pages.iter().enumerate() {
        let page_idx = page_idx as u32;
        // Prefer page-local XObject dictionary
        if let Ok(page_dict) = doc.get_dictionary(page.id) {
            walk_resources_for_images(doc, page_dict, page_idx, &mut images, &mut seen);
        }
        if let Some(res_id) = page.resources {
            if let Ok(res) = doc.get_dictionary(res_id) {
                collect_xobject_images(doc, res, page_idx, &mut images, &mut seen, None);
            }
        }
        // lopdf helper
        if let Ok((Some(res), _fonts)) = doc.get_page_resources(page.id) {
            collect_xobject_images(doc, res, page_idx, &mut images, &mut seen, None);
        }
    }
    images
}

fn walk_resources_for_images(
    doc: &Document,
    page_dict: &Dictionary,
    page_idx: u32,
    images: &mut Vec<ImageObject>,
    seen: &mut HashSet<(u32, u16, u32)>,
) {
    match page_dict.get(b"Resources") {
        Ok(Object::Dictionary(res)) => {
            collect_xobject_images(doc, res, page_idx, images, seen, None);
        }
        Ok(Object::Reference(id)) => {
            if let Ok(res) = doc.get_dictionary(*id) {
                collect_xobject_images(doc, res, page_idx, images, seen, None);
            }
        }
        _ => {}
    }
}

fn collect_xobject_images(
    doc: &Document,
    res: &Dictionary,
    page_idx: u32,
    images: &mut Vec<ImageObject>,
    seen: &mut HashSet<(u32, u16, u32)>,
    name_hint: Option<&str>,
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
        let (obj_id, dict) = match resolve_dict(doc, obj) {
            Some(v) => v,
            None => continue,
        };
        let subtype = dict
            .get(b"Subtype")
            .ok()
            .and_then(|o| o.as_name_str().ok())
            .unwrap_or("");
        if subtype != "Image" {
            // Form XObject may embed further images
            if subtype == "Form" {
                // Form resources
                if let Ok(Object::Dictionary(inner_res)) = dict.get(b"Resources") {
                    collect_xobject_images(doc, inner_res, page_idx, images, seen, Some(&name));
                } else if let Ok(Object::Reference(rid)) = dict.get(b"Resources") {
                    if let Ok(inner_res) = doc.get_dictionary(*rid) {
                        collect_xobject_images(doc, inner_res, page_idx, images, seen, Some(&name));
                    }
                }
            }
            continue;
        }
        let key = (obj_id.0, obj_id.1, page_idx);
        if !seen.insert(key) {
            continue;
        }
        let width = dict
            .get(b"Width")
            .ok()
            .and_then(|o| o.as_i64().ok())
            .map(|w| w as u32);
        let height = dict
            .get(b"Height")
            .ok()
            .and_then(|o| o.as_i64().ok())
            .map(|h| h as u32);
        images.push(ImageObject {
            name: Some(name_hint.unwrap_or(&name).to_string()),
            width,
            height,
            page: page_idx,
        });
    }
}

fn resolve_dict<'a>(doc: &'a Document, obj: &'a Object) -> Option<(ObjectId, &'a Dictionary)> {
    match obj {
        Object::Reference(id) => {
            // Image XObjects are streams; their dictionary holds Subtype/Width/Height.
            match doc.get_object(*id).ok()? {
                Object::Stream(s) => Some((*id, &s.dict)),
                Object::Dictionary(d) => Some((*id, d)),
                _ => None,
            }
        }
        Object::Dictionary(d) => Some(((0, 0), d)),
        Object::Stream(s) => Some(((0, 0), &s.dict)),
        _ => None,
    }
}

fn collect_links(doc: &Document, pages: &PageTree) -> Vec<LinkAnnotation> {
    let mut links = Vec::new();
    for (page_idx, page) in pages.iter().enumerate() {
        let page_idx = page_idx as u32;
        let annots = match doc.get_page_annotations(page.id) {
            Ok(a) => a,
            Err(_) => continue,
        };
        for annot in annots {
            let subtype = annot
                .get(b"Subtype")
                .ok()
                .and_then(|o| o.as_name_str().ok())
                .unwrap_or("");
            if subtype != "Link" {
                continue;
            }
            if let Some(uri) = link_uri(doc, annot) {
                links.push(LinkAnnotation {
                    uri,
                    page: page_idx,
                });
            }
        }
    }
    links
}

fn link_uri(doc: &Document, annot: &Dictionary) -> Option<String> {
    // /A << /S /URI /URI (url) >>
    let action = match annot.get(b"A").ok() {
        Some(Object::Dictionary(d)) => d,
        Some(Object::Reference(id)) => doc.get_dictionary(*id).ok()?,
        _ => return None,
    };
    let s = action
        .get(b"S")
        .ok()
        .and_then(|o| o.as_name_str().ok())
        .unwrap_or("");
    if s != "URI" {
        return None;
    }
    match action.get(b"URI").ok()? {
        Object::String(bytes, _) => Some(pdf_string_to_utf8(bytes)),
        Object::Reference(id) => match doc.get_object(*id).ok()? {
            Object::String(bytes, _) => Some(pdf_string_to_utf8(bytes)),
            _ => None,
        },
        _ => None,
    }
}

fn collect_form_fields(doc: &Document) -> Vec<FormField> {
    let catalog = match doc.catalog() {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };
    let acro = match catalog.get(b"AcroForm") {
        Ok(Object::Dictionary(d)) => d,
        Ok(Object::Reference(id)) => match doc.get_dictionary(*id) {
            Ok(d) => d,
            Err(_) => return Vec::new(),
        },
        _ => return Vec::new(),
    };
    let fields_obj = match acro.get(b"Fields") {
        Ok(o) => o,
        Err(_) => return Vec::new(),
    };
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    walk_fields(doc, fields_obj, "", &mut out, &mut seen, 0);
    out
}

fn walk_fields(
    doc: &Document,
    fields_obj: &Object,
    parent_name: &str,
    out: &mut Vec<FormField>,
    seen: &mut HashSet<String>,
    depth: u32,
) {
    if depth > 32 {
        return;
    }
    let arr = match fields_obj {
        Object::Array(a) => a,
        Object::Reference(id) => {
            if let Ok(Object::Array(a)) = doc.get_object(*id) {
                // can't return ref easily — recurse differently
                for item in a {
                    walk_field_item(doc, item, parent_name, out, seen, depth);
                }
            } else if let Ok(d) = doc.get_dictionary(*id) {
                walk_field_dict(doc, d, parent_name, out, seen, depth);
            }
            return;
        }
        Object::Dictionary(d) => {
            walk_field_dict(doc, d, parent_name, out, seen, depth);
            return;
        }
        _ => return,
    };
    for item in arr {
        walk_field_item(doc, item, parent_name, out, seen, depth);
    }
}

fn walk_field_item(
    doc: &Document,
    item: &Object,
    parent_name: &str,
    out: &mut Vec<FormField>,
    seen: &mut HashSet<String>,
    depth: u32,
) {
    match item {
        Object::Reference(id) => {
            if let Ok(d) = doc.get_dictionary(*id) {
                walk_field_dict(doc, d, parent_name, out, seen, depth);
            }
        }
        Object::Dictionary(d) => walk_field_dict(doc, d, parent_name, out, seen, depth),
        _ => {}
    }
}

fn walk_field_dict(
    doc: &Document,
    d: &Dictionary,
    parent_name: &str,
    out: &mut Vec<FormField>,
    seen: &mut HashSet<String>,
    depth: u32,
) {
    let partial = d
        .get(b"T")
        .ok()
        .and_then(object_to_text)
        .unwrap_or_default();
    let name = if parent_name.is_empty() {
        partial.clone()
    } else if partial.is_empty() {
        parent_name.to_string()
    } else {
        format!("{parent_name}.{partial}")
    };

    // Kids = non-terminal field
    if let Ok(kids) = d.get(b"Kids") {
        let parent = if name.is_empty() { parent_name } else { name.as_str() };
        walk_fields(doc, kids, parent, out, seen, depth + 1);
        // Some terminals also have Kids (widget annots) — still emit if /FT or /V present
    }

    let is_terminal = d.get(b"FT").is_ok() || d.get(b"V").is_ok() || d.get(b"Kids").is_err();
    if is_terminal && !name.is_empty() && seen.insert(name.clone()) {
        let value = d.get(b"V").ok().and_then(object_to_text);
        out.push(FormField { name, value });
    }
}

fn collect_outline_titles(doc: &Document) -> Vec<String> {
    // Prefer direct outline tree walk (more reliable for titles than Destination alone)
    let mut titles = Vec::new();
    let catalog = match doc.catalog() {
        Ok(c) => c,
        Err(_) => return titles,
    };
    let outlines_root = match catalog.get(b"Outlines") {
        Ok(Object::Dictionary(d)) => d,
        Ok(Object::Reference(id)) => match doc.get_dictionary(*id) {
            Ok(d) => d,
            Err(_) => return titles,
        },
        _ => return titles,
    };
    let first = match outlines_root.get(b"First") {
        Ok(o) => o,
        Err(_) => return titles,
    };
    walk_outline_titles(doc, first, &mut titles, 0);
    titles
}

fn walk_outline_titles(doc: &Document, node: &Object, titles: &mut Vec<String>, depth: u32) {
    if depth > 64 {
        return;
    }
    let mut current = match resolve_dict_ref(doc, node) {
        Some(d) => d,
        None => return,
    };
    loop {
        if let Ok(title_obj) = current.get(b"Title") {
            if let Some(t) = object_to_text(title_obj) {
                if !t.is_empty() {
                    titles.push(t);
                }
            }
        }
        if let Ok(first) = current.get(b"First") {
            walk_outline_titles(doc, first, titles, depth + 1);
        }
        match current.get(b"Next") {
            Ok(next) => match resolve_dict_ref(doc, next) {
                Some(d) => current = d,
                None => break,
            },
            Err(_) => break,
        }
    }
}

fn resolve_dict_ref<'a>(doc: &'a Document, obj: &'a Object) -> Option<&'a Dictionary> {
    match obj {
        Object::Dictionary(d) => Some(d),
        Object::Reference(id) => doc.get_dictionary(*id).ok(),
        _ => None,
    }
}

fn object_to_text(obj: &Object) -> Option<String> {
    match obj {
        Object::String(bytes, _) => Some(pdf_string_to_utf8(bytes)),
        Object::Name(n) => Some(String::from_utf8_lossy(n).into_owned()),
        Object::Integer(i) => Some(i.to_string()),
        Object::Real(f) => Some(f.to_string()),
        Object::Boolean(b) => Some(if *b { "true" } else { "false" }.into()),
        Object::Reference(_) => None,
        _ => None,
    }
}

fn pdf_string_to_utf8(bytes: &[u8]) -> String {
    // UTF-16BE with BOM
    if bytes.len() >= 2 && bytes[0] == 0xFE && bytes[1] == 0xFF {
        let mut s = String::new();
        let mut i = 2;
        while i + 1 < bytes.len() {
            let cu = u16::from_be_bytes([bytes[i], bytes[i + 1]]);
            i += 2;
            if let Some(ch) = char::from_u32(cu as u32) {
                s.push(ch);
            } else if (0xD800..=0xDBFF).contains(&cu) && i + 1 < bytes.len() {
                let cu2 = u16::from_be_bytes([bytes[i], bytes[i + 1]]);
                i += 2;
                if (0xDC00..=0xDFFF).contains(&cu2) {
                    let cp = 0x10000 + (((cu as u32) - 0xD800) << 10) + ((cu2 as u32) - 0xDC00);
                    if let Some(ch) = char::from_u32(cp) {
                        s.push(ch);
                    }
                }
            }
        }
        return s;
    }
    // PDFDocEncoding ≈ Latin-1 for common cases
    String::from_utf8_lossy(bytes).into_owned()
}

