//! End-to-end: image-painted ruled grids via raster morphology → lattice.
use pdfparser::{Document, TableOptions, TablePreset, TextOptions};
use std::path::PathBuf;

fn path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../benchmark/corpus/compete_synthetic")
        .join(name)
}

fn auto_tables(pdf: &str) -> Vec<pdfparser::Table> {
    let doc = Document::open(path(pdf)).unwrap();
    let opts = TableOptions::from_preset(TablePreset::Auto);
    doc.page(0)
        .unwrap()
        .tables(&TextOptions::default(), &opts)
        .unwrap()
}

#[test]
fn c100_image_rules_detects_table() {
    let tabs = auto_tables("C100_img_rules_8x4.pdf");
    assert!(!tabs.is_empty(), "expected table from image grid");
    assert!(
        tabs[0].rows >= 6 && tabs[0].cols >= 3,
        "shape {:?}",
        (tabs[0].rows, tabs[0].cols)
    );
    assert!(
        tabs[0].notes.iter().any(|n| n.contains("raster_lines"))
            || format!("{:?}", tabs[0].strategy_provenance).contains("Raster"),
        "expected raster provenance notes={:?}",
        tabs[0].notes
    );
}

#[test]
fn c101_image_rules_detects_table() {
    let tabs = auto_tables("C101_img_rules_10x5.pdf");
    assert!(!tabs.is_empty(), "C101 expected table");
    assert!(
        tabs[0].rows >= 8 && tabs[0].cols >= 4,
        "C101 shape {:?}",
        (tabs[0].rows, tabs[0].cols)
    );
}

#[test]
fn c102_image_rules_detects_table() {
    let tabs = auto_tables("C102_img_rules_12x6.pdf");
    assert!(!tabs.is_empty(), "C102 expected table");
    assert!(
        tabs[0].rows >= 10 && tabs[0].cols >= 5,
        "C102 shape {:?}",
        (tabs[0].rows, tabs[0].cols)
    );
}

#[test]
fn raster_off_misses_image_only_grid() {
    let doc = Document::open(path("C100_img_rules_8x4.pdf")).unwrap();
    let mut opts = TableOptions::from_preset(TablePreset::Auto);
    opts.raster_line_detect = false;
    let tabs = doc
        .page(0)
        .unwrap()
        .tables(&TextOptions::default(), &opts)
        .unwrap();
    // Without raster, image-painted grids have no vector rules → no lattice.
    // (May still get network/stream FPs; lattice count should be 0.)
    let lattice = tabs
        .iter()
        .filter(|t| matches!(t.method, pdfparser::TableMethod::Lattice))
        .count();
    assert_eq!(
        lattice, 0,
        "raster off should not form lattice from image rules"
    );
}

#[test]
fn c103_to_c107_image_rules_smoke() {
    for (pdf, min_r, min_c) in [
        ("C103_img_rules_15x6.pdf", 12u32, 4u32),
        ("C104_img_rules_21x6.pdf", 16, 4),
        ("C105_img_rules_12x8.pdf", 10, 6),
        ("C106_img_rules_18x5.pdf", 14, 3),
        ("C107_img_rules_9x7.pdf", 7, 5),
    ] {
        let tabs = auto_tables(pdf);
        assert!(!tabs.is_empty(), "{pdf}: expected table");
        assert!(
            tabs[0].rows >= min_r && tabs[0].cols >= min_c,
            "{pdf}: shape {:?} want >={}x{}",
            (tabs[0].rows, tabs[0].cols),
            min_r,
            min_c
        );
    }
}
