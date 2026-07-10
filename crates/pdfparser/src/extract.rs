//! Page extract orchestration (text + Phase V tables).
use crate::font_load::load_page_fonts;
use crate::options::{ExtractOptions, TextOptions};
use pdfparser_content::{interpret_page, InterpretOptions};
use pdfparser_core::{Error, PdfDocument, Result};
use pdfparser_ir::{
    DocumentMetadata, Element, ExtractWarning, ExtractedDocument, ExtractedPage, TextRun,
    WarningCode, SCHEMA_VERSION,
};
use pdfparser_layout::{apply_page_rotate_to_runs, insert_spaces, reading_order_text};
use pdfparser_tables::{
    detect_tables_page, materialize_stitched, stitch_document, Table, TableOptions,
};
// form scrub is applied via materialize path in tables crate after stitch

/// Intermediate page content after interpret.
pub struct PageContent {
    pub runs: Vec<TextRun>,
    pub rules: Vec<pdfparser_content::RuleSegment>,
    pub warnings: Vec<ExtractWarning>,
}

pub fn page_content(
    doc: &PdfDocument,
    page_index: usize,
    opts: &TextOptions,
    capture_rules: bool,
) -> Result<PageContent> {
    let page = doc.pages.get(page_index).ok_or(Error::PageOutOfRange {
        index: page_index as u32,
    })?;
    let content = doc.page_content_bytes(page_index)?;
    let font_refs = doc.page_font_map(page_index)?;
    let fonts = doc.with_doc(|d| load_page_fonts(d, &font_refs))?;

    let iopts = InterpretOptions {
        max_ops: doc.governor.limits.max_page_ops,
        capture_rules,
    };
    let mut result = interpret_page(&content, &fonts, &iopts);
    let mut warnings: Vec<ExtractWarning> = result
        .warnings
        .into_iter()
        .map(|message| ExtractWarning {
            code: WarningCode::UnknownOperator,
            page: Some(page_index as u32),
            message,
            recoverable: true,
        })
        .collect();

    if !opts.include_invisible {
        result.runs.retain(|r| !r.invisible);
    }

    if opts.apply_page_rotate {
        apply_page_rotate_to_runs(&mut result.runs, page.rotate, page.media_box);
        // Rotate rule endpoints as well
        if page.rotate.rem_euclid(360) != 0 {
            use pdfparser_ir::Point;
            use pdfparser_layout::rotate_point;
            for r in &mut result.rules {
                let p0 = rotate_point(Point { x: r.x0, y: r.y0 }, page.rotate, page.media_box);
                let p1 = rotate_point(Point { x: r.x1, y: r.y1 }, page.rotate, page.media_box);
                r.x0 = p0.x;
                r.y0 = p0.y;
                r.x1 = p1.x;
                r.y1 = p1.y;
            }
        }
    }

    if opts.insert_spaces {
        result.runs = insert_spaces(&result.runs);
    }

    if result.runs.is_empty() && !content.is_empty() {
        warnings.push(ExtractWarning {
            code: WarningCode::Other,
            page: Some(page_index as u32),
            message: "no text runs extracted".into(),
            recoverable: true,
        });
    }

    Ok(PageContent {
        runs: result.runs,
        rules: result.rules,
        warnings,
    })
}

pub fn page_elements(
    doc: &PdfDocument,
    page_index: usize,
    opts: &TextOptions,
) -> Result<(Vec<TextRun>, Vec<ExtractWarning>)> {
    let pc = page_content(doc, page_index, opts, false)?;
    Ok((pc.runs, pc.warnings))
}

pub fn page_text(doc: &PdfDocument, page_index: usize, opts: &TextOptions) -> Result<String> {
    let (runs, _) = page_elements(doc, page_index, opts)?;
    if opts.sort_reading_order {
        Ok(reading_order_text(&runs))
    } else {
        Ok(runs.into_iter().map(|r| r.text).collect())
    }
}

pub fn page_tables(
    doc: &PdfDocument,
    page_index: usize,
    text_opts: &TextOptions,
    table_opts: &TableOptions,
) -> Result<Vec<Table>> {
    if !table_opts.detect_tables {
        return Ok(Vec::new());
    }
    let pc = page_content(doc, page_index, text_opts, true)?;
    Ok(detect_tables_page(
        page_index as u32,
        &pc.runs,
        &pc.rules,
        table_opts,
    ))
}

/// Document-level table extract: page fragments + optional D1 stitched logical tables.
pub fn document_tables(
    doc: &PdfDocument,
    text_opts: &TextOptions,
    table_opts: &TableOptions,
) -> Result<(Vec<Vec<Table>>, Vec<Table>)> {
    if !table_opts.detect_tables {
        return Ok((Vec::new(), Vec::new()));
    }
    let n = doc.page_count() as usize;
    let mut page_runs: Vec<Vec<TextRun>> = Vec::with_capacity(n);
    let mut page_rules: Vec<Vec<pdfparser_content::RuleSegment>> = Vec::with_capacity(n);
    for i in 0..n {
        let pc = page_content(doc, i, text_opts, true)?;
        page_runs.push(pc.runs);
        page_rules.push(pc.rules);
    }
    let mut page_tables: Vec<Vec<Table>> = (0..n)
        .map(|i| detect_tables_page(i as u32, &page_runs[i], &page_rules[i], table_opts))
        .collect();
    if table_opts.stitch_multipage {
        stitch_document(&mut page_tables, table_opts);
    }
    let mut logical = if table_opts.stitch_multipage {
        materialize_stitched(&page_tables)
    } else {
        page_tables.iter().flatten().cloned().collect()
    };
    if table_opts.form_discriminator {
        logical = pdfparser_tables::scrub_document_table_fps(logical);
    }
    Ok((page_tables, logical))
}

/// Extract whole document (text + optional tables).
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
