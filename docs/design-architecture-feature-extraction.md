# Native PDF Parser — Architecture & Feature Extraction Design (Volume 2)

| Field | Value |
|-------|-------|
| **Title** | Native PDF Parser — Deep Architecture & Feature Extraction Design |
| **Document type** | Companion / Volume 2 to Product Design |
| **Companion to** | [`docs/design-native-pdf-parser.md`](../docs/design-native-pdf-parser.md) (rev 2.2) |
| **Author** | TBD |
| **Date** | 2026-07-10 |
| **Status** | Draft (rev 1.2 — ASCII architecture diagrams for IDE readability) |
| **Product / crate** | **`pdfparser`** |
| **Audience** | Senior engineers implementing pipeline stages and feature modules |
| **Scope priority** | **Text extraction first** (aligns with product K35) |
| **Language** | Rust (edition 2021+) |
| **Workspace** | `/Users/gautamkumar/Desktop/pdfParser` (greenfield) |
| **Diagrams** | **ASCII / monospaced boxes only** (no Mermaid) — visible in any IDE |

> **Relationship to product design (rev 2.2).** This document does **not** reopen locked product decisions (K1–K36). Where the product doc is correct, this volume **references** it and expands algorithms, types, control flow, and per-feature extraction contracts so implementers do not invent mid-flight. Conflicts: **product design wins** for product scope; this volume wins for algorithm/detail specificity within that scope.
>
> **Conflict log (Volume 2 vs product — explicit deltas):**

| Topic | Product rev 2.2 | Volume 2 (this doc) | Resolution |
|-------|-----------------|---------------------|------------|
| `XRefEntry` | `Free` / `Uncompressed { offset }` / `Compressed { stream_id, index }` (trait sketch) | Core-internal richer form with generation | **Trait surface matches product**; core may store gen via adapter (Issue 4) |
| Image default | K23 encoded/deferred, `decode_pixels: false` | Same — **no** default sample expansion for Flate images | Aligned (Issue 3) |
| `FontInput.wmode` | Not in product sketch | Additive field | Volume-2 normative additive; product sync recommended |
| Limits | Product `ResourceLimits` / `LimitKind` | Additive fields: ObjStm, path segments, string bytes | Volume-2 normative additives; product doc may absorb later |
| `WarningCode` extensions | Product base set | Additive variants | Additive = non-breaking before 0.1 freeze |
| `apply_page_rotate` | true for text/layout; false for raw elements | Restated per-API defaults | Aligned with product |


---

## Overview

This volume specifies the **implementable architecture** of `pdfparser` and the **full extraction algorithms** for every major feature category. Engineers should be able to implement object resolution, stream decoding, the content VM, font metrics, text geometry, images, tables (all supported types), annotations, forms, structure tree, OCG, and document-level objects from this document alone—without inventing data models or detection heuristics on the fly.

The system is a **layered, library-first pipeline**:

1. **Object layer** — `ObjectBackend` supplies encoded objects + xref; core owns ObjStm unpack and all filter decoding under a **Resource Governor**.
2. **Document model** — page tree, resource inheritance, font loading (pure data in → metrics/unicode out), image/XObject metadata.
3. **Content VM** — tokenize operators → graphics/text state → **Page IR** (`Element`s with geometry).
4. **Layout & semantics** — space insertion, lines/blocks/reading order, structure mapping, **experimental** table detectors (off by default in 0.1).
5. **Public surface** — idiomatic sync Rust API + JSON export (`schema_version`); CLI is a thin consumer.

**0.1 bar (from product doc):** unencrypted digital PDFs only (`Error::Encryption` on any `/Encrypt`); solid L0–L2; basic L3; tables off by default; images default encoded/deferred. **1.0 bar** tightens CER/bbox and stabilizes tables/structure.

---

## Background & Motivation

### Why a second design volume

The product design freezes **what** we build, **when**, and **which contracts** (ObjectBackend, governor, IR allowlist, metrics gates). Implementation still needs:

- Normative step-by-step algorithms for ObjStm, filter chains, text positioning, lattice/stream tables, etc.
- Exhaustive taxonomies (table types, annotation subtypes, form field types).
- Per-feature confidence signals, edge cases, acceptance tests, and phase tags.
- Crate-level module maps and sequence diagrams for `Document::open` → JSON.

### Current state

Greenfield workspace; only product design exists under `docs/`. No crates yet. This volume is the **engineering blueprint** for P0–P2 feature modules.

### Pain points this design addresses

| Pain | Design response |
|------|-----------------|
| PDF is paint-order, not semantics | Unified IR + optional layout/structure/table layers |
| Wrong bboxes without metrics | Font widths pipeline separate from ToUnicode |
| Third-party decode bypasses limits | K16: encoded-only backend; owned filters |
| Tables are heuristic | Taxonomy + confidence + off-by-default + eval harness |
| Multi-tenant DoS | Governor + hard_max + process isolation guidance |
| Feature discovery for embedders | `WarningCode` + `partial` + feature flags |

---

## Goals & Non-Goals

### Goals (this volume)

1. Specify **crate boundaries**, dependency DAG, and type ownership so PRs do not create cycles.
2. Specify **every pipeline stage** with inputs/outputs, algorithms, Rust type sketches, failure modes, limits, and tests.
3. Specify **every extraction feature** (C1–C12) to implementation depth: taxonomy, algorithm, data model, confidence, edges, acceptance, phase.
4. Unify **IR + API + JSON** contracts with product K20 freeze list; mark unstable vs frozen.
5. Provide **extraction-specific key decisions (EX-*)**, risk table, PR plan ordered by text-first critical path.

### Non-Goals (inherited; not reopened)

- Full rasterizer, OCR primary path, PDF write, async/WASM in v1
- Encrypted open in 0.1 (K15)
- Perfect untagged tables
- JS/XFA execution, signature crypto verify
- Contradiction of product K1–K36

---

# Part A — High-Level Architecture

## A1. System context

Embedders call the library; the library never shells out for parse/extract. CLI and CI tools are consumers of the same API.

```text
══════════════════════════════════════════════════════════════════════════════
                         SYSTEM CONTEXT — who talks to whom
══════════════════════════════════════════════════════════════════════════════

  CONSUMERS (outside the product)              INPUT
  ┌─────────────────────────────┐              ┌──────────────────┐
  │  Rust app / service         │              │  PDF file        │
  │  ETL / RAG pipeline         │── open / ───▶│  path | bytes    │
  │  pdfparser-cli (secondary)  │   extract    │  Read+Seek       │
  │  CI golden / oracle jobs    │              └────────┬─────────┘
  └──────────────┬──────────────┘                       │
                 │                                      │ encoded
                 ▼                                      ▼
  ┌──────────────────────────────────────────────────────────────────────────┐
  │                     pdfparser  (public façade crate)                     │
  │  ┌────────────────┐   ┌─────────────────┐   ┌─────────────────────────┐  │
  │  │ Document / Page│──▶│ extract / text  │──▶│ JSON / text export      │  │
  │  │ API            │   │ / images        │   │ (schema_version)        │  │
  │  └───────┬────────┘   └────────┬────────┘   └─────────────────────────┘  │
  └──────────┼─────────────────────┼─────────────────────────────────────────┘
             │                     │
             ▼                     ▼
  ┌──────────────────────┐  ┌──────────────────────────────────────────────┐
  │  pdfparser-core      │  │  content VM  │  fonts  │  layout / tables    │
  │  + ResourceGovernor  │  │  (interpret page content → IR)               │
  │  + owned filters     │  └──────────────────────────────────────────────┘
  │  + ObjectBackend ─────┼──────▶  PDF object graph (encoded only)
  └──────────────────────┘

  Data in:  path | Read+Seek+Send+'static | Vec<u8>  + OpenOptions / ExtractOptions
  Data out: ExtractedDocument | Vec<Element> | plain text | image handles | warnings
  Never:    decrypted encrypted PDFs (0.1) | executed JS | full-page bitmaps (v1)
```

## A2. Layered architecture

```text
══════════════════════════════════════════════════════════════════════════════
              LAYERED ARCHITECTURE (data flows top → bottom)
══════════════════════════════════════════════════════════════════════════════

  ┌────────────────────────────────────────────────────────────────────────┐
  │  L4  PUBLIC API                                                        │
  │      Document / Page  ·  ExtractOptions  ·  serde JSON export          │
  │      crates: pdfparser  ·  pdfparser-export  ·  pdfparser-ir           │
  └─────────────────────────────────┬──────────────────────────────────────┘
                                    │ ExtractedDocument / Element IR
  ┌─────────────────────────────────▼──────────────────────────────────────┐
  │  L3  LAYOUT & SEMANTICS          (optional; tables OFF by default)     │
  │      ┌────────────┐ ┌──────────┐ ┌────────────┐ ┌───────────────────┐  │
  │      │ spaces /   │ │ structure│ │ table      │ │ annots / forms /  │  │
  │      │ lines /    │ │ tree     │ │ detectors  │ │ OCG               │  │
  │      │ reading    │ │ (MCID)   │ │ (exp.)     │ │                   │  │
  │      │ order      │ └──────────┘ └────────────┘ └───────────────────┘  │
  │      crates: pdfparser-layout  (+ extractors in core)                  │
  └─────────────────────────────────┬──────────────────────────────────────┘
                                    │ paint-order Page IR  (Vec<Element>)
  ┌─────────────────────────────────▼──────────────────────────────────────┐
  │  L2  CONTENT VM                                                        │
  │      tokenizer  →  operator dispatch  →  graphics/text state  →  IR    │
  │      crate: pdfparser-content                                          │
  └─────────────────────────────────┬──────────────────────────────────────┘
                                    │ fonts + XObjects + content streams
  ┌─────────────────────────────────▼──────────────────────────────────────┐
  │  L1  DOCUMENT MODEL                                                    │
  │      Catalog / Info / XMP                                              │
  │      Page tree + resource inheritance                                  │
  │      Font system (metrics + Unicode)     XObject registry              │
  │      crates: pdfparser-core  ·  pdfparser-fonts                        │
  └─────────────────────────────────┬──────────────────────────────────────┘
                                    │ decoded streams under limits
  ┌─────────────────────────────────▼──────────────────────────────────────┐
  │  L0  OBJECT MODEL  +  SECURITY BOUNDARY                                │
  │                                                                        │
  │   ObjectBackend          ObjectStore + ObjStm       Owned filters      │
  │   (encoded only)  ──▶    (resolve refs)      ──▶   (+ Predictor)       │
  │         ▲                         ▲                     ▲              │
  │         └──────── Resource Governor (caps, cycles, budgets) ───────────┘
  │   crate: pdfparser-core                                                │
  └────────────────────────────────────────────────────────────────────────┘
                                    ▲
                                    │ PDF bytes / path / reader
```

| Layer | Responsibility | Crate(s) |
|-------|----------------|----------|
| L0 | Syntax, xref, encoded objects, decode under limits | `pdfparser-core` + backend |
| L1 | Pages, resources, fonts, XObjects, doc metadata | `pdfparser-core`, `pdfparser-fonts` |
| L2 | Operators → positioned elements | `pdfparser-content` |
| L3 | Reading order, tables, structure, annots | `pdfparser-layout`, core extractors |
| L4 | Public types, extract orchestration, JSON | `pdfparser-ir`, `pdfparser`, `pdfparser-export` |

## A3. Crate boundaries and dependency DAG

Aligns with product design; repeated here as **normative for implementers**.

```text
pdfParser/
├── Cargo.toml
├── crates/
│   ├── pdfparser-ir/          # public IR + WarningCode
│   ├── pdfparser-core/        # backend, governor, filters, page tree
│   ├── pdfparser-fonts/       # pure font logic (bytes in → metrics out)
│   ├── pdfparser-content/     # content lexer + graphics/text VM
│   ├── pdfparser-layout/      # clustering, spaces, tables, structure
│   ├── pdfparser-export/      # JSON / text / md / csv
│   ├── pdfparser/             # public façade (only crates.io crate in 0.1)
│   └── pdfparser-cli/         # secondary binary
├── fuzz/
├── corpus/
├── schemas/extract-v1.json
└── docs/
```

**Dependency direction (arrows = “depends on”; no cycles):**

```text
══════════════════════════════════════════════════════════════════════════════
                         CRATE DEPENDENCY DAG
══════════════════════════════════════════════════════════════════════════════

                    pdfparser-cli
                          │
                          ▼
                      pdfparser          ◄── only public crates.io crate (0.1)
                     /  |  |  \  \
                    /   |  |   \  \
                   ▼    ▼  ▼    ▼  ▼
              content layout core export  ir
                 │  \    │    │     │
                 │   \   │    │     │
                 ▼    ▼  ▼    ▼     │
              fonts ─────core       │
                 │         │        │
                 └────►   ir   ◄────┘
                          ▲
                          │
              pdfparser-fonts  (leaf: depends on ir only; NEVER core)

  Rules:
    · fonts  →  ir only          (no core import; pure data in)
    · core   →  ir, fonts
    · content→  ir, fonts, core
    · layout →  ir, content
    · export →  ir only
    · api    →  ir, core, content, layout, export
    · cli    →  api only
```

| Concern | Crate | Rule |
|---------|-------|------|
| Public IR, `WarningCode`, `Rect`, `ObjectId` | `pdfparser-ir` | No core/fonts deps |
| `PdfObject`, `EncodedStream`, filters, governor, page tree, ObjStm | `pdfparser-core` | Never re-export as 0.1 public freeze |
| `FontInput` → `LoadedFont` | `pdfparser-fonts` | **No** import of core; bytes-in only |
| Lexer + graphics VM | `pdfparser-content` | |
| Clustering, tables, structure map helpers | `pdfparser-layout` | |
| JSON/text/md/csv | `pdfparser-export` | Depends on ir only |
| Façade only publishable in 0.1 | `pdfparser` | |

### Module map (core)

```text
pdfparser-core/
  backend/     # ObjectBackend, LopdfBackend, FixtureBackend
  object/      # PdfObject, PdfDict, EncodedStream
  xref/        # entry types (if owned), incremental merge helpers
  objstm/      # parse N/First header + objects
  filters/     # flate, ascii85/hex, lzw, runlength, predictor, chain
  limits/      # ResourceGovernor, ResourceLimits, hard_max
  page_tree/   # Kids walk, inheritance
  resources/   # merge Resources dicts
  image/       # ImageMeta from XObject / inline
  annot/       # annotation extract
  form/        # AcroForm field tree
  structure/   # StructTreeRoot walk (or shared with layout)
  catalog/     # Info, XMP, outline, names, OCProperties
```

## A4. End-to-end sequence: open → extract → JSON

```text
══════════════════════════════════════════════════════════════════════════════
              END-TO-END FLOW: Document::open → extract → JSON
══════════════════════════════════════════════════════════════════════════════

  PHASE 1 — OPEN (once per document)
  ───────────────────────────────────
  App
   │  open_with(path, OpenOptions)
   ▼
  Document
   │  1. clamp ResourceLimits to hard_max
   │  2. ObjectBackend: parse header / xref / trailer
   │  3. if trailer has /Encrypt  →  Error::Encryption  (0.1 hard stop)
   │  4. resolve /Root → /Pages tree (kids may stay lazy)
   ▼
  Document handle returned to App  (Arc<DocumentInner>)


  PHASE 2 — EXTRACT (per page, lazy)
  ──────────────────────────────────
  App
   │  extract(ExtractOptions)   or   page(i).text() / .elements()
   ▼
  for each page index i:
      │
      ├─▶ Page::page(i)
      │       │
      │       ├─ ObjectStore  ──get_encoded──▶  ObjectBackend
      │       │       │                         (xref; Compressed → ObjStm id)
      │       │       ▼
      │       │   filters::decode_stream(governor, encoded)
      │       │       │  charge expanded bytes / op budgets
      │       │       ▼
      │       ├─ fonts::load_font(FontInput)     ◄── decoded font programs
      │       │
      │       └─ ContentVM::interpret(content, resources, fonts)
      │               │
      │               ▼
      │         Vec<Element>   (paint-order page IR)
      │               │
      │               ├─ [if layout needed]  spaces → lines → reading order
      │               └─ [if detect_tables]  structure ∪ lattice ∪ stream ∪ hybrid
      │                                      → NMS → tables
      ▼
  ExtractedDocument { pages[], warnings[], partial, metadata, … }


  PHASE 3 — EXPORT
  ────────────────
  export::to_json(ExtractedDocument)  ──▶  JSON  (schema_version = 1)
```

### Pseudocode orchestration

```rust
// pdfparser façade (normative control flow)
fn extract(doc: &Document, opts: &ExtractOptions) -> Result<ExtractedDocument, Error> {
    let _t = doc.governor.time_budget_open_or_extract();
    let mut pages = Vec::with_capacity(doc.page_count() as usize);
    let mut warnings = doc.warnings().to_vec();
    let mut partial = false;
    // Internal interpret options may force path segments when tables on (EX11 / Issue 2).
    let interpret_opts = opts.with_internal_path_capture_if_tables();

    for i in 0..doc.page_count() {
        match extract_one_page(doc, i, &interpret_opts) {
            Ok(page) => pages.push(page),
            Err(Error::LimitExceeded { kind }) if !opts.allow_partial_document => {
                return Err(Error::LimitExceeded { kind });
            }
            Err(Error::LimitExceeded { kind }) => {
                // allow_partial_document == true
                partial = true;
                warnings.push(ExtractWarning {
                    code: WarningCode::PageSkipped,
                    page: Some(i),
                    message: format!("limit exceeded: {kind:?}"),
                    recoverable: false,
                });
                pages.push(ExtractedPage::empty_placeholder(i, doc.page_boxes(i)?));
            }
            Err(e) if is_page_recoverable(&e, doc.parse_mode()) => {
                // ParseMode::Recoverable only: non-limit page failures
                partial = true;
                warnings.push(warning_from_error(i, &e));
                pages.push(ExtractedPage::empty_placeholder(i, doc.page_boxes(i).ok()).with_warning(e));
            }
            Err(e) => return Err(e),
        }
    }
    Ok(ExtractedDocument {
        schema_version: 1,
        metadata: doc.metadata_snapshot(),
        pages,
        warnings,
        partial,
        ..Default::default()
    })
}

fn extract_one_page(doc: &Document, i: u32, opts: &ExtractOptions) -> Result<ExtractedPage, Error> {
    let page = doc.page(i)?;
    // Single interpret produces paint IR (+ internal RuleSegmentBuf when needed).
    let ir = page.interpret(opts)?; // see A7 fingerprint / path policy
    let layout = if needs_layout(opts) {
        Some(layout::analyze(&ir, &opts.layout, opts.effective_apply_page_rotate_for_layout())?)
    } else {
        None
    };
    let tables = if opts.layout.detect_tables {
        // Uses ir.rule_segments (internal) even if Path elements stripped from public IR.
        Some(tables::detect_all(&ir, layout.as_ref(), &opts.layout)?)
    } else {
        None
    };
    Ok(build_extracted_page(i, ir, layout, tables, opts))
}
```

#### Normative helper definitions

| Helper | Definition |
|--------|------------|
| `needs_layout(opts)` | `true` iff any of: `opts.layout.detect_tables`; full-document extract with `opts.text.sort_reading_order` when exporting reading-order text/layout fields; caller invoked `Page::layout` / `Page::tables`; `opts` requests `PageLayout` in export. **False** for raw paint-order `elements()` alone with `sort_reading_order=false` and tables off. |
| `with_internal_path_capture_if_tables()` | If `layout.detect_tables && table_modes` intersects lattice/hybrid bits → set internal `capture_rule_segments=true` (EX11). Public `include_paths` may remain false (paths not emitted as `Element::Path`). |
| `is_page_recoverable(e, mode)` | `mode == Recoverable` **and** `e` is not `LimitExceeded` / `Encryption` / `Io` (those stay hard). Maps to `ContentTruncated`, `PageSkipped`, etc. |
| `ExtractedPage::empty_placeholder` | Same `index`, `media_box`/`crop_box`/`rotate`/`user_unit` if known; `elements = []`; optional per-page warning; **index preserved** (never omit middle pages when `allow_partial_document`). |
| `build_extracted_page` | Packs frozen fields; attaches `experimental.tables` only if detect_tables; strips internal `rule_segments` from public IR unless `include_paths`. |
| `effective_apply_page_rotate_for_layout()` | See D3 per-API defaults (layout/text: default true). |

**Partial document shape (normative):** when `allow_partial_document=true` and a page fails after a hard limit or recoverable content error, **keep page index with empty elements** (placeholder). Do **not** renumber. Set document `partial: true`. When `allow_partial_document=false` (default), any `LimitExceeded` aborts the whole `extract()` call with `Err`.

## A5. Error model, warning model, partial recovery

### Hard errors (`Error`)

Normative enum from product design—implement as-is:

| Variant | When |
|---------|------|
| `Io` | Unreadable path/reader |
| `Syntax` | Unrecoverable structure (no trailer/startxref) |
| `LimitExceeded { kind }` | **Always hard**, even in Recoverable (K11) |
| `Encryption` | Any `/Encrypt` in 0.1 (K15) |
| `Unsupported` | Explicitly unsupported product feature when configured strict |
| `PageOutOfRange` | Bad page index |
| `Backend` | Backend adapter failure |
| `Internal` | Bug; should be rare; prefer not panic |

### Warnings (`ExtractWarning` + `WarningCode`)

Warnings are **structured** for metrics (`warnings{code}`). Full taxonomy includes product codes plus extraction-specific extensions (additive = non-breaking):

```rust
pub enum WarningCode {
    // Product (0.1 freeze base)
    ObjectSkipped,
    ContentTruncated,
    UnknownOperator,
    MissingToUnicode,
    MissingFontWidths,
    LowMetricsConfidence,
    UnsupportedFilter,
    UnsupportedPaint,
    UnsupportedColorspace,
    ImageSkipped,
    FormNestingLimit,
    FormCycle,
    AnnotationSkipped,
    FieldSkipped,
    XfaIgnored,
    OcgUnknown,
    StructureBroken,
    ActualTextEmpty,
    InlineImageSkipped,
    PageSkipped,
    VerticalMetricsStub,
    // Volume-2 additive (still non-breaking if added carefully before 0.1 freeze)
    HiddenTextLayer,
    SoftMaskIgnored,
    PatternSkipped,
    ShadingSkipped,
    TableLowConfidence,
    TableAmbiguous,
    NestedTableFlattened,
    MultiPageTableSplit,
    RotatedTableBestEffort,
    ImageTableSkipped,
    JsActionPresent,
    SignatureDictPresent,
    EmbeddedFileSkipped,
    DecodeArrayNonDefault,
    IndexedColorspaceMetaOnly,
    IccProfileNotApplied,
    ArtifactContent,
    RoleMapMissing,
    ReadingOrderFallbackPaint,
}
```

### Recovery matrix (summary)

| Class | Strict | Recoverable (default) |
|-------|--------|------------------------|
| Limits / time budget | Hard | **Hard** |
| Encryption | Hard | Hard |
| Object syntax | Hard | Skip + `ObjectSkipped` |
| Content syntax mid-page | Hard | Truncate + `ContentTruncated` |
| Unknown op | Hard if `strict_ops` | Warn + continue |
| Missing font/ToUnicode | Optional hard | U+FFFD / empty + warnings |
| Unsupported filter | Hard | Skip stream use + `UnsupportedFilter` |
| Form cycle | Hard | Skip + `FormCycle` |

**`allow_partial_document` (default false):** only when true may multi-page `extract` return `partial: true` after a limit trip on a later page. Single-page APIs still return `Err(LimitExceeded)` for that call.

## A6. Resource Governor and security boundaries

### Governor role

```rust
pub struct ResourceGovernor {
    limits: ResourceLimits, // clamped to hard_max
    // atomics under feature "parallel"
    expanded_total: AtomicU64,
    // per-decode ticket RAII
}

impl ResourceGovernor {
    pub fn begin_stream_decode(&self, encoded_len: u64) -> Result<DecodeTicket, Error>;
    pub fn charge_expanded(&self, ticket: &DecodeTicket, n: u64) -> Result<(), Error>;
    pub fn charge_ops(&self, page: PageOpCounter, n: u32) -> Result<(), Error>;
    pub fn check_image_pixels(&self, w: u64, h: u64) -> Result<(), Error>;
    pub fn check_time(&self, clock: &BudgetClock) -> Result<(), Error>;
}
```

**Security boundaries:**

1. **Trust boundary A** — file bytes enter only via backend; no automatic decode.
2. **Trust boundary B** — all expansion through `filters::decode_stream` + governor.
3. **Trust boundary C** — content VM: op caps, form depth, visited sets.
4. **Trust boundary D** — font parsers: size-capped pure Rust; fuzz targets.
5. **Trust boundary E** — XFA/XML, embedded files: size limits; never execute.
6. **Process isolation** — multi-tenant services must still cgroup/sandbox (product K30).

### Default vs hard_max

Use product constants exactly (`ResourceLimits::default` and `hard_max::*`) for all product-defined fields. OpenOptions cannot exceed hard_max in release.

### Volume-2 additive limits (normative — Issue 8)

Product `ResourceLimits` / `LimitKind` do not yet name these; **implement in core** and recommend a small product-doc additive sync later. Map trips to structured errors:

```rust
// Additive fields on ResourceLimits (defaults / hard_max)
pub struct ResourceLimits {
    // ... all product fields ...
    /// Max objects parsed from a single ObjStm (N).
    pub max_objects_per_objstm: u32,      // default 8_192; hard_max 65_536
    /// Max path segments retained per page (for paths IR + lattice rules).
    pub max_path_segments_per_page: u32,  // default 500_000; hard_max 5_000_000
    /// Max bytes for any single PDF string / annot Contents / field V dump.
    pub max_string_bytes: u64,            // default 1_048_576; hard_max 16_777_216
    /// Max inline image payload bytes (BI/ID/EI).
    pub max_inline_image_bytes: u64,      // default 64 MiB; hard_max = MAX_EXPANDED_STREAM_BYTES
}

// LimitKind additives (or map as commented):
// ObjStmObjectCount → ObjectCount
// PathSegments → ContentOps or new PathSegments
// StringBytes → new StringBytes
// InlineImageBytes → StreamExpandedBytes
```

| Limit | Default | hard_max | On exceed |
|-------|---------|----------|-----------|
| `max_objects_per_objstm` | 8192 | 65536 | `LimitExceeded` (ObjectCount) |
| `max_path_segments_per_page` | 500_000 | 5_000_000 | `LimitExceeded` (PathSegments) |
| `max_string_bytes` | 1 MiB | 16 MiB | truncate with warning **or** LimitExceeded if configured strict strings; **0.1 default: truncate + warning** for annot/field strings; hard error if single object string > hard_max |
| `max_inline_image_bytes` | 64 MiB | stream expanded hard_max | `LimitExceeded` (StreamExpandedBytes) |

EX15 string caps bind to `max_string_bytes`. Table CPU: lattice/stream also respect `max_page_interpret_time` and path segment caps.

## A7. Memory lifecycle and laziness

| Policy | 0.1 default | Notes |
|--------|-------------|-------|
| Lazy pages | `OpenOptions::lazy_pages = true` | Page tree nodes resolved on demand |
| Decode cache | Off | Ephemeral decoded content per interpret |
| Page IR cache | Off unless `cache_page_ir` | Key: `(page_index, options_fingerprint)` — **fingerprint defined below** |
| Image pixels | Not decoded unless `decode_pixels` | Encoded/Deferred hold bytes or obj ref (K23) |
| Form XObjects | Inlined into page IR | Nested IR flattened; no retained form tree by default |
| Drop after extract | Caller owns `Vec<Element>` | Document retains encoded structure only |
| Rule segments | Ephemeral with interpret | Internal buffer for lattice; not cached separately from IR |

### IR cache fingerprint (normative — Issue 2)

When `OpenOptions::cache_page_ir` is true, cache key is:

```text
fingerprint = hash(
  page_index,
  include_paths,                    // public Path elements
  capture_rule_segments,            // internal lattice feed (true if detect_tables∩lattice/hybrid)
  detect_tables,
  table_modes bits,
  text.* (insert_spaces, sort_reading_order, unknown_glyph, preserve_positions),
  images.decode_pixels,
  images.include_inline_images,
  apply_page_rotate,                // effective value for this call
  include_structure,
  visible_ocg_only,
  mark_form_boundaries,
  // layout y_tolerance / min_table_confidence when layout or tables requested
)
```

**Cache miss / re-interpret rules:**

1. `Page::elements(opts)` with `include_paths=false` and tables off → IR without paths/segments.
2. Later `Page::tables()` or `layout(detect_tables=true)` with lattice/hybrid → **must not** reuse a path-less cached IR. Either cache miss (different fingerprint) or dedicated interpret with `capture_rule_segments=true`.
3. `Page::tables` / layout **always** request rule segments internally when lattice or hybrid is in `table_modes`, even if public IR omits `Element::Path` (`include_paths=false`).
4. Internal-only segments are cheap axis-aligned polylines (see C4); full Bézier retention only when `include_paths=true`.

```text
══════════════════════════════════════════════════════════════════════════════
                    DOCUMENT / PAGE LIFECYCLE (state machine)
══════════════════════════════════════════════════════════════════════════════

                    ┌─────────┐
                    │ Closed  │  (no Document)
                    └────┬────┘
                         │  open()  →  trailer + xref in memory
                         ▼
                    ┌─────────────┐
                    │  OpenMeta   │  metadata, page count, catalog
                    └──────┬──────┘
                           │  page(i)
                           ▼
                    ┌─────────────┐
                    │  PageBound  │  page boxes, resources resolved
                    └──────┬──────┘
                           │  elements() / text() / tables()
                           ▼
                    ┌──────────────┐
          ┌────────│ Interpreting │────────┐
          │        └──────┬───────┘        │
          │               │ IR built       │ re-extract
          │               ▼                │ (cache miss or
          │        ┌────────────┐          │  fingerprint change)
          │        │  IRReady   │──────────┘
          │        └──────┬─────┘
          │               │  cache_page_ir = true
          │               ▼
          │        ┌────────────┐
          │        │   Cached   │── hit ──▶ return IR without re-interpret
          │        └────────────┘
          │
          └──── drop Document ──▶ free backend, caches, governor ──▶ Closed

  Cache fingerprint includes: include_paths, capture_rule_segments,
  table_modes, apply_page_rotate, text options that affect IR.
  Path-less IR must NOT be reused when lattice/hybrid tables need rule segments.
```

**RSS targets (product SLOs):** 300-page digital report page-at-a-time text < 256 MB peak lazy; 1000-page metadata open < 128 MB typical.

---

# Part B — Low-Level Design of Core Pipeline

## B1. File open / trailer / xref

### Inputs / outputs

| | |
|--|--|
| **In** | `Read+Seek` / bytes / path; `OpenOptions` |
| **Out** | Backend handle, trailer `PdfDict`, xref index, `pdf_version`, encryption gate result |
| **Errors** | Io, Syntax (no `startxref`/trailer), Encryption, LimitExceeded (file size) |

### Algorithm — open

```text
1. Measure file length (seek end); if > limits.max_file_bytes → LimitExceeded(FileSize)
2. Read header from start: `%PDF-x.y` (tolerate whitespace/junk prefixes common in wild within first 1 KiB)
3. Locate startxref: search last 1–2 KiB for `startxref` → offset
4. Backend builds xref from that offset (table and/or stream); follow /Prev for incremental updates
5. Merge xref generations: **highest generation wins** for each obj number; free entries tracked
6. Trailer = dict associated with newest xref section; require /Root
7. If trailer has /Encrypt (non-null) → Error::Encryption (0.1; do not open empty password)
8. Store trailer, version, governor; do not load all objects
```

### Xref kinds

| Kind | Detection | Handling |
|------|-----------|----------|
| Classic table | `xref` keyword + subsections | Backend maps obj → offset |
| XRef stream | trailer `/Type /XRef` stream | Parse W array, Index; entries Free/Uncompressed/Compressed |
| Hybrid | Classic table **and** trailer `/XRefStm` | **Normative merge below** |
| Incremental | `/Prev` chain | Walk Prev; later updates override |
| Linearized | `/Linearized` dict | **Open without hint tables** (0.1); treat as normal xref |

### Hybrid + incremental merge algorithm (normative — Issue 9)

```text
1. Collect trailer chain: start at primary trailer; follow /Prev offsets until none or depth cap.
   Order chain as [newest, …, oldest].

2. Initialize map: obj_num → XRefEntry (empty).

3. For each trailer T in chain from **oldest to newest** (so newer overwrites):
   a. If T has a classic xref table section:
        for each entry in table: map[obj] = table_entry
   b. If T has /XRefStm (hybrid) or is itself an XRef stream trailer:
        parse xref stream entries;
        for each entry: map[obj] = stream_entry
          // **XRefStm / stream entries override classic table** for the same obj_num
          // within the same trailer generation (PDF hybrid practice).

4. Within a single classic subsection, higher generation in the table row wins for that obj
   (standard xref). Across incremental updates, the **newest trailer that mentions obj wins**
   entirely (not field-wise merge)—step 3 order ensures this.

5. Free entries: store as Free; resolving get(id) → NotFound.

6. Fixtures required: hybrid where table and XRefStm disagree on same obj;
   incremental Prev with hybrid child; free-then-reused object numbers.
```

### Key types

**Product trait surface (ObjectBackend — matches product rev 2.2 exactly):**

```rust
// Exposed on ObjectBackend / adapter boundary — product-normative
pub enum XRefEntry {
    Free,
    Uncompressed { offset: u64 },
    Compressed { stream_id: ObjectId, index: u32 },
}
```

**Core-internal richer form (Volume-2 additive; adapter maps to/from trait):**

```rust
// pdfparser-core only — not the trait return type
pub struct XRefEntryFull {
    pub kind: XRefEntry,       // product enum
    pub generation: u16,       // from xref row; 0 for compressed
    pub next_free: Option<u32>, // when Free
}
```

Implementors of `ObjectBackend::xref_entry` return the **product** `XRefEntry`. Core may retain `generation` in its own xref index built during open if the backend exposes it via an optional extension method `xref_entry_full` (not required for 0.1 extract). **Do not** treat Volume 2’s older Free/Uncompressed-with-gen enum as the trait contract.

```rust
pub struct TrailerInfo {
    pub root: ObjectId,
    pub encrypt: Option<ObjectId>, // if Some → Encryption in 0.1
    pub info: Option<ObjectId>,
    pub id: Option<[PdfString; 2]>,
    pub prev: Option<u64>,
    pub xrefstm: Option<u64>,
}
```

### Failure modes

- Truncated xref → Syntax or Recoverable skip with incomplete object map
- Wrong generation → NotFound when resolving
- Cyclic Prev → depth-limited Prev walk (`max_nesting_depth`)

### Limits / tests

- File size, object count from xref_len
- Fixtures: classic, xref stream, hybrid, 3× incremental update, linearized, missing startxref, encrypted trailer

---

## B2. Object resolution (including object streams)

### Inputs / outputs

| | |
|--|--|
| **In** | `ObjectId`, backend, governor |
| **Out** | `PdfObject` (streams still **encoded** until explicit decode) |
| **Cache** | `ObjectStore` in core (optional memo of resolved objects) |

### Algorithm — `ObjectStore::get`

```text
1. If cache hit → return clone/arc of PdfObject
2. entry = backend.xref_entry(id)
3. match entry:
   Free → NotFound
   Uncompressed { offset } → backend.get_encoded(id)  // or parse at offset
   Compressed { stream_id, index } → resolve_objstm(stream_id, index)
4. If result is Ref → resolve recursively with visited set (depth ≤ max_nesting_depth)
5. Cache and return
```

### Algorithm — ObjStm (normative, product K16/K32)

```text
1. container = backend.get_encoded(stream_id) → Stream(EncodedStream)
2. decoded = filters::decode_stream(gov, &container)  // counts expansion
3. N = dict.N; First = dict.First; optional Extends
4. Parse header region decoded[0..First] as N pairs (obj_num, relative_offset)
5. Enforce: N ≤ limits.max_objects_per_objstm
   (default 8192; hard_max 65_536) else LimitExceeded { ObjectCount }
   // maps to LimitKind::ObjectCount (or additive LimitKind::ObjStmObjectCount — see limits section)
6. For requested index: abs = First + relative_offset[index]
7. Parse one PDF object from decoded[abs..] with size bound
8. If child is Stream, bytes are **encoded** child payload
9. Extends: if present and object missing, recurse Extends stream
   (depth ≤ max_nesting_depth; each Extends decode charges governor)
```

```rust
pub fn parse_objstm(
    gov: &ResourceGovernor,
    decoded: &[u8],
    dict: &PdfDict,
) -> Result<Vec<(u32 /*obj_num*/, PdfObject)>, ObjStmError>;
```

### Failure modes

- Index OOB → NotFound / ObjectSkipped
- Nested bomb via huge ObjStm → LimitExceeded
- Circular Extends → FormNesting-like depth error

### Tests

- Single ObjStm with mixed types; nested stream child; malicious N; Extends chain; decode-guard that backend never returns decompressed ObjStm children

---

## B3. Stream filter chain

### Inputs / outputs

| | |
|--|--|
| **In** | `EncodedStream { dict, encoded_bytes }`, governor |
| **Out** | `DecodedBytes(Bytes)` |
| **Never** | Decode Crypt ciphertext as content in 0.1 (encrypted files never open) |

### Algorithm — `decode_stream`

```text
1. filters = normalize /Filter: Name → [Name]; Array → list; missing → identity
2. if filters.len() > max_filter_chain_len → LimitExceeded(FilterChainLength)
3. parms = normalize /DecodeParms to per-filter dicts (nulls allowed)
4. cur = encoded_bytes
5. for (i, filter) in filters:
     a. estimate / enforce expansion: peak ≤ max_expanded_stream_bytes;
        total ≤ max_total_expanded_bytes;
        if expanded > encoded * max_filter_expansion_ratio → LimitExceeded
     b. cur = apply_filter(filter, cur, parms[i], gov)
     c. if filter in {FlateDecode, LZWDecode} and Predictor in parms:
          cur = apply_predictor(cur, parms, columns, colors, bpc)
6. return DecodedBytes(cur)
```

### Filter support matrix

| Filter | Phase | Behavior |
|--------|-------|----------|
| FlateDecode | **0.1 Full** | zlib/deflate; then Predictor |
| ASCIIHexDecode | **0.1 Full** | |
| ASCII85Decode | **0.1 Full** | |
| LZWDecode | **0.1 Full** | EarlyChange param; Predictor |
| RunLengthDecode | **0.1 Full** | |
| DCTDecode | **0.1** | Pass-through for extract; optional JPEG decode if `decode_pixels` |
| JPXDecode | 0.1 best-effort | Usually `UnsupportedFilter` + skip |
| CCITTFaxDecode | **0.2** | Pure Rust preferred |
| JBIG2Decode | optional feature off | FFI isolated |
| Crypt | post-0.1 only | Never in 0.1 open path |

### Predictor (1–15)

```text
PNG predictors 10–15: row filters None/Sub/Up/Average/Paeth/Optimum
TIFF predictor 2: horizontal differencing
Requires: Columns, Colors, BitsPerComponent from DecodeParms (defaults 1, 1, 8)

Row geometry (normative when height unknown — Issue 20):
  bits_per_pixel = Colors * BitsPerComponent
  row_size = ceil(Columns * bits_per_pixel / 8)          // sample bytes per row
  png_row_stride = row_size + 1                          // predictor 10–15: filter byte per row
  For predictor 1 (None/no-op): output = input
  For predictor 2 (TIFF): 
      if input_len % row_size != 0 → fail decode (warn + skip stream use in Recoverable)
      rows = input_len / row_size
  For predictor 10–15:
      if input_len % png_row_stride != 0 → fail decode (same policy)
      rows = input_len / png_row_stride
  Output size = rows * row_size
  If image Height H is known from stream dict and rows != H → fail / skip (inconsistent)
  Charge governor for output size before allocating
```

### Failure modes

- Truncated Flate → error or partial per Recoverable policy on that stream
- Predictor Columns=0 → treat as invalid; skip stream use
- Chain Flate+Flate bombs → ratio + absolute caps

### Tests

- Each filter golden; Predictor 1–15 fixtures; bomb corpus; chain length 9; ratio trip; integration decode-guard

---

## B4. Page tree & resource inheritance

### Algorithm — enumerate pages

```text
1. catalog = get(trailer.Root)
2. pages_root = get(catalog.Pages)
3. DFS Kids:
     if Type==Pages or has Kids: inherit push Resources/MediaBox/CropBox/Rotate/UserUnit; recurse
     if Type==Page or has Contents: materialize PageNode with merged inheritance
4. Cap page count at limits.max_pages
5. Detect cycles via visited ObjectId set
```

### Inheritance fields

| Key | Inherit? | Default |
|-----|----------|---------|
| Resources | yes | empty |
| MediaBox | yes | required ultimately |
| CropBox | yes | = MediaBox |
| Rotate | yes | 0 |
| UserUnit | yes | 1.0 |

### Resource merge

Page Resources may contain ExtGState, ColorSpace, Pattern, Shading, XObject, Font, Properties. **Names shadow** from page over parents. Form XObjects carry own Resources; content VM merges form resources for duration of `Do` (see C5).

### Types

```rust
pub struct PageNode {
    pub id: ObjectId,
    pub index: u32,
    pub media_box: Rect,
    pub crop_box: Rect,
    pub rotate: i32, // 0,90,180,270
    pub user_unit: f32,
    pub resources: ResourceDict,
    pub contents: Vec<ObjectId>, // or inline stream refs
    pub annots: Vec<ObjectId>,
    pub group: Option<ObjectId>, // transparency group
}
```

### Tests

- Deep Kids; missing MediaBox on leaf but present parent; Rotate 90; resource name collision parent/child

---

## B5. Content stream tokenization & operator dispatch

### Inputs / outputs

| | |
|--|--|
| **In** | Decoded content bytes (concat multi-stream Contents in order) |
| **Out** | Stream of `(operands, Operator)` → side effects on GState + IR |

### Tokenizer rules

```text
1. Skip whitespace and comments (`%` to EOL)
2. Tokens: numbers, names (/...), literal strings (...), hex strings <...>, arrays [], dicts <<>>, booleans, null
3. Operator = keyword (sequence of alphabetic chars / `'` / `"` / `*`) after its operands on the operand stack
4. BI starts inline-image mode (normative EI algorithm below)
```

### P1 operator arity table (normative — Issue 5)

Operands are consumed from a stack (push numbers/names/strings/arrays/dicts; operator pops arity).

| Operator | Arity | Notes |
|----------|------:|-------|
| `q` `Q` | 0 | |
| `cm` | 6 | a b c d e f |
| `m` `l` | 2 | |
| `c` | 6 | |
| `v` `y` | 4 | |
| `h` | 0 | |
| `re` | 4 | |
| `S` `s` `f` `F` `f*` `B` `B*` `b` `b*` `n` | 0 | |
| `W` `W*` | 0 | |
| `Tc` `Tw` `Tz` `TL` `Ts` | 1 | |
| `Tf` | 2 | font name + size |
| `Tr` | 1 | |
| `BT` `ET` | 0 | |
| `Td` `TD` | 2 | |
| `Tm` | 6 | |
| `T*` | 0 | |
| `Tj` | 1 | string |
| `TJ` | 1 | array |
| `'` | 1 | string |
| `"` | 3 | aw ac string |
| `Do` | 1 | name |
| `gs` | 1 | name |
| `g` `G` `i` | 1 | |
| `rg` `RG` | 3 | |
| `k` `K` | 4 | |
| `cs` `CS` | 1 | name |
| `sc` `SC` | **variadic** | 1..N based on current color space; 0.1: pop until stack empty or next would be operator—**prefer** pop `n_components` if known from CS, else pop **all** operands currently on stack as color args (documented recovery) |
| `scn` `SCN` | **variadic** | same as sc; may include trailing name (pattern) |
| `BMC` | 1 | tag name |
| `BDC` | 2 | tag + properties (name or dict) |
| `EMC` | 0 | |
| `MP` | 1 | |
| `DP` | 2 | |
| `BI` | special | inline image mode; not stack arity |
| `ID` `EI` | special | only valid inside inline image |
| `sh` | 1 | P2 |
| `ri` | 1 | P2 |
| `d` | 2 | dash array + phase (P2) |
| `J` `j` `M` `w` | 1 | P2 |

Cross-link product **Appendix A** operator priority (P1 must / P2 / P3).

### Unknown-operator recovery (normative — Issue 5)

```text
1. If keyword is not in the known operator table:
   a. Emit WarningCode::UnknownOperator (include keyword in message; do not log operand content at info).
   b. **Clear the entire operand stack** (do not guess arity).
   c. Continue with next token. Never invent pop counts for unknown names.
2. If keyword is known but stack has fewer operands than arity:
   a. Recoverable: warn, clear stack, skip operator (treat as no-op).
   b. Strict (`strict_ops` / ParseMode::Strict): hard Error::Syntax.
3. If stack has extra operands before an operator:
   PDF allows only the operator's operands; **extra left on stack** until next operator.
   Unknown-op clear (step 1b) discards leftovers—prevents cascade corruption after unknowns.
4. Never execute unknown operators as `Do`/`gs` lookalikes.
```

### Inline image `BI` / `ID` / `EI` locator (normative — Issue 6, 0.1 Full)

```text
1. On BI:
   a. Read key/value pairs (abbreviated keys allowed: /W /H /CS /BPC /F /DP /D /IM /Intent …)
      until token ID. Keys/values use same tokenizer as content (names, numbers, arrays, dicts).
   b. Build InlineImageDict; expand abbreviations to full names.

2. Compute expected payload length when possible:
   a. If dict has /L (some producers) → expected_len = L.
   b. Else if Width W, Height H, BitsPerComponent BPC, and color components C known from /CS
      (DeviceGray=1, RGB=3, CMYK=4, Indexed=1, ICCBased N from ICC stream or /N):
        bits = W * H * C * BPC
        expected_len = ceil(bits / 8)
      For ImageMask /IM true: C=1, BPC treated as 1.
   c. Else expected_len = unknown.

3. After ID, optional single whitespace byte consumed per ISO (whitespace following ID).

4. Locate end of data (EI):
   Case A — expected_len known:
     Take exactly expected_len bytes as payload (bounded by remaining stream and
     limits.max_inline_image_bytes, default = min(max_expanded_stream_bytes, 64 MiB)).
     Then skip whitespace; require token EI. If not EI → warn InlineImageSkipped;
     attempt Case B resync from current pos with max bound.
   Case B — expected_len unknown:
     Scan forward for a candidate end:
       - Find byte sequence where: (whitespace or start) + `EI` + (whitespace or delimiter
         that begins a valid following token / operator).
       - Candidate EI must not be accepted inside a longer name token.
       - Max scan = min(remaining, limits.max_inline_image_bytes).
     On first valid candidate: payload = bytes before EI; consume EI.
     If no candidate within bound → InlineImageSkipped; resync: skip to next whitespace-delimited
     token that looks like a known operator or `Q`/`EMC`/`Do` (best-effort); charge ops for skips.

5. Governor:
   - Charge max(payload_len, expected_len) against expanded/page image budgets before copy.
   - Pixel budget: W*H checked like XObject images when W,H present.
   - max_inline_image_bytes trip → LimitExceeded { StreamExpandedBytes } or ImagePixels as appropriate.

6. Filters on inline dict `/F`: apply owned filter pipeline to payload only when decode_pixels
   or when needed for non-extract paths—default store Embedded encoded (K23).

7. Fixtures: (a) EI bytes inside DCT/JPEG payload with known W,H; (b) unknown length Flate-less raw;
   (c) missing EI truncated; (d) abbreviations only.
```

### Dispatch table (P1 subset)

| Ops | Handler |
|-----|---------|
| `q Q cm` | gstack push/pop; CTM multiply (B6 math) |
| path `m l c v y h re` | path builder |
| paint `S s f f* B B* n` | emit PathElement if include_paths; append rule segments if capture_rule_segments; clear path |
| clip `W W*` | clip stack intersect |
| text state `Tc Tw Tz TL Tf Tr Ts` | TextState fields |
| text `BT ET Td TD Tm T* Tj TJ ' "` | text showing pipeline |
| `Do` | XObject image or form |
| `BI ID EI` | inline image (algorithm above) |
| `gs` | merge ExtGState (C11 key list) |
| colors `g rg k G RG K cs CS sc SC scn SCN` | store Color in state |
| marked `BMC BDC EMC MP DP` | marked content stack |

### Op accounting

Every dispatched operator: `gov.charge_ops(1)`; at limit → `LimitExceeded(ContentOps)`. Path segment appends also charge toward `max_path_segments_per_page`.

### Tests

- Lexer fuzz; golden operator streams; unknown ops **clear stack** (fixture: unknown then `Tj` must still work)
- BI/ID/EI with embedded `EI` ASCII inside binary with known dimensions
- Arity underflow in Strict vs Recoverable

---

## B6. Graphics state machine

### State structures

```rust
pub struct GraphicsState {
    pub ctm: Matrix3x2,
    pub clip_stack: Vec<ClipPath>,
    pub fill_color: Color,
    pub stroke_color: Color,
    pub line_width: f32,
    pub line_cap: u8,
    pub line_join: u8,
    pub miter_limit: f32,
    pub dash: DashPattern,
    pub render_intent: Option<String>,
    pub stroke_adjust: bool,
    pub blend_mode: BlendMode, // record only
    pub alpha_fill: f32,
    pub alpha_stroke: f32,
    pub soft_mask: Option<SoftMaskRef>,
    pub overprint_fill: bool,
    pub overprint_stroke: bool,
    pub text: TextState,
    pub path: PathBuilder,
}

pub struct TextState {
    pub char_spacing: f32,      // Tc
    pub word_spacing: f32,      // Tw
    pub horizontal_scale: f32,  // Tz raw percent, default 100
    pub leading: f32,           // TL
    pub font: Option<FontRef>,
    pub font_size: f32,         // Tf size
    pub render_mode: u8,        // Tr
    pub rise: f32,              // Ts
    pub text_matrix: Matrix3x2, // Tm
    pub text_line_matrix: Matrix3x2,
}

pub struct GStateStack {
    stack: Vec<GraphicsState>, // q/Q; depth ≤ max_nesting_depth
}
```

### CTM / text matrix (normative math — Issue 7)

**Storage:** matrices are six `f32`s `[a, b, c, d, e, f]` meaning:

```text
| a  b  0 |
| c  d  0 |
| e  f  1 |
```

**Points are row vectors** on the left: `p' = p × M` with `p = [x, y, 1]`:

```text
x' = a*x + c*y + e
y' = b*x + d*y + f
```

**Multiply** `M = A × B` (apply B first, then A—standard PDF concatenation for `cm`):

```text
// cm: CTM' = M_cm × CTM_old   (new matrix left-multiplies)
a' = a_cm*a_old + b_cm*c_old
b' = a_cm*b_old + b_cm*d_old
c' = c_cm*a_old + d_cm*c_old
d' = c_cm*b_old + d_cm*d_old
e' = e_cm*a_old + f_cm*c_old + e_old
f' = e_cm*b_old + f_cm*d_old + f_old
```

```text
cm a b c d e f:  CTM = Matrix(a,b,c,d,e,f) × CTM
Tm a b c d e f:  text_matrix = M; text_line_matrix = M
Td tx ty:        Tlm = [1,0,0,1,tx,ty] × Tlm;  Tm = Tlm
TD tx ty:        leading = -ty; then Td(tx, ty)
T*:              Td(0, -leading)
```

**Glyph user-space transform:** for a point in text space, `p_user = p_text × Tm × CTM`  
(equivalent to `p_text × (Tm × CTM)` with multiply as defined above).  
`TextRun.transform` export = six coefficients of `Tm × CTM` at run anchor (start of run).

**Worked golden example (normative fixture `matrix_glyph_1`):**

```text
Initial CTM = I = [1,0,0,1,0,0]
cm 2 0 0 2 10 20     → CTM = [2,0,0,2,10,20]
BT
Tm 1 0 0 1 5 5       → Tm = [1,0,0,1,5,5]
Tf /F1 10              // font with space width 500 units for 'A' width 600
// Show one glyph 'A' at text origin (0,0), advance 6.0 text units:
//   width=600 → adv = (600/1000)*10*(100/100) = 6.0  (Th=100, Tc=0, Tw=0)
// Glyph quad text-space (before Tm): (0, descent)..(6, ascent)
// Using ascent=800, descent=-200 design → y from -2 to 8 at fs=10
// Corner (0,0) text → user:
//   via Tm: (5,5); via CTM: (2*5+10, 2*5+20) = (20, 30)
// Corner (6,8) text → Tm: (11,13); CTM: (2*11+10, 2*13+20) = (32, 46)
// Axis-aligned bbox user: x0=20, y0≈26 (descent corner), x1=32, y1=46
// (exact descent corner: (0,-2)→Tm(5,3)→CTM(20,26))
```

Implementations must match this example within golden `f32` 3-decimal rounding (K19). Aligns with product font-positioning section (advance formula unchanged).

**Coordinate export:** glyph quads → axis-aligned bbox in **unrotated page user space** (MediaBox origin). `apply_page_rotate` per D3 for upright layout/text APIs only.

### Clip stack

`W`/`W*` + paint `n` (or with fill): intersect current path with clip. For lattice tables, **retain clip geometry only if needed**; path capture stores pre-clip path when `include_paths`.

### Colors

Store as enum; **no CMS** in 0.1:

```rust
pub enum Color {
    Gray(f32),
    Rgb([f32; 3]),
    Cmyk([f32; 4]),
    Pattern(String),
    Separation { name: String, alt: Box<Color> },
    IccBased { n: u8, values: Vec<f32> },
    Unparsed,
}
```

### Failure modes

- Unbalanced `Q` → warn, ignore
- `q` depth > limit → LimitExceeded(NestingDepth)
- Missing font on Tj → skip show + warning

---

## B7. Font loading & metrics

### Pipeline

```text
══════════════════════════════════════════════════════════════════════════════
                         FONT LOADING & METRICS PIPELINE
══════════════════════════════════════════════════════════════════════════════

  Font dict (page Resources)
        │
        │  core extracts /FontFile* streams (encoded)
        ▼
  governor decode  ──▶  program bytes  (+ widths / ToUnicode / CMap streams)
        │
        ▼
  FontInput  (plain data; no ObjectStore pointers)
        │
        │  pdfparser_fonts::load_font(input)
        ▼
  LoadedFont
        │
        ├──▶  width_for_code / CID     →  glyph advances → bboxes
        └──▶  unicode_for_code / CID   →  string text (+ ActualText override)

  Rule: fonts never import core; core always decodes, then push-bytes-in.
```

### FontInput (normative sketch — product aligned + Volume-2 additive)

Product rev 2.2 `FontInput` sketch is the base. **Volume-2 additive field:** `wmode: u8` (and optional `base_font`) — recommend product doc sync. Not a product conflict; pure additive.

```rust
pub struct FontInput<'a> {
    pub subtype: FontSubtype,
    pub base_font: Option<String>,           // Volume-2 additive convenience
    pub descriptor: FontDescriptorData,
    pub widths_simple: Option<SimpleWidths>,
    pub cid_widths: Option<CidWidthArray>,
    pub to_unicode_cmap_bytes: Option<&'a [u8]>,
    pub encoding_name: Option<String>,
    pub differences: Option<Vec<(u8, String)>>,
    pub embedded_program: Option<&'a [u8]>,
    pub descendant: Option<Box<FontInput<'a>>>,
    pub cmap_name_or_bytes: Option<CMapSource<'a>>,
    /// Volume-2 additive (not in product sketch): 0 = horizontal, 1 = vertical.
    /// Derived by core from CIDFont /WMode or font dict before load_font.
    pub wmode: u8,
}

pub struct LoadedFont {
    pub name: String,
    pub metrics: FontMetrics,
    pub mapper: UnicodeMapper,
    pub mapping_confidence_base: f32,
    pub metrics_confidence_base: f32,
    pub wmode: u8,
}
```

### Width sources (product table — implement exactly)

| Font | Widths | Fallback |
|------|--------|----------|
| Type1 / TrueType simple | FirstChar/LastChar/Widths | hmtx/Type1; MissingWidth; 0 |
| Type0 CIDFontType2 | W + DW | DW=1000 |
| Type0 CIDFontType0 | W + DW + CFF | DW |
| Type3 | Widths / charproc bbox | FontBBox |
| Vertical WMode=1 | W2/DW2 | stub + `VerticalMetricsStub` |

### Encodings

| Encoding | Phase | Notes |
|----------|-------|-------|
| WinAnsiEncoding | 0.1 | |
| MacRomanEncoding | 0.1 | |
| StandardEncoding | 0.1 | |
| MacExpertEncoding | 0.1 best-effort | |
| Differences | 0.1 | Name → Unicode via Adobe Glyph List subset |
| Identity-H / Identity-V | 0.1 | CID = code; need ToUnicode for text |
| Embedded CMap | 0.1 after license PR | |
| Predefined CJK CMaps | 0.1 after license | package TBD (T1 open) |

### ToUnicode / ActualText priority for **string content**

```text
1. If marked content ActualText covers glyphs → use ActualText (geometry still from paints)
2. Else ToUnicode CMap
3. Else encoding name + Differences + AGL
4. Else Identity for Latin single-byte heuristic (low confidence)
5. Else U+FFFD per TextOptions::unknown_glyph
```

### Advance formula (horizontal, normative)

```text
Th = TextState.horizontal_scale  // percent, default 100
advance_x = (width/1000) * font_size * (Th/100) + Tc + (is_space_for_tw ? Tw : 0)
TJ number: tx -= (number/1000) * font_size * (Th/100)
```

`is_space_for_tw`: product rules (code 0x20 / /space / Unicode Zs from mapping).

### Type3

0.1: widths + optional empty unicode; charprocs **not** required for text export. Optional later: interpret charprocs as paths for lattice.

### Vertical stub (0.1)

Emit runs with low `metrics_confidence`; no full vertical reading-order claim.

### Tests

- metrics_20 bbox smoke; missing Widths + hmtx; Identity-H + ToUnicode; Differences; Type3; WMode=1 fixture; font fuzz; size cap

---

## B8. IR construction & coordinate spaces

### Spaces (normative)

| Space | Use |
|-------|-----|
| Glyph space | Font design units |
| Text space | After font size + Tm |
| User space | After CTM; before page Rotate |
| Upright export | Optional rotate apply for layout/text |

**Numeric:** `f32`; goldens 3 decimal places (K19).

### Run coalescing rules

```text
Start new TextRun when any of:
  - font or font_size changes
  - fill/stroke/render_mode change (if tracked)
  - horizontal gap > split_threshold (optional; default coalesce within same show op only in 0.1 paint IR)
  - ActualText boundary
  - marked content MCID change
  - OCG change
Within a single Tj/TJ, emit one run (or split on large TJ kerns if export requests glyph-level later)
```

### Element emission order

Paint order by default in `elements()`. Reading order is layout-layer reordering when `sort_reading_order`.

### Types (frozen fields bold)

See Part D; paint IR builder pushes `Element::Text` / `Image` / optional `Path` / `FormBoundary`.

---

# Part C — Feature Extraction Deep Dives

---

## C1. Text extraction

### Problem definition

PDF stores **glyph painting**, not strings. Users want Unicode text, reading order, word/line structure, and **accurate bboxes**. Glyph IDs may lack ToUnicode; spacing may omit space characters; paint order ≠ reading order.

### In-scope variants / types

| Variant | Phase | Notes |
|---------|-------|-------|
| Simple fonts Type1/TrueType + WinAnsi/MacRoman/Standard | **0.1** | |
| Differences encodings | **0.1** | |
| Type0 / CID + Identity-H | **0.1** | |
| Identity-V / WMode=1 | **0.1 stub** | geometry best-effort |
| Embedded / predefined CMaps | **0.1** post-license | |
| ToUnicode CMaps | **0.1** | |
| ActualText spans | **0.1** | prefer for unicode |
| Tj / TJ / ' / " | **0.1** | |
| Tc Tw Tz/Th TL Tf Tr Ts | **0.1** | |
| Space insertion heuristic | **0.1** | layout |
| Multi-column reading order | **0.1 basic** | |
| Soft hyphen U+00AD | **0.1** | preserve or optional strip on plain text |
| Missing glyphs → U+FFFD | **0.1** | K26 |
| Invisible text Tr=3 | **0.1** | extract + flag |
| Type3 text | **0.1** geometry + fallback unicode | |
| Vertical full layout | **0.2** | |
| Complex scripts shaping | **later** | no HarfBuzz dependency in 0.1 |

### Out of scope

| Item | Phase |
|------|-------|
| OCR for image-only text | later / separate track |
| Full BiDi reordering to Unicode UAX#9 perfection | later (basic order OK) |
| Font subset repair / glyph outlines for text | non-goal for extract |

### Extraction algorithm (step-by-step)

#### C1.1 Content interpretation (paint-order runs)

```text
1. Enter BT: text object active; reset is not required beyond ISO (Tm independent)
2. On Tf: bind LoadedFont + size
3. On text positioning ops: update Tm / Tlm
4. On Tj(string):
   a. Decode string bytes per font (single-byte vs multi-byte CMap)
   b. For each code/CID:
      - width = font.width(code)
      - uni = mapper.map(code) or FFFD
      - apply advance with Tc/Tw/Th
      - compute glyph quad from ascent/descent + advance
      - transform quad by Tm then CTM → user space
      - accumulate into current TextRun builder
   c. Update Tm translation by total advance
5. On TJ(array):
   for item in array:
     if string: same as Tj fragment
     if number: Tm.tx -= (n/1000)*fs*(Th/100)  // horizontal
6. On '/": apply leading / word spacing per ISO; show string
7. On ET: flush run builder
8. Marked content: if ActualText property active, replace unicode of spanned runs; set from_actual_text=true; keep geometry
```

#### C1.2 Multi-byte string parsing (Type0)

```text
1. Use CMap codespace ranges (e.g. Identity-H: 2 bytes)
2. Consume bytes greedily per codespace
3. Map code → CID → Unicode via ToUnicode (CID-based)
```

#### C1.3 Space insertion (layout; `insert_spaces=true`)

```text
1. Group runs into baseline bands: |y_center_i - y_center_j| ≤ y_tolerance
   default y_tolerance = 0.25 * median_font_size among candidate runs
2. Sort runs in band by x0 ascending
3. median_space_width = median of font space widths in band (or 0.25 * median_font_size)
4. For adjacent pair (a,b):
     gap = b.x0 - a.x1
     if gap > α * median_space_width (α = 0.25) and no boundary whitespace:
        insert U+0020 between in plain-text assembly
5. If Tw already expanded spaces in geometry, avoid double-insert when glyphs include space
```

#### C1.4 Reading order (0.1 basic) — parameters + structure merge (Issues 13, 18)

**Defaults (normative for 0.1):**

| Parameter | Symbol | Default | Meaning |
|-----------|--------|---------|---------|
| Line band y tolerance | — | `0.25 * median_font_size` | Same as space heuristic band |
| Block vertical gap | β | **1.5** | New block if gap > β * median_line_height |
| Column x-overlap ratio | ρ | **0.15** | Lines in different columns if horizontal overlap IoU_x < ρ |
| Min column gap | g_col | **1.5 * median_space_width** | Median gap between right edge of left band and left edge of right band |
| Min column width | w_min | **3 * median_font_size** | Discard column bands narrower than this |
| Min lines per column | — | **2** | Else treat as single column |
| Writing direction | — | **LTR only in 0.1** | RTL/vertical order out of 0.1 bar |

```text
1. Exclude Artifact-marked runs from Page::text assembly by default (E-T4);
   keep them in raw elements() with artifact flag if tracked.
2. Cluster lines (baseline bands); sort lines by y descending (PDF y-up: higher y first).
3. Cluster lines into blocks: vertical gap between successive lines > β * median_line_height.
4. Multi-column detection within a page region:
   a. Project line bboxes onto x-axis; build histogram of x0 and x1 with bin = max(2.0, 0.25*median_font_size).
   b. Find vertical gutters: x ranges with near-zero line coverage width ≥ g_col.
   c. Split into column bands; each band width ≥ w_min; each band ≥ 2 lines.
   d. If ≥2 bands pass → multi-column; else single column.
5. Spanning headers: a line whose bbox.x0..x1 covers ≥ 80% of the union width of all columns
   and sits above column tops (y greater than all column first-lines) is a **spanning line**:
   emit it before column content (document order: spanning → col0 lines → col1 lines …).
6. Order: spanning lines top-to-bottom; then columns left-to-right; within column top-to-bottom.
7. Structure-prefer path (EX16 / E-T7 collapsed here — normative):
   a. coverage = (count of non-artifact text runs with mcid that appear in structure leaf order)
                 / (count of non-artifact text runs on page)
   b. If include_structure available AND coverage ≥ 0.80 AND structure walk succeeded:
        primary_order = structure depth-first leaf MCID order
        Append untagged non-artifact runs (no mcid or mcid not in tree) after tagged content,
        sorted by basic geometric order within the same page.
   c. Else: geometric order from steps 2–6; if structure was expected but coverage < 0.80,
        WarningCode::ReadingOrderFallbackPaint (or StructureBroken if tree corrupt).
8. Fallback if layout fails entirely: paint order + ReadingOrderFallbackPaint.
```

#### C1.4a `Page::text` assembly (normative for CER)

```text
1. Apply reading order (C1.4) when TextOptions.sort_reading_order == true (default true for text()).
2. Within a line: concatenate runs with space insertion (C1.3) if insert_spaces.
3. Between lines in the same block/column: insert U+000A (newline) in the String returned by Page::text.
4. Between blocks/columns: insert U+000A.
5. Oracle CER primary path (product metrics): NFKC; collapse Unicode whitespace to single U+0020;
   strip leading/trailing WS per line; **join lines with single spaces** for primary CER
   (secondary report keeps newlines). Implementers: keep newlines in Page::text; CER harness normalizes.
```

#### C1.5 Word / line / paragraph clustering

```text
Line: same baseline band, sorted x
Word: split line text on whitespace; word bbox = union of glyph boxes
Paragraph/block: vertical clustering with β above; optional indent detection (unstable)
```

#### C1.6 Soft hyphens

```text
If U+00AD present: keep in TextRun.text; plain Page::text may strip at line-break edges if end-of-line hyphenation detected (optional TextOptions later; 0.1 keep as-is in IR)
```

### Data model

```rust
// Frozen 0.1 fields marked *
pub struct TextRun {
    pub text: String,                      // *
    pub glyph_ids: Option<Vec<u32>>,       // *
    pub font_name: String,                 // *
    pub font_size: f32,                    // *
    pub bbox: Rect,                        // *
    pub transform: [f32; 6],               // *
    pub mcid: Option<u32>,                 // *
    pub mapping_confidence: f32,           // *
    pub metrics_confidence: f32,           // *
    pub from_actual_text: bool,            // *
    // unstable extras
    pub fill_color: Option<Color>,
    pub rendering_mode: u8,
    pub invisible: bool, // Tr==3
    pub char_spacing: f32,
    pub word_spacing: f32,
    pub rise: f32,
    pub ocg: Option<String>,
}

pub struct TextOptions {
    pub preserve_positions: bool,
    pub sort_reading_order: bool, // default true for text()
    pub insert_spaces: bool,      // default true
    pub unknown_glyph: char,      // default U+FFFD
}
```

### Confidence / quality signals

| Signal | Computation |
|--------|-------------|
| `mapping_confidence` | 1.0 ActualText; 0.95 ToUnicode complete; 0.8 encoding+AGL; 0.5 partial CMap; 0.2 identity guess; 0.0 all FFFD |
| `metrics_confidence` | 1.0 explicit Widths/W; 0.85 hmtx; 0.5 MissingWidth; 0.3 vertical stub; 0.0 zero widths |
| Run aggregate | min or mean of glyph confidences (document mean for gates) |

### Edge cases & failure modes

| Case | Behavior |
|------|----------|
| Empty show string | no-op |
| Overlapping text (draw twice) | two runs; CER may double-count—oracle policy joins |
| Clip removes visual text | still extract (PDF extract ≠ visibility) unless future visible-only |
| Tr=3 invisible | extract; `invisible=true` unstable field; still in text() |
| Hidden OCR layer under image | extract text; `HiddenTextLayer` if text overlaps image bbox heavily with Tr=3 or render mode |
| Missing ToUnicode CID | FFFD + MissingToUnicode |
| Huge TJ arrays | op/count limits |

### Acceptance criteria / tests

| Test | Gate |
|------|------|
| digital_text_easy_15 CER ≤ 5% | **0.1 blocking** |
| digital_text_50 CER report | 0.1 report; 1.0 ≤2% |
| metrics_20 bbox IoU smoke | **0.1** 5 fixtures ≥70% |
| ActualText preferred over bad ToUnicode | unit golden |
| Space insertion Latin brochure | golden |
| Multi-column 2-col fixture | reading order golden |
| Tr=3 still in text | unit |
| Encryption not applicable | N/A |

### Phase

**P0/0.1 critical path** for paint + metrics + spaces + basic order; vertical/advanced BiDi later.

---

## C2. Image extraction

### Problem definition

Images appear as **Image XObjects** or **inline images**. Users want pixels or portable encoded bytes, dimensions, color metadata, and placement bboxes—not necessarily a full color-managed decode.

### In-scope variants

| Variant | Phase |
|---------|-------|
| Image XObject Do | **0.1** |
| Inline BI/ID/EI | **0.1** |
| DeviceGray/RGB/CMYK | **0.1** metadata + extract |
| Indexed | **0.1** metadata; decode optional |
| ICCBased | **0.1** metadata; **no CMS** |
| DCTDecode JPEG pass-through | **0.1** |
| Flate/LZW/RunLength image streams | **0.1** decode under governor |
| JPX | warn/skip best-effort |
| CCITT | **0.2** |
| JBIG2 | feature off |
| SMask / Mask / soft mask | **0.1** record refs; apply optional later |
| BitsPerComponent, Decode arrays | **0.1** store |
| Form XObject containing images | **0.1** via form inline |
| decode_pixels RGBA | feature `image-decode`, default off |

### Out of scope

| Item | Phase |
|------|-------|
| Full ICC transform to sRGB always | later |
| JPEG2000 full support | later |
| Image semantic classification (logo vs photo) | non-goal |

### Extraction algorithm

#### XObject image

```text
1. On Do(name): resolve Resources.XObject[name]
2. If Subtype==Image:
   a. Read Width, Height, ColorSpace, BitsPerComponent, Filter(s), Decode, ImageMask, Mask, SMask, Intent
      from **stream dictionary only** — never requires sample decode (K23).
   b. pixel_count = width * height; checked_mul; gov.check_image_pixels (metadata gate even if not decoding)
   c. Default (decode_pixels=false):
        ImageData::Deferred { object_id }  // preferred when object id known
        OR ImageData::Embedded {
             bytes: **original encoded stream payload** (filters NOT applied),
             encoding: infer from Filter (Jpeg if sole DCTDecode; else Unknown/Raw-encoded)
           }
      **Do not** run Flate/LZW/RunLength/ASCII* expansion for default extract.
   d. bbox = unit square [0,0,1,1] transformed by CTM (image space → user space)
   e. Emit Element::Image
3. If decode_pixels=true (opt-in; feature `image-decode` for RGBA path):
   - apply owned filter pipeline under governor to obtain samples
   - apply Decode array; Indexed → expand palette; optional RGBA8
   - if pixel buffer or expansion > budget → LimitExceeded
4. Optional future flag `expand_image_filters` (not in 0.1 freeze): decode filters but not color-convert;
   still off by default. **0.1 implementers: only Deferred/Embedded-encoded unless decode_pixels.**
```

#### Inline image

```text
1. BI/ID/EI per B5 normative locator (expected_len, EI scan, bounds).
2. Same K23 default: Embedded { bytes: payload as stored after ID, encoding from /F } — **no** filter expand.
3. Same pixel limits when W,H present; WarningCode::InlineImageSkipped on failure.
```

#### Encoded vs decoded policy (K23 — normative, Issue 3)

| Mode | Default | Behavior |
|------|---------|----------|
| `decode_pixels=false` | **yes** | **Never** expand image filter chains. Store `Deferred` or `Embedded` with **encoded** bytes as in the file (DCT JPEG bitstream, Flate-compressed samples, etc.). Metadata from dict only. |
| `decode_pixels=true` | opt-in | Expand filters under governor; optional RGBA8 behind `image-decode`. |

Rationale: default Flate→raw expansion burns `max_expanded_stream_bytes` / total budget on every image page and violates product K23.

### Data model

```rust
pub struct ImageElement {
    pub id: Option<ObjectId>,          // frozen allowlist uses id
    pub bbox: Rect,
    pub transform: [f32; 6],
    pub width_px: u32,
    pub height_px: u32,
    pub color_space: String,           // normalized name
    pub bits_per_component: u8,
    pub filters: Vec<String>,
    pub data: ImageData,
    // unstable
    pub smask: Option<ObjectId>,
    pub mask: Option<ImageMaskKind>,
    pub decode: Option<Vec<f32>>,
    pub interpolate: bool,
    pub image_mask: bool,
}

pub enum ImageData {
    Embedded { bytes: Vec<u8>, encoding: ImageEncoding },
    Deferred { object_id: ObjectId },
    DecodedRgba { width: u32, height: u32, pixels: Vec<u8> }, // feature
}

pub enum ImageEncoding {
    Jpeg,
    Raw,
    Unknown,
}
```

### Confidence / quality

- Metadata completeness score: required Width/Height present → extract OK
- `UnsupportedFilter` → image skipped, not low-confidence image

### Edge cases

| Case | Behavior |
|------|----------|
| Zero dimensions | skip + ImageSkipped |
| Pixel bomb 1e9 × 1e9 | LimitExceeded before alloc |
| Stencil ImageMask | export as mask metadata |
| SMask | record; soft mask compose later |
| Form nests 50 images | nesting + pixel budgets |

### Acceptance

- ≥95% metadata success where XObject images present (0.1 appendix)
- Inline image fixture golden
- Bomb tests

### Phase

**0.1** core; CCITT 0.2; JBIG2 optional later.

---

## C3. Tables — all types (critical)

### Problem definition

PDF almost never has a native “table” object for visual grids. Tables appear as: structure tags, ruled vectors + text, or whitespace-aligned columns. Users want cells, spans, headers, and confidence.

### Orchestration (normative — Issue 1: no structure short-circuit)

```text
══════════════════════════════════════════════════════════════════════════════
                    TABLE DETECTION ORCHESTRATION (per page)
══════════════════════════════════════════════════════════════════════════════

  detect_tables == false ?  ──yes──▶  return []
         │
         no
         ▼
  table_modes bits (Structure | Lattice | Stream | Hybrid)
         │
         ├──────────────────┬──────────────────┬─────────────────┐
         ▼                  ▼                  ▼                 │
  ┌─────────────┐   ┌─────────────┐   ┌─────────────┐           │
  │ Structure   │   │ Lattice     │   │ Stream      │           │
  │ detector    │   │ detector    │   │ detector    │           │
  │ (if bit)    │   │ (if bit)    │   │ (if bit)    │           │
  └──────┬──────┘   └──────┬──────┘   └──────┬──────┘           │
         │                 │                 │                  │
         │                 └────────┬────────┘                  │
         │                          ▼                           │
         │                 ┌─────────────────┐                  │
         │                 │ Hybrid refine   │◄── if Hybrid bit │
         │                 │ (partial rules  │                  │
         │                 │  + whitespace)  │                  │
         │                 └────────┬────────┘                  │
         │                          │                           │
         └────────────┬─────────────┴───────────────────────────┘
                      ▼
              Candidate multiset  (ALL enabled detectors always run;
                                   structure success NEVER skips others)
                      │
                      ▼
              Drop conf < min_table_confidence  (default 0.55)
                      │
                      ▼
              NMS / IoU dedupe  (IoU ≥ 0.5 → keep higher conf;
                                 tie-break: Structure > Hybrid > Lattice > Stream)
                      │
                      ▼
              Cap max_tables_per_page  (default 32)
                      │
                      ▼
              Emit tables[]
```

**Normative policy (critical):**

1. When `detect_tables=true`, run **every detector enabled in `table_modes`** on the page. Structure success **never** skips lattice/stream.
2. “Partial structure coverage” is **only** a confidence input on structure candidates (MCID gaps), **not** a control-flow short-circuit.
3. Union all candidates → filter by `min_table_confidence` (default **0.55**) → **NMS**:
   - If two tables IoU(bbox) ≥ **0.5**, keep higher confidence; **tie-break: Structure > Hybrid > Lattice > Stream**.
4. `max_tables_per_page` default **32** (hard_max 256); excess dropped with `TableLowConfidence`/`TableAmbiguous` note lowest conf first.
5. Early-exit (skip heuristics when structure found) is **not** default; only allowed behind future opt-in `layout.tables_structure_only_fast_path` (default **false**) for embedders who accept recall loss.
6. Path rule segments: interpret with `capture_rule_segments` when lattice/hybrid bits set (A7 / EX11).

**Default:** `detect_tables=false` (K21); when flipped true, `TableModeSet::all()` (structure|lattice|stream|hybrid).  
**Module:** `pdfparser::experimental` until 0.2.

### Shared data model

```rust
pub struct Table {
    pub bbox: Rect,
    pub page: u32,
    pub method: TableMethod,
    pub confidence: f32,
    pub rows: u32,
    pub cols: u32,
    pub cells: Vec<TableCell>,
    pub header_rows: u32,
    pub continued_from_previous_page: bool,
    pub notes: Vec<String>,
    /// Nested tables (structure or lattice-in-cell); depth capped (C3.6).
    pub children: Vec<Table>,
    /// Optional hints for financial/KV stream subclass (C3.5); not a separate method in 0.2 F1.
    pub kv_hints: Option<KvHints>,
}

pub struct KvHints {
    pub label_value_columns: bool,
    pub numeric_right_align_score: f32,
}

pub enum TableMethod {
    Structure,
    Lattice,
    Stream,
    Hybrid,
    FormLayout, // C3.10 — not in 0.2 F1 gates
}

pub struct TableCell {
    pub row: u32,
    pub col: u32,
    pub rowspan: u32,
    pub colspan: u32,
    pub bbox: Rect,
    pub text: String,
    pub is_header: bool,
    pub source_mcids: Vec<u32>,
}
```

### Confidence term definitions (normative — Issue 19)

All component scores clamped to `[0.0, 1.0]`. ε geometry = **1.0** user unit unless noted.

| Term | Definition |
|------|------------|
| `grid_regularity` | Let row heights = cell row band heights; col widths = column band widths. `cv(xs) = stddev(xs)/max(mean(xs), ε)`. `grid_regularity = 1 - min(1, 0.5*(cv(row_heights)+cv(col_widths)))`. |
| `rule_support` | For each cell, 4 edges; edge “supported” if an axis-aligned rule segment covers ≥ 70% of edge length within distance ≤ **1.5** user units. `rule_support = supported_edges / (4 * cell_count)`. |
| `fill_rate` | `(cells with non-empty normalized text) / max(total_cells, 1)`. |
| `alignment_score` | For each column, collect text run x0 (or x1 if majority right-aligned: median x1 variance < median x0 variance). `col_align = 1 - min(1, stddev(xs) / max(col_width, ε))`. `alignment_score = mean(col_align)`. |
| `column_separation_score` | For stream: mean over internal column boundaries of `min(1, gap_i / (2 * median_space_width))` where `gap_i` is median horizontal gap at that boundary across rows. |
| `row_consistency` | `1 - min(1, cv(row_cell_counts))` where row_cell_counts is number of non-empty cells per row (or expected cols if ragged). |

**Lattice v0 (product weights):**

```text
confidence =
  0.35 * grid_regularity +
  0.25 * rule_support +
  0.20 * fill_rate +
  0.10 * alignment_score +
  0.10 * min(1, cells/6)
```

**Stream v0:**

```text
confidence =
  0.30 * column_separation_score +
  0.25 * row_consistency +
  0.20 * fill_rate +
  0.15 * alignment_score +
  0.10 * min(1, cells/6)
```

**Structure:** start 0.9; subtract 0.1 per major MCID gap (row with >50% empty MCIDs); floor 0.5.

**Hybrid:** on candidates refined by both: `0.5 * conf_lattice + 0.5 * conf_stream` using scores recomputed on the merged grid; **+0.05** if both agreed on `(rows, cols)` before merge (cap 1.0).

**Golden tests:** synthetic 3×3 full-border grid expects lattice confidence ≥ 0.75 ± 0.05; empty prose block stream confidence ≤ 0.40.

### Eval metrics

| Metric | Definition |
|--------|------------|
| Cell match | IoU≥0.5 **or** same indices if equal grid; text normalized |
| Cell F1 | micro-F1 on matched cell text |
| TEDS | secondary |
| **0.2 gates (only these)** | lattice F1≥0.70; stream≥0.55; structure≥0.90 |
| C3.5–C3.11 | **not** in eval v0 / 0.2 F1 gates — report-only if implemented |

---

### C3.1 Tagged / structure-tree tables

**Taxonomy:** `Table`, `THead`, `TBody`, `TFoot`, `TR`, `TH`, `TD`; attributes `RowSpan`, `ColSpan`; optional headers scope.

#### Algorithm

```text
1. Walk StructTreeRoot; find elements with role mapped to Table (after RoleMap)
2. For each Table:
   a. Collect child TR in order (including under THead/TBody/TFoot)
   b. For each TR, collect TH/TD
   c. Read RowSpan/ColSpan from attributes (A dict / HTML-like attrs)
   d. Build occupancy grid; place cells with spans
   e. Resolve MCID → text from page IR marked content map; concatenate in leaf order
   f. bbox = union of cell bboxes or structure BBox if present
3. confidence per structure formula
4. Nested Table children → nested Table objects (or flatten with NestedTableFlattened warning if depth>2 in 0.1 experimental)
```

#### Edge cases

- RoleMap maps `CustomTable` → Table
- Missing MCIDs → empty cell text; lower confidence
- Artifact rows → skip if artifact

#### Phase

**0.1 experimental / 0.2 stable** for structure tables; harness PR20–21.

---

### C3.2 Lattice / ruled tables

**Variants:** full border grid; partial borders; double lines; outer frame only + internal ticks.

#### Algorithm

```text
1. Collect segments from **internal RuleSegmentBuf** (A7) produced during interpret when
   capture_rule_segments=true — NOT dependent on public include_paths:
   - axis-aligned only for v0 (angle within 2° of 0/90°)
   - merge colinear segments within tol = 1.0 user unit
   - cap at max_path_segments_per_page
2. Cluster horizontal y-coordinates and vertical x-coordinates
   (snap tol = max(1.0, 0.5 * median_stroke_width))
3. Form candidate grid lines if ≥2 H and ≥2 V lines spanning a common region
4. Intersections define candidate cell rectangles
5. Assign text runs to cells by bbox center containment (else max IoU ≥ 0.5)
6. Compute confidence terms (Issue 19 definitions)
7. Filter grids with rows<2 or cols<2
8. Page-level NMS is global (orchestration); detector may pre-suppress self-overlaps
```

#### Double lines

```text
Merge parallel lines distance < δ into single logical rule (keep outer or midline consistently)
```

#### Failure modes

- Charts with gridlines → false positive tables (min fill_rate + text density gate)
- Diagonal forms → missed (v0 axis-aligned only)
- Very light gray rules missing if paths not stroked as expected

#### Phase

**0.1 experimental** (off default); **0.2** opt-in stable target.

---

### C3.3 Stream / whitespace tables

**Variants:** tab-stop columns; space-padded financial columns; monospace reports.

#### Parameters (normative — Issue 12)

| Parameter | Default | Meaning |
|-----------|---------|---------|
| γ gap factor | **2.0** | Word gap > γ * median_space_width → column break candidate |
| Histogram bin | **max(2.0, 0.3 * median_font_size)** | For boundary x positions |
| Peak prominence | **≥ 30% of lines** in the candidate set must contribute a gap near the peak (within one bin) |
| Min lines | **3** | |
| Min internal boundaries | **2** | ⇒ ≥3 columns **or** 1 boundary ⇒ 2 columns with min lines **4** |
| Prose reject: `column_separation_score` | **< 0.35** | Drop candidate |
| Prose reject: punctuation density | **> 0.12** | Mean of `.?!,;:` chars / text chars across cells → drop if also alignment_score < 0.55 |
| Multi-line cells | **v0: off** | One layout line = one table row (E-T6); no wrap merge in 0.1/0.2 F1 |

#### Algorithm

```text
1. Take reading-order lines from layout (C1.4); skip artifact-only lines.
2. median_space_width from fonts on page (fallback 0.25 * median_font_size).
3. For each line, words = runs split on whitespace bboxes; find gaps between successive words
   where gap > γ * median_space_width; record gap midpoint x as boundary candidate.
4. Histogram all boundary x across lines (bin width above). Peaks = bins with count
   ≥ peak_prominence * num_lines; merge adjacent peak bins.
5. A line “supports” a peak if it has a gap within one bin of peak x.
6. Keep peak set if ≥ min_lines lines each support ≥ min_internal_boundaries peaks
   (or 2-col special case: ≥1 peak, ≥4 lines).
7. Column x-edges = page/cluster left, sorted peaks, page/cluster right.
8. Cell text = concatenation of words whose centers fall in column band; row = **exactly one line** (v0).
9. Compute stream confidence terms; apply prose rejection predicates above.
10. Emit method=Stream if conf ≥ min_table_confidence.
```

#### Failure modes

- Justified prose → false columns (prose predicates)
- Multi-line cells → split across rows in v0 (accepted limitation)
- Nested indentation lists → C3.11

#### Phase

Experimental 0.1 (off default) → 0.2 opt-in gate for stream F1.

---

### C3.4 Hybrid (partial rulings + whitespace)

```text
1. Run lattice and stream independently
2. If lattice finds outer frame + few internals: use stream to split internal columns within frame
3. If stream columns align with lattice vertical rules: boost confidence
4. Emit method=Hybrid
```

---

### C3.5 Borderless financial / key-value grids

**Status:** Stream **subclass** (not a separate `TableMethod` for 0.2 F1). Populates `Table.kv_hints`.

| | |
|--|--|
| **In scope** | 2-column label/value; right-aligned numeric amount columns |
| **Out of scope** | Cross-page KV stitch; handwritten forms |
| **Eval** | **Not** in 0.2 F1 gates |

#### Algorithm

```text
1. Start from a Stream candidate with cols == 2 OR cols >= 2 with last column numeric.
2. numeric_right_align_score =
     fraction of last-column cells matching /^-?\$?[\d,]+(\.\d+)?%?$/
     * (1 - min(1, stddev(x1) / col_width))
3. label_colon_score = fraction of col0 cells ending with ':' or matching key-like short text (≤ 40 chars, no sentence end).
4. If numeric_right_align_score ≥ 0.6 OR (cols==2 AND label_colon_score ≥ 0.5):
     set kv_hints = KvHints { label_value_columns: cols==2, numeric_right_align_score }
     boost confidence by min(0.05, 0.05 * numeric_right_align_score) (cap 1.0)
5. Else: no kv_hints (still may emit as Stream if conf ok).
```

**Edges:** multi-currency symbols; parentheses negatives. **Phase:** 0.2 refinement after stream detector.

---

### C3.6 Nested tables

| | |
|--|--|
| **In scope** | Structure-recursive Table; one level of lattice-in-cell |
| **Out of scope** | Arbitrary depth lattice nesting in 0.1 |
| **Eval** | Not in v0 F1 (structure nested goldens optional later) |

#### Algorithm

```text
1. Structure: when a Table StructElement contains a child Table role, parse child into Table
   and push onto parent.children (depth ≤ max_table_nesting default **2**).
2. Lattice: after primary grid, for each cell with area ≥ 4× median cell area and ≥2 H + ≥2 V
   rule segments strictly inside cell inset by 2 units, run lattice restricted to that bbox;
   if conf ≥ min_table_confidence, append to parent.children (depth cap 2).
3. If nesting would exceed depth: flatten child cells into parent notes + WarningCode::NestedTableFlattened.
4. Stream: no nested stream in v0.
```

**Phase:** structure nesting 0.2; lattice nesting experimental later.

---

### C3.7 Multi-page tables

| | |
|--|--|
| **In scope** | Per-page flag `continued_from_previous_page` |
| **Out of scope for scoring** | Cross-page stitch (product eval v0) |
| **Eval** | Not scored in v0 |

#### Algorithm

```text
1. Detect tables independently per page.
2. For page i>0, candidate top tables (bbox.y1 in top 25% of page height):
   Compare to page i-1 bottom tables (bbox.y0 in bottom 25%):
   a. header_sim = normalized text equality of row 0 (NFKC, collapse WS) ≥ 0.9
      OR (header_rows≥1 and same comparison)
   b. col_align = mean |col_x_center_i - col_x_center_{i-1}| ≤ 3.0 user units for min(cols)
   c. same cols count
3. If a∧b∧c: set continued_from_previous_page=true on page i table; note previous page index in notes.
4. Stitch API (later): TableRegion { pages, rows concatenated without duplicate header }.
```

**Phase:** flags **0.2**; stitch **later**. Warning `MultiPageTableSplit` if cols match but header_sim low.

---

### C3.8 Rotated tables

| | |
|--|--|
| **In scope** | Page `/Rotate` via upright transform; CTM 90° content best-effort |
| **Out of scope** | Arbitrary 15° skewed tables in v0 |

#### Algorithm

```text
1. When effective apply_page_rotate for layout/tables (default true for layout APIs):
   transform rule segments + text bboxes by page rotate about media box before detectors.
2. Content rotated via CTM (~90°): if median absolute text baseline angle within 2° of 90/270:
   a. Swap interpretation of H/V segments (x-cluster ↔ y-cluster)
   b. Rebuild grid in “visual upright” space
   c. WarningCode::RotatedTableBestEffort
3. Else if angle not near 0/90/180/270: skip lattice (stream may still run on reading-order lines).
```

**Phase:** best-effort **0.2**. **Acceptance:** one rotated-page fixture report-only.

---

### C3.9 Tables made of images (scanned)

| | |
|--|--|
| **In scope** | Detection of “likely image table” for warning |
| **Out of scope** | OCR cell text (separate product track) |

#### Algorithm

```text
1. If page has ImageElement with area ≥ 0.5 * page area AND text run count inside image bbox < 5
   AND lattice/stream found no candidate with conf ≥ min:
     emit no table; WarningCode::ImageTableSkipped
2. If text runs exist (OCR layer): normal detectors apply (C12 hidden text).
```

**Phase:** warn-only **0.1+**; OCR **later**.

---

### C3.10 Tables from AcroForm layouts

| | |
|--|--|
| **In scope** | Widget rect grid → `TableMethod::FormLayout` |
| **Out of scope** | XFA layout grids |
| **Eval** | Not in 0.2 F1 |

#### Algorithm

```text
1. Collect terminal widget rects for fields under AcroForm on this page (page annots subtype Widget).
2. Cluster rect.y centers into rows (tol = 0.25 * median widget height); need ≥ 2 rows.
3. Cluster rect.x centers into cols (tol = 0.25 * median widget width); need ≥ 2 cols.
4. grid_regularity as in Issue 19 on widget sizes; require ≥ 0.5.
5. Cell text = field V (or export state for buttons); empty if missing.
6. confidence = 0.4 * grid_regularity + 0.3 * fill_rate + 0.3 * min(1, widgets/6)
7. Emit only if detect_tables && table_modes includes an experimental form bit
   (default **off** even when TableModeSet::all — FormLayout requires `table_modes.form_layout`).
```

**Phase:** **later** / 0.2 stretch; do not build in same PR as lattice (protect K35).

---

### C3.11 Markdown-like / list-as-table false positives

#### Rejection predicates (applied to Stream/Lattice candidates before emit)

```text
Reject if any:
  R1. cols < 2
  R2. Structure role of majority text is L/LI/Lbl/LBody (list) when structure present
  R3. Stream with exactly 1 internal boundary AND median gap x < page_left + 0.2*page_width
      AND left column mean text length < 4 chars (bullet/number column) → list indent FP
  R4. fill_rate ≥ 0.9 AND alignment_score < 0.4 AND column_separation_score < 0.45 (prose)
  R5. Lattice rule_support < 0.15 AND fill_rate > 0.85 (chart/plot with frame only) — drop lattice
```

Score: no separate confidence; boolean reject. Log note in `table.notes` when rejected during debug (`trace-ops` / table debug feature).

**Phase:** ship with stream/lattice detectors.

---

### Table acceptance summary

| Suite | Target | Gate |
|-------|--------|------|
| Structure fixtures | ≥0.90 cell F1 | **0.2** |
| Lattice labeled | ≥0.70 | **0.2** |
| Stream labeled | ≥0.55 | **0.2** |
| C3.5–C3.11 | report-only / none | no 0.2 F1 |
| 0.1 | harness runs; tables off | no release gate |

**Implementation fence (K35):** PRs 21–23 implement structure/lattice/stream/hybrid only. C3.5 hints may ship as stream post-pass; C3.6–C3.10 must not block 0.1 text path.

---

## C4. Vector graphics / paths

### Problem definition

Paths paint shapes and **also** provide lattice evidence. Users rarely want full Bézier dumps; tables/layout need rules.

### In-scope

| Item | Phase |
|------|-------|
| Path construction m/l/c/v/y/h/re | **0.1 capture** |
| Fill/stroke/clip | **0.1** record op type |
| Axis-aligned segments for tables | **0.1/0.2** when tables |
| Full path export in IR | unstable; `include_paths` |

### Out of scope

- True bezier lattice reconstruction for curved “tables”
- Rasterization

### Algorithm

```text
1. Maintain current path subpaths (transform points by CTM at append time — recommended).
2. On stroke (S/s/B/b…):
   a. Flatten curves to polylines (flatness default 1.0 user unit).
   b. If capture_rule_segments (lattice/hybrid tables): append axis-aligned edges to RuleSegmentBuf;
      charge max_path_segments_per_page.
   c. If include_paths: emit PathElement (full polyline + stroke style).
3. On fill-only (f/f*): emit PathElement only if include_paths; do **not** add fill-only edges to RuleSegmentBuf.
4. Clip (W/W* + n): update clip stack; not lattice rules unless also stroked.
5. Optional: drop fill-only rects covering ≥ 80% page from PathElement noise.
```

**Internal vs public:** `RuleSegmentBuf` may populate when `include_paths=false` (Issue 2).

### Data model

```rust
pub struct PathElement {
    pub bbox: Rect,
    pub segments: Vec<PathSeg>, // unstable granularity
    pub fill: Option<Color>,
    pub stroke: Option<Color>,
    pub line_width: f32,
    pub even_odd: bool,
}
```

### Phase

**0.1** capture when requested; lattice consumer **0.1 exp / 0.2**.

---

## C5. Form XObjects & soft masks / transparency groups

### Problem definition

Forms reuse content streams with local resources and matrix. Soft masks/transparency affect visibility, not extract semantics primarily.

### Algorithm — Form `Do`

```text
1. Resolve XObject subtype Form
2. If form_id in visited → warn FormCycle; return
3. If depth ≥ max_nesting_depth → FormNestingLimit; return
4. Push visited; depth++
5. Save GState (q semantics): CTM = FormMatrix * CTM; clip to BBox transformed
6. Merge Resources: form resources override for name lookup during form
7. Interpret form content stream ops into **same page IR** (inline policy K27)
8. Optional FormBoundary markers if mark_form_boundaries
9. Pop visited; restore GState
```

### Soft masks / groups

| Feature | 0.1 |
|---------|-----|
| Group dictionary on page/form | record metadata |
| SMask on ExtGState | SoftMaskIgnored; do not compose |
| Blend modes | record name only |
| Alpha | store on state; not used for text omission |

### Phase

Forms **0.1**; soft mask compose **later**.

---

## C6. Annotations

### Problem definition

Annotations are page-level dicts (not content stream). Extract appearance-independent fields: subtype, rect, contents, actions—**never execute JS**.

### In-scope subtypes (0.1 extract)

| Subtype | Fields of interest |
|---------|-------------------|
| Link | Rect, Dest / A (URI, GoTo), QuadPoints |
| Text | Contents, RC (rich, store raw), Rect |
| FreeText | Contents, DA, Rect |
| Highlight, Underline, StrikeOut, Squiggly | QuadPoints, Contents |
| Stamp | Subj, Contents, Rect |
| Widget | Field ref / FT, T, V (coordinate with forms) |
| FileAttachment | FS, Contents |
| Popup | Parent, Contents, Open |
| Line, Square, Circle, Polygon, PolyLine | geometry coords |
| Caret, Ink | points |
| Sound, Movie, Screen, RichMedia | metadata only; skip heavy media |
| PrinterMark, TrapNet, Watermark, 3D, Redact | metadata best-effort |

### Algorithm

```text
1. For page.Annots array: resolve each annot dict
2. Read Subtype, Rect, Contents, NM, M, F flags, C color, Border, AP presence (bool only)
3. Parse A or PA action:
   - URI: extract URI string
   - GoTo: extract dest
   - GoToR, Launch, SubmitForm, JavaScript: record type; if JS → JsActionPresent warning; **do not run**
4. Dest: name or explicit array → resolve to page index + view if possible
5. Malformed → AnnotationSkipped
```

### Data model (Issue 17 — no unbounded JSON bag)

```rust
pub struct Annotation {
    pub subtype: String,
    pub rect: Rect,
    pub contents: Option<String>, // truncated to max_string_bytes
    pub name: Option<String>,
    pub flags: u32,
    pub color: Option<Color>,
    pub action: Option<AnnotAction>,
    pub dest: Option<Destination>,
    pub quad_points: Option<Vec<f32>>,
    /// Bounded subtype-specific keys (not serde_json::Value).
    pub extra: BTreeMap<String, AnnotValue>,
}

pub enum AnnotValue {
    Bool(bool),
    Int(i64),
    Real(f32),
    Text(String), // each ≤ max_string_bytes; map entries ≤ 32
    Name(String),
}

pub enum AnnotAction {
    Uri { uri: String },
    GoTo { dest: Destination },
    Named { name: String },
    JavaScript { script_present: bool },
    Other { s: String },
}
```

### Acceptance criteria / tests (Issue 21)

| Fixture | Expect |
|---------|--------|
| Link + URI | `action = Uri` |
| Link + GoTo | dest resolves when possible |
| Highlight QuadPoints | len multiple of 8 |
| JS action | `script_present`, JsActionPresent, no exec |
| Malformed annot | AnnotationSkipped |
| Huge Contents | truncate to max_string_bytes |

### Phase

**0.1 extract** core subtypes; media-heavy later.

---

## C7. Interactive forms (AcroForm)

### Problem definition

AcroForm field tree holds values independent of visual content. Users want field names, types, values, and hierarchy—not calculations.

### Algorithm

```text
1. catalog.AcroForm → Fields array
2. DFS field dicts:
   - T partial name; build fully qualified name with parent
   - FT: Tx, Btn, Ch, Sig (inherit FT from parent)
   - V value; DV default
   - Kids vs widget annotations (terminal)
   - Ff flags (ReadOnly, Required, etc.)
   - DA default appearance string (store)
   - Opt for choice
3. Btn: determine radio/checkbox/push via flags; V / AS export states
4. Sig: extract dictionary metadata only (ByteRange, Contents length)—no crypto verify
5. XFA: if AcroForm.XFA present:
     - measure size; if > max_xfa_bytes → hard LimitExceeded or reject dataset
     - else XfaIgnored warning; do not execute; optional store truncated flag
```

### Data model

```rust
pub struct FormTree {
    pub fields: Vec<FormField>,
    pub xfa: XfaStatus,
}

pub struct FormField {
    pub full_name: String,
    pub partial_name: String,
    pub field_type: FieldType, // Tx, Btn, Ch, Sig, Unknown
    pub value: Option<FieldValue>,
    pub default_value: Option<FieldValue>,
    pub flags: u32,
    pub kids: Vec<FormField>,
    pub widgets: Vec<ObjectId>,
    pub tooltip: Option<String>, // TU
}

pub enum XfaStatus {
    Absent,
    Ignored { bytes: u64 },
    RejectedTooLarge { bytes: u64 },
}
```

### Acceptance criteria / tests (Issue 21)

| Fixture | Expect |
|---------|--------|
| Nested Kids fields | FQ names `Parent.Child` |
| Tx with V | text value matches |
| Btn checkbox | V/AS recorded |
| Ch Opt+V | choice extracted |
| Sig | metadata only |
| XFA over max | LimitExceeded / RejectedTooLarge |
| XFA under max | XfaIgnored; fields still OK |
| Field AA JS | warn; no exec |

### Phase

**0.1** values/widgets; XFA reject/limit; no calculate.

---

## C8. Structure tree & marked content

### Problem definition

Tagged PDF provides semantics (PDF/UA). Marked content ties operators to structure via MCID.

### Algorithm — interpret side

```text
1. BDC/BMC with Properties: push MarkedContent { tag, props, mcid }
2. Properties may be inline dict or name → Resources.Properties
3. On EMC pop; associate nested ops' elements with mcid
4. ActualText/Alt/E/Lang from props
5. Artifact tag → mark elements artifact; optional exclude from reading text
```

### Algorithm — structure tree

```text
1. catalog.StructTreeRoot
2. Walk K entries (dicts / integers / arrays)
3. RoleMap: map custom → standard
4. ClassMap: merge attributes
5. Build tree nodes: role, children, mcid+page refs (Pg)
6. Reading order: depth-first leaf MCIDs → map to elements
```

### Data model (unstable 0.1)

```rust
pub struct StructureTree {
    pub root_kids: Vec<StructElement>,
    pub role_map: BTreeMap<String, String>,
}

pub struct StructElement {
    pub role: String,
    pub mapped_role: String,
    pub kids: Vec<StructChild>,
    pub attributes: BTreeMap<String, String>,
    pub alt: Option<String>,
    pub actual_text: Option<String>,
}

pub enum StructChild {
    Element(StructElement),
    Mcid { page: u32, mcid: u32 },
    ObjectRef(ObjectId),
}
```

### Acceptance criteria / tests (Issue 21)

| Fixture | Expect |
|---------|--------|
| Tagged PDF with P/Span MCIDs | tree non-empty; runs carry mcid |
| RoleMap custom → P | mapped_role == P |
| ActualText on BDC | from_actual_text on runs |
| Broken K ref | StructureBroken; partial tree |
| Artifact BMC | excluded from Page::text; in elements |

### Phase

**0.1 walk + MCID map**; full UA completeness later.

---

## C9. Optional content (OCG / layers)

### Problem definition

Optional Content Groups hide/show graphics. Extractors tag elements and may filter to default-visible content without full usage-expression evaluation in 0.1.

### In-scope / out

| In (0.1) | Out |
|----------|-----|
| OCProperties / OCGs / D default BaseState ON/OFF | Full P/VE usage expressions beyond default config |
| Tag elements with OCG name | Interactive layer UI |
| `visible_ocg_only` filter | Per-viewer intent overrides |

### Algorithm (normative)

```text
1. Read catalog.OCProperties (if missing → no OCG; done).
2. Build OCG list: object ids + Name from OCGs array (resolve refs).
3. Default config D:
   a. BaseState = ON | OFF | Unchanged (treat Unchanged as ON for extract defaults).
   b. visibility[all] = (BaseState == ON).
   c. Apply D.ON → visible=true; D.OFF → visible=false.
   d. Ignore AS (auto state) events in 0.1.
4. During interpret: BDC/BMC properties /OC (name or OCG ref) → push tag; stamp elements.
   Form dict /OC similarly.
5. If visible_ocg_only: drop elements with ocg tag and visibility[ocg]==false.
   Untagged elements always kept.
6. Unknown OC → OcgUnknown; treat as **visible** when filtering (fail open).
```

### Data model

```rust
pub struct OptionalContentSummary {
    pub groups: Vec<OcgInfo>,
}

pub struct OcgInfo {
    pub id: ObjectId,
    pub name: String,
    pub default_visible: bool,
}
// unstable on elements: ocg: Option<String>
```

### Edges / acceptance

| Case | Behavior |
|------|----------|
| Nested BDC OC | innermost wins |
| visible_ocg_only + OFF | omitted from export |
| 2-layer fixture | tags + filter golden |

### Phase

**0.1** tag + filter option.

---

## C10. Document-level special objects

### Inventory + phase

| Object | Extract | Phase | Notes |
|--------|---------|-------|-------|
| Catalog | yes | 0.1 | feature flags |
| Info dict | yes | 0.1 freeze | Title/Author/… |
| XMP Metadata | yes | 0.1 | algorithm below |
| Outline | yes | 0.1 unstable | below |
| Page labels | yes | unstable | number tree |
| Destinations / Names | yes | 0.1 | name tree |
| Embedded files | meta + limited bytes | 0.1 | below |
| OutputIntents | metadata | 0.1 | no CMS |
| ViewerPreferences | key subset | 0.1 | |
| Threads/articles | best-effort | later | |
| JavaScript | presence | 0.1 | never execute |
| Signatures | dict meta | 0.1 | no verify |
| OpenAction / AA | type presence | 0.1 | no execute |

### Outline algorithm

```text
1. catalog.Outlines → First/Last/Count
2. Walk Next linked list; recurse First children
3. Title (max_string_bytes); Dest or A (no JS exec)
4. Cycle detect ObjectId; depth ≤ max_nesting_depth
5. Resolve Dest to page index when possible
```

### Name tree algorithm (dests, embedded files, JS)

```text
1. Node has Names [name val …] and/or Kids with Limits [min max]
2. iter/lookup: leaf Names pairs; Kids recurse if key in Limits
3. Cycle detect; max nodes ≤ min(max_objects, 100_000)
4. Export named dests: DFS leaves; cap 100_000 entries
```

### Number tree algorithm (page labels)

```text
1. PageLabels: nums [start label_dict …] and/or Kids+Limits
2. Expand PageLabelRange { start_page, style D|R|r|A|a|None, prefix, start_num }
3. Label for page i: greatest start_page ≤ i; format per ISO 32000
4. Cap 10_000 ranges
```

### XMP algorithm

```text
1. catalog.Metadata stream
2. Decode under governor; if > min(8 MiB, budget) skip + warn
3. Store raw XML truncated to 8 MiB and/or best-effort dc:title/creator tag scan
4. Malformed XMP: warn; continue with Info dict
```

### Embedded files / portfolio

```text
1. Names.EmbeddedFiles name tree → FileSpec
2. UF/F name; EF stream Length; Desc; Subtype mime
3. Default: metadata only. Optional bytes API: size ≤ max_expanded_stream_bytes else EmbeddedFileSkipped
4. catalog.Collection: presence + schema keys only
```

### OutputIntents / ViewerPreferences / JS / signatures

```text
OutputIntents: OutputConditionIdentifier, Info, DestOutputProfile ref (no ICC apply)
ViewerPreferences: known keys only (HideToolbar, DisplayDocTitle, Direction, …)
JavaScript: Names.JavaScript or OpenAction S=JavaScript → JsActionPresent; no script body by default
Sig: ByteRange/Contents length/Filter/SubFilter only; no crypto verify
```

### Edges / tests

| Fixture | Expect |
|---------|--------|
| 3-level outline | depth depth 3 |
| Named dest | in dest map |
| PageLabels D | page 0 → "1" |
| Embedded 100MB | skipped |
| Bad XMP | no panic |

### Phase

Per inventory; threads later.

---

## C11. Color & graphics state specials

### Problem definition

ExtGState and exotic colors affect rendering; record state without CMS or pattern painting.

### ExtGState merge algorithm (normative)

```text
1. gs /Name → Resources.ExtGState[Name]
2. Known keys applied; **unknown keys ignored** (optional trace only)

| Key | Effect |
|-----|--------|
| LW LC LJ ML D | stroke style |
| RI | render_intent |
| OP op OPM | overprint flags/mode |
| SA | stroke_adjust |
| FL | flatness for path flatten |
| BM | blend_mode name (record only) |
| SMask | soft_mask ref; SoftMaskIgnored; no compose |
| CA ca | alpha_stroke / alpha_fill |
| AIS TK | store bools if present |
| Font BG UCR TR HT … | ignore (print/halftone) in 0.1 |

3. Missing name → warn once; no-op
```

### Patterns / shadings

```text
1. Pattern color: store Pattern(name); paint → PatternSkipped / UnsupportedPaint; no tile expand
2. sh: record type; ShadingSkipped; no paint
3. Never interpret tiling pattern content streams in 0.1
```

### Color spaces metadata

```text
Device*: store channels
Indexed: base + hival; table deferred
ICCBased: N; IccProfileNotApplied
Separation/DeviceN: names + alt metadata
```

### Tests / phase

- gs BM Multiply + ca 0.5 recorded
- Pattern paint warns, no panic
- **0.1** record; CMS/pattern paint later

---

## C12. Composite / special content cases

### Type3 charprocs

```text
0.1: Widths + optional ToUnicode; do not run CharProcs for text export
Later: mini-VM for lattice paths
Test: Type3 bbox smoke
```

### Patterns as content

```text
No auto-interpret of tiling pattern streams in 0.1 (CPU/depth risk)
```

### Clipping-only content

```text
W/W*+n updates clip; text still extracted; paths optional via include_paths
```

### Invisible text (Tr=3)

```text
Extract; unstable invisible=true; include in Page::text by default
```

### OCR-like hidden text layers (normative thresholds)

```text
large_image: ImageElement area ≥ 0.40 * page_area
text_over_image: non-artifact TextRuns with center inside image bbox
coverage: area(union(text bboxes)) / area(image)
invisible_frac: fraction with Tr==3

Warn HiddenTextLayer when large_image AND coverage ≥ 0.50 AND (
  invisible_frac ≥ 0.50
  OR median font_size among those runs ≤ 3.0
  OR ≥ 30% of runs pairwise IoU ≥ 0.8
)
Still extract all text.

Test: scan + invisible layer → warning + text present in Page::text
```

### Phase

Tr=3 / hidden warn **0.1**; Type3 paths later.

---

## D1. Element enum taxonomy

```rust
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Element {
    Text(TextRun),       // frozen
    Image(ImageElement), // frozen
    Path(PathElement),   // unstable
    FormBoundary { name: Option<String>, begin: bool }, // unstable
    // future: ShadingStub, PatternStub — not in 0.1
}
```

## D2. Document / Page model

```rust
pub struct Document { inner: Arc<DocumentInner> }
pub struct Page { inner: Arc<DocumentInner>, index: u32 }

pub struct ExtractedDocument {
    pub schema_version: u32, // 1
    pub metadata: DocumentMetadata,
    pub pages: Vec<ExtractedPage>,
    pub warnings: Vec<ExtractWarning>,
    pub partial: bool,
    // unstable:
    pub outline: Vec<OutlineItem>,
    pub structure: Option<StructureTree>,
    pub form: Option<FormTree>,
}

pub struct ExtractedPage {
    pub index: u32,
    pub media_box: Rect,
    pub crop_box: Rect,
    pub rotate: i32,
    pub user_unit: f32,
    pub elements: Vec<Element>,
    pub warnings: Option<Vec<ExtractWarning>>,
    // unstable:
    pub tables: Vec<Table>,
    pub annotations: Vec<Annotation>,
    pub layout: Option<PageLayout>,
}
```

## D3. ExtractOptions feature flags

```rust
pub struct ExtractOptions {
    pub text: TextOptions,
    pub images: ImageOptions,
    pub layout: LayoutOptions,
    pub include_paths: bool,           // default false — public Path elements
    pub include_structure: bool,
    pub include_annotations: bool,
    pub include_forms: bool,
    /// See per-API defaults below (Issue 16). Not a single global default for all methods.
    pub apply_page_rotate: Option<bool>, // None = use per-method default
    pub mark_form_boundaries: bool,
    pub allow_partial_document: bool,  // default false
    pub visible_ocg_only: bool,        // default false
}

pub struct LayoutOptions {
    pub detect_tables: bool,           // default false (K21)
    pub table_modes: TableModeSet,     // see below
    pub y_tolerance: Option<f32>,
    pub min_table_confidence: f32,     // default 0.55
    /// Opt-in recall sacrifice; default false (Issue 1).
    pub tables_structure_only_fast_path: bool,
}

bitflags! {
    pub struct TableModeSet: u8 {
        const STRUCTURE = 0b0001;
        const LATTICE   = 0b0010;
        const STREAM    = 0b0100;
        const HYBRID    = 0b1000;
        /// Off unless explicitly set — not part of `all()` for 0.2 F1 (C3.10).
        const FORM_LAYOUT = 0b1_0000;
    }
}

impl Default for TableModeSet {
    fn default() -> Self {
        // Used when detect_tables becomes true without caller override
        Self::STRUCTURE | Self::LATTICE | Self::STREAM | Self::HYBRID
    }
}
```

### `apply_page_rotate` per-API defaults (product coordinate spaces — Issue 16)

| API | Default when `ExtractOptions.apply_page_rotate` is `None` |
|-----|-------------------------------------------------------------|
| `Page::elements` / paint-order IR | **false** — raw unrotated page user space |
| `Page::text` / `Page::layout` / `Page::tables` | **true** — upright visual page |
| Full `Document::extract` JSON elements[] | **false** for element bboxes (schema paint IR); layout/tables sections use upright when present |
| Oracle bbox IoU path | **false** (product metrics — unrotated user space) |
| Oracle CER text path | reading-order text (rotate applied for order when multi-col on rotated pages) |

When `Some(v)`, all methods honor `v`.

### `TableModeSet` interaction (Issue 15)

| `detect_tables` | `table_modes` | Behavior |
|-----------------|---------------|----------|
| false | ignored | no detectors |
| true | default all (S\|L\|Str\|H) | run all four orchestration paths |
| true | STRUCTURE only | structure only (no lattice/stream) |
| true | LATTICE\|STREAM | heuristics only |
| true | + FORM_LAYOUT | also C3.10 (explicit bit required) |

## D4. JSON export shape

```json
{
  "schema_version": 1,
  "metadata": {
    "pdf_version": "1.7",
    "info": { "title": "...", "author": "..." },
    "page_count": 3,
    "encrypted": false,
    "tagged": true
  },
  "pages": [
    {
      "index": 0,
      "media_box": { "x0": 0, "y0": 0, "x1": 612, "y1": 792 },
      "crop_box": { "x0": 0, "y0": 0, "x1": 612, "y1": 792 },
      "rotate": 0,
      "user_unit": 1.0,
      "elements": [
        {
          "type": "text",
          "text": "Hello",
          "glyph_ids": [43, 72, 79, 79, 82],
          "font_name": "F1",
          "font_size": 12.0,
          "bbox": { "x0": 72.0, "y0": 720.0, "x1": 100.5, "y1": 734.0 },
          "transform": [1,0,0,1,72,720],
          "mcid": 0,
          "mapping_confidence": 0.95,
          "metrics_confidence": 1.0,
          "from_actual_text": false
        }
      ]
    }
  ],
  "warnings": [],
  "partial": false
}
```

Unstable sections (`tables`, `structure`, `form`, `outline`) omitted or under `"experimental"` key until 0.2.

## D5. Discovering partial / unsupported features

| Mechanism | Use |
|-----------|-----|
| `warnings[].code` | Machine-readable gaps |
| `partial: true` | Document incomplete |
| `mapping_confidence` / `metrics_confidence` | Per-run quality |
| `Error::Unsupported` / `Encryption` | Hard stop |
| Feature flags / `detect_tables` | Opt-in experimental |
| Operator coverage report (PR25) | Corpus-level unknown ops |

---

# Part E — Cross-Cutting

## E1. Alternatives considered

### Product-level

See product doc Alternatives A–F (greenfield parser, Pdfium-first, thin wrap, hybrid owned extract **chosen**, pdf-rs backend spike, oracle process). **Not re-litigated.**

### Text extraction algorithm alternatives

| Alt | Pros | Cons | Verdict |
|-----|------|------|---------|
| Paint-order only dump | Simple | Bad multi-column | Insufficient alone |
| Structure-only order | Perfect when tagged | Rare in wild | Prefer when present |
| Geometric clustering (chosen) | Works untagged | Heuristic errors | **0.1 basic** |
| ML reading order | High ceiling | Deps/heavy | later research |

### Table algorithm alternatives

| Alt | Pros | Cons | Verdict |
|-----|------|------|---------|
| Structure only | High precision | Low recall untagged | First in orchestration |
| Lattice (Camelot-style) | Strong on ruled | Misses borderless | **Adopt** |
| Stream (Tabula-style) | Borderless | Prose FPs | **Adopt** |
| Deep learning detectors | SOTA possible | Not native-light; **reject for 0.x**: no Python/native ML deps (K28 staffing; binary size; supply chain) | later research only |
| Render+CV | Works scans | Needs full-page raster; **reject for 0.x product path** — raster only behind optional oracle feature, never table core | deferred OCR track |

### Image policy alternatives

| Alt | Verdict |
|-----|---------|
| Always RGBA decode | Reject default (memory); opt-in |
| Always deferred refs | Adopt default K23 |

---

## E2. Security implications per feature

| Feature | Threat | Severity | Mitigation |
|---------|--------|----------|------------|
| Filters | Zip bombs | Critical | Governor ratio+caps |
| Images | Pixel bombs | High | checked mul + max_image_pixels |
| Forms XObject | Deep recursion | High | depth + cycle |
| Fonts | Parser bugs | High | size cap + fuzz |
| XFA | XML bomb | High | max_xfa_bytes; no exec |
| Embedded files | Zip/malware payload | Medium | size limit; don't exec |
| JS actions | Code exec | High | never execute |
| Annotations | Huge Contents | Medium | size cap strings |
| Tables | CPU quadratic clustering | Medium | op/time budgets; caps on path segments |
| OCG | Negligible | Low | |
| Signatures | Misuse as verify | Medium | metadata only docs |

---

## E3. Observability per feature

| Feature | Span / metric |
|---------|----------------|
| Open | `open_document` histogram |
| Decode | `decode_stream` by filter; `limit_trips` |
| Fonts | DEBUG resolution report; `font_load` |
| Content | `interpret_content`; ops count; `unknown_operator` |
| Text | CER job; mean confidences |
| Images | `image_extract_total`; skipped reasons |
| Tables | `detect_tables` span; cell F1 report |
| Annots/forms | counts; skip reasons |
| Structure | nodes walked; broken links |

Logging: `tracing`; no content strings at info by default; `debug-content` feature.

---

## E4. Risk table (extraction fidelity)

| ID | Risk | Sev | Mitigation |
|----|------|-----|------------|
| XR1 | Wrong widths → bad bbox/tables | High | metrics PRs + IoU smoke |
| XR2 | Missing ToUnicode → CER fail | High | ActualText; easy_15 gate; FFFD policy |
| XR3 | Multi-column order wrong | Med | structure prefer; basic column detect |
| XR4 | Lattice FP on charts | Med | fill_rate + text density |
| XR5 | Stream FP on prose | Med | separation variance gates |
| XR6 | Inline image EI scan errors | Med | fixtures + fuzz |
| XR7 | Form resource shadowing bugs | Med | inheritance tests |
| XR8 | Hidden text double extract | Low | warnings; consumer filters |
| XR9 | JPX/JBIG2 gaps | Med | warnings; phase matrix |
| XR10 | Table multi-page miss | Med | documented limit v0 |
| XR11 | ObjStm bugs lose objects | High | goldens + fuzz |
| XR12 | Backend decode bypass | Critical | decode-guard tests K16 |

---

## Key Decisions

Product decisions **K1–K36** remain authoritative (see product design). Extraction-specific decisions:

| ID | Decision | Rationale |
|----|----------|-----------|
| **EX1** | Paint-order IR is source of truth; layout reorders non-destructively | Debuggability + fidelity |
| **EX2** | ActualText overrides glyph unicode but never geometry | PDF/UA correctness |
| **EX3** | Space insertion is layout-layer (not Tw fabrication) | Spec separation |
| **EX4** | Table orchestration Structure→Lattice→Stream→Hybrid with NMS | Precision first |
| **EX5** | Axis-aligned lattice only in v0 | Engineering bound |
| **EX6** | Image tables without OCR → skip + warn | Scope honesty |
| **EX7** | Invisible text (Tr=3) extracted by default | OCR-layer utility |
| **EX8** | Form XObjects always inlined into page IR by default | K27; simpler API |
| **EX9** | JS/XFA/signatures: metadata only, never execute/verify | Security |
| **EX10** | `visible_ocg_only` opt-in filter; default keep all tagged | Completeness default |
| **EX11** | Internal `capture_rule_segments` when lattice/hybrid; public `include_paths` independent | Lattice needs rules without forcing Path IR |
| **EX12** | Confidence fields on TextRun mandatory in freeze | Consumer quality |
| **EX13** | Multi-page table stitch not in v0 scoring | Eval simplicity |
| **EX14** | Nested tables depth cap 2 experimental flatten | Complexity control |
| **EX15** | String size caps on annot contents / field values (e.g. 1 MB each) | DoS |
| **EX16** | Reading order prefers structure when tagged & coherent | PDF/UA |
| **EX17** | Export JSON omits experimental keys unless flag | Schema stability |
| **EX18** | Type3 charprocs not required for 0.1 text path | Schedule |
| **EX19** | Hybrid table conf = agreement-aware blend | Reduce FP |
| **EX20** | HiddenTextLayer heuristic warn-only | Don't drop text |
| **EX21** | Table detectors never short-circuit: structure ∪ heuristics + NMS | Mixed tagged/untagged pages |
| **EX22** | IR cache fingerprint includes path/table/rotate flags | Prevent lattice starvation |
| **EX23** | Unknown ops clear operand stack (no arity guess) | Avoid cascade corruption |

---

## Open Questions

### Resolved by product (do not reopen)

Naming `pdfparser`, library-first, text-first, K15 encryption, K16 decode ownership, tables off default, image deferred default, IR allowlist, metrics gates, fonts DAG, etc.

### Closed into normative text (rev 1.1)

| # | Resolution |
|---|------------|
| E-T3 | **Internal** `capture_rule_segments` auto-on when lattice/hybrid (EX11); public `include_paths` stays independent (A7, C4) |
| E-T4 | Artifact excluded from `Page::text` by default; kept in raw elements (C1.4) |
| E-T5 | `max_string_bytes` default 1 MiB (A6 additive limits; EX15) |
| E-T6 | Stream v0: one layout line = one row (C3.3); wrap merge later |
| E-T7 | Structure reading order when coverage ≥ 0.80 (C1.4 step 7) |
| E-T8 | Prefer `"experimental": { "tables": ... }` wrapper for 0.1 JSON (EX17) |

### Still open (extraction-technical)

| # | Question | Recommendation |
|---|----------|----------------|
| E-T1 | Exact CMap package | PR 10-license (product T1) |
| E-T2 | Table corpus license set | manifest at kickoff (product T2) |

---

## PR Plan

Aligned with product PR plan; refined for **feature module** order. **Text-first critical path unchanged (K35).**

| PR | Focus | Phase | Size | Deps |
|----|-------|-------|------|------|
| 01 | Workspace skeleton & CI | P0 | S | — |
| 02 | IR freeze + WarningCode (+ additive codes reserved) | P0 | M | 01 |
| 03 | Governor / LimitKind / errors | P0 | M | 01 |
| 04 | ObjectBackend + lopdf spike + decode-guard | P0 | L | 03 |
| 05 | Owned filters + Predictor + bombs + **ObjStm parse** (`objstm` module, Extends depth, Compressed xref path, decode-guard tests) | P0 | L | 04 |
| 05b | Fuzz open+decode | P0 | S | 05 |
| 06 | Page tree + inheritance + encryption gate | P0 | M | 04–05 |
| 07 | Document/Page API façade | P0 | M | 02, 06 |
| 08 | Content lexer | P1 | M | 01 |
| 08b | Fuzz content | P1 | S | 08 |
| 09 | Graphics/text state VM | P1 | L | 02, 08 |
| 10-license | CMap license gate | P1 | S | 01 |
| 10a | Simple fonts widths+encodings+ToUnicode | P1 | L | 05, 09 |
| 10b | Type0/CID/CMap/Identity | P1 | L | 10a, 10-license |
| 10c | hmtx/CFF widths | P1 | L | 10a |
| 10d | Type3 + ActualText | P1 | M | 10a, 09 |
| 11 | Images XObject + inline | P1 | L | 05, 09 |
| 12 | Form XObject inline | P1 | M | 09, 03 |
| 13 | Path capture | P1/P2 | M | 09 |
| 14 | Page::elements/text + spaces | P1 | L | 07, 10a+, 11, 12 |
| 15 | JSON/text export + thin CLI | P1 | M | 14, 02 |
| 16 | Layout lines/blocks/columns | P2 | L | 14 |
| 17 | Structure tree MCID | P2 | M | 06, 14 |
| 18 | Annots + AcroForm + XFA limit | P2 | M | 07 |
| 19 | OCG tag + visible_only | P2 | S | 09, 14 |
| 20 | Table eval harness | P2 | M | 02 |
| 21 | Structure tables | P2 | M | 17, 20 |
| 22 | Lattice tables | P2 | L | 13, 16, 20 |
| 23 | Stream + hybrid | P2 | L | 16, 20 |
| 24 | Tables CSV/MD export | 0.2 | S | 15, 21–23 |
| 25 | Operator coverage tool | P1 | S | 08, 14 |
| 27 | Adversarial expansion | P0/P1 | M | 05, 07 |
| 28 | Golden corpus runner | P1 | M | 14–15 |
| 29 | Benches + RSS | P1 | M | 14 |
| 30 | Oracle pdfium-diff | P1 | M | 14 |
| 31a–c | Encryption matrix | 0.2 | L | 06 |
| 32–33 | CCITT / JBIG2 optional | 0.2 | L | 11 |
| 34 | inspect CLI | 0.1.1 | M | 15, 10a |
| 35 | 0.1.0 polish | P3 | M | 15, 27–29 |

```text
Critical path 0.1:
01→02→03→04→05→05b→06→07
       ↘08→08b→09→10a→10-license→10b/10c/10d→11→12→14→15→35
Feature side paths: 13→16→22; 17→21; 18; 19; 20 harness early
```

**Note:** There is no PR 26 (product absorbed former PR 26 into **10-license**). ObjStm ownership tests live in **PR 05** checklist: unit parse N/First, Extends depth, Compressed xref → core decode, backend decode-guard.

---

## Rollout Plan (extraction features)

| Version | Extraction surface |
|---------|-------------------|
| **0.1.0** | Text+metrics, images encoded, basic layout/order, structure walk, annots/forms meta, OCG tags, tables **off**, experimental module present but disabled default |
| **0.2.0** | Tables opt-in stable targets, encryption subset, CCITT, richer structure, multi-page table flags |
| **1.0.0** | Full CER/bbox gates; freeze expanded schema; table metrics gates |

Feature flags: product table (`serde`, `image-decode`, `parallel`, `mmap`, `trace-ops`, `debug-content`, `jbig2-ffi`, `oracle-pdfium`).

---

## Observability

See E3 and product Observability. Extraction-specific dashboards: CER trends, warning code histogram, table F1 by method, unknown operator frequency.

---

## References

- ISO 32000-1:2008 / ISO 32000-2:2020 (esp. §7 file structure, §8 graphics, §9 text, §14 structure)
- PDF Association — Tagged PDF / PDF/UA
- Product design: `docs/design-native-pdf-parser.md` rev 2.2
- Camelot (lattice) / Tabula (stream) paradigms
- TEDS / table recognition evaluation literature
- Adobe Glyph List (for Differences)
- `lopdf`, `pdf` (pdf-rs), Pdfium (oracle only)
- Rust: `cargo fuzz`, `cargo audit`, `zeroize`, `tracing`

---

## Appendix F1 — Text positioning reference implementation sketch

Matches **B6** matrix convention: storage `[a,b,c,d,e,f]`; points are **row vectors**; `p' = p × M`;
`mat_mul(A,B)` computes `A × B` (B applied first). Glyph placement: `p_user = p_text × Tm × CTM`
(implemented as apply Tm then CTM). After each horizontal glyph: `Tm := [1 0 0 1 adv 0] × Tm`.

```rust
fn apply_mat(m: [f32; 6], x: f32, y: f32) -> (f32, f32) {
    (m[0] * x + m[2] * y + m[4], m[1] * x + m[3] * y + m[5])
}

/// A × B (B first) — identical to B6 `cm` concatenation formulas.
fn mat_mul(a: [f32; 6], b: [f32; 6]) -> [f32; 6] {
    [
        a[0] * b[0] + a[1] * b[2],
        a[0] * b[1] + a[1] * b[3],
        a[2] * b[0] + a[3] * b[2],
        a[2] * b[1] + a[3] * b[3],
        a[4] * b[0] + a[5] * b[2] + b[4],
        a[4] * b[1] + a[5] * b[3] + b[5],
    ]
}

fn show_horizontal(state: &mut GraphicsState, font: &LoadedFont, bytes: &[u8], ir: &mut IrBuilder) {
    let fs = state.text.font_size;
    let th = state.text.horizontal_scale / 100.0;
    for code in font.decode_bytes(bytes) {
        let w = font.width(code);
        let adv = (w / 1000.0) * fs * th
            + state.text.char_spacing
            + if font.is_space_for_tw(code) { state.text.word_spacing } else { 0.0 };
        let (asc, desc) = font.ascent_descent();
        let y0 = desc / 1000.0 * fs;
        let y1 = asc / 1000.0 * fs;
        let mut xs = [0.0f32; 4];
        let mut ys = [0.0f32; 4];
        for (i, (x, y)) in [(0.0, y0), (adv, y0), (0.0, y1), (adv, y1)].into_iter().enumerate() {
            let (tx, ty) = apply_mat(state.text.text_matrix, x, y);
            let (ux, uy) = apply_mat(state.ctm, tx, ty);
            xs[i] = ux;
            ys[i] = uy;
        }
        ir.push_glyph(
            font.unicode(code).unwrap_or(ir.unknown_glyph),
            code,
            Rect::from_minmax(&xs, &ys),
            font,
        );
        // Tm := translate(adv,0) × Tm
        state.text.text_matrix = mat_mul([1.0, 0.0, 0.0, 1.0, adv, 0.0], state.text.text_matrix);
    }
}
```

Golden: fixture `matrix_glyph_1` (B6 worked example) must pass on this path (3-decimal f32).

## Appendix F2 — Lattice segment snap sketch

```rust
fn snap_axis(values: &[f32], tol: f32) -> Vec<f32> {
    let mut v = values.to_vec();
    v.sort_by(|a,b| a.partial_cmp(b).unwrap());
    let mut out = Vec::new();
    for x in v {
        if out.last().map(|y| (x - y).abs() > tol).unwrap_or(true) {
            out.push(x);
        } else {
            // optional average into cluster
        }
    }
    out
}
```

## Appendix F3 — Glossary (extraction)

| Term | Meaning |
|------|---------|
| Paint order | Order of content operators |
| Reading order | Human consumption order |
| Lattice table | Ruled grid detection |
| Stream table | Whitespace column detection |
| MCID | Marked content ID for structure |
| Deferred image | Bytes not expanded to RGBA |
| ObjStm | Object stream (compressed objects) |
| Governor | Resource limit accountant |

## Appendix F4 — Phase tag legend

| Tag | Meaning |
|-----|---------|
| **P0** | Foundation before text quality |
| **0.1** | Ship in 0.1.0 |
| **0.2** | Post-0.1 |
| **later** | Backlog |

---

## Appendix F5 — Capability × phase matrix (extraction)

| Capability | 0.1 | 0.2 | later |
|------------|-----|-----|-------|
| Unencrypted open | Full | Full | Full |
| Encrypted open | Error | Subset | Broader |
| Text metrics+unicode | Full common | Vertical better | Complex scripts |
| Images | Meta+encoded | CCITT | JBIG2 opt |
| Paths | Capture | Lattice solid | Curves |
| Structure | Walk | Polish | UA certify |
| Tables | Off default exp | Opt-in gates | DL optional |
| Annots/forms | Extract | Richer | — |
| OCG | Tag/filter | — | — |
| OCR | — | — | Track |

---

## Appendix F6 — Revision history (Volume 2)

| Rev | Date | Notes |
|-----|------|-------|
| 1.2 | 2026-07-10 | Replaced all Mermaid diagrams with monospaced ASCII architecture diagrams for IDE visibility |
| 1.1 | 2026-07-10 | Design review: table orchestration no short-circuit; path/cache fingerprint; K23 image policy; XRefEntry product alignment; op arity/EI/matrix math; additive ResourceLimits; hybrid xref merge; deepen C3.5–C3.11 and C9–C12; stream/reading-order params; extract helpers; TableModeSet; annot model; confidence terms; closed E-T3–E-T8 |
| 1.0 | 2026-07-10 | Initial Volume 2 draft |

---

*End of Volume 2 — Architecture & Feature Extraction Design — Status: Draft (rev 1.2) — 2026-07-10*

*Companion to: Native PDF Parser Product Design rev 2.3*
