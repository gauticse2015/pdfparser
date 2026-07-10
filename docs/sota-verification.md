# SOTA verification — unbiased harness audit

**Date:** 2026-07-10 · **Fresh multi-library run** after Phase O  
**Command:** `python benchmark/scripts/run_accuracy_benchmark.py`  
**Libraries:** pdfparser, pymupdf, pdfplumber, pypdf, pypdfium2, pdfminer.six  
**Docs:** 33 (basic + stress + real) · **shared** `ground_truth/*.json` + `metrics.py`

## Claim (precise)

On **this born-digital (native) PDF accuracy harness**, **pdfparser leads overall mean score** and **grid-gold table cell F1**, with text and objects at parity with the best competitors — **without moving competitor scores**.

This is **not** a claim of universal SOTA on all PDFs forever (OCR scans, encryption, every enterprise corpus).

---

## 1. Competitor stability (suite not gamed)

Historical baseline from Phase U era (same gold schema) vs **this run**:

| Library | hist overall | now overall | Δ | hist grid cell F1 | now | Δ |
|---------|-------------:|------------:|--:|------------------:|----:|--:|
| pymupdf | 84.481 | 84.481 | **0** | 0.774 | 0.774 | **0** |
| pdfplumber | 82.980 | 82.980 | **0** | 0.826 | 0.826 | **0** |
| pypdf | 56.515 | 56.515 | **0** | 0.000 | 0.000 | **0** |
| pypdfium2 | 53.194 | 53.194 | **0** | 0.000 | 0.000 | **0** |
| pdfminer.six | 53.097 | 53.097 | **0** | 0.000 | 0.000 | **0** |

**Interpretation:** Gold + metrics + competitor adapters are stable. Our gains come from **pdfparser improving**, not from rewriting gold to hurt others.

---

## 2. Fresh leaderboard

| Library | overall | text F1 | CER↓ | table | grid cell F1 | objects | ms |
|---------|--------:|--------:|-----:|------:|-------------:|--------:|---:|
| **pdfparser** | **93.07** | **1.000** | **0.000** | **75.85** | **0.990** | **100** | 35 |
| pymupdf | 84.48 | 0.992 | 0.000 | 69.70 | 0.774 | 100 | 391 |
| pdfplumber | 82.98 | 0.992 | 0.267 | 67.49 | 0.826 | 97.5 | 304 |
| pypdf | 56.52 | 1.000 | 0.000 | 39.39 | 0.000 | 100 | 69 |
| pypdfium2 | 53.19 | 0.992 | 0.021 | 39.39 | 0.000 | 0 | 10 |
| pdfminer.six | 53.10 | 0.984 | 0.279 | 39.39 | 0.000 | 97.5 | 366 |

**Grid-gold (11 synthetic full cell grids)** — fairest table quality slice:

| Library | detect F1 | shape exact | **cell F1** |
|---------|----------:|------------:|------------:|
| **pdfparser** | **1.000** | **0.909** | **0.990** |
| pdfplumber | 0.909 | 0.727 | 0.826 |
| pymupdf | 0.818 | 0.727 | 0.774 |
| others | 0 | 0 | 0 |

---

## 3. Gold standards are independent of pdfparser

| Source | How gold is defined |
|--------|---------------------|
| Synthetic basic/stress | Written into GT at PDF generation (`generate_corpus.py`, `generate_complex_corpus.py`) — known tokens, shapes, grids |
| Objects | Expected image counts, form field names, URI, outline titles from generator |
| Real PDFs | Count ranges / tolerances from document analysis (`build_gold_standards.py`), **not** exported from our extractor |
| Metrics | Single `metrics.py` for **all** libraries (shared normalize, cell F1, detect F1, objects sets) |

No gold file encodes “pdfparser expected output.” Field `our_library_priority` is roadmap metadata only.

---

## 4. Where we win (real gaps competitors still fail)

| Doc | pdfparser | Best competitor | Why |
|-----|----------:|----------------:|-----|
| `07_table_stream` | **100** | 25 (all others) | Only structured stream tables |
| `08_table_partial_border` | **100** | 55.7 plumber | Hybrid 5×5 vs 1×2 collapse |
| `20_bank_statement_multipage` | **100** | 89.1 plumber | Multi-page stitch detect F1=1.0 |
| `21_table_overflow_cells` | **100** | 98.4 plumber | Cell text fidelity |
| Lattice/complex/dense | ~1.0 cell | ~1.0 plumber / ~0.99 mupdf | Parity on ruled grids |

Per-doc overall: **7 wins / 20 ties / 6 losses** vs best other.

---

## 5. Honest losses (not SOTA everywhere)

| Doc | Issue |
|-----|--------|
| `12_encrypted_password` | **Intentional fail** — product rejects encryption (K15); others open with password |
| `22_table_merged_headers` | cell F1 0.919 vs plumber 0.987 |
| `23_side_by_side_tables` | 99.1 vs plumber 100 |
| `36_real_two_tables` | under-detect (pred=1 vs exp=2); mupdf 100 |
| `39_real_fed_beigebook` | table FP noise (90 vs 100) |
| `41_real_nist_withdrawn_notice` | residual FPs (70 vs 100 for libraries that emit 0) |

---

## 6. Unbiased suite how-to

```bash
cargo build --release -p pdfparser-cli
source .venv/bin/activate
pip install -r benchmark/requirements.txt   # once
python benchmark/scripts/run_accuracy_benchmark.py
# → docs/accuracy-scoreboard.md
# → benchmark/results/accuracy_results.json
```

Same command scores **all** adapters. Do not tune gold to pdfparser outputs.

---

## 7. Bottom line

| Question | Answer |
|----------|--------|
| Are competitor numbers stable / unchanged? | **Yes — Δ ≈ 0 vs Phase U baseline** |
| Is gold independent? | **Yes — generator + domain counts, shared metrics** |
| Are we SOTA on **this native-PDF harness**? | **Yes on overall mean and grid-gold tables** |
| Are we SOTA on **all PDFs / all tasks**? | **No** — no password decrypt, no OCR, some real-doc gaps |
| Text? | **Tied SOTA** with pypdf (F1=1.0) |
| Objects? | **Tied SOTA** with pymupdf/pypdf (100) on scored docs |
| Tables? | **Clear SOTA** on this grid-gold set (0.990 cell F1) |

*Verification run archived in `benchmark/results/sota_verify_run.log` and refreshed scoreboard.*
