# Market Analysis & Competitive Benchmark Report  
## PDF Parsing Libraries vs Future `pdfparser` (Rust)

| Field | Value |
|-------|-------|
| **Date** | 2026-07-10 |
| **Status** | v1.0 — corpus + measured benchmark |
| **Benchmark harness** | `benchmark/` |
| **Results** | `benchmark/results/benchmark_results.json` |
| **Corpus** | `benchmark/corpus/` (12 synthetic multi-scenario PDFs) |
| **Purpose** | Baseline competitors so we can measure `pdfparser` against the same fixtures |

> **How to re-run**
> ```bash
> cd pdfParser
> python3 -m venv .venv && source .venv/bin/activate
> pip install -r benchmark/requirements.txt
> python benchmark/scripts/generate_corpus.py
> python benchmark/scripts/run_benchmark.py
> ```

---

## 1. Executive summary

### 1.1 What we did

1. Surveyed SOTA / widely used **native digital** PDF extractors (Python ecosystem + engines they wrap).
2. Built a **reproducible benchmark pipeline** with ground-truth fixtures covering:
   - simple text, multi-column, large multipage, image-heavy, special objects  
   - lattice / stream / partial-border / complex financial tables  
   - mixed pages, rotated pages, encrypted PDFs  
3. Measured **pdfplumber, pdfminer.six, pypdf, PyMuPDF, pypdfium2** on every fixture for:
   - success/failure, wall time, RSS delta  
   - token recall vs ground-truth strings  
   - table detection + cell token recall  
   - images, links, forms, outline where APIs exist  
4. Mapped each library’s **support matrix** against **what `pdfparser` plans to support** (design docs).

### 1.2 Headline findings (measured)

| Rank (this corpus) | Library | Strength | Weakness |
|--------------------|---------|----------|----------|
| **Speed (large doc)** | **pypdfium2** (27 ms / 80 pages) then **pypdf** (111 ms) | Fast text | Almost no structure/tables API |
| **Tables (ruled)** | **pdfplumber** + **PyMuPDF** | Lattice/complex/mixed tables perfect cell tokens | Stream (no borders) **both failed** to detect tables |
| **Geometry / layout objects** | **pdfplumber** (chars/lines/rects) | Table settings + visual debug | Slow + high memory on large docs |
| **Forms + outline + links** | **pypdf** / **PyMuPDF** | Full special-object story | No first-class tables (pypdf) |
| **Rotated pages** | pypdf, PyMuPDF, pypdfium2, pdfplumber OK | pdfminer.six **broke** rotated text (char soup, 0.5 recall) |
| **Encryption** | All correctly **refuse** without password; all succeed **with** password | — | Matches our 0.1 `Error::Encryption` posture if no password |

### 1.3 Strategic implication for `pdfparser`

Competitors force a **trade-off triangle**:

```text
           SPEED (Pdfium / MuPDF)
              /\
             /  \
            /    \
           /      \
          /________\
   STRUCTURE      SEMANTICS
  (pdfminer/       (tables, forms,
   plumber chars)   outline, IR)
```

- **No single library wins all three** on our corpus.  
- **Stream / borderless tables** are a **market gap** (pdfplumber + PyMuPDF both detected **0** tables).  
- **Partial-border hybrid tables** are a second gap (pdfplumber “detects” but returns **1×2 garbage grid**, not 5×5).  
- **Unified IR** (text geometry + images + paths + structure + tables + forms) is still product space—most tools are either *text engines* or *table tools*, not a coherent extract API.  
- **Rust + owned governor + metrics-correct text** remains differentiated vs AGPL MuPDF and vs slow pure-Python pdfminer stack.

---

## 2. Competitive landscape

### 2.1 Libraries evaluated (versions pinned at run time)

| Library | Version tested | Language / engine | License (approx.) | Stack notes |
|---------|----------------|-------------------|-------------------|-------------|
| **pdfplumber** | 0.11.10 | Python on **pdfminer.six** | MIT | Chars/lines/rects + table finder (Nurminen/Tabula-inspired) |
| **pdfminer.six** | 20260107 | Pure Python | MIT | Layout analysis (`LAParams`), no table API |
| **pypdf** | 6.14.2 | Pure Python | BSD | Merge/encrypt/forms; basic text |
| **PyMuPDF (fitz)** | 1.28.0 | Python + **MuPDF C** | AGPL / commercial | Fast engine; `find_tables()` in recent versions |
| **pypdfium2** | 5.11.0 | Python + **PDFium C/C++** | Apache-2 (bindings; PDFium licenses apply) | Chrome’s PDFium; text very fast |

### 2.2 Adjacent tools (not timed in v1 harness; analyzed for capability)

| Tool | Role | Why it matters for us |
|------|------|------------------------|
| **Camelot** | Lattice/stream tables (often Ghostscript) | Classic “table-only” product; lattice vs stream modes |
| **Tabula / tabula-py** | Java Tabula stream/lattice | Same taxonomy our design uses |
| **Unstructured** | Pipeline / layout ML hybrid | RAG-oriented; not a low-level parser |
| **Nougat / TATR** | DL text & tables | Strong on hard scientific/scanned; out of “native digital” scope |
| **Apache PDFBox** | Java full PDF | Enterprise Java baseline |
| **Poppler (pdftotext)** | C++ CLI | Unix default text dump |
| **lopdf / pdf-rs** | Rust object layer | Candidate backends for our ObjectBackend (not full extract products) |

Academic context: arXiv *“A Comparative Study of PDF Parsing Tools Across Diverse Document Categories”* (2024) found **PyMuPDF / pypdfium** strong on text across DocLayNet categories; table winners vary (TATR, Camelot, PyMuPDF by domain). That aligns with our speed ranking and “tables are hard / domain-dependent” story.  
Source: https://arxiv.org/html/2410.09871v1

Industry speed benchmarks (py-pdf/benchmarks) also show PyMuPDF ≫ pypdf ≫ pdfminer ≫ pdfplumber for pure text—consistent with our large-doc numbers once cold-start noise is ignored.

---

## 3. Capability matrix (features we want vs competitors)

Legend: **Full** = first-class API · **Partial** = possible with effort / limited · **None** = not productized · **N/A** = out of their model

| Feature (our design target) | Our phase | pdfplumber | pdfminer.six | pypdf | PyMuPDF | pypdfium2 |
|----------------------------|-----------|------------|--------------|-------|---------|-----------|
| Open unencrypted digital PDF | 0.1 | Full | Full | Full | Full | Full |
| Encrypted without password → clear error | 0.1 | Full (throws) | Full | Full | Full | Full |
| Encrypted with user password | 0.2 | Full | Full | Full | Full | Full |
| Plain text extraction | 0.1 | Full | Full | Full | Full | Full |
| Char/run **geometry** (bbox, font) | 0.1 | Full | Full | None | Full | Partial |
| Font metrics / ToUnicode fidelity | 0.1 | Via miner | Full low-level | Weak | Strong engine | Strong engine |
| Multi-column reading order | 0.1 | Partial | Partial | Partial | Better | Better |
| Page `/Rotate` handling | 0.1 | Full* | **Weak** (broken on fixture) | Full | Full | Full |
| Image XObject enumeration | 0.1 | Full | Partial | Full | Full | Partial/None in adapter |
| Image bytes export | 0.1 | Partial | Partial | Full | Full | Partial |
| Vector paths/lines/rects | 0.1–0.2 | Full | Full | None | Partial | None |
| **Lattice tables** | 0.2 | Full | None | None | Full | None |
| **Stream tables** (no lines) | 0.2 | **Failed fixture** | None | None | **Failed fixture** | None |
| **Hybrid / partial border tables** | 0.2 | Detects badly | None | None | Failed | None |
| Structure tree / tagged PDF | 0.1–0.2 | Partial (mcid exp.) | Partial | None | Partial | Limited |
| Annotations / links | P2 | Hyperlinks | Weak | Full | Full | Weak |
| AcroForm field values | P2 | Partial | None | Full | Full | Weak |
| Outline / bookmarks | P2 | None | Full | Full | Full | Weak |
| Optional content (OCG) | P2 | None/Partial | Partial | Partial | Partial | Partial |
| Unified versioned IR + JSON | 0.1 | None | None | None | None | None |
| Resource limits / DoS governor | 0.1 | None | None | None | Process-level | Process-level |
| Pure Rust / no C engine default | 0.1 | No | No (py) | No (py) | No (MuPDF) | No (PDFium) |
| OCR / scanned | later | No | No | No | Via other tools | Via other tools |

\*pdfplumber rotated text arrived character-wrapped but tokens present.

### 3.1 Gap map: scenarios we want that market still fails

| Scenario | Market status (this run) | `pdfparser` opportunity |
|----------|--------------------------|-------------------------|
| **Stream / whitespace tables** | 0/5 libraries produced a table structure | High — EX stream detector |
| **Hybrid partial borders** | Only pdfplumber “finds” table; **wrong grid** (1 col × 2 rows dump) | High — hybrid + confidence |
| **Unified extract IR** | Nobody ships one coherent Element IR + schema_version | High — product core |
| **Security governor** | Python libs expand freely; pdfplumber **~287 MB RSS delta** on 80-page doc | High — ResourceGovernor |
| **Rotated layout correctness** | pdfminer.six failed | Must match MuPDF/pypdf quality |
| **Forms + tables + geometry together** | Split across pypdf vs plumber vs MuPDF | One library API |
| **Native Rust embed** | Only via FFI to C engines (AGPL/complexity) | MIT/Apache pure-Rust story |
| Tagged structure tables | Not measured (synthetic corpus lacks real StructTreeRoot) | Our EX structure path still needed on real tagged PDFs |

---

## 4. Benchmark design

### 4.1 Corpus (12 fixtures)

| ID | Category | What it stresses |
|----|----------|------------------|
| `01_simple_text` | Simple digital text | Baseline tokens, metadata |
| `02_multi_column` | Two-column | Reading order |
| `03_large_multipage` | 80 pages | Throughput, memory |
| `04_image_heavy` | 4 PNGs + captions | Image discovery |
| `05_special_objects` | Link, AcroForm, outline | Annotations / forms / TOC |
| `06_table_lattice` | Full grid borders | Lattice tables |
| `07_table_stream` | No borders, aligned columns | Stream tables |
| `08_table_partial_border` | Outer box + header line only | Hybrid tables |
| `09_table_complex` | Financial 7×5 lattice | Dense numeric cells |
| `10_mixed_document` | Text + image + small table | Multi-element page |
| `11_rotated_page` | `/Rotate 90` on page 2 | Coordinate / extract correctness |
| `12_encrypted_password` | AES user password | Auth error vs decrypt |

Ground truth: `benchmark/ground_truth/*.json` (must-contain tokens, expected table shape, image counts, passwords, etc.).

### 4.2 Metrics

| Metric | Definition |
|--------|------------|
| **success** | No exception during open+extract |
| **wall_ms** | Wall clock open through full extract |
| **rss_delta_mb** | Process RSS increase during extract (approx.) |
| **token_recall** | Fraction of ground-truth `must_contain` substrings found in extracted text |
| **reading_order_score** | Fraction of consecutive ideal markers that appear in increasing offsets |
| **table_count** | Number of tables returned by native table API (0 if no API) |
| **table_cell_recall** | Fraction of `table_cells_must_include` found inside serialized cells |
| **image_count** | Images discovered via library API |
| **links / forms / outline** | Probed when API exists |

**Note on cold start:** First library load (pdfplumber/pymupdf/pypdfium2) can include hundreds of ms of import/native init. Prefer **median per-doc** and **large-doc** times for ranking; simple_text first-hit times are noisy.

### 4.3 Harness layout

```text
benchmark/
├── requirements.txt
├── corpus/                 # generated PDFs + manifest.json
├── ground_truth/           # per-doc expectations
├── assets/                 # PNGs used by generators
├── scripts/
│   ├── generate_corpus.py  # rebuild fixtures
│   └── run_benchmark.py    # adapters + metrics
└── results/
    ├── benchmark_results.json
    └── benchmark_results.tsv
```

Adapters implement a common extract contract; results are JSON for later **`pdfparser` competitor mode**.

---

## 5. Measured results (this machine)

**Environment:** macOS arm64 · Python 3.14 · venv packages as above · synthetic corpus only.

### 5.1 Overall summary (includes encrypted no-password failure as expected)

| Library | Successes / runs | Mean token recall (ok runs) | Mean wall_ms | Median wall_ms | Notes |
|---------|------------------|-----------------------------|--------------|----------------|-------|
| pdfplumber | 12/13 | **1.00** | 160.5 | 4.3 | Fail only encrypted w/o password |
| pdfminer.six | 12/13 | **0.96** | 106.1 | 5.2 | Rotated page hurts mean |
| pypdf | 12/13 | **1.00** | 12.7 | 1.2 | Fast pure Python |
| pymupdf | 12/13 | **1.00** | 230.5 | 5.6 | Cold start skews mean |
| pypdfium2 | 12/13 | **1.00** | 35.5 | **0.7** | Fastest median |

### 5.2 Throughput — 80-page large document (`03_large_multipage`)

| Library | wall_ms | pages/s (approx) | text_chars | rss_delta_mb |
|---------|---------|------------------|------------|--------------|
| **pypdfium2** | **27.3** | ~2930 | 145700 | ~0 |
| **pypdf** | **110.6** | ~723 | 143700 | ~3 |
| pdfminer.six | 1214.6 | ~66 | 145861 | ~4 |
| pdfplumber | 1508.2 | ~53 | 143620 | **~287** |
| pymupdf | 1614.0* | ~50 | 143700 | ~0 |

\*PyMuPDF large-doc time here is higher than typical industry benches (likely table finder path / first-use / measurement noise); still excellent on small docs (~3–6 ms). Re-check with text-only API in a future harness revision.

**Takeaway for us:** page-at-a-time + no global char cache is mandatory; pdfplumber’s memory spike is a **negative example**.

### 5.3 Text quality by scenario (token_recall)

| Doc | plumber | miner | pypdf | mupdf | pdfium |
|-----|---------|-------|-------|-------|--------|
| simple | 1.0 | 1.0 | 1.0 | 1.0 | 1.0 |
| multi_column | 1.0 | 1.0 | 1.0 | 1.0 | 1.0 |
| large | 1.0 | 1.0 | 1.0 | 1.0 | 1.0 |
| image_heavy | 1.0 | 1.0 | 1.0 | 1.0 | 1.0 |
| special | 1.0 | 1.0 | 1.0 | 1.0 | 1.0 |
| lattice | 1.0 | 1.0 | 1.0 | 1.0 | 1.0 |
| stream | 1.0 | 1.0 | 1.0 | 1.0 | 1.0 |
| hybrid | 1.0 | 1.0 | 1.0 | 1.0 | 1.0 |
| complex | 1.0 | 1.0 | 1.0 | 1.0 | 1.0 |
| mixed | 1.0 | 1.0 | 1.0 | 1.0 | 1.0 |
| rotated | 1.0 | **0.5** | 1.0 | 1.0 | 1.0 |
| encrypted + pw | 1.0 | 1.0 | 1.0 | 1.0 | 1.0 |
| encrypted no pw | FAIL | FAIL | FAIL | FAIL | FAIL |

**Rotated failure detail (pdfminer.six):** extracted text becomes vertical character soup (`R\nO\nT\nA\nT\nE\nD...`); `ROTATED_PAGE_TOKEN` not contiguous → token miss. **We must not regress to this.**

### 5.4 Reading order (`02_multi_column`)

Ideal marker order: `LEFT_COL_START` → `LEFT_COL_END` → `RIGHT_COL_START` → `RIGHT_COL_END`.

| Library | reading_order_score |
|---------|---------------------|
| pypdf | **1.0** |
| pymupdf | **1.0** |
| pypdfium2 | **1.0** |
| pdfplumber | **0.67** |
| pdfminer.six | **0.67** |

pdfminer/plumber interleave columns more often (paint-order / layout params). Our layout phase should target **≥ MuPDF quality** on multi-column fixtures.

### 5.5 Tables (the important gap)

| Doc | Expected | pdfplumber | PyMuPDF | Others |
|------|----------|------------|---------|--------|
| lattice 5×4 | 1 table, full cells | **1 table, cell_recall 1.0 (20 cells)** | **same** | 0 tables |
| stream 6×4 | 1 table | **0 tables** (text OK as lines) | **0 tables** | 0 |
| partial border 5×5 | 1 table | **1 table but 2 cells** (header row + body blob) cell tokens luckily present | **0 tables** | 0 |
| complex 7×5 | 1 table | **1, cell_recall 1.0 (35 cells)** | **same** | 0 |
| mixed 4×2 | 1 table | **1, cell_recall 1.0** | **same** | 0 |

**Stream table raw text** is recoverable as plain text (all libraries), but **none** returned a structured grid—exactly where Camelot “stream” / our stream detector must win.

**Hybrid partial border:** pdfplumber’s default settings merge everything into two mega-cells:

```text
[["Region Q1 Q2 Q3 Q4"],
 ["North 10 ... West 7 8 9 11"]]
```

Detection without **structure quality** is not success. Our confidence formula + cell F1 harness is justified.

### 5.6 Images

| Doc | Expected | plumber | miner | pypdf | mupdf | pdfium adapter |
|------|----------|---------|-------|-------|-------|----------------|
| image_heavy | 4 | 4 | 4 | 4 | 4 | not measured |
| mixed | 1 | 1 | 1 | 1 | 1 | not measured |

Image **discovery** is commodity. Differentiation = encoded/deferred policy, limits, masks, inline images, Form XObject nesting (our design)—not bare count.

### 5.7 Special objects (`05_special_objects`)

| Capability | plumber | miner | pypdf | mupdf | pdfium |
|------------|---------|-------|-------|-------|--------|
| URI link | Yes | No | Yes | Yes | No (adapter) |
| Form field names/values | Weak/raw annots | No | **Yes** (`customer_name=FORM_FIELD_VALUE_ALPHA`) | **Yes** | No |
| Outline titles | No | **Yes** | **Yes** | **Yes** | No |

**Implication:** for P2, copy **pypdf/MuPDF form+outline completeness**, not pdfplumber.

### 5.8 Encryption

| Mode | All five libraries |
|------|--------------------|
| No password | Exception / encrypted error — **correct** |
| Password `benchpass` | Text recall 1.0 — **correct** |

Aligns with product **K15**: 0.1 refuse encrypted; 0.2 decrypt subset.

---

## 6. Deep dive per library

### 6.1 pdfplumber

**Positioning:** Best-known **data-mining** PDF library for Python; built on pdfminer.six; famous for tables + visual debugging.

**Supports well**
- Character-level geometry, lines, rects, curves, images  
- Table detection with rich `table_settings` (vertical/horizontal strategy, snap tolerances)  
- Hyperlinks  
- Cropping / filtering objects  
- CLI dump of objects  

**Supports poorly / not at all**
- Outline API  
- Clean AcroForm values (raw annot dicts)  
- Stream tables without lines (failed our fixture)  
- Large-doc memory (huge RSS in our run)  
- Speed vs native engines  

**Algorithm (tables):** lines/rects → intersections → cells → contig tables (Nurminen / Tabula-inspired). Defaults = **lattice-oriented**. Stream mode needs explicit strategies (`text` alignment)—still failed default on our whitespace table.

**When users choose it:** financial scraping, ruled tables, layout debugging.

**vs pdfparser:** We should beat it on **stream/hybrid tables**, **memory**, **unified IR**, **Rust embed**, while matching lattice quality and char geometry.

### 6.2 pdfminer.six

**Positioning:** Canonical pure-Python PDF layout parser; foundation for pdfplumber.

**Supports well**
- Low-level layout (`LTChar`, `LTTextBox`, figures)  
- Configurable `LAParams`  
- Outlines  
- Encrypted open with password  

**Supports poorly**
- No table API  
- No forms API  
- Rotated pages can produce **unusable text** (measured)  
- Slow  

**vs pdfparser:** Use as **semantic reference** for layout box clustering ideas; do **not** copy rotation bugs; do not depend on it at runtime (Python).

### 6.3 pypdf

**Positioning:** Swiss-army pure Python PDF (merge, encrypt, forms, basic text).

**Supports well**
- Forms, outline, links, encryption  
- Reasonable multipage text speed  
- Image XObject walk  

**Supports poorly**
- No tables  
- Limited geometry  
- Text layout often one-token-per-line on columnar content (stream table sample)  

**vs pdfparser:** Competitor for **document manipulation**, not for **extract quality**. Our 0.1 is extract-only—don't compete on write/merge.

### 6.4 PyMuPDF

**Positioning:** De-facto **fast production** extractor; wraps MuPDF.

**Supports well**
- Text + dict/rawdict geometry  
- `find_tables()` on ruled content (excellent on lattice/complex/mixed)  
- Images, links, widgets, TOC  
- Rendering (out of our non-goals but market power)  

**Supports poorly / risks**
- **AGPL** (or commercial license) — hard for many products  
- Stream tables failed same as plumber on our fixture  
- Not “native pure Rust”  
- Binary size / FFI surface  

**vs pdfparser:** Primary **quality bar for text+lattice**. Primary **license/architecture alternative**. Optional **oracle** in our design (not runtime).

### 6.5 pypdfium2 (PDFium)

**Positioning:** Chrome PDFium bindings; extreme text speed.

**Supports well**
- Fast correct-enough text  
- Password docs  
- Rendering (engine)  

**Supports poorly (API surface)**
- No first-class tables  
- Structure/forms/outline weaker in typical bindings usage  
- Not pure Rust  

**vs pdfparser:** Speed oracle for text path; not a product competitor for full IR.

### 6.6 Camelot / Tabula (capability analysis; not timed v1)

| Mode | Idea | Maps to our detector |
|------|------|----------------------|
| Lattice | Use ruling lines | EX lattice |
| Stream | Use whitespace gaps | EX stream |
| Hybrid | Combine | EX hybrid |

These tools prove the **taxonomy** but:
- Often need Java/Ghostscript  
- Not full document IR  
- Fragile on partial borders / multi-page  

Our product should **absorb their table modes** into a broader library.

---

## 7. Scenario-by-scenario: support we want vs market

### 7.1 Simple digital text
- **Market:** Solved (all 1.0 recall).  
- **We:** Must match; differentiate confidence + geometry + warnings.

### 7.2 Multi-column reading order
- **Market:** MuPDF/pypdf/pdfium better than miner/plumber on markers.  
- **We:** Structure-first when tagged; else column clustering (design C1.4).

### 7.3 Large multipage
- **Market:** Pdfium/pypdf win speed; plumber loses memory.  
- **We:** Lazy pages, governor, no full-doc char cache.

### 7.4 Image-heavy
- **Market:** Count is easy.  
- **We:** Encoded default (K23), pixel budgets, masks, inline images.

### 7.5 Special objects
- **Market:** Split brain (pypdf/mupdf forms; miner outline; plumber links).  
- **We:** Single API for annots + forms + outline (P2).

### 7.6 Lattice tables
- **Market:** plumber + mupdf SOTA among tested.  
- **We:** Match cell F1; add confidence + NMS with structure.

### 7.7 Stream tables
- **Market:** **All failed structured extract.**  
- **We:** **Major differentiator** if quality is real.

### 7.8 Hybrid / partial border
- **Market:** False confidence (detect without grid quality).  
- **We:** Confidence formula + cell F1; don't ship detection without structure quality.

### 7.9 Complex financial tables
- **Market:** plumber/mupdf good on fully ruled.  
- **We:** Numeric/alignment scores; KV hints optional.

### 7.10 Mixed pages
- **Market:** Works if table ruled + image counted separately.  
- **We:** Unified page IR ordering (paint vs reading).

### 7.11 Rotation
- **Market:** Avoid miner failure mode.  
- **We:** Normative coordinate spaces from design B8.

### 7.12 Encryption
- **Market:** Consensus = fail closed without password.  
- **We:** K15 then 0.2 matrix.

---

## 8. Performance portrait (ASCII)

```text
Relative speed on 80-page digital text (lower bar = faster)

pypdfium2   |█                          27 ms
pypdf       |████                       111 ms
pdfminer    |████████████████████████   1215 ms
pdfplumber  |██████████████████████████ 1508 ms
pymupdf*    |███████████████████████████ 1614 ms  (*see note §5.2)

Table structure quality (lattice fixture)

pdfplumber  |####################  cell_recall 1.0
pymupdf     |####################  cell_recall 1.0
others      |                      no table API / 0 tables

Stream table structure quality

all tested  |                      0 tables detected
```

---

## 9. Implications for `pdfparser` product strategy

### 9.1 Quality bars to beat (use as acceptance targets)

| Area | Competitor bar | Our 0.1 / 0.2 target |
|------|----------------|----------------------|
| Simple/large text token recall | 1.0 on this corpus | ≥ same |
| Large 80-p text latency | ≤ pypdf (~100–150 ms class) aspirational; never plumber memory | Lazy + governor |
| Multi-column order | ≥ MuPDF/pypdf (score 1.0 on fixture) | Layout P1 |
| Lattice cell_recall | 1.0 plumber/mupdf | ≥ 0.9 cell F1 on eval set (0.2) |
| Stream tables | Market **0** | **Ship any reliable >0.55 conf** = win |
| Hybrid | Market broken | Confidence + don't emit junk grids |
| Forms/outline | pypdf/mupdf | Parity in P2 |
| Encryption | Fail closed | K15 |
| Rotated text | Not miner | Match pypdf/mupdf |

### 9.2 What NOT to copy

| Anti-pattern | Seen in | Our response |
|--------------|---------|--------------|
| Cache all chars for whole PDF | pdfplumber memory | Page-local IR, optional LRU |
| Table detect without cell grid quality | plumber hybrid | min_confidence + cell F1 |
| Python-only architecture | miner stack | Rust |
| AGPL by default | MuPDF | Pure Rust + optional oracle only |
| Text-only product | pdfium adapters | Full IR |

### 9.3 Recommended competitor set for ongoing CI

| Tier | Library | Role |
|------|---------|------|
| Always | PyMuPDF | Text + lattice oracle |
| Always | pypdfium2 | Speed oracle |
| Always | pdfplumber | Table + geometry reference |
| Optional | pypdf | Forms/outline reference |
| Optional | pdfminer.six | Layout regression canary (incl. rotation) |
| Future | Camelot stream/lattice | Table-mode comparison |

Harness should grow a **`pdfparser` adapter** returning the same JSON schema as soon as the Rust library exposes C-ABI or CLI JSON.

---

## 10. Limitations of this study

1. **Synthetic corpus** — generators (reportlab) are cleaner than wild PDFs; real corpus (arXiv, SEC filings, invoices) required before marketing claims.  
2. **No Camelot/Tabula/Unstructured timed** in v1 (extra system deps).  
3. **No tagged PDF / StructTreeRoot fixture** yet — structure-table path unmeasured.  
4. **No scanned/OCR** — correctly out of 0.1 scope.  
5. **Cold start / import noise** — first calls inflate means; use medians + large-doc.  
6. **Table cell_recall ≠ cell F1** — substring presence inside cells, not IoU grid match.  
7. **Single machine** — absolute ms not portable; ranks are.  
8. **PyMuPDF large-doc timing** may include non-optimal API path — re-benchmark text-only.

---

## 11. Roadmap for the benchmark (next iterations)

| Item | Why |
|------|-----|
| Add Camelot lattice+stream + Tabula | Classic table SOTA comparison |
| Add real PDFs (license-clean) | External validity |
| Tagged PDF with `/Structure` tables | Structure detector |
| Nested + multi-page tables | Design C3.6–C3.7 |
| Character CER vs oracle text | Design metrics gate |
| Memory via subprocess isolation | Cleaner RSS |
| `pdfparser` CLI JSON adapter | Direct A/B |
| pdfplumber stream `table_settings` sweep | Fairer plumber stream attempt |

---

## 12. Appendix A — Per-library feature checklist (detailed)

### pdfplumber
- Text: `Page.extract_text`, `extract_text_lines`, chars with fonts/sizes/matrix  
- Tables: `find_tables`, `extract_tables`, strategies lines/text/explicit  
- Images: `page.images`  
- Lines/rects/curves: first-class  
- Annots: `annots`, `hyperlinks`  
- Forms: limited  
- Debug: `to_image().debug_tablefinder()`  
- Password: supported  
- **Not:** PDF write, robust stream tables OOTB, outline, low memory mode by default  

### pdfminer.six
- Text + layout boxes via `extract_text` / `extract_pages`  
- LAParams for line/word grouping  
- Outlines via `PDFDocument.get_outlines`  
- **Not:** tables, forms UX, rotation robustness (measured)  

### pypdf
- Text `extract_text`  
- Images via XObject  
- `get_fields`, annotations, outline, encrypt/decrypt, merge  
- **Not:** tables, rich geometry, layout engine  

### PyMuPDF
- Text modes: text, blocks, dict, rawdict, html, xhtml  
- Tables: `Page.find_tables`  
- Images extract, drawings, widgets, TOC, links  
- Render pixmap  
- **License:** AGPL/commercial  

### pypdfium2
- Text page objects, render bitmaps  
- Password  
- **Not:** high-level tables/forms in thin adapters  

---

## 13. Appendix B — Mapping to our design documents

| Design artifact | Use of this report |
|-----------------|--------------------|
| `design-native-pdf-parser.md` | Product phases P0–P4 validated against market gaps |
| `design-architecture-feature-extraction.md` | Table EX detectors justified by stream/hybrid failures |
| PR plan | Prioritize text+geometry before tables; table harness PR 20 before detectors |
| Metrics gates | Competitor numbers become soft oracles |

---

## 14. Appendix C — Result files

| File | Content |
|------|---------|
| `benchmark/results/benchmark_results.json` | Full structured results |
| `benchmark/results/benchmark_results.tsv` | Spreadsheet-friendly |
| `benchmark/corpus/manifest.json` | Corpus index |
| `benchmark/ground_truth/*.json` | Per-doc expectations |

---

## 15. Conclusion

The market offers:

1. **Fast engines** (PDFium, MuPDF) with incomplete product IR.  
2. **Deep Python layout** (pdfminer/pdfplumber) with table superpowers on **ruled** grids, weak stream/hybrid, and costly memory/speed.  
3. **Document toolkits** (pypdf) strong on forms/encryption, weak on layout/tables.

**No competitor** on this benchmark delivers the full `pdfparser` vision:  
**secure, pure-Rust, metrics-correct text geometry + images + paths + structure + multi-mode tables with confidence + forms/outline in one versioned API.**

The biggest **near-term competitive openings** are:

1. **Stream & hybrid tables with real cell quality**  
2. **Bounded-memory large document extract**  
3. **Unified IR / JSON schema for embedders**  
4. **License-friendly native stack** (vs AGPL MuPDF)

Use this harness as the permanent **scoreboard**: when `pdfparser` lands, add an adapter and fill the same columns.

---

*Report generated 2026-07-10 · Benchmark corpus v1 · Libraries: pdfplumber 0.11.10, pdfminer.six 20260107, pypdf 6.14.2, PyMuPDF 1.28.0, pypdfium2 5.11.0*


---

# Part II — Extended Corpus v2: Real PDFs + Stress Tables (2026-07-10)

This section supersedes the “synthetic-only” limitation of Part I. Corpus **v2** adds **hard stress fixtures** and **real-world public PDFs** that expose failures basic fixtures never show.

## II.1 Corpus composition (v2)

| Tier | Count | Location | Purpose |
|------|------:|----------|---------|
| **basic** | 12 | `benchmark/corpus/*.pdf` | Smoke / regression |
| **stress** | 8 | `benchmark/corpus/stress/` | Controlled hard cases |
| **real** | 13 | `benchmark/corpus/real/` | Wild layouts from open sources |
| **Total** | **33** | `manifest.json` | |

### Stress fixtures (synthetic, controlled failure modes)

| ID | Scenario | Challenges injected |
|----|----------|---------------------|
| `20_bank_statement_multipage` | Multi-page checking ledger | Long wrapped descriptions, dense rows, page-broken table, mixed txn types |
| `21_table_overflow_cells` | Overflowing / multi-line cells | Narrow columns, mega wrapped text, uneven row heights |
| `22_table_merged_headers` | Colspan/rowspan headers | Two-level FY headers, empty placeholders |
| `23_side_by_side_tables` | Two tables side-by-side | Horizontal multi-table, asymmetric row counts |
| `24_invoice_line_items` | Invoice + totals spans | Long line items, footer spans, currency |
| `25_dense_numeric_grid` | 12×30 landscape grid | Tiny fonts, high cell count |
| `26_watermark_overlap` | Diagonal watermark | Overlapping text / z-order |
| `27_table_with_footnotes` | Table + footnotes | Superscripts, table boundary vs notes |

### Real PDFs (downloaded public samples)

| ID | Source | Why it’s hard |
|----|--------|----------------|
| `30_real_ca_warn_report` | pdfplumber examples (16 pp) | Wide multi-page government layoff tables |
| `31_real_background_checks` | pdfplumber examples | Multi-line headers, dense columns |
| `32_real_census_table324` | Tabula tests | Classic statistical abstract lattice |
| `33_real_argentina_votes` | Tabula tests | Irregular political voting grid |
| `34_real_schools_contributions` | Tabula tests (5 pp) | Stream-like campaign finance rows |
| `35_real_camelot_fuel` | Camelot sample | Scientific multi-table page |
| `36_real_two_tables` | Camelot tests | Multiple tables / health outbreak layout |
| `37_real_liabilities_superscript` | Camelot tests | Superscripts + “Contd.” financial table |
| `38_real_irs_f1040` | IRS.gov | Tax **form** (not data table) — false-positive trap |
| `39_real_fed_beigebook` | Federal Reserve (56 pp) | Long narrative report |
| `40_real_arxiv_tensorflow` | arXiv 1603.04467 (19 pp) | Multi-column scientific paper |
| `41_real_nist_*` | NIST series (~52 pp) | Long formal doc; line-heavy false tables |
| `42_real_insurance_italian` | Open sample | Non-English prose + table regions |

Sources catalog: `benchmark/sources.json`. Regenerate stress + re-copy reals:

```bash
python benchmark/scripts/generate_complex_corpus.py
python benchmark/scripts/run_benchmark.py
```

---

## II.2 What basic fixtures hid (and v2 exposes)

| Failure mode | Basic corpus | Stress/real v2 |
|--------------|--------------|----------------|
| Page-broken ledger tables | Not present | Bank statement → **2 tables** instead of 1 logical table |
| Overflowing cell text integrity | Perfect short cells | PyMuPDF **drops/mangles** long tokens in cells |
| Merged headers | Simple grids | Empty placeholder cells; partial span recovery |
| Side-by-side tables | Single table pages | Can work (plumber/mupdf=2) but text token corruption (mupdf) |
| Stream tables | Explicit fail | Still fail on basic stream; real schools = stream-like |
| Superscripts / continued tables | None | **Both plumber & mupdf: 0 tables** on liabilities PDF |
| False-positive tables | Rare | IRS form **7–15 “tables”**; NIST **83–99 “tables”** |
| Over-segmentation | Rare | `two_tables` real: plumber **20** vs mupdf **2** |
| Under-extraction structure | Rare | Census: plumber **8 cells** vs mupdf **17 cells** |
| Watermark / overlap | None | Several engines miss watermark token |
| Long docs (50+ pages) | 80p synthetic clean | Beige Book / NIST: multi-second extract, memory pressure |
| Multi-column science | Toy 2-col | arXiv TensorFlow: tables often figure captions / layout junk |

**Conclusion:** basic fixtures measure “can you read clean digital PDF text?” — not “are you a production extractor?”

---

## II.3 Stress results — table structure quality

Libraries with table APIs: **pdfplumber**, **PyMuPDF**. Others always `table_count=0`.

| Stress doc | pdfplumber tables/cells/cell_recall | PyMuPDF tables/cells/cell_recall | Limitation exposed |
|------------|-------------------------------------|----------------------------------|--------------------|
| Bank statement multi-page | **2 / 235 / 1.0** | **2 / 235 / 0.67** | Page break splits ledger; MuPDF **loses long overflow tokens** in cells (`TOKEN_BANK_OVERFLOW_DESC`, `TOKEN_BANK_WIRE`) |
| Overflow cells | 1 / 52 / **1.0** | 1 / 52 / **0.5** | MuPDF mangles/misses `TOKEN_OVERFLOW_MEGA` & multiline token in cell text |
| Merged headers | 1 / 38 / **1.0** | 1 / 38 / **0.8** | Span headers become empty string cells; MuPDF misses some note tokens |
| Side-by-side | **2 / 18 / 1.0** | **2 / 18 / 0.5** | Count OK; MuPDF corrupts underscores: `TOKEN_SIDE_L` → `TOKEN SIDE L` with newlines |
| Invoice | 1 / 32 / **1.0** | 1 / 32 / **0.4** | Long description tokens often missing from MuPDF cell strings |
| Dense 12×30 | 1 / 372 / **1.0** | 1 / 372 / **0.67** | Grid found; marker token missing in MuPDF cells |
| Footnotes table | 1 / 15 / **1.0** | 1 / 15 / **0.5** | Underscore/token corruption again in MuPDF |
| Watermark | 0 tables; text recall **0.75** | 0 tables; recall **0.75** | Both miss watermark token; miner/pypdf get full text |

### Critical insight: “table found” ≠ “cell text usable”

PyMuPDF often **matches pdfplumber on geometry** (same table_count, cell_count, row_counts) but **fails cell token recall** because extracted cell strings rewrite characters (spaces inserted into identifiers, underscores broken).  

For `pdfparser`, evaluation must use:

1. table detection  
2. grid shape  
3. **normalized cell text fidelity**  
4. multi-page table stitching  
not detection alone.

### Bank statement specifics

```text
Logical table: 1 ledger across 2 pages (header repeats)
pdfplumber / pymupdf: detect 2 tables (rows 38 + 9)  ← no multi-page stitch
pypdf / pdfminer / pdfium: 0 tables (text only)

Overflow description length: hundreds of chars in Description column
→ pdfplumber cell_recall 1.0
→ pymupdf missing long TOKEN_* strings inside cells
```

This is exactly production bank-statement pain.

---

## II.4 Real PDF results — limitation catalog

| Real doc | pdfplumber | PyMuPDF | pypdf/pdfium | What breaks |
|----------|------------|---------|--------------|-------------|
| CA WARN (16p) | 17 tables, **4537 cells**, ~2.0s | same structure, ~2.3s | text only, pdfium **32ms** | Speed vs structure trade-off; still no schema/types |
| Background checks | 1×313 cells | same | text only | Multi-line headers — structure OK for both table engines |
| Census 324 | **2 tables, only 8 cells** | 2 tables, **17 cells** | text | **Under-segmented cells** on classic lattice — plumber worse |
| Argentina votes | 1×128 | 2×130 | text | Split vs merge disagreement |
| Schools contributions | 5×2259 | same | text | Large stream-like multipage; heavy |
| Camelot fuel | 1×43 | 1×43 | text | OK small lattice |
| Two tables (health) | **20 tables** / 117 cells | **2 tables** / 69 cells | text | **Catastrophic over-segmentation** (plumber) |
| Liabilities superscript | **0 tables** | **0 tables** | text | **Both SOTA table tools fail** (superscripts/contd) |
| IRS 1040 form | **15 tables** | **7 tables** | text | **False positives** on form rules/lines |
| Fed Beige Book 56p | 2 tiny tables / 7 cells, ~2.0s | 0 tables, ~1.5s | pdfium **41ms** | Narrative docs: tables noise; speed gap huge |
| arXiv TensorFlow 19p | 3 tables / 12 cells | 6 / 26 | pdfium **25ms** | Multi-column science; “tables” often junk |
| NIST long | **99 tables** / 3030 cells, ~1.9s | **83** / 2957, ~3.1s | pdfium **43ms** | **Massive false-positive tables** on ruled layouts |
| Italian insurance | 2 / 21 | 2 / 21 | text | Non-English OK for detection count |

### Ranked real-world limitations by severity

1. **False-positive tables on forms & ruled prose** (IRS, NIST) — ship with high confidence filters or users drown in junk.  
2. **Complete miss on superscript/continued financial tables** (liabilities) — both plumber & mupdf.  
3. **Over-segmentation** (20 tables when ~2 exist).  
4. **Under-extraction of cell content** (census 8 cells).  
5. **No multi-page table identity** (WARN/schools = many page tables, not one logical table).  
6. **Cell text corruption** (MuPDF underscores/tokens) on stress suite.  
7. **No table API** on pypdf/pdfminer/pdfium — fine for text RAG, useless for structured finance.  
8. **Memory/time** on wide multipage tables (WARN ~2s; plumber historically high RSS on large).  

---

## II.5 Updated competitor scorecard (after v2)

| Capability | pdfplumber | pdfminer | pypdf | PyMuPDF | pypdfium2 | **pdfparser target** |
|------------|------------|----------|-------|---------|-----------|----------------------|
| Clean lattice | Strong | — | — | Strong geom | — | Match + confidence |
| Long wrapped bank cells | **Strong text** | — | — | **Weak cell text** | — | **Beat MuPDF text fidelity** |
| Multi-page ledger stitch | Weak (splits) | — | — | Weak | — | **Explicit continuation** |
| Stream / borderless | Weak | — | — | Weak | — | Differentiator |
| Superscript / contd tables | **Fail** | — | — | **Fail** | — | Hard research item |
| Side-by-side multi-table | Count OK | — | — | Count OK, text issues | — | NMS + text quality |
| Form-as-table FP | High FP | — | — | High FP | — | Form detector vs table |
| Long narrative speed | Slow | Slow | Mid | Mid | **Fast** | Lazy + governor |
| Multi-column science | OK text | Rotated issues elsewhere | OK | OK | **Fast** | Reading order |
| Unified IR | No | No | No | No | No | **Yes** |

---

## II.6 Implications for `pdfparser` (revised)

### Must-have evaluation fixtures (gate development)

| Fixture class | Why mandatory |
|---------------|---------------|
| Bank multi-page + overflow desc | Real finance workloads |
| Overflow / multi-line cells | Cell integrity |
| Merged headers | Span model |
| Side-by-side | Multi-table NMS |
| Stream borderless | Known market gap |
| Superscript / contd (real 37) | Known dual failure |
| IRS/NIST FP set | Confidence calibration |
| Over-segmentation set (real 36) | Precision |
| CA WARN scale | Performance + memory |
| arXiv multi-col | Reading order |

### Product requirements sharpened by v2

1. **Table confidence** must suppress form/NIST-style false positives.  
2. **Multi-page table stitching** (`continued_from_previous_page`) is not optional for statements.  
3. **Cell text pipeline** must preserve underscores, long wraps, unicode superscripts.  
4. **Separate “form layout” path** from “data table” path (1040 vs WARN).  
5. **Stream detector** still wide open competitively.  
6. **Speed tier**: pdfium remains text-speed oracle; do not use table quality from non-table engines.  
7. **Never claim table SOTA** from basic 5×4 fixtures alone.

### Competitor-specific notes for design

| Library | Copy | Avoid |
|---------|------|-------|
| pdfplumber | Lattice heuristics, cell text quality, table_settings knobs | Global char cache memory; FP on forms; over-segmentation |
| PyMuPDF | Fast geometry, multipage throughput | AGPL; cell string rewriting; still no stream/superscript magic |
| pypdfium2 | Text latency target | Assuming structure from text |
| pypdf | Forms/outline reference | Using as table engine |
| pdfminer | Layout box ideas | Rotation bugs; no tables |

---

## II.7 Aggregate timing (full v2 run, 33 docs + encrypted variants)

| Library | Successes | Mean wall_ms | Median wall_ms | Mean token recall |
|---------|-----------|--------------|----------------|-------------------|
| **pypdfium2** | 33/34 | **9.1** | **1.7** | 0.992 |
| pypdf | 33/34 | 69.4 | 5.0 | 1.0 |
| pdfplumber | 33/34 | 302 | 31 | 0.992 |
| pdfminer.six | 33/34 | 365 | 26 | 0.984 |
| pymupdf | 33/34 | 387 | 37 | 0.992 |

(Failure = encrypted without password — expected.)

---

## II.8 Artifacts

| Path | Content |
|------|---------|
| `benchmark/scripts/generate_complex_corpus.py` | Stress generators + real PDF registry |
| `benchmark/corpus/stress/` | Hard synthetic PDFs |
| `benchmark/corpus/real/` | Copied real samples |
| `benchmark/downloads/` | Original downloads |
| `benchmark/sources.json` | Provenance / license notes |
| `benchmark/results/benchmark_results.json` | Full v2 measurements |
| `benchmark/ground_truth/*.json` | Challenges + probes per doc |

---

## II.9 Bottom line

After v2, the market picture is sharper:

- **Text extraction** on digital PDFs is largely a solved commodity (pdfium wins on speed).  
- **Ruled table detection** is available (plumber/mupdf) but **fragile** on real documents: false positives, over/under-segmentation, page splits, and **cell text corruption**.  
- **Hard tables** (overflow, multi-page statements, superscripts, stream, form-like) still **break or degrade every popular library**.  
- **`pdfparser`’s opportunity is not “another text dumper”** — it is **robust structured extraction with confidence, multi-page semantics, cell fidelity, and FP control**, in a secure Rust library API.

*End of Part II — Extended Corpus Analysis*
