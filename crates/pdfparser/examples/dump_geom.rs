//! Dump text run and rule geometry for debugging table detectors.
use pdfparser::{Document, TextOptions};
use std::env;

fn main() {
    let path = env::args().nth(1).expect("pdf path");
    let doc = Document::open(&path).expect("open");
    let opts = TextOptions::default();
    for i in 0..doc.page_count() {
        let page = doc.page(i).unwrap();
        let runs = page.text_runs(&opts).unwrap();
        println!("=== page {} runs={} ===", i, runs.len());
        for (j, r) in runs.iter().enumerate() {
            println!(
                "  [{j:3}] x0={:7.1} x1={:7.1} y0={:7.1} y1={:7.1} fs={:5.1} {:?}",
                r.bbox.x0, r.bbox.x1, r.bbox.y0, r.bbox.y1, r.font_size, r.text
            );
        }
    }
}
