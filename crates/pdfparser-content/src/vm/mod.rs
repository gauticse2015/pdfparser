//! Text + path graphics state machine.
mod path;
mod state;
mod text;

use crate::lexer::{tokenize, Token};
use pdfparser_fonts::LoadedFont;
use pdfparser_ir::{Matrix3x2, ObjectId, Rect, TextRun};
use std::collections::HashMap;
use std::fmt;

use path::{clip_rule_segment, expand_dash_segment, intersect_rect, PathBuilder};
use state::{GState, TextState};
use text::{show_text, show_text_array};

/// Max Form XObject nesting depth (PR2a / K19).
pub const MAX_FORM_DEPTH: u32 = 4;
/// Max Form expansions on a single page interpret.
pub const MAX_FORM_EXPANSIONS_PER_PAGE: u32 = 32;
/// Hard max `q`/`Q` graphics-state nesting (aligned with core `hard_max::MAX_NESTING_DEPTH`).
pub const MAX_GSTACK_DEPTH: u32 = 64;
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
    /// Max `q`/`Q` graphics-state stack depth (`ResourceLimits.max_nesting_depth`).
    /// Clamped to [`MAX_GSTACK_DEPTH`] at interpret time. Default 64.
    pub max_nesting_depth: u32,
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
            max_nesting_depth: MAX_GSTACK_DEPTH,
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
    /// Start x (user space).
    pub x0: f32,
    /// Start y (user space).
    pub y0: f32,
    /// End x (user space).
    pub x1: f32,
    /// End y (user space).
    pub y1: f32,
}

impl RuleSegment {
    /// True when the segment is near-horizontal within `tol`.
    pub fn is_horizontal(&self, tol: f32) -> bool {
        (self.y0 - self.y1).abs() <= tol
    }
    /// True when the segment is near-vertical within `tol`.
    pub fn is_vertical(&self, tol: f32) -> bool {
        (self.x0 - self.x1).abs() <= tol
    }
    /// Euclidean length in user space.
    pub fn len(&self) -> f32 {
        let dx = self.x1 - self.x0;
        let dy = self.y1 - self.y0;
        (dx * dx + dy * dy).sqrt()
    }
}

/// Soft VM diagnostic (unknown ops, budgets).
///
/// [`Display`] keeps the legacy string forms so extract can still serialize
/// messages as strings (P2.1b maps variants to `WarningCode`).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum VmWarning {
    /// Unrecognized content operator (not handled and not in the skip set).
    UnknownOperator(String),
    /// Numeric operand missing; interpreter used `0.0` (fail-soft).
    StackUnderflowNumeric,
    /// Page operator budget exceeded (`InterpretOptions::max_ops`).
    MaxPageOps,
    /// Per-form operator budget exceeded.
    PerFormMaxOps,
    /// Form XObject cycle skipped (`Do` resource name).
    FormCycle(String),
    /// Form nesting exceeded [`MAX_FORM_DEPTH`] (`Do` resource name).
    FormDepth(String),
    /// Page form expansion count exceeded [`MAX_FORM_EXPANSIONS_PER_PAGE`].
    FormExpansions(String),
    /// Clip is axis-aligned bbox only (reserved; not emitted this PR).
    ClipAabbOnly,
    /// Known ISO op intentionally ignored (reserved; skip set unchanged).
    IgnoredOperator(&'static str),
    /// Stroke width `w` ignored (reserved until A2.6).
    StrokeWidthIgnored,
    /// `q`/`Q` graphics stack hit `max_nesting_depth` (P2.2b).
    GstackNestingDepth,
}

impl fmt::Display for VmWarning {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VmWarning::UnknownOperator(op) => write!(f, "unknown_op:{op}"),
            VmWarning::StackUnderflowNumeric => write!(f, "stack_underflow:numeric"),
            VmWarning::MaxPageOps => write!(f, "max_page_ops exceeded"),
            VmWarning::PerFormMaxOps => write!(f, "per_form_max_ops exceeded"),
            VmWarning::FormCycle(name) => write!(f, "form_cycle_skipped:{name}"),
            VmWarning::FormDepth(name) => write!(f, "form_depth_exceeded:{name}"),
            VmWarning::FormExpansions(name) => write!(f, "form_expansions_exceeded:{name}"),
            VmWarning::ClipAabbOnly => write!(f, "clip_aabb_only"),
            VmWarning::IgnoredOperator(op) => write!(f, "ignored_op:{op}"),
            VmWarning::StrokeWidthIgnored => write!(f, "stroke_width_ignored"),
            VmWarning::GstackNestingDepth => write!(f, "gstack_nesting_depth exceeded"),
        }
    }
}

/// Full page interpret output.
#[derive(Debug, Clone, Default)]
pub struct InterpretResult {
    /// Extracted text runs.
    pub runs: Vec<TextRun>,
    /// Axis-aligned rule segments (when `capture_rules`).
    pub rules: Vec<RuleSegment>,
    /// Image XObject placements for raster line sensing.
    pub image_placements: Vec<ImagePlacement>,
    /// Soft warnings (unknown ops, budgets).
    pub warnings: Vec<VmWarning>,
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
        gstack_limit_warned: false,
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
    warnings: Vec<VmWarning>,
    ops: u64,
    form_expansions: u32,
    form_depth: u32,
    form_cycle: Vec<ObjectId>,
    gstack_limit_warned: bool,
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
            state.warnings.push(VmWarning::MaxPageOps);
            break;
        }
        if let Some(ref mut left) = form_ops_left {
            if *left == 0 {
                state.warnings.push(VmWarning::PerFormMaxOps);
                break;
            }
            *left -= 1;
        }
        match &tokens[i] {
            Token::Operator(op) => {
                let op = op.as_str();
                match op {
                    "q" => {
                        // A2.12 / P2.2b: cap q/Q nesting; fail-soft warn, do not panic.
                        let cap = state.opts.max_nesting_depth.min(MAX_GSTACK_DEPTH) as usize;
                        if gstack.len() >= cap {
                            if !state.gstack_limit_warned {
                                state.warnings.push(VmWarning::GstackNestingDepth);
                                state.gstack_limit_warned = true;
                            }
                        } else {
                            gstack.push(gs.clone());
                        }
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
                            let f = pop_num(&mut stack, &mut state.warnings);
                            let e = pop_num(&mut stack, &mut state.warnings);
                            let d = pop_num(&mut stack, &mut state.warnings);
                            let c = pop_num(&mut stack, &mut state.warnings);
                            let b = pop_num(&mut stack, &mut state.warnings);
                            let a = pop_num(&mut stack, &mut state.warnings);
                            let m = Matrix3x2 {
                                m: [a, b, c, d, e, f],
                            };
                            gs.ctm = m.concat(gs.ctm);
                        }
                        stack.clear();
                    }
                    "m" => {
                        let y = pop_num(&mut stack, &mut state.warnings);
                        let x = pop_num(&mut stack, &mut state.warnings);
                        path.move_to(x, y);
                        stack.clear();
                    }
                    "l" => {
                        let y = pop_num(&mut stack, &mut state.warnings);
                        let x = pop_num(&mut stack, &mut state.warnings);
                        path.line_to(x, y);
                        stack.clear();
                    }
                    "re" => {
                        let h = pop_num(&mut stack, &mut state.warnings);
                        let w = pop_num(&mut stack, &mut state.warnings);
                        let y = pop_num(&mut stack, &mut state.warnings);
                        let x = pop_num(&mut stack, &mut state.warnings);
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
                        if op == "s" || op == "b" || op == "b*" {
                            path.close();
                        }
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
                                    expand_dash_segment(seg, &gs.dash, gs.dash_phase)
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
                        let phase = pop_num(&mut stack, &mut state.warnings);
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
                            for seg in
                                path.thin_fill_rules(&gs.ctm, state.opts.thin_fill_rule_max, 2.0)
                            {
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
                        let size = pop_num(&mut stack, &mut state.warnings);
                        let name = pop_name(&mut stack);
                        gs.text.font_size = size;
                        gs.text.font = name;
                        stack.clear();
                    }
                    "Tc" => {
                        gs.text.char_spacing = pop_num(&mut stack, &mut state.warnings);
                        stack.clear();
                    }
                    "Tw" => {
                        gs.text.word_spacing = pop_num(&mut stack, &mut state.warnings);
                        stack.clear();
                    }
                    "Tz" => {
                        gs.text.horizontal_scale = pop_num(&mut stack, &mut state.warnings);
                        stack.clear();
                    }
                    "TL" => {
                        gs.text.leading = pop_num(&mut stack, &mut state.warnings);
                        stack.clear();
                    }
                    "Ts" => {
                        gs.text.rise = pop_num(&mut stack, &mut state.warnings);
                        stack.clear();
                    }
                    "Tr" => {
                        gs.text.render_mode = pop_num(&mut stack, &mut state.warnings) as i32;
                        stack.clear();
                    }
                    "Td" => {
                        let ty = pop_num(&mut stack, &mut state.warnings);
                        let tx = pop_num(&mut stack, &mut state.warnings);
                        let m = Matrix3x2 {
                            m: [1.0, 0.0, 0.0, 1.0, tx, ty],
                        };
                        gs.text.tlm = m.concat(gs.text.tlm);
                        gs.text.tm = gs.text.tlm;
                        stack.clear();
                    }
                    "TD" => {
                        let ty = pop_num(&mut stack, &mut state.warnings);
                        let tx = pop_num(&mut stack, &mut state.warnings);
                        gs.text.leading = -ty;
                        let m = Matrix3x2 {
                            m: [1.0, 0.0, 0.0, 1.0, tx, ty],
                        };
                        gs.text.tlm = m.concat(gs.text.tlm);
                        gs.text.tm = gs.text.tlm;
                        stack.clear();
                    }
                    "Tm" => {
                        let f = pop_num(&mut stack, &mut state.warnings);
                        let e = pop_num(&mut stack, &mut state.warnings);
                        let d = pop_num(&mut stack, &mut state.warnings);
                        let c = pop_num(&mut stack, &mut state.warnings);
                        let b = pop_num(&mut stack, &mut state.warnings);
                        let a = pop_num(&mut stack, &mut state.warnings);
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
                        if op == "\"" {
                            // PDF: aw ac string "  — set Tw/Tc then next-line show.
                            let _string = pop_string(&mut stack);
                            gs.text.char_spacing = pop_num(&mut stack, &mut state.warnings);
                            gs.text.word_spacing = pop_num(&mut stack, &mut state.warnings);
                            // Re-push string for the shared show path below.
                            // pop_string already consumed it; show from saved bytes.
                            if let Some(bytes) = _string {
                                let m = Matrix3x2 {
                                    m: [1.0, 0.0, 0.0, 1.0, 0.0, -gs.text.leading],
                                };
                                gs.text.tlm = m.concat(gs.text.tlm);
                                gs.text.tm = gs.text.tlm;
                                if let Some(run) = show_text(gs, state.fonts, &bytes, in_text) {
                                    state.runs.push(run);
                                }
                            }
                            stack.clear();
                        } else {
                            if op == "'" {
                                let m = Matrix3x2 {
                                    m: [1.0, 0.0, 0.0, 1.0, 0.0, -gs.text.leading],
                                };
                                gs.text.tlm = m.concat(gs.text.tlm);
                                gs.text.tm = gs.text.tlm;
                            }
                            if let Some(bytes) = pop_string(&mut stack) {
                                if let Some(run) = show_text(gs, state.fonts, &bytes, in_text) {
                                    state.runs.push(run);
                                }
                            }
                            stack.clear();
                        }
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
                        if let Some(run) = show_text_array(gs, state.fonts, &items, in_text) {
                            state.runs.push(run);
                        }
                        stack.clear();
                    }
                    "c" | "v" | "y" => {
                        // Curves are not lattice rules. Consume operands only —
                        // do not wipe earlier line/`re` segments in the same path.
                        stack.clear();
                    }
                    "Do" => {
                        // Paint XObject: Form expansion (if resolver) else image placement.
                        let name = pop_name(&mut stack);
                        stack.clear();
                        if let Some(name) = name {
                            let mut expanded = false;
                            if resolver.is_some() {
                                expanded = try_expand_form(state, gs, resolver, &name);
                            }
                            if !expanded && state.opts.capture_image_placements {
                                state
                                    .image_placements
                                    .push(ImagePlacement { name, ctm: gs.ctm });
                            }
                        }
                    }
                    "CS" | "cs" | "SC" | "SCN" | "sc" | "scn" | "G" | "g" | "RG" | "rg" | "K"
                    | "k" | "sh" | "gs" | "MP" | "DP" | "BMC" | "BDC" | "EMC" | "BX" | "EX"
                    | "ri" | "i" | "J" | "j" | "M" | "w" | "d0" | "d1" => {
                        // Skip set unchanged (P2.1a): still silent, no IgnoredOperator.
                        stack.clear();
                    }
                    _ => {
                        state
                            .warnings
                            .push(VmWarning::UnknownOperator(op.to_string()));
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
    if state.form_cycle.contains(&form.id) {
        state.warnings.push(VmWarning::FormCycle(name.to_string()));
        return true;
    }
    if state.form_depth >= MAX_FORM_DEPTH {
        state.warnings.push(VmWarning::FormDepth(name.to_string()));
        return true;
    }
    if state.form_expansions >= MAX_FORM_EXPANSIONS_PER_PAGE {
        state
            .warnings
            .push(VmWarning::FormExpansions(name.to_string()));
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

fn pop_num(stack: &mut Vec<Token>, warnings: &mut Vec<VmWarning>) -> f32 {
    loop {
        match stack.pop() {
            Some(Token::Number(n)) => return n,
            Some(Token::ArrayEnd) | Some(Token::ArrayStart) => continue,
            Some(_) => continue,
            None => {
                warnings.push(VmWarning::StackUnderflowNumeric);
                return 0.0;
            }
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

/// Back-compat alias used by older call sites.
#[allow(dead_code)]
pub fn interpret_text(
    content: &[u8],
    fonts: &HashMap<String, LoadedFont>,
    opts: &InterpretOptions,
) -> (Vec<TextRun>, Vec<String>) {
    let r = interpret_page(content, fonts, opts);
    (
        r.runs,
        r.warnings.into_iter().map(|w| w.to_string()).collect(),
    )
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
        let result = interpret_page_with_resolver(page, &fonts, &opts, Some(&mut resolver));

        assert!(
            result.rules.len() >= 2,
            "expected rules from form, got {} warnings={:?}",
            result.rules.len(),
            result.warnings
        );
        let has_h = result
            .rules
            .iter()
            .any(|r| r.is_horizontal(1.5) && r.len() >= 50.0);
        let has_v = result
            .rules
            .iter()
            .any(|r| r.is_vertical(1.5) && r.len() >= 50.0);
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
        assert!(
            (r.x0 - 30.0).abs() < 1.0 && (r.x1 - 150.0).abs() < 1.0,
            "{r:?}"
        );
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
            result
                .warnings
                .iter()
                .any(|w| matches!(w, VmWarning::FormCycle(_))),
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
        let result = interpret_page(b"/Im0 Do", &empty_fonts(), &InterpretOptions::default());
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
        let v: Vec<_> = result.rules.iter().filter(|r| r.is_vertical(1.5)).collect();
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
        assert_eq!(
            result.rules.len(),
            1,
            "solid empty-dash rules={:?}",
            result.rules
        );
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

    #[test]
    fn vm_warning_display_preserves_legacy_strings() {
        assert_eq!(
            VmWarning::UnknownOperator("foo".into()).to_string(),
            "unknown_op:foo"
        );
        assert_eq!(
            VmWarning::StackUnderflowNumeric.to_string(),
            "stack_underflow:numeric"
        );
        assert_eq!(VmWarning::MaxPageOps.to_string(), "max_page_ops exceeded");
        assert_eq!(
            VmWarning::GstackNestingDepth.to_string(),
            "gstack_nesting_depth exceeded"
        );
    }

    #[test]
    fn unknown_op_is_typed_warning() {
        let result = interpret_page(b"1 2 foo", &empty_fonts(), &InterpretOptions::default());
        assert_eq!(
            result.warnings,
            vec![VmWarning::UnknownOperator("foo".into())]
        );
    }

    #[test]
    fn ignored_ops_stay_silent() {
        let content = b"1 w 0 G 0 g 1 0 0 RG 1 0 0 rg 0 0 0 1 K 0 0 0 1 k \
            /CS CS /cs cs 1 SC 1 SCN 1 sc 1 scn /Sh sh /GS gs \
            /MP MP /DP DP /BMC BMC /BDC BDC EMC BX EX /ri ri 1 i 0 J 0 j 10 M d0 d1\n";
        let result = interpret_page(content, &empty_fonts(), &InterpretOptions::default());
        assert!(
            result.warnings.is_empty(),
            "skip set must stay silent: {:?}",
            result.warnings
        );
    }

    #[test]
    fn pop_num_underflow_is_zero_and_typed() {
        let result = interpret_page(b"re", &empty_fonts(), &InterpretOptions::default());
        assert_eq!(
            result.warnings,
            vec![
                VmWarning::StackUnderflowNumeric,
                VmWarning::StackUnderflowNumeric,
                VmWarning::StackUnderflowNumeric,
                VmWarning::StackUnderflowNumeric,
            ]
        );
        assert!(result.rules.is_empty());
        assert!(result.runs.is_empty());
    }

    #[test]
    fn max_page_ops_is_typed_warning() {
        let opts = InterpretOptions {
            max_ops: 0,
            ..InterpretOptions::default()
        };
        let result = interpret_page(b"0 0 m 40 0 l S", &empty_fonts(), &opts);
        assert_eq!(result.warnings, vec![VmWarning::MaxPageOps]);
    }

    fn has_gstack_warn(ws: &[VmWarning]) -> bool {
        ws.iter()
            .any(|w| w.to_string().contains("gstack_nesting_depth"))
    }

    #[test]
    fn q_stack_caps_at_max_nesting_depth() {
        let mut content = Vec::new();
        for _ in 0..8 {
            content.extend_from_slice(b"q\n");
        }
        content.extend_from_slice(b"0 0 m 40 0 l S\n");
        for _ in 0..8 {
            content.extend_from_slice(b"Q\n");
        }
        let opts = InterpretOptions {
            max_nesting_depth: 3,
            ..InterpretOptions::default()
        };
        let result = interpret_page(&content, &empty_fonts(), &opts);
        assert_eq!(
            result
                .warnings
                .iter()
                .filter(|w| w.to_string().contains("gstack_nesting_depth"))
                .count(),
            1
        );
        assert_eq!(result.rules.len(), 1);
        assert!((result.rules[0].len() - 40.0).abs() < 0.5);
    }

    #[test]
    fn q_stack_exact_cap_does_not_warn() {
        let content = b"q\nq\n0 0 m 40 0 l S\nQ\nQ\n";
        let opts = InterpretOptions {
            max_nesting_depth: 2,
            ..InterpretOptions::default()
        };
        let result = interpret_page(content, &empty_fonts(), &opts);
        assert!(
            !has_gstack_warn(&result.warnings),
            "warnings={:?}",
            result.warnings
        );
        assert_eq!(result.rules.len(), 1);
    }

    #[test]
    fn q_restore_underflow_is_soft() {
        let result = interpret_page(
            b"Q Q Q 0 0 m 40 0 l S\n",
            &empty_fonts(),
            &InterpretOptions::default(),
        );
        assert_eq!(result.rules.len(), 1);
        assert!(
            !has_gstack_warn(&result.warnings),
            "warnings={:?}",
            result.warnings
        );
    }

    #[test]
    fn q_stack_default_cap_matches_hard_max() {
        let mut content = Vec::new();
        for _ in 0..MAX_GSTACK_DEPTH {
            content.extend_from_slice(b"q\n");
        }
        content.extend_from_slice(b"0 0 m 10 0 l S\n");
        for _ in 0..MAX_GSTACK_DEPTH {
            content.extend_from_slice(b"Q\n");
        }
        let ok = interpret_page(&content, &empty_fonts(), &InterpretOptions::default());
        assert!(!has_gstack_warn(&ok.warnings), "{:?}", ok.warnings);

        content.clear();
        for _ in 0..(MAX_GSTACK_DEPTH + 1) {
            content.extend_from_slice(b"q\n");
        }
        content.extend_from_slice(b"0 0 m 10 0 l S\n");
        let over = interpret_page(&content, &empty_fonts(), &InterpretOptions::default());
        assert!(has_gstack_warn(&over.warnings), "{:?}", over.warnings);
    }

    #[test]
    fn q_stack_clamps_huge_option_to_hard_max() {
        let mut content = Vec::new();
        for _ in 0..(MAX_GSTACK_DEPTH + 2) {
            content.extend_from_slice(b"q\n");
        }
        content.extend_from_slice(b"0 0 m 8 0 l S\n");
        let opts = InterpretOptions {
            max_nesting_depth: u32::MAX,
            ..InterpretOptions::default()
        };
        let result = interpret_page(&content, &empty_fonts(), &opts);
        assert!(has_gstack_warn(&result.warnings), "{:?}", result.warnings);
        assert_eq!(result.rules.len(), 1);
    }
}
