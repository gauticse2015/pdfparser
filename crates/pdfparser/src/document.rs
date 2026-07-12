//! Document / Page handles.
use crate::extract::{document_tables, page_elements, page_tables, page_text};
use crate::font_load::load_page_fonts;
use crate::options::{OpenOptions, TextOptions};
use pdfparser_core::{Error, PdfDocument, Result};
use pdfparser_ir::{Element, ExtractWarning, TextRun};
use pdfparser_tables::{Table, TableOptions};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Opened document (cheap to clone via Arc).
#[derive(Clone)]
pub struct Document {
    inner: Arc<PdfDocument>,
    /// Source path when opened from disk (needed for external full-page render).
    source_path: Option<PathBuf>,
}

impl Document {
    /// Open path with defaults.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        Self::open_with(path, OpenOptions::default())
    }

    /// Open with options.
    pub fn open_with(path: impl AsRef<Path>, opts: OpenOptions) -> Result<Self> {
        let path = path.as_ref();
        let pdf = PdfDocument::open(path, opts.limits)?;
        Ok(Self {
            inner: Arc::new(pdf),
            source_path: Some(path.to_path_buf()),
        })
    }

    /// Open from bytes.
    pub fn from_bytes(data: &[u8], opts: OpenOptions) -> Result<Self> {
        let pdf = PdfDocument::from_bytes(data, opts.limits)?;
        Ok(Self {
            inner: Arc::new(pdf),
            source_path: None,
        })
    }

    /// Filesystem path if opened via [`Document::open`].
    pub fn source_path(&self) -> Option<&Path> {
        self.source_path.as_deref()
    }

    /// Page count.
    pub fn page_count(&self) -> u32 {
        self.inner.page_count()
    }

    /// PDF version string.
    pub fn version(&self) -> &str {
        &self.inner.version
    }

    /// Info dict string.
    pub fn info(&self, key: &str) -> Option<String> {
        self.inner.info_string(key.as_bytes())
    }

    /// Get page handle (0-based).
    pub fn page(&self, index: u32) -> Result<Page> {
        if index >= self.page_count() {
            return Err(Error::PageOutOfRange { index });
        }
        Ok(Page {
            doc: self.inner.clone(),
            index,
            source_path: self.source_path.clone(),
        })
    }

    /// Detect tables across all pages.
    ///
    /// Returns `(page_fragments, logical_tables)`. When `stitch_multipage` is
    /// enabled, fragments carry `continued_*` / `logical_table_id` and
    /// `logical_tables` are stitched logical tables when multipage stitch is on.
    pub fn tables(
        &self,
        text_opts: &TextOptions,
        table_opts: &TableOptions,
    ) -> Result<(Vec<Vec<Table>>, Vec<Table>)> {
        document_tables(
            &self.inner,
            text_opts,
            table_opts,
            self.source_path.as_deref(),
        )
    }

    /// Stitched logical tables only (Phase V D1).
    pub fn tables_stitched(
        &self,
        text_opts: &TextOptions,
        table_opts: &TableOptions,
    ) -> Result<Vec<Table>> {
        let (_, logical) = self.tables(text_opts, table_opts)?;
        Ok(logical)
    }

    /// Images, URI links, AcroForm fields, and outline titles.
    pub fn objects(&self) -> Result<pdfparser_core::DocumentObjects> {
        self.inner.objects()
    }
}

/// Lazy page handle.
pub struct Page {
    doc: Arc<PdfDocument>,
    index: u32,
    /// Path when parent [`Document`] was opened from disk (full-page render).
    source_path: Option<PathBuf>,
}

impl Page {
    /// 0-based index.
    pub fn index(&self) -> u32 {
        self.index
    }

    /// Page /Rotate.
    pub fn rotate(&self) -> i32 {
        self.doc
            .pages
            .get(self.index as usize)
            .map(|p| p.rotate)
            .unwrap_or(0)
    }

    /// Media box.
    pub fn media_box(&self) -> pdfparser_ir::Rect {
        self.doc
            .pages
            .get(self.index as usize)
            .map(|p| p.media_box)
            .unwrap_or(pdfparser_ir::Rect {
                x0: 0.0,
                y0: 0.0,
                x1: 612.0,
                y1: 792.0,
            })
    }

    /// Extract plain text with options.
    pub fn text(&self, opts: &TextOptions) -> Result<String> {
        page_text(&self.doc, self.index as usize, opts)
    }

    /// Paint-order text runs (after optional rotate).
    pub fn text_runs(&self, opts: &TextOptions) -> Result<Vec<TextRun>> {
        let (runs, _) = page_elements(&self.doc, self.index as usize, opts)?;
        Ok(runs)
    }

    /// Elements.
    pub fn elements(&self, opts: &TextOptions) -> Result<(Vec<Element>, Vec<ExtractWarning>)> {
        let (runs, warns) = page_elements(&self.doc, self.index as usize, opts)?;
        let elements = runs.into_iter().map(Element::Text).collect();
        Ok((elements, warns))
    }

    /// Detect tables on this page (page-local fragments; no cross-page stitch).
    ///
    /// When the parent document was opened from a path and
    /// `table_opts.enable_full_page_render` is set, may fail-soft to an external
    /// CLI gray render (pdftoppm / mutool / gs) for line sensing.
    pub fn tables(&self, text_opts: &TextOptions, table_opts: &TableOptions) -> Result<Vec<Table>> {
        page_tables(
            &self.doc,
            self.index as usize,
            text_opts,
            table_opts,
            self.source_path.as_deref(),
        )
    }

    /// Load fonts for this page (testing/debug).
    pub fn font_names(&self) -> Result<Vec<String>> {
        let refs = self.doc.page_font_map(self.index as usize)?;
        Ok(refs.into_iter().map(|(n, _)| n).collect())
    }

    #[allow(dead_code)]
    pub(crate) fn load_fonts(
        &self,
    ) -> Result<std::collections::HashMap<String, pdfparser_fonts::LoadedFont>> {
        let refs = self.doc.page_font_map(self.index as usize)?;
        self.doc.with_doc(|d| Ok(load_page_fonts(d, &refs)))?
    }
}
