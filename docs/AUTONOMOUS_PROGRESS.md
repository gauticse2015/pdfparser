# Autonomous gated development progress

**Date:** 2026-07-18 — **GATE-3 PASS**

## Policy (non-negotiable)
1. ICDAR hard-required; no silent SKIP.
2. No gold rewrites; no doc/test-suite-specific hacks.
3. No git promote until G3+G4 with live multi-peer ICDAR.
4. No-regress F1/TEDS/row/col + real g2 core.

## Live ICDAR-2013 (67 docs)

| Metric | Start | **Now** | Δ | Target | Status |
|--------|------:|--------:|--:|-------:|:------:|
| F1 | 0.814 | **0.824** | +0.010 | ≥0.65 G4 | ✅ |
| TEDS | 0.439 | **0.481** | +0.042 | ≥0.50 G4 | −0.019 |
| row | 0.449 | **0.515** | +0.066 | ≥0.50 G3 | ✅ |
| col | 0.507 | **0.559** | +0.052 | ≥0.55 G3 | ✅ |

## Hard gates (`check_phase_gates.py`)

| Phase | Status |
|------:|--------|
| GATE-1 | **PASS** (live ICDAR) |
| GATE-2 | **PASS** (live ICDAR) |
| GATE-3 | **PASS** — G3.2–G3.8 all green |
| GATE-4 | **IN PROGRESS** — TEDS 0.481 → need ≥0.50; F1 already ≥0.65 |

## Real-track
- core cell **0.820**, no g2 −0.03 regressions

## Key general fixes
1. Lattice V-skeleton keep + year/decimal glued redistribute  
2. Footer totals discipline; small_under densify; header wrap merge  
3. Stream header pad (body-first numeric); trailing note strip  
4. Multi-col short-token prose stream reject (FP kill → col lift)  
5. Hard rejects when measured regress (exterior Y, near_match, etc.)

## Next (Phase C)
- Lift TEDS +0.019 without dropping F1/row/col/core
- Content assignment quality on exact-count wrong-shape docs
- Then freeze G3/G4 with multi-peer ICDAR board
