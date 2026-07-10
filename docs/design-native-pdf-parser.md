# Native PDF Parser (Rust) — Product Design Document

| Field | Value |
|-------|-------|
| **Title** | Native PDF Parser — Architecture & Product Design |
| **Author** | TBD |
| **Date** | 2026-07-10 |
| **Status** | Draft (rev 2.3 — ASCII architecture diagrams for IDE readability) |
| **Product / publish name** | **`pdfparser`** (crates.io; fallback `pdfparser-native` only if name taken) |
| **Primary audience** | **Rust library embedders** |
| **Primary product surface** | Idiomatic Rust crate API + JSON export |
| **CLI** | Secondary (validation/tooling), not the primary product |
| **Scope priority** | **Text extraction first** (unencrypted digital → metrics text + images + library/JSON; layout/tables later; no early pull of tables/encryption) |
| **Language** | Rust (edition 2021+) |
| **API style** | Synchronous; no async runtime in v1 |
| **Workspace** | `/Users/gautamkumar/Desktop/pdfParser` (greenfield) |
| **Assumed staffing** | **1–2 full-time engineers** for P0–P3; calendar dates are **illustrative**, not commitments |

---

## Overview

This document specifies the product architecture for a **native, library-first PDF parser written in Rust**, published as the **`pdfparser`** crate. The **primary audience is Rust library embedders**: the product surface is an idiomatic crate API plus structured **JSON export**. The optional CLI is **secondary** (validation, batch tooling, golden/oracle workflows)—not the primary product.

**Scope priority (user decision, final):** **text extraction first**. Critical path stays: unencrypted digital PDFs → metrics-correct text + images + library/JSON (and secondary text CLI) → then layout/tables. **Do not** pull tables or encryption earlier than that path already specifies.

The product’s mission is to open digital (born-digital / vector-text) PDF files and extract a comprehensive, structured representation of their contents: text with geometry and fonts, images, vector graphics, annotations, form fields, optional content, structure tree (when present), and higher-level constructs such as tables inferred from layout (post-text phases).

“Native” means we parse the PDF file format ourselves (ISO 32000 object model, streams, content operators) using pure Rust or carefully chosen low-level crates—not by shelling out to Python toolchains or commercial cloud OCR APIs as the primary path. Phase 1 explicitly targets **digital PDFs** where text and graphics are encoded as operators and resources; scanned image-only PDFs and OCR are deferred.

The design favors a **layered pipeline**: tokenize/parse objects → resolve cross-references → **decode streams only through our Resource Governor** → interpret content streams into a page Intermediate Representation (IR) → run layout analysis and structure extraction → expose results primarily through a versioned, idiomatic **Rust library API** and **JSON export**, with optional CLI as a thin consumer of the same API.

We reuse a mature pure-Rust object **syntax/xref** backend (initially `lopdf`, evaluated against `pdf`/pdf-rs) **only for encoded object access**. All stream filter decoding, expansion accounting, content interpretation, font metrics, layout/tables, and the public API are **owned product code**. Security-critical paths never call third-party “decoded stream” APIs.

**0.1.0 scope (explicit):** **unencrypted PDFs only**. Presence of `/Encrypt` in the trailer (or encryption dictionary reachable from the trailer) → **`Error::Encryption`**, including empty-user-password and owner-only files—**no exceptions in 0.1**. Solid L0–L2; reading-order text + images; experimental layout; tables **off by default**. OCR, full rendering, PDF writing, XFA execution, and JS are out of scope. Empty-user-password open is **0.2+ only** (with the crypto matrix).

---

## Background & Motivation

### Why PDF parsing is hard

PDF is a **presentation format**, not a document data model. A page is a sequence of painting operators (draw text at transform *T*, stroke path *P*, paint image *I*). Semantic notions such as “paragraph,” “table cell,” or “reading order” are often absent unless the document is **Tagged PDF** (structure tree / Marked Content). Consequently:

- Text may be encoded as glyph IDs with custom ToUnicode maps, CID fonts, or Type3 fonts.
- Correct **bounding boxes** require font **width metrics** independent of Unicode mapping.
- Logical reading order may differ from paint order (multi-column, absolute positioning).
- Tables are usually visual: ruled lines + aligned text runs, not first-class objects.
- Incremental updates, hybrid xref tables, object streams, encryption, and malformed producers are common in the wild.
- The format has a long history of being abused for denial-of-service (zip-bomb streams, object cycles, malicious streams, XFA XML bombs).

### Current state of the workspace

The workspace at `/Users/gautamkumar/Desktop/pdfParser` is empty. This is a greenfield product: we define crate layout, API surface, dependency policy, test corpus strategy, and an incremental PR plan before implementation.

### Ecosystem snapshot (as of design date 2026-07-10; re-validate at kickoff)

| Approach | Examples | Strengths | Weaknesses for this product |
|----------|----------|-----------|------------------------------|
| Pure-Rust object layer | `lopdf` | Zero FFI; solid for structural ops | Text/layout not product-grade; real-world open-rate **unknown until spike** (do not trust informal percentages) |
| Pure-Rust lazy/object model | `pdf` (pdf-rs lineage) | Lazy parsing philosophy; different API shape | Maintenance/maturity vary; same need to own decode + extract |
| Pure-Rust extract helpers | `pdf-extract`, community extractors | Quick text dump | Weak layout IR; incomplete metrics |
| C++ engines via FFI | `pdfium-render`, MuPDF bindings | High fidelity, rendering, resilience | Not native parse ownership; large binary; sandboxing harder |
| Commercial/cloud | OCR/VLM APIs | Strong on scans & tables | Violates native-first constraint; cost/privacy |

**Version pins** in this document (e.g. illustrative `lopdf`) are **not normative**. Kickoff includes a lockfile spike PR; re-evaluate crate versions and open-rates on corpus samples before freezing K3 backend choice.

**Motivation for building:** existing pure-Rust crates stop short of a **unified element IR**, **metrics-correct text geometry**, **table inference**, **structure-aware export**, and **security-hardened resource limits** as a coherent product. Our value is owning the pipeline from bytes → structured elements with clear correctness contracts and a versioned API.

---

## Goals & Non-Goals

### Goals

1. **Parse digital PDFs** conforming to common ISO 32000-1/2 features used in **unencrypted** production documents (PDF 1.4–2.0 subset; see capability matrix). Encrypted support is post-0.1 (see K15).
2. **Extract first-class and reconstructible elements** into a unified IR: text runs (with bbox, font, unicode, metrics confidence), images, paths/graphics, annotations, form fields, Form XObjects (inlined into page elements by default), optional content, bookmarks/outline, metadata, and structure tree when present.
3. **Table extraction** (post-text path; **experimental / off by default in 0.1**) for untagged digital PDFs via layout heuristics (lattice + stream), with confidence scores; structure-tree tables when available.
4. **Library-first for embedders:** idiomatic **sync** Rust API + serde JSON export are the **primary product surface**; CLI is secondary tooling only (K1, K34).
5. **Security by default**: resource limits with hard maxima, cycle detection, stream expansion only in our filter pipeline, no panics on malformed input, process-isolation guidance for multi-tenant embedders.
6. **Testability**: golden corpus, adversarial suite, early fuzz, specification coverage matrix, table evaluation protocol, optional Pdfium oracle workflow.
7. **Performance**: page-at-a-time extraction; bounded memory; defined reference machine for SLOs.
8. **Phased roadmap** implementable by 1–2 engineers without inventing critical architecture mid-flight.
9. **Honor scope priority:** ship metrics-correct **text + images** for unencrypted digital PDFs via the library/JSON path before investing in tables or encryption (K35).

### Product positioning (user decisions — final)

| Decision | Choice | Implication |
|----------|--------|-------------|
| **Name** | **`pdfparser`** | Workspace + crates.io publish name; fallback `pdfparser-native` only if taken at reserve time |
| **Primary audience** | Rust **library embedders** | Design API for embedding (`Document`/`Page`/`extract`); docs lead with library examples |
| **Primary surface** | Crate API + **JSON export** | IR/schema quality and rustdoc are first-class; CLI must not drive API design |
| **CLI role** | Secondary | Validation, corpus tooling, operator inspect; thin wrapper over library |
| **Scope priority** | **Text extraction first** | Path: unencrypted digital → text/images library+JSON → layout → tables experimental → encryption 0.2. **No** advancing tables or encryption ahead of text-critical path |

### Non-Goals (v1 / 0.1 unless noted)

| Non-goal | Rationale |
|----------|-----------|
| Full visual rendering engine (to bitmap) | Different product; oracle-only later |
| OCR / scanned PDF primary path | Phase 2+ / separate product track |
| PDF writing, editing, merge/split product | Focus on parse & extract |
| Encrypted PDF open in 0.1.0 | Explicit: `Error::Encryption` (K15); P1.5/P4 matrix |
| 100% of ISO 32000 edge cases day one | Asymptotic; phased matrix |
| Perfect table extraction on all untagged docs | Heuristic + confidence |
| JavaScript / AcroForm calculation execution | Extract field values/widgets only |
| XFA form rendering/execution | Reject or size-limit XFA datasets; no XFA UI |
| Digital signature cryptographic validation | Metadata only |
| Cloud APIs as required dependency | Forbidden as primary path |
| Async API / WASM target in v1 | Sync library first; WASM explicit non-goal until reopened |
| Shipping C codec deps (JBIG2) in default features | Optional, default-off, process-isolated recommendation |

---

## Product Capabilities & Phased Roadmap

### What “parse any PDF” realistically means

| Level | Meaning |
|-------|---------|
| **L0 — Open** | Header, trailer, xref (table + stream), object graph load with limits |
| **L1 — Resources** | Pages tree, fonts (subset), XObjects (image/form), color spaces metadata |
| **L2 — Content IR** | Content stream → **metrics-positioned** text runs, images, paths, ActualText |
| **L3 — Layout** | Lines, blocks, reading order, columns (heuristic), space reconstruction |
| **L4 — Structure** | Structure tree / Marked Content roles when present |
| **L5 — Semantics** | Tables, lists, form field values, annotation content |

**0.1.0 product bar:** L0–L2 solid on unencrypted digital PDFs; L3 basic (lines + reading order); L4–L5 best-effort / experimental. **1.0 GA bar:** L0–L2 solid, L3 good, L4–L5 with measured table metrics and encryption subset.

### Feature complexity matrix

| Feature | Spec area | Target | Notes |
|---------|-----------|--------|-------|
| PDF header 1.0–2.0 | File structure | 0.1 Full | Tolerate nonstandard headers where safe |
| Classic xref table | File structure | 0.1 Full | Via backend; contract in ObjectBackend |
| XRef stream | PDF 1.5+ | 0.1 Full | |
| Hybrid xref / incremental updates | File structure | 0.1 Full | Prefer newest generation |
| Object streams | PDF 1.5+ | 0.1 Full | |
| Linearized PDF | Hint tables | 0.1 Open w/o hints | |
| FlateDecode + **Predictor** (1–15) | Filters | **0.1 Full** | PNG/TIFF predictors in **our** pipeline |
| ASCII85, ASCIIHex, LZW, RunLength | Filters | 0.1 Full | Cap expansion + chain length |
| DCTDecode (JPEG) | Image filters | 0.1 Extract stream / optional decode | |
| JPXDecode | Image filters | Best-effort / warn | `UnsupportedFilter` warning |
| CCITTFaxDecode | Image filters | 0.2+ pure Rust preferred | |
| JBIG2Decode | Image filters | Optional feature, **default off** | C codec isolated; see Security |
| Crypt filter | Filters | Post-auth only; post-0.1 | Never decode ciphertext as content |
| Standard security handler | Encryption | **Out of 0.1** (any `/Encrypt` → error); 0.2 subset incl. empty user password | See K15 / encryption section |
| Public-key encryption | Encryption | Later | |
| Type1 / TrueType widths + ToUnicode | Fonts | 0.1 Full common | Metrics required for bbox |
| Type0 / CIDFontType0/2 + CMap | Fonts | 0.1 Core paths | Identity-H + embedded CMaps |
| Type3 fonts | Fonts | 0.1 Geometry + fallback unicode | Charprocs optional paths |
| Content text ops + **TJ adjustments** | Content | 0.1 Full | Tw/Tc/Th + TJ numerics |
| **Inline images** `BI/ID/EI` | Content | 0.1 Full | Same limits as XObject images |
| **ActualText** (Span properties) | Content/Structure | 0.1 Full | Prefer ActualText over glyph decode when present |
| **Space insertion heuristic** | Layout/Text | 0.1 Full | When Tw=0 and no space glyphs |
| Path ops, clip, fill/stroke | Content | 0.1 Capture | Not full rasterizer |
| Do (XObject) | Content | 0.1 Image + Form depth-capped | Forms **inlined** into page IR by default |
| gs, ExtGState, transparency | Content | Record state | Limited blend semantics |
| Shading / Pattern | Content | Metadata + skip | `UnsupportedPaint` warning |
| Annotations | Annots | 0.1 Extract | No JS |
| AcroForm fields | Forms | 0.1 Values/widgets | No calculate |
| **XFA** datasets | Forms | **Reject/limit** | Max XFA bytes; no execute; warning |
| Optional Content (OCG) | Layers | 0.1 Tag + filter option | |
| Tagged PDF / StructTreeRoot | Structure | 0.1 Walk + map MCID | |
| Table inference (untagged) | Layout | **0.2 experimental→stable** | Off by default in 0.1 |
| Multi-column reading order | Layout | 0.1 Basic | |
| Vertical writing / CJK | Layout | 0.2 if fonts OK | |
| Separation / DeviceN / ICC | Color | Metadata; no CMS in 0.1 | |

### Staffing & schedule assumptions

| Assumption | Value |
|------------|-------|
| Team size | **1–2 engineers** (one generalist + optional part-time) |
| Parallelism | Content VM and export/CLI can parallelize after IR + core open; fonts are critical path for L2 quality |
| Gantt dates below | **Illustrative** capacity planning only; slip fonts → slip layout/tables |
| Review | Each PR: 1 reviewer minimum; security-sensitive (filters, crypto, limits): 2 reviewers preferred |

```text
══════════════════════════════════════════════════════════════════════════════
         PHASE ROADMAP (illustrative, 1–2 engineers — not a commitment)
══════════════════════════════════════════════════════════════════════════════

  2026-07        2026-09        2026-11        2027-01        2027-03
     │              │              │              │              │
     ▼              ▼              ▼              ▼              ▼

  ┌────────────────────┐
  │ P0 FOUNDATION      │  backend + governor + filters (~7w)
  │                    │  early fuzz smoke (~2w from Aug)
  └─────────┬──────────┘
            ▼
  ┌────────────────────┐
  │ P1 EXTRACTION CORE │  content VM + simple fonts (~6w)
  │                    │  CID + TT widths + ActualText (~6w)
  │                    │  library/JSON text path (~3w)  ◄── first usable product
  └─────────┬──────────┘
            ▼
  ┌────────────────────┐
  │ P2 LAYOUT/SEMANTICS│  layout + structure (~5w)
  │                    │  tables experimental + eval (~6w)
  │                    │  annots / forms / OCG (~3w)
  └─────────┬──────────┘
            ▼
  ┌────────────────────┐
  │ P3 0.1.0 POLISH    │  docs, SLOs, known limitations (~4w)
  └─────────┬──────────┘
            ▼
  ┌────────────────────┐
  │ P4 POST-0.1        │  encryption subset + hard image filters (~8w)
  └────────────────────┘
```

| Phase | Deliverables | Exit criteria (measurable) |
|-------|--------------|----------------------------|
| **P0** | Backend trait, governor, **our** filters (incl. Predictor), page tree, early fuzz | 0 panics on adversarial suite; bombs → `LimitExceeded`; open-rate spike report on ≥100 public PDFs |
| **P1** | Content VM, font metrics pipeline, images, inline images, ActualText, space heuristic, **text CLI** | **Release-blocking:** 0 panics; metrics fixtures run. **Reported (not 0.1-blocking):** CER/IoU on digital_text_50 / metrics_20—see Metrics gate matrix |
| **P2** | Layout, structure tree, tables experimental, forms/annots | Table cell-F1 reported on labeled set (no absolute gate for 0.1); structure tables pass golden fixtures |
| **P3** | `0.1.0` API freeze subset, docs, perf SLOs | SLOs on reference machine; Appendix B metrics |
| **P4** | Encryption R=2/3/4 user-password; CCITT; optional JBIG2 feature | Encrypted fixture matrix; security checklist |

---

## Proposed Design

### High-level architecture

```text
══════════════════════════════════════════════════════════════════════════════
                    HIGH-LEVEL ARCHITECTURE — pdfparser
══════════════════════════════════════════════════════════════════════════════

  INPUT
  ┌────────────────────────────┐
  │  PDF bytes / path / reader │
  └──────────────┬─────────────┘
                 │
                 ▼
  ┌──────────────────────────────────────────────────────────────────────────┐
  │  L0  OBJECT MODEL                                                        │
  │                                                                          │
  │   ObjectBackend ──▶ XRef ──▶ Object Store ──▶ Owned filters + Predictor  │
  │        ▲                         ▲                      ▲                │
  │        └────────── Resource Governor (limits, cycles) ──┘                │
  └──────────────────────────────────┬───────────────────────────────────────┘
                                     │ decoded objects / streams
                                     ▼
  ┌──────────────────────────────────────────────────────────────────────────┐
  │  L1  DOCUMENT MODEL                                                      │
  │   Catalog / Pages tree  ·  Resource resolver  ·  Fonts  ·  XObjects      │
  └──────────────────────────────────┬───────────────────────────────────────┘
                                     │ page content + resources
                                     ▼
  ┌──────────────────────────────────────────────────────────────────────────┐
  │  L2  CONTENT INTERPRETER                                                 │
  │   Content stream parser  ──▶  Graphics/text state VM  ──▶  Page IR       │
  └──────────────────────────────────┬───────────────────────────────────────┘
                                     │ paint-order elements
                     ┌───────────────┼───────────────┐
                     ▼               ▼               ▼
  ┌──────────────────────┐ ┌──────────────┐ ┌────────────────────┐
  │ L3 LAYOUT            │ │ STRUCTURE    │ │ ANNOTS / FORMS     │
  │ spaces · lines ·     │ │ tree / MCID  │ │ OCG tags           │
  │ reading order        │ └──────────────┘ └────────────────────┘
  │ table detectors (off │
  │ by default in 0.1)   │
  └──────────┬───────────┘
             │
             ▼
  ┌──────────────────────────────────────────────────────────────────────────┐
  │  PUBLIC SURFACE                                                          │
  │   pdfparser crate API  ──▶  JSON / export                                │
  │         │                                                                │
  │         └──▶  pdfparser-cli  (secondary; thin wrapper)                   │
  └──────────────────────────────────────────────────────────────────────────┘
```

### Crate layout (Cargo workspace) — cycle-free DAG

```
pdfParser/
├── Cargo.toml
├── crates/
│   ├── pdfparser-ir/          # public IR + WarningCode (stable-ish)
│   ├── pdfparser-core/        # backend trait, governor, filters, page tree, annots
│   ├── pdfparser-fonts/       # pure font logic: descriptors in, metrics/unicode out
│   ├── pdfparser-content/     # content lexer/VM
│   ├── pdfparser-layout/      # clustering, spaces, tables, structure map
│   ├── pdfparser-export/      # JSON/text/md/csv
│   ├── pdfparser/             # public façade (only published API crate for 0.1)
│   └── pdfparser-cli/         # binary
├── fuzz/
├── corpus/                    # manifests + hashes in git
├── schemas/
│   └── extract-v1.json
└── docs/
```

**Resolved dependency DAG (no cycles; arrows = “depends on”):**

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

  fonts is a leaf: depends on ir only; never imports core (bytes-in API).
```

**Type ownership (normative for PR 01):**

| Type / concern | Crate | Notes |
|----------------|-------|-------|
| `Rect`, `ObjectId`, `Color`, `Matrix3x2`, public IR elements, `WarningCode` | `pdfparser-ir` | No dependency on core/fonts; **no** `PdfObject`/`PdfDict` here |
| `PdfObject`, `PdfDict`, `PdfString`, `EncodedStream`, `XRefEntry`, `BackendError`, `ObjectStore`, ObjStm parser | **`pdfparser-core`** | Low-level PDF syntax model; **not** in 0.1 public freeze list; not re-exported from façade by default |
| `ObjectBackend`, `ResourceGovernor`, filters, page tree | `pdfparser-core` | Depends on fonts to **build** `LoadedFont` from extracted bytes |
| `FontInput`, `LoadedFont`, `FontMetrics`, `CMap`, `ToUnicode` | `pdfparser-fonts` | **Leaf inputs are plain data** (`&[u8]`, owned structs)—never imports `pdfparser-core` |
| Content VM | `pdfparser-content` | Calls `fonts` for decode+advance; `core` for stream decode via governor |
| Public crates.io | **`pdfparser` only** in 0.1 | Other crates path-deps / `publish = false` until 0.2; core types stay internal |

**Font API direction (push pure data in):**

```rust
// pdfparser-fonts — no core imports
pub fn load_font(input: FontInput<'_>) -> Result<LoadedFont, FontError>;

pub struct FontInput<'a> {
    pub subtype: FontSubtype,           // Type1, TrueType, Type0, Type3, …
    pub descriptor: FontDescriptorData, // flags, bbox, ascent, descent, …
    pub widths_simple: Option<SimpleWidths>,      // FirstChar/LastChar/Widths
    pub cid_widths: Option<CidWidthArray>,          // W / DW / W2 / DW2
    pub to_unicode_cmap_bytes: Option<&'a [u8]>,
    pub encoding_name: Option<String>,
    pub differences: Option<Vec<(u8, String)>>,
    pub embedded_program: Option<&'a [u8]>, // font file stream **already decoded by core**
    pub descendant: Option<Box<FontInput<'a>>>,     // Type0 → CIDFont
    pub cmap_name_or_bytes: Option<CMapSource<'a>>,
}
```

Core extracts encoded streams → **governor decode** → passes bytes into `load_font`. Fonts never call the object store.

---

### ObjectBackend trait & stream ownership (normative)

**K16 — Encoded-stream-only backend access.** The product **never** relies on third-party decoded stream getters for security-critical paths. All `/Filter` / `/DecodeParms` processing runs in `pdfparser-core::filters` under `ResourceGovernor`.

```rust
/// Object identity (generation included).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ObjectId { pub obj: u32, pub gen: u16 }

/// Backend supplies **syntax + xref + encoded bytes only**.
/// Implementors: LopdfBackend (initial), PdfRsBackend (spike), InMemoryFixtureBackend (tests).
pub trait ObjectBackend: Send + Sync {
    /// PDF version string if known (e.g. "1.7").
    fn pdf_version(&self) -> Option<&str>;

    /// Trailer dictionary (`PdfDict` is a **core** object-model type—not public IR).
    fn trailer(&self) -> Result<PdfDict, BackendError>;

    /// Resolve xref: return where an object lives (file offset or object-stream home).
    fn xref_entry(&self, id: ObjectId) -> Result<XRefEntry, BackendError>;

    /// Number of objects known to xref (upper bound for governor).
    fn xref_len(&self) -> u32;

    /// Load an object **without** decoding stream filters.
    /// Streams must appear as `PdfObject::Stream(EncodedStream { dict, encoded_bytes })`.
    fn get_encoded(&self, id: ObjectId) -> Result<PdfObject, BackendError>;

    /// Convenience: catalog ref from trailer `/Root`.
    fn root(&self) -> Result<ObjectId, BackendError>;

    /// Optional: raw file slice for debugging (not required for extract).
    fn file_len(&self) -> u64;
}

pub enum XRefEntry {
    Free,
    Uncompressed { offset: u64 },
    Compressed { stream_id: ObjectId, index: u32 },
}

pub struct EncodedStream {
    pub dict: PdfDict,
    /// Exact encoded payload; filters NOT applied.
    pub encoded_bytes: Bytes,
}

pub enum PdfObject {
    Null,
    Bool(bool),
    Int(i64),
    Real(f64),
    Name(String),
    String(PdfString),
    Array(Vec<PdfObject>),
    Dict(PdfDict),
    Stream(EncodedStream),
    Ref(ObjectId),
}

// All of the above live in `pdfparser-core` (module `object` / `backend`).
// `ObjectId` is defined in `pdfparser-ir` and re-used by core to avoid dual IDs.

#[derive(Debug, thiserror::Error)]
pub enum BackendError {
    #[error("object {0:?} not found")]
    NotFound(ObjectId),
    #[error("syntax: {0}")]
    Syntax(String),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("unsupported backend feature: {0}")]
    Unsupported(String),
}
```

#### lopdf adapter contract

| Operation | How |
|-----------|-----|
| XRef / trailer / incremental | Delegated to lopdf parse **if** spike shows acceptable open-rate; else fork. Backend exposes **xref entries only** (offsets / Compressed homes)—not decoded objects. |
| `get_encoded` for **uncompressed** objects | Use lopdf APIs / file-slice from xref offset that yield **raw** object bytes or parsed non-stream objects / **encoded** streams. **Forbidden:** any path that auto-applies filters. |
| `get_encoded` for **ObjStm streams** | Return `PdfObject::Stream(EncodedStream { dict, encoded_bytes })` for the object-stream **container** only—same as any other stream. |
| Object streams (compressed objects) | **Not unpacked in the backend.** See normative ObjStm algorithm below—**core owns decode + parse**. |
| If lopdf cannot expose raw streams | **Vendor/fork lopdf** (K17) in `third_party/lopdf` and patch; tracked as explicit PR—not an open question |

**PR 05 replaces decoding for product paths:** even if lopdf has decode helpers, production `decode_stream(gov, EncodedStream)` is **only** `pdfparser-core::filters`. Integration tests assert a malicious Flate bomb never grows past governor caps (instrument adapter to fail if third-party decode is invoked—feature `backend-decode-guard` in debug).

#### Object stream (ObjStm) resolution — normative algorithm (K16)

**Ownership:** Object-stream **body parsing and filter decoding live entirely in `pdfparser-core`**. The backend never returns pre-decoded stream payloads and never materializes compressed-object children by calling lopdf’s full object graph APIs.

When the product needs object `id` whose xref entry is compressed:

```text
1. entry = backend.xref_entry(id)
   → XRefEntry::Compressed { stream_id, index }

2. obj = backend.get_encoded(stream_id)
   → must be PdfObject::Stream(EncodedStream)   // still encoded

3. decoded = core::filters::decode_stream(governor, &encoded)
   // applies /Filter + /DecodeParms (incl. Predictor); counts expanded bytes
   // Encrypted PDFs never reach here in 0.1 (K15)

4. objects = core::objstm::parse(decoded, dict)
   // read N, First; parse header of object numbers + offsets;
   // slice and parse PDF objects at each offset under size/count limits
   // (max objects in one ObjStm, max decoded ObjStm size = stream limits)

5. result = objects[index]   // PdfObject; if that child is itself a Stream,
                             // its bytes are the child's encoded payload
                             // (nested filters applied only when that stream is later decoded)

6. Optional: ObjectStore cache (id → PdfObject) inside core—never inside backend
```

**Invariants:**

| Rule | Requirement |
|------|-------------|
| Backend `get_encoded` | Encoded bytes only for every `Stream` |
| Who calls `decode_stream` | **Only** `pdfparser-core` (or content layer via core helpers) |
| Who parses ObjStm body | **`pdfparser-core::objstm`** |
| lopdf adapter | Must not call lopdf “get decompressed object” for compressed IDs |
| Governor | ObjStm decode counts toward `max_expanded_stream_bytes` / total budget |

`get_encoded(id)` for a **compressed** object id is **not required** on the trait. Callers use `ObjectStore::get(id)` in core, which implements the algorithm above. Trait method remains: xref + encoded containers + uncompressed objects.

#### Stream decode pipeline (owned)

```text
EncodedStream
  → parse /Filter (name or array) + /DecodeParms
  → max chain length (hard max 8)
  → for each filter:
        check remaining budget (max_expanded_stream_bytes, max_total_expanded_bytes, ratio)
        apply filter
        if Flate/LZW and Predictor in DecodeParms → apply predictor (1–15)
  → return DecodedBytes (not cached by default; see Memory lifecycle)
```

---

### Resource Governor (normative constants)

```rust
pub struct ResourceLimits {
    pub max_file_bytes: u64,
    pub max_objects: u32,
    pub max_expanded_stream_bytes: u64,
    pub max_total_expanded_bytes: u64,
    pub max_filter_expansion_ratio: u32,
    pub max_filter_chain_len: u32,
    pub max_nesting_depth: u32,
    pub max_pages: u32,
    pub max_content_ops_per_page: u32,
    pub max_image_pixels: u64,
    pub max_xfa_bytes: u64,
    pub max_embedded_font_bytes: u64,
    /// Wall-clock budget for a single page interpret (`elements`/`text`/`layout`).
    /// `None` = disabled (default for CLI interactive use).
    pub max_page_interpret_time: Option<std::time::Duration>,
    /// Wall-clock budget for `Document::open` / `extract` whole-document calls.
    /// `None` = disabled by default; multi-tenant services should set this.
    pub max_open_or_extract_time: Option<std::time::Duration>,
}

/// Hard compile-time ceilings — OpenOptions cannot exceed these in release builds.
pub mod hard_max {
    pub const MAX_FILE_BYTES: u64 = 2 * 1024 * 1024 * 1024; // 2 GiB
    pub const MAX_OBJECTS: u32 = 5_000_000;
    pub const MAX_EXPANDED_STREAM_BYTES: u64 = 256 * 1024 * 1024;
    pub const MAX_TOTAL_EXPANDED_BYTES: u64 = 1024 * 1024 * 1024; // 1 GiB
    pub const MAX_FILTER_EXPANSION_RATIO: u32 = 1000;
    pub const MAX_FILTER_CHAIN_LEN: u32 = 8;
    pub const MAX_NESTING_DEPTH: u32 = 64;
    pub const MAX_PAGES: u32 = 100_000;
    pub const MAX_CONTENT_OPS_PER_PAGE: u32 = 10_000_000;
    pub const MAX_IMAGE_PIXELS: u64 = 200_000_000;
    pub const MAX_XFA_BYTES: u64 = 16 * 1024 * 1024;
    pub const MAX_EMBEDDED_FONT_BYTES: u64 = 64 * 1024 * 1024;
    pub const MAX_PAGE_INTERPRET_TIME_SECS: u64 = 300; // clamp Option values
    pub const MAX_OPEN_OR_EXTRACT_TIME_SECS: u64 = 3_600;
}

impl Default for ResourceLimits {
    fn default() -> Self {
        Self {
            max_file_bytes: 512 * 1024 * 1024,
            max_objects: 2_000_000,
            max_expanded_stream_bytes: 64 * 1024 * 1024,
            max_total_expanded_bytes: 512 * 1024 * 1024,
            max_filter_expansion_ratio: 100,
            max_filter_chain_len: 8,
            max_nesting_depth: 32,
            max_pages: 50_000,
            max_content_ops_per_page: 5_000_000,
            max_image_pixels: 100_000_000,
            max_xfa_bytes: 4 * 1024 * 1024,
            max_embedded_font_bytes: 32 * 1024 * 1024,
            max_page_interpret_time: None,
            max_open_or_extract_time: None,
        }
    }
}
```

Under `parallel`, budgets use **atomic** counters on a shared `Arc<Governor>`; any page that trips a limit returns `Error::LimitExceeded { kind }` for that unit of work (see recovery table).

#### Time budget accounting (normative)

| Field | Clock starts | Clock ends | On exceed |
|-------|--------------|------------|-----------|
| `max_page_interpret_time` | Enter `Page::elements` / `text` / `layout` / `tables` | Return from that call | `LimitExceeded { TimeBudget }` — **hard error** even in Recoverable |
| `max_open_or_extract_time` | Enter `Document::open*` or `extract` | Return from that call | Same |

Implementation: check elapsed at governor checkpoints (after each stream decode, every N content ops, end of page). Cooperative—not a preemptive OS kill (process isolation still recommended for multi-tenant hard walls). Default both `None` (disabled). When `Some(d)`, clamp `d` to `hard_max` ceilings. Maps to `LimitKind::TimeBudget`.


---

### ParseMode recovery policy (normative)

| Failure class | `ParseMode::Strict` | `ParseMode::Recoverable` (default) |
|---------------|---------------------|--------------------------------------|
| I/O / unreadable file | Hard error | Hard error |
| Missing startxref / unreadable trailer | Hard error | Hard error |
| **Any resource limit trip** (stream bomb, ops, pixels, XFA size, time budget, …) | Hard error | **Hard error** (limits always fail closed; never “skip” a bomb or timeout) |
| Single object syntax error | Hard error | Skip object; `WarningCode::ObjectSkipped`; dependents may degrade |
| Page content stream syntax error | Hard error | Skip remainder of page content; page may be partial; `ContentTruncated` |
| Unknown operator | Hard error if `strict_ops` else warn | `UnknownOperator` warning; continue |
| Missing font / ToUnicode | Hard error if configured | Empty/replacement glyphs; `MissingToUnicode` / low confidence |
| Unsupported filter (e.g. JPX) | Hard error | Skip image/stream use; `UnsupportedFilter` |
| Encrypted document (0.1) | `Error::Encryption` | `Error::Encryption` (not recoverable to partial plaintext) |
| XFA present over max size | Hard error | Hard error on that stream; form XFA ignored if under limit with warning |
| Form XObject cycle / depth | Hard error | Skip form; `FormNestingLimit` |
| Annotation malformed | Hard error | Skip annot; warning |

**`extract()` / `page().elements()`:** if a **limit** trips mid-page → return `Err(LimitExceeded)`. Prior complete pages in a multi-page `extract()` may be omitted (fail closed for the call) **or** returned with `partial: true` only when `ExtractOptions::allow_partial_document = true` (default **false**).

**Encryption ordering:** authentication runs before any content stream decode that requires decryption. Failures never return decrypted-looking content objects.

---

### Thread-safety & Page handle model (normative)

| Type | Send | Sync | Notes |
|------|------|------|-------|
| `Document` | yes | yes | Internals: `Arc<DocumentInner>` with backend + governor + page tree |
| `Page` | yes | yes | **Owned snapshot handle**: `Page { doc: Arc<DocumentInner>, index }` — not a short lifetime borrow |
| Input reader | yes | via mutex | `from_reader`: `R: Read+Seek+Send+'static` stored as **`Mutex<R>`** (or backend-owned lock) so `Document: Sync` without requiring `R: Sync` |
| Page IR cache | — | — | `Mutex` or `DashMap` optional LRU inside `DocumentInner`; cache key = `(page, ExtractOptions fingerprint)` |
| `parallel` feature | — | — | `rayon` over page indices; each task clones `Arc<DocumentInner>`; **shared atomic governor**; backend I/O serialized through reader mutex if not fully loaded into memory |

**Rationale:** `Page<'a>` lifetime fights parallel extract and caching; **Arc-based `Page`** is the 0.1 API (K18). **K31:** do not require `Sync` on user readers.

---

### Pipeline stages (detailed)

#### Stage 0 — Load & govern

1. Accept `impl Read + Seek`, `&[u8]`, or feature-gated mmap.
2. Clamp `OpenOptions.limits` to `hard_max::*`.
3. Backend parses xref/trailer (encoded only).
4. **Encryption gate (K15, normative for 0.1):** if trailer contains `/Encrypt` (any non-null encryption dictionary) → return `Error::Encryption` immediately. **Do not** attempt empty-user-password detection, key derivation, or partial open in 0.1.

#### Stage 1 — Object model

1. Uncompressed: `backend.get_encoded` → typed `PdfObject`.
2. Compressed (ObjStm): follow **normative ObjStm algorithm** (core decode + `objstm::parse`); never backend-side unpack.
3. Resolve refs with visited-set; depth limit; `ObjectStore` cache in core.
4. Decode non-ObjStm streams **only** via owned filter pipeline when content/fonts need bytes.
5. Build page tree with inheritance.

#### Stage 2 — Resource binding + fonts

1. Merge page resources.
2. For each font: pull descriptor + encoded font program stream → governor decode → `pdfparser-fonts::load_font`.
3. Images: metadata + encoded bytes; decode pixels only if `ImageOptions::decode_pixels`.

#### Stage 3 — Content interpretation

1. Tokenize content; maintain graphics/text state.
2. Text showing → metrics advances + unicode (or ActualText).
3. `Do` images/forms; `BI/ID/EI` inline images.
4. Paths optional; BDC/EMC for structure/ActualText/OCG.

#### Stage 4 — Layout & structure

1. Space insertion heuristic → lines → blocks → reading order.
2. Structure tree map by MCID (independent of table heuristics).
3. Tables experimental when enabled.

#### Stage 5 — Export

JSON (schema subset), text, markdown (lossy), CSV tables when present.

---

### Font metrics & text positioning (normative — critical path)

Correct `TextRun.bbox` and table IoU require **widths**, not only ToUnicode.

#### Pipeline

```text
══════════════════════════════════════════════════════════════════════════════
                    FONT METRICS & TEXT POSITIONING PIPELINE
══════════════════════════════════════════════════════════════════════════════

  Show-string bytes  (Tj / TJ / ' / ")
           │
           ▼
  Encoding / CMap  ──▶  char code or CID
           │
           ├──────────────────────────────┐
           ▼                              ▼
    Width lookup                   ToUnicode / ActualText
           │                              │
           ▼                              │
    Advance in text space                 │
           │                              │
           ▼                              │
    Apply TJ kern numbers                 │
           │                              │
           ▼                              │
    Apply Tc / Tw / Th                    │
           │                              │
           ▼                              │
    Transform: Tm then CTM                │
           │                              │
           ▼                              ▼
    Axis-aligned bbox              Unicode string
    (unrotated page user space)    (U+FFFD if missing)
```

#### Width data sources by font type

| Font type | Horizontal width source | Fallback |
|-----------|-------------------------|----------|
| Type1 / MMType1 / TrueType (simple) | `/Widths` for `FirstChar..LastChar`; else embedded program (Type1 width array / TT `hmtx`) | `/MissingWidth` from descriptor; else 0 with `metrics_confidence=0` |
| Type0 → CIDFontType2 | CIDFont `/W` array + `/DW` (default 1000); vertical `/W2`/`DW2` when WMode=1 | DW=1000; warn |
| Type0 → CIDFontType0 | `/W`/`DW` + optional CFF widths | DW |
| Type3 | `/Widths` or charproc bbox rough estimate | FontBBox / char count |
| Missing widths entirely | Synthetic uniform width from FontBBox | `metrics_confidence` low; still emit text |

Widths are in **glyph space** (typically 1000 units per text space unit when `font_size` is applied). 

**`Th` storage (normative):** `TextState.horizontal_scale` stores the **raw PDF percent** from `Tz` (default **100**, not 1.0). Advance always uses `Th / 100.0` once—never store both a fraction and a percent.

```text
# horizontal WMode = 0 (0.1 required path)
advance_x_text = (width / 1000.0) * font_size * (Th / 100.0) + Tc + (is_space_for_tw ? Tw : 0)
advance_y_text = 0
```

For each `TJ` array element: strings draw glyphs; **numbers** adjust position by  
`tx -= (number / 1000.0) * font_size * (Th / 100.0)` in horizontal mode (ISO 32000).

After each glyph, update `Tm` translation; glyph quad uses advance + ascent/descent (below); **bbox** = axis-aligned bounds of quad in **page user space before page `/Rotate`** (see Coordinate spaces). Store `transform: [a,b,c,d,e,f]` as `CTM * Tm` at run start (or per-glyph if we split runs on large gaps).

#### Tw space detection, vertical metrics, WMode (normative residuals)

**`is_space_for_tw` (when to add `Tw`):**

| Check order | Rule |
|-------------|------|
| 1 | If the **character code** (simple fonts) or **CID** is the space identifier for this font’s encoding: for WinAnsi/MacRoman/StandardEncoding, code **0x20**; for PDF `Differences`, the name `/space`. |
| 2 | Else if ToUnicode (or ActualText span) maps this code/CID to a Unicode scalar with `White_Space=Y` **and** general category Zs (e.g. U+0020, U+00A0)—treat as space for Tw. |
| 3 | Else **false** (CID fonts with no space CID: Tw does not apply to “visual gaps”—those are handled by the **space insertion heuristic** at layout time, not Tw). |

Do **not** use only “gap size” for Tw; Tw is a PDF text-state operator tied to space glyphs.

**Ascent / descent for glyph quads (priority):**

| Priority | Source |
|----------|--------|
| 1 | FontDescriptor `/Ascent` and `/Descent` (PDF space, usually 1000-unit design) when present and non-zero |
| 2 | Embedded font program OS/2 or hhea (`ascender`/`descender`) / Type1 FontBBox-derived |
| 3 | Fallback: `ascent = 0.8 * 1000`, `descent = -0.2 * 1000` (design units), `metrics_confidence *= 0.5` |

Quad in text space before `Tm`: roughly `(0, descent/1000 * font_size)` to `(advance_x_text, ascent/1000 * font_size)` for horizontal mode (then apply `Tm` and CTM).

**WMode = 1 (vertical writing) in 0.1:**

| Item | 0.1 policy |
|------|------------|
| Detection | Read CIDFont `/WMode` or font dictionary; if `1`, mark font vertical |
| Advances | **Best-effort stub:** use `/W2`/`DW2` when present for `(vx, vy)` displacement; else `DW2` defaults per ISO; if missing, fall back to horizontal widths with **`metrics_confidence ≤ 0.3`** and `WarningCode::VerticalMetricsStub` |
| Full vertical layout | **Out of 0.1 bar** for CER/layout gates; 0.2 goal |
| Export | Still emit `TextRun`s; do not claim upright reading-order quality |

Add `WarningCode::VerticalMetricsStub` to the warning taxonomy.

#### Unicode vs geometry confidence

| Field | Meaning |
|-------|---------|
| `mapping_confidence` | Quality of byte→Unicode (ToUnicode present, CMap complete, ActualText used) |
| `metrics_confidence` | Quality of widths (explicit Widths/W, embedded hmtx, vs MissingWidth/guess) |

Both are `0.0..=1.0` on `TextRun`.

#### ActualText

When marked-content sequence has property dict `/ActualText` (or structure element), **emit that Unicode** for the spanned glyphs (one or more runs coalesced), while **geometry still comes from painted glyphs**. Warning if ActualText empty but glyphs present.

#### Space insertion heuristic (P1)

When building plain text / lines:

1. Sort glyphs/runs on a baseline band.
2. If gap between adjacent runs **> α * median_space_width** (default α=0.25) and neither side ends/starts with whitespace, insert U+0020.
3. If `Tw > 0`, rely on already-applied word spacing in advances first.
4. Configurable via `TextOptions::insert_spaces` (default **true**).

#### Type3 strategy

v1: treat as simple font with Widths; optional later expand charprocs to `PathElement` for lattice (not required for text export).

#### PR split for fonts (see PR plan)

Gate layout/tables on **metrics golden tests**, not Unicode alone.

---

### Coordinate spaces (normative)

| Space | Definition |
|-------|------------|
| Glyph / text space | Font design units scaled by `Tf` size and text matrix |
| User space | After CTM; **MediaBox origin**; **before** page `/Rotate` |
| Device / upright export | Optional: consumers set `ExtractOptions::apply_page_rotate = true` (default **true** for `text()` / layout; **false** for raw `elements()` paint-order IR) |

`ExtractedPage.rotate` is always recorded. Bboxes in raw elements are user space; layout module applies rotate when option set so tables align with visual upright page.

**Numeric type:** IR uses **`f32`** for coordinates in 0.1 (K19). Goldens round to **3 decimal places**. f64 reserved if we hit precision issues on huge user units (rare).

---

### Element extraction model (IR)

**PR 02 freeze scope (K20) — explicit field allowlist:**

Only the following are **normative for 0.1** (semver-frozen once 0.1.0 ships). Everything else in sketches is **illustrative / non-normative**.

| Type | Frozen fields (allowlist) |
|------|---------------------------|
| `ExtractedDocument` | `schema_version`, `metadata`, `pages`, `warnings`, **`partial`** |
| `DocumentMetadata` | `pdf_version`, `info` (Title/Author/Subject/Creator/Producer/CreationDate/ModDate as optional strings), `page_count`, `encrypted`, `tagged` |
| `ExtractedPage` | `index`, `media_box`, `crop_box`, `rotate`, `user_unit`, `elements`, `warnings` (optional per-page) |
| `Element` | `Text` / `Image` variants only for freeze; `Path` / `FormBoundary` **unstable** |
| `TextRun` | `text`, `glyph_ids`, `font_name`, `font_size`, `bbox`, `transform`, `mcid`, `mapping_confidence`, `metrics_confidence`, **`from_actual_text`** |
| `ImageElement` | `id`, `bbox`, `transform`, `width_px`, `height_px`, `color_space`, `bits_per_component`, `filters`, `data` (`Embedded` \| `Deferred`; `DecodedRgba` behind feature) |
| `ImageData` | enum shape above |
| `ExtractWarning` | `code`, `page`, `message`, `recoverable` |
| `WarningCode` | full enum (additive new variants = non-breaking; renames = breaking) |
| `Rect` | `x0,y0,x1,y1` as `f32` |
| `ObjectId` | `obj`, `gen` |

**Explicitly NOT frozen in 0.1:** `outline`, `structure`, `form`, `page_labels` / `PageLabelTree`, `PageLayout`, `Table*`, `PathElement` details, `Annotation` full shape, colors/stroke on `TextRun`, `char_spacing`/`word_spacing`/`rise`/`rendering_mode`/`ocg` on `TextRun` (may ship as unstable extras), `fill_color`/`stroke_color`.

**Unstable module `experimental` until 0.2+:** tables, structure tree, path op granularity, form boundaries, page label trees, outline.

Illustrative full IR (fields beyond the allowlist are non-normative):

```rust
// crates/pdfparser-ir — sketches; freeze list above is authoritative for 0.1

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractedDocument {
    pub schema_version: u32, // 1 for 0.1 export
    pub metadata: DocumentMetadata,
    pub outline: Vec<OutlineItem>,
    pub pages: Vec<ExtractedPage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub structure: Option<StructureTree>, // unstable shape
    #[serde(skip_serializing_if = "Option::is_none")]
    pub form: Option<FormTree>,
    pub warnings: Vec<ExtractWarning>,
    pub partial: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentMetadata {
    pub pdf_version: Option<String>,
    pub info: InfoDict,
    pub page_count: u32,
    pub encrypted: bool,
    pub tagged: bool,
    /// Number tree expanded only when requested; not a naive Vec in the object model.
    pub page_labels: Option<PageLabelTree>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageLabelTree {
    /// ISO-style ranges: start_page, style, prefix, start_num.
    pub ranges: Vec<PageLabelRange>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextRun {
    pub text: String,
    pub glyph_ids: Option<Vec<u32>>,
    pub font_name: String,
    /// Text-space font size from Tf (not “CTM-scaled px”).
    pub font_size: f32,
    pub bbox: Rect,
    /// a b c d e f for CTM*Tm at run anchor
    pub transform: [f32; 6],
    pub fill_color: Option<Color>,
    pub stroke_color: Option<Color>,
    pub rendering_mode: u8,
    pub char_spacing: f32,
    pub word_spacing: f32,
    pub rise: f32,
    pub mcid: Option<u32>,
    pub ocg: Option<String>,
    pub mapping_confidence: f32,
    pub metrics_confidence: f32,
    pub from_actual_text: bool,
}

/// Form XObjects are **inlined** into `elements` by default (flattened paint).
/// A boundary marker may be emitted when ExtractOptions::mark_form_boundaries.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Element {
    Text(TextRun),
    Image(ImageElement),
    Path(PathElement),
    /// Optional debug/structure aid — not the primary Form representation.
    FormBoundary { name: Option<String>, begin: bool },
}

/// Marked content tracked on a **side stack** during interpret; not necessarily
/// one Element per BMC/EMC. MCID lands on TextRun/Image; structure tree is separate.
```

#### WarningCode taxonomy (stable for support)

```rust
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum WarningCode {
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
    // …
}
```

CLI and metrics key off these codes (not free-text only).

---

### Table detection & evaluation design

#### Runtime methods (unchanged priority)

1. Structure tree `Table`/`TR`/`TH`/`TD` via MCID  
2. Lattice from path rules  
3. Stream from whitespace columns  
4. Hybrid score pick  

**Default `LayoutOptions::detect_tables = false` in 0.1** (K21).  
**Default `min_table_confidence = 0.55`.**

#### Confidence formula v0 (lattice)

```text
confidence =
  0.35 * grid_regularity +      // 1 - CV of row heights / col widths
  0.25 * rule_support +         // fraction of cell edges with a rule
  0.20 * fill_rate +            // cells with non-empty text / total cells
  0.10 * alignment_score +      // text x-align within columns
  0.10 * min(1, cells/6)        // prefer ≥2x3-ish
```

Stream mode replaces `rule_support` with `gap_separation_score`. Structure tree starts at `confidence = 0.9` and drops if MCID gaps.

#### Table evaluation protocol (before PR tables land)

| Item | Definition |
|------|------------|
| Dataset v0 | 30–50 pages: 15 lattice, 15 stream, 10 tagged tables; sources: public government/financial PDFs + synthetic fixtures; **license-clean**; listed in `corpus/tables/manifest.json` |
| Annotation format | JSON: page, table bbox, grid of cells with `row,col,rowspan,colspan,text` (human-normalized whitespace) |
| Cell match | Predicted cell matches GT if IoU(bbox) ≥ **0.5** **or** (same row/col index when both grids equal size) and normalized text equality |
| **Cell F1** | Primary metric on matched cells’ text (micro-F1 over cells) |
| **TEDS** (optional) | Tree-Edit-Distance-based Similarity when structure complex—secondary |
| Multi-line cells | GT text joined with `\n`; normalize collapse of internal whitespace for equality option |
| Header detection | Accuracy reported separately; not gated for 0.1 |
| Multi-page tables | Out of scope for scoring v0 (per-page only) |
| Exit for “tables experimental” | Harness runs in CI; metrics **reported**, not hard-gated until 0.2 |
| Exit for 0.2 tables | Cell F1 ≥ 0.70 on lattice subset; ≥ 0.55 on stream subset; structure subset ≥ 0.90 |

Text API and 0.1 release **do not block** on table metrics.

---

### Memory lifecycle

```text
══════════════════════════════════════════════════════════════════════════════
                           MEMORY LIFECYCLE
══════════════════════════════════════════════════════════════════════════════

  Document::open
        │
        ▼
  XRef + trailer in memory
        │
        ▼
  Object offsets / optional encoded object cache
        │
        │  Page::elements() / text()
        ▼
  Decode content stream  (under Resource Governor)
        │
        ▼
  Build page IR  (Vec<Element>)
        │
        ├──▶ drop decoded content bytes  (do not retain globally)
        │
        ▼
  Return elements / layout to caller
        │
        ├──▶ caller drops results  →  IR freed
        │
        └──▶ [if cache_page_ir]  LRU page IR cache on Document


  Document drop  ──▶  free backend + caches + governor  ──▶  done
```

| Policy | 0.1 default |
|--------|-------------|
| Cache decoded streams globally | **No** — decode per use (or short-lived page-local) |
| Cache encoded stream bytes | Backend-dependent; prefer not to duplicate file |
| Cache page IR | Off unless `OpenOptions::cache_page_ir` |
| mmap feature | File-backed pages; RSS appears lower; still count expanded **decoded** bytes in governor |
| After `Page` extract without cache | IR owned by caller; document keeps encoded structure only |

---

## API / Interface Changes

```rust
use zeroize::Zeroizing;
use std::sync::Arc;

pub use pdfparser_ir::{
    ExtractedDocument, ExtractedPage, Element, TextRun, ImageElement,
    Rect, ExtractWarning, WarningCode, DocumentMetadata, /* … freeze list */
};

pub struct Document { inner: Arc<DocumentInner> }

#[derive(Clone)]
pub struct OpenOptions {
    /// Zeroized on drop; never logged.
    pub password: Option<Zeroizing<String>>,
    pub limits: ResourceLimits,
    pub parse_mode: ParseMode,
    pub lazy_pages: bool,
    pub cache_page_ir: bool,
}

#[derive(Debug, Clone, Copy)]
pub enum ParseMode { Strict, Recoverable }

pub struct Page {
    inner: Arc<DocumentInner>,
    index: u32,
}

impl Document {
    pub fn open(path: impl AsRef<std::path::Path>) -> Result<Self, Error>;
    pub fn open_with(path: impl AsRef<std::path::Path>, opts: OpenOptions) -> Result<Self, Error>;
    /// Reader must be `Send + 'static` only (not `Sync`).
    /// Internally stored as `Mutex<R>` (or equivalent) so `Document: Sync`.
    pub fn from_reader<R: std::io::Read + std::io::Seek + Send + 'static>(
        reader: R, opts: OpenOptions,
    ) -> Result<Self, Error>;
    pub fn from_bytes(bytes: impl Into<Vec<u8>>, opts: OpenOptions) -> Result<Self, Error>;

    pub fn page_count(&self) -> u32;
    pub fn metadata(&self) -> &DocumentMetadata;
    pub fn warnings(&self) -> &[ExtractWarning];
    pub fn page(&self, index: u32) -> Result<Page, Error>;
    pub fn pages(&self) -> impl Iterator<Item = Result<Page, Error>> + '_;
    pub fn extract(&self, opts: &ExtractOptions) -> Result<ExtractedDocument, Error>;
}

impl Page {
    pub fn index(&self) -> u32;
    pub fn media_box(&self) -> Rect;
    pub fn elements(&self, opts: &ExtractOptions) -> Result<Vec<Element>, Error>;
    pub fn text(&self, opts: &TextOptions) -> Result<String, Error>;
    pub fn layout(&self, opts: &LayoutOptions) -> Result<PageLayout, Error>;
    pub fn images(&self, opts: &ImageOptions) -> Result<Vec<ImageElement>, Error>;
    pub fn annotations(&self) -> Result<Vec<Annotation>, Error>;
    pub fn tables(&self, opts: &LayoutOptions) -> Result<Vec<Table>, Error>;
}

#[derive(Debug, Clone)]
pub struct ExtractOptions {
    pub text: TextOptions,
    pub images: ImageOptions,
    pub layout: LayoutOptions,
    pub include_paths: bool,
    pub include_structure: bool,
    pub include_annotations: bool,
    pub include_forms: bool,
    pub apply_page_rotate: bool,
    pub mark_form_boundaries: bool,
    pub allow_partial_document: bool,
}

#[derive(Debug, Clone)]
pub struct TextOptions {
    pub preserve_positions: bool,
    pub sort_reading_order: bool,
    pub insert_spaces: bool,
    /// Default: U+FFFD REPLACEMENT CHARACTER
    pub unknown_glyph: char,
}

impl Default for TextOptions {
    fn default() -> Self {
        Self {
            preserve_positions: false,
            sort_reading_order: true,
            insert_spaces: true,
            unknown_glyph: '\u{FFFD}',
        }
    }
}

#[derive(Debug, Clone)]
pub struct ImageOptions {
    /// Default false — keep encoded/deferred streams.
    pub decode_pixels: bool,
    pub include_inline_images: bool,
    pub max_pixels: u64,
}

#[derive(Debug, Clone)]
pub struct LayoutOptions {
    /// Default false in 0.1.
    pub detect_tables: bool,
    pub table_modes: TableModeSet,
    pub y_tolerance: Option<f32>,
    pub min_table_confidence: f32, // default 0.55
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("I/O: {0}")]
    Io(#[from] std::io::Error),
    #[error("syntax at {pos}: {msg}")]
    Syntax { pos: u64, msg: String },
    #[error("limit exceeded: {kind:?}")]
    LimitExceeded { kind: LimitKind },
    #[error("encrypted: password required, incorrect, or unsupported revision")]
    Encryption,
    #[error("unsupported: {0}")]
    Unsupported(String),
    #[error("page {0} out of range")]
    PageOutOfRange(u32),
    #[error("backend: {0}")]
    Backend(String),
    #[error("internal: {0}")]
    Internal(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LimitKind {
    FileSize,
    ObjectCount,
    StreamExpandedBytes,
    TotalExpandedBytes,
    FilterExpansionRatio,
    FilterChainLength,
    NestingDepth,
    PageCount,
    ContentOps,
    ImagePixels,
    XfaBytes,
    EmbeddedFontBytes,
    TimeBudget,
}
```

**Semver coupling:** façade re-exports freeze-list IR types; **IR breaking changes are pdfparser breaking changes** (K22). Unstable table/structure types live in `pdfparser::experimental` or behind docs(“unstable”).

**Debug redaction:** `Document`/`Page`/`TextRun` implement `Debug` that **omits or truncates** string content unless feature `debug-content` is enabled. Passwords never appear in Debug.

### CLI (secondary surface)

The CLI is **not** the primary product. It exists to exercise the library, support CI/goldens, and help operators. Feature prioritization and API design always optimize for **embedders**, not CLI UX.

```text
# 0.1 — text/info/images without tables required
pdfparser info input.pdf
pdfparser extract input.pdf --out out.json --format json|text|markdown
pdfparser pages input.pdf --text
pdfparser images input.pdf --out-dir ./imgs

# 0.2+
pdfparser tables input.pdf --csv-dir ./tables
pdfparser inspect fonts|resources|ops input.pdf --page 0
```

Limits flags map to `ResourceLimits`. `--strict` selects `ParseMode::Strict`.

---

## Data Model Changes

No external DB. Internal:

| Structure | Lifetime |
|-----------|----------|
| Backend + xref | `Document` lifetime |
| Encoded objects | As needed; no mandatory full decode |
| Decoded streams | Ephemeral unless cache |
| Page IR | Caller-owned or optional LRU |
| Export JSON | User files; `schema_version` |

---

## Dependency Strategy

### Recommendation

| Layer | Strategy |
|-------|----------|
| XRef/objects syntax | `ObjectBackend`; initial **lopdf adapter** after spike; **pdf-rs** as Alternative E competitor in spike |
| Stream filters + Predictor | **Owned only** |
| Content / fonts / layout / export | **Owned** |
| Codecs | `flate2`/`miniz_oxide`, JPEG pure Rust; **no JBIG2 C lib in default** |
| Pdfium | Dev/CI oracle optional |

### K3 acceptance metrics (backend spike)

After PR 04–06 on a **Corpus Spike S0** (≥100 public unencrypted PDFs, license-clean):

| Metric | Proceed with lopdf adapter | Trigger fork/vendor or pdf-rs trial | Trigger rewrite plan |
|--------|----------------------------|------------------------------------|----------------------|
| Open success (trailer+page tree) | ≥ 90% | 75–90% | < 75% |
| Panic count | 0 | 0 | any |
| Raw stream access possible without third-party decode | Required | — | — |

Document results in `docs/adr/0001-object-backend.md`. **Revisit K3** in a decision meeting before P1 content work freezes on lopdf quirks.

### Why not trust informal “80%” figures

Prior draft cited informal real-world open-rate numbers without methodology. **Removed.** Only spike measurements gate backend choice.

---

## Alternatives Considered

### Alternative A — Full greenfield ISO 32000 object parser
Max control; 6–12+ months delay. **Reject as default**; possible later rewrite using corpus.

### Alternative B — Pdfium/MuPDF-first product
Fast fidelity; loses native ownership; FFI/CVE surface. **Reject as primary**; oracle only.

### Alternative C — Thin lopdf + pdf-extract wrapper
Fast demo; weak IR/metrics/security. **Spike only**, not product architecture.

### Alternative D — Hybrid pure-Rust owned extract (chosen)
`ObjectBackend` + owned filters/content/fonts/layout. **Adopt.**

### Alternative E — pdf-rs (`pdf` crate) as ObjectBackend
| Pros | Cons |
|------|------|
| Lazy parsing philosophy may fit large docs | API/maturity churn risk |
| Different failure modes than lopdf (diversity) | Same need for encoded-stream discipline |
| Pure Rust | May still need fork for raw streams |

**Verdict:** Implement `PdfRsBackend` **spike** in parallel with lopdf if S0 open-rate weak; keep trait so choice is reversible. Not default until metrics win.

### Alternative F — Oracle-first development process (process, not engine)
Use Pdfium as development companion (see Correctness). **Adopt as process**; not as runtime.

---

## Security & Privacy Considerations

### Threat model (extended)

| Threat | Severity | Mitigation |
|--------|----------|------------|
| Stream decompression bomb | Critical | Owned filters + budgets + hard_max |
| Predictor / filter chain abuse | High | Chain len ≤ 8; predictor output size checks |
| Object cycles / deep forms | High | Visited sets; nesting limits |
| XFA XML bomb | High | `max_xfa_bytes`; never execute XFA |
| Content op flood | High | `max_content_ops_per_page` |
| Image pixel bomb | High | Checked mul + `max_image_pixels` |
| Font program parse bugs | High | Size cap; **fuzz font loaders**; pure Rust parsers preferred |
| JBIG2 C codec (if enabled) | Critical | Feature `jbig2-ffi` **default off**; document **process isolation** mandatory for multi-tenant |
| JS / dangerous actions | Medium | Never execute |
| Encryption partial leak | High | Fail closed; no content before auth |
| Supply chain | High | `cargo audit`, minimal deps |
| Log leakage | Medium | Redacted Debug; no content logs default |

### Multi-tenant / service embedding (ops requirement)

Limits reduce DoS but **do not replace isolation**:

1. Prefer **one process (or container) per untrusted job** with cgroups CPU/memory walls.  
2. OS sandbox for CLI batch (seccomp/AppArmor/macOS sandbox).  
3. Any `*-ffi` codec feature: **same process isolation requirement** in docs; default builds stay pure Rust.  
4. Set `max_open_or_extract_time` / `max_page_interpret_time` for runaway CPU pure-Rust paths (cooperative `TimeBudget`); still combine with OS cgroups for hard kill.

### C codec policy

| Codec | Policy |
|-------|--------|
| Flate, ASCII*, LZW, RunLength, Predictor | Pure Rust in-tree |
| JPEG | Pure Rust crate |
| JBIG2 | Optional FFI, default off, isolated |
| CCITT | Prefer pure Rust implementation in 0.2 |

### Password handling

`Zeroizing<String>` in `OpenOptions`; zeroize after auth; never log.

---

## Observability

### Logging

`tracing` spans: `open_document`, `decode_stream`, `load_font`, `interpret_content`, `detect_tables`.  
`trace-ops` feature for operator dumps. Font resolution report at DEBUG: encoding, ToUnicode present, widths source, confidences.

### Metrics

Counters include `limit_trips{limit_kind}`, `warnings{code}`, open/page timings. Structured `LimitKind` enables paging.

### Performance targets

**Reference machine (normative for SLO claims):**  
8-core x86_64 or Apple Silicon, 16 GB RAM, release build (`--release`), CPU frequency governors default, single-threaded extract unless noted, input from warm page cache, sample fixture set `corpus/perf/*`.

| Workload | Target |
|----------|--------|
| Open 5 MB / 20-page digital PDF (lazy) | p50 < 50 ms, p99 < 200 ms |
| Full text extract same | p50 < 150 ms, p99 < 500 ms |
| 300-page digital report page-at-a-time text | < 5 s total; peak RSS < 256 MB lazy, no IR cache |
| 1000-page lazy open metadata only | peak RSS < 128 MB typical |
| Table detect per page | p50 < 30 ms additional (when enabled) |

Parallel governor: atomic expanded-byte counters; no double-count free on failure paths (RAII budget tickets).

---

## Correctness & Test Strategy

### Layers

Unit → integration fixtures → golden IR → adversarial → **early continuous fuzz** → optional oracle → table eval harness → regression dashboards.

### Metrics gate matrix (normative — single source of truth)

Gates are split so 0.1 remains achievable for 1–2 engineers while still measuring fidelity.

| Metric | Definition | Reported from | **Blocks P1 exit?** | **Blocks 0.1.0 release?** | **Blocks 1.0.0?** |
|--------|------------|---------------|---------------------|---------------------------|-------------------|
| Panic rate | Any panic on corpus A / adversarial | P0 | **Yes** (0) | **Yes** (0) | **Yes** (0) |
| Open rate | Trailer+page tree, unencrypted corpus A | P0 | **Yes** (≥90% S0; ≥95% by 0.1) | **Yes** ≥95% | **Yes** ≥98% |
| Limit / bomb tests | Adversarial suite | P0 | **Yes** | **Yes** | **Yes** |
| Encryption behavior | `/Encrypt` → `Error::Encryption` | P0 | **Yes** | **Yes** | N/A→crypto tests |
| CER mean | vs oracle plain text on `digital_text_50` (see oracle paths) | P1 | **No** — report only | **No** — report in release notes | **Yes** ≤ 2% mean *or* documented waiver |
| CER easy subset | `digital_text_easy_15` (single-column, embedded ToUnicode, Latin) | P1 | **Yes** ≤ 5% mean | **Yes** ≤ 5% mean | subsumed by full CER |
| Unicode coverage | pages with mean `mapping_confidence` ≥ 0.8 | P1 | No | Report | ≥ 90% on digital_text_50 |
| BBox recall | oracle char-box IoU≥0.5 vs predicted run union on `metrics_20` | P1 | **No** — report only | **No** — report | **Yes** ≥ 85% *or* waiver |
| BBox smoke | 5 hand-checked fixtures, IoU≥0.5 on ≥70% boxes | P1 | **Yes** | **Yes** | subsumed |
| Table cell-F1 | labeled table set | P2 | No | No (tables off) | experimental→gate in 0.2+ |

**Aspirational (track, never silent):** CER ≤ 2% and bbox recall ≥ 85% on full sets—**1.0 release-blocking** (or explicit product waiver), **not** 0.1-blocking.

#### Oracle comparison paths (normative)

| Path | Oracle source | Our side | Normalization |
|------|---------------|----------|---------------|
| **Text (CER)** | Pdfium (preferred) or MuPDF **page plain-text API** (not renderer OCR) | `Page::text` with `sort_reading_order=true`, `insert_spaces=true` | NFKC; Unicode whitespace collapse to single `U+0020`; strip leading/trailing WS per line; **do not** require identical newline layout—join lines with single spaces for CER primary; secondary report keeps lines |
| **BBox** | Pdfium **character boxes** (or equivalent char/glyph quads) in **unrotated page user space** | Union of our `TextRun.bbox` with `apply_page_rotate=false` (raw user space) | Convert oracle boxes to same user space if API returns upright/device space; document transform once in `docs/oracle-coordinates.md` |
| **Matching** | Each oracle char box matches if IoU ≥ 0.5 with **union of overlapping predicted runs** (not requiring 1:1 run segmentation) | | Unmatched oracle boxes count against recall |

Reading-order / multi-column disagreements affect CER: that is why **full CER is 1.0-gated** and 0.1 uses **easy_15** plus reports.

### Oracle development workflow (Alternative F process)

```text
1. Add fixture PDF under corpus/fixtures/<id>/
2. cargo test / pdfparser extract → out.json
3. Optional: pdfium-diff --text-cer --bbox-iou 0.5 (paths above)
4. Mismatches: triage (a) our bug (b) acceptable ambiguity (c) oracle quirk
5. Accept ambiguity with Warning or golden note
6. CI 0.1: unit/integration + easy_15 CER + bbox smoke **required**;
   full digital_text_50 CER/IoU **nightly report only** (non-merge-blocking)
7. CI 1.0: full CER ≤ 2% and bbox recall ≥ 85% required (or signed waiver)
```

### Fuzz

Start **as soon as filters + content lexer exist** (PR 05b / 08b), not after product features:

- `fuzz_open_document`
- `fuzz_decode_stream`
- `fuzz_content_ops`
- `fuzz_font_load` (when fonts land)

### Adversarial

Cycles, bombs, truncated files, nested forms, oversize XFA, bad Predictors, inline image floods.

---

## Encryption scope (normative product decision)

**K15 — 0.1.0 (single rule):** If the trailer dictionary contains an `/Encrypt` entry that resolves to an encryption dictionary → **`Error::Encryption`**. Applies to **all** encrypted files, including empty user password, blank owner password, and R=2–6. **No** silent open, **no** partial extract, **no** “trivial empty-password” exception in 0.1. Document in README prominently. Empty-user-password support is exclusively a **0.2** crypto-matrix item.

**0.2 encryption subset (planned):**

| Revision / algorithm | Support target |
|----------------------|----------------|
| R=2 RC4 40-bit | Yes |
| R=3 RC4 128-bit | Yes |
| R=4 AES-128 | Yes |
| R=5/6 AES-256 | Best-effort follow-up |
| Empty user password | Yes when matrix lands |
| Owner password only / permissions enforcement | Metadata; extract if user opens |
| Crypt filter per-stream | With handler |
| Public-key (PKCS#7) | Out of 0.2 |

Split implementation across multiple PRs (handler parse, key derivation, RC4, AES-CBC, tests)—not one PR.

---

## Rollout Plan

| Version | Contents |
|---------|----------|
| **0.0.x** | Internal; P0–P1 |
| **0.1.0** | Unencrypted open; text+images+basic layout; CLI text/json; tables **off default / experimental module**; no encryption; JSON schema freeze subset |
| **0.2.0** | Tables on-by-opt-in stable; structure polish; encryption subset; CCITT; inspect subcommands |
| **1.0.0** | API+schema freeze; **full CER ≤2% + bbox recall ≥85% gates in CI** (or waiver); security audit |

### Feature flags

| Feature | Default | Purpose |
|---------|---------|---------|
| `serde` | on | Serialization |
| `image-decode` | off | RGBA decode |
| `parallel` | off | rayon |
| `mmap` | off | memmap2 |
| `trace-ops` | off | operator dump |
| `debug-content` | off | Debug prints text |
| `jbig2-ffi` | **off** | Optional C codec |
| `oracle-pdfium` | off | dev/CI |

---

## Risk Table

| ID | Risk | Sev | Mitigation |
|----|------|-----|------------|
| R1 | Font metrics wrong → tables fail | High | Metrics pipeline + IoU gates; split font PRs |
| R2 | Table heuristics weak | High | Eval harness; off-by-default 0.1 |
| R3 | Backend open-rate plateau | High | S0 metrics; trait; pdf-rs spike; fork policy |
| R4 | Stream bomb via third-party decode | Critical | Encoded-only trait; owned filters; decode-guard tests |
| R5 | Scope creep to renderer | Medium | Non-goals |
| R6 | 1–2 eng schedule slip on fonts | High | Critical path explicit; tables after text CLI |
| R7 | CMap license | Medium | Audit PR before embed |
| R8 | Unknown ops miss content | Medium | Operator coverage report tool |
| R9 | Enterprise encrypted PDFs | Medium | K15 clear errors; 0.2 matrix |
| R10 | FFI codec CVEs | High | Default off + process isolation |
| R11 | core↔fonts cycle | Medium | Resolved DAG: fonts leaf data-in |

---

## Key Decisions

| ID | Decision | Rationale |
|----|----------|-----------|
| **K1** | **Library-first for Rust embedders**; CLI optional/secondary | Primary audience embeds the crate; CLI does not drive design |
| **K2** | Native pure-Rust pipeline; no Python/cloud primary | Product identity |
| **K3** | ObjectBackend; lopdf initial **after S0 metrics**; reversible | Time-to-value + measurement |
| **K4** | Own content, fonts, layout, IR | Differentiation + correct bboxes |
| **K5** | Pdfium oracle optional, not runtime | Native ownership |
| **K6** | Tables heuristic + confidence; structure first | Honest PDF limits |
| **K7** | Phase-1 digital PDFs; OCR later | Feasible correctness |
| **K8** | Resource limits always on; hard_max ceilings | Threat model |
| **K9** | Lazy pages default | Large-doc memory |
| **K10** | schema_version + semver | Downstream stability |
| **K11** | Recoverable default; **limits always hard errors** | Dirty PDFs vs bombs |
| **K12** | No full rasterizer in v1 | Schedule |
| **K13** | Multi-crate workspace; fonts leaf DAG | Compile isolation, no cycles |
| **K14** | JSON primary export | Pipeline interop |
| **K15** | **Any `/Encrypt` → `Error::Encryption` in 0.1** (no empty-password exception) | Single implementable rule; crypto only in 0.2 |
| **K16** | **Encoded-stream-only backend; owned decode** | Governor enforceability |
| **K17** | If raw streams impossible, **vendor/fork** backend—not silent wrap | Security honesty |
| **K18** | **`Page` = Arc handle**, `Document: Send+Sync` | Parallel + cache |
| **K19** | **f32** coords; golden 3 decimals | Simplicity |
| **K20** | **PR02 freezes explicit field allowlist** (incl. `partial`, `from_actual_text`, `glyph_ids`); tables/structure unstable | Avoid churn |
| **K21** | **Tables default off in 0.1** | Quality bar |
| **K22** | Façade re-exports IR freeze list = coupled semver | One version story |
| **K23** | **Images default encoded/deferred** (`decode_pixels: false`) | Memory |
| **K24** | **Sync API only in v1**; WASM non-goal | Focus |
| **K25** | **Publish name `pdfparser`** (only façade crate public in 0.1); fallback `pdfparser-native` if taken | User-decided name |
| **K26** | **Unknown glyph = U+FFFD** | Explicit |
| **K27** | **Form XObjects inlined** into page elements by default | Simpler consumers |
| **K28** | **1–2 engineer** plan; Gantt illustrative | Feasibility honesty |
| **K29** | CMap package chosen in license audit PR before embed | Legal |
| **K30** | Multi-tenant: **process isolation recommended** | Ops reality |
| **K31** | `from_reader`: **`Send+'static` only**; interior `Mutex` for Sync Document | Avoid over-constraining readers |
| **K32** | ObjStm decode+parse **only in core**; backend returns Compressed xref + encoded ObjStm | Preserve K16 |
| **K33** | Metrics: 0.1 blocks on easy CER + bbox smoke; full CER/IoU **1.0** | Feasible 0.1 bar |
| **K34** | **Primary audience = Rust library embedders**; API + JSON primary; CLI secondary | User decision |
| **K35** | **Text extraction first** — no early pull of tables or encryption vs critical path | User decision |
| **K36** | Product/crate name **`pdfparser`** (user decision) | Branding / crates.io |


---

## Open Questions

### Resolved (user or design decisions)

1. ~~Object backend raw streams~~ → **Resolved K16/K17**; remaining engineering: S0 measurement outcome (lopdf vs pdf-rs).
2. ~~Default tables~~ → **K21 off**.
3. ~~Image default~~ → **K23 encoded**.
4. ~~Encryption priority / 0.1 scope~~ → **K15** locked; 0.2 subset defined; **not** pulled earlier than critical path (**K35**).
5. ~~**Naming**~~ → **`pdfparser`** (user decision, **K36**); fallback `pdfparser-native` only if crates.io name taken at reserve time.
6. ~~**Primary audience / delivery form**~~ → **Rust library embedders**; crate API + JSON primary, CLI secondary (**K34**).
7. ~~**Scope priority**~~ → **Text extraction first** (unencrypted digital → metrics text + images + library/JSON → layout/tables later; no early tables/encryption) (**K35**).
8. ~~WASM~~ → non-goal v1 (**K24**).
9. ~~Empty-user-password in 0.1?~~ → **K15:** any `/Encrypt` → `Error::Encryption`; empty-user-password only in 0.2.

### Still open (technical / legal — recommendations stand)

| # | Question | Recommendation (unless later decided) |
|---|----------|----------------------------------------|
| T1 | **Which exact CMap package / license** to embed? | Resolve in **PR 10-license** with legal/SPDX allowlist before CID embed |
| T2 | **Table corpus final PDF selection** license pass | Protocol defined; pick license-clean fixtures at kickoff (`corpus/tables/manifest.json`) |
| T3 | **CLI parallel default** (`rayon`)? | **Off** until governor stress tests pass; library `parallel` feature remains opt-in |
| T4 | S0 backend outcome: lopdf vs pdf-rs vs fork? | Measure ≥100 PDFs after PR 04–06; apply K3 thresholds |
| T5 | Partial region extract API in 0.1? | **Defer** to 0.2 (`page.region(rect)`) |

---

## References

- ISO 32000-1:2008 / ISO 32000-2:2020
- PDF Association — Tagged PDF / PDF/UA
- `lopdf`, `pdf` (pdf-rs), `pdf-extract`, `pdfium-render` — re-validate at kickoff
- Camelot/Tabula — lattice vs stream table paradigms
- TEDS / table recognition literature for evaluation inspiration
- Rust: `cargo fuzz`, `cargo audit`, `zeroize`

---

## PR Plan

Assumed **1–2 engineers**. Each PR: **S** (<2d), **M** (~3–5d), **L** (1–2w), **XL** (split further if growing). Independently reviewable; `main` always green.

### PR 01 — Workspace skeleton & CI — **S**
- **Files:** workspace `Cargo.toml`, crate stubs, CI fmt/clippy/test, license
- **Deps:** none
- **Description:** Cycle-free crate graph per DAG; `publish = false` on internals; only `pdfparser` publishable.

### PR 02 — IR freeze subset + WarningCode — **M**
- **Files:** `pdfparser-ir`, `schemas/extract-v1.json` (subset), serde tests
- **Deps:** PR 01
- **Description:** Freeze K20 fields only; modules `experimental` for tables/structure placeholders; document non-freeze types as illustrative.

### PR 03 — Governor, LimitKind, errors, tracing — **M**
- **Files:** `limits.rs`, `error.rs`, hard_max, unit tests
- **Deps:** PR 01
- **Description:** Atomic budgets; structured `LimitKind`.

### PR 04 — ObjectBackend trait + fixture backend + lopdf spike adapter — **L**
- **Files:** `backend/mod.rs`, `fixture.rs`, `lopdf_backend.rs`, S0 measurement harness
- **Deps:** PR 03
- **Description:** Encoded-only `get_encoded`; **decode-guard** tests; publish S0 open-rate report; ADR draft for K3.

### PR 05 — Owned filter pipeline + Predictor + bomb tests — **L**
- **Files:** `filters/**` (Flate, ASCII85/Hex, LZW, RunLength, **Predictor 1–15**), integration bombs
- **Deps:** PR 04
- **Description:** **Replaces** third-party decode on product paths; expansion ratio + absolute caps; object-stream unpack via our filters.

### PR 05b — Early fuzz smoke (open + decode) — **S**
- **Files:** `fuzz/fuzz_open_document`, `fuzz/fuzz_decode_stream`, CI smoke job (short)
- **Deps:** PR 05
- **Description:** Continuous fuzz from foundation—not after features.

### PR 06 — Page tree & resource inheritance — **M**
- **Files:** `page_tree.rs`, `resources.rs`, nested Kids fixtures
- **Deps:** PR 04, PR 05
- **Description:** Page enumerate, boxes, rotate, merged resources; encryption detection → `Error::Encryption`.

### PR 07 — Public `Document` / Arc `Page` API — **M**
- **Files:** `pdfparser` façade, OpenOptions with `Zeroizing`, Send+Sync tests
- **Deps:** PR 02, PR 06
- **Description:** Lazy open; metadata; warnings; no content yet required.

### PR 08 — Content stream lexer/parser — **M**
- **Files:** `pdfparser-content` lexer/ops
- **Deps:** PR 01
- **Description:** Operators + operands; unknown ops list.

### PR 08b — Fuzz content lexer — **S**
- **Deps:** PR 08
- **Description:** `fuzz_content_ops`.

### PR 09 — Graphics state VM (metrics-pluggable) — **L**
- **Files:** state/vm; matrix; text ops calling `FontMetrics` trait object
- **Deps:** PR 02, PR 08
- **Description:** TJ/Tw/Tc/Th positioning; **temporary uniform metrics** allowed only behind `cfg(test)` stub; production path requires PR 10a.

### PR 10-license — CMap/font data license audit gate — **S**
- **Files:** `docs/licenses/cmap.md`, allowlist, SPDX; no binary CMap embed until merge
- **Deps:** PR 01; **must merge before PR 10b** (CID/CMap embed)
- **Description:** Legal README; choose redistributable CMap package; blocks embedding copyrighted data. *(Formerly mis-numbered as PR 26.)*

### PR 10a — Simple fonts: Widths + encodings + ToUnicode — **L**
- **Files:** `pdfparser-fonts` simple fonts; core wiring push-bytes-in
- **Deps:** PR 05, PR 09
- **Description:** Type1/TrueType simple; bbox smoke tests start. No third-party CMap pack required yet (built-in encodings).

### PR 10b — Type0/CID/CMap/Identity-H — **L**
- **Deps:** PR 10a, **PR 10-license**
- **Description:** W/DW arrays; vertical stubs (**WMode=1 best-effort**); embedded CMap parse only after license gate.

### PR 10c — Embedded TT/OT `hmtx` (+ CFF widths best-effort) — **L**
- **Deps:** PR 10a
- **Description:** When `/Widths` missing; font fuzz target; size caps.

### PR 10d — Type3 strategy + ActualText wiring — **M**
- **Deps:** PR 10a, PR 09
- **Description:** Type3 widths; BDC ActualText preferred unicode; `from_actual_text`.

### PR 11 — Image XObject + inline `BI/ID/EI` — **L**
- **Deps:** PR 05, PR 09
- **Description:** Shared image limit path; DCT pass-through; inline images P1 operators.

### PR 12 — Form XObject inline recursion — **M**
- **Deps:** PR 09, PR 03
- **Description:** Depth/cycle; inlined elements; optional boundaries.

### PR 13 — Path capture for lattice — **M**
- **Deps:** PR 09
- **Description:** PathElement when `include_paths`.

### PR 14 — Wire Page::elements/text + space heuristic — **L**
- **Deps:** PR 07, PR 10b (min 10a), PR 11, PR 12
- **Description:** First production text path; insert_spaces; CER fixtures begin.

### PR 15 — Text-only export + thin CLI (no tables) — **M**
- **Files:** `pdfparser-export` text/json subset; `pdfparser-cli` thin wrapper (info/extract/pages)
- **Deps:** PR 14, PR 02
- **Description:** **Primary product slice for embedders**: library extract + JSON. CLI is a thin validation wrapper only—does not expand API surface. Lands before layout/tables per **K35**.

### PR 16 — Layout: lines, blocks, reading order — **L**
- **Deps:** PR 14
- **Description:** Clustering; rotate option; goldens. **Gates on metrics tests green.**

### PR 17 — Structure tree map (MCID) — **M**
- **Deps:** PR 06, PR 14 (**not** dependent on layout clustering)
- **Description:** StructTreeRoot walk; role map; independent of PR 16.

### PR 18 — Annots + AcroForm + XFA limit — **M**
- **Deps:** PR 07
- **Description:** Can land early for demos; XFA size reject; no JS.

### PR 19 — OCG tagging — **S**
- **Deps:** PR 09, PR 14
- **Description:** Tag elements; visible-only filter option.

### PR 20 — Table eval harness + fixtures (no detector required) — **M**
- **Deps:** PR 02
- **Description:** Annotation format, cell-F1 tooling, empty-detector baseline; unblocks metrics culture.

### PR 21 — Structure-tree tables — **M**
- **Deps:** PR 17, PR 20
- **Description:** High-confidence tables from tags.

### PR 22 — Lattice tables — **L**
- **Deps:** PR 13, PR 16, PR 20
- **Description:** Confidence formula v0; experimental module.

### PR 23 — Stream tables + hybrid orchestration — **L**
- **Deps:** PR 16, PR 20
- **Description:** Whitespace columns; hybrid pick.

### PR 24 — Export tables CSV/MD + CLI tables — **S**
- **Deps:** PR 15, PR 21–23
- **Description:** Optional; 0.2 oriented but can merge disabled-by-default.

### PR 25 — Operator coverage report tool — **S**
- **Deps:** PR 08, PR 14
- **Description:** R8 mitigation; CI artifact of unknown op frequency on corpus.

### PR 27 — Adversarial suite expansion + backend malicious stream tests — **M**
- **Deps:** PR 05, PR 07, PR 05b
- **Description:** Decode-guard; cycles; XFA; predictors.

### PR 28 — Golden corpus runner + coverage matrix — **M**
- **Deps:** PR 14, PR 15
- **Description:** Snapshots; CER job optional.

### PR 29 — Criterion benches + RSS budgets — **M**
- **Deps:** PR 14
- **Description:** Reference machine notes; lazy RSS.

### PR 30 — Oracle pdfium-diff tool (optional CI) — **M**
- **Deps:** PR 14
- **Description:** Workflow docs; non-gating 0.1.

### PR 31a — Encryption: handler parse + permissions dict — **M**
- **Deps:** PR 06; **post-0.1**
- **Description:** Detect revisions; still no decrypt in 0.1 branch.

### PR 31b — Encryption: RC4 R=2/3 — **L**
- **Deps:** PR 31a

### PR 31c — Encryption: AES-128 R=4 + tests matrix — **L**
- **Deps:** PR 31a

### PR 32 — CCITT pure Rust best-effort — **L**
- **Deps:** PR 05, PR 11; post-0.1

### PR 33 — Optional `jbig2-ffi` feature (default off) + isolation docs — **M**
- **Deps:** PR 11; post-0.1

### PR 34 — `inspect` CLI (fonts/resources/ops) — **M**
- **Deps:** PR 15, PR 10a; can be 0.1.1 stretch

### PR 35 — 0.1.0 polish: docs, changelog, crates.io, known limitations — **M**
- **Deps:** PR 15, PR 27, PR 28, PR 29 (min)
- **Description:** Freeze API subset; tables experimental warning; encryption unsupported documented.

```text
Critical path (0.1):
01 → 02 → 03 → 04 → 05 → 05b → 06 → 07
              ↘ 08 → 08b → 09 → 10a → 10-license → 10b/10c/10d → 11 → 12 → 14 → 15 → 35
13 → 16 → 22…
06 → 17 → 21
05 → 27; 14 → 28/29/30
Post-0.1: 31a–c, 32, 33, 24 tables CLI
```

---

## Appendix A — Operator priority list

**P1 (must):** `q Q cm`, path `m l c v y h re`, paint `S s f f* B B* n`, clip `W W*`, text state `Tc Tw Tz TL Tf Tr Ts`, text `BT ET Td TD Tm T* Tj TJ ' "`, **`BI ID EI`**, `Do`, `gs` (partial), colors store-only, **`BMC BDC EMC`** (for ActualText/MCID/OCG).

**P2:** `MP DP`, `sh`, `ri`, `i`, `d`, `J j M w`.

**P3 / stub:** patterns, complex soft masks, full Type3 paint.

---

## Appendix B — Success metrics for 0.1.0

**Blocking (must pass to ship 0.1.0):**

| Metric | Target |
|--------|--------|
| Panic rate corpus A + adversarial | 0 |
| Open success unencrypted (Recoverable, page tree) | ≥ 95% |
| CER on `digital_text_easy_15` | ≤ 5% mean |
| BBox smoke (5 fixtures) | ≥ 70% oracle boxes IoU≥0.5 |
| Image extract where XObject images present | ≥ 95% metadata success |
| Encrypted files with `/Encrypt` | Deterministic `Error::Encryption` (incl. empty user password) |
| Security checklist (limits, fuzz smoke, no default FFI) | Pass |
| Public docs + limitations (incl. no encryption, tables off) | Yes |
| p50 extract 20-page doc on reference machine | < 150 ms |

**Reported in 0.1 release notes (not release-blocking):**

| Metric | Aspirational track |
|--------|--------------------|
| CER mean on `digital_text_50` | Heading toward ≤ 2% at 1.0 |
| BBox recall on `metrics_20` | Heading toward ≥ 85% at 1.0 |
| Unicode coverage digital_text_50 | Heading toward ≥ 90% at 1.0 |

See **Metrics gate matrix** for P1 vs 0.1 vs 1.0 alignment.

---

## Appendix C — Glossary

| Term | Meaning |
|------|---------|
| ObjectBackend | Trait for encoded PDF object access |
| Governor | Resource limit accountant |
| CTM | Current Transformation Matrix |
| ActualText | Accessibility replacement string for marked content |
| Predictor | Pre-filter on Flate/LZW image/stream data |
| CER | Character Error Rate |
| Lattice / Stream table | Rules vs whitespace heuristics |
| S0 | Backend open-rate spike corpus |

---

## Appendix D — Revision history

| Rev | Date | Notes |
|-----|------|-------|
| 1 | 2026-07-10 | Initial draft |
| 2 | 2026-07-10 | Design review round 1→2: ObjectBackend/governor, font metrics, DAG, PR realism, recovery, filters, security, table eval, K15–K30 |
| 2.1 | 2026-07-10 | Re-review: K15 empty-password alignment; ObjStm core ownership algorithm; metrics gate matrix; TimeBudget fields; font Tw/ascent/WMode/Th; PdfDict in core; IR field allowlist; from_reader Send-only; PR 10-license |
| 2.3 | 2026-07-10 | Replaced all Mermaid diagrams with monospaced ASCII architecture diagrams for IDE visibility |
| 2.2 | 2026-07-10 | User decisions: name `pdfparser`; primary audience library embedders (API+JSON primary, CLI secondary); text-extraction-first scope priority (K34–K36); Open Questions reconciled |

---

*End of design document — Status: Draft (rev 2.3) — 2026-07-10*
