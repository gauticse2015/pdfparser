# ICDAR-2013 Competitive Analysis (external)

**Policy:** ICDAR is **not** part of the regression corpus. This report is for competitive measurement only.

**Docs:** 67 · **Gold:** ICDAR `*-str.xml` · **Metrics:** Camelot `bench/_metrics.score` (F1, TEDS proxy, row/col)

## Leaderboard

| Rank | Tool | F1 | TEDS | row | col | time (s) |
|-----:|------|---:|-----:|----:|----:|---------:|
| 1 | **camelot_auto** | 0.864 | 0.786 | 0.564 | 0.792 | 72.65 |
| 2 | **pymupdf** | 0.776 | 0.674 | 0.578 | 0.642 | 10.70 |
| 3 | **camelot_lattice_vector** | 0.766 | 0.784 | 0.748 | 0.806 | 3.50 |
| 4 | **pdfplumber** | 0.662 | 0.650 | 0.571 | 0.533 | 8.48 |
| 5 | **pdfparser** ← **ours** | 0.584 | 0.333 | 0.338 | 0.500 | 0.80 |

## pdfparser vs Camelot (headline)

| Metric | pdfparser | camelot lattice/vector | camelot auto | Δ vs lattice |
|--------|----------:|-----------------------:|-------------:|-------------:|
| f1 | 0.584 | 0.766 | 0.864 | -0.182 |
| teds | 0.333 | 0.784 | 0.786 | -0.451 |
| row | 0.338 | 0.748 | 0.564 | -0.409 |
| col | 0.500 | 0.806 | 0.792 | -0.306 |

## Improvement vs previous ICDAR run

| Metric | Previous | Now | Δ |
|--------|---------:|----:|--:|
| f1 | 0.672 | 0.584 | -0.088 |
| teds | 0.331 | 0.333 | +0.001 |
| row | 0.382 | 0.338 | -0.043 |
| col | 0.489 | 0.500 | +0.011 |

## Failure mode histogram (pdfparser)

| Mode | Docs |
|------|-----:|
| WRONG_SHAPE | 49 |
| BAD_STRUCTURE | 45 |
| ROW_MISCOUNT | 44 |
| OVER_DETECT | 39 |
| COL_MISCOUNT | 35 |
| MULTI_PAGE_DOC | 28 |
| MULTI_TABLE_PAGE | 17 |
| UNDER_DETECT | 9 |
| MISS_ALL | 3 |

### Buckets

- **miss_all:** 3
- **under:** 6
- **over:** 39
- **bad_struct_ok_count:** 15
- **good:** 2

## Multi-table vs single-table

- Multi-table docs (n=33): mean F1 us=0.678, camelot=0.631; TEDS us=0.282, camelot=0.476
- Single-table docs (n=34): mean F1 us=0.560, camelot=0.775; TEDS us=0.437, camelot=0.519

## Worst TEDS gap vs Camelot lattice (top 15)

| Doc | ΔTEDS | F1 us/c | TEDS us/c | n_gt/us/c | modes |
|------|------:|--------:|----------:|----------:|-------|
| `eu-014.pdf` | -1.000 | 0.09/1.00 | 0.000/1.000 | 1/22/1 | OVER_DETECT, ROW_MISCOUNT, WRONG_SHAPE, BAD_STRUCTURE |
| `eu-012.pdf` | -0.940 | 0.30/1.00 | 0.060/1.000 | 5/15/5 | OVER_DETECT, MULTI_TABLE_PAGE, MULTI_PAGE_DOC, ROW_MISCOUNT |
| `eu-001.pdf` | -0.876 | 0.93/1.00 | 0.124/1.000 | 7/8/7 | OVER_DETECT, MULTI_TABLE_PAGE, MULTI_PAGE_DOC, ROW_MISCOUNT |
| `eu-007.pdf` | -0.865 | 1.00/1.00 | 0.135/1.000 | 6/6/6 | MULTI_TABLE_PAGE, MULTI_PAGE_DOC, ROW_MISCOUNT, WRONG_SHAPE |
| `us-012.pdf` | -0.858 | 1.00/1.00 | 0.049/0.906 | 1/1/1 | ROW_MISCOUNT, WRONG_SHAPE, BAD_STRUCTURE |
| `us-005.pdf` | -0.800 | 0.67/1.00 | 0.000/0.800 | 1/2/1 | OVER_DETECT, BAD_STRUCTURE |
| `us-016.pdf` | -0.800 | 1.00/1.00 | 0.075/0.875 | 1/1/1 | ROW_MISCOUNT, WRONG_SHAPE, BAD_STRUCTURE |
| `us-014.pdf` | -0.736 | 0.50/1.00 | 0.040/0.776 | 2/2/2 | MULTI_PAGE_DOC, ROW_MISCOUNT, WRONG_SHAPE, BAD_STRUCTURE |
| `us-013.pdf` | -0.724 | 0.00/1.00 | 0.000/0.724 | 1/3/1 | OVER_DETECT |
| `eu-013.pdf` | -0.685 | 0.18/0.80 | 0.070/0.755 | 4/41/6 | OVER_DETECT, MULTI_PAGE_DOC, ROW_MISCOUNT, COL_MISCOUNT |
| `us-004.pdf` | -0.609 | 0.67/1.00 | 0.153/0.762 | 1/2/1 | OVER_DETECT, ROW_MISCOUNT, WRONG_SHAPE, BAD_STRUCTURE |
| `eu-015.pdf` | -0.556 | 0.89/1.00 | 0.389/0.945 | 5/4/5 | UNDER_DETECT, MULTI_TABLE_PAGE, MULTI_PAGE_DOC, ROW_MISCOUNT |
| `eu-009a.pdf` | -0.542 | 0.67/1.00 | 0.236/0.778 | 1/2/1 | OVER_DETECT, ROW_MISCOUNT, WRONG_SHAPE, BAD_STRUCTURE |
| `us-015.pdf` | -0.525 | 0.40/1.00 | 0.125/0.650 | 2/3/2 | OVER_DETECT, MULTI_PAGE_DOC, ROW_MISCOUNT, WRONG_SHAPE |
| `eu-025.pdf` | -0.522 | 1.00/1.00 | 0.478/1.000 | 5/5/5 | MULTI_TABLE_PAGE, MULTI_PAGE_DOC, ROW_MISCOUNT, WRONG_SHAPE |

## Detect OK, structure bad (F1≥0.9, TEDS&lt;0.35) — n=12

- `eu-001.pdf`: shapes us=[[[8, 4], [10, 4], [13, 4], [4, 5]], [[23, 4], [24, 4]], [[9, 4], [31, 4]]] gt=[[[8, 4], [13, 4], [10, 4]], [[24, 4], [23, 4]], [[18, 4], [9, 4]]] TEDS=0.124
- `eu-003.pdf`: shapes us=[[[3, 5], [3, 3], [10, 6]]] gt=[[[3, 3], [7, 5], [4, 6]]] TEDS=0.080
- `eu-007.pdf`: shapes us=[[[5, 4]], [[2, 7]], [[11, 3], [2, 3]], [[18, 4], [2, 4]]] gt=[[[5, 4]], [[2, 7]], [[2, 3], [11, 3]], [[2, 4], [9, 4]]] TEDS=0.135
- `eu-018.pdf`: shapes us=[[[5, 2], [3, 3]]] gt=[[[7, 13], [10, 13]]] TEDS=0.012
- `us-012.pdf`: shapes us=[[[12, 6]]] gt=[[[21, 6]]] TEDS=0.049
- `us-016.pdf`: shapes us=[[[23, 2]]] gt=[[[8, 2]]] TEDS=0.075
- `us-017.pdf`: shapes us=[[[3, 12]], [[3, 3]], [[3, 7]], [[3, 2]], [[3, 2]], [[3, 2]]] gt=[[[31, 10]], [[31, 10]], [[31, 9]], [[31, 8]], [[31, 8]], [[31, 8]]] TEDS=0.008
- `us-019.pdf`: shapes us=[[[7, 2]], [[3, 4]], [[6, 2], [6, 2]]] gt=[[[18, 2]], [[27, 11]], [[14, 5], [9, 5]]] TEDS=0.089
- `us-021.pdf`: shapes us=[[[8, 10], [5, 5]]] gt=[[[12, 8], [5, 4]]] TEDS=0.302
- `us-024.pdf`: shapes us=[[[5, 2]], [[6, 4]], [[3, 3]], [[3, 10]]] gt=[[[44, 12]], [[44, 11]], [[42, 10]], [[42, 9]]] TEDS=0.005
- `us-032.pdf`: shapes us=[[[18, 3]]] gt=[[[8, 4]]] TEDS=0.252
- `us-037.pdf`: shapes us=[[[3, 34]]] gt=[[[16, 14]]] TEDS=0.016

## MISS_ALL (n=3)

- `us-003.pdf`: gt=1 camelot=0 camelot TEDS=0.000
- `us-011a.pdf`: gt=2 camelot=2 camelot TEDS=0.025
- `us-023.pdf`: gt=1 camelot=0 camelot TEDS=0.000

## Gap analysis (where we still lack)

pdfparser F1=0.584 TEDS=0.333 row=0.338 col=0.500 vs camelot lattice F1=0.766 TEDS=0.784.

### Primary remaining gaps

1. **Structure quality (TEDS / row / col)** — Detection has improved more than content alignment. ROW_MISCOUNT=44, COL_MISCOUNT=35, WRONG_SHAPE=49, BAD_STRUCTURE=45.
2. **MISS_ALL / UNDER_DETECT** — MISS_ALL=3, UNDER_DETECT=9. Often stream-only or faint/incomplete rules where lattice CC has too few joints; Camelot raster/auto recovers some of these.
3. **MULTI_TABLE_PAGE** — 17 docs. Multi-region CC helps; residual fusion or order-mismatch vs gold still hurts F1/TEDS (order-based matching).
4. **Spans & partial rules** — High F1 / low TEDS cases usually have wrong row/col counts from extra decorative lines or missing span merge on real competition layouts.
5. **Metric sensitivity** — ICDAR matching is **page order**, not IoU. Correct tables in wrong order look like structure failures. TEDS is a difflib proxy, not tree-edit TEDS.
6. **No raster line engine** — Camelot `auto`/`combined` can find painted/faint rules; we are vector-only.

---
*Generated by `benchmark/scripts/run_icdar_competitive.py`. ICDAR files remain external; never copied into `benchmark/corpus/`.*
