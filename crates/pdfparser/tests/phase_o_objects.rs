//! Phase O: images, links, forms, outline extraction.
use pdfparser::Document;
use std::path::PathBuf;

fn corpus(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../benchmark/corpus")
        .join(name)
}

#[test]
fn image_heavy_count() {
    let doc = Document::open(corpus("04_image_heavy.pdf")).unwrap();
    let objs = doc.objects().unwrap();
    assert_eq!(objs.image_count(), 4, "images={:?}", objs.images);
}

#[test]
fn special_objects_links_forms_outline() {
    let doc = Document::open(corpus("05_special_objects.pdf")).unwrap();
    let objs = doc.objects().unwrap();

    let uris = objs.link_uris();
    assert!(
        uris.iter().any(|u| u.contains("example.com/pdfparser-bench")),
        "links={uris:?}"
    );

    let names: Vec<_> = objs.form_fields.iter().map(|f| f.name.as_str()).collect();
    assert!(
        names.iter().any(|n| n.contains("customer_name")),
        "forms={names:?}"
    );
    assert!(
        names.iter().any(|n| n.contains("agree_terms")),
        "forms={names:?}"
    );

    assert!(
        objs.outline_titles
            .iter()
            .any(|t| t.contains("Section 1")),
        "outline={:?}",
        objs.outline_titles
    );
    assert!(
        objs.outline_titles
            .iter()
            .any(|t| t.contains("Section 2")),
        "outline={:?}",
        objs.outline_titles
    );
}

#[test]
fn mixed_document_has_image() {
    let doc = Document::open(corpus("10_mixed_document.pdf")).unwrap();
    let objs = doc.objects().unwrap();
    assert!(
        objs.image_count() >= 1,
        "expected >=1 image, got {:?}",
        objs.images
    );
}
