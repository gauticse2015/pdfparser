//! Text showing operators (Tj / TJ / quote / double-quote).
use super::state::GState;
use crate::lexer::Token;
use pdfparser_fonts::LoadedFont;
use pdfparser_ir::{Matrix3x2, Rect, TextRun};
use std::collections::HashMap;

fn resolve_font(fonts: &HashMap<String, LoadedFont>, name: &str) -> (String, LoadedFont) {
    if let Some(f) = fonts.get(name) {
        return (name.to_string(), f.clone());
    }
    if let Some(pos) = name.find('+') {
        let base = &name[pos + 1..];
        if let Some(f) = fonts.get(base) {
            return (base.to_string(), f.clone());
        }
    }
    if let Some((k, f)) = fonts.iter().next() {
        return (k.clone(), f.clone());
    }
    ("Helvetica".into(), LoadedFont::simple_latin("Helvetica"))
}

fn codes_for_show(font: &LoadedFont, bytes: &[u8]) -> Vec<u32> {
    if bytes.len() >= 2 && bytes[0] == 0xFE && bytes[1] == 0xFF {
        let mut out = Vec::new();
        let mut i = 2;
        while i + 1 < bytes.len() {
            let u = ((bytes[i] as u32) << 8) | (bytes[i + 1] as u32);
            i += 2;
            out.push(u);
        }
        return out;
    }
    font.codes_from_bytes(bytes)
}

pub(crate) fn show_text(
    gs: &mut GState,
    fonts: &HashMap<String, LoadedFont>,
    bytes: &[u8],
    in_text: bool,
) -> Option<TextRun> {
    if !in_text {
        return None;
    }
    let font_name = gs.text.font.clone().unwrap_or_else(|| "Helvetica".into());
    let (resolved, font) = resolve_font(fonts, &font_name);
    let codes = codes_for_show(&font, bytes);
    show_codes(gs, &font, &resolved, &codes)
}

pub(crate) fn show_text_array(
    gs: &mut GState,
    fonts: &HashMap<String, LoadedFont>,
    items: &[Token],
    in_text: bool,
) -> Option<TextRun> {
    if !in_text {
        return None;
    }
    let font_name = gs.text.font.clone().unwrap_or_else(|| "Helvetica".into());
    let (resolved, font) = resolve_font(fonts, &font_name);
    let font_name = resolved;
    let mut text = String::new();
    let mut bbox: Option<Rect> = None;
    let mut map_conf = 1.0f32;
    let mut met_conf = 1.0f32;
    // Text rendering matrix Trm = Tm × CTM (ISO 32000 §9.4.4).
    let transform = gs.text.tm.concat(gs.ctm);
    let fs = gs.text.font_size;
    // IR contract: font_size is user-space (Trm linear scale × Tf).
    let user_fs = (transform.linear_scale() * fs).abs().max(1e-3);
    let th = gs.text.horizontal_scale / 100.0;
    let invisible = gs.text.render_mode == 3;

    for item in items {
        match item {
            Token::LiteralString(s) | Token::HexString(s) => {
                let codes = codes_for_show(&font, s);
                for code in codes {
                    let (ch, cconf) = font.to_unicode(code);
                    map_conf = map_conf.min(cconf);
                    let w = font.width(code);
                    let mut adv = (w / 1000.0) * fs * th + gs.text.char_spacing;
                    if font.is_space_for_tw(code) {
                        adv += gs.text.word_spacing;
                    }
                    let trm = gs.text.tm.concat(gs.ctm);
                    let p0 = trm.apply(0.0, gs.text.rise);
                    let ascent = (font.ascent / 1000.0) * fs;
                    let descent = (font.descent / 1000.0) * fs;
                    let p_bl = trm.apply(0.0, gs.text.rise + descent);
                    let p_tr = trm.apply(adv, gs.text.rise + ascent);
                    let glyph_bb = Rect {
                        x0: p0.x.min(p_bl.x).min(p_tr.x),
                        y0: p0.y.min(p_bl.y).min(p_tr.y),
                        x1: p0.x.max(p_bl.x).max(p_tr.x),
                        y1: p0.y.max(p_bl.y).max(p_tr.y),
                    };
                    bbox = Some(match bbox {
                        Some(b) => b.union(glyph_bb),
                        None => glyph_bb,
                    });
                    text.push_str(&ch);
                    let adj = Matrix3x2 {
                        m: [1.0, 0.0, 0.0, 1.0, adv, 0.0],
                    };
                    gs.text.tm = adj.concat(gs.text.tm);
                }
            }
            Token::Number(n) => {
                let dx = -(*n / 1000.0) * fs * th;
                let adj = Matrix3x2 {
                    m: [1.0, 0.0, 0.0, 1.0, dx, 0.0],
                };
                gs.text.tm = adj.concat(gs.text.tm);
                met_conf = met_conf.min(0.95);
            }
            _ => {}
        }
    }
    if text.is_empty() {
        return None;
    }
    Some(TextRun {
        text,
        bbox: bbox.unwrap_or(Rect::zero()),
        transform,
        font_name: Some(font_name),
        font_size: user_fs,
        mapping_confidence: map_conf,
        metrics_confidence: met_conf,
        mcid: None,
        invisible,
        from_actual_text: false,
    })
}

fn show_codes(
    gs: &mut GState,
    font: &LoadedFont,
    font_name: &str,
    codes: &[u32],
) -> Option<TextRun> {
    let mut text = String::new();
    let mut bbox: Option<Rect> = None;
    let mut map_conf = 1.0f32;
    // Text rendering matrix Trm = Tm × CTM (ISO 32000 §9.4.4).
    let transform = gs.text.tm.concat(gs.ctm);
    let fs = gs.text.font_size;
    let user_fs = (transform.linear_scale() * fs).abs().max(1e-3);
    let th = gs.text.horizontal_scale / 100.0;
    let invisible = gs.text.render_mode == 3;

    for &code in codes {
        let (ch, cconf) = font.to_unicode(code);
        map_conf = map_conf.min(cconf);
        let w = font.width(code);
        let mut adv = (w / 1000.0) * fs * th + gs.text.char_spacing;
        if font.is_space_for_tw(code) {
            adv += gs.text.word_spacing;
        }
        let trm = gs.text.tm.concat(gs.ctm);
        let p0 = trm.apply(0.0, gs.text.rise);
        let ascent = (font.ascent / 1000.0) * fs;
        let descent = (font.descent / 1000.0) * fs;
        let p_bl = trm.apply(0.0, gs.text.rise + descent);
        let p_tr = trm.apply(adv, gs.text.rise + ascent);
        let glyph_bb = Rect {
            x0: p0.x.min(p_bl.x).min(p_tr.x),
            y0: p0.y.min(p_bl.y).min(p_tr.y),
            x1: p0.x.max(p_bl.x).max(p_tr.x),
            y1: p0.y.max(p_bl.y).max(p_tr.y),
        };
        bbox = Some(match bbox {
            Some(b) => b.union(glyph_bb),
            None => glyph_bb,
        });
        text.push_str(&ch);
        let adj = Matrix3x2 {
            m: [1.0, 0.0, 0.0, 1.0, adv, 0.0],
        };
        gs.text.tm = adj.concat(gs.text.tm);
    }
    if text.is_empty() {
        return None;
    }
    Some(TextRun {
        text,
        bbox: bbox.unwrap_or(Rect::zero()),
        transform,
        font_name: Some(font_name.to_string()),
        font_size: user_fs,
        mapping_confidence: map_conf,
        metrics_confidence: 0.9,
        mcid: None,
        invisible,
        from_actual_text: false,
    })
}
