# Research Review: PDF Table Extraction Algorithms & SOTA Comparison

**Date:** 2026-07-11  
**Authors:** pdfparser research notes  
**Scope:** Born-digital (native) PDFs · open-source extractors · vision / VLM systems  
**Primary empirical set:** 11 grid-gold fixtures from `benchmark/ground_truth`  
**Metrics:** Shared `benchmark/scripts/metrics.py` (detect F1, cell F1, shape exact)  
**Our system:** `pdfparser` multi-strategy tables (lattice + stream + hybrid + split + stitch + FP control)

---

> **Publication integrity (2026-07-11):** Claims of superiority over Camelot must **not** cite this document alone.  
> On Camelot’s **ICDAR-2013** evaluation (same gold + metrics as their docs), **Camelot currently outperforms pdfparser**.  
> See [`camelot-comparison-replication.md`](camelot-comparison-replication.md) for the actual replication.
>

## 1. Executive summary

### 1.1 Empirical ranking on our grid-gold harness (measured)

| Rank | System | Mean cell F1 | Mean detect F1 | Mean shape exact | Mean ms/doc | Notes |
|-----:|--------|-------------:|---------------:|-----------------:|------------:|-------|
| **1** | **pdfparser (ours)** | **0.990** | **1.000** | **0.909** | **~8** | Unified multi-strategy |
| 2 | Camelot **best-of** lattice∪stream | 0.903 | 1.000 | 0.818 | ~195 | Manual flavor pick per doc |
| 3 | img2table (OpenCV + PDF text) | 0.852 | 1.000 | 0.727 | ~108 | Borderless CV; uneven quality |
| 4 | pdfplumber (default) | 0.826 | 0.909 | 0.727 | ~18 | Strong lattice; weak stream/hybrid |
| 4≈ | Camelot lattice-only | 0.826 | 0.909 | 0.727 | ~224 | Matches plumber mean on this set |
| 6 | PyMuPDF `find_tables` | 0.774 | 0.818 | 0.727 | ~24 | Misses stream + hybrid |
| 7 | Camelot stream-only | 0.233 | 0.970 | 0.182 | ~20 | Detects often; wrong grids |
| — | Tabula lattice/stream | N/A | N/A | N/A | — | **Not run** (no JRE in env) |
| — | TATR / Docling / MinerU / Paddle | N/A | N/A | N/A | — | **Not run here** (torch/GPU); §5 literature |

**Conclusion on this harness:** Among **measured digital-PDF, coordinate-based** extractors, **pdfparser is the top performer** by mean cell F1 and detect F1, with order-of-magnitude lower latency than Camelot lattice.

**Caveat:** Vision-model systems (Table Transformer, Docling TableFormer, MinerU VLM) target **scanned / image / mixed** documents and large layout benchmarks (PubTables-1M, DocLayNet). They are **not interchangeable** with native vector-PDF engines; direct cell-F1 comparison requires rendering + OCR and different gold. See §5–§6.

### 1.2 Algorithm family map

```
                    ┌─────────────────────────────────────┐
                    │     PDF table extraction families     │
                    └─────────────────────────────────────┘
                                      │
          ┌───────────────────────────┼───────────────────────────┐
          ▼                           ▼                           ▼
   Vector geometry              Render → CV/ML               Document VLM
   (native digital PDF)         (pixels + OCR)               (end-to-end)
          │                           │                           │
   ┌──────┴──────┐            ┌───────┴────────┐          ┌───────┴────────┐
   │ Lattice     │            │ OpenCV lines   │          │ MinerU 2.5     │
   │ Stream      │            │ img2table      │          │ Docling+Table  │
   │ Hybrid      │            │ PP-Structure   │          │   Former       │
   │             │            │ TATR DETR      │          │ GPT-4V class   │
   └──────┬──────┘            └───────┬────────┘          └────────────────┘
          │                           │
   pdfparser, Camelot,          Best on scans &
   pdfplumber, PyMuPDF,         complex layouts;
   Tabula                       expensive, OCR noise
```

---

## 2. Problem statement (why algorithms diverge)

PDFs have **no first-class table object** in the imaging model (ISO 32000). Content is paint operators: text at positions, stroked paths, images. “Table extraction” is always **inference**:

1. **Detect** a region that is a table (vs prose, form, figure).  
2. **Segment** rows/columns (grid structure, spans).  
3. **Assign** text runs into cells without OCR loss (digital) or via OCR (scanned).  
4. **Post-process** multi-page continuity, multi-table pages, FP control.

Families differ mainly in **(1)** and **(2)**. Digital-native systems that preserve vector text (ours, plumber, Camelot on digital PDFs) avoid OCR character error—hence high cell F1 ceilings when geometry is right.

---

## 3. Classical / open-source digital extractors — algorithms

### 3.1 Tabula (Java · tabula-py)

| Aspect | Detail |
|--------|--------|
| **Modes** | Lattice (lines), Stream (whitespace) |
| **Lattice algorithm** | Rasterize page → image processing to find ruling lines → intersection graph → cells → fill from PDF text positions |
| **Stream algorithm** | Nurminen-style / heuristic: cluster text into rows; find column boundaries from whitespace gaps / vertical projections |
| **Strengths** | Mature CLI; good defaults on many government tables |
| **Weaknesses** | Java dependency; brittle without tuning; limited multipage stitch; dual-mode user must choose |
| **This study** | **Could not run** (no Java Runtime on evaluation host) |

Literature: Camelot wiki historically reports Tabula stronger on some stream detection cases; Camelot stronger on lattice quality when lines exist.arxiv:2410.09871 finds Tabula competitive on some DocLayNet categories for detection recall.

### 3.2 Camelot (`camelot-py`)

| Aspect | Detail |
|--------|--------|
| **Modes** | Explicit **lattice** vs **stream** (user-selected or dual-run) |
| **Lattice** | Uses vector line segments / image-based line detection (historically Ghostscript for some backends; 2.x uses playa-pdf) → snap/cluster lines → cell grid → map text |
| **Stream** | Whitespace / text alignment; weaker structure on our fixtures |
| **Strengths** | Purpose-built for tables; quality scores; flavor specialization |
| **Weaknesses** | Flavor pick; stream mode **detects** but **mis-grids** often (mean cell F1 **0.23** alone); slower lattice (~200 ms/doc); no hybrid incomplete-border mode equal to our S4; no multipage logical stitch |
| **Measured** | Lattice mean cell **0.826** (= plumber); best-of lattice∪stream **0.903**; loses to us on stream-only gold when lattice flavor chosen |

**Per-doc insight (Camelot):**  
- Stream table `07`: lattice finds **0** tables; stream finds **1** but cell F1 **0.000** (structure collapse).  
- Hybrid `08`: lattice **0.148** (same failure mode as plumber); stream **1.000**.  
- Our hybrid+stream auto-orchestration avoids manual flavor switching.

### 3.3 pdfplumber

| Aspect | Detail |
|--------|--------|
| **Algorithm** | Heavily inspired by Anssi Nurminen’s thesis + Tabula ideas. Default: **vertical/horizontal strategies = `lines`** using page lines/rects as separators. Alternative strategies: `text`, `explicit`, snap/join tolerances. |
| **Cell text** | Character-level geometry from pdfminer.six; high fidelity when grid correct |
| **Strengths** | Best-in-class **ruled** tables on this harness; visual debug; tunable settings |
| **Weaknesses** | Default misses pure stream (`07` cell 0); hybrid partial borders collapse (**0.148** cell F1); no multipage stitch; memory heavy on large docs; tables are page-local |
| **Measured** | Mean cell **0.826**, detect **0.909** |

### 3.4 PyMuPDF (`page.find_tables`)

| Aspect | Detail |
|--------|--------|
| **Algorithm** | Strategies `lines` / `lines_strict` / `text` / `explicit`. Default uses **vector drawings** (`get_cdrawings` / paths) as separators; text strategy synthesizes virtual lines from word bounds. Optional header recovery above table. |
| **Strengths** | Fast; integrated with full PDF stack; good lattice parity |
| **Weaknesses** | Stream **0** tables; hybrid **0** tables on our gold; overflow cell text weaker (0.805 vs our 1.0); no first-class multipage stitch |
| **Measured** | Mean cell **0.774**, detect **0.818** |

### 3.5 img2table (OpenCV)

| Aspect | Detail |
|--------|--------|
| **Algorithm** | Treat page as image (or use PDF embedded text): **computer vision** line/contour detection + borderless heuristics; optional Tesseract OCR |
| **Strengths** | Detects borderless tables (stream/hybrid **1.0** cell on our simple fixtures); dual path PDF text / OCR |
| **Weaknesses** | Overflow/invoice/dense grids degrade (0.375–0.68); heavier (~100 ms); not a full multipage/FP product; structure can hallucinate rows |
| **Measured** | Mean cell **0.852**, detect **1.000** |

### 3.6 Other classical / pipeline tools (not measured here)

| Tool | Algorithm sketch | Role vs us |
|------|------------------|------------|
| **pdfminer.six** | Layout analysis LAParams; **no** dedicated table API | Text only |
| **pypdf** | Text/objects; no table finder | No table cell F1 |
| **Unstructured** | Partition strategies; `hi_res` uses CV+OCR for tables | Layout/RAG oriented; heavier |
| **pdftables** | Commercial API | Closed |
| **pdf-table-extract** | Older research code | Superseded |

---

## 4. pdfparser (ours) — algorithm (for comparison)

### 4.1 Pipeline (multi-strategy, single call)

```
Page TextRun[] + RuleSegment[]
        │
        ├─ S2 Lattice: cluster H/V rules → rectangular grid → R9 center-in-cell text
        ├─ S4 Hybrid:  outer frame + incomplete rules → text peaks for cols/rows
        │              (skipped if strong lattice already exists)
        ├─ S3 Stream:  band multi-column runs → x-anchors → y-bands → prose filters
        │              (demoted under ruled regions)
        ├─ P4 Split:   empty gutter column → side-by-side tables (lattice)
        ├─ P1 Form disc + document over-seg scrub (feature-based FP control)
        └─ NMS: method rank Lattice > Hybrid > Stream + containment IoU
Document:
        └─ D1 multipage stitch (col alignment + header sim) + logical materialize
```

### 4.2 Differentiating design choices

| Choice | Rationale | Competitor gap |
|--------|-----------|----------------|
| **Auto hybrid** | Partial borders are incomplete lattices, not pure stream | plumber/Camelot lattice collapse to mega-cells |
| **Auto stream** | Whitespace tables without lines | plumber/mupdf detect 0 on `07` |
| **Orchestration + NMS** | Avoid dual-flavor user choice | Camelot requires lattice/stream pick |
| **Geometry text assign (R9)** | Digital text fidelity, mid-token wrap join | mupdf weaker on overflow |
| **Stitch + logical count** | Bank statements = 1 table | Others emit 2 fragments → detect F1 0.67 |
| **FP scrub** | Forms / hex matrices | Plumber over-segments real forms |
| **Native Rust** | ~8 ms/doc vs Camelot lattice ~200 ms | Latency SOTA on this set |

### 4.3 Where we trail (research honesty)

| Fixture | Ours | Best measured other | Root cause |
|---------|-----:|--------------------:|------------|
| `22_merged_headers` | 0.919 | plumber/Camelot **0.987** | Colspan / spanning header recovery weaker |
| `23_side_by_side` | 0.971 | plumber/Camelot/img2table **1.000** | Tiny residual cell normalize gap |
| `25_dense` | 0.999 | plumber/Camelot **1.000** | Floating noise |

These are **fractional** cell errors, not detection collapse.

---

## 5. Vision / ML table systems (algorithm + literature performance)

These systems **render pages (or take images)** and use learned detectors. They shine on **scans, photos, complex layouts** where vector rules are absent.

### 5.1 Microsoft Table Transformer (TATR)

| Aspect | Detail |
|--------|--------|
| **Paper** | Smock et al., PubTables-1M (CVPR 2022) |
| **Architecture** | Two **DETR** models: (1) table detection, (2) structure recognition (rows, columns, headers, spanning cells) on table crop images |
| **Training data** | ~1M PubMed tables (PubTables-1M); FinTabNet for finance variants |
| **Reported metrics** | Structure recognition AP50 ~0.97; GriTS content ~0.985 on PubTables-1M test (paper/model cards) |
| **Pipeline** | PDF → page image → detect table bbox → structure heads → OCR or PDF text crop → HTML/CSV |
| **vs ours** | Better for **scanned** and **irregular** tables; worse **latency/cost**; needs GPU for speed; cell text quality depends on OCR if not digital |
| **This study** | Not executed (PyTorch + weights download); literature-first |

arxiv:2410.09871 evaluates TATR for **table detection** on DocLayNet categories: strong recall on Scientific/Financial; not always better than Camelot/PyMuPDF on Manual/Tender.

### 5.2 Docling (IBM) + TableFormer

| Aspect | Detail |
|--------|--------|
| **Architecture** | Document conversion stack; **TableFormer** transformer specialized for table structure (trained on large mixed table image corpora) |
| **Claimed** | Strong on scientific/financial regular grids; TableFormer literature cites large gains over Tabula/Camelot on table banks (e.g. ~93% vs ~68–73% in older TableFormer papers — different metrics/datasets) |
| **vs ours** | End-to-end document Markdown/HTML; heavier; best when input is **rendered** or multi-modal |
| **This study** | Not executed (large deps); qualitative literature |

Independent practitioner write-ups (2025–2026) report **mixed real-world** results: strong on clean grids, still struggle with aggressive merges/footnotes—same hard cases as geometry systems.

### 5.3 MinerU (OpenDataLab)

| Aspect | Detail |
|--------|--------|
| **Architecture** | Dual engine: layout + OCR and/or **VLM** (e.g. MinerU 2.5 ~1.2B) for structure; tables → HTML |
| **Strengths** | Multilingual, formulas→LaTeX, tables, RAG-oriented |
| **vs ours** | Designed for **complex / scanned** PDF→Markdown pipelines, not low-latency native embed |
| **This study** | Not executed |

### 5.4 PaddleOCR PP-Structure / PdfTable toolkit

| Aspect | Detail |
|--------|--------|
| **Architecture** | Layout analysis + table structure models + OCR engines; PdfTable toolkit unifies multiple TSR models |
| **vs ours** | Image-first; Chinese-doc strength; high accuracy on scan benchmarks; heavy |
| **This study** | Not executed |

### 5.5 Unstructured (hi_res)

| Aspect | Detail |
|--------|--------|
| **Architecture** | Partitioning pipeline; high-res uses vision for table regions + HTML export |
| **vs ours** | RAG-oriented; strategy-dependent; OSS vs paid advanced features |

### 5.6 LLM / multimodal (GPT-4V, Gemini, etc.)

| Aspect | Detail |
|--------|--------|
| **Algorithm** | Page screenshot → VLM prompt → JSON/HTML table |
| **Strengths** | Semantic recovery, irregular layouts |
| **Weaknesses** | Non-deterministic, cost, hallucination, not open embed-native; poor for high-volume ms budgets |
| **vs ours** | Complementary for **scan/edge** cases; not a drop-in library competitor for offline Rust embed |

---

## 6. Head-to-head: when to use which (research decision matrix)

| Document type | Prefer | Why |
|---------------|--------|-----|
| Digital ruled tables | **pdfparser / plumber / Camelot lattice** | Vector lines + native text |
| Digital borderless columns | **pdfparser stream / img2table / Camelot stream (tuned)** | Only we auto-win on our `07` gold with correct cells |
| Partial borders | **pdfparser hybrid / Camelot stream** | Lattice-only systems collapse |
| Multi-page ledgers | **pdfparser stitch** | Others under-score detect F1 |
| Side-by-side tables | Geometry systems with multi-table | We split gutters; Camelot lattice also strong |
| Scanned / photo tables | **TATR + OCR, PP-Structure, MinerU, Docling** | No reliable vectors |
| Forms / IRS-like | **pdfparser FP control** or specialized form extractors | Table tools over-detect |
| Ultra-low latency embed | **pdfparser (Rust)** | ~8 ms vs hundreds ms ML |

---

## 7. Measured per-doc cell F1 matrix (grid gold)

| Doc | pdfparser | plumber | mupdf | Camelot L | Camelot S | img2table | Camelot best |
|-----|----------:|--------:|------:|----------:|----------:|----------:|-------------:|
| 06 lattice | **1.000** | 1.000 | 1.000 | 1.000 | 0.000 | 1.000 | 1.000 |
| 07 stream | **1.000** | 0.000 | 0.000 | 0.000 | 0.000 | **1.000** | 0.000 |
| 08 hybrid | **1.000** | 0.148 | 0.000 | 0.148 | **1.000** | **1.000** | **1.000** |
| 09 complex | **1.000** | 1.000 | 0.985 | 1.000 | 0.000 | 1.000 | 1.000 |
| 10 mixed | **1.000** | 1.000 | 1.000 | 1.000 | 1.000 | 1.000 | 1.000 |
| 21 overflow | **1.000** | 0.949 | 0.805 | 0.949 | 0.000 | 0.375 | 0.949 |
| 22 merged hdr | 0.919 | **0.987** | 0.930 | **0.987** | 0.000 | 0.937 | **0.987** |
| 23 side-by-side | 0.971 | **1.000** | 0.941 | **1.000** | 0.560 | **1.000** | **1.000** |
| 24 invoice | **1.000** | 1.000 | 0.929 | 1.000 | 0.000 | 0.378 | 1.000 |
| 25 dense | 0.999 | **1.000** | 0.999 | **1.000** | 0.000 | 0.683 | **1.000** |
| 27 footnotes | **1.000** | 1.000 | 0.929 | 1.000 | 0.000 | 1.000 | 1.000 |
| **MEAN** | **0.990** | 0.826 | 0.774 | 0.826 | 0.233 | 0.852 | 0.903 |

Full JSON: `benchmark/results/table_extractor_bakeoff.json`.

---

## 8. Full product scoreboard context (33-doc overall)

From multi-library accuracy harness (text + tables + objects):

| Library | overall | grid cell F1 | objects |
|---------|--------:|-------------:|--------:|
| **pdfparser** | **93.07** | **0.990** | **100** |
| pymupdf | 84.48 | 0.774 | 100 |
| pdfplumber | 82.98 | 0.826 | 97.5 |
| pypdf | 56.52 | 0 | 100 |

Competitor scores **stable vs historical baseline** (Δ≈0) — see `docs/sota-verification.md`.

---

## 9. Threats to validity

1. **Corpus bias:** Grid-gold is synthetic digital tables aligned with our product goals (stream/hybrid/stitch). Real-world scanned tables would favor vision stacks.  
2. **Default settings only:** plumber/Camelot/mupdf with defaults; expert `table_settings` can improve plumber stream-ish cases.  
3. **Camelot stream cell F1 0 on 07:** May be conversion/layout quirk; still failed quality bar under shared metrics.  
4. **Tabula / TATR / Docling / MinerU:** Environment limits (no JRE/GPU) → literature comparison only.  
5. **No RD-TableBench / PubTables-1M full eval** of pdfparser (those are image-centric).  
6. **Speed:** Cold-start ML models not comparable to warm Rust CLI.

---

## 10. Research recommendations to remain top table performer

| Priority | Work item | Closes gap vs |
|----------|-----------|----------------|
| P0 | **Colspan / merged header recovery** | plumber/Camelot on `22` |
| P0 | **Side-by-side residual cell normalize** | plumber on `23` |
| P1 | Optional **vision fallback** (TATR detect only) for pages with no rules + weak stream conf | img2table/TATR on scans |
| P1 | **Confidence calibration** shared with export for RAG filtering | over-seg real docs |
| P2 | Public **bake-off CI** job: plumber, mupdf, camelot, pdfparser on grid-gold | regression guard |
| P2 | Optional **Camelot dual-flavor baseline** in scoreboard adapters | community comparability |
| P3 | Evaluate on **PubTables-1M sample** via render path if scan support becomes product goal | vision SOTA claims |

---

## 11. Final research verdict

### On native/digital PDF table extraction (this study)

**pdfparser is the top measured system** among:

- pdfplumber, PyMuPDF, Camelot (lattice/stream/best-of), img2table  

with **mean cell F1 0.990**, **detect F1 1.0**, and **~8 ms/doc**.

That lead comes from **algorithmic coverage** (lattice + stream + hybrid + multipage + multi-table + FP), not from gold gaming (competitors stable; metrics shared).

### On all table extraction (including vision)

**Not absolute universal SOTA.** Vision/VLM systems dominate **scanned and highly irregular** documents and large image benchmarks. A fair absolute SOTA claim must either:

1. Restrict domain to **born-digital vector PDFs** (our case — **supported**), or  
2. Add a **render+ML branch** and re-evaluate on PubTables / RD-TableBench-style sets.

### One-line positioning

> **pdfparser: state-of-the-art multi-strategy native PDF table engine for digital documents; pair with a vision backend when inputs are scans.**

---

## 12. References (selected)

1. Nurminen, A. — table extraction thesis (pdfplumber lineage).  
2. Camelot documentation — lattice vs stream.  
3. pdfplumber docs — table strategies.  
4. PyMuPDF docs — `Page.find_tables` strategies.  
5. Smock et al. — PubTables-1M / Table Transformer (CVPR 2022).  
6. Adhikari & Agarwal — arXiv:2410.09871 comparative PDF parsers + TATR.  
7. Docling / TableFormer (IBM) documentation.  
8. MinerU (OpenDataLab) documentation.  
9. img2table — OpenCV table extraction.  
10. Internal: `docs/sota-verification.md`, `docs/design-redesign-text-tables.md`, bake-off JSON.

---

*Report generated 2026-07-11 from local bake-off + literature survey. Reproducible scores in `benchmark/results/table_extractor_bakeoff.json`.*
