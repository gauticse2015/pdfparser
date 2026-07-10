//! Text-focused graphics state machine.
use crate::lexer::{tokenize, Token};
use pdfparser_fonts::LoadedFont;
use pdfparser_ir::{Matrix3x2, Rect, TextRun};
use std::collections::HashMap;

/// Interpretation options.
#[derive(Debug, Clone)]
pub struct InterpretOptions {
    /// Max operators.
    pub max_ops: u64,
}

impl Default for InterpretOptions {
    fn default() -> Self {
        Self { max_ops: 2_000_000 }
    }
}

#[derive(Clone)]
struct TextState {
    font: Option<String>,
    font_size: f32,
    char_spacing: f32,
    word_spacing: f32,
    horizontal_scale: f32, // percent, default 100
    leading: f32,
    rise: f32,
    render_mode: i32,
    tm: Matrix3x2,
    tlm: Matrix3x2,
}

impl Default for TextState {
    fn default() -> Self {
        Self {
            font: None,
            font_size: 12.0,
            char_spacing: 0.0,
            word_spacing: 0.0,
            horizontal_scale: 100.0,
            leading: 0.0,
            rise: 0.0,
            render_mode: 0,
            tm: Matrix3x2::identity(),
            tlm: Matrix3x2::identity(),
        }
    }
}

struct GState {
    ctm: Matrix3x2,
    text: TextState,
}

/// Interpret content stream into paint-order text runs.
pub fn interpret_text(
    content: &[u8],
    fonts: &HashMap<String, LoadedFont>,
    opts: &InterpretOptions,
) -> (Vec<TextRun>, Vec<String>) {
    let tokens = tokenize(content);
    let mut runs = Vec::new();
    let mut warnings = Vec::new();
    let mut stack: Vec<Token> = Vec::new();
    let mut gs = GState {
        ctm: Matrix3x2::identity(),
        text: TextState::default(),
    };
    let mut gstack: Vec<GState> = Vec::new();
    let mut in_text = false;
    let mut ops = 0u64;

    let mut i = 0;
    while i < tokens.len() {
        ops += 1;
        if ops > opts.max_ops {
            warnings.push("max_page_ops exceeded".into());
            break;
        }
        match &tokens[i] {
            Token::Operator(op) => {
                let op = op.as_str();
                match op {
                    "q" => {
                        gstack.push(GState {
                            ctm: gs.ctm,
                            text: gs.text.clone(),
                        });
                        stack.clear();
                    }
                    "Q" => {
                        if let Some(prev) = gstack.pop() {
                            gs = prev;
                        }
                        stack.clear();
                    }
                    "cm" => {
                        if stack.len() >= 6 {
                            let f = pop_num(&mut stack);
                            let e = pop_num(&mut stack);
                            let d = pop_num(&mut stack);
                            let c = pop_num(&mut stack);
                            let b = pop_num(&mut stack);
                            let a = pop_num(&mut stack);
                            let m = Matrix3x2 {
                                m: [a, b, c, d, e, f],
                            };
                            // CTM = M * CTM (new matrix multiplies on left in PDF when using cm)
                            gs.ctm = m.concat(gs.ctm);
                        }
                        stack.clear();
                    }
                    "BT" => {
                        in_text = true;
                        gs.text.tm = Matrix3x2::identity();
                        gs.text.tlm = Matrix3x2::identity();
                        stack.clear();
                    }
                    "ET" => {
                        in_text = false;
                        stack.clear();
                    }
                    "Tf" => {
                        let size = pop_num(&mut stack);
                        let name = pop_name(&mut stack);
                        gs.text.font_size = size;
                        gs.text.font = name;
                        stack.clear();
                    }
                    "Tc" => {
                        gs.text.char_spacing = pop_num(&mut stack);
                        stack.clear();
                    }
                    "Tw" => {
                        gs.text.word_spacing = pop_num(&mut stack);
                        stack.clear();
                    }
                    "Tz" => {
                        gs.text.horizontal_scale = pop_num(&mut stack);
                        stack.clear();
                    }
                    "TL" => {
                        gs.text.leading = pop_num(&mut stack);
                        stack.clear();
                    }
                    "Ts" => {
                        gs.text.rise = pop_num(&mut stack);
                        stack.clear();
                    }
                    "Tr" => {
                        gs.text.render_mode = pop_num(&mut stack) as i32;
                        stack.clear();
                    }
                    "Td" => {
                        let ty = pop_num(&mut stack);
                        let tx = pop_num(&mut stack);
                        let m = Matrix3x2 {
                            m: [1.0, 0.0, 0.0, 1.0, tx, ty],
                        };
                        gs.text.tlm = m.concat(gs.text.tlm);
                        gs.text.tm = gs.text.tlm;
                        stack.clear();
                    }
                    "TD" => {
                        let ty = pop_num(&mut stack);
                        let tx = pop_num(&mut stack);
                        gs.text.leading = -ty;
                        let m = Matrix3x2 {
                            m: [1.0, 0.0, 0.0, 1.0, tx, ty],
                        };
                        gs.text.tlm = m.concat(gs.text.tlm);
                        gs.text.tm = gs.text.tlm;
                        stack.clear();
                    }
                    "Tm" => {
                        let f = pop_num(&mut stack);
                        let e = pop_num(&mut stack);
                        let d = pop_num(&mut stack);
                        let c = pop_num(&mut stack);
                        let b = pop_num(&mut stack);
                        let a = pop_num(&mut stack);
                        gs.text.tm = Matrix3x2 {
                            m: [a, b, c, d, e, f],
                        };
                        gs.text.tlm = gs.text.tm;
                        stack.clear();
                    }
                    "T*" => {
                        let m = Matrix3x2 {
                            m: [1.0, 0.0, 0.0, 1.0, 0.0, -gs.text.leading],
                        };
                        gs.text.tlm = m.concat(gs.text.tlm);
                        gs.text.tm = gs.text.tlm;
                        stack.clear();
                    }
                    "Tj" | "'" | "\"" => {
                        if op == "'" || op == "\"" {
                            if op == "\"" {
                                // aw ac string "
                                // stack: aw ac string — but string already separate
                            }
                            let m = Matrix3x2 {
                                m: [1.0, 0.0, 0.0, 1.0, 0.0, -gs.text.leading],
                            };
                            gs.text.tlm = m.concat(gs.text.tlm);
                            gs.text.tm = gs.text.tlm;
                        }
                        if let Some(bytes) = pop_string(&mut stack) {
                            if let Some(run) = show_text(&mut gs, fonts, &bytes, in_text) {
                                runs.push(run);
                            }
                        }
                        stack.clear();
                    }
                    "TJ" => {
                        // array of strings and numbers on stack as nested — our lexer flattens arrays
                        // We collect from stack until ArrayStart
                        let mut items: Vec<Token> = Vec::new();
                        while let Some(t) = stack.pop() {
                            match t {
                                Token::ArrayStart => break,
                                other => items.push(other),
                            }
                        }
                        items.reverse();
                        if let Some(run) = show_text_array(&mut gs, fonts, &items, in_text) {
                            runs.push(run);
                        }
                        stack.clear();
                    }
                    // Ignore path/paint for Phase T text
                    "m" | "l" | "c" | "v" | "y" | "h" | "re" | "S" | "s" | "f" | "F" | "f*"
                    | "B" | "B*" | "b" | "b*" | "n" | "W" | "W*" | "CS" | "cs" | "SC" | "SCN"
                    | "sc" | "scn" | "G" | "g" | "RG" | "rg" | "K" | "k" | "sh" | "Do" | "gs"
                    | "MP" | "DP" | "BMC" | "BDC" | "EMC" | "BX" | "EX" | "ri" | "i" | "d"
                    | "J" | "j" | "M" | "w" | "d0" | "d1" => {
                        stack.clear();
                    }
                    _ => {
                        // unknown: clear stack (EX23)
                        warnings.push(format!("unknown_op:{op}"));
                        stack.clear();
                    }
                }
                i += 1;
            }
            other => {
                stack.push(other.clone());
                i += 1;
            }
        }
    }
    (runs, warnings)
}

fn pop_num(stack: &mut Vec<Token>) -> f32 {
    loop {
        match stack.pop() {
            Some(Token::Number(n)) => return n,
            Some(Token::ArrayEnd) | Some(Token::ArrayStart) => continue,
            Some(_) => continue,
            None => return 0.0,
        }
    }
}

fn pop_name(stack: &mut Vec<Token>) -> Option<String> {
    while let Some(t) = stack.pop() {
        if let Token::Name(n) = t {
            return Some(n);
        }
    }
    None
}

fn pop_string(stack: &mut Vec<Token>) -> Option<Vec<u8>> {
    while let Some(t) = stack.pop() {
        match t {
            Token::LiteralString(s) | Token::HexString(s) => return Some(s),
            _ => continue,
        }
    }
    None
}

fn resolve_font(fonts: &HashMap<String, LoadedFont>, name: &str) -> (String, LoadedFont) {
    if let Some(f) = fonts.get(name) {
        return (name.to_string(), f.clone());
    }
    // try without subset prefix ABCDEF+
    if let Some(pos) = name.find('+') {
        let base = &name[pos + 1..];
        if let Some(f) = fonts.get(base) {
            return (base.to_string(), f.clone());
        }
    }
    // any font
    if let Some((k, f)) = fonts.iter().next() {
        return (k.clone(), f.clone());
    }
    ("Helvetica".into(), LoadedFont::simple_latin("Helvetica"))
}

fn show_text(
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

/// Choose code units for a PDF string. Generic: CID fonts use CMap widths (2-byte Identity);
/// simple fonts use 1-byte codes (Differences/ToUnicode applied later).
fn codes_for_show(font: &LoadedFont, bytes: &[u8]) -> Vec<u32> {
    // UTF-16BE with BOM in literal string (PDF spec)
    if bytes.len() >= 2 && bytes[0] == 0xFE && bytes[1] == 0xFF {
        let mut out = Vec::new();
        let mut i = 2;
        while i + 1 < bytes.len() {
            let u = ((bytes[i] as u32) << 8) | (bytes[i + 1] as u32);
            i += 2;
            out.push(u);
        }
        // For UTF-16BE embedded as unicode code units, treat as "identity unicode" path:
        // map via to_unicode only if cmap has them; else use char directly in show_codes_unicode
        return out;
    }
    font.codes_from_bytes(bytes)
}

fn show_text_array(
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
    let font = &font;
    let mut text = String::new();
    let mut bbox: Option<Rect> = None;
    let mut map_conf = 1.0f32;
    let mut met_conf = 1.0f32;
    let transform = gs.ctm.concat(gs.text.tm);
    let fs = gs.text.font_size;
    let th = gs.text.horizontal_scale / 100.0;
    let invisible = gs.text.render_mode == 3;

    for item in items {
        match item {
            Token::LiteralString(s) | Token::HexString(s) => {
                let codes = font.codes_from_bytes(s);
                for code in codes {
                    let (ch, cconf) = font.to_unicode(code);
                    map_conf = map_conf.min(cconf);
                    let w = font.width(code);
                    let mut adv = (w / 1000.0) * fs * th + gs.text.char_spacing;
                    if font.is_space_for_tw(code) {
                        adv += gs.text.word_spacing;
                    }
                    let p0 = gs.ctm.concat(gs.text.tm).apply(0.0, gs.text.rise);
                    let p1 = gs
                        .ctm
                        .concat(gs.text.tm)
                        .apply(adv, gs.text.rise + fs * 0.8);
                    let glyph_bb = Rect {
                        x0: p0.x.min(p1.x),
                        y0: p0.y.min(p1.y),
                        x1: p0.x.max(p1.x),
                        y1: p0.y.max(p1.y),
                    };
                    bbox = Some(match bbox {
                        Some(b) => b.union(glyph_bb),
                        None => glyph_bb,
                    });
                    text.push_str(&ch);
                    // advance Tm
                    let adj = Matrix3x2 {
                        m: [1.0, 0.0, 0.0, 1.0, adv, 0.0],
                    };
                    gs.text.tm = adj.concat(gs.text.tm);
                }
            }
            Token::Number(n) => {
                // TJ adjustment: tx -= (n/1000)*fs*Th
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
        font_size: fs,
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
    let transform = gs.ctm.concat(gs.text.tm);
    let fs = gs.text.font_size;
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
        let p0 = gs.ctm.concat(gs.text.tm).apply(0.0, gs.text.rise);
        let ascent = (font.ascent / 1000.0) * fs;
        let descent = (font.descent / 1000.0) * fs;
        let p_bl = gs.ctm.concat(gs.text.tm).apply(0.0, gs.text.rise + descent);
        let p_tr = gs.ctm.concat(gs.text.tm).apply(adv, gs.text.rise + ascent);
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
        font_size: fs,
        mapping_confidence: map_conf,
        metrics_confidence: 0.9,
        mcid: None,
        invisible,
        from_actual_text: false,
    })
}
