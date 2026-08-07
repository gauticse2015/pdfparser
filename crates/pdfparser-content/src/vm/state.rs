//! Graphics / text state for the content VM.
use pdfparser_ir::Matrix3x2;
use pdfparser_ir::Rect;

#[derive(Clone)]
pub(crate) struct TextState {
    pub(crate) font: Option<String>,
    pub(crate) font_size: f32,
    pub(crate) char_spacing: f32,
    pub(crate) word_spacing: f32,
    pub(crate) horizontal_scale: f32,
    pub(crate) leading: f32,
    pub(crate) rise: f32,
    pub(crate) render_mode: i32,
    pub(crate) tm: Matrix3x2,
    pub(crate) tlm: Matrix3x2,
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
pub(crate) struct GState {
    pub(crate) ctm: Matrix3x2,
    pub(crate) text: TextState,
    /// Dash array from `d` (empty = solid stroke). Alternating on/off lengths.
    pub(crate) dash: Vec<f32>,
    /// Dash phase from `d` (distance into pattern at stroke start).
    pub(crate) dash_phase: f32,
    /// Axis-aligned clip rectangle in user space after CTM (PR2c subset).
    /// `None` = no clip. Intersected on successive `W`/`W*`.
    pub(crate) clip_rect: Option<Rect>,
}
