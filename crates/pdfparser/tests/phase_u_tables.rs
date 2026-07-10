//! Phase U gates: lattice tables on synthetic ruled fixtures.
use pdfparser::{Document, TableOptions, TablePreset, TextOptions};
use std::path::PathBuf;

fn corpus(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../benchmark/corpus")
        .join(name)
}

fn text_opts() -> TextOptions {
    TextOptions::default()
}

fn table_opts() -> TableOptions {
    TableOptions::from_preset(TablePreset::LatticeOnly)
}

#[test]
fn lattice_sku_table() {
    let doc = Document::open(corpus("06_table_lattice.pdf")).unwrap();
    let tabs = doc
        .page(0)
        .unwrap()
        .tables(&text_opts(), &table_opts())
        .unwrap();
    assert_eq!(tabs.len(), 1, "expected 1 table, got {}", tabs.len());
    let t = &tabs[0];
    assert_eq!((t.rows, t.cols), (5, 4), "shape {:?}", (t.rows, t.cols));
    assert!(t.confidence >= 0.75, "conf {}", t.confidence);
    let texts: Vec<_> = t.cells.iter().map(|c| c.text.as_str()).collect();
    for need in ["SKU", "Widget", "A-100", "15.00", "Thingamajig"] {
        assert!(
            texts.iter().any(|x| x.contains(need)),
            "missing {need} in {texts:?}"
        );
    }
}

#[test]
fn complex_financial_table() {
    let doc = Document::open(corpus("09_table_complex.pdf")).unwrap();
    let tabs = doc
        .page(0)
        .unwrap()
        .tables(&text_opts(), &table_opts())
        .unwrap();
    assert!(!tabs.is_empty());
    let t = &tabs[0];
    assert_eq!(t.rows, 7);
    assert_eq!(t.cols, 5);
    let blob: String = t
        .cells
        .iter()
        .map(|c| c.text.as_str())
        .collect::<Vec<_>>()
        .join(" ");
    assert!(blob.contains("EBITDA"));
    assert!(blob.contains("COMPLEX_TABLE_TOKEN"));
    assert!(blob.contains("1,450,000"));
}

#[test]
fn mixed_document_table() {
    let doc = Document::open(corpus("10_mixed_document.pdf")).unwrap();
    let tabs = doc
        .page(0)
        .unwrap()
        .tables(&text_opts(), &table_opts())
        .unwrap();
    assert_eq!(tabs.len(), 1);
    let t = &tabs[0];
    assert_eq!((t.rows, t.cols), (4, 2));
    let blob: String = t
        .cells
        .iter()
        .map(|c| c.text.clone())
        .collect::<Vec<_>>()
        .join("|");
    assert!(blob.contains("Apples"));
    assert!(blob.contains("Bananas"));
}

#[test]
fn tables_off_by_default() {
    let doc = Document::open(corpus("06_table_lattice.pdf")).unwrap();
    let tabs = doc
        .page(0)
        .unwrap()
        .tables(&text_opts(), &TableOptions::default())
        .unwrap();
    assert!(tabs.is_empty());
}
