# Architecture Redesign for Competitive Text Parity and Best-in-Class Table Extraction

| Field | Value |
|-------|-------|
| **Title** | Architecture Redesign — Text Parity + Table Excellence |
| **Document type** | Architecture & LLD redesign (extends Volume 2; does not reopen product K-lock without R-IDs) |
| **Companions** | [`design-native-pdf-parser.md`](design-native-pdf-parser.md) (product, K1–K36); [`design-architecture-feature-extraction.md`](design-architecture-feature-extraction.md) (Volume 2 LLD); [`market-analysis-pdf-parsers.md`](market-analysis-pdf-parsers.md) (Parts I–II); [`accuracy-scoreboard.md`](accuracy-scoreboard.md) |
| **Author** | TBD (assign DRI before freeze: **Text DRI** owns §5 / Phase T; **Tables DRI** owns §6 / Phases U–V; decision log for R* acceptance) |
| **Date** | 2026-07-10 |
| **Status** | Draft (rev 1.2 — design review approved; R* ready for implementation planning) |
| **Product / crate** | **`pdfparser`** |
| **Audience** | Senior engineers implementing text path and table subsystem |
| **Scope priority** | **Text extraction first** (K35 retained); table engine designed early, **enabled later** behind quality gates |
| **Language** | Rust (edition 2021+) |
| **Workspace** | `/Users/gautamkumar/Desktop/pdfParser` (greenfield) |
| **Diagrams** | **ASCII / monospaced only** (no Mermaid) |
| **Supersedes / extends** | Extends Volume 2 for **text + tables paths**. Supersedes Volume 2 items with redesign IDs **R1–R20**. Does **not** break K15/K16/K21/K34/K35/governor/library-first without an explicit R-* supersession. |
| **Volume 2 errata (normative)** | **R4 / R20:** Volume 2 C3.7 phase line (“stitch later / out of scope for scoring”) and **EX13** are **superseded** for product 0.2. C3.7 algorithm steps (header_sim, col_align, same cols) remain the **base** algorithm; bands and scoreboard binding are redefined here. **R19:** Volume 2 Issue 19 **component term definitions** retained; **base confidence weights** retained; form/over-seg applied **only** as post-penalties (this doc §6.4). |

> **Reading order for implementers.** Product doc (scope, K-lock) → this redesign (competitive targets + text/table architecture) → Volume 2 (shared L0–L2 algorithms still normative unless R-* overrides) → `benchmark/` harness (acceptance metrics).

---

## 1. Overview

This redesign is driven by a measured competitive mandate, not speculation. On the 33-document accuracy scoreboard, **text extraction is a commodity** among mature engines (token F1 ≈ 0.99–1.0 for pymupdf, pdfplumber, pypdf, pypdfium2) while **table extraction is fragile and incomplete**: pdfplumber leads grid-gold cell F1 at **0.826**, pymupdf at **0.774**, and **every library scores detect F1 = 0 and cell F1 = 0 on stream tables**. Hybrid partial-border fixtures yield detection-without-structure (plumber cell F1 ≈ **0.15**). Real documents expose mass false positives (IRS, NIST), catastrophic over-segmentation (plumber **20** tables on `36_real_two_tables`), complete dual-engine failure on superscripts (`37_real_liabilities_superscript`: **0 tables**), and multi-page ledger splits without stitch (`20_bank_statement_multipage`: detect F1 ≈ **0.67** when expected is one logical table). Speed for text belongs to pypdfium2 (mean ≈ **9–12 ms**).

**Product mandate (user final):** (1) excel across fields where competitors are strong or weak; (2) **text extraction must at least match** best libraries (token F1, CER, multi-column, rotation); (3) **table extraction must exceed** pdfplumber/PyMuPDF via a first-class multi-strategy subsystem; (4) redesign architecture/LLD as needed while preserving locked product decisions (library-first, governor, text-first, tables off by default until quality gates). This document elevates tables from a layout afterthought inside `pdfparser-layout` to a **first-class `pdfparser-tables` engine**, deepens the text path for reading-order and rotation parity, and binds every acceptance bar to `benchmark/scripts/metrics.py` and the accuracy scoreboard.

**Harness truth (critical):** All gates below use `metrics.py` semantics. In particular, when `expected_table_count == 0`, `table_detect_f1` is **1.0 only if `predicted == 0`**, else **0.0** — there is no fractional F1 for “a few FPs.” NIST `table_count_tolerance` affects `within_tolerance` only, not F1, unless the harness is extended. Gates are written in harness-native predicates (§2.2, §2.3).

---

## 2. Competitive gap analysis → design requirements

Sources: `docs/accuracy-scoreboard.md` (33 docs), market analysis Parts I–II, `benchmark/scripts/metrics.py`.

### 2.1 Competitor headline numbers (normative baseline)

| Metric (mean over successful runs) | pymupdf | pdfplumber | pypdf | pypdfium2 | pdfminer.six |
|------------------------------------|--------:|-----------:|------:|----------:|-------------:|
| **overall_score** | **84.48** | 82.98 | 56.52 | 53.19 | 53.10 |
| **text score** | 99.24 | 98.43 | **100.0** | 99.18 | 97.64 |
| **text_token_f1** | 0.992 | 0.992 | **1.000** | 0.992 | 0.984 |
| **text_cer** (lower better) | **0.000** | 0.267 | **0.000** | 0.021 | 0.279 |
| **table score** | **69.70** | 67.49 | 39.39 | 39.39 | 39.39 |
| **table_detect_f1** | **0.710** | 0.696 | 0.394 | 0.394 | 0.394 |
| **table_cell_f1** (where grid gold) | 0.774 | **0.826** | 0.000 | 0.000 | 0.000 |
| **wall_ms mean** | 405 | 321 | 75 | **12.2** | 376 |

**Grid-gold subset (11 docs with full cell grids):**

| Library | detect F1 | shape exact | cell F1 | table score |
|---------|----------:|------------:|--------:|------------:|
| pdfplumber | **0.909** | 0.727 | **0.826** | **83.03** |
| pymupdf | 0.818 | 0.727 | 0.774 | 77.79 |
| others | 0.000 | 0.000 | 0.000 | 0.000 |

**Scenario failures (market gaps):**

| Scenario | Competitor outcome | Scoreboard / Part II |
|----------|--------------------|----------------------|
| Stream tables (`07`) | All libraries **0 tables** | detect F1=0, cell F1=0, overall 25 |
| Hybrid partial border (`08`) | plumber detects, cell F1 **0.148**; mupdf 0 | overall plumber 55.7 |
| Bank multi-page (`20`) | Both engines emit **2** tables; no stitch; mupdf cell text weak | detect F1 ~0.67 vs 1 logical |
| Overflow cells (`21`) | plumber cell F1 **0.950**; mupdf **0.805** (token corruption) | underscore/wrap issues |
| two_tables real (`36`) | plumber **20** tables; mupdf **2** | over-segmentation |
| liabilities superscript (`37`) | **Both 0 tables** | dual SOTA failure |
| IRS f1040 (`38`) | plumber **15** / mupdf **7** tables; expected **0** | mass FP (detect F1=0 if any pred) |
| NIST (`41`) | plumber **99** / mupdf **83** tables | mass FP; gold tol=5 |
| Multi-column (`02`) | pypdf/mupdf/pdfium order **1.0**; plumber/miner **0.67** | reading order |
| Rotated (`11`) | miner **fails** (CER/token soup); others OK | must not copy miner |
| Text speed | pypdfium2 mean **~9–12 ms** | text-only SLO class |
| Census under-extract (`32`) | plumber 8 vs mupdf 17 cells | under-segmentation |
| arXiv science (`40`) | caption/figure “tables” junk | non-form FP class |
| CA WARN scale (`30`) | wide multipage tables ~2s | perf/memory stress |

### 2.2 Quantitative acceptance targets

**Phases:** **0.1** = text-critical path solid, tables experimental/off; **0.2** = tables quality-gated on. Metrics match `benchmark/scripts/metrics.py` unless a gate explicitly names a **harness extension** shipped in the same milestone.

| Area | Competitor bar | Our floor (must ship) | Our stretch |
|------|----------------|------------------------|-------------|
| **Text token F1** (basic+stress) | pypdf 1.0; others ~0.99 | ≥ **0.99** mean; ≥ best competitor on each basic doc with tokens | **1.0** on all synthetic token fixtures |
| **Text CER** (where `reference_text`) | mupdf/pypdf **0.000** | ≤ **0.02** mean; never worse than miner on rotated | ≤ **0.005** mean vs oracle |
| **Multi-column reading order** | pypdf/mupdf score **1.0** on `02` | score **1.0** on `02`; ≥ mupdf on arXiv multi-col text | structure-prefer + geometric dual pass |
| **Rotation** | miner fails; others 1.0 | **token F1 1.0** on `11_rotated_page`; no char-soup | upright layout + raw elements both correct |
| **Lattice / grid-gold cell F1** | plumber **0.826** best | mean ≥ **0.90** **and** per-doc floors (§6.7) | mean ≥ **0.95**; shape_exact ≥ **0.85** |
| **Lattice detect F1** | plumber 0.909 (grid-gold) | ≥ **0.95** on lattice+complex+mixed fixtures | **1.0** on synthetic ruled set |
| **Stream tables** | market **0 / 0** | detect_f1 ≥ **0.90**, cell_f1 ≥ **0.85** on `07` | cell_f1 ≥ **0.95** |
| **Hybrid partial border** | plumber cell_f1 **0.148** | cell_f1 ≥ **0.70** on `08`; prefer shape 5×5 | ≥ **0.90** + shape_exact |
| **Multi-page bank** | detect ~0.67 (2 fragments) | Scoreboard adapter emits **stitched logical count=1** when `stitch_multipage` → **detect_f1 = 1.0** (exp=1, pred=1); also `within_tolerance` true for gold tol=1 | header-deduped stitched grid + cell tokens |
| **Overflow cell fidelity** | plumber 0.95; mupdf 0.805 | cell_f1 ≥ **0.95** on `21`; preserve `TOKEN_*` underscores | **1.0** |
| **FP control (IRS, exp=0, tol=0)** | plumber/mupdf many FPs → **detect_f1 = 0** | **`predicted_table_count == 0`** (⇔ detect_f1 **== 1.0**). Any pred>0 ⇒ F1=0 under `metrics.py` | same (hard zero) |
| **FP control (NIST)** | 83–99 junk tables | **`predicted_table_count ≤ 5`** (gold `table_count_tolerance: 5` → `within_tolerance`); **do not** claim fractional detect_f1 | pred==0 (detect_f1=1.0) |
| **Over-seg (`36_two_tables`)** | plumber 20 vs ideal ~2 | predicted count ∈ **[1, 3]** | exact expected count |
| **Superscript / contd (`37`)** | both engines **0** | **Report-only** until spike+gold (T8); do not block V10 | detect≥1 + cell_f1 ≥ 0.70 when gold grids exist |
| **Side-by-side multi-table** | count OK; mupdf text weak | detect exact; cell_f1 ≥ plumber on grid | cell_f1 **1.0** + underscores |
| **Dense numeric** | both ~1.0 cell | cell_f1 ≥ **0.99** | shape_exact **1.0** |
| **Text-only latency** | pypdfium2 ~9–12 ms; pypdf ~75 ms | **Report** median wall + RSS on CI label hardware; soft budgets only (T6) — **not** ship-blockers until calibrated | approach pdfium class where pure-Rust allows |
| **Objects (images/forms/links)** | mupdf/pypdf 100 | **Out of this redesign’s gates** (product P2) | unified IR |
| **Overall scoreboard** | mupdf **84.48** | **Modeled floor ≥ 88** via §2.4 contribution model (tables+text only); objects-dependent docs non-blocking | ≥ **92** aspirational **post** objects + real-corpus polish — **not** Phase V exit |

#### Expected-0 detect F1 semantics (normative — Issue 1 fix)

```text
From metrics.py table_detection_metrics(n_pred, n_exp, tol):

  if n_exp == 0 and n_pred == 0:  precision=1, recall=1, f1=1.0
  if n_exp == 0 and n_pred  > 0:  TP=0, FP=n_pred → precision=0, f1=0.0  ALWAYS
  within_tolerance = abs(n_pred - n_exp) <= tol   # does NOT change f1

Therefore:
  IRS (expected=0, tol=0): ONLY legal pass is predicted_table_count == 0.
  NIST (expected=0, tol=5): detect_f1 still 0 if any table emitted;
    product gate uses predicted <= 5 (within_tolerance) OR predicted == 0.
  Optional future metric (not required for 0.2): fp_rate = n_pred when n_exp==0;
    graded score = max(0, 1 - n_pred / k). Ship only with metrics.py PR.
```

### 2.3 Metric definitions we must hit (from `metrics.py`)

| Metric | Definition (summary) |
|--------|----------------------|
| `token_f1` | Bag F1 on `must_contain` substrings in extracted text |
| `cer` | Levenshtein(NFKC ref, hyp) / len(ref) |
| `table_detect_f1` | Count-based TP/FP/FN vs `expected_table_count` (tol sets `within_tolerance` only) |
| `table_cell_f1` | Micro F1 of `normalize_cell` texts after greedy gold↔pred table alignment |
| `shape_exact` | Exact row/col counts vs gold grids |
| `content_token_recall` | Legacy: gold cell tokens present in any serialized cell |
| Aggregate table score | 0.35 detect F1 + 0.25 shape exact rate + 0.40 cell F1 (when available) |

**Normalization policy (align with harness):** NFKC; collapse whitespace; cell: lower, join newlines to spaces, soft underscore repair for scoring only (`" _"` → `"_"`)—**extraction must not need underscore repair**. Soft numeric match strips `$,%,comma`.

### 2.4 Overall scoreboard contribution model (Issue 8)

Overall is a **per-doc weighted blend** of text / tables / objects (`overall_accuracy` in metrics). This redesign **does not gate objects** (images/forms/links/outline) — those remain product P2.

**Path to mean overall ≥ 88 (vs mupdf 84.48):**

| Bucket | Docs (examples) | How we gain vs mupdf |
|--------|-----------------|----------------------|
| Stream/hybrid fails | `07` (25), `08` (25 mupdf / 55.7 plumber) | Open stream + fix hybrid → large per-doc jumps (table weight high) |
| Stress lattice already strong | `21–25`, `27` | Cell fidelity edge on overflow/side-by-side; small gains |
| Bank multipage | `20` (~76.7 mupdf) | Stitched pred=1 → detect_f1 1.0 + cell tokens |
| FP sinks — IRS (`38`) | overall ~60 with table detect F1=0 when pred>0 | **Floor = scoreboard recovery:** `pred==0` → detect_f1=1.0 and table score 100 → overall ↑ (same class as non-table engines) |
| FP sinks — NIST (`41`) | overall ~70; gold expected=0, tol=5 | **Product floor T5b (V9):** `pred≤5` → `within_tolerance` only; under `metrics.py` **detect_f1 stays 0** and **table score stays 0** for any pred∈[1,5]. **Does not lift overall.** **Stretch / overall recovery:** `pred==0` only (detect_f1=1.0; ~+0.9 on equal-weight 33-doc mean if one doc 70→100). Not required for ≥88 model. |
| Over-seg | `36` (plumber 42.7; mupdf 100) | Match mupdf-class count, not plumber 20 |
| Text already high | most docs | Hold parity; multi-col/rotate prevent regressions |
| Objects-heavy | `05_special_objects` | **Non-blocking** for this redesign’s overall claim |
| Real scale / under-seg | `30` WARN, `32` census, `40` arXiv | **Report-only / stretch** (§6.8); not required for ≥88 model |

**Arithmetic intuition (illustrative, not a promise of exact blend weights):** recovering `07` and `08` from ~25 to ≥80 table-influenced overall, plus **IRS at pred==0** (table detect recovers) and bank stitched detect_f1=1.0, is sufficient lift on a 33-doc mean to clear **+3.5 points** over mupdf if text holds. **NIST T5b (pred≤5) is a product quality/FP gate only** — it does **not** contribute scoreboard detect_f1 or overall; optional NIST stretch pred==0 adds ~+0.9 mean at most. Stretch overall **≥92** requires objects parity + real-corpus under-seg/caption FP work (+ optional NIST pred==0) → **post Phase V / P2**, not a V10 exit criterion.

**Normative separation (Issue R1):**

| Gate | Predicate | Affects `table_detect_f1` / table score? | Affects overall? |
|------|-----------|------------------------------------------|------------------|
| T5 IRS floor | `pred == 0` | **Yes** (F1=1.0) | **Yes** |
| T5b NIST product floor | `pred ≤ 5` | **No** if pred∈[1,5] (F1=0); only logs `within_tolerance` | **No** |
| NIST stretch | `pred == 0` | **Yes** (F1=1.0) | **Yes** (small mean lift) |

---

## 3. What changes vs Volume 2 / product design

### 3.1 Explicit delta table

| Topic | Product / Volume 2 | This redesign | Status |
|-------|--------------------|---------------|--------|
| Library-first, CLI secondary | K1, K34 | Unchanged | **Keep** |
| Text-first critical path | K35 | Unchanged; tables designed early, shipped after text gates | **Keep** |
| ResourceGovernor / encoded backend | K16, K8 | Unchanged | **Keep** |
| Encryption refuse in 0.1 | K15 | Unchanged | **Keep** |
| Tables default off | K21 | Unchanged until quality gates; progressive presets (R2) | **Keep** (+ **R2**) |
| Tables live in `pdfparser-layout` | Volume 2 crate map | **New crate `pdfparser-tables`**; layout keeps spaces/lines/order/**structure map** | **Revise** (**R1**, **R20** structure ownership) |
| Table orchestration union+NMS | Volume 2 C3 / EX4 / EX21 | Keep union+NMS; add FP classifier, stitcher, post-processors | **Extend** (**R3**, **R16**, **R17**) |
| Multi-page stitch “later / not scored” | EX13, C3.7 out of scoring | **0.2 first-class** + scoreboard adapter emits logical count | **Supersede EX13** (**R4**); Vol2 C3.7 phase line superseded (**R20** errata) |
| Stream multi-line cells “v0 off” | C3.3 E-T6 | Multi-line cell reconstruction ON | **Revise** (**R5**) |
| min_table_confidence 0.55 | Volume 2 | Raise to 0.60 (+ stream 0.55); form/over-seg **post-penalties only** | **Revise** (**R6**) |
| Issue 19 confidence **weights** | Lattice 0.35/0.25/0.20/0.10/0.10; stream 0.30/… | **Weights retained** (R19); no silent replace; form not in base formula | **Keep weights** (**R19**) |
| Reading order params C1.4 | Volume 2 | Keep; acceptance score 1.0 on multi-col | **Extend** (**R7**) |
| apply_page_rotate | Product/D3 | Normalize page rotate before text + tables; **frozen matrices** §5.3 | **Keep + freeze** (**R8**) |
| Cell assignment center/IoU | C3.2 | Geometry assign from ContentVM runs; never lossy rewrite | **Deepen** (**R9**) |
| Path capture vs public Path IR | EX11/EX22 | Unchanged | **Keep** |
| Table IR fields | Volume 2 Table | Stitch metadata, strategy provenance, per-cell confidence | **Extend** (**R10**) |
| 0.2 gates lattice≥0.70 stream≥0.55 | Volume 2 | Raise to scoreboard floors §2.2 / §6.7 | **Supersede** (**R11**) |
| Perfect tables non-goal | Product non-goal | Heuristics + confidence; beat market on measured corpus | **Keep honesty** |
| Async / WASM / OCR | Non-goals | Unchanged | **Keep** |

### 3.2 Relationship diagram

```text
  product design (K1-K36)          market + scoreboard
           │                              │
           │         +--------------------+
           v         v
  +--------------------------------------------+
  |  THIS REDESIGN (R1-R20)                    |
  |  text parity LLD + table engine LLD        |
  +------------------+-------------------------+
                     | extends / supersedes slices of
                     v
  +--------------------------------------------+
  |  Volume 2 (L0-L2 still normative;          |
  |  C1/C3 overridden where R-* says;          |
  |  C3.7 phase/EX13 superseded by R4/R20)     |
  +--------------------------------------------+
```

---

## 4. High-level architecture

### 4.1 Revised layered architecture (first-class Table Engine)

```text
==============================================================================
     REVISED LAYERED ARCHITECTURE — text-first + first-class Table Engine
==============================================================================

  INPUT: PDF bytes / path / reader
                 |
                 v
  +--------------------------------------------------------------------------+
  |  L0  OBJECT MODEL + SECURITY                                             |
  |  ObjectBackend (encoded only) · ObjectStore/ObjStm · owned filters       |
  |  ResourceGovernor (K16/K8) — UNCHANGED                                   |
  |  crate: pdfparser-core                                                   |
  +----------------------------------+---------------------------------------+
                                     |
                                     v
  +--------------------------------------------------------------------------+
  |  L1  DOCUMENT MODEL                                                      |
  |  Page tree · resources · fonts (metrics+ToUnicode) · XObjects            |
  |  crates: pdfparser-core · pdfparser-fonts                                |
  +----------------------------------+---------------------------------------+
                                     |
                                     v
  +--------------------------------------------------------------------------+
  |  L2  CONTENT VM → Page IR                                                |
  |  operators → TextRuns + Images + (optional) Path IR                      |
  |  + internal RuleSegmentBuf (when tables need lattice/hybrid)             |
  |  crate: pdfparser-content                                                |
  +----------------------------------+---------------------------------------+
                                     |
              paint-order IR (source of truth, EX1)
                                     |
           +-------------------------+-----------------------------+
           | CRITICAL PATH (0.1)     | OPTIONAL (after IR)         |
           v                         v                             v
  +--------------------+   +------------------+   +------------------------+
  | L3a TEXT / LAYOUT  |   | L3b STRUCTURE    |   | L3c TABLE ENGINE       |
  | spaces · lines ·   |   | StructurePageMap |   | **pdfparser-tables**   |
  | reading order ·    |   | built in layout  |   | S* detectors + P* post |
  | Page::text CER     |   | (RoleMap+MCID)   |   | D1 stitch (doc-level)  |
  | crate: layout      |   | crate: layout    |   | (parallelizable)       |
  +---------+----------+   +--------+---------+   +-----------+------------+
            |                       |                         |
            |                       | consume-only ----------+
            +-----------------------+-------------------------+
                                    v
  +--------------------------------------------------------------------------+
  |  L4  PUBLIC SURFACE                                                      |
  |  pdfparser façade · ExtractedDocument · JSON export · experimental tables|
  |  CLI secondary                                                           |
  +--------------------------------------------------------------------------+
```

**R20 — Structure ownership (normative):**

| Concern | Crate | Notes |
|---------|-------|-------|
| StructTreeRoot walk, RoleMap, MCID→role | **`pdfparser-layout`** (module `structure/`) | May call core only for catalog dict access via façade-provided snapshot; **no** `layout → tables` edge |
| `StructurePageMap` type | `pdfparser-ir` or layout-public type re-exported | Plain data for consumers |
| S1 StructureTagged detector | **`pdfparser-tables`** | **Consumes** `&StructurePageMap` only; never builds tree |
| Reading-order structure prefer (C1.4) | **`pdfparser-layout`** | Same map |

### 4.2 Why Table Engine is first-class (not a layout afterthought)

Volume 2 placed detectors under `pdfparser-layout` next to line clustering. Market data shows tables need:

1. Independent strategy lifecycle and feature flags
2. Geometry index shared with lattice/cell assignment
3. Document-level stitch (cross-page) — not page-local layout
4. FP classifier tuned against IRS/NIST fixtures
5. Separate compile/test/bench surface and SLO budgets

**R1:** introduce `pdfparser-tables` depending on `pdfparser-ir` + layout helpers (lines/reading-order/`StructurePageMap`) + content IR views; façade orchestrates.

### 4.3 Revised crate / module map

```text
pdfParser/
├── crates/
│   ├── pdfparser-ir/           # Rect, Element, Table IR extensions, WarningCode
│   ├── pdfparser-core/         # backend, governor, filters, page tree
│   ├── pdfparser-fonts/        # metrics + unicode (leaf)
│   ├── pdfparser-content/      # content VM → Page IR + RuleSegmentBuf
│   ├── pdfparser-layout/       # spaces, lines, blocks, reading order,
│   │                           # **structure/** StructurePageMap builder (R20)
│   ├── pdfparser-tables/       # **NEW** multi-strategy table engine (R1)
│   │     geometry/             # R-tree / sweep index
│   │     strategies/           # S1–S4 page detectors
│   │     post/                 # P1–P5 page post-processors
│   │     fusion/               # candidate union, NMS, two-phase conf
│   │     cells/                # high-fidelity cell text assignment
│   │     confidence/           # Vol2 base weights + post penalties (R19)
│   │     stitch/               # D1 TableFragment → stitched Table
│   ├── pdfparser-export/
│   ├── pdfparser/              # façade only public 0.1
│   └── pdfparser-cli/
├── benchmark/                  # accuracy scoreboard (normative gates)
└── docs/
```

**Dependency DAG (arrows = depends on):**

```text
                    pdfparser-cli
                          |
                          v
                      pdfparser
               /    /    |    \     \
              v    v     v     v     v
          content layout tables export  ir
             |   /  \     |      |
             |  /    \    |      |
             v v      v   v      |
           fonts     core        |
             |         |         |
             +---->   ir   <-----+

  Rules:
    layout  → ir  (+ core only via façade-fed data for structure walk)
    tables  → ir, layout (lines + StructurePageMap views), NOT core
    tables NEVER imported by layout (one-way) — R20
    content → ir, fonts, core
    fonts   → ir only
```

### 4.4 Data flow — unified pipeline IDs (Issue 2 fix)

**Normative ID matrix (single source of truth):**

| ID | Kind | Name | Runs when |
|----|------|------|-----------|
| **S1** | Page detector | StructureTagged | `modes.structure` + map present |
| **S2** | Page detector | LatticeRuled | `modes.lattice` + rules |
| **S3** | Page detector | StreamWhitespace | `modes.stream` |
| **S4** | Page detector | HybridPartialBorder | `modes.hybrid` |
| **P1** | Page post | FormVsTableDiscriminator | tables on; **veto/score only**, does not emit |
| **P2** | Page post | DenseNumericSpecialization | after candidates; boost/refine |
| **P3** | Page post | OverflowMultilineCell | after grid + cell assign |
| **P4** | Page post | SideBySideAntiOverSeg | after NMS; split/merge |
| **P5** | Page post | SuperscriptContinuedRecovery | experimental; report-only gate |
| **D1** | Document pass | MultiPageStitch | after all pages; `stitch_multipage` |

```text
==============================================================================
                         TABLE ENGINE DATA FLOW
==============================================================================

  ContentVM
     |
     +-> TextRuns[]  (unicode + bbox + font + mcid + confidences)
     +-> RuleSegmentBuf  (internal; EX11)
     +-> StructurePageMap?  (from pdfparser-layout, R20)

     |
     v
  GeometryIndex  (R-tree of runs + segments; build O(n log n))
     |
     v
  +------------------------------------------------------------------------+
  |  PAGE DETECTORS S1–S4 (candidates; may run in parallel after IR frozen)|
  |    S1 StructureTagged · S2 LatticeRuled · S3 Stream · S4 Hybrid        |
  +--------------------------------+---------------------------------------+
                                   | Vec<TableCandidate>
                                   v
  P1 FormVsTableDiscriminator  (attach form_likeness; hard veto)
                                   |
                                   v
  Fusion + NMS  (IoU≥0.5; tie-break Structure>Hybrid>Lattice>Stream)
    uses pre_nms_confidence (cheap fill estimate) — §6.4.4 two-phase
                                   |
                                   v
  CellTextAssigner  (geometry of glyphs/runs → cells; R9)
                                   |
                                   v
  P2 DenseNumeric · P3 Overflow · P5 Superscript (optional)
                                   |
                                   v
  Final confidence_v1 (Vol2 base - post penalties) + min_confidence filter
                                   |
                                   v
  P4 SideBySideAntiOverSeg
                                   |
                                   v
  Page-local Table IR fragments
                                   |
                                   v  (document level)
  D1 MultiPageStitch → logical_table_id + flags; adapter may emit stitched grids
```

### 4.5 Text path stays on critical path; tables parallelizable after IR

```text
==============================================================================
              ORCHESTRATION — text critical path vs table off-path
==============================================================================

  extract_one_page(opts):
      ir = interpret(page)                    // always (L2)
      structure = layout::build_structure_map(...)  // cheap if tree absent
      if needs_text_or_layout(opts):
          layout = layout::analyze(ir, structure)
          text  = assemble_text(layout)
      if opts.table.detect_tables:            // default false (K21)
          tables = tables::detect_page(ir, layout, structure, opts.table)
      return ExtractedPage { text, layout?, tables?, elements: ir }

  extract_document(...):
      pages = map extract_one_page
      if opts.table.detect_tables && opts.table.stitch_multipage:
          tables::stitch_document(&mut pages.tables, opts)  // D1

  Invariant (R12): Page::text never requires pdfparser-tables work.
```

---

## 5. Low-level design — Text extraction (match or beat)

### 5.1 Goals for text path

| Goal | Acceptance |
|------|------------|
| Token F1 | ≥ 0.99 mean; 1.0 on synthetic must_contain fixtures |
| CER | Competitive with mupdf/pypdf; ≤ 0.02 mean |
| Multi-column | reading_order_score **1.0** on `02_multi_column` |
| Rotation | **Never** fail like pdfminer on `11_rotated_page` |
| Spaces/words | Correct assembly; underscore preservation |
| Latency | Text-only: **report** pypdf-class; page-lazy; no whole-PDF char cache |
| Geometry | Metrics-correct widths + ToUnicode (Volume 2 B7/C1 — keep) |

### 5.2 Keep from Volume 2 (normative, not re-litigated)

- Font widths independent of ToUnicode; advance formula Tc/Tw/Th/TJ
- ActualText overrides unicode, never geometry (EX2)
- Space insertion as layout layer (EX3, C1.3)
- Unknown glyph U+FFFD (K26); paint-order IR source of truth (EX1)
- f32 coords, 3-decimal goldens (K19)

### 5.3 Page rotation normalize (R8) — frozen matrices

**Root cause of miner failure:** treating rotated glyphs as upright without transforming coordinates / baseline → vertical character soup.

**Frozen upright transform** about MediaBox origin (x_min, y_min), width W, height H (user space). Input point (x, y) in unrotated page user space; output upright export space for text/layout/tables when `apply_page_rotate=true`:

```text
R = ((page.Rotate % 360) + 360) % 360

R = 0:
  x' = x
  y' = y
  upright MediaBox size = (W, H)

R = 90:   # page content was rotated 90° CW in viewer convention for /Rotate 90
  x' = y - y_min
  y' = W - (x - x_min)
  upright size = (H, W)

R = 180:
  x' = W - (x - x_min)
  y' = H - (y - y_min)
  upright size = (W, H)

R = 270:
  x' = H - (y - y_min)
  y' = x - x_min
  upright size = (H, W)
```

**Property tests (normative):**

1. Applying the map four times for R=90 (or composing 90×4) returns identity within 1e-3 on MediaBox corners.
2. `11_rotated_page`: `Page::text` contains contiguous `ROTATED_PAGE_TOKEN` (token F1 = 1.0).
3. Raw `elements()` with `apply_page_rotate=false` remains unrotated user space (product default for paint IR).

```rust
pub fn rotate_point(p: Point, rotate: i32, media: Rect) -> Point {
    let r = ((rotate % 360) + 360) % 360;
    let x = p.x - media.x0;
    let y = p.y - media.y0;
    let w = media.width();
    let h = media.height();
    match r {
        0 => Point { x: media.x0 + x, y: media.y0 + y },
        90 => Point { x: media.x0 + y, y: media.y0 + (w - x) },
        180 => Point { x: media.x0 + (w - x), y: media.y0 + (h - y) },
        270 => Point { x: media.x0 + (h - y), y: media.y0 + x },
        _ => Point { x: media.x0 + x, y: media.y0 + y }, // non-multiples: treat as 0 + warn
    }
}

pub fn to_upright_rect(r: Rect, page_rotate: i32, media: Rect) -> Rect {
    Rect::from_points(&r.corners().map(|p| rotate_point(p, page_rotate, media)))
}
```

**Table engine:** when layout APIs use upright geometry, S2–S4 consume **upright** runs and rule segments (transform rules with same R8 map). **In-page 90° content** (CTM rotation, not page /Rotate): best-effort swap H/V clustering when median baseline angle ≈ 90/270 (Volume 2 C3.8); **report-only** for 0.2, not a floor gate (§6.8).

### 5.4 Reading order (match pypdf/mupdf score 1.0)

Volume 2 C1.4 base. Parameters (normative defaults):

| Parameter | Symbol | Default |
|-----------|--------|---------|
| Line band y tolerance | τ_y | `0.25 * median_font_size` |
| Block vertical gap factor | β | **1.5** |
| Column x-overlap IoU | ρ | **0.15** |
| Min column gap | g_col | **1.5 * median_space_width** |
| Min column width | w_min | **3 * median_font_size** |
| Min lines per column | n_col | **2** |
| Spanning header coverage | σ | **0.80** |
| Structure prefer coverage | cov | **0.80** (EX16) |

```text
1. Exclude Artifact-marked runs from Page::text by default.
2. Apply upright geometry if apply_page_rotate (R8).
3. Cluster lines; sort by x0 within line.
4. Sort lines y_top descending (PDF y-up after upright).
5. Blocks when vertical gap > β * median_line_height.
6. Multi-column: gutters ≥ g_col; bands ≥ w_min and ≥ n_col lines.
7. Spanning lines (≥ σ union width above columns) emit first.
8. Order: spanning → columns L→R → lines top→bottom.
9. Structure prefer if MCID coverage ≥ cov; else geometric + warning.
10. Fallback: paint order + ReadingOrderFallbackPaint.
```

**Acceptance:** `02_multi_column` markers in order (score 1.0); arXiv multi-col report-only for body interleave.

### 5.5 Space insertion / word reconstruction

Keep Volume 2 C1.3; α=0.25; preserve U+005F; no space insert on zero/negative gaps.

### 5.6 CER targets and regression harness hook

```text
hyp = Page::text(sort_reading_order=true, insert_spaces=true)
cer = levenshtein(NFKC_ws_collapse(ref), NFKC_ws_collapse(hyp)) / len(ref)

CI:
  cargo test -p pdfparser --test text_accuracy
  python benchmark/scripts/run_accuracy_benchmark.py --lib pdfparser
  gate: mean text_token_f1 ≥ 0.99 basic; 11 token F1==1.0; 02 order==1.0
```

### 5.7 Optional dual-path validation mode (CI only)

**R13:** feature `oracle-ci` only; never runtime dep (K5).

### 5.8 Performance: page-lazy, anti-pdfplumber memory

| Anti-pattern | Our rule |
|--------------|----------|
| Global char cache (~287 MB / 80p plumber) | Page-local IR; optional LRU |
| Eager all-page interpret | Lazy pages (K9) |
| Tables always on | `detect_tables=false` default |

```text
Text-only budgets (illustrative; T6 REPORT-ONLY until calibrated on CI label HW):
  simple page median wall:  soft comment if > 15 ms class
  80-page large multipage:  soft comment if >> pypdf class
  RSS delta 80p:            soft fail comment if > 50 MB; hard concern if hundreds MB
  NOT ship-blockers for 0.1 text GA (K33 text quality first)
```

### 5.9 Text pipeline ASCII

```text
  Encoded content stream
        | decode under governor
        v
  ContentVM (Tj/TJ + fonts)
        |
        v
  TextRun[] paint-order (user space)
        |
        +-- apply_page_rotate? --> upright runs (R8 matrices)
        |
        v
  Space insertion → Lines → Blocks → Columns → Reading order
        |
        v
  Page::text string  -->  CER / token F1 harness
```

---

## 6. Low-level design — Table extraction (EXCEED competitors)

**Core of the redesign.** Beat pdfplumber grid-gold cell F1 (0.826); open stream gap; fix hybrid; control FPs with harness-true gates; stitch multi-page; preserve cell text.

### 6.0 Pipeline ID recap

Detectors **S1–S4**, posts **P1–P5**, document **D1** — see §4.4. Do not renumber elsewhere.

### 6.1 Architecture of table subsystem

```text
RuleSegmentBuf + TextRuns + StructurePageMap + LayoutLines
        |
        v
  GeometryIndex
        |
        v
  S1–S4 strategy runners (parallel candidates)
        |
        v
  P1 form disc → Candidate fusion + NMS (pre_nms conf)
        |
        v
  Cell text assignment (high fidelity) → P2/P3/P5
        |
        v
  confidence_v1 + min_confidence → P4 anti over-seg
        |
        v
  D1 Multi-page stitcher (document)
        |
        v
  Table IR (+ adapter stitched grids for scoreboard)
```

#### 6.1.1 Core types

```rust
pub struct TablePageInput<'a> {
    pub page_index: u32,
    pub media_box: Rect,
    pub crop_box: Rect,
    pub rotate: i32,
    pub runs: &'a [TextRun],
    pub lines: &'a [LayoutLine],
    pub rules: &'a RuleSegmentBuf,
    pub structure: Option<&'a StructurePageMap>, // from layout (R20)
    pub widgets: &'a [WidgetRect],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PipelineId {
    S1Structure,
    S2Lattice,
    S3Stream,
    S4Hybrid,
    P1FormDisc,
    P2DenseNumeric,
    P3OverflowCell,
    P4SideBySide,
    P5Superscript,
    D1Stitch,
}

#[derive(Clone, Debug)]
pub struct TableCandidate {
    pub bbox: Rect,
    pub rows: u32,
    pub cols: u32,
    pub cells: Vec<CellGeom>,
    pub method: TableMethod,
    pub pipeline_ids: Vec<PipelineId>,
    pub pre_nms_confidence: f32,  // geometry + cheap fill (§6.4.4)
    pub features: CandidateFeatures,
    pub header_rows: u32,
}

#[derive(Clone, Debug, Default)]
pub struct CandidateFeatures {
    pub grid_regularity: f32,
    pub rule_support: f32,
    pub fill_rate: f32,              // cheap estimate pre-NMS; exact post-assign
    pub alignment_score: f32,
    pub column_separation_score: f32,
    pub row_consistency: f32,
    pub form_likeness: f32,          // §6.4.0
    pub prose_likeness: f32,         // §6.4.0
    pub numeric_density: f32,        // §6.4.0
    pub multi_line_cell_ratio: f32,
    pub header_repeat_score: f32,
    pub rule_to_text_ratio: f32,     // rules.count / max(text_runs,1) normalized
    pub widget_overlap: f32,         // area fraction overlapping AcroForm widgets
    pub empty_small_cell_ratio: f32, // checkbox-like
    pub punctuation_density: f32,
    pub mean_cell_chars: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TableMethod {
    Structure,
    Lattice,
    Stream,
    Hybrid,
    DenseNumeric,          // method tag after P2 refine
    SuperscriptRecovered,  // after P5
    FormLayout,            // rarely emitted
}

/// Public IR (R10)
#[derive(Clone, Debug)]
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
    pub continued_to_next_page: bool,
    pub logical_table_id: Option<u64>,
    pub fragment_index: u32,
    pub strategy_provenance: Vec<PipelineId>,
    pub notes: Vec<String>,
    pub children: Vec<Table>,
    pub kv_hints: Option<KvHints>,
}

#[derive(Clone, Debug)]
pub struct TableCell {
    pub row: u32,
    pub col: u32,
    pub rowspan: u32,
    pub colspan: u32,
    pub bbox: Rect,
    pub text: String,
    pub is_header: bool,
    pub source_mcids: Vec<u32>,
    pub confidence: f32,
    pub source_run_ids: Vec<u32>,
}
```

#### 6.1.2 Engine entry

```rust
pub fn detect_tables_page(
    input: &TablePageInput<'_>,
    opts: &TableOptions,
    governor: &ResourceGovernor,
) -> Result<Vec<Table>, Error> {
    let _t = governor.time_budget_table_page()?;
    let index = GeometryIndex::build(input.runs, input.rules, opts)?;

    let mut cands = Vec::new();
    for s in enabled_detectors(opts) { // S1–S4 only
        cands.extend(s.run(input, &index, opts)?);
    }

    // P1: features form_likeness / veto before NMS
    p1_form_discriminator(&mut cands, input, opts);
    cands.retain(|c| !c.hard_vetoed);

    // Cheap fill + base conf for NMS (phase A)
    for c in &mut cands {
        estimate_fill_and_features(c, input, &index);
        c.pre_nms_confidence = base_confidence_vol2(c) ; // R19 weights, no form in base
    }

    let mut fused = fuse_and_nms(cands, opts); // uses pre_nms_confidence

    for t in &mut fused {
        assign_cell_text(t, input, &index, opts)?; // R9 exact
        recompute_fill_and_features(t);           // phase B exact fill
        p2_dense_numeric(t, opts);
        p3_overflow_multiline(t, input, opts)?;
        p5_superscript_recovery(t, input, opts);  // experimental
        t.confidence = confidence_v1(t, opts);    // base - post penalties only
    }
    fused.retain(|t| t.confidence >= min_conf_for(t, opts));
    fused = p4_anti_oversegmentation(fused, input, opts);
    fused.truncate(opts.max_tables_per_page as usize);
    Ok(fused.into_iter().map(into_public_table).collect())
}

/// D1 — after all pages:
pub fn stitch_document(page_tables: &mut [Vec<Table>], opts: &TableOptions) {
    d1_stitch_multipage(page_tables, opts);
}
```

### 6.2 Page detectors (S1–S4)

#### 6.2.1 S1 — StructureTagged

**When:** `modes.structure` and `StructurePageMap` present (built by layout, R20).

```text
1. Walk map roles after RoleMap; find Table nodes.
2. TR under THead/TBody/TFoot; TH/TD with RowSpan/ColSpan; occupancy grid.
3. MCID → text from page IR; bbox = union cells or structure BBox.
4. fill_rate from non-empty; form_likeness computed later by P1.
5. pre_nms base via structure formula: 0.9 - 0.1 * major_mcid_gaps (floor 0.5).
6. Emit method=Structure, provenance S1.
```

Never short-circuit other detectors (EX21).

#### 6.2.2 S2 — LatticeRuled

**Bar:** plumber grid-gold cell F1 **0.826** → our mean ≥ **0.90**.

```text
1. Axis-aligned segments (angle within 2° of 0/90°); merge colinear tol=1.0;
   double-line merge if parallel dist < max(1.0, 0.75*median_stroke).
2. Snap H y / V x (tol = max(1.0, 0.5*median_stroke)).
3. Regions with ≥2 H and ≥2 V spanning lines → intersection cell rects.
4. Require rows≥2, cols≥2.
5. Features: grid_regularity, rule_support; cheap fill via run centers in cells.
6. Reject chart frames: rule_support < 0.15 AND fill_rate > 0.85 (C3.11 R5).
7. pre_nms_confidence = lattice Vol2 weights (R19).
8. Provenance S2; method=Lattice.
```

**Merged headers (`22`):** empty cells under text spanning multiple columns → colspan; assign header once.

**Under-segmentation note (`32` census):** optional future post (not 0.2 floor) — split cells when multiple text islands with large internal gap; §6.8.

#### 6.2.3 S3 — StreamWhitespace (primary differentiator)

**Market:** detect F1=0, cell F1=0 on `07`.

| Parameter | Default |
|-----------|---------|
| γ gap factor | **2.0** |
| Histogram bin | `max(2.0, 0.3 * median_font_size)` |
| Peak prominence | ≥ **30%** of lines |
| Min lines | **3** (≥4 for 2-col) |
| Min internal boundaries | **2** (or 1 for 2-col with ≥4 lines) |
| Multi-line cells | **ON (R5)** |
| Row y-cluster tol | `0.6 * median_line_height` |

```text
1. Reading-order LayoutLines (upright); skip artifacts.
2. Word gaps > γ * median_space_width → boundary midpoints; histogram peaks.
3. Validate peak set; column edges = [left]+peaks+[right].
4. ROW MODEL (R5): cluster lines into row groups when gap < row y-cluster tol
   AND column-band pattern holds; multi-line cell text join with '\n'.
5. Features: column_separation, row_consistency, fill, alignment, prose_likeness.
6. Prose reject: column_separation < 0.35; or punct_density > 0.12 && alignment < 0.55.
7. pre_nms_confidence = stream Vol2 weights (R19).
8. Provenance S3; method=Stream.
```

**Acceptance:** `07` detect_f1 ≥ 0.9, cell_f1 ≥ 0.85.

#### 6.2.4 S4 — HybridPartialBorder (5×5 recovery)

**Market:** plumber cell F1 **0.148** (1×2 collapse); mupdf 0.

**Goal:** recover near **5×5** on `08_table_partial_border` with cell_f1 ≥ **0.70**.

##### Frame finding

```text
find_outer_frames(rules):
  1. Build graph of H/V segments; snap endpoints within 2.0 units.
  2. A closed frame is a rectangle where each of 4 sides has ≥70% edge coverage
     by colinear segments (union length / side length).
  3. Rank frames by area descending; keep non-nested maximal frames
     (if frame A contains B with area_B < 0.9*area_A and margins < 3, keep A only
      unless B has its own internal grid with ≥2x2 cells).
  4. If no closed frame: use largest axis-aligned bbox of union of all rules
     with ≥3 sides supported (partial frame); mark incomplete=true.
```

##### Incomplete lattice classification

```text
incomplete if:
  rule_support ∈ (0.05, 0.55)
  OR (outer frame present AND internal_H + internal_V < (rows_est-1)+(cols_est-1)
      with rows_est from text line bands)
```

##### Deterministic grid builder inside frame F (normative for `08`)

```text
hybrid_grid(F, rules, lines, runs):
  # Columns
  1. Stream peak detection (S3 steps) restricted to x ∈ [F.x0+inset, F.x1-inset],
     inset = 1.5 user units; only lines whose y-center ∈ [F.y0, F.y1].
  2. Also collect V rules inside F; snap peaks to nearest V rule if |dx|≤3.
  3. col_edges = [F.x0] + sorted unique peaks/rules + [F.x1].
  4. Require n_cols = len(col_edges)-1 ≥ 2; else abort hybrid for this frame.

  # Rows — recover when internal H rules missing
  5. Let H_int = horizontal rules fully inside F (not the outer top/bottom).
  6. If len(H_int) >= 1:
       row_edges = [F.y1 (top)] + sorted H_int.y + [F.y0 (bottom)]  # y-up
     Else:
       # Text baseline recovery (critical for partial-border):
       a. Take lines with y-center in F; cluster baselines with tol =
          0.5 * median_line_height.
       b. Each cluster → a row band; row_edges from midpoints between
          successive cluster y-centers, plus F top/bottom.
       c. If only 1–2 clusters but fill suggests more rows (many y-distinct
          word baselines), re-cluster with tighter tol = 0.35 * median_fs.
  7. Optional equal-split fallback: if text clusters give rows < 3 AND
     F.height / median_line_height >= 4, estimate n_rows =
     round(F.height / median_line_height) clamped [3, 12], equal H splits.
     Mark notes "equal_row_estimate".

  # Reject plumber-like 1×2 collapse
  8. If n_rows < 3 OR n_cols < 3:
       if text-derived clusters can produce ≥3 rows and stream peaks ≥2:
         rebuild; else drop hybrid candidate (do not emit mega-cells).
  9. Build cell rects from row_edges × col_edges; method=Hybrid; provenance S4.

Worked example fixture 08 (expected 5×5):
  - Outer box + header H line only → incomplete lattice.
  - Stream/text inside F finds 4 internal column peaks → 5 cols.
  - Five baseline clusters (header + 4 body) → 5 rows.
  - Reject any candidate that is 1×2 with body blob.
```

```text
confidence: pre_nms = 0.5 * conf_L + 0.5 * conf_S + 0.05 if shape agreement
Reject if rows<2 or cols<2 (after anti-collapse, prefer ≥3×3 for partial frame).
```

**Acceptance:** `08` cell_f1 ≥ 0.70; stretch shape_exact 5×5 and cell_f1 ≥ 0.90.

### 6.3 Page post-processors (P1–P5)

#### 6.3.1 P1 — FormVsTableDiscriminator (IRS/NIST)

**Market:** IRS plumber 15 / mupdf 7; NIST 99/83. Expected data tables 0.

**Does not emit tables.** Attaches features and may `hard_vetoed=true`.

**Order:** after S1–S4 candidates, **before** NMS; final conf penalties after assign.

**Acceptance (harness-true):**

| Doc | Gold | Floor gate |
|-----|------|------------|
| `38_real_irs_f1040` | expected=0, tol=0 | **`predicted_table_count == 0`** (detect_f1 == 1.0) |
| `41_real_nist_*` | expected=0, tol=5 | **`predicted_table_count ≤ 5`** (`within_tolerance`); stretch pred==0 |

Formulas for features: §6.4.0.

```text
hard veto if form_likeness ≥ 0.75 AND numeric_density < 0.15
  UNLESS method==Structure AND structure_base_conf ≥ 0.85
```

#### 6.3.2 P2 — DenseNumericSpecialization

```text
Trigger: numeric_density ≥ 0.5 OR (cols≥4 AND mean_cell_chars ≤ 12).
Prefer right-align last k columns; tighter lattice snap; +0.05 conf if
  grid_regularity high and fill_rate ≥ 0.7 (applied as bonus after base, cap 1.0).
```

**Acceptance:** `25` cell_f1 ≥ 0.99; `09` ≥ 0.98.

#### 6.3.3 P3 — OverflowMultilineCell (beat MuPDF)

**Market:** mupdf cell_f1 0.805 vs plumber 0.950 on overflow.

```text
1. Grid geometry first; no text mutation.
2. Assign each TextRun to max IoU cell (IoU≥0.5 or center); never re-OCR.
3. Sort runs in cell reading order; join space / '\n' by baseline bands.
4. Preserve '_', superscripts, ActualText unicode (EX2 geometry still paints).
5. No NFKC in extraction.
```

**Acceptance:** `21` cell_f1 ≥ 0.95; bank overflow tokens; `TOKEN_SIDE_L` exact.

#### 6.3.4 P4 — SideBySideAntiOverSeg

```text
1. After conf filter: if one bbox covers two text islands with vertical gutter,
   split along gutter (local S2/S3).
2. Merge vertically abutting small tables with same col x-edges if gap small
   unless title between.
3. If page_candidate_count > soft_max (8) and form/prose-like page:
   over_segmentation_penalty already in conf; drop lowest conf first.
```

**Acceptance:** `36` predicted ∈ [1,3]; `23` exact 2.

#### 6.3.5 P5 — SuperscriptContinuedRecovery (report-only)

**Market:** both engines 0 tables on `37`.

```text
Phase A (spike-driven, report-only until gold+spike):
1. Continuation markers: /cont'?d\.?/i, /continued/i.
2. Relax lattice min rows if strong V rules + numeric columns; superscripts
   (font_size ≤ 0.75*body AND raised baseline) attach to previous run, not new row.
3. Stream fallback if lattice fails.

Do NOT make V10 depend on P5. V8 is report-only (Issue 16).
```

### 6.4 Confidence model v1 + feature formulas

#### 6.4.0 Normative feature definitions (Issue 3 / Issue-19 grade)

All scores clamped to `[0.0, 1.0]`. ε = **1.0** user unit unless noted.

**Reuse Volume 2 Issue 19 as-is for:**
`grid_regularity`, `rule_support`, `fill_rate`, `alignment_score`, `column_separation_score`, `row_consistency`.

**`numeric_density` (normative):**

```text
Let cells_ne = non-empty cells after normalize_cell (or cheap estimate: runs in cell).
numeric_token(s) = true if normalize_numeric_soft(s) matches
  ^-?\$?[\d]+([.,]\d+)*%?$  OR accounting ( ... ) negatives after strip.
numeric_density = |{c in cells_ne : numeric_token(c)}| / max(|cells_ne|, 1)
```

**`punctuation_density`:**

```text
punct_chars = count of characters in .?!,;: across all cell text
text_chars  = count of non-space chars
punctuation_density = punct_chars / max(text_chars, 1)
```

**`prose_likeness` (normative):**

```text
mean_cell_chars = mean length of non-empty cell strings
long_cell = 1 if mean_cell_chars ≥ 40 else mean_cell_chars / 40

prose_likeness = clamp01(
  0.35 * punctuation_density / 0.12 +          // 0.12 → ~1.0 contribution cap via clamp
  0.30 * (1 - alignment_score) +
  0.20 * long_cell +
  0.15 * (1 - column_separation_score)         // stream; 0 if lattice-only N/A → use 0.5 neutral
)
# For lattice-only candidates without column_separation, substitute 0.5 for that term's input.
```

**`form_likeness` (normative):**

```text
s_rule_text = clamp01( (n_rule_segments / max(n_text_runs, 1)) / 3.0 )
  # high rules per text run → form-like
s_empty_small = empty_small_cell_ratio
  # fraction of cells with area < 0.002 * page_area AND empty text
s_widget = widget_overlap
  # area(candidate ∩ union(widget_rects)) / area(candidate)
s_low_numeric = 1 - numeric_density
s_low_columnar = 1 - clamp01(alignment_score)  # weak columns
s_underline = clamp01( fraction of H rules with no text within 4 units above )
s_data = clamp01( 0.5 * numeric_density + 0.5 * fill_rate * alignment_score )

form_likeness = clamp01(
  0.20 * s_rule_text +
  0.15 * s_empty_small +
  0.20 * s_widget +
  0.15 * s_low_numeric +
  0.10 * s_low_columnar +
  0.10 * s_underline +
  0.10 * (1 - s_data)           # invert data-likeness
)

# Unit tests:
#   clean 3×3 lattice numeric: form_likeness ≤ 0.35
#   IRS snippet ruled sparse: form_likeness ≥ 0.75
```

**Feature computation order:**

```text
1. Geometry features (grid, rules, alignment, separation, row_consistency)
2. Cheap fill (run centers) for pre-NMS
3. numeric_density, punctuation_density, prose_likeness, form_likeness
4. P1 hard veto
5. NMS on pre_nms_confidence = base_confidence_vol2 (no form in base)
6. Exact cell assign → recompute fill_rate, numeric_density, form_likeness
7. confidence_v1 = base - form_layout_penalty - over_seg_penalty - low_cell_text_penalty
8. min_confidence filter
```

#### 6.4.1 Base formulas — Volume 2 Issue 19 weights retained (R19)

**R19:** Issue 19 **component term definitions and base weights** are **normative**. Redesign does **not** replace base weights with form terms inside the weighted sum. Form/over-seg are **post-penalties only** (avoids double-count).

**Lattice (Volume 2):**

```text
base_lattice =
  0.35 * grid_regularity +
  0.25 * rule_support +
  0.20 * fill_rate +
  0.10 * alignment_score +
  0.10 * min(1, cells/6)
```

**Stream (Volume 2):**

```text
base_stream =
  0.30 * column_separation_score +
  0.25 * row_consistency +
  0.20 * fill_rate +
  0.15 * alignment_score +
  0.10 * min(1, cells/6)
```

**Structure:** `0.9 - 0.1 * mcid_gaps` (floor 0.5).

**Hybrid:** `0.5 * base_lattice + 0.5 * base_stream + 0.05 * agreement` (cap 1.0) on merged grid.

**Golden (retained):** synthetic 3×3 full-border lattice expects base_lattice ≥ **0.75 ± 0.05** with form_likeness low so post-penalty ≈ 0; empty prose stream base ≤ **0.40** or rejected by prose predicates.

#### 6.4.2 Post-penalties only (no form in base)

```text
form_layout_penalty       = 0.50 * form_likeness
over_segmentation_penalty =
  if page_emit_count_before_filter > soft_max_tables_per_page (8):
    0.05 * min(6, count - 8)
  else 0
  + 0.10 if cluster of tiny tables (area < 0.02 * page) sharing band
low_cell_text_penalty     = 0.20 if fill_rate < 0.15 after exact assign else 0

confidence_v1 = clamp01(
  base_confidence_vol2
  - form_layout_penalty
  - over_segmentation_penalty
  - low_cell_text_penalty
)
# Optional P2 dense bonus: min(0.05, ...) added after, still cap 1.0
```

**Worked examples:**

```text
Clean 3×3 lattice: base≈0.80, form_likeness≈0.2 → penalty 0.10 → conf≈0.70 ≥ 0.60 pass
IRS form grid: base≈0.55 (rules, weak fill), form≈0.85 → hard veto OR conf≈0.55-0.425≈0.13 drop
Prose stream: rejected by prose_likeness / separation before conf, or base≤0.40
```

#### 6.4.3 Thresholds (R6)

```text
min_confidence: 0.60            # lattice/hybrid/dense
min_confidence_stream: 0.55
min_confidence_structure: 0.50
hard_veto_form_likeness: 0.75
max_tables_per_page: 32
soft_max_tables_per_page: 8
```

#### 6.4.4 Two-phase confidence / NMS (Issue 14)

```text
Phase A (pre-NMS):
  fill_rate_cheap = fraction of cells with ≥1 run center inside (or IoU≥0.25)
  base = base_confidence_vol2 using fill_rate_cheap
  NMS compares pre_nms_confidence = base  (geometry-aware, not zero fill)

Phase B (post-assign):
  fill_rate exact; recompute numeric_density, form_likeness
  confidence_v1 with penalties; filter min_confidence
```

NMS: IoU(bbox) ≥ 0.5 → keep higher pre_nms_confidence; tie-break Structure > Hybrid > Lattice > Stream.

### 6.5 Multi-page table model (D1) — measurable scoreboard (Issue 5)

**Supersedes EX13 / Vol2 C3.7 phase** (R4, R20). Algorithm base from C3.7; **band parameter:** top/bottom **30%** of page height (**supersedes** C3.7 25%).

#### 6.5.1 Matching (greedy page-sequential, one-to-one)

```text
d1_stitch_multipage(page_tables[0..n]):
  next_logical_id = 1
  for i in 1..n:
    bottoms = tables on page i-1 with bbox in bottom 30% height, not yet paired down
    tops    = tables on page i   with bbox in top 30% height, not yet paired up
    // greedy: score all pairs, sort by score desc, assign one-to-one
    for (a,b) in candidate_pairs sorted by stitch_score:
      if a or b already matched: continue
      if match(a,b):
        link a -> b
        if a.logical_table_id is None: a.logical_table_id = next_logical_id++
        b.logical_table_id = a.logical_table_id
        b.continued_from_previous_page = true
        a.continued_to_next_page = true
        b.fragment_index = a.fragment_index + 1

match(a,b):
  cols equal
  AND mean |col_x_center diffs| ≤ 3.0
  AND (header_sim ≥ 0.9 OR header_rows_of(b) text is subset of a header (repeated header)
       OR (b has no header row AND body col alignment holds AND a.continued chain active))
  AND methods compatible (Lattice/Stream/Hybrid/Structure)

stitch_score = 0.5*header_sim + 0.5*(1 - mean_col_dx/3) clamped

Multi-page chains: transitive via continued_* flags (page0-1, page1-2, ...).
Conflicting multi-match: one-to-one greedy prevents one bottom linking two tops.
```

#### 6.5.2 API matrix

| API | Returns |
|-----|---------|
| `Page::tables()` | **Always page-local fragments** with flags + logical_table_id when stitched |
| `Document::tables_stitched()` | Opt-in: `StitchedTable` rows concatenated; drop repeated headers when header_sim≥0.9 |
| JSON default | fragments + flags (`emit_stitched_export=false`) |
| **Scoreboard adapter (normative for V6)** | When `stitch_multipage=true` (default on detect), adapter emits **one grid per logical_table_id** (stitched body) as `tables[]` for metrics — **so bank exp=1 → pred=1 → detect_f1=1.0** |

```text
Bank gold: expected_table_count=1, table_count_tolerance=1
Without stitch adapter: pred=2 → detect_f1 = 2*1/(2+1)? wait: TP=min(2,1)=1, FP=1, FN=0
  prec=1/2, rec=1/1, f1=2/3 ≈ 0.67  (plumber/mupdf)
With stitch adapter: pred=1 → f1=1.0
within_tolerance(|2-1|<=1)=true does NOT raise f1 — adapter stitch required for column win.
```

**V6 gate:** bank **detect_f1 == 1.0** via stitched adapter output (not flags-only).

### 6.6 Evaluation binding

| Engine output | Metric |
|---------------|--------|
| `tables: List[grid]` from adapter | detect_f1, cell_f1, shape |
| IRS/NIST | **pred count** predicates (§2.2), not fractional F1 |
| Stitched logical grids | bank detect_f1 |

```text
extract adapter:
  text, tables (logical if stitch else fragments),
  table_meta[{confidence, method, continued_*, page, logical_id}],
  predicted_table_count = len(tables)  # after stitch policy
```

CI:

```bash
python benchmark/scripts/run_accuracy_benchmark.py --lib pdfparser
# grid-gold mean cell_f1 >= 0.90 AND per-doc floors
# 07 stream; 08 hybrid; 21 overflow; 38 pred==0; 41 pred<=5; 20 detect_f1==1.0
```

### 6.7 Quantitative targets (normative)

| ID | Target | Phase |
|----|--------|-------|
| T1 | Grid-gold **mean cell_f1 ≥ 0.90** AND all per-doc floors below | 0.2 |
| T1a | `06,09,10` cell_f1 ≥ 0.95; `21` ≥ 0.95; `22,23,24,25,27` ≥ 0.90; `07` ≥ 0.85; `08` ≥ 0.70 | 0.2 |
| T2 | Stream `07` detect_f1 ≥ 0.9 and cell_f1 ≥ 0.85 | 0.2 |
| T3 | Hybrid `08` cell_f1 ≥ 0.70 | 0.2 |
| T4 | Bank: adapter stitched **detect_f1 == 1.0** | 0.2 |
| T5 | IRS: **`predicted_table_count == 0`** | 0.2 |
| T5b | NIST **product** floor: **`predicted_table_count ≤ 5`** (`within_tolerance`; **not** detect_f1) | 0.2 |
| T5c | NIST **stretch / overall**: **`predicted_table_count == 0`** (detect_f1=1.0); not required for ≥88 | stretch |
| T6 | `36` predicted ∈ [1, 3] | 0.2 |
| T7 | Overflow `21` cell_f1 ≥ 0.95 | 0.2 |
| T8 | Liabilities `37`: **report-only** (spike before promoting) | — |
| T9 | Side-by-side underscores preserved | 0.2 |
| T10 | Dense numeric cell_f1 ≥ 0.99 | 0.2 |
| T11 | Tables off by default; no text regression | 0.1 |

**V10 composite (Issue 20):** CI fails if **either** mean < 0.90 **or** any T1a per-doc floor fails (not mean-only).

**Sensitivity:** 11 grid-gold docs; missing `07` (0) and weak `08` (0.15) pulls plumber to 0.826. Hitting T2+T3+rest≈1.0 yields mean ~**0.95**. Failing only hybrid at 0.70 still allows mean ≥0.90 if stream≥0.85 and others≥0.95.

### 6.8 Explicit out-of-0.2-gate / report-only (Issue 9)

| Gap | Handling |
|-----|----------|
| Census under-extraction (`32`) | Report-only; optional future **P6 under-seg split** |
| arXiv caption/figure junk tables (`40`) | Report-only; extend P1 with caption/figure proximity later |
| CA WARN scale (`30`) table page-ms | Report wall/RSS; soft table budget `max_table_ms_per_page`; not hard 0.2 accuracy gate |
| In-page 90° rotated tables (C3.8) | Best-effort H/V swap; report-only |
| Liabilities detect≥1 (`37`) | Report-only until spike (T8) |

Exceed-competitors claims for 0.2 **must not** depend on these without promoting them to floors in a later R-*.

### 6.9 Cell text fidelity pipeline

```text
  TextRun[] (canonical from ContentVM)
        |
        |  NEVER render→OCR / never lossy table rewrite
        v
  GeometryIndex.query(cell.bbox) → runs
        |
        v
  Assign by max IoU / center → order → join (P3)
        |
        v
  Cell.text + conf + source_run_ids
```

---

## 7. Supporting systems

### 7.1 Geometry spatial index

```rust
pub struct GeometryIndex {
    run_tree: RTreeOrSweep<RunItem>,
    rule_h: Vec<RuleSegment>,
    rule_v: Vec<RuleSegment>,
}
```

Prefer pure-Rust sweep for axis-aligned v0 or `rstar` (Q1).

### 7.2 Path capture (EX11/EX22 keep)

Internal `capture_rule_segments` when lattice|hybrid; independent of public `include_paths`; cache fingerprint includes flags.

### 7.3 Resource governor unchanged (K16)

Caps: path segments, table candidates (256), tables/page (32), optional `max_table_ms_per_page` (report + soft).

---

## 8. API / IR changes

### 8.1 Crate surface

| Item | 0.1 | 0.2 |
|------|-----|-----|
| `pdfparser-tables` | workspace, publish=false | internal/experimental |
| `Page::tables()` | experimental fragments | stable if gates pass |
| `Document::tables_stitched()` | experimental opt-in | experimental |

### 8.2 ExtractOptions

```rust
pub struct ExtractOptions {
    pub text: TextOptions,
    pub layout: LayoutOptions,  // no detectors; structure map build lives here
    pub table: TableOptions,    // R14
    pub apply_page_rotate: bool,
    pub include_paths: bool,
    pub allow_partial_document: bool,
}

pub struct TableOptions {
    pub detect_tables: bool,              // default false (K21)
    pub modes: TableModeSet,
    pub preset: TablePreset,              // R2 progressive
    pub min_confidence: f32,              // 0.60
    pub min_confidence_stream: f32,       // 0.55
    pub max_tables_per_page: u32,         // 32
    pub soft_max_tables_per_page: u32,    // 8
    pub stitch_multipage: bool,           // default true when detect_tables
    pub emit_stitched_export: bool,       // default false
    pub enable_form_discriminator: bool,  // default true
    pub enable_superscript_recovery: bool,// default true experimental
    pub enable_overflow_merge: bool,      // default true
}

pub enum TablePreset {
    /// S2 (+ S1 if map): after U-phase
    LatticeOnly,
    /// S1–S3 + P1: after stream green
    LatticeStream,
    /// S1–S4 + P1–P4 + D1: after V3 form disc green (recommended default when detect on post-V3)
    Full,
}
```

**R2 progressive presets (Issue 13):**

| When `detect_tables=true` | Recommended preset | Modes |
|---------------------------|--------------------|-------|
| Experimental early | `LatticeOnly` | structure+lattice; stream/hybrid **false** |
| After V1 stream green | `LatticeStream` | +stream; hybrid false until V2 |
| After V3 form disc green | `Full` | all S1–S4 + posts + D1 |

`TableModeSet::all()` only when preset=`Full` **or** embedder overrides. Document that flipping `detect_tables` alone before V3 should use `LatticeOnly` default in experimental API until form disc lands.

### 8.3 Warning codes (additive)

`FormLikenessSuppressed`, `OverSegmentationMerged`, `SuperscriptRowCollapsed`, `StreamMultiLineMerged`, `StitchedHeaderDeduped`, `HybridEqualRowEstimate`.

---

## 9. Alternatives considered

### A) Wrap Camelot/Tabula via FFI — **Rejected**
Java/Ghostscript; breaks native/governor; not pure Rust.

### B) MuPDF-only tables — **Rejected**
AGPL; cell corruption; stream/superscript still fail.

### C) Lattice-only — **Rejected**
Leaves stream gap at 0.

### D) Multi-strategy fusion S1–S4 + P1–P5 + D1 — **Chosen (R3)**

---

## 10. Risks, security, observability, rollout

### 10.1 Risks

| ID | Risk | Severity | Mitigation |
|----|------|----------|------------|
| RK1 | Table CPU vs text SLOs | High | Default off; time budget; presets |
| RK2 | Stream prose FPs | High | prose_likeness + P1 |
| RK3 | Liabilities still hard | Med | P5 report-only |
| RK4 | Stitch false merges | Med | greedy one-to-one; header_sim |
| RK5 | Geometry index cost | Med | caps |
| RK6 | Scope vs K35 | High | Phase T first |
| RK7 | min_conf hurts recall | Med | dual thresholds |
| RK8 | Expected-0 metric misuse | High | §2.2 harness-true gates only |

### 10.2 Security

Tables consume governor-decoded IR only; caps; no JS/XFA; K30 process isolation.

### 10.3 Observability

Spans: `table.detect_page`, `table.S{1-4}`, `table.P{1-5}`, `table.D1`, `table.nms`, `table.cell_assign`.  
Metrics: candidates/emitted/vetoed_form/page_ms.

### 10.4 Rollout

```text
1. Phase T text gates green
2. detect_tables default false
3. U: LatticeOnly preset
4. V1 stream → LatticeStream preset
5. V3 form disc → Full preset recommended
6. V6 stitch adapter required for bank scoreboard
7. Overall ≥88 modeled; ≥92 aspirational post-P2
```

---

## 11. Key Decisions (redesign IDs)

| ID | Decision | Rationale |
|----|----------|-----------|
| **R1** | Crate **`pdfparser-tables`** | First-class subsystem |
| **R2** | Progressive **presets** (LatticeOnly → LatticeStream → Full); detect default false | K21 + FP risk before P1 tuned |
| **R3** | Multi-strategy S1–S4 + posts + D1 chosen | Beat market gaps |
| **R4** | Multi-page stitch first-class 0.2; supersedes **EX13** | Bank statements; adapter logical count |
| **R5** | Stream multi-line cells ON (revises E-T6) | Overflow/stream excellence |
| **R6** | min_confidence 0.60 (stream 0.55); post-penalties for form/over-seg | FP control without double-count |
| **R7** | Reading order score 1.0 on multi-col fixture | Match pypdf/mupdf |
| **R8** | Page /Rotate frozen matrices before text+tables | Never miner failure |
| **R9** | Cell text from ContentVM geometry assign | Beat MuPDF corruption |
| **R10** | Table IR stitch metadata + provenance + per-cell conf | Observability |
| **R11** | 0.2 gates = scoreboard floors §6.7 | Competitive mandate |
| **R12** | Text path never depends on table engine | K35 |
| **R13** | Dual-path oracle CI-only | K5 |
| **R14** | `TableOptions` on ExtractOptions | API clarity |
| **R15** | No Camelot/Tabula/MuPDF runtime deps | Native pure-Rust |
| **R16** | P1 form discriminator mandatory when tables on | Real FP severity #1 |
| **R17** | P4 side-by-side / anti over-seg explicit | plumber 20-table fail |
| **R18** | Scoreboard normative for claims | Measured product |
| **R19** | Vol2 Issue 19 **weights retained**; form only post-penalty | No silent weight replace; no double-count |
| **R20** | StructurePageMap built in **layout**; tables consume only; Vol2 C3.7 phase/EX13 errata | Ownership + supersession clarity |

**Locked product decisions retained:** K1, K2, K8, K9, K15, K16, K21, K23, K34, K35, K36.

---

## 12. Open Questions

| # | Question | Lean | Owner |
|---|----------|------|-------|
| Q1 | R-tree vs sweep for GeometryIndex? | sweep first if AA-only | impl |
| Q2 | Stitched JSON default? | opt-in export; adapter stitch for bench | API |
| Q3 | NIST expected=0 with tol=5 permanent? | gate pred≤5; optional graded fp metric later | corpus |
| Q4 | Graduate tables from experimental? | after T1–T7 green 2 releases | PM |
| Q5 | Rayon parallel S1–S4? | sequential first | perf |
| Q6 | Liabilities gold grid richness? | required before promoting T8 | corpus |
| Q7 | Adaptive min_conf without ML? | form_likeness only v1 | tables |
| Q8 | Deprecate layout.detect_tables alias? | single field 0.2 freeze | API |

---

## 13. PR Plan (ordered; text-first then table milestones)

### Phase T — Text parity

| PR | Deliverable | Gate |
|----|-------------|------|
| T0 | Workspace skeleton incl. tables stub | compile |
| T1 | Backend + governor + filters | adversarial limits |
| T2 | Content VM + fonts | metrics bbox smoke |
| T3 | Page::text + space + **R8 matrices** | `11` token F1=1.0 |
| T4 | Reading order | `02` score 1.0 |
| T5 | CER + accuracy adapter | text_token_f1 ≥ 0.99 basic |
| T6 | Lazy pages; **report** ms/RSS (soft only) | no plumber-class RSS; numbers logged |

### Phase U — Table foundation

| PR | Deliverable | Gate |
|----|-------------|------|
| U0 | tables crate + IR + TableOptions/presets | unit types |
| U1 | GeometryIndex + RuleSegmentBuf | lattice segments golden |
| U2 | S2 + R9 cell assign | `06,09,10` cell_f1 ≥ 0.95 |
| U3 | S1 structure (consume layout map) | structure ≥ 0.90 when gold |
| U4 | NMS + two-phase conf + Vol2 weights | 3×3 conf ≥ 0.75 |

### Phase V — Differentiation

| PR | Deliverable | Gate |
|----|-------------|------|
| V1 | S3 stream (R5) | `07` detect≥0.9 cell≥0.85 |
| V2 | S4 hybrid grid builder | `08` cell_f1 ≥ 0.70 |
| V3 | P1 form disc + formulas | **IRS pred==0**; unit form_likeness goldens |
| V4 | P4 anti over-seg | `36` ∈[1,3]; `23` exact |
| V5 | P3 overflow | `21` cell_f1 ≥ 0.95 |
| V6 | D1 stitch + **adapter logical tables** | bank **detect_f1==1.0** |
| V7 | P2 dense numeric | `25` ≥ 0.99 |
| V8 | P5 superscript | **report-only**; spike notes; not V10 blocker |
| V9 | NIST pred≤5 product floor (T5b); report detect_f1 separately (0 unless pred==0) | within_tolerance; do not claim table score recovery |
| V10 | Grid-gold mean≥0.90 **and** T1a floors | composite CI |

### Phase W — Stabilize

| PR | Deliverable | Gate |
|----|-------------|------|
| W1 | CI scoreboard on text/table PRs | flake budget |
| W2 | Docs quality matrix + known limits §6.8 | |
| W3 | Full preset after V3; detect still default false | |
| W4 | Optional stable API | 2 green releases |

```text
  T0..T6 --text gates--+
                       |
  U0..U4 --------------+--> V1..V7,V9,V10 --> W*
                       |         V8 report-only
```

---

## 14. References

1. Product design: `docs/design-native-pdf-parser.md`
2. Volume 2 LLD: `docs/design-architecture-feature-extraction.md` (C1, C3, EX*, Issue 19)
3. Market analysis Parts I–II: `docs/market-analysis-pdf-parsers.md`
4. Accuracy scoreboard: `docs/accuracy-scoreboard.md`
5. Metrics: `benchmark/scripts/metrics.py` (`table_detection_metrics` expected-0 behavior)
6. Harness: `benchmark/scripts/run_accuracy_benchmark.py`
7. Camelot/Tabula taxonomy (reference only)
8. ISO 32000 structure roles Table/TR/TH/TD

---

## Appendix A — Confidence quick reference

```text
base = Vol2 Issue 19 weights (R19) using fill (cheap pre-NMS, exact post-assign)
confidence_v1 = clamp01(base - 0.5*form_likeness - over_seg_penalty - low_fill_penalty)

emit iff confidence_v1 >= min_for(method) AND not hard_vetoed
hard_veto: form_likeness ≥ 0.75 && numeric_density < 0.15
           (unless Structure && base ≥ 0.85)

IRS: success <=> predicted_table_count == 0
NIST product floor (T5b): success <=> predicted_table_count ≤ 5  (within_tolerance only; detect_f1 still 0 if pred>0)
NIST stretch / overall recovery: predicted_table_count == 0  (detect_f1 == 1.0)
```

## Appendix B — Pipeline matrix vs scoreboard docs

| Doc | Primary pipeline | Success |
|-----|------------------|---------|
| 06 lattice | S2 | cell_f1 1.0 |
| 07 stream | S3 | detect+cell high |
| 08 hybrid | S4 | cell_f1 ≥ 0.70; not 1×2 |
| 09 complex | S2+P2 | cell_f1 ~1 |
| 20 bank | S2/S3+D1+P3 | detect_f1 1.0 stitched |
| 21 overflow | S2+P3 | cell_f1 ≥ 0.95 |
| 23 side-by-side | S2+P4+P3 | 2 tables, clean tokens |
| 25 dense | S2+P2 | cell_f1 ≥ 0.99 |
| 36 two_tables | P4+NMS | count 1–3 |
| 37 liabilities | P5 | report-only |
| 38 IRS | P1 | **pred==0** |
| 41 NIST | P1 | product: **pred≤5** (no F1 lift); stretch overall: **pred==0** |
| 32 census | — | report-only under-seg |
| 40 arXiv | — | report-only caption FP |
| 30 WARN | — | report perf |

## Appendix C — Text options sketch

```rust
pub struct TextOptions {
    pub preserve_positions: bool,
    pub sort_reading_order: bool,
    pub insert_spaces: bool,
    pub unknown_glyph: char,
    pub apply_page_rotate: bool,
}
```

## Appendix D — Non-goals reminder

- OCR primary path; perfect untagged tables; AGPL engines; breaking K15 in 0.1; tables on critical path before text parity (K35); claiming fractional detect F1 when expected=0 without metrics.py change.

## Appendix E — Revision log

| Rev | Date | Notes |
|-----|------|-------|
| 1.0 | 2026-07-10 | Initial redesign draft |
| 1.1 | 2026-07-10 | Review remediation: Issues 1–20 — harness-true FP gates; unified S/P/D IDs; feature formulas; R19 weights; stitch adapter; hybrid 5×5 builder; structure ownership R20; overall model; report-only gaps; frozen rotate matrices; presets; two-phase NMS |
| 1.2 | 2026-07-10 | Issue R1: §2.4 NIST pred≤5 product gate separated from scoreboard detect F1 / overall recovery (pred==0 only) |

---

*End of redesign document — 2026-07-10 rev 1.1*
