# ICDAR F1 regression analysis (0.58 → 0.46)

**Date:** 2026-07-12  
**Question:** Why did ICDAR F1 fall after recent work, and is Engine V2 to blame?

## Executive answer

| Claim | Verdict |
|-------|---------|
| Engine V2 “broke” ICDAR vs legacy Auto on the **same binary** | **False** — F1 0.457 (V2) vs **0.454 (legacy router)** |
| ICDAR degraded vs **July mid-cycle snapshot (F1 ≈0.58)** | **True** — shared builder/sensing changes ↑ over-detect |
| Revert product Auto to legacy because V2 is incompetent | **Not supported by A/B** — same ICDAR failure mode; real G1 **improved** |

## Numbers

### ICDAR-2013 (67 docs, Camelot metric)

| Snapshot | pdfparser F1 | TEDS | row | col | notes |
|----------|-------------:|-----:|----:|----:|-------|
| Prior published board | ~0.58–0.60 | ~0.33 | ~0.34 | ~0.50 | Pre nested densify / Auto=V2 era |
| **Latest product Auto (V2)** | **0.457** | 0.321 | 0.336 | 0.459 | Full competitive script |
| **Same binary `--legacy-router`** | **0.454** | 0.347 | 0.324 | 0.497 | Controlled A/B |
| Camelot lattice (ref) | 0.766 | 0.784 | 0.748 | 0.806 | Unchanged reference |
| Camelot auto | 0.864 | 0.786 | 0.564 | 0.792 | Unchanged reference |

### Controlled A/B (same release binary, 67 docs)

| Metric | Engine V2 (Auto) | Legacy NMS | Δ |
|--------|-----------------:|-----------:|--:|
| F1 | 0.457 | 0.454 | **+0.003** |
| TEDS | 0.321 | 0.347 | −0.026 |
| pred tables (total) | **481** | **481** | 0 |
| GT tables (total) | 158 | 158 | — |
| FP (count metric) | 335 | 336 | ~0 |
| FN | 12 | 13 | ~0 |

**Conclusion:** On ICDAR, exclusive AutoRouter vs soup NMS does **not** explain the drop. Both emit ~**3.0×** as many tables as gold (481 vs 158). That over-detect **destroys detection F1** under Camelot’s count-based pairing.

## What actually regressed (shared stack, not “V2 vs Auto”)

Changes landed for **real_structure G1** that also affect ICDAR:

1. **More lattice fragments** — multi-region CC, densify, exterior stubs, nested parent/child keep, thin-fill rules.  
   Example `eu-001` page 1: gold 3 tables (8×4, 13×4, 10×4); we emit **4** (extra **4×4** slice) → OVER_DETECT + ROW_MISCOUNT even when the big grids are present.

2. **More stream/network proposals** on competition layouts with multi-col text but weak rules.

3. **Nested multi-table keep** — correct for Italian insurance (G1 win); on ICDAR often keeps a **corner grid + full grid** as two tables when gold has one.

4. **Trm / geometry fixes** — correct coords change joint graphs and region boundaries vs the older wrong-matrix world; can create *new* splits/merges.

5. **Runtime ~30s** — product Auto has `allow_auto_render`; opportunistic full-page render probes can fire on sparse-rule pages (latency, not F1 directly).

Failure histogram (latest): **OVER_DETECT 54/67**, WRONG_SHAPE 56, ROW_MISCOUNT 54, BAD_STRUCTURE 51, UNDER_DETECT only 3, good only 2.

## Why real G1 improved while ICDAR got worse

| Track | Goal of recent work | Outcome |
|-------|---------------------|---------|
| **real_structure (15 reviewed golds)** | Recover donors, nested 42, GDP stubs, Trm, stream | cell F1 **~0.49 → ~0.64**; det **~0.96** |
| **ICDAR (67 competition pages)** | Not tuned (policy: no ICDAR in loop) | Count F1 collapses under **3× over-detect** |

Same code can help **recall/structure on human gold** and hurt **detection F1 on ICDAR’s order-based, count-sensitive metric**. That is a **metric/domain mismatch**, not proof that V2 is “incompetent” relative to legacy.

## Should we revert Auto → legacy?

| Option | ICDAR | Real G1 | Nested 42 | Recommendation |
|--------|-------|---------|-----------|----------------|
| Stay on **Engine V2 Auto** | ≈ legacy | Best current | Works | **Preferred** for product track |
| Revert **legacy_router** as default | No ICDAR gain | Loses exclusive cleanup / same builders still over-detect | Nested keep still in NMS path but different selection | **Does not fix ICDAR** |
| Keep V2 + **ICDAR-aware over-detect pass** (separate, policy-safe external eval only) | Possible lift | Must A/B real G1 | Protect nested keep | Future work if ICDAR rank matters |

**Do not flip product back to legacy solely because of ICDAR.** A/B shows legacy does not restore the 0.58 snapshot; that snapshot was an **older engine** before densify/nested/Trm.

## What the 0.58 snapshot was

The ~0.58 F1 board was measured on a **pre-flip, pre-nested, pre-densify-exterior** pipeline (multi-region lattice era). It was already **#5**, not SOTA. The new run is a regression **relative to that intermediate point**, while real_structure and owned synthetic boards moved **up**.

## If we want ICDAR F1 back up (without destroying G1)

Prioritized, testable levers (external ICDAR A/B + real_structure guard):

1. **Stricter multi-region lattice merge** when same-schema adjacent fragments share almost full width (kill eu-001-style 4×4 phantom).  
2. **Nested keep only when both regions have strong independent structure** (min area ratio + min rows), not every corner.  
3. **Cap tables per page** for competition-style dense rule soup (product-tunable, not magic on file names).  
4. **Disable opportunistic render for latency** when measuring pure vector ICDAR (CLI flag already: avoid HQ; tighten K25).  
5. **Never** retune thresholds on ICDAR gold in CI (CONTRIBUTING).

## Bottom line

- Degradation is real on ICDAR: **0.58 → 0.46**, driven by **over-detect (481 vs 158 tables)**.  
- Engine V2 vs legacy on **today’s code** is a **wash** on ICDAR F1.  
- Product Auto=V2 remains justified by **real_structure** and nested multi-table; legacy is **not** a magic ICDAR fix.  
- Improving ICDAR means **shared detector discipline**, not undoing the router flip.
