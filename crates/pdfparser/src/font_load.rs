//! Load fonts from page resources via lopdf.
//! Generic handling of BaseEncoding + Differences and ToUnicode streams.
use flate2::read::{DeflateDecoder, ZlibDecoder};
use lopdf::{Dictionary, Document, Object, ObjectId};
use pdfparser_core::ResourceGovernor;
use pdfparser_fonts::{FontParts, LoadedFont};
use std::collections::HashMap;
use std::io::Read;

pub fn load_page_fonts(
    doc: &Document,
    font_refs: &[(String, ObjectId)],
    governor: &ResourceGovernor,
) -> HashMap<String, LoadedFont> {
    let mut map = HashMap::new();
    for (name, id) in font_refs {
        match load_one(doc, *id, name, governor) {
            Ok(f) => {
                map.insert(name.clone(), f);
            }
            Err(_) => {
                map.insert(name.clone(), LoadedFont::simple_latin(name));
            }
        }
    }
    if map.is_empty() {
        map.insert("F1".into(), LoadedFont::simple_latin("Helvetica"));
        map.insert("Helvetica".into(), LoadedFont::simple_latin("Helvetica"));
    }
    map
}

fn load_one(
    doc: &Document,
    id: ObjectId,
    res_name: &str,
    governor: &ResourceGovernor,
) -> Result<LoadedFont, ()> {
    let dict = doc.get_dictionary(id).map_err(|_| ())?;
    let subtype = dict
        .get(b"Subtype")
        .ok()
        .and_then(name_str)
        .unwrap_or_else(|| "Type1".into());

    if subtype == "Type0" {
        return load_type0(doc, dict, res_name, governor);
    }

    let base_font = dict.get(b"BaseFont").ok().and_then(name_str);
    let (encoding_name, differences) = parse_encoding(doc, dict);
    let first_char = dict
        .get(b"FirstChar")
        .ok()
        .and_then(int_val)
        .map(|i| i as u8);
    let widths = dict.get(b"Widths").ok().and_then(float_array);
    let (missing, ascent, descent) = descriptor_metrics(doc, dict);
    let to_unicode_bytes = to_unicode_stream(doc, dict, governor);

    let parts = FontParts {
        subtype,
        base_font: base_font.as_deref().or(Some(res_name)),
        encoding_name,
        differences,
        first_char,
        widths,
        missing_width: missing,
        to_unicode_bytes: to_unicode_bytes.as_deref(),
        dw: None,
        w_ranges: Vec::new(),
        ascent,
        descent,
    };
    LoadedFont::from_parts(parts).map_err(|_| ())
}

fn load_type0(
    doc: &Document,
    dict: &Dictionary,
    res_name: &str,
    governor: &ResourceGovernor,
) -> Result<LoadedFont, ()> {
    let base_font = dict.get(b"BaseFont").ok().and_then(name_str);
    let (encoding_name, _diff) = parse_encoding(doc, dict);
    let to_unicode_bytes = to_unicode_stream(doc, dict, governor);
    let mut dw = Some(1000.0);
    let mut w_ranges = Vec::new();
    let mut ascent = Some(800.0);
    let mut descent = Some(-200.0);

    if let Ok(Object::Array(desc)) = dict.get(b"DescendantFonts") {
        if let Some(Object::Reference(did)) = desc.first() {
            if let Ok(dd) = doc.get_dictionary(*did) {
                if let Ok(Object::Integer(v)) = dd.get(b"DW") {
                    dw = Some(*v as f32);
                } else if let Ok(Object::Real(v)) = dd.get(b"DW") {
                    dw = Some(*v);
                }
                if let Ok(Object::Array(w)) = dd.get(b"W") {
                    w_ranges = parse_w_array(w);
                }
                let m = descriptor_metrics(doc, dd);
                ascent = m.1.or(ascent);
                descent = m.2.or(descent);
            }
        }
    }

    let parts = FontParts {
        subtype: "Type0".into(),
        base_font: base_font.as_deref().or(Some(res_name)),
        encoding_name,
        differences: Vec::new(),
        first_char: None,
        widths: None,
        missing_width: None,
        to_unicode_bytes: to_unicode_bytes.as_deref(),
        dw,
        w_ranges,
        ascent,
        descent,
    };
    LoadedFont::from_parts(parts).map_err(|_| ())
}

/// Parse `/Encoding` name or dictionary with BaseEncoding + Differences.
fn parse_encoding(doc: &Document, font_dict: &Dictionary) -> (Option<String>, Vec<(u8, String)>) {
    match font_dict.get(b"Encoding") {
        Ok(Object::Name(n)) => (Some(String::from_utf8_lossy(n).into_owned()), Vec::new()),
        Ok(Object::Reference(id)) => {
            if let Ok(d) = doc.get_dictionary(*id) {
                parse_encoding_dict(d)
            } else {
                (None, Vec::new())
            }
        }
        Ok(Object::Dictionary(d)) => parse_encoding_dict(d),
        _ => (None, Vec::new()),
    }
}

fn parse_encoding_dict(d: &Dictionary) -> (Option<String>, Vec<(u8, String)>) {
    let base = d.get(b"BaseEncoding").ok().and_then(name_str);
    let mut differences = Vec::new();
    if let Ok(Object::Array(arr)) = d.get(b"Differences") {
        let mut code: i32 = 0;
        for obj in arr {
            match obj {
                Object::Integer(n) => code = *n as i32,
                Object::Name(n) => {
                    if (0..=255).contains(&code) {
                        differences.push((code as u8, String::from_utf8_lossy(n).into_owned()));
                    }
                    code += 1;
                }
                Object::String(s, _) => {
                    if (0..=255).contains(&code) {
                        differences.push((code as u8, String::from_utf8_lossy(s).into_owned()));
                    }
                    code += 1;
                }
                _ => {}
            }
        }
    }
    (base, differences)
}

fn parse_w_array(arr: &[Object]) -> Vec<(u32, u32, f32)> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < arr.len() {
        let start = match &arr[i] {
            Object::Integer(n) => *n as u32,
            _ => {
                i += 1;
                continue;
            }
        };
        i += 1;
        if i >= arr.len() {
            break;
        }
        match &arr[i] {
            Object::Array(ws) => {
                for (k, w) in ws.iter().enumerate() {
                    let width = match w {
                        Object::Integer(n) => *n as f32,
                        Object::Real(n) => *n,
                        _ => 1000.0,
                    };
                    let c = start + k as u32;
                    out.push((c, c, width));
                }
                i += 1;
            }
            Object::Integer(end) => {
                i += 1;
                if i >= arr.len() {
                    break;
                }
                let width = match &arr[i] {
                    Object::Integer(n) => *n as f32,
                    Object::Real(n) => *n,
                    _ => 1000.0,
                };
                out.push((start, *end as u32, width));
                i += 1;
            }
            _ => i += 1,
        }
    }
    out
}

fn descriptor_metrics(
    doc: &Document,
    font_dict: &Dictionary,
) -> (Option<f32>, Option<f32>, Option<f32>) {
    let mut missing = None;
    let mut ascent = None;
    let mut descent = None;
    let desc = match font_dict.get(b"FontDescriptor") {
        Ok(Object::Reference(id)) => doc.get_dictionary(*id).ok(),
        Ok(Object::Dictionary(d)) => Some(d),
        _ => None,
    };
    if let Some(d) = desc {
        if let Ok(Object::Integer(m)) = d.get(b"MissingWidth") {
            missing = Some(*m as f32);
        }
        if let Ok(Object::Integer(a)) = d.get(b"Ascent") {
            ascent = Some(*a as f32);
        } else if let Ok(Object::Real(a)) = d.get(b"Ascent") {
            ascent = Some(*a);
        }
        if let Ok(Object::Integer(a)) = d.get(b"Descent") {
            descent = Some(*a as f32);
        } else if let Ok(Object::Real(a)) = d.get(b"Descent") {
            descent = Some(*a);
        }
    }
    (missing, ascent, descent)
}

fn to_unicode_stream(
    doc: &Document,
    font_dict: &Dictionary,
    governor: &ResourceGovernor,
) -> Option<Vec<u8>> {
    let obj = match font_dict.get(b"ToUnicode").ok()? {
        Object::Reference(r) => doc.get_object(*r).ok()?.clone(),
        Object::Stream(s) => Object::Stream(s.clone()),
        _ => return None,
    };
    match obj {
        Object::Stream(s) => {
            // Prefer full stream filter decode (FlateDecode + predictors, etc.).
            // Raw-content zlib alone fails for many ToUnicode CMaps.
            match pdfparser_core::decode_stream_data(&s.dict, &s.content, governor) {
                Ok(decoded) if looks_like_cmap(&decoded) || !decoded.is_empty() => {
                    return Some(decoded);
                }
                // Shared document budget: do not inflate again outside the governor.
                Err(pdfparser_core::Error::LimitExceeded { .. }) => return None,
                _ => {}
            }
            decode_maybe_compressed(&s.content)
        }
        _ => None,
    }
}

fn decode_maybe_compressed(data: &[u8]) -> Option<Vec<u8>> {
    // Already text?
    if data.starts_with(b"/CIDInit") || data.windows(8).any(|w| w == b"begincmap") {
        return Some(data.to_vec());
    }
    let mut out = Vec::new();
    let mut z = ZlibDecoder::new(data);
    if z.read_to_end(&mut out).is_ok() && looks_like_cmap(&out) {
        return Some(out);
    }
    out.clear();
    let mut z = DeflateDecoder::new(data);
    if z.read_to_end(&mut out).is_ok() && looks_like_cmap(&out) {
        return Some(out);
    }
    // raw fallback
    Some(data.to_vec())
}

fn looks_like_cmap(data: &[u8]) -> bool {
    data.windows(8).any(|w| w == b"begincmap")
        || data.windows(9).any(|w| w == b"beginbfcha")
        || data.windows(7).any(|w| w == b"/CIDIni")
}

fn name_str(o: &Object) -> Option<String> {
    match o {
        Object::Name(n) => Some(String::from_utf8_lossy(n).into_owned()),
        Object::String(s, _) => Some(String::from_utf8_lossy(s).into_owned()),
        _ => None,
    }
}

fn int_val(o: &Object) -> Option<i64> {
    match o {
        Object::Integer(i) => Some(*i),
        _ => None,
    }
}

fn float_array(o: &Object) -> Option<Vec<f32>> {
    match o {
        Object::Array(a) => Some(
            a.iter()
                .filter_map(|x| match x {
                    Object::Integer(i) => Some(*i as f32),
                    Object::Real(r) => Some(*r),
                    _ => None,
                })
                .collect(),
        ),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::write::ZlibEncoder;
    use flate2::Compression;
    use lopdf::Stream;
    use pdfparser_core::{decode_stream_data, ResourceLimits};
    use std::io::Write;

    fn flate_tounicode() -> (Dictionary, Vec<u8>) {
        let cmap = b"/CIDInit /ProcSet findresource begin 12 dict begin begincmap \
            1 beginbfchar <41> <0041> endbfchar endcmap\n";
        let mut enc = ZlibEncoder::new(Vec::new(), Compression::default());
        enc.write_all(cmap).unwrap();
        let compressed = enc.finish().unwrap();
        let mut dict = Dictionary::new();
        dict.set("Filter", Object::Name(b"FlateDecode".to_vec()));
        dict.set("Length", Object::Integer(compressed.len() as i64));
        (dict, compressed)
    }

    fn font_dict_with_stream(stream_dict: Dictionary, content: Vec<u8>) -> Dictionary {
        let mut font_dict = Dictionary::new();
        font_dict.set(
            "ToUnicode",
            Object::Stream(Stream::new(stream_dict, content)),
        );
        font_dict
    }

    #[test]
    fn to_unicode_decode_charges_passed_governor() {
        let (sdict, content) = flate_tounicode();
        let measure = ResourceGovernor::new(ResourceLimits::default());
        let decoded_len = decode_stream_data(&sdict, &content, &measure)
            .expect("measure decode")
            .len() as u64;
        assert!(decoded_len > 0);

        let gov = ResourceGovernor::new(ResourceLimits {
            max_total_expanded_bytes: decoded_len,
            ..ResourceLimits::default()
        });
        let font_dict = font_dict_with_stream(sdict.clone(), content.clone());
        let bytes = to_unicode_stream(&Document::with_version("1.4"), &font_dict, &gov);
        assert!(
            bytes.as_ref().is_some_and(|b| looks_like_cmap(b)),
            "expected CMap from governed decode"
        );

        // Same governor must already hold the ToUnicode charge; a second decode
        // of the same size would succeed only if a fresh default governor was used.
        let again = decode_stream_data(&sdict, &content, &gov);
        assert!(
            matches!(
                again,
                Err(pdfparser_core::Error::LimitExceeded {
                    kind: pdfparser_core::LimitKind::ExpandedBytes
                })
            ),
            "expected shared-governor LimitExceeded, got {again:?}"
        );
    }

    #[test]
    fn to_unicode_limit_exceeded_does_not_bypass_governor() {
        let (sdict, content) = flate_tounicode();
        let measure = ResourceGovernor::new(ResourceLimits::default());
        let decoded_len = decode_stream_data(&sdict, &content, &measure)
            .expect("measure decode")
            .len() as u64;

        let gov = ResourceGovernor::new(ResourceLimits {
            max_total_expanded_bytes: decoded_len,
            ..ResourceLimits::default()
        });
        gov.charge_expanded(decoded_len).expect("pre-charge");

        let font_dict = font_dict_with_stream(sdict, content);
        let bytes = to_unicode_stream(&Document::with_version("1.4"), &font_dict, &gov);
        assert!(
            bytes.is_none(),
            "LimitExceeded must not fall back to ungoverned inflate"
        );
    }
}
