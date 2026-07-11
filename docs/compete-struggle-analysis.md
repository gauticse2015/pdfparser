# Competitive struggle analysis & dataset requirements

**Date:** 2026-07-11  
**Status:** Pre-algorithm freeze locked; **post-raster re-baseline** recorded  
**Policy:** No ICDAR PDFs in regression; synthetic + legal public only.  
**Baselines:** `compete_hard_baseline_frozen.json` (pre-algorithm) · `compete_hard_baseline_post_raster.json` (current)

---

## 1. Why we still do not have “enough dataset”

| Existing suite | n | Mean cell F1 (pdfparser) | Problem |
|----------------|--:|-------------------------:|---------|
| hard 50–62 | 13 | ~1.0 | Modes already fixed; few variants |
| precision 70–82 | 13 | ~1.0 | FP chrome only; not TEDS/row chaos |
| sensing 90–95 | 6 | ~1.0 post-fix | Single instance per mode |
| compete C001–C068 | 64 | **0.94** | Only C043–C046 (false underlines) fail |

**ICDAR-2013 competitive reality (external, not in corpus):**

| Metric | pdfparser | Camelot lattice |
|--------|----------:|----------------:|
| Detection F1 | ~0.65–0.67 | ~0.77 |
| TEDS (difflib proxy) | **~0.34** | ~0.78 |
| Row accuracy | **~0.35** | high |
| Col accuracy | **~0.40** | high |
| Docs with TEDS&lt;0.2 | **32 / 67** | few |
| “Good” (F1≥0.95 & TEDS≥0.7) | **4 / 67** | many |

A handful of fixtures that score 1.0 cannot drive TEDS/row/col work. We need a **large, parametric, currently-failing** owned suite.

### Root cause of synthetic illusion

From `docs/icdar-plateau-analysis-and-plan.md`:

1. Synthetic grids are **clean vector rules** → our lattice shines.
2. On ICDAR, lattice fires on **~6/67** docs; stream/hybrid dominate and **fragment**.
3. Camelot wins with **raster line sensing** + **network borderless** + **exclusive auto routing**, not span knobs.

So dataset must include modes where:

- vector lattice is **silent** (image/painted/dashed/broken that we still miss),
- stream **fragments** large borderless tables (header-only slices),
- lattice **miscounts rows/cols** (underlines, partial V, sparse densify),
- multi-table **count/order** breaks F1 matching.

---

## 2. Failure taxonomy → dataset dimensions

Mapped from ICDAR `icdar_failure_analysis.json` + live engine probes:

| Class | ICDAR signal | Dataset dimension | Probe status on pdfparser |
|-------|--------------|-------------------|---------------------------|
| C1/C2 OVER_DETECT | 36 docs | Chrome + joint-rich minigrids + 1 real table | Severe minigrids: often over-detect or miss real table |
| C3 MISS_ALL / UNDER | 15 | **Image-only rules**, faint/missing vector | Image tables: **0 tables** |
| C4 ROW_FRAGMENT | large | 28–58 row irregular borderless + section breaks | Fragments into 3–7 stream tables |
| C5 HEADER_ONLY_SLICE | us-017 class | Multi-level header + irregular body | Same as C4 |
| C6 COL_SKEW / partial V | wide stats | Full H, V every 2–4 cols | Lattice emits **half/third columns** |
| C7 ROW_OVERCOUNT | underlines | Per-cell underlines inside true grid | Rows **exactly 2×** gold |
| C8 MULTI_TABLE | 17 | 3–6 close heterogeneous tables + chrome | Count/order sensitive |
| C9 MULTIPAGE | 28 | Multi-page + chrome + row noise | Underline multipage fails row acc |
| C10 COUNT_OK_STRUCT_BAD | 13 | Shape wrong while n_tables ok | Partial V, sparse densify |
| C11 ZERO_TEDS same shape | rare | Wrapped multi-run cells | Cell F1 sensitive |
| C13 MIXED | — | Lattice + irregular stream same page | Stream often wrong shape/split |
| C14 INVOICE | — | Fee rows inside same grid as body | Footer strip incomplete |
| C15 SPANS | — | Multi colspan/rowspan financial | Partial residual |

---

## 3. What dataset we built

### 3.1 Coverage wave (C001–C068) — *too easy, keep as regression of solved modes*

Parametric but mostly **already solved** after sensing/precision work. Retained so we do not regress easy wins.

### 3.2 Hard struggle wave (C100–C180) — **primary open_struggle suite**

| Struggle mode | n | Typical pred vs gold |
|---------------|--:|----------------------|
| image_painted_miss_all | 8 | `[]` vs NxM |
| header_slice_fragmentation | 8 | 3–7 stream fragments vs 1 large table |
| partial_v_col_undercount | 18 | cols ≈ nc/step |
| false_underline_row_overcount | 8 | rows = 2× gold |
| sparse_densify_row_undercount | 8 | fewer rows than gold |
| mixed_stream_miss / fragment | 6 | wrong stream shape or count |
| multi_table_close_chrome | 5 | count/order stress |
| multipage_underline_overcount | 4 | 2× rows each page |
| complex_spans | 4 | span residual |
| invoice_fees_inside_grid | 4 | extra fee rows |
| severe_minigrid_overdetect | 4 | over-detect or miss |
| cell_content_assign | 4 | wrapped multi-run cells |

**Total hard synthetic: 81** with full `expected_tables` grids.

### 3.3 Real public (compete_real)

Legal public PDFs (US gov / project samples). Soft gold (no full grids for most); used for detect chaos / smoke:

- R001–R009 (wave 1), R010, R016–R020 (wave 2)  
- Includes Tabula classic hard tables, Camelot samples, IRS forms, BEA GDP.

ICDAR competition packages are **not** downloaded into corpus.

---

## 4. Metrics the suite must report (F1 / TEDS / row / col)

Harness (`run_accuracy_benchmark.py` suite `regression_compete` / `regression_compete_hard`):

| Metric | Role |
|--------|------|
| `table_detect_f1` | Count-based detection F1 (ICDAR F1 proxy) |
| `table_row_accuracy` | Exact row count fraction |
| `table_col_accuracy` | Exact col count fraction |
| `table_shape_exact_rate` | Exact RxC |
| `table_cell_f1` | Aligned cell text micro-F1 (**TEDS-like** content quality) |
| Per `failure_classes` / `struggle_mode` | Breakdown for targeting |

ICDAR official score stays external via `run_icdar_competitive.py`.

---

## 5. Success criteria for the *dataset* (before any algorithm PR)

1. **Hard wave mean cell F1 clearly &lt; 0.7** (not another 0.94 suite).
2. Every taxonomy class has **≥4 hard fixtures** (see class index).
3. Image-miss, partial-V, underline, header-slice each show **systematic** failures.
4. Baseline JSON frozen under `benchmark/results/accuracy_results_compete_hard.json`.
5. Peers (optional later) show differentiated wins — proves fixtures are “real hard,” not broken gold.

---

## 6. What synthetic cannot fully replace

| Gap | Why synthetic is weak | Mitigation |
|-----|----------------------|------------|
| True raster anti-aliased rules | ReportLab image grids approximate; real scanners differ | Real public statistical PDFs + future raster engine eval |
| Vendor-specific content streams | ICDAR has exotic painters | Real downloads (BEA, BLS, IRS, Tabula samples) |
| Order-matched multi-page gold XML | Manual gold expensive | Soft count gold on reals; full grids only on synthetic |
| Network-class borderless quirks | Irregular synthetic still “tabular looking” | Header-slice + jitter + section breaks (validated fail) |

**Conclusion:** Synthetic at **scale with probe-validated fail modes** closes most development loops. Real PDFs cover residual “wild” sensing. ICDAR remains **external competitive scoreboard only**.

---

## 7. Process going forward

```
1. Fixture + gold (this suite)
2. Freeze baseline (pdfparser metrics per class)
3. Diagnose failure class
4. Generic algorithm fix (Auto routing / Network / ROI raster …)
5. Re-measure hard + coverage suites
6. Mark mode solved_regression only after hard fixtures pass without ICDAR leakage
```

Do **not** fix-first and then invent soft fixtures (process debt from doc 54).

---

## 8. Post-raster re-baseline (2026-07-11)

After production Camelot-class raster line sensing on embedded Image XObjects:

| Metric | Pre-algorithm | Post-raster |
|--------|-------------:|------------:|
| overall | 50.4 | **61.5** |
| cell F1 | 0.383 | **0.445** |
| detect F1 | 0.795 | **0.884** |
| shape exact | 0.204 | **0.444** |
| imperfect rate | 87.7% | **74.1%** |

- **Solved structure for `image_painted_miss_all`:** detect/shape 0→1.0 (cell F1 stays 0 without OCR).
- **Solved `false_underline_row_overcount` / multipage underline:** cell/shape 1.0.
- **Still open:** header-slice, partial-V col undercount, sparse densify, mixed stream, some spans.

Scoreboard: `docs/accuracy-scoreboard-compete-hard.md`.

## 9. Commands

```bash
# regenerate hard wave
.venv/bin/python benchmark/scripts/generate_compete_hard_corpus.py

# coverage wave (easy)
.venv/bin/python benchmark/scripts/generate_compete_corpus.py

# reals
.venv/bin/python benchmark/scripts/fetch_compete_real.py

# re-baseline hard only (pdfparser)
.venv/bin/python benchmark/scripts/run_accuracy_benchmark.py \
  --suite regression_compete_hard --libs pdfparser --tag compete_hard

# full compete (coverage + hard + real)
.venv/bin/python benchmark/scripts/run_accuracy_benchmark.py \
  --suite regression_compete --libs pdfparser --tag compete
```
