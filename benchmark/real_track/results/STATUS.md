# Real-track status

**Updated:** 2026-07-18 honest restart (ICDAR hard-required)

| Phase | Status |
|------:|--------|
| 0–2 | **PASS** (live ICDAR enforced) |
| 3 Topology | **FAIL** — ICDAR row 0.44 / col 0.49 (need ≥0.50 / ≥0.55) |
| 4–5 | blocked on GATE-3 |

## Live ICDAR rank
1. camelot_auto F1 **0.864** TEDS **0.786**
2. **pdfparser F1 0.814 TEDS 0.440**
3. pymupdf F1 0.776
4. camelot lattice 0.766
5. pdfplumber 0.662

## Real core
cell **0.820** (g2 was 0.637); **no g2 regressions**; donors fixed 0.943

## Freezes
- `phase1_detect_precision.json` — PASS with live ICDAR
- `phase2_detect_recall.json` — PASS with live ICDAR
- `phase4_cells.json` / `g3_industry.json` — **INVALID** (prior skip)

See `docs/AUTONOMOUS_PROGRESS.md`.
