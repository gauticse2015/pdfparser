# Phase 0–2 Validation & ICDAR Status Report

**Generated:** 2026-07-16 16:27 UTC  
**Binary:** `target/release/pdfparser` (product Auto = Engine V2)  
**Overall:** **ALL GATES PASS**

---

## Executive summary

Fresh re-run of measurement harnesses, hard phase gates, and external ICDAR-2013 competitive analysis.

| Phase | Result | Headline |
|------:|:------:|----------|
| **0 Measurement** | **PASS** | Discipline n=34, FP n=12, T3 n=23, baseline red on over-detect |
| **1 Over-detect / precision** | **PASS** | exact **0.971**, over **0.00**, FP **1.00**, ICDAR pred/GT **0.93** |
| **2 Completeness** | **PASS** | under **0.029**, multi exact **1.0**, ICDAR under **0.239** ≤ 0.28 |
| **3 Shape** | not started | core shape_exact **0.533**; full T3 **0.406** |
| **4 Cells** | not started | core cell F1 **0.637** held; full T3 **0.518** |

**ICDAR (external, not CI):** F1 **0.761**, TEDS **0.490**, row **0.500**, col **0.595** on 67 docs.

---

## Gate checklist (fresh run)

| Gate | Criterion | Status | Value |
|------|-----------|:------:|-------|
| G0.1 | discipline docs ≥25 | PASS | n=34 |
| G0.2 | fp_strict docs ≥12 | PASS | n=12 |
| G0.3 | T3 structure golds ≥15 | PASS | n=23 |
| G0.3b | count golds ≥25 | PASS | n=36 |
| G0.7 | baseline red on over-detect | PASS | exact=0.559 over=0.441 |
| G1.1 | exact_count_rate ≥0.88 | PASS | 0.9706 |
| G1.2 | over_doc_rate ≤0.12 | PASS | 0.0000 |
| G1.3 | pred/gt ≤1.15 | PASS | 0.9655 |
| G1.4 | fp_zero_rate ≥0.85 | PASS | 1.000 |
| G1.5 | no severe over | PASS | n=0 |
| G1.6 | core det F1 ≥0.88 | PASS | 0.9333 |
| G1.6b | core det IoU ≥0.90 | PASS | 1.0000 |
| G1.7 | core cell no-regress (freeze-0.03) | PASS | 0.6372 vs 0.6372 |
| G1.8 | nested 42 = 2 tables | PASS | pred=2 exp=2 |
| G1.10 | ICDAR pred/GT ≤1.50 | PASS | 0.930 |
| G1.11 | ICDAR over_doc ≤0.35 | PASS | 0.239 |
| G2.2 | under_doc_rate ≤0.08 | PASS | 0.0294 |
| G2.3 | multi exact ≥0.85 | PASS | 1.000 n=7 |
| G2.structure | T3 ≥20 | PASS | n=23 |
| G2.7 | ICDAR under ≤0.28 | PASS | 0.239 |

**Result: PASS — all listed gates green**

---

## Phase 0 — Measurement foundation

| Check | Status | Detail |
|-------|:------:|--------|
| detect_discipline manifest | PASS | n=34 docs, failure_families tagged |
| fp_strict manifest | PASS | n=12 zero-table / form docs |
| structure T3 golds | PASS | n=23 (≥15 required, ≥20 preferred for Phase 2) |
| count golds | PASS | n=36 under `real_track/gold/count/` |
| runners | PASS | `run_detect_discipline.py`, `run_fp_strict.py`, `run_real_structure.py`, `check_phase_gates.py` |
| freezes | PASS | `baseline_pre_v3.json`, `phase1_detect_precision.json`, `g2.json` |
| ICDAR not in CI | PASS | external-only; gates optional via `--with-icdar` |

### Baseline (pre-V3) vs now — detect discipline

| Metric | Baseline pre-V3 | Current | Δ |
|--------|----------------:|--------:|--:|
| exact_count_rate | 0.559 | 0.971 | +0.412 |
| over_doc_rate | 0.441 | 0.000 | -0.441 |
| under_doc_rate | 0.000 | 0.029 | +0.029 |
| pred/gt | 1.880 | 0.966 | -0.914 |
| severe over (Δ≥3) | 2 | 0 | -2 |

Interpretation: over-detect was the baseline failure mode (exact 0.56, over 0.44, pred/gt 1.88). Current discipline is precision-dominant with a single under-detect residual.

---

## Phase 1 — Over-detect / precision

### Detect discipline (n=34)

| Metric | Threshold | Current |
|--------|----------:|--------:|
| exact_count_rate | ≥ 0.88 | **0.9706** |
| over_doc_rate | ≤ 0.12 | **0.0000** |
| under_doc_rate | (info) | 0.0294 |
| pred/gt | ≤ 1.15 | **0.9655** |
| n_exact / n_over / n_under | — | 33 / 0 / 1 |
| severe over | 0 | **0** |

- **Under residual:** [('R010_bea_nipa_highlights', 0, 1)]
- **Over residual:** none

### FP strict (n=12)

- zero_rate = **1.000** (threshold ≥ 0.85)
- pass = 12/12 (IRS forms, NIST notice, arxiv, beigebook, Schedule C/D)

### Real structure — core freeze (n=15 from `g2.json`)

| Metric | Freeze g2 | Current core | Gate |
|--------|----------:|-------------:|------|
| micro cell F1 | 0.6372 | **0.6372** | no-regress ≥ freeze−0.03 → PASS |
| micro det count F1 | 0.9644 | **0.9333** | ≥ 0.88 → PASS |
| shape exact rate | — | **0.5333** | Phase 3 target |
| nested 42 | 2 tables | **2** | PASS |

### Highlight recoveries (core / struggle)

| Doc | det F1 | cell F1 | shape | Notes |
|-----|-------:|--------:|------:|-------|
| `45_real_campaign_donors` | 1.00 | 0.943 | 1.00 | pred=1 exp=1 |
| `37_real_liabilities_superscript` | 1.00 | 0.247 | 0.00 | pred=1 exp=1 |
| `42_real_insurance_italian` | 1.00 | 0.950 | 1.00 | pred=2 exp=2 |
| `32_real_census_table324` | 1.00 | 0.000 | 0.00 | pred=2 exp=2 |
| `30_real_ca_warn_report` | 1.00 | 0.839 | 1.00 | pred=1 exp=1 |

---

## Phase 2 — Detection completeness

| Metric | Threshold | Current |
|--------|----------:|--------:|
| GATE-1 still green | required | **PASS** |
| under_doc_rate | ≤ 0.08 | **0.0294** |
| multi_table exact | ≥ 0.85 | **1.000** (n=7) |
| T3 structure golds | ≥ 20 | **23** |
| ICDAR under_doc_rate | ≤ 0.28 | **0.239** |

### Architecture notes that landed

1. **Ruled-owns-page** exclusivity for solid lattice (precision).
2. **Dual-mode borderless:** recall when no solid lattice; multitable stream recovery beside lattice.
3. **Dense multi-col stream exemption** in form disc + borderless recall (donors 56×7, liabilities detection).
4. **IRS keyword veto** kept FP-strict at 1.0 (Schedule C / OMB not reopened).
5. Hybrid over-wide densify does not kill stream (lattice-only exclusivity).

### Expanded T3 (Phase 3 tracking)

| Slice | n | cell F1 | det F1 | shape exact |
|-------|--:|--------:|-------:|------------:|
| Core freeze (g2) | 15 | 0.637 | 0.933 | 0.533 |
| Expansion only | 8 | 0.293 | 0.875 | 0.167 |
| Full T3 | 23 | 0.518 | 0.913 | 0.406 |

Expansion docs (lower cell/shape by design — peer golds for Phase 3 work):

| Doc | det | cell | shape |
|-----|----:|-----:|------:|
| `R005_census_acs_sample` | 0.00 | 0.000 | 0.00 |
| `R008_cdc_mmwr_sample` | 1.00 | 0.000 | 0.00 |
| `R009_sec_10q_sample` | 1.00 | 0.000 | 0.33 |
| `R016_tabula_12s0324` | 1.00 | 0.364 | 0.00 |
| `R017_tabula_argentina` | 1.00 | 0.597 | 0.00 |
| `R019_camelot_foo` | 1.00 | 0.716 | 1.00 |
| `R021_camelot_left_twocol` | 1.00 | 0.421 | 0.00 |
| `R022_camelot_superscript` | 1.00 | 0.247 | 0.00 |

GATE-1 cell/det no-regress **only scores core freeze ids** so expansion does not dilute Phase 1.

---

## ICDAR-2013 competitive analysis

**Policy:** external measurement only — not used for CI tuning loops.

**Dataset:** 67 docs (`competition-dataset-eu` + `competition-dataset-us`)  
**Runner:** `benchmark/scripts/run_icdar_competitive.py --libs pdfparser`  
**Artifacts:** `benchmark/results/camelot_icdar_headtohead.json`, `icdar_failure_analysis.json`, `docs/icdar-competitive-report.md`

### Headline metrics

| Metric | Pre-prodready snapshot | Phase-1 freeze | **Current** |
|--------|-----------------------:|---------------:|------------:|
| F1 | 0.457 | 0.762 | **0.761** |
| TEDS | 0.321 | 0.542 | **0.490** |
| row | 0.336 | 0.548 | **0.500** |
| col | 0.459 | 0.663 | **0.595** |
| time (s) | — | — | 26.3 |
| errors | — | — | 0 |

### Count discipline on ICDAR

| Metric | Phase-1 freeze | **Current** | Gate |
|--------|---------------:|------------:|------|
| pred/GT | 0.728 | **0.930** | ≤ 1.50 PASS |
| over_doc_rate | 0.119 | **0.239** | ≤ 0.35 PASS |
| under_doc_rate | 0.328 | **0.239** | ≤ 0.28 PASS (Phase 2) |
| exact count docs | — | **35/67** | — |
| miss_all | — | **9** | — |

**Reading:** vs Phase-1 freeze, under-detect improved substantially (0.33 → **0.24**) while F1 held ~**0.76**. TEDS drifted slightly lower (0.54 → **0.49**) — structure/shape work remains Phase 3–4. Over-doc rate rose vs Phase-1 freeze (0.12 → 0.24) but stays under the Phase-1 gate (0.35).

### Failure mode histogram

| Mode | Docs | Interpretation |
|------|-----:|----------------|
| WRONG_SHAPE | 43 | row/col schema mismatch (Phase 3) |
| ROW_MISCOUNT | 41 | row fragmentation / merge (Phase 3) |
| BAD_STRUCTURE | 36 | cell content alignment fails (Phase 4) |
| COL_MISCOUNT | 29 | column split/collapse (Phase 3) |
| MULTI_PAGE_DOC | 28 | multipage stitch off by design in some evals |
| MULTI_TABLE_PAGE | 17 | multi-region ownership |
| OVER_DETECT | 16 | extra tables (precision residual) |
| UNDER_DETECT | 16 | missed tables (completeness residual) |
| MISS_ALL | 9 | zero tables on non-empty GT page |

### Buckets

- **miss_all:** 9
- **under:** 7
- **over:** 16
- **bad_struct_ok_count:** 15
- **good:** 17

### Worst under-detect (by GT − pred)

| Doc | pred | GT | Modes |
|------|-----:|---:|-------|
| `us-017.pdf` | 1 | 6 | UNDER_DETECT, MULTI_PAGE_DOC, ROW_MISCOUNT, COL_MISCOUNT, WRONG_SHAPE, BAD_STRUCTURE |
| `us-035a.pdf` | 0 | 5 | MISS_ALL, UNDER_DETECT, MULTI_TABLE_PAGE, MULTI_PAGE_DOC |
| `us-024.pdf` | 0 | 4 | MISS_ALL, UNDER_DETECT, MULTI_PAGE_DOC |
| `us-025.pdf` | 2 | 6 | UNDER_DETECT, MULTI_TABLE_PAGE, MULTI_PAGE_DOC, ROW_MISCOUNT, COL_MISCOUNT, WRONG_SHAPE, BAD_STRUCTURE |
| `eu-015.pdf` | 2 | 5 | UNDER_DETECT, MULTI_TABLE_PAGE, MULTI_PAGE_DOC, ROW_MISCOUNT, WRONG_SHAPE, BAD_STRUCTURE |
| `us-011a.pdf` | 0 | 2 | MISS_ALL, UNDER_DETECT, MULTI_PAGE_DOC |
| `us-015.pdf` | 0 | 2 | MISS_ALL, UNDER_DETECT, MULTI_PAGE_DOC |
| `us-018.pdf` | 5 | 7 | UNDER_DETECT, MULTI_PAGE_DOC, ROW_MISCOUNT, COL_MISCOUNT, WRONG_SHAPE, BAD_STRUCTURE |
| `us-020.pdf` | 2 | 4 | UNDER_DETECT, MULTI_PAGE_DOC, ROW_MISCOUNT, WRONG_SHAPE, BAD_STRUCTURE |
| `us-034.pdf` | 0 | 2 | MISS_ALL, UNDER_DETECT, MULTI_TABLE_PAGE |
| `eu-014.pdf` | 0 | 1 | MISS_ALL, UNDER_DETECT |
| `eu-018.pdf` | 1 | 2 | UNDER_DETECT, MULTI_TABLE_PAGE, ROW_MISCOUNT, COL_MISCOUNT, WRONG_SHAPE, BAD_STRUCTURE |

### Worst over-detect (by pred − GT)

| Doc | pred | GT | Modes |
|------|-----:|---:|-------|
| `eu-026.pdf` | 9 | 3 | OVER_DETECT, MULTI_PAGE_DOC, ROW_MISCOUNT, COL_MISCOUNT, WRONG_SHAPE, BAD_STRUCTURE |
| `us-040.pdf` | 5 | 1 | OVER_DETECT, ROW_MISCOUNT, COL_MISCOUNT, WRONG_SHAPE, BAD_STRUCTURE |
| `eu-027.pdf` | 3 | 1 | OVER_DETECT, ROW_MISCOUNT, WRONG_SHAPE |
| `us-002.pdf` | 4 | 2 | OVER_DETECT, MULTI_PAGE_DOC, ROW_MISCOUNT, COL_MISCOUNT, WRONG_SHAPE, BAD_STRUCTURE |
| `eu-004.pdf` | 13 | 12 | OVER_DETECT, MULTI_TABLE_PAGE, MULTI_PAGE_DOC, ROW_MISCOUNT, COL_MISCOUNT, WRONG_SHAPE, BAD_STRUCTURE |
| `eu-011.pdf` | 2 | 1 | OVER_DETECT, ROW_MISCOUNT, WRONG_SHAPE |
| `eu-012.pdf` | 6 | 5 | OVER_DETECT, MULTI_TABLE_PAGE, MULTI_PAGE_DOC, ROW_MISCOUNT, COL_MISCOUNT, WRONG_SHAPE, BAD_STRUCTURE |
| `eu-013.pdf` | 5 | 4 | OVER_DETECT, MULTI_PAGE_DOC, ROW_MISCOUNT, COL_MISCOUNT, WRONG_SHAPE, BAD_STRUCTURE |
| `eu-019.pdf` | 2 | 1 | OVER_DETECT, ROW_MISCOUNT, WRONG_SHAPE |
| `us-001.pdf` | 3 | 2 | OVER_DETECT, MULTI_PAGE_DOC, COL_MISCOUNT, WRONG_SHAPE, BAD_STRUCTURE |

### Detect OK, structure weak (F1≥0.9, TEDS&lt;0.35)

These dominate the TEDS gap and are **Phase 3/4** work, not detection:

n=8
- `eu-003.pdf`: F1=1.00 TEDS=0.238
- `us-003.pdf`: F1=1.00 TEDS=0.043
- `us-004.pdf`: F1=1.00 TEDS=0.210
- `us-012.pdf`: F1=1.00 TEDS=0.095
- `us-014.pdf`: F1=1.00 TEDS=0.108
- `us-021.pdf`: F1=1.00 TEDS=0.254
- `us-026.pdf`: F1=1.00 TEDS=0.138
- `us-032.pdf`: F1=1.00 TEDS=0.252

---

## Full real_structure board (T3 n=23)

| Doc | det F1 | cell F1 | shape | pred/exp |
|-----|-------:|--------:|------:|---------:|
| `30_real_ca_warn_report` **core** | 1.00 | 0.839 | 1.00 | 1/1 |
| `33_real_argentina_votes` **core** | 1.00 | 0.597 | 0.00 | 1/1 |
| `34_real_schools_contributions` **core** | 1.00 | 0.460 | 1.00 | 1/1 |
| `35_real_camelot_fuel` **core** | 1.00 | 0.716 | 1.00 | 1/1 |
| `36_real_two_tables` **core** | 1.00 | 0.421 | 0.00 | 2/2 |
| `37_real_liabilities_superscript` **core** | 1.00 | 0.247 | 0.00 | 1/1 |
| `31_real_background_checks` **core** | 1.00 | 0.821 | 0.00 | 1/1 |
| `32_real_census_table324` **core** | 1.00 | 0.000 | 0.00 | 2/2 |
| `R018_camelot_health` **core** | 1.00 | 0.995 | 1.00 | 1/1 |
| `R003_bea_gdp_sample` **core** | 1.00 | 0.868 | 0.00 | 1/1 |
| `R010_bea_nipa_highlights` **core** | 0.00 | 0.000 | 0.00 | 0/1 |
| `42_real_insurance_italian` **core** | 1.00 | 0.950 | 1.00 | 2/2 |
| `43_real_row_span_gmc` **core** | 1.00 | 0.995 | 1.00 | 1/1 |
| `44_real_spanning_cells` **core** | 1.00 | 0.706 | 1.00 | 2/2 |
| `45_real_campaign_donors` **core** | 1.00 | 0.943 | 1.00 | 1/1 |
| `R005_census_acs_sample` | 0.00 | 0.000 | 0.00 | 0/1 |
| `R008_cdc_mmwr_sample` | 1.00 | 0.000 | 0.00 | 1/1 |
| `R009_sec_10q_sample` | 1.00 | 0.000 | 0.33 | 3/3 |
| `R016_tabula_12s0324` | 1.00 | 0.364 | 0.00 | 2/2 |
| `R017_tabula_argentina` | 1.00 | 0.597 | 0.00 | 1/1 |
| `R019_camelot_foo` | 1.00 | 0.716 | 1.00 | 1/1 |
| `R021_camelot_left_twocol` | 1.00 | 0.421 | 0.00 | 2/2 |
| `R022_camelot_superscript` | 1.00 | 0.247 | 0.00 | 1/1 |

---

## Residual risks & Phase 3 entry criteria

### Still weak (do not block Phase 2)

1. **R010_bea_nipa_highlights** — only discipline under-detect (0 vs 1); multipage stream fragments exist but gold-page match fails.
2. **R005 / R008 expansion golds** — detection or cell alignment weak; peer golds for tracking.
3. **ICDAR multipage miss_all** — us-024, us-035a, us-011a, us-015, us-016, us-034, eu-014, …
4. **ICDAR over clusters** — eu-026 (9 vs 3), us-040 (5 vs 1).
5. **Shape/cell** — WRONG_SHAPE 43/67, ROW_MISCOUNT 41/67; core shape_exact still **0.53**.

### Safe to start Phase 3 when

- [x] GATE-0 PASS
- [x] GATE-1 PASS (incl. ICDAR G1.10–G1.11)
- [x] GATE-2 PASS (incl. ICDAR under ≤0.28)
- [x] Core cell F1 no-regress vs g2
- [x] FP strict ≥ 0.85 (currently 1.0)
- [ ] Phase 3 plan: shape_exact / row-col accuracy on T3 without reopening form/borderless FP soup

---

## How to re-verify

```bash
cargo build --release -p pdfparser-cli
python3 benchmark/scripts/check_phase_gates.py --phase 0
python3 benchmark/scripts/run_detect_discipline.py
python3 benchmark/scripts/run_fp_strict.py
python3 benchmark/scripts/run_real_structure.py --preset auto
python3 benchmark/scripts/check_phase_gates.py --phase 1 --with-icdar /tmp
python3 benchmark/scripts/check_phase_gates.py --phase 2 --with-icdar /tmp
python3 benchmark/scripts/run_icdar_competitive.py \
  --data-dir /tmp/camelot-upstream/tests/files/tabula/icdar2013-dataset \
  --libs pdfparser
```

---

## Artifact index

| Artifact | Path |
|----------|------|
| Discipline latest | `benchmark/real_track/results/detect_discipline_latest.json` |
| FP strict latest | `benchmark/real_track/results/real_fp_strict_latest.json` |
| Structure latest | `benchmark/real_track/results/real_structure_latest.json` |
| ICDAR head-to-head | `benchmark/results/camelot_icdar_headtohead.json` |
| ICDAR failure analysis | `benchmark/results/icdar_failure_analysis.json` |
| ICDAR markdown report | `docs/icdar-competitive-report.md` |
| This validation report | `docs/phase0-2-validation-report.md` |
| Autonomous progress | `docs/AUTONOMOUS_PROGRESS.md` |
| Gate checker | `benchmark/scripts/check_phase_gates.py` |

