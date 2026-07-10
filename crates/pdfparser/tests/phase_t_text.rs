//! Phase T acceptance gates (corpus fixtures).
use pdfparser::{Document, TextOptions};
use std::path::PathBuf;

fn corpus(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../benchmark/corpus")
        .join(name)
}

#[test]
fn simple_tokens() {
    let doc = Document::open(corpus("01_simple_text.pdf")).unwrap();
    let t = doc.page(0).unwrap().text(&TextOptions::default()).unwrap();
    for tok in [
        "SIMPLE_TOKEN_ALPHA",
        "SIMPLE_TOKEN_BETA",
        "Simple Digital PDF",
        "12345",
    ] {
        assert!(t.contains(tok), "missing {tok} in {t}");
    }
}

#[test]
fn multi_column_reading_order() {
    let doc = Document::open(corpus("02_multi_column.pdf")).unwrap();
    let t = doc.page(0).unwrap().text(&TextOptions::default()).unwrap();
    let keys = [
        "LEFT_COL_START",
        "LEFT_COL_END",
        "RIGHT_COL_START",
        "RIGHT_COL_END",
    ];
    let pos: Vec<_> = keys.iter().map(|k| t.find(k)).collect();
    assert!(pos.iter().all(|p| p.is_some()), "missing markers in {t}");
    let pos: Vec<usize> = pos.into_iter().map(|p| p.unwrap()).collect();
    assert!(pos.windows(2).all(|w| w[0] < w[1]), "order fail: {t}");
}

#[test]
fn rotated_page_tokens() {
    let doc = Document::open(corpus("11_rotated_page.pdf")).unwrap();
    let mut all = String::new();
    for i in 0..doc.page_count() {
        all.push_str(&doc.page(i).unwrap().text(&TextOptions::default()).unwrap());
        all.push('\n');
    }
    assert!(all.contains("NORMAL_PAGE_TOKEN"), "{all}");
    assert!(all.contains("ROTATED_PAGE_TOKEN"), "{all}");
}

#[test]
fn encryption_rejected() {
    let r = Document::open(corpus("12_encrypted_password.pdf"));
    assert!(r.is_err());
    let e = r.err().unwrap().to_string();
    assert!(e.to_lowercase().contains("encrypt"), "{e}");
}

#[test]
fn large_multipage_tokens() {
    let doc = Document::open(corpus("03_large_multipage.pdf")).unwrap();
    assert_eq!(doc.page_count(), 80);
    let t0 = doc.page(0).unwrap().text(&TextOptions::default()).unwrap();
    let t79 = doc.page(79).unwrap().text(&TextOptions::default()).unwrap();
    assert!(t0.contains("PAGE_TOKEN_0001"), "{t0}");
    assert!(t79.contains("PAGE_TOKEN_0080"), "{t79}");
}
