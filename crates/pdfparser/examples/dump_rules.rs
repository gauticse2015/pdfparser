//! Dump table candidates for debugging.
use pdfparser::{Document, TableOptions, TablePreset, TextOptions};
use std::env;

fn main() {
    let path = env::args().nth(1).expect("pdf path");
    let doc = Document::open(&path).expect("open");
    let text_opts = TextOptions::default();
    let table_opts = TableOptions::from_preset(TablePreset::Full);
    for i in 0..doc.page_count() {
        let page = doc.page(i).unwrap();
        let tabs = page.tables(&text_opts, &table_opts).unwrap();
        println!("=== page {i} tables={} ===", tabs.len());
        for (j, t) in tabs.iter().enumerate() {
            println!(
                "  [{j}] {}x{} method={:?} conf={:.3} notes={:?}",
                t.rows, t.cols, t.method, t.confidence, t.notes
            );
        }
    }
}
