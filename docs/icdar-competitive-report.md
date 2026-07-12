# ICDAR-2013 Competitive Analysis (external)

**Policy:** ICDAR is **not** part of the regression corpus. This report is for competitive measurement only.

**Docs:** 67 · **Gold:** ICDAR `*-str.xml` · **Metrics:** Camelot `bench/_metrics.score` (F1, TEDS proxy, row/col)

## Leaderboard

| Rank | Tool | F1 | TEDS | row | col | time (s) |
|-----:|------|---:|-----:|----:|----:|---------:|
| 1 | **camelot_auto** | 0.864 | 0.786 | 0.564 | 0.792 | 73.36 |
| 2 | **pymupdf** | 0.776 | 0.674 | 0.578 | 0.642 | 10.73 |
| 3 | **camelot_lattice_vector** | 0.766 | 0.784 | 0.748 | 0.806 | 3.57 |
| 4 | **pdfplumber** | 0.662 | 0.650 | 0.571 | 0.533 | 8.54 |
| 5 | **pdfparser** ← **ours** | 0.495 | 0.322 | 0.329 | 0.452 | 28.13 |

## pdfparser vs Camelot (headline)

| Metric | pdfparser | camelot lattice/vector | camelot auto | Δ vs lattice |
|--------|----------:|-----------------------:|-------------:|-------------:|
| f1 | 0.495 | 0.766 | 0.864 | -0.271 |
| teds | 0.322 | 0.784 | 0.786 | -0.462 |
| row | 0.329 | 0.748 | 0.564 | -0.419 |
| col | 0.452 | 0.806 | 0.792 | -0.354 |

## Improvement vs previous ICDAR run

| Metric | Previous | Now | Δ |
|--------|---------:|----:|--:|
| f1 | 0.495 | 0.495 | +0.000 |
| teds | 0.321 | 0.322 | +0.000 |
| row | 0.329 | 0.329 | +0.000 |
| col | 0.452 | 0.452 | +0.000 |

## Failure mode histogram (pdfparser)

| Mode | Docs |
|------|-----:|
| WRONG_SHAPE | 56 |
| ROW_MISCOUNT | 54 |
| OVER_DETECT | 52 |
| BAD_STRUCTURE | 51 |
| COL_MISCOUNT | 42 |
| MULTI_PAGE_DOC | 28 |
| MULTI_TABLE_PAGE | 17 |
| UNDER_DETECT | 3 |

### Buckets

- **miss_all:** 0
- **under:** 3
- **over:** 52
- **bad_struct_ok_count:** 9
- **good:** 2

## Multi-table vs single-table

- Multi-table docs (n=33): mean F1 us=0.618, camelot=0.631; TEDS us=0.266, camelot=0.476
- Single-table docs (n=34): mean F1 us=0.466, camelot=0.775; TEDS us=0.368, camelot=0.519

## Worst TEDS gap vs Camelot lattice (top 15)

| Doc | ΔTEDS | F1 us/c | TEDS us/c | n_gt/us/c | modes |
|------|------:|--------:|----------:|----------:|-------|
| `eu-014.pdf` | -0.897 | 0.09/1.00 | 0.103/1.000 | 1/21/1 | OVER_DETECT, ROW_MISCOUNT, COL_MISCOUNT, WRONG_SHAPE |
| `us-012.pdf` | -0.811 | 1.00/1.00 | 0.095/0.906 | 1/1/1 | ROW_MISCOUNT, WRONG_SHAPE, BAD_STRUCTURE |
| `us-005.pdf` | -0.800 | 0.40/1.00 | 0.000/0.800 | 1/4/1 | OVER_DETECT, ROW_MISCOUNT, COL_MISCOUNT, WRONG_SHAPE |
| `us-016.pdf` | -0.795 | 0.22/1.00 | 0.080/0.875 | 1/8/1 | OVER_DETECT, ROW_MISCOUNT, WRONG_SHAPE, BAD_STRUCTURE |
| `us-014.pdf` | -0.776 | 0.44/1.00 | 0.000/0.776 | 2/7/2 | OVER_DETECT, MULTI_PAGE_DOC, ROW_MISCOUNT, COL_MISCOUNT |
| `eu-015.pdf` | -0.762 | 1.00/1.00 | 0.183/0.945 | 5/5/5 | MULTI_TABLE_PAGE, MULTI_PAGE_DOC, ROW_MISCOUNT, COL_MISCOUNT |
| `us-004.pdf` | -0.754 | 0.33/1.00 | 0.008/0.762 | 1/5/1 | OVER_DETECT, ROW_MISCOUNT, COL_MISCOUNT, WRONG_SHAPE |
| `us-029.pdf` | -0.715 | 0.22/1.00 | 0.053/0.769 | 1/8/1 | OVER_DETECT, ROW_MISCOUNT, COL_MISCOUNT, WRONG_SHAPE |
| `eu-013.pdf` | -0.710 | 0.17/0.80 | 0.045/0.755 | 4/44/6 | OVER_DETECT, MULTI_PAGE_DOC, ROW_MISCOUNT, COL_MISCOUNT |
| `us-027.pdf` | -0.690 | 0.20/1.00 | 0.082/0.772 | 2/18/2 | OVER_DETECT, MULTI_PAGE_DOC, ROW_MISCOUNT, COL_MISCOUNT |
| `us-013.pdf` | -0.677 | 0.25/1.00 | 0.047/0.724 | 1/7/1 | OVER_DETECT, ROW_MISCOUNT, COL_MISCOUNT, WRONG_SHAPE |
| `us-030.pdf` | -0.672 | 0.29/1.00 | 0.139/0.811 | 1/6/1 | OVER_DETECT, ROW_MISCOUNT, COL_MISCOUNT, WRONG_SHAPE |
| `eu-009a.pdf` | -0.661 | 0.67/1.00 | 0.117/0.778 | 1/2/1 | OVER_DETECT, ROW_MISCOUNT, WRONG_SHAPE, BAD_STRUCTURE |
| `eu-012.pdf` | -0.660 | 0.42/1.00 | 0.340/1.000 | 5/19/5 | OVER_DETECT, MULTI_TABLE_PAGE, MULTI_PAGE_DOC, ROW_MISCOUNT |
| `us-015.pdf` | -0.650 | 0.29/1.00 | 0.000/0.650 | 2/5/2 | OVER_DETECT, MULTI_PAGE_DOC, COL_MISCOUNT, WRONG_SHAPE |

## Detect OK, structure bad (F1≥0.9, TEDS&lt;0.35) — n=7

- `eu-003.pdf`: shapes us=[[[6, 3], [6, 5], [26, 6]]] gt=[[[3, 3], [7, 5], [4, 6]]] TEDS=0.238
- `eu-015.pdf`: shapes us=[[[16, 14], [7, 2]], [[11, 7], [4, 2], [6, 4]]] gt=[[[12, 2], [7, 2]], [[32, 2], [33, 2], [33, 2]]] TEDS=0.183
- `eu-026.pdf`: shapes us=[[[11, 16]], [[8, 11]], [[3, 5]]] gt=[[[5, 5]], [[5, 4]], [[5, 4]]] TEDS=0.100
- `us-011a.pdf`: shapes us=[[[3, 2]], [[3, 2]]] gt=[[[13, 2]], [[7, 2]]] TEDS=0.016
- `us-012.pdf`: shapes us=[[[23, 6]]] gt=[[[21, 6]]] TEDS=0.095
- `us-017.pdf`: shapes us=[[[9, 2]], [[3, 2]], [[3, 5]], [[3, 5]], [[3, 5]]] gt=[[[31, 10]], [[31, 10]], [[31, 9]], [[31, 8]], [[31, 8]], [[31, 8]]] TEDS=0.008
- `us-032.pdf`: shapes us=[[[18, 3]]] gt=[[[8, 4]]] TEDS=0.252

## MISS_ALL (n=0)


## Gap analysis (where we still lack)

pdfparser F1=0.495 TEDS=0.322 row=0.329 col=0.452 vs camelot lattice F1=0.766 TEDS=0.784.

### Primary remaining gaps

1. **Structure quality (TEDS / row / col)** — Detection has improved more than content alignment. ROW_MISCOUNT=54, COL_MISCOUNT=42, WRONG_SHAPE=56, BAD_STRUCTURE=51.
2. **MISS_ALL / UNDER_DETECT** — MISS_ALL=0, UNDER_DETECT=3. Often stream-only or faint/incomplete rules where lattice CC has too few joints; Camelot raster/auto recovers some of these.
3. **MULTI_TABLE_PAGE** — 17 docs. Multi-region CC helps; residual fusion or order-mismatch vs gold still hurts F1/TEDS (order-based matching).
4. **Spans & partial rules** — High F1 / low TEDS cases usually have wrong row/col counts from extra decorative lines or missing span merge on real competition layouts.
5. **Metric sensitivity** — ICDAR matching is **page order**, not IoU. Correct tables in wrong order look like structure failures. TEDS is a difflib proxy, not tree-edit TEDS.
6. **No raster line engine** — Camelot `auto`/`combined` can find painted/faint rules; we are vector-only.

---
*Generated by `benchmark/scripts/run_icdar_competitive.py`. ICDAR files remain external; never copied into `benchmark/corpus/`.*
