//! Minimal text extract example.
//!
//! ```bash
//! cargo run --example extract_text -- path/to/file.pdf
//! ```

use pdfparser::{Document, TextOptions};
use std::env;

fn main() -> pdfparser::Result<()> {
    let path = env::args().nth(1).expect("usage: extract_text <file.pdf>");
    let doc = Document::open(&path)?;
    let opts = TextOptions::default();
    for i in 0..doc.page_count() {
        let text = doc.page(i)?.text(&opts)?;
        println!("----- page {i} -----\n{text}\n");
    }
    Ok(())
}
