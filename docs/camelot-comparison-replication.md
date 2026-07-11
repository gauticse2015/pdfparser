# Camelot comparison replication — honest head-to-head

**Date:** 2026-07-11  
**Question:** Can we publish that pdfparser outperforms the tools on [Camelot’s comparison page](https://camelot-py.readthedocs.io/en/latest/user/comparison.html) **using the same documents, gold, and metrics Camelot uses**?  
**Answer:** **No — not with current pdfparser.** On Camelot’s ICDAR-2013 quantitative harness, **Camelot wins decisively**. Our earlier “SOTA” claims apply only to **our own synthetic grid-gold harness**, not to Camelot’s corpus.

**Constraint honored:** no business-logic changes to pdfparser; pure black-box CLI extract vs peers.

---

## 1. What Camelot’s comparison page actually is

| Component | What it is | Quality metrics? |
|-----------|------------|------------------|
| Capability matrix | Qualitative ✓/✗ features | No |
| Side-by-side `agstat.pdf` | Visual / CSV examples | Qualitative |
| `docs/benchmark/` | ~21 PDFs (lattice + stream) + per-tool CSVs | **No independent gold** — CSVs are tool outputs |
| `python bench/comparison.py` | Table **count + timing** only | Explicitly **not** quality |
| **ICDAR-2013 head-to-head** (`bench/benchmark_icdar.py`) | **67 born-digital PDFs** + `*-str.xml` structure GT | **Yes** — F1, TEDS proxy, row, col |

Camelot’s **only published quantitative quality comparison** against peers on a fixed set with independent structure ground truth (in their current docs) is the **ICDAR-2013** block vs tablers:

| tool / config | F1 | TEDS | row | col | time |
|---|---:|---:|---:|---:|---:|
| camelot lattice `combined` | **0.778** | **0.789** | **0.762** | **0.829** | 101 s |
| camelot lattice `vector` | 0.766 | 0.784 | 0.748 | 0.806 | 13 s |
| tablers | 0.750 | 0.724 | 0.657 | 0.741 | 1.5 s |

Source: Camelot docs “Head-to-head on ruled tables”, last verified 2026-05-25.

---

## 2. Did we use the same dataset / gold / pipeline?

| Requirement | Status |
|-------------|--------|
| Same PDFs as Camelot ICDAR set | **Yes** — shipped at `camelot-dev/camelot` → `tests/files/tabula/icdar2013-dataset/` (**67 PDFs**) |
| Same ground truth | **Yes** — ICDAR `*-str.xml` parsed with **Camelot’s** `parse_icdar_str_xml` |
| Same metrics | **Yes** — Camelot `bench/_metrics.score` (detection F1, difflib TEDS proxy, exact row/col) |
| Same methodology | **Yes** — page-keyed `{page: [grid,...]}`, order match on page, aggregate score |
| pdfparser code changes | **None** — `pdfparser extract --format json --tables` only |
| Reproduced Camelot baseline | **Yes** — lattice/vector **F1=0.766 TEDS=0.784** matches their published table **exactly** |

**We did not use our synthetic `benchmark/corpus` grid-gold for this claim.** That was a separate product harness.

---

## 3. ICDAR-2013 results (authoritative for this question)

**Measured 2026-07-11 on this machine** (camelot-py 2.0.0, pdfparser release CLI, pdfplumber, pymupdf).

| Rank | Tool / config | F1 | TEDS | row | col | time (s) |
|-----:|---------------|---:|-----:|----:|----:|---------:|
| 1 | **camelot `auto`** | **0.864** | **0.786** | 0.564 | 0.792 | 72.6 |
| 2 | pymupdf `find_tables` | 0.776 | 0.674 | 0.578 | 0.642 | 10.2 |
| 3 | **camelot lattice/vector** | **0.766** | **0.784** | **0.748** | **0.806** | 3.2 |
| 4 | camelot hybrid/vector | 0.726 | 0.755 | 0.464 | 0.715 | 4.9 |
| 5 | pdfplumber | 0.662 | 0.650 | 0.571 | 0.533 | 8.4 |
| **5** | pdfplumber | 0.662 | 0.650 | 0.571 | 0.533 | 8.4 |
| **6** | **pdfparser (pre multi-region baseline)** | **0.627** | **0.238** | **0.090** | **0.191** | **0.6** |

**Latest re-run (post multi-region + review fixes):** pdfparser **F1=0.601 TEDS=0.210 row=0.134 col=0.193** (~0.5 s). Full write-up: [`icdar-competitive-report.md`](icdar-competitive-report.md).
| 7 | camelot stream | 0.585 | 0.470 | 0.053 | 0.490 | 3.3 |

Machine-readable: `benchmark/results/camelot_icdar_headtohead.json`.

### Interpretation (honest)

1. **pdfparser is not SOTA on ICDAR-2013.**  
2. Detection F1 (~0.63) is behind Camelot lattice (~0.77) and auto (~0.86).  
3. **TEDS 0.238 and row/col ~0.09/0.19** show that when tables are matched, **structure/content alignment is poor** on this corpus (spans, multi-table pages, background lines, etc.).  
4. **Speed:** pdfparser is fastest (~0.6 s full set) but quality does not lead.  
5. Reproduced Camelot lattice/vector line-for-line with their published numbers → pipeline validity confirmed.

---

## 4. Camelot `docs/benchmark/` side-by-side corpus (21 PDFs)

This is the “dozen lattice + stream” set linked from the comparison page (agstat, column_span_*, health, budget, …).

**Important:** Files named `*-data-camelot-*.csv` are **Camelot’s own outputs**, not independent ground truth. Scoring “TEDS vs Camelot CSV” **favors Camelot by construction**. We still ran it for transparency.

| Tool | cases | mean detect F1 vs Camelot-CSV count | mean TEDS vs Camelot CSV |
|------|------:|------------------------------------:|-------------------------:|
| camelot_stream | 20 | 0.927 | 0.671 |
| camelot_lattice | 20 | 0.700 | 0.434 |
| pdfplumber | 21 | 0.565 | 0.306 |
| **pdfparser** | 20 | **0.650** | **0.183** |

Highlights from case logs:

- **Lattice column_span_1 / column_span_2:** Camelot/plumber extract; **pdfparser predicted 0 tables**.  
- **agstat (Camelot’s showcase PDF):** all detect 1 table; TEDS vs Camelot CSV: camelot_lattice **0.98**, pdfparser **0.01**.  
- **Stream missing_values / population_growth / superscript:** Camelot stream strong; **pdfparser often 0 tables**.  
- **birdisland:** encrypted — pdfparser rejects (product K15); Camelot also fails text extraction without decrypt.

Raw: `benchmark/results/camelot_docs_benchmark_headtohead.json`.

Even granting Camelot CSVs as soft reference, **we do not outperform Camelot on their comparison assets.**

---

## 5. Explicit answer to the publishing question

| Claim | Allowed? |
|-------|----------|
| “We used the same ICDAR-2013 set + Camelot metrics as Camelot’s published head-to-head.” | **Yes** (this report) |
| “We beat Camelot / all tools on that set.” | **No — false with current code** |
| “We are top on our synthetic grid-gold harness.” | **Yes** (separate study; different data) |
| “We are better than Camelot’s comparison page analysis overall.” | **No** |

### What we may publish truthfully

> On Camelot’s in-repo **ICDAR-2013** evaluation (67 PDFs, official structure XML, Camelot’s F1/TEDS metrics), **camelot auto / lattice currently outperform pdfparser** (F1 0.86 / 0.77 vs **0.63**). pdfparser is **fastest** but **not highest quality** on that corpus.  
> On **our product grid-gold suite**, pdfparser leads digital extractors we measured earlier; that suite is **not** Camelot’s comparison corpus.

### What we must not publish

> “pdfparser is better than Camelot on the documents and evaluation used in Camelot’s comparison documentation.”

---

## 6. Why results differ from our grid-gold “SOTA”

| Factor | Our grid-gold | ICDAR-2013 (Camelot) |
|--------|---------------|----------------------|
| Origin | Synthetic controlled tables | Real competition PDFs (US/EU) |
| Difficulty | Stream/hybrid engineered wins | Spans, multi-table, background lines, rotation |
| Gold | Our generator cells | ICDAR structure XML |
| Metrics | Our cell F1 micro | Camelot detection F1 + TEDS proxy |
| pdfparser mean cell F1 | **0.990** | TEDS **0.238** |

**No contradiction:** strong on a product-shaped synthetic set ≠ strong on ICDAR. Publication must name **which** dataset.

---

## 7. Reproducibility

```bash
# 1. Clone Camelot (ships ICDAR data)
git clone --depth 1 https://github.com/camelot-dev/camelot.git /tmp/camelot-upstream

# 2. Reproduce Camelot baseline (must match published lattice/vector)
cd /tmp/camelot-upstream
python bench/benchmark_icdar.py --flavor lattice --engine vector
# → F1=0.766 TEDS=0.784 row=0.748 col=0.806

# 3. Head-to-head results (this repo)
# Results files:
#   benchmark/results/camelot_icdar_headtohead.json
#   benchmark/results/camelot_docs_benchmark_headtohead.json
```

Environment: camelot-py 2.0.0, pdfplumber, pymupdf, pdfparser release CLI, macOS arm64. Tabula not included (no JRE).

---

## 8. Implications for product (testing only — no code changed here)

If the goal is to **beat Camelot on ICDAR-style data**, future work (separate from this test report) would need quality lifts on:

- Multi-table pages / under-detection  
- Column/row spans  
- Background decorative lines  
- Rotated tables  
- Stream variants present in ICDAR  

This document does **not** implement those changes.

---

## 9. Bottom line

| Question | Answer |
|----------|--------|
| Same Camelot dataset + gold + metrics? | **Yes (ICDAR-2013)** |
| pdfparser better than Camelot there? | **No** |
| Top among all tools on that set? | **No — 6th of 7 configs tested** |
| Safe for an external publish claiming Camelot-comparison superiority? | **No** |
| What is safe to claim? | Strong on **our** digital grid-gold suite; **not** on Camelot ICDAR without further work |

---

*Prepared for publication integrity review. Prefer this file over any earlier report that mixed harnesses without this disclaimer.*
