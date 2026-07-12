//! Phase 15: census multi-table recovery (324 + 325 regions).
use pdfparser::{Document, TableOptions, TablePreset, TextOptions};
use std::path::PathBuf;

fn p32() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../benchmark/corpus/real/32_real_census_table324.pdf")
}

#[test]
fn census_32_finds_two_vertically_disjoint_tables() {
    let doc = Document::open(p32()).unwrap();
    let opts = TableOptions::from_preset(TablePreset::Auto);
    let tabs = doc
        .page(0)
        .unwrap()
        .tables(&TextOptions::default(), &opts)
        .unwrap();
    assert!(
        tabs.len() >= 2,
        "expected upper stream + lower lattice regions, got {} {:?}",
        tabs.len(),
        tabs.iter()
            .map(|t| (format!("{:?}", t.method), t.rows, t.cols, t.bbox.y0))
            .collect::<Vec<_>>()
    );
    let mut ys: Vec<f32> = tabs.iter().map(|t| (t.bbox.y0 + t.bbox.y1) * 0.5).collect();
    ys.sort_by(|a, b| a.partial_cmp(b).unwrap());
    assert!(
        ys.last().unwrap() - ys.first().unwrap() > 50.0,
        "tables should be vertically separated: {:?}",
        ys
    );
}
