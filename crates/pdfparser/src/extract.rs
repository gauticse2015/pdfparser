//! Page extract orchestration.
use crate::font_load::load_page_fonts;
use crate::options::{ExtractOptions, TextOptions};
use pdfparser_content::{interpret_text, InterpretOptions};
use pdfparser_core::{Error, PdfDocument, Result};
use pdfparser_ir::{
    DocumentMetadata, Element, ExtractWarning, ExtractedDocument, ExtractedPage, TextRun,
    WarningCode, SCHEMA_VERSION,
};
use pdfparser_layout::{apply_page_rotate_to_runs, insert_spaces, reading_order_text};

pub fn page_elements(
    doc: &PdfDocument,
    page_index: usize,
    opts: &TextOptions,
) -> Result<(Vec<TextRun>, Vec<ExtractWarning>)> {
    let page = doc.pages.get(page_index).ok_or(Error::PageOutOfRange {
        index: page_index as u32,
    })?;
    let content = doc.page_content_bytes(page_index)?;
    let font_refs = doc.page_font_map(page_index)?;
    let fonts = doc.with_doc(|d| load_page_fonts(d, &font_refs))?;

    let iopts = InterpretOptions {
        max_ops: doc.governor.limits.max_page_ops,
    };
    let (mut runs, warn_msgs) = interpret_text(&content, &fonts, &iopts);
    let mut warnings: Vec<ExtractWarning> = warn_msgs
        .into_iter()
        .map(|message| ExtractWarning {
            code: WarningCode::UnknownOperator,
            page: Some(page_index as u32),
            message,
            recoverable: true,
        })
        .collect();

    if !opts.include_invisible {
        runs.retain(|r| !r.invisible);
    }

    if opts.apply_page_rotate {
        apply_page_rotate_to_runs(&mut runs, page.rotate, page.media_box);
    }

    if opts.insert_spaces {
        runs = insert_spaces(&runs);
    }

    if runs.is_empty() && !content.is_empty() {
        warnings.push(ExtractWarning {
            code: WarningCode::Other,
            page: Some(page_index as u32),
            message: "no text runs extracted".into(),
            recoverable: true,
        });
    }

    Ok((runs, warnings))
}

pub fn page_text(doc: &PdfDocument, page_index: usize, opts: &TextOptions) -> Result<String> {
    let (runs, _) = page_elements(doc, page_index, opts)?;
    if opts.sort_reading_order {
        Ok(reading_order_text(&runs))
    } else {
        Ok(runs.into_iter().map(|r| r.text).collect())
    }
}

/// Extract whole document.
pub fn extract_document(doc: &PdfDocument, opts: &ExtractOptions) -> Result<ExtractedDocument> {
    let mut pages = Vec::new();
    let mut warnings = Vec::new();
    let n = doc.page_count();
    for i in 0..n {
        let page_info = doc.pages.get(i as usize).unwrap();
        let text = page_text(doc, i as usize, &opts.text)?;
        let (runs, mut pw) = page_elements(doc, i as usize, &opts.text)?;
        warnings.append(&mut pw);
        pages.push(ExtractedPage {
            index: i,
            media_box: page_info.media_box,
            crop_box: page_info.crop_box,
            rotate: page_info.rotate,
            text,
            elements: runs.into_iter().map(Element::Text).collect(),
            warnings: Vec::new(),
        });
    }
    Ok(ExtractedDocument {
        schema_version: SCHEMA_VERSION,
        metadata: DocumentMetadata {
            title: doc.info_string(b"Title"),
            author: doc.info_string(b"Author"),
            producer: doc.info_string(b"Producer"),
            pdf_version: Some(doc.version.clone()),
            page_count: n,
        },
        pages,
        warnings,
        partial: false,
    })
}
