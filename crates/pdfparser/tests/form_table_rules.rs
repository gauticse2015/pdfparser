//! PR2a: Form XObject content expands into vector RuleSegments on the table path.
//!
//! Builds a minimal PDF whose page only paints a Form XObject; the form draws
//! a ruled 3×3 grid plus cell labels. Lattice should recover a table when form
//! expansion is wired (`capture_rules` path in `page_content`).

use pdfparser::{Document, OpenOptions, TableOptions, TablePreset, TextOptions};

/// Minimal PDF: page content is `/Fm1 Do`; Form stream draws a 3×3 ruled grid.
fn form_grid_pdf() -> Vec<u8> {
    let form = b"\
0.5 w
40 40 m 220 40 l S
40 100 m 220 100 l S
40 160 m 220 160 l S
40 220 m 220 220 l S
40 40 m 40 220 l S
100 40 m 100 220 l S
160 40 m 160 220 l S
220 40 m 220 220 l S
BT /F1 9 Tf 48 190 Td (AlphaOne) Tj ET
BT /F1 9 Tf 108 190 Td (BravoTwo) Tj ET
BT /F1 9 Tf 168 190 Td (Charlie3) Tj ET
BT /F1 9 Tf 48 130 Td (DeltaFour) Tj ET
BT /F1 9 Tf 108 130 Td (EchoFive) Tj ET
BT /F1 9 Tf 168 130 Td (Foxtrot6) Tj ET
BT /F1 9 Tf 48 70 Td (GolfSeven) Tj ET
BT /F1 9 Tf 108 70 Td (HotelEight) Tj ET
BT /F1 9 Tf 168 70 Td (IndiaNine) Tj ET
";
    let page_content = b"/Fm1 Do\n";
    let form_len = form.len();
    let page_len = page_content.len();

    let mut body = String::new();
    body.push_str("%PDF-1.4\n");
    let o1 = body.len();
    body.push_str("1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n");
    let o2 = body.len();
    body.push_str("2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n");
    let o3 = body.len();
    body.push_str(
        "3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 300] \
         /Contents 4 0 R \
         /Resources << \
           /Font << /F1 6 0 R >> \
           /XObject << /Fm1 5 0 R >> \
         >> >>\nendobj\n",
    );
    let o4 = body.len();
    body.push_str(&format!("4 0 obj\n<< /Length {page_len} >>\nstream\n"));
    let mut bytes = body.into_bytes();
    bytes.extend_from_slice(page_content);
    let mut body = String::from_utf8(bytes).unwrap();
    body.push_str("endstream\nendobj\n");
    let o5 = body.len();
    body.push_str(&format!(
        "5 0 obj\n<< /Type /XObject /Subtype /Form /BBox [0 0 300 300] \
         /Matrix [1 0 0 1 0 0] \
         /Resources << /Font << /F1 6 0 R >> >> \
         /Length {form_len} >>\nstream\n"
    ));
    let mut bytes = body.into_bytes();
    bytes.extend_from_slice(form);
    let mut body = String::from_utf8(bytes).unwrap();
    body.push_str("endstream\nendobj\n");
    let o6 = body.len();
    body.push_str("6 0 obj\n<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>\nendobj\n");
    let xref_pos = body.len();
    body.push_str("xref\n0 7\n0000000000 65535 f \n");
    for off in [o1, o2, o3, o4, o5, o6] {
        body.push_str(&format!("{off:010} 00000 n \n"));
    }
    body.push_str("trailer\n<< /Size 7 /Root 1 0 R >>\n");
    body.push_str(&format!("startxref\n{xref_pos}\n%%EOF\n"));
    body.into_bytes()
}

#[test]
fn form_xobject_rules_enable_lattice_table() {
    let pdf = form_grid_pdf();
    let doc = Document::from_bytes(&pdf, OpenOptions::default()).expect("open form pdf");
    assert_eq!(doc.page_count(), 1);

    let mut table_opts = TableOptions::from_preset(TablePreset::LatticeOnly);
    table_opts.raster_line_detect = false;
    let tabs = doc
        .page(0)
        .unwrap()
        .tables(&TextOptions::default(), &table_opts)
        .expect("tables");

    assert!(
        !tabs.is_empty(),
        "expected lattice table from Form-drawn rules; got none"
    );
    let t = &tabs[0];
    assert!(
        t.rows >= 3 && t.cols >= 3,
        "expected at least 3x3, got {}x{}",
        t.rows,
        t.cols
    );
    let blob: String = t
        .cells
        .iter()
        .map(|c| c.text.as_str())
        .collect::<Vec<_>>()
        .join(" ");
    for need in ["AlphaOne", "EchoFive", "IndiaNine"] {
        assert!(blob.contains(need), "missing cell token {need} in {blob:?}");
    }
}

#[test]
fn form_pdf_is_valid_document() {
    let pdf = form_grid_pdf();
    let doc = Document::from_bytes(&pdf, OpenOptions::default()).unwrap();
    assert_eq!(doc.page_count(), 1);
}
