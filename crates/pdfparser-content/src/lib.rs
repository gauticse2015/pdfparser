//! Content stream interpretation → TextRun IR + rule segments.
#![allow(missing_docs)]

mod lexer;
mod vm;

pub use lexer::{tokenize, Token};
pub use vm::{
    interpret_page, interpret_page_with_resolver, FormContentResolver, FormXObject, ImagePlacement,
    InterpretOptions, InterpretResult, RuleSegment, MAX_FORM_DEPTH, MAX_FORM_EXPANSIONS_PER_PAGE,
};
