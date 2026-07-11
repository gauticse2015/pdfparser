# ICDAR-2013 Competitive Analysis (external)

**Policy:** ICDAR is **not** part of the regression corpus. This report is for competitive measurement only.

**Docs:** 67 · **Gold:** ICDAR `*-str.xml` · **Metrics:** Camelot `bench/_metrics.score` (F1, TEDS proxy, row/col)

## Leaderboard

| Rank | Tool | F1 | TEDS | row | col | time (s) |
|-----:|------|---:|-----:|----:|----:|---------:|
| 1 | **camelot_auto** | 0.864 | 0.786 | 0.564 | 0.792 | 72.21 |
| 2 | **pymupdf** | 0.776 | 0.674 | 0.578 | 0.642 | 10.58 |
| 3 | **camelot_lattice_vector** | 0.766 | 0.784 | 0.748 | 0.806 | 3.63 |
| 4 | **pdfplumber** | 0.662 | 0.650 | 0.571 | 0.533 | 8.53 |
| 5 | **pdfparser** ← **ours** | 0.601 | 0.210 | 0.134 | 0.193 | 0.55 |

## pdfparser vs Camelot (headline)

| Metric | pdfparser | camelot lattice/vector | camelot auto | Δ vs lattice |
|--------|----------:|-----------------------:|-------------:|-------------:|
| f1 | 0.601 | 0.766 | 0.864 | -0.165 |
| teds | 0.210 | 0.784 | 0.786 | -0.574 |
| row | 0.134 | 0.748 | 0.564 | -0.613 |
| col | 0.193 | 0.806 | 0.792 | -0.613 |

## Improvement vs previous ICDAR run

| Metric | Previous | Now | Δ |
|--------|---------:|----:|--:|
| f1 | 0.627 | 0.601 | -0.026 |
| teds | 0.238 | 0.210 | -0.028 |
| row | 0.090 | 0.134 | +0.045 |
| col | 0.191 | 0.193 | +0.002 |

## Failure mode histogram (pdfparser)

| Mode | Docs |
|------|-----:|
| WRONG_SHAPE | 52 |
| ROW_MISCOUNT | 50 |
| BAD_STRUCTURE | 50 |
| COL_MISCOUNT | 47 |
| OVER_DETECT | 37 |
| MULTI_PAGE_DOC | 28 |
| MULTI_TABLE_PAGE | 17 |
| UNDER_DETECT | 13 |
| MISS_ALL | 4 |

### Buckets

- **miss_all:** 4
- **under:** 9
- **over:** 37
- **bad_struct_ok_count:** 11
- **good:** 3

## Multi-table vs single-table

- Multi-table docs (n=33): mean F1 us=0.560, camelot=0.631; TEDS us=0.125, camelot=0.476
- Single-table docs (n=34): mean F1 us=0.626, camelot=0.775; TEDS us=0.300, camelot=0.519

## Worst TEDS gap vs Camelot lattice (top 15)

| Doc | ΔTEDS | F1 us/c | TEDS us/c | n_gt/us/c | modes |
|------|------:|--------:|----------:|----------:|-------|
| `eu-012.pdf` | -0.973 | 0.43/1.00 | 0.027/1.000 | 5/9/5 | OVER_DETECT, MULTI_TABLE_PAGE, MULTI_PAGE_DOC, ROW_MISCOUNT |
| `eu-014.pdf` | -0.897 | 0.33/1.00 | 0.103/1.000 | 1/5/1 | OVER_DETECT, ROW_MISCOUNT, COL_MISCOUNT, WRONG_SHAPE |
| `eu-025.pdf` | -0.890 | 1.00/1.00 | 0.110/1.000 | 5/5/5 | MULTI_TABLE_PAGE, MULTI_PAGE_DOC, ROW_MISCOUNT, COL_MISCOUNT |
| `us-007.pdf` | -0.888 | 0.44/1.00 | 0.052/0.940 | 2/7/2 | OVER_DETECT, MULTI_PAGE_DOC, ROW_MISCOUNT, COL_MISCOUNT |
| `us-012.pdf` | -0.873 | 1.00/1.00 | 0.034/0.906 | 1/1/1 | ROW_MISCOUNT, COL_MISCOUNT, WRONG_SHAPE, BAD_STRUCTURE |
| `eu-005.pdf` | -0.861 | 1.00/0.80 | 0.139/1.000 | 2/2/3 | MULTI_TABLE_PAGE, ROW_MISCOUNT, COL_MISCOUNT, WRONG_SHAPE |
| `eu-001.pdf` | -0.860 | 0.82/1.00 | 0.140/1.000 | 7/10/7 | OVER_DETECT, MULTI_TABLE_PAGE, MULTI_PAGE_DOC, ROW_MISCOUNT |
| `us-016.pdf` | -0.836 | 0.40/1.00 | 0.039/0.875 | 1/4/1 | OVER_DETECT, ROW_MISCOUNT, COL_MISCOUNT, WRONG_SHAPE |
| `eu-007.pdf` | -0.808 | 0.67/1.00 | 0.192/1.000 | 6/3/6 | UNDER_DETECT, MULTI_TABLE_PAGE, MULTI_PAGE_DOC, ROW_MISCOUNT |
| `eu-015.pdf` | -0.803 | 0.33/1.00 | 0.142/0.945 | 5/1/5 | UNDER_DETECT, MULTI_TABLE_PAGE, MULTI_PAGE_DOC, COL_MISCOUNT |
| `us-005.pdf` | -0.800 | 1.00/1.00 | 0.000/0.800 | 1/1/1 | BAD_STRUCTURE |
| `us-027.pdf` | -0.772 | 0.31/1.00 | 0.000/0.772 | 2/11/2 | OVER_DETECT, MULTI_PAGE_DOC, ROW_MISCOUNT, COL_MISCOUNT |
| `us-014.pdf` | -0.697 | 0.80/1.00 | 0.079/0.776 | 2/3/2 | OVER_DETECT, MULTI_PAGE_DOC, ROW_MISCOUNT, COL_MISCOUNT |
| `us-004.pdf` | -0.695 | 0.67/1.00 | 0.067/0.762 | 1/2/1 | OVER_DETECT, ROW_MISCOUNT, COL_MISCOUNT, WRONG_SHAPE |
| `us-038.pdf` | -0.679 | 1.00/1.00 | 0.000/0.679 | 1/1/1 | ROW_MISCOUNT, COL_MISCOUNT, WRONG_SHAPE, BAD_STRUCTURE |

## Detect OK, structure bad (F1≥0.9, TEDS&lt;0.35) — n=10

- `eu-005.pdf`: shapes us=[[[14, 3], [15, 5]]] gt=[[[15, 3], [16, 9]]] TEDS=0.139
- `eu-025.pdf`: shapes us=[[[9, 7], [3, 6], [5, 7]], [[12, 8], [12, 6]]] gt=[[[4, 4], [11, 4], [6, 4]], [[14, 3], [14, 4]]] TEDS=0.110
- `us-005.pdf`: shapes us=[[[5, 2]]] gt=[[[5, 2]]] TEDS=0.000
- `us-009.pdf`: shapes us=[[[20, 2]]] gt=[[[22, 7]]] TEDS=0.178
- `us-012.pdf`: shapes us=[[[5, 8]]] gt=[[[21, 6]]] TEDS=0.034
- `us-017.pdf`: shapes us=[[[3, 12]], [[3, 3]], [[3, 7]], [[3, 10]], [[3, 10]], [[3, 10]]] gt=[[[31, 10]], [[31, 10]], [[31, 9]], [[31, 8]], [[31, 8]], [[31, 8]]] TEDS=0.028
- `us-018.pdf`: shapes us=[[[3, 8]], [[3, 5]], [[3, 8]], [[4, 7]], [[29, 3]], [[4, 13]], [[4, 8]]] gt=[[[58, 11]], [[58, 10]], [[59, 5]], [[32, 7]], [[29, 4]], [[32, 6]], [[32, 6]]] TEDS=0.048
- `us-023.pdf`: shapes us=[[[11, 14]]] gt=[[[10, 13]]] TEDS=0.223
- `us-030.pdf`: shapes us=[[[15, 4]]] gt=[[[8, 8]]] TEDS=0.224
- `us-038.pdf`: shapes us=[[[4, 2]]] gt=[[[9, 3]]] TEDS=0.000

## MISS_ALL (n=4)

- `eu-018.pdf`: gt=2 camelot=2 camelot TEDS=0.215
- `us-003.pdf`: gt=1 camelot=0 camelot TEDS=0.000
- `us-011a.pdf`: gt=2 camelot=2 camelot TEDS=0.025
- `us-026.pdf`: gt=1 camelot=0 camelot TEDS=0.000

## Gap analysis (where we still lack)

### Executive read

On **ICDAR-2013** (67 born-digital PDFs, Camelot metrics), **pdfparser is not competitive on structure quality** yet:

| Metric | pdfparser | camelot lattice | camelot auto | pymupdf | pdfplumber |
|--------|----------:|----------------:|-------------:|--------:|-----------:|
| F1 | 0.601 | 0.766 | 0.864 | 0.776 | 0.662 |
| TEDS | 0.210 | 0.784 | 0.786 | 0.674 | 0.650 |
| row exact | 0.134 | 0.748 | 0.564 | 0.578 | 0.571 |
| col exact | 0.193 | 0.806 | 0.792 | 0.642 | 0.533 |

**Speed:** pdfparser **0.5s** full set vs camelot auto **72.2s**, lattice **3.6s**.

### What changed vs our earlier ICDAR run (pre multi-region)

| Metric | Then | Now | Note |
|--------|-----:|----:|------|
| F1 | ~0.627 | **0.601** | Slightly **worse** — over-detection tax |
| TEDS | ~0.238 | **0.210** | Still very low |
| row | ~0.090 | **0.134** | Small structure gain |
| col | ~0.191 | **0.193** | Flat |

Our synthetic **hard suite** (multi-table CC + stream regions) improved a lot; ICDAR shows the **precision/structure** side of that coin: more tables emitted, many wrong shapes.

### Gap 1 — Over-detection (dominant now)

- **OVER_DETECT on 37 / 67 docs** (was more under-detect before).
- MISS_ALL dropped to **4** (was ~15) — good for recall side.
- Extra tables come from: stream FPs on prose/headers, lattice CCs on decorative rules/frames, hybrid fragments, page chrome.
- Detection F1 is count-based: every extra table is an FP. Peers (Camelot lattice) are more conservative.

**Need:** stronger FP gates (whitespace/edge/fill), region NMS across strategies, demote small weak lattices, form/deco line filters without corpus hardcoding.

### Gap 2 — Structure / TEDS (still the main quality hole)

Even when table **count** is right:
- Mean TEDS on count-exact docs is still poor (many WRONG_SHAPE).
- ROW_MISCOUNT **50**, COL_MISCOUNT **47**, WRONG_SHAPE **52**, BAD_STRUCTURE **50**.
- Typical pattern: extra H/V lines (header rules, footnotes, partial borders) become extra grid lines; gold expects coarser rows/cols or spans.
- Camelot lattice **row/col exact ~0.75/0.81** vs us **~0.13/0.19** — joint/edge model is still weaker on competition PDFs.

**Need:** joint-supported line filtering (drop lines with weak joint support), span recovery on real partial edges, better text-in-cell (split_text, multi-line), optional raster line recovery for incomplete strokes.

### Gap 3 — Multi-table pages & order matching

- Multi-table docs: mean F1 **0.560**, TEDS **0.125**.
- Metric matches tables **by page order**, not spatial IoU. Extra fragment before a real table shifts all pairs → TEDS collapse even if a good grid exists later.
- Examples: `eu-001` (10 vs 7), `eu-012` (9 vs 5), `us-027` (11 vs 2).

**Need:** ranking tables reading-order consistently; suppress tiny FPs; consider IoU-based matching for analysis (not for published Camelot metric).

### Gap 4 — Stream / borderless / weak rules

- Camelot **auto** still leads (F1 0.86) by picking stream/lattice per page.
- Several US docs where lattice finds 0 and stream/auto works (`us-017`–`us-025` region) — we either over-segment or mis-grid.
- Vector-only: no OpenCV line recovery for anti-aliased / image rules.

### Gap 5 — What is already good

- **Latency** best-in-class.
- **MISS_ALL reduced** substantially vs first ICDAR run.
- Multi-region lattice **helps count** on some multi-table EU docs when not flooded by FPs.
- Synthetic regression hard suite remains strong (do not use ICDAR for day-to-day training).

### Recommended next work (priority for ICDAR competitiveness)

1. **Precision pass (P0):** document-level / page-level FP scrub for tiny low-fill lattices and stream under prose; raise min joints / edge_score for emission.
2. **Line support (P0):** keep only lines with multi-joint support inside a region (Camelot-like) to fix COL/ROW miscount.
3. **Match hygiene (P1):** stable reading order; drop sub-tables contained in larger grids (containment NMS).
4. **Spans (P1):** real missing-edge spans on competition headers.
5. **Optional raster fallback (P2)** for lattice miss cases only.
6. Keep **regression on synthetic hard**; ICDAR as periodic competitive checkpoint only.

---
*Generated by `benchmark/scripts/run_icdar_competitive.py`. ICDAR files remain external; never copied into `benchmark/corpus/`.*
