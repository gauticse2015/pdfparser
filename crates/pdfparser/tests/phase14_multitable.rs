
//! Phase 14 multi-table recovery.
use pdfparser::{Document, TableOptions, TablePreset, TextOptions};
use std::path::PathBuf;

fn corpus(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../benchmark/corpus/real")
        .join(name)
}

#[test]
fn disease_two_tables_36_finds_two() {
    let doc = Document::open(corpus("36_real_two_tables.pdf")).unwrap();
    let opts = TableOptions::from_preset(TablePreset::Auto);
    let tabs = doc.page(0).unwrap().tables(&TextOptions::default(), &opts).unwrap();
    assert!(
        tabs.len() >= 2,
        "expected ≥2 tables on 36, got {} {:?}",
        tabs.len(),
        tabs.iter().map(|t| (t.rows, t.cols)).collect::<Vec<_>>()
    );
    assert!(
        tabs.iter().all(|t| t.cols >= 3),
        "tables should be multi-col: {:?}",
        tabs.iter().map(|t| t.cols).collect::<Vec<_>>()
    );
}

#[test]
fn census_32_finds_at_least_one() {
    let doc = Document::open(corpus("32_real_census_table324.pdf")).unwrap();
    let opts = TableOptions::from_preset(TablePreset::Auto);
    let tabs = doc.page(0).unwrap().tables(&TextOptions::default(), &opts).unwrap();
    assert!(!tabs.is_empty());
}
