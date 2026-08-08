//! Content stream interpretation → TextRun IR + rule segments.
#![deny(missing_docs)]

mod lexer;
mod vm;

pub use lexer::{tokenize, Token};
pub use vm::{
    interpret_page, interpret_page_with_resolver, FormContentResolver, FormXObject, ImagePlacement,
    InterpretOptions, InterpretResult, RuleSegment, VmWarning, MAX_FORM_DEPTH,
    MAX_FORM_EXPANSIONS_PER_PAGE,
};
