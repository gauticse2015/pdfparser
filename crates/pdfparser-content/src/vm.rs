//! Text + path graphics state machine.
use crate::lexer::{tokenize, Token};
use pdfparser_fonts::LoadedFont;
use pdfparser_ir::{Matrix3x2, ObjectId, Rect, TextRun};
use std::collections::HashMap;

/// Max Form XObject nesting depth (PR2a / K19).
pub const MAX_FORM_DEPTH: u32 = 4;
/// Max Form expansions on a single page interpret.
pub const MAX_FORM_EXPANSIONS_PER_PAGE: u32 = 32;
/// Floor for per-form operator budget.
const PER_FORM_MAX_OPS_FLOOR: u64 = 50_000;

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
    /// Capture image XObject placements (`Do`) for raster line sensing.
    pub capture_image_placements: bool,
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
            capture_image_placements: true,
        }
    }
}

/// Image XObject drawn via `Do` (unit square mapped by current CTM).
#[derive(Debug, Clone)]
pub struct ImagePlacement {
    /// Resource name (without leading `/`).
    pub name: String,
    /// CTM at paint time. Unit square (0,0)–(1,1) maps to page space.
    pub ctm: Matrix3x2,
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
    /// Image XObject placements for raster line sensing.
    pub image_placements: Vec<ImagePlacement>,
    pub warnings: Vec<String>,
}

/// Resolved Form XObject payload injected by the façade (PR2a / K19).
///
/// The content VM never opens the PDF object graph; callers supply stream
/// bytes, matrix, and a stable id for cycle detection.
#[derive(Debug, Clone)]
pub struct FormXObject {
    /// Object id for cycle detection (`(num, gen)`).
    pub id: ObjectId,
    /// Decoded form content stream bytes.
    pub stream: Vec<u8>,
    /// Form `/Matrix` (identity if absent).
    pub matrix: Matrix3x2,
    /// Optional form `/BBox` in form space.
    pub b_box: Option<Rect>,
}

/// Injected from the façade to resolve Form XObjects by resource name.
///
/// The VM calls [`enter_form`](FormContentResolver::enter_form) /
/// [`leave_form`](FormContentResolver::leave_form) around recursive
/// expansion so the façade can maintain a resource scope stack.
pub trait FormContentResolver {
    /// Resolve Form XObject by resource name under the current resource stack.
    fn resolve_form(&mut self, name: &str) -> Option<FormXObject>;

    /// Enter form resource scope after resolve, before interpreting form content.
    fn enter_form(&mut self, form: &FormXObject) {
        let _ = form;
    }

    /// Leave form resource scope after form content is interpreted.
    fn leave_form(&mut self) {}
}

/// Interpret content stream (text + optional lattice rules).
///
/// Equivalent to [`interpret_page_with_resolver`] with `resolver = None`
/// (no Form XObject expansion).
pub fn interpret_page(
    content: &[u8],
    fonts: &HashMap<String, LoadedFont>,
    opts: &InterpretOptions,
) -> InterpretResult {
    interpret_page_with_resolver(content, fonts, opts, None)
}

/// Interpret content stream with optional Form XObject expansion (PR2a).
///
/// On `Do`: try form resolve via `resolver` first; if not a form (or no
/// resolver), record an image placement when `capture_image_placements`.
pub fn interpret_page_with_resolver(
    content: &[u8],
    fonts: &HashMap<String, LoadedFont>,
    opts: &InterpretOptions,
    mut resolver: Option<&mut dyn FormContentResolver>,
) -> InterpretResult {
    let mut state = InterpretState {
        fonts,
        opts,
        runs: Vec::new(),
        rules: Vec::new(),
        image_placements: Vec::new(),
        warnings: Vec::new(),
        ops: 0,
        form_expansions: 0,
        form_depth: 0,
        form_cycle: Vec::new(),
    };
    let mut gs = GState {
        ctm: Matrix3x2::identity(),
        text: TextState::default(),
        dash: Vec::new(),
        dash_phase: 0.0,
        clip_rect: None,
    };
    let mut gstack: Vec<GState> = Vec::new();
    interpret_stream(
        &mut state,
        content,
        &mut gs,
        &mut gstack,
        &mut resolver,
        None,
    );
    InterpretResult {
        runs: state.runs,
        rules: state.rules,
        image_placements: state.image_placements,
        warnings: state.warnings,
    }
}

struct InterpretState<'a> {
    fonts: &'a HashMap<String, LoadedFont>,
    opts: &'a InterpretOptions,
    runs: Vec<TextRun>,
    rules: Vec<RuleSegment>,
    image_placements: Vec<ImagePlacement>,
    warnings: Vec<String>,
    ops: u64,
    form_expansions: u32,
    form_depth: u32,
    form_cycle: Vec<ObjectId>,
}

fn per_form_max_ops(max_ops: u64) -> u64 {
    (max_ops / 4).max(PER_FORM_MAX_OPS_FLOOR)
}

/// Interpret one content stream. `form_ops_left` is `Some` when inside a form
/// and tracks the remaining per-form op budget.
fn interpret_stream(
    state: &mut InterpretState<'_>,
    content: &[u8],
    gs: &mut GState,
    gstack: &mut Vec<GState>,
    resolver: &mut Option<&mut dyn FormContentResolver>,
    mut form_ops_left: Option<u64>,
) {
    let tokens = tokenize(content);
    let mut stack: Vec<Token> = Vec::new();
    let mut path = PathBuilder::default();
    let mut in_text = false;

    let mut i = 0;
    while i < tokens.len() {
        state.ops += 1;
        if state.ops > state.opts.max_ops {
            state.warnings.push("max_page_ops exceeded".into());
            break;
        }
        if let Some(ref mut left) = form_ops_left {
            if *left == 0 {
                state.warnings.push("per_form_max_ops exceeded".into());
                break;
            }
            *left -= 1;
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
                            *gs = prev;
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
                        if state.opts.capture_rules {
                            for seg in path.segments_user(&gs.ctm) {
                                // Keep near axis-aligned segments of meaningful length
                                if !(seg.is_horizontal(1.5) || seg.is_vertical(1.5)) {
                                    continue;
                                }
                                let segs: Vec<RuleSegment> = if gs.dash.is_empty() {
                                    if seg.len() >= 2.0 {
                                        vec![seg]
                                    } else {
                                        Vec::new()
                                    }
                                } else {
                                    // Expand dashed H/V strokes into ON pieces only.
                                    expand_dash_segment(
                                        seg,
                                        &gs.dash,
                                        gs.dash_phase,
                                    )
                                };
                                for piece in segs {
                                    if let Some(clipped) = clip_rule_segment(piece, gs.clip_rect) {
                                        if clipped.len() >= 1.0 {
                                            state.rules.push(clipped);
                                        }
                                    }
                                }
                            }
                            // Fill+stroke ops (B/b) also paint thin filled rects as rules
                            // (common in Word/Excel PDF export). Stroke-only path capture
                            // misses fill-drawn grid lines when stroke width is zero-ish.
                            if matches!(op, "B" | "B*" | "b" | "b*")
                                && state.opts.thin_fill_rule_max > 0.0
                            {
                                for seg in path.thin_fill_rules(
                                    &gs.ctm,
                                    state.opts.thin_fill_rule_max,
                                    2.0,
                                ) {
                                    if let Some(clipped) = clip_rule_segment(seg, gs.clip_rect) {
                                        if clipped.len() >= 1.0 {
                                            state.rules.push(clipped);
                                        }
                                    }
                                }
                            }
                        }
                        if op == "s" || op == "b" || op == "b*" {
                            // close then stroke already in segments if close called; path may need close
                        }
                        path.clear();
                        stack.clear();
                    }
                    "d" => {
                        // array phase d — dash pattern (empty array = solid)
                        let phase = pop_num(&mut stack);
                        let mut dash = Vec::new();
                        while let Some(t) = stack.pop() {
                            match t {
                                Token::ArrayStart => break,
                                Token::Number(n) => dash.push(n),
                                Token::ArrayEnd => continue,
                                _ => continue,
                            }
                        }
                        dash.reverse();
                        gs.dash = dash;
                        gs.dash_phase = phase;
                        stack.clear();
                    }
                    "f" | "F" | "f*" => {
                        // Thin filled rectangles are a common way to paint table rules
                        // (ReportLab/canvas rect fill, some Word/Excel exporters). Capture
                        // them as lattice segments; thick filled shapes stay ignored.
                        if state.opts.capture_rules && state.opts.thin_fill_rule_max > 0.0 {
                            for seg in path.thin_fill_rules(
                                &gs.ctm,
                                state.opts.thin_fill_rule_max,
                                2.0,
                            ) {
                                if let Some(clipped) = clip_rule_segment(seg, gs.clip_rect) {
                                    if clipped.len() >= 1.0 {
                                        state.rules.push(clipped);
                                    }
                                }
                            }
                        }
                        path.clear();
                        stack.clear();
                    }
                    "W" | "W*" => {
                        // PR2c: axis-aligned clip from path bbox (user space after CTM).
                        // Intersect with existing clip if present. Path kept for following paint.
                        if let Some(bb) = path.axis_aligned_bbox_user(&gs.ctm) {
                            gs.clip_rect = Some(match gs.clip_rect {
                                None => bb,
                                Some(prev) => intersect_rect(prev, bb),
                            });
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
                            let m = Matrix3x2 {
                                m: [1.0, 0.0, 0.0, 1.0, 0.0, -gs.text.leading],
                            };
                            gs.text.tlm = m.concat(gs.text.tlm);
                            gs.text.tm = gs.text.tlm;
                        }
                        if let Some(bytes) = pop_string(&mut stack) {
                            if let Some(run) =
                                show_text(gs, state.fonts, &bytes, in_text)
                            {
                                state.runs.push(run);
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
                        if let Some(run) =
                            show_text_array(gs, state.fonts, &items, in_text)
                        {
                            state.runs.push(run);
                        }
                        stack.clear();
                    }
                    "c" | "v" | "y" => {
                        // curves — approximate as nothing for lattice Phase U
                        path.clear();
                        stack.clear();
                    }
                    "Do" => {
                        // Paint XObject: Form expansion (if resolver) else image placement.
                        let name = pop_name(&mut stack);
                        stack.clear();
                        if let Some(name) = name {
                            let mut expanded = false;
                            if resolver.is_some() {
                                expanded = try_expand_form(
                                    state, gs, resolver, &name,
                                );
                            }
                            if !expanded && state.opts.capture_image_placements {
                                state.image_placements.push(ImagePlacement {
                                    name,
                                    ctm: gs.ctm,
                                });
                            }
                        }
                    }
                    "CS" | "cs" | "SC" | "SCN" | "sc" | "scn" | "G" | "g" | "RG" | "rg" | "K"
                    | "k" | "sh" | "gs" | "MP" | "DP" | "BMC" | "BDC" | "EMC" | "BX" | "EX"
                    | "ri" | "i" | "J" | "j" | "M" | "w" | "d0" | "d1" => {
                        stack.clear();
                    }
                    _ => {
                        state.warnings.push(format!("unknown_op:{op}"));
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
}

/// Attempt Form XObject expansion. Returns true if a form was expanded (or
/// deliberately skipped for cycle/depth/budget — not treated as image).
fn try_expand_form(
    state: &mut InterpretState<'_>,
    gs: &GState,
    resolver: &mut Option<&mut dyn FormContentResolver>,
    name: &str,
) -> bool {
    let Some(res) = resolver.as_mut() else {
        return false;
    };
    let Some(form) = res.resolve_form(name) else {
        return false;
    };

    // Form resolved: do not fall through to image placement even if we skip expand.
    if state.form_cycle.iter().any(|id| *id == form.id) {
        state
            .warnings
            .push(format!("form_cycle_skipped:{name}"));
        return true;
    }
    if state.form_depth >= MAX_FORM_DEPTH {
        state
            .warnings
            .push(format!("form_depth_exceeded:{name}"));
        return true;
    }
    if state.form_expansions >= MAX_FORM_EXPANSIONS_PER_PAGE {
        state
            .warnings
            .push(format!("form_expansions_exceeded:{name}"));
        return true;
    }

    state.form_expansions += 1;
    state.form_depth += 1;
    state.form_cycle.push(form.id);

    res.enter_form(&form);

    // CTM' = form.matrix × CTM (PDF form paint); isolate GState / path / q-stack.
    let mut form_gs = gs.clone();
    form_gs.ctm = form.matrix.concat(form_gs.ctm);
    let mut form_gstack: Vec<GState> = Vec::new();
    let budget = per_form_max_ops(state.opts.max_ops);

    interpret_stream(
        state,
        &form.stream,
        &mut form_gs,
        &mut form_gstack,
        resolver,
        Some(budget),
    );

    if let Some(res) = resolver.as_mut() {
        res.leave_form();
    }
    state.form_cycle.pop();
    state.form_depth -= 1;
    true
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
    /// Dash array from `d` (empty = solid stroke). Alternating on/off lengths.
    dash: Vec<f32>,
    /// Dash phase from `d` (distance into pattern at stroke start).
    dash_phase: f32,
    /// Axis-aligned clip rectangle in user space after CTM (PR2c subset).
    /// `None` = no clip. Intersected on successive `W`/`W*`.
    clip_rect: Option<Rect>,
}

/// Intersect two axis-aligned rects (may be empty).
fn intersect_rect(a: Rect, b: Rect) -> Rect {
    Rect {
        x0: a.x0.max(b.x0),
        y0: a.y0.max(b.y0),
        x1: a.x1.min(b.x1),
        y1: a.y1.min(b.y1),
    }
}

/// Clip a near-axis-aligned rule to an optional clip rect (PR2c).
/// Returns `None` if fully outside or degenerate.
fn clip_rule_segment(seg: RuleSegment, clip: Option<Rect>) -> Option<RuleSegment> {
    let Some(c) = clip else {
        return Some(seg);
    };
    if c.x1 <= c.x0 || c.y1 <= c.y0 {
        return None;
    }
    let tol = 1.5f32;
    if seg.is_horizontal(tol) {
        let y = (seg.y0 + seg.y1) * 0.5;
        if y < c.y0 - tol || y > c.y1 + tol {
            return None;
        }
        let x0 = seg.x0.min(seg.x1).max(c.x0);
        let x1 = seg.x0.max(seg.x1).min(c.x1);
        if x1 - x0 < 1.0 {
            return None;
        }
        return Some(RuleSegment {
            x0,
            y0: y,
            x1,
            y1: y,
        });
    }
    if seg.is_vertical(tol) {
        let x = (seg.x0 + seg.x1) * 0.5;
        if x < c.x0 - tol || x > c.x1 + tol {
            return None;
        }
        let y0 = seg.y0.min(seg.y1).max(c.y0);
        let y1 = seg.y0.max(seg.y1).min(c.y1);
        if y1 - y0 < 1.0 {
            return None;
        }
        return Some(RuleSegment {
            x0: x,
            y0,
            x1: x,
            y1,
        });
    }
    None
}

/// Expand an axis-aligned stroked segment through a PDF dash pattern.
///
/// Walks distance along `seg`, alternating on/off from `dash` (phase applied).
/// Emits a [`RuleSegment`] for each ON interval with length ≥ 1.0.
/// Empty / all-zero dash → single solid segment (caller usually checks empty).
fn expand_dash_segment(seg: RuleSegment, dash: &[f32], phase: f32) -> Vec<RuleSegment> {
    if dash.is_empty() || dash.iter().all(|&d| d <= 0.0) {
        return if seg.len() >= 1.0 {
            vec![seg]
        } else {
            Vec::new()
        };
    }

    // PDF: odd-length arrays are effectively doubled to even length.
    let mut pattern: Vec<f32> = dash.iter().map(|&d| d.max(0.0)).collect();
    if pattern.len() % 2 == 1 {
        let copy = pattern.clone();
        pattern.extend_from_slice(&copy);
    }
    let pattern_len: f32 = pattern.iter().sum();
    if pattern_len <= 0.0 {
        return if seg.len() >= 1.0 {
            vec![seg]
        } else {
            Vec::new()
        };
    }

    let total_len = seg.len();
    if total_len < 1.0 {
        return Vec::new();
    }

    let dx = (seg.x1 - seg.x0) / total_len;
    let dy = (seg.y1 - seg.y0) / total_len;

    // Locate start position inside the repeating pattern.
    let mut dist_in_pattern = phase.rem_euclid(pattern_len);
    let mut idx = 0usize;
    let mut acc = 0.0f32;
    while idx < pattern.len() {
        let next = acc + pattern[idx];
        if next > dist_in_pattern + 1e-6 {
            break;
        }
        acc = next;
        idx += 1;
    }
    if idx >= pattern.len() {
        idx = 0;
        acc = 0.0;
        dist_in_pattern = 0.0;
    }
    let mut remaining_in_elem = (pattern[idx] - (dist_in_pattern - acc)).max(0.0);
    if remaining_in_elem < 1e-6 {
        idx = (idx + 1) % pattern.len();
        remaining_in_elem = pattern[idx];
    }
    let mut is_on = idx % 2 == 0;

    let mut out = Vec::new();
    let mut pos = 0.0f32;
    while pos < total_len - 1e-6 {
        let avail = total_len - pos;
        let step = remaining_in_elem.min(avail).max(0.0);
        if step < 1e-8 {
            // Zero-length dash element: advance pattern without moving.
            idx = (idx + 1) % pattern.len();
            remaining_in_elem = pattern[idx];
            is_on = idx % 2 == 0;
            continue;
        }
        if is_on && step >= 1.0 {
            out.push(RuleSegment {
                x0: seg.x0 + dx * pos,
                y0: seg.y0 + dy * pos,
                x1: seg.x0 + dx * (pos + step),
                y1: seg.y0 + dy * (pos + step),
            });
        }
        pos += step;
        remaining_in_elem -= step;
        if remaining_in_elem < 1e-6 {
            idx = (idx + 1) % pattern.len();
            remaining_in_elem = pattern[idx];
            is_on = idx % 2 == 0;
        }
    }
    out
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

    /// Axis-aligned bounding box of the path in user space (after CTM).
    fn axis_aligned_bbox_user(&self, ctm: &Matrix3x2) -> Option<Rect> {
        let segs = self.segments_user(ctm);
        if segs.is_empty() {
            return None;
        }
        let mut x0 = f32::INFINITY;
        let mut y0 = f32::INFINITY;
        let mut x1 = f32::NEG_INFINITY;
        let mut y1 = f32::NEG_INFINITY;
        for s in &segs {
            x0 = x0.min(s.x0.min(s.x1));
            y0 = y0.min(s.y0.min(s.y1));
            x1 = x1.max(s.x0.max(s.x1));
            y1 = y1.max(s.y0.max(s.y1));
        }
        if !x0.is_finite() || (x1 - x0) < 1e-3 || (y1 - y0) < 1e-3 {
            return None;
        }
        Some(Rect { x0, y0, x1, y1 })
    }

    /// Convert thin axis-aligned filled shapes into lattice rule segments.
    ///
    /// A path whose axis-aligned bounding box has one side ≤ `thin_max` and the
    /// other ≥ `min_len` is treated as a single horizontal or vertical rule
    /// through the box center (the painted “line”).
    ///
    /// **Subpath-aware:** PDF exporters often accumulate many closed thin
    /// rectangles then paint with a single `f`. Using the union bbox would look
    /// like a fat area and drop all rules. Split into connected closed subpaths
    /// (break when a segment does not continue from the previous endpoint) and
    /// emit a rule per thin subpath.
    fn thin_fill_rules(&self, ctm: &Matrix3x2, thin_max: f32, min_len: f32) -> Vec<RuleSegment> {
        if self.segs.is_empty() {
            return Vec::new();
        }
        let mut out = Vec::new();
        let mut sub: Vec<((f32, f32), (f32, f32))> = Vec::new();
        let flush = |sub: &mut Vec<((f32, f32), (f32, f32))>,
                     out: &mut Vec<RuleSegment>,
                     ctm: &Matrix3x2| {
            if sub.is_empty() {
                return;
            }
            if let Some(seg) = thin_fill_bbox_rule(sub, ctm, thin_max, min_len) {
                out.push(seg);
            }
            sub.clear();
        };
        for &(a, b) in &self.segs {
            if let Some((_, prev_b)) = sub.last() {
                let cont = (prev_b.0 - a.0).abs() < 1e-3 && (prev_b.1 - a.1).abs() < 1e-3;
                if !cont {
                    flush(&mut sub, &mut out, ctm);
                }
            }
            sub.push((a, b));
        }
        flush(&mut sub, &mut out, ctm);
        // Fallback: whole-path bbox (single open/closed thin rect).
        if out.is_empty() {
            if let Some(seg) = thin_fill_bbox_rule(&self.segs, ctm, thin_max, min_len) {
                out.push(seg);
            }
        }
        out
    }
}

/// Axis-aligned thin bbox → one H or V rule, or None.
fn thin_fill_bbox_rule(
    segs: &[((f32, f32), (f32, f32))],
    ctm: &Matrix3x2,
    thin_max: f32,
    min_len: f32,
) -> Option<RuleSegment> {
    if segs.is_empty() {
        return None;
    }
    let mut x0 = f32::INFINITY;
    let mut y0 = f32::INFINITY;
    let mut x1 = f32::NEG_INFINITY;
    let mut y1 = f32::NEG_INFINITY;
    for ((ax, ay), (bx, by)) in segs {
        for &(x, y) in &[(ax, ay), (bx, by)] {
            let p = ctm.apply(*x, *y);
            x0 = x0.min(p.x);
            y0 = y0.min(p.y);
            x1 = x1.max(p.x);
            y1 = y1.max(p.y);
        }
    }
    if !x0.is_finite() {
        return None;
    }
    let w = (x1 - x0).abs();
    let h = (y1 - y0).abs();
    if h <= thin_max && w >= min_len {
        let y = (y0 + y1) * 0.5;
        return Some(RuleSegment {
            x0: x0.min(x1),
            y0: y,
            x1: x0.max(x1),
            y1: y,
        });
    }
    if w <= thin_max && h >= min_len {
        let x = (x0 + x1) * 0.5;
        return Some(RuleSegment {
            x0: x,
            y0: y0.min(y1),
            x1: x,
            y1: y0.max(y1),
        });
    }
    None
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

#[cfg(test)]
mod tests {
    use super::*;

    struct MockResolver {
        forms: HashMap<String, FormXObject>,
        enter_count: u32,
        leave_count: u32,
    }

    impl FormContentResolver for MockResolver {
        fn resolve_form(&mut self, name: &str) -> Option<FormXObject> {
            self.forms.get(name).cloned()
        }
        fn enter_form(&mut self, _form: &FormXObject) {
            self.enter_count += 1;
        }
        fn leave_form(&mut self) {
            self.leave_count += 1;
        }
    }

    fn empty_fonts() -> HashMap<String, LoadedFont> {
        HashMap::new()
    }

    #[test]
    fn form_expansion_captures_stroked_rules() {
        // Form draws a thin horizontal and vertical stroke.
        let form_stream = b"0 0 m 100 0 l S\n0 0 m 0 80 l S\n";
        let mut resolver = MockResolver {
            forms: HashMap::from([(
                "Fm1".into(),
                FormXObject {
                    id: ObjectId { num: 5, gen: 0 },
                    stream: form_stream.to_vec(),
                    matrix: Matrix3x2::identity(),
                    b_box: Some(Rect {
                        x0: 0.0,
                        y0: 0.0,
                        x1: 100.0,
                        y1: 80.0,
                    }),
                },
            )]),
            enter_count: 0,
            leave_count: 0,
        };
        let page = b"/Fm1 Do";
        let opts = InterpretOptions::default();
        let fonts = empty_fonts();
        let result =
            interpret_page_with_resolver(page, &fonts, &opts, Some(&mut resolver));

        assert!(
            result.rules.len() >= 2,
            "expected rules from form, got {} warnings={:?}",
            result.rules.len(),
            result.warnings
        );
        let has_h = result.rules.iter().any(|r| r.is_horizontal(1.5) && r.len() >= 50.0);
        let has_v = result.rules.iter().any(|r| r.is_vertical(1.5) && r.len() >= 50.0);
        assert!(has_h, "missing horizontal rule: {:?}", result.rules);
        assert!(has_v, "missing vertical rule: {:?}", result.rules);
        assert_eq!(resolver.enter_count, 1);
        assert_eq!(resolver.leave_count, 1);
        // Form must not also be recorded as an image placement.
        assert!(
            result.image_placements.is_empty(),
            "form should not be image placement: {:?}",
            result.image_placements
        );
    }

    #[test]
    fn form_expansion_thin_fill_rect() {
        // Thin filled band (h=1) → horizontal rule.
        let form_stream = b"10 50 120 1 re f\n";
        let mut resolver = MockResolver {
            forms: HashMap::from([(
                "R1".into(),
                FormXObject {
                    id: ObjectId { num: 9, gen: 0 },
                    stream: form_stream.to_vec(),
                    matrix: Matrix3x2 {
                        m: [1.0, 0.0, 0.0, 1.0, 20.0, 30.0],
                    },
                    b_box: None,
                },
            )]),
            enter_count: 0,
            leave_count: 0,
        };
        let page = b"/R1 Do";
        let result = interpret_page_with_resolver(
            page,
            &empty_fonts(),
            &InterpretOptions::default(),
            Some(&mut resolver),
        );
        assert_eq!(result.rules.len(), 1, "rules={:?}", result.rules);
        let r = &result.rules[0];
        assert!(r.is_horizontal(1.5), "{r:?}");
        // Matrix translates by (20, 30): x in [30, 150], y ≈ 80.5
        assert!((r.y0 - 80.5).abs() < 1.0, "y={:?}", r.y0);
        assert!((r.x0 - 30.0).abs() < 1.0 && (r.x1 - 150.0).abs() < 1.0, "{r:?}");
    }

    #[test]
    fn form_cycle_detection() {
        // Fm1 Do → resolves to stream that also Do's Fm1.
        let form_stream = b"/Fm1 Do\n0 0 m 50 0 l S\n";
        let mut resolver = MockResolver {
            forms: HashMap::from([(
                "Fm1".into(),
                FormXObject {
                    id: ObjectId { num: 1, gen: 0 },
                    stream: form_stream.to_vec(),
                    matrix: Matrix3x2::identity(),
                    b_box: None,
                },
            )]),
            enter_count: 0,
            leave_count: 0,
        };
        let result = interpret_page_with_resolver(
            b"/Fm1 Do",
            &empty_fonts(),
            &InterpretOptions::default(),
            Some(&mut resolver),
        );
        assert!(
            result.warnings.iter().any(|w| w.contains("form_cycle")),
            "warnings={:?}",
            result.warnings
        );
        // Outer expansion still paints the stroke after nested Do is skipped.
        assert!(!result.rules.is_empty(), "rules={:?}", result.rules);
        assert_eq!(resolver.enter_count, 1);
        assert_eq!(resolver.leave_count, 1);
    }

    #[test]
    fn no_resolver_records_image_placement() {
        let result = interpret_page(
            b"/Im0 Do",
            &empty_fonts(),
            &InterpretOptions::default(),
        );
        assert_eq!(result.image_placements.len(), 1);
        assert_eq!(result.image_placements[0].name, "Im0");
        assert!(result.rules.is_empty());
    }

    #[test]
    fn interpret_page_wrapper_matches_none_resolver() {
        let content = b"0 0 m 40 0 l S";
        let fonts = empty_fonts();
        let opts = InterpretOptions::default();
        let a = interpret_page(content, &fonts, &opts);
        let b = interpret_page_with_resolver(content, &fonts, &opts, None);
        assert_eq!(a.rules.len(), b.rules.len());
        assert_eq!(a.runs.len(), b.runs.len());
    }

    #[test]
    fn dash_horizontal_line_emits_on_segments() {
        // [4 2] 0 d — 4 on, 2 off. Line 0→20 → ON: [0,4], [6,10], [12,16], [18,20]
        let content = b"[4 2] 0 d\n0 0 m 20 0 l S\n";
        let result = interpret_page(content, &empty_fonts(), &InterpretOptions::default());
        let h: Vec<_> = result
            .rules
            .iter()
            .filter(|r| r.is_horizontal(1.5))
            .collect();
        assert!(
            h.len() >= 3,
            "expected multiple H ON segments for dash [4 2], got {} rules={:?}",
            h.len(),
            result.rules
        );
        for r in &h {
            assert!(r.len() >= 1.0, "ON piece too short: {r:?}");
            assert!(
                r.is_horizontal(1.5),
                "expected horizontal dash piece: {r:?}"
            );
        }
        // Total ON length ≈ 4+4+4+2 = 14 (not the full 20 solid).
        let on_len: f32 = h.iter().map(|r| r.len()).sum();
        assert!(
            (on_len - 14.0).abs() < 0.5,
            "expected ~14 ON length, got {on_len} rules={:?}",
            result.rules
        );
        // No single segment covering the full solid span.
        assert!(
            !h.iter().any(|r| r.len() >= 19.0),
            "dash should split solid line: {:?}",
            result.rules
        );
    }

    #[test]
    fn dash_vertical_line_emits_on_segments() {
        // Same pattern on a vertical stroke.
        let content = b"[3 3] 0 d\n5 0 m 5 18 l S\n";
        let result = interpret_page(content, &empty_fonts(), &InterpretOptions::default());
        let v: Vec<_> = result
            .rules
            .iter()
            .filter(|r| r.is_vertical(1.5))
            .collect();
        assert!(
            v.len() >= 2,
            "expected multiple V ON segments, got {} rules={:?}",
            v.len(),
            result.rules
        );
        let on_len: f32 = v.iter().map(|r| r.len()).sum();
        // 18 long, 3 on / 3 off → ON: 0-3,6-9,12-15 → 9 total (last 15-18 is off)
        assert!(
            (on_len - 9.0).abs() < 0.5,
            "expected ~9 ON length, got {on_len} rules={:?}",
            result.rules
        );
    }

    #[test]
    fn dash_solid_stroke_still_works() {
        // No dash operator → one solid H rule of length 40.
        let content = b"0 0 m 40 0 l S\n";
        let result = interpret_page(content, &empty_fonts(), &InterpretOptions::default());
        assert_eq!(result.rules.len(), 1, "rules={:?}", result.rules);
        assert!(result.rules[0].is_horizontal(1.5));
        assert!((result.rules[0].len() - 40.0).abs() < 0.5);

        // Empty dash array is solid.
        let solid = b"[] 0 d\n0 10 m 50 10 l S\n";
        let result = interpret_page(solid, &empty_fonts(), &InterpretOptions::default());
        assert_eq!(result.rules.len(), 1, "solid empty-dash rules={:?}", result.rules);
        assert!((result.rules[0].len() - 50.0).abs() < 0.5);
    }

    #[test]
    fn clip_horizontal_rule_is_trimmed() {
        // Clip rect 10..30 x 0..20; stroke H line 0→50 at y=10 → clipped to 10..30
        let content = b"10 0 20 20 re W n\n0 10 m 50 10 l S\n";
        let result = interpret_page(content, &empty_fonts(), &InterpretOptions::default());
        assert_eq!(result.rules.len(), 1, "rules={:?}", result.rules);
        let r = &result.rules[0];
        assert!(r.is_horizontal(1.5));
        assert!((r.x0.min(r.x1) - 10.0).abs() < 0.5, "{r:?}");
        assert!((r.x0.max(r.x1) - 30.0).abs() < 0.5, "{r:?}");
    }

    #[test]
    fn clip_drops_rule_outside_box() {
        // Clip 0..20; stroke H line at y=50 (outside) → no rule
        let content = b"0 0 20 20 re W n\n0 50 m 40 50 l S\n";
        let result = interpret_page(content, &empty_fonts(), &InterpretOptions::default());
        assert!(
            result.rules.is_empty(),
            "outside clip should drop rule: {:?}",
            result.rules
        );
    }

    #[test]
    fn dash_phase_shifts_on_intervals() {
        // phase = 2 into [4 2]: start 2 into first ON → remaining ON=2, then OFF=2, ON=4, ...
        // Line 0→12: ON [0,2], [4,8], [10,12] → lengths 2,4,2
        let content = b"[4 2] 2 d\n0 0 m 12 0 l S\n";
        let result = interpret_page(content, &empty_fonts(), &InterpretOptions::default());
        let h: Vec<_> = result
            .rules
            .iter()
            .filter(|r| r.is_horizontal(1.5))
            .cloned()
            .collect();
        assert!(
            h.len() >= 2,
            "phase-shifted dash should emit ON pieces: {:?}",
            result.rules
        );
        let on_len: f32 = h.iter().map(|r| r.len()).sum();
        assert!(
            (on_len - 8.0).abs() < 0.5,
            "expected ~8 ON with phase 2, got {on_len} rules={:?}",
            result.rules
        );
    }
}
