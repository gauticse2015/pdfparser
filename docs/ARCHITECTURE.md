# Architecture (as-built stub)

This file is a **pointer**, not a second status plane.  
Gate PASS/FAIL: [`STATUS.md`](STATUS.md) only.

Diagrams: **ASCII only** (no Mermaid).

---

## Design map

| Doc | Role |
|-----|------|
| [`design-table-engine-v2.md`](design-table-engine-v2.md) | Capability ladder, K-decisions, ICDAR ban, Fast never render, nested keep. **As-built: Auto already Engine V2** (migration "Auto still legacy until G1" is historical). |
| [`design-zero-regression-hardening.md`](design-zero-regression-hardening.md) | As-built A1-A5/B inventory + zero-regression PR bible. P0.1 landed; A1.12 evidence is pre-fix; living board is STATUS.md. |
| [`design-table-engine-v3-industry.md`](design-table-engine-v3-industry.md) | Exclusive-first **target** (Sense -> Classify -> Build once). **Not** what product Auto does today. |
| [`implementation-plan-v3-gated.md`](implementation-plan-v3-gated.md) | GATE-0..5 **quality** metric defs. Do not claim GATE-3/4/5 green. |
| [`options-deprecation-map.md`](options-deprecation-map.md) | 12-field product surface. **No Deref.** |

---

## Product Auto call graph (today)

Soup-then-`finalize_engine_v2`. Detectors still all run. Exclusive page flavor is **not** shipped.

```text
page (post-/Rotate runs + rules + optional rasters)
    |
    v
+--------------------------------------------------------------+
| detect_tables_page_with_raster  (orchestrator/page.rs)       |
|  1. LatticeDetector  (always if modes.lattice)               |
|  2. HybridDetector   (always if modes.hybrid; filter bbox    |
|                       if ruled_owns_page)                    |
|  3. NetworkDetector  (precision or recall / recovery)        |
|  4. optional StreamDetector if allow_classic_stream          |
|  5. optional Network again if hybrid_over_wide               |
|  6. form discriminator                                       |
|  7. optional Network again if form wiped lattice             |
|  8. demote lattice column slices                             |
|  9. if use_engine_v2 && !legacy_router:                      |
|        finalize_engine_v2  (tables -> proposals ->           |
|          vertical_merge + partition -> emit -> cleanup)      |
|     else: soup NMS                                           |
+--------------------------------------------------------------+
    |
    v
optional stitch (product default on; eval --no-stitch)
```

| Preset | Router | Full-page render |
|--------|--------|------------------|
| Auto / Full | Engine V2 finalize | opportunistic (`allow_auto_render`); fail-soft |
| Fast | Engine V2 finalize | **never** |
| HighQuality | Engine V2 + diagnostics | explicit on |
| Rollback | `legacy_router=true` soup NMS | unchanged |

`PageEvidence<'a>` is diagnostics-only (borrowed; dump clone is `PageEvidenceOwned`). `route_proposals` is not on the product call graph.

---

## Crate sketch

```text
pdfparser-cli
    -> pdfparser (facade: Document / extract / tables)
        -> pdfparser-content (VM: text + rules + forms)
        -> pdfparser-tables  (lattice / hybrid / network / router)
        -> pdfparser-core / fonts / ir / export
```

Parse-stack correctness and exclusive-first shadow are sequenced in the hardening design (Phase 2 / Phase 3). Do not mix them into mechanical splits.
