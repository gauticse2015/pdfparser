//! Text + path graphics state machine.
use crate::lexer::{tokenize, Token};
use pdfparser_fonts::LoadedFont;
use pdfparser_ir::{Matrix3x2, Rect, TextRun};
use std::collections::HashMap;

/// Interpretation options.
#[derive(Debug, Clone)]
pub struct InterpretOptions {
    /// Max operators.
    pub max_ops: u64,
    /// Capture stroked axis-aligned segments for table lattice.
    pub capture_rules: bool,
    /// Max thickness (user units) for a filled rect to count as a ruled line.
    /// Many PDFs draw table rules as thin filled rectangles (`re` + `f`/`f*`)
    /// rather than stroked segments (`S`). 0 disables thin-fill capture.
    pub thin_fill_rule_max: f32,
}

/// Default max thickness for thin filled rects treated as lattice rules.
/// Slightly higher than 2.0 so medium painted bars still become rules (vector
/// stand-in for Camelot-style line recovery without a full raster engine).
const DEFAULT_THIN_FILL_RULE_MAX: f32 = 3.5;

impl Default for InterpretOptions {
    fn default() -> Self {
        Self {
            max_ops: 2_000_000,
            capture_rules: true,
            thin_fill_rule_max: DEFAULT_THIN_FILL_RULE_MAX,
        }
    }
}

/// Axis-aligned (or near) stroked segment in page user space.
#[derive(Debug, Clone, Copy)]
pub struct RuleSegment {
    pub x0: f32,
    pub y0: f32,
    pub x1: f32,
    pub y1: f32,
}

impl RuleSegment {
    pub fn is_horizontal(&self, tol: f32) -> bool {
        (self.y0 - self.y1).abs() <= tol
    }
    pub fn is_vertical(&self, tol: f32) -> bool {
        (self.x0 - self.x1).abs() <= tol
    }
    pub fn len(&self) -> f32 {
        let dx = self.x1 - self.x0;
        let dy = self.y1 - self.y0;
        (dx * dx + dy * dy).sqrt()
    }
}

/// Full page interpret output.
#[derive(Debug, Clone, Default)]
pub struct InterpretResult {
    pub runs: Vec<TextRun>,
    pub rules: Vec<RuleSegment>,
    pub warnings: Vec<String>,
}

/// Interpret content stream (text + optional lattice rules).
pub fn interpret_page(
    content: &[u8],
    fonts: &HashMap<String, LoadedFont>,
    opts: &InterpretOptions,
) -> InterpretResult {
    let tokens = tokenize(content);
    let mut runs = Vec::new();
    let mut rules = Vec::new();
    let mut warnings = Vec::new();
    let mut stack: Vec<Token> = Vec::new();
    let mut gs = GState {
        ctm: Matrix3x2::identity(),
        text: TextState::default(),
    };
    let mut gstack: Vec<GState> = Vec::new();
    let mut path = PathBuilder::default();
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
                        gstack.push(gs.clone());
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
                            gs.ctm = m.concat(gs.ctm);
                        }
                        stack.clear();
                    }
                    "m" => {
                        let y = pop_num(&mut stack);
                        let x = pop_num(&mut stack);
                        path.move_to(x, y);
                        stack.clear();
                    }
                    "l" => {
                        let y = pop_num(&mut stack);
                        let x = pop_num(&mut stack);
                        path.line_to(x, y);
                        stack.clear();
                    }
                    "re" => {
                        let h = pop_num(&mut stack);
                        let w = pop_num(&mut stack);
                        let y = pop_num(&mut stack);
                        let x = pop_num(&mut stack);
                        path.rect(x, y, w, h);
                        stack.clear();
                    }
                    "h" => {
                        path.close();
                        stack.clear();
                    }
                    "n" => {
                        path.clear();
                        stack.clear();
                    }
                    "S" | "s" | "B" | "B*" | "b" | "b*" => {
                        if opts.capture_rules {
                            for seg in path.segments_user(&gs.ctm) {
                                // Keep near axis-aligned segments of meaningful length
                                if (seg.is_horizontal(1.5) || seg.is_vertical(1.5))
                                    && seg.len() >= 2.0
                                {
                                    rules.push(seg);
                                }
                            }
                        }
                        if op == "s" || op == "b" || op == "b*" {
                            // close then stroke already in segments if close called; path may need close
                        }
                        path.clear();
                        stack.clear();
                    }
                    "f" | "F" | "f*" => {
                        // Thin filled rectangles are a common way to paint table rules
                        // (ReportLab/canvas rect fill, some Word/Excel exporters). Capture
                        // them as lattice segments; thick filled shapes stay ignored.
                        if opts.capture_rules && opts.thin_fill_rule_max > 0.0 {
                            for seg in
                                path.thin_fill_rules(&gs.ctm, opts.thin_fill_rule_max, 2.0)
                            {
                                rules.push(seg);
                            }
                        }
                        path.clear();
                        stack.clear();
                    }
                    "W" | "W*" => {
                        // clip — keep path for following paint; don't clear yet
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
                    "c" | "v" | "y" => {
                        // curves — approximate as nothing for lattice Phase U
                        path.clear();
                        stack.clear();
                    }
                    "CS" | "cs" | "SC" | "SCN" | "sc" | "scn" | "G" | "g" | "RG" | "rg" | "K"
                    | "k" | "sh" | "Do" | "gs" | "MP" | "DP" | "BMC" | "BDC" | "EMC" | "BX"
                    | "EX" | "ri" | "i" | "d" | "J" | "j" | "M" | "w" | "d0" | "d1" => {
                        stack.clear();
                    }
                    _ => {
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
    InterpretResult {
        runs,
        rules,
        warnings,
    }
}

#[derive(Clone)]
struct TextState {
    font: Option<String>,
    font_size: f32,
    char_spacing: f32,
    word_spacing: f32,
    horizontal_scale: f32,
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

#[derive(Clone)]
struct GState {
    ctm: Matrix3x2,
    text: TextState,
}

#[derive(Default)]
struct PathBuilder {
    start: Option<(f32, f32)>,
    current: Option<(f32, f32)>,
    segs: Vec<((f32, f32), (f32, f32))>,
}

impl PathBuilder {
    fn clear(&mut self) {
        self.start = None;
        self.current = None;
        self.segs.clear();
    }
    fn move_to(&mut self, x: f32, y: f32) {
        self.start = Some((x, y));
        self.current = Some((x, y));
    }
    fn line_to(&mut self, x: f32, y: f32) {
        if let Some(cur) = self.current {
            self.segs.push((cur, (x, y)));
        }
        self.current = Some((x, y));
        if self.start.is_none() {
            self.start = Some((x, y));
        }
    }
    fn close(&mut self) {
        if let (Some(s), Some(c)) = (self.start, self.current) {
            if (s.0 - c.0).abs() > 1e-4 || (s.1 - c.1).abs() > 1e-4 {
                self.segs.push((c, s));
            }
            self.current = self.start;
        }
    }
    fn rect(&mut self, x: f32, y: f32, w: f32, h: f32) {
        self.move_to(x, y);
        self.line_to(x + w, y);
        self.line_to(x + w, y + h);
        self.line_to(x, y + h);
        self.close();
    }
    fn segments_user(&self, ctm: &Matrix3x2) -> Vec<RuleSegment> {
        let mut out = Vec::new();
        for ((x0, y0), (x1, y1)) in &self.segs {
            let p0 = ctm.apply(*x0, *y0);
            let p1 = ctm.apply(*x1, *y1);
            out.push(RuleSegment {
                x0: p0.x,
                y0: p0.y,
                x1: p1.x,
                y1: p1.y,
            });
        }
        out
    }

    /// Convert thin axis-aligned filled shapes into lattice rule segments.
    ///
    /// A path whose axis-aligned bounding box has one side ≤ `thin_max` and the
    /// other ≥ `min_len` is treated as a single horizontal or vertical rule
    /// through the box center (the painted “line”).
    fn thin_fill_rules(&self, ctm: &Matrix3x2, thin_max: f32, min_len: f32) -> Vec<RuleSegment> {
        if self.segs.is_empty() {
            return Vec::new();
        }
        let mut x0 = f32::INFINITY;
        let mut y0 = f32::INFINITY;
        let mut x1 = f32::NEG_INFINITY;
        let mut y1 = f32::NEG_INFINITY;
        for ((ax, ay), (bx, by)) in &self.segs {
            for &(x, y) in &[(ax, ay), (bx, by)] {
                let p = ctm.apply(*x, *y);
                x0 = x0.min(p.x);
                y0 = y0.min(p.y);
                x1 = x1.max(p.x);
                y1 = y1.max(p.y);
            }
        }
        if !x0.is_finite() {
            return Vec::new();
        }
        let w = (x1 - x0).abs();
        let h = (y1 - y0).abs();
        // Horizontal rule: flat filled band
        if h <= thin_max && w >= min_len {
            let y = (y0 + y1) * 0.5;
            return vec![RuleSegment {
                x0: x0.min(x1),
                y0: y,
                x1: x0.max(x1),
                y1: y,
            }];
        }
        // Vertical rule: thin filled strip
        if w <= thin_max && h >= min_len {
            let x = (x0 + x1) * 0.5;
            return vec![RuleSegment {
                x0: x,
                y0: y0.min(y1),
                x1: x,
                y1: y0.max(y1),
            }];
        }
        Vec::new()
    }
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
                let codes = codes_for_show(&font, s);
                for code in codes {
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

/// Back-compat alias used by older call sites.
#[allow(dead_code)]
pub fn interpret_text(
    content: &[u8],
    fonts: &HashMap<String, LoadedFont>,
    opts: &InterpretOptions,
) -> (Vec<TextRun>, Vec<String>) {
    let r = interpret_page(content, fonts, opts);
    (r.runs, r.warnings)
}
