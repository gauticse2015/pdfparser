//! Content stream interpretation → TextRun IR + rule segments.
#![allow(missing_docs)]

mod lexer;
mod vm;

pub use lexer::{tokenize, Token};
pub use vm::{
    interpret_page, ImagePlacement, InterpretOptions, InterpretResult, RuleSegment,
};
