//! Page extract orchestration (text + Phase V tables).
use crate::font_load::load_page_fonts;
use crate::options::{ExtractOptions, TextOptions};
use crate::raster_images::raster_pages_for_page;
use pdfparser_content::{interpret_page, interpret_page_with_resolver, InterpretOptions};
use pdfparser_core::{Error, PdfDocument, Result};
use pdfparser_ir::{
    DocumentMetadata, Element, ExtractWarning, ExtractedDocument, ExtractedPage, TextRun,
    WarningCode, SCHEMA_VERSION,
};
use pdfparser_layout::{apply_page_rotate_to_runs, insert_spaces, reading_order_text};
use pdfparser_tables::{
    detect_tables_page_with_raster, materialize_stitched, stitch_document, ExternalCliPageRenderer,
    PageRenderer, RasterPage, RenderSafety, Table, TableOptions,
};
use std::path::Path;
// form scrub is applied via materialize path in tables crate after stitch

// Form XObject resolver (PR2a) — sibling module, not registered in lib.rs to
// keep façade surface small; owned by the extract path.
#[path = "form_resolve.rs"]
mod form_resolve;
use form_resolve::DocFormResolver;

/// Intermediate page content after interpret.
pub struct PageContent {
    pub runs: Vec<TextRun>,
    pub rules: Vec<pdfparser_content::RuleSegment>,
    /// Image placements for raster line sensing.
    pub image_placements: Vec<pdfparser_content::ImagePlacement>,
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
        capture_image_placements: capture_rules, // same gate as rules for table path
        ..InterpretOptions::default()
    };
    // When capturing rules (table path), expand Form XObjects so vector rules
    // painted inside forms become RuleSegments (PR2a / K19).
    let mut result = if capture_rules {
        match DocFormResolver::for_page(doc, page_index) {
            Ok(mut resolver) => {
                interpret_page_with_resolver(&content, &fonts, &iopts, Some(&mut resolver))
            }
            Err(_) => interpret_page(&content, &fonts, &iopts),
        }
    } else {
        interpret_page(&content, &fonts, &iopts)
    };
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
        image_placements: result.image_placements,
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
    source_path: Option<&Path>,
) -> Result<Vec<Table>> {
    if !table_opts.detect_tables {
        return Ok(Vec::new());
    }
    let pc = page_content(doc, page_index, text_opts, true)?;
    let mut raster = if table_opts.raster_line_detect {
        raster_pages_for_page(doc, page_index, &pc.image_placements).unwrap_or_default()
    } else {
        Vec::new()
    };
    // PR3 / K25: full-page render (external CLI) — explicit HQ or opportunistic.
    if want_full_page_render(table_opts, &pc.rules, &pc.runs) {
        if let Some(rp) = try_full_page_render(doc, page_index, source_path, table_opts) {
            raster.push(rp);
        }
    }
    let page = doc.pages.get(page_index);
    let page_size = page.map(|p| {
        (
            (p.media_box.x1 - p.media_box.x0).abs().max(1.0),
            (p.media_box.y1 - p.media_box.y0).abs().max(1.0),
        )
    });
    let tabs = detect_tables_page_with_raster(
        page_index as u32,
        &pc.runs,
        &pc.rules,
        table_opts,
        &raster,
        page_size,
    );
    Ok(tabs)
}

/// K25 / HQ: whether to request external full-page gray render for this page.
///
/// - Explicit HQ (`enable_full_page_render`): request render **unless** vector
///   lattice is already rich. Blind full-page render on strong vector pages
///   can inject decorative rules and regress cell F1 (e.g. schools contributions).
/// - Opportunistic Auto (`allow_auto_render`): weak vector + multi-col text.
fn want_full_page_render(
    table_opts: &TableOptions,
    rules: &[pdfparser_content::RuleSegment],
    runs: &[pdfparser_ir::TextRun],
) -> bool {
    let h = rules
        .iter()
        .filter(|r| r.is_horizontal(1.5) && r.len() >= 8.0)
        .count();
    let v = rules
        .iter()
        .filter(|r| r.is_vertical(1.5) && r.len() >= 8.0)
        .count();
    let vector_rich = h >= 4 && v >= 3;

    if table_opts.enable_full_page_render {
        // HighQuality: skip when vector lattice already owns the page.
        return !vector_rich;
    }
    if !table_opts.allow_auto_render {
        return false;
    }
    // Opportunistic: few axis-aligned rules but strong multi-col text bands.
    if vector_rich {
        return false;
    }
    let mut lefts: Vec<f32> = runs
        .iter()
        .filter(|r| !r.text.trim().is_empty())
        .map(|r| r.bbox.x0)
        .collect();
    if lefts.len() < 12 {
        return false;
    }
    lefts.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    // Cluster rough columns
    let mut cols = 1u32;
    let mut prev = lefts[0];
    for &x in &lefts[1..] {
        if x - prev > 12.0 {
            cols += 1;
            prev = x;
        }
    }
    cols >= 4 && (h + v) < 6
}

/// Attempt external full-page gray render for line sensing (Tier 1).
fn try_full_page_render(
    doc: &PdfDocument,
    page_index: usize,
    source_path: Option<&Path>,
    table_opts: &TableOptions,
) -> Option<RasterPage> {
    if !table_opts.allow_auto_render && !table_opts.enable_full_page_render {
        return None;
    }
    let path = source_path?;
    let page = doc.pages.get(page_index)?;
    let w = (page.media_box.x1 - page.media_box.x0).abs().max(1.0);
    let h = (page.media_box.y1 - page.media_box.y0).abs().max(1.0);
    let renderer = ExternalCliPageRenderer::new(path, w, h);
    let safety = RenderSafety::default();
    // Fail-soft: continue with vector + embedded when render is unavailable.
    renderer.render_gray(page_index as u32, 150, &safety).ok()
}

/// Document-level table extract: page fragments + optional D1 stitched logical tables.
pub fn document_tables(
    doc: &PdfDocument,
    text_opts: &TextOptions,
    table_opts: &TableOptions,
    source_path: Option<&Path>,
) -> Result<(Vec<Vec<Table>>, Vec<Table>)> {
    if !table_opts.detect_tables {
        return Ok((Vec::new(), Vec::new()));
    }
    let n = doc.page_count() as usize;
    let mut page_runs: Vec<Vec<TextRun>> = Vec::with_capacity(n);
    let mut page_rules: Vec<Vec<pdfparser_content::RuleSegment>> = Vec::with_capacity(n);
    let mut page_raster: Vec<Vec<RasterPage>> = Vec::with_capacity(n);
    let mut page_heights: Vec<f32> = Vec::with_capacity(n);
    for i in 0..n {
        let pc = page_content(doc, i, text_opts, true)?;
        let mut raster = if table_opts.raster_line_detect {
            raster_pages_for_page(doc, i, &pc.image_placements).unwrap_or_default()
        } else {
            Vec::new()
        };
        if want_full_page_render(table_opts, &pc.rules, &pc.runs) {
            if let Some(rp) = try_full_page_render(doc, i, source_path, table_opts) {
                raster.push(rp);
            }
        }
        page_runs.push(pc.runs);
        page_rules.push(pc.rules);
        page_raster.push(raster);
        let height = doc
            .pages
            .get(i)
            .map(|p| (p.media_box.y1 - p.media_box.y0).abs().max(1.0))
            .unwrap_or(0.0);
        page_heights.push(height);
    }
    let mut page_tables: Vec<Vec<Table>> = (0..n)
        .map(|i| {
            let page_size = doc.pages.get(i).map(|p| {
                (
                    (p.media_box.x1 - p.media_box.x0).abs().max(1.0),
                    (p.media_box.y1 - p.media_box.y0).abs().max(1.0),
                )
            });
            detect_tables_page_with_raster(
                i as u32,
                &page_runs[i],
                &page_rules[i],
                table_opts,
                &page_raster[i],
                page_size,
            )
        })
        .collect();
    if table_opts.stitch_multipage {
        stitch_document(&mut page_tables, &page_heights, table_opts);
    }
    let mut logical = if table_opts.stitch_multipage {
        materialize_stitched(&page_tables)
    } else {
        page_tables.iter().flatten().cloned().collect()
    };
    if table_opts.form_discriminator {
        logical = pdfparser_tables::scrub_document_table_fps(logical, table_opts);
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

#[cfg(test)]
mod form_expand_tests {
    use super::*;
    use pdfparser_core::ResourceLimits;

    /// 3×3 ruled grid painted only inside a Form XObject (page is `/Fm1 Do`).
    fn form_grid_pdf() -> Vec<u8> {
        // 4H + 4V lines, longer cell labels so tiny-chrome reject does not fire.
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
    fn page_content_expands_form_rules_and_text() {
        let data = form_grid_pdf();
        let doc = PdfDocument::from_bytes(&data, ResourceLimits::default()).unwrap();
        let pc = page_content(&doc, 0, &TextOptions::default(), true).unwrap();
        assert!(
            pc.rules.len() >= 8,
            "expected grid rules from Form XObject, got {} warnings={:?}",
            pc.rules.len(),
            pc.warnings
        );
        assert!(
            pc.runs.len() >= 9,
            "expected text from form, got {}",
            pc.runs.len()
        );
        // Without capture_rules the resolver is not used — no form expand.
        let pc_no = page_content(&doc, 0, &TextOptions::default(), false).unwrap();
        assert!(
            pc_no.rules.is_empty(),
            "text-only path should not expand forms for rules"
        );
    }

    #[test]
    fn form_grid_lattice_detects() {
        use pdfparser_tables::{detect_tables_page_with_diagnostics, TableOptions, TablePreset};
        let data = form_grid_pdf();
        let doc = PdfDocument::from_bytes(&data, ResourceLimits::default()).unwrap();
        let pc = page_content(&doc, 0, &TextOptions::default(), true).unwrap();
        let mut opts = TableOptions::from_preset(TablePreset::LatticeOnly);
        opts.raster_line_detect = false;
        let (tabs, diag) =
            detect_tables_page_with_diagnostics(0, &pc.runs, &pc.rules, &opts, &[], 300.0, 300.0);
        assert!(
            !tabs.is_empty(),
            "lattice should find form grid; rules={} runs={} diag={diag:?}",
            pc.rules.len(),
            pc.runs.len()
        );
        assert!(
            tabs[0].rows >= 3 && tabs[0].cols >= 3,
            "shape {:?}",
            (tabs[0].rows, tabs[0].cols)
        );
    }
}
