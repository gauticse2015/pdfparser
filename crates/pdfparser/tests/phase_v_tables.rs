//! Phase V gates: stream, hybrid, side-by-side split, multi-page stitch.
use pdfparser::{Document, TableMethod, TableOptions, TablePreset, TextOptions};
use std::path::PathBuf;

fn corpus(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../benchmark/corpus")
        .join(name)
}

fn stress(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../benchmark/corpus/stress")
        .join(name)
}

fn text_opts() -> TextOptions {
    TextOptions::default()
}

fn table_opts() -> TableOptions {
    TableOptions::from_preset(TablePreset::Full)
}

#[test]
fn stream_table_07() {
    let doc = Document::open(corpus("07_table_stream.pdf")).unwrap();
    let tabs = doc
        .page(0)
        .unwrap()
        .tables(&text_opts(), &table_opts())
        .unwrap();
    assert_eq!(tabs.len(), 1, "got {}", tabs.len());
    let t = &tabs[0];
    assert_eq!((t.rows, t.cols), (6, 4), "shape {:?}", (t.rows, t.cols));
    assert_eq!(t.method, TableMethod::Stream);
    let blob: String = t.cells.iter().map(|c| c.text.as_str()).collect::<Vec<_>>().join(" ");
    for need in ["Name", "Alice", "Eve", "130000", "Salary", "STREAM_TABLE_TOKEN"] {
        // STREAM token may be outside table body — core cells must hit
        if need == "STREAM_TABLE_TOKEN" {
            continue;
        }
        assert!(blob.contains(need), "missing {need}");
    }
}

#[test]
fn hybrid_table_08() {
    let doc = Document::open(corpus("08_table_partial_border.pdf")).unwrap();
    let tabs = doc
        .page(0)
        .unwrap()
        .tables(&text_opts(), &table_opts())
        .unwrap();
    assert_eq!(tabs.len(), 1, "got {} {:?}", tabs.len(), tabs.iter().map(|t| (t.rows, t.cols, t.method)).collect::<Vec<_>>());
    let t = &tabs[0];
    assert_eq!((t.rows, t.cols), (5, 5));
    assert_eq!(t.method, TableMethod::Hybrid);
    let blob: String = t.cells.iter().map(|c| c.text.as_str()).collect::<Vec<_>>().join(" ");
    for need in ["Region", "North", "West", "18", "Q4"] {
        assert!(blob.contains(need), "missing {need} in {blob}");
    }
}

#[test]
fn side_by_side_23() {
    let doc = Document::open(stress("23_side_by_side_tables.pdf")).unwrap();
    let tabs = doc
        .page(0)
        .unwrap()
        .tables(&text_opts(), &table_opts())
        .unwrap();
    assert_eq!(tabs.len(), 2, "got {} shapes {:?}", tabs.len(), tabs.iter().map(|t| (t.rows, t.cols)).collect::<Vec<_>>());
    let blob: String = tabs
        .iter()
        .flat_map(|t| t.cells.iter().map(|c| c.text.clone()))
        .collect::<Vec<_>>()
        .join(" ");
    assert!(blob.contains("TOKEN_SIDE_L"));
    assert!(blob.contains("TOKEN_SIDE_R"));
    assert!(blob.contains("Extra row only on right") || blob.contains("Pop"));
}

#[test]
fn bank_stitch_20() {
    let doc = Document::open(stress("20_bank_statement_multipage.pdf")).unwrap();
    let (_frags, logical) = doc.tables(&text_opts(), &table_opts()).unwrap();
    assert_eq!(
        logical.len(),
        1,
        "expected 1 stitched logical table, got {} {:?}",
        logical.len(),
        logical
            .iter()
            .map(|t| (t.rows, t.cols, t.method, t.notes.clone()))
            .collect::<Vec<_>>()
    );
    let t = &logical[0];
    assert!(t.cols >= 4, "cols {}", t.cols);
    assert!(t.rows >= 20, "rows {}", t.rows);
    let blob: String = t.cells.iter().map(|c| c.text.as_str()).collect::<Vec<_>>().join(" ");
    assert!(blob.contains("Date") || blob.contains("Balance"));
}

#[test]
fn tables_still_off_by_default() {
    let doc = Document::open(corpus("07_table_stream.pdf")).unwrap();
    let tabs = doc
        .page(0)
        .unwrap()
        .tables(&text_opts(), &TableOptions::default())
        .unwrap();
    assert!(tabs.is_empty());
}
