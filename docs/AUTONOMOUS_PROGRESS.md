# Autonomous gated development progress

**Superseded as a status plane.** Gate PASS/FAIL: [`STATUS.md`](STATUS.md) only.

**Date:** 2026-07-18 - historical notes. **GATE-3 / GATE-4 / GATE-5 are NOT green.**  
Core freeze remains `g2.json` cell **0.637**. The **0.820** figure below is **not** a freeze and must not be cited.

## Policy (non-negotiable)
1. ICDAR never in CI / corpus / tuning (external honesty only).
2. No gold rewrites; no doc/test-suite-specific hacks.
3. Do not promote GATE-3/4/5 from this file.
4. No-regress vs real g2 core freeze (cell 0.637).

## Live ICDAR-2013 (67 docs) - historical 2026-07-18 snapshot

Not a merge bar. Latest solo dump is **peers=1** (not rank #1). Multi-peer README table: pdfparser **#2** detection F1.

| Metric | Start | **Then** | Δ | Target | Status |
|--------|------:|--------:|--:|-------:|:------:|
| F1 | 0.814 | **0.824** | +0.010 | ≥0.65 G4 | snapshot only |
| TEDS | 0.439 | **0.481** | +0.042 | ≥0.50 G4 | snapshot only |
| row | 0.449 | **0.515** | +0.066 | ≥0.50 G3 | snapshot only |
| col | 0.507 | **0.559** | +0.052 | ≥0.55 G3 | snapshot only |

## Hard gates (`check_phase_gates.py`)

See [`STATUS.md`](STATUS.md). The 2026-07-18 table that claimed GATE-3 PASS is **retracted** (disagrees with `g3_industry.json` INVALID and `phase-structure-gates.md`).

## Real-track
- freeze core cell **0.637** (`g2.json`); later live ~0.738 is not a freeze
- ignore historical "core 0.820" claim in this file

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
