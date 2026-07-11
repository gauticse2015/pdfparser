//! Decode page Image XObjects → [`RasterPage`] for raster line sensing.

use lopdf::{Dictionary, Object, ObjectId};
use pdfparser_content::ImagePlacement;
use pdfparser_core::{Error, PdfDocument, Result};
use pdfparser_ir::Matrix3x2;
use pdfparser_tables::{gray_from_rgb, RasterPage};
use std::collections::HashMap;

/// Build raster pages from image placements on one PDF page.
pub fn raster_pages_for_page(
    doc: &PdfDocument,
    page_index: usize,
    placements: &[ImagePlacement],
) -> Result<Vec<RasterPage>> {
    if placements.is_empty() {
        return Ok(Vec::new());
    }
    let xobjects = page_xobject_map(doc, page_index)?;
    let mut out = Vec::new();
    for p in placements {
        let obj_id = match xobjects.get(&p.name) {
            Some(id) => *id,
            None => {
                // soft match by suffix
                match xobjects
                    .iter()
                    .find(|(k, _)| k.ends_with(&p.name) || p.name.ends_with(k.as_str()))
                {
                    Some((_, id)) => *id,
                    None => continue,
                }
            }
        };
        if let Some(rp) = decode_image_xobject(doc, obj_id, &p.ctm)? {
            out.push(rp);
        }
    }
    Ok(out)
}

fn page_xobject_map(doc: &PdfDocument, page_index: usize) -> Result<HashMap<String, ObjectId>> {
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

fn collect_xobjects(doc: &lopdf::Document, page_dict: &Dictionary, map: &mut HashMap<String, ObjectId>) {
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

fn collect_xobjects_from_res(
    doc: &lopdf::Document,
    res: &Dictionary,
    map: &mut HashMap<String, ObjectId>,
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
            if let Ok(Object::Stream(s)) = doc.get_object(*id) {
                let subtype = s
                    .dict
                    .get(b"Subtype")
                    .ok()
                    .and_then(|o| o.as_name_str().ok())
                    .unwrap_or("");
                if subtype == "Form" {
                    if let Ok(Object::Dictionary(inner)) = s.dict.get(b"Resources") {
                        collect_xobjects_from_res(doc, inner, map);
                    } else if let Ok(Object::Reference(rid)) = s.dict.get(b"Resources") {
                        if let Ok(inner) = doc.get_dictionary(*rid) {
                            collect_xobjects_from_res(doc, inner, map);
                        }
                    }
                }
            }
        }
    }
}

fn decode_image_xobject(
    doc: &PdfDocument,
    id: ObjectId,
    ctm: &Matrix3x2,
) -> Result<Option<RasterPage>> {
    let bytes = match doc.decode_stream_id(id) {
        Ok(b) => b,
        Err(_) => return Ok(None),
    };
    let meta = doc.with_doc(|d| {
        let obj = d.get_object(id).ok()?;
        let dict = match obj {
            Object::Stream(s) => &s.dict,
            _ => return None,
        };
        let w = dict.get(b"Width").ok().and_then(|o| o.as_i64().ok())? as usize;
        let h = dict.get(b"Height").ok().and_then(|o| o.as_i64().ok())? as usize;
        let cs = dict
            .get(b"ColorSpace")
            .ok()
            .and_then(|o| o.as_name_str().ok())
            .unwrap_or("DeviceRGB")
            .to_string();
        let bpc = dict
            .get(b"BitsPerComponent")
            .ok()
            .and_then(|o| o.as_i64().ok())
            .unwrap_or(8) as u8;
        Some((w, h, cs, bpc))
    })?;
    let Some((width, height, color_space, bpc)) = meta else {
        return decode_via_image_crate(&bytes, ctm);
    };
    if width == 0 || height == 0 || bpc != 8 {
        return decode_via_image_crate(&bytes, ctm);
    }

    let gray = match color_space.as_str() {
        "DeviceGray" | "Gray" => {
            if bytes.len() < width * height {
                return decode_via_image_crate(&bytes, ctm);
            }
            bytes[..width * height].to_vec()
        }
        "DeviceRGB" | "RGB" => {
            if bytes.len() < width * height * 3 {
                return decode_via_image_crate(&bytes, ctm);
            }
            match gray_from_rgb(&bytes, width, height) {
                Some(g) => g,
                None => return decode_via_image_crate(&bytes, ctm),
            }
        }
        _ => return decode_via_image_crate(&bytes, ctm),
    };

    Ok(Some(raster_page_from_gray(width, height, gray, ctm)))
}

fn decode_via_image_crate(bytes: &[u8], ctm: &Matrix3x2) -> Result<Option<RasterPage>> {
    let img = match image::load_from_memory(bytes) {
        Ok(i) => i,
        Err(_) => return Ok(None),
    };
    let rgb = img.to_rgb8();
    let (w, h) = rgb.dimensions();
    let w = w as usize;
    let h = h as usize;
    let Some(gray) = gray_from_rgb(rgb.as_raw(), w, h) else {
        return Ok(None);
    };
    Ok(Some(raster_page_from_gray(w, h, gray, ctm)))
}

fn raster_page_from_gray(width: usize, height: usize, gray: Vec<u8>, ctm: &Matrix3x2) -> RasterPage {
    let p00 = ctm.apply(0.0, 0.0);
    let p10 = ctm.apply(1.0, 0.0);
    let p01 = ctm.apply(0.0, 1.0);
    let origin_x = p01.x.min(p00.x).min(p10.x);
    let origin_y = p00.y.min(p01.y);
    let scale_x = (p10.x - p00.x).abs() / (width as f32).max(1.0);
    let scale_y = (p01.y - p00.y).abs() / (height as f32).max(1.0);
    RasterPage {
        width,
        height,
        pixels: gray,
        scale_x: scale_x.max(1e-6),
        scale_y: scale_y.max(1e-6),
        origin_x,
        origin_y,
        y_down_pixels: true,
    }
}
