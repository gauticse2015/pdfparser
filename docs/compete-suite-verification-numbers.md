# Compete suite verification numbers (ran live)

**Date:** 2026-07-11  
**Binary:** `target/release/pdfparser`  
**Harness:** `run_accuracy_benchmark.py` + extract probe + Camelot/pdfplumber peers

---

## Executive answer

| Corpus | Accuracy scoreable? | Reproduces issues? | Headline metrics |
|--------|--------------------:|--------------------|------------------|
| Hard synthetic C100–C180 | **Yes** (full grids) | **Yes** — 87.7% imperfect | cellF1 **0.383**, shape **0.204**, row **0.426**, col **0.596** |
| Coverage C001–C068 | Yes | Only underlines | cellF1 **0.938** (too easy) |
| Downloaded reals R001–R020 | **No cell/row/col** (soft gold only) | **Mostly yes via probe** | shapes vs peers; 1 parse fail |

**Honest gap:** reals were downloaded with soft gold stubs. The accuracy suite **cannot** emit TEDS/row/col/cell F1 without `expected_tables`. What we *did* run is full extract inventory + peer shape comparison — enough to see struggle patterns, not enough for a formal F1 scoreboard.

---

## A) Hard synthetic (full grid gold) — accuracy benchmark

```
suite = regression_compete_hard
n = 81
imperfect = 71 / 81 (87.7%)
detect_F1 = 0.795
row_accuracy = 0.426
col_accuracy = 0.596
shape_exact = 0.204
cell_F1 (TEDS-like) = 0.383
overall_score = 50.4
```

### By struggle mode

| Mode | n | fail% | detF1 | row | col | shape | cellF1 |
|------|--:|------:|------:|----:|----:|------:|-------:|
| image_painted_miss_all | 8 | 100% | 0.00 | 0.00 | 0.00 | 0.00 | **0.00** |
| partial_v_col_undercount | 18 | 100% | 1.00 | 1.00 | 0.00 | 0.00 | **0.00** |
| false_underline_row_overcount | 8 | 100% | 1.00 | 0.00 | 1.00 | 0.00 | **0.20** |
| multipage_underline_overcount | 4 | 100% | 1.00 | 0.00 | 1.00 | 0.00 | **0.24** |
| header_slice_fragmentation | 8 | 100% | 0.44 | 0.00 | 0.62 | 0.00 | **0.32** |
| complex_spans | 4 | 75% | 0.83 | 0.25 | 0.50 | 0.25 | 0.47 |
| sparse_densify_row_undercount | 8 | 100% | 1.00 | 0.00 | 1.00 | 0.00 | 0.49 |
| mixed_stream_miss | 6 | 83% | 0.83 | 0.58 | 0.83 | 0.58 | 0.61 |
| multi_table_close_chrome | 5 | 20% | 0.88 | 0.80 | 0.85 | 0.80 | 0.89 |
| invoice_fees_inside_grid | 4 | 100% | 1.00 | 0.00 | 1.00 | 0.00 | 1.00* |
| severe_minigrid_overdetect | 4 | 100% | 0.53 | 1.00 | 1.00 | 1.00 | 1.00* |
| cell_content_assign | 4 | 0% | 1.00 | 1.00 | 1.00 | 1.00 | 1.00 |

\* cell F1 high but detect or row still fails (fees add rows; minigrids over-detect).

Artifacts: `benchmark/results/accuracy_results_compete_hard.json`, `docs/accuracy-scoreboard-compete-hard.md`

---

## B) Coverage synthetic C001–C068 — accuracy benchmark

```
n = 65
detect_F1 = 1.000
row = 0.938  col = 1.000
shape = 0.938
cell_F1 = 0.938
overall = 95.9
```

**Only fails:** C043–C046 false underlines (cellF1=0, row=0).  
This wave is **not** the struggle suite — kept as non-regression of easy modes.

Artifact: `benchmark/results/accuracy_results_compete_coverage.json`

---

## C) Downloaded real PDFs — extract + peer probe (not full accuracy F1)

Soft gold → accuracy harness skips table structure metrics. Live extract:

| Doc | pages | us n | us max shape | Camelot L n | Camelot S n | Plumber n | Issue signal |
|-----|------:|-----:|-------------:|------------:|------------:|----------:|--------------|
| R001 Schedule C | 2 | 3 | 16×4 | 5 | 4 | 3 | form chrome |
| R002 f1040 | 2 | 3 | 13×19 | 4 | 8 | 15 | form chrome / wide stream |
| R003 BEA SNTables | — | **ERR** | — | — | — | — | **parse fail** |
| R005 Census ACS | 168 | 8 | 5×8 | 6 | 186 | 0 | multipage stream fragment |
| R006 Beige Book | 54 | 8 | 7×5 | 0 | 54 | 1 | stream soup / sparse |
| R007 NIST FIPS | 51 | 8 | 16×5 | **83** | 52 | 35 | under-detect vs lattice |
| R008 CDC MMWR | 6 | 8 | 11×**67** | 0 | 11 | 0 | col skew / hybrid mess |
| R009 SEC 10-Q form | 10 | 2 | 8×7 | 2 | 14 | 2 | form under-detect stream |
| R010 BEA GDP | 18 | 5 | 43×8 | **12** | 20 | 13 | under-detect; peers find wide tables |
| R016 Tabula 12s0324 | 1 | 2 | 35×13 + 35×2 | 2 | 2 | 2 | structure disagree (stream vs lattice) |
| R017 Argentina votes | 1 | 2 | 4×12 + 33×4 | **1** (32×4) | 3 | 1 | **over-detect** vs camL/plumber |
| R018 Camelot health | 1 | 1 | **50×8** | 1 (50×8) | 1 | 1 | **OK shape match** lattice |
| R019 Camelot foo | 1 | 1 | **7×7** | 1 (7×7) | 1 | 1 | **OK shape match** lattice |
| R020 Schedule D | 2 | 2 | 5×3 | 3 | 4 | 3 | form under-count vs peers |

### Real docs that clearly reproduce struggle

1. **R016** — us: stream 33×13 + broken lattice 35×2; Camelot stream 41×10 / 39×6 — classic hard statistical table, structure not agreed.
2. **R017** — us emits **2** tables; Camelot lattice + plumber emit **1×32×4** — over-detect / wrong stream slice.
3. **R008** — hybrid 4×67 and 6×43 — pathological col inflation.
4. **R010** — us 5 tables max 43×8; Camelot lattice 12 with 22-col shapes; plumber 13 with 46×22 — under-detect + missing wide stats.
5. **R005/R006** — multipage stream fragmentation (small max rows on long docs).
6. **R003** — does not parse (broken download / stream syntax).
7. **R001/R002/R020** — form lattice/stream mix; peers disagree on count (over/under).

### Real docs that do **not** currently struggle (shape-level)

- **R018 health** — 50×8 lattice matches Camelot lattice / plumber ≈51×8  
- **R019 foo** — 7×7 matches Camelot lattice / plumber  

These are still useful as **regression anchors** on real PDFs, not open struggles.

Artifacts:  
`benchmark/results/compete_real_pdfparser_probe.json`  
`benchmark/results/compete_real_peers_probe.json`

---

## Comparison to ICDAR external (context)

| Metric | Hard owned suite | ICDAR (external) |
|--------|-----------------:|-----------------:|
| cellF1 / TEDS | 0.38 | ~0.34 |
| row | 0.43 | ~0.35 |
| col | 0.60 | ~0.40 |
| detect F1 | 0.80 | ~0.65 |

Hard synthetic is in the right difficulty band. Reals show wild-document signals but need **manual/peer-derived grid gold** before they contribute to cellF1/row/col scoreboard.

---

## Next if we want reals on the formal scoreboard

1. Drop or replace R003 (parse fail).  
2. Label `expected_table_count` + shapes for R016/R017/R010/R008 (highest value).  
3. Optionally promote peer-agreed grids (R018/R019) as soft regression gold.  
4. Re-run `run_accuracy_benchmark.py --suite regression_compete --tier compete_real`.
