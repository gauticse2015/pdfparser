# Why ICDAR Quality Is Still Unacceptable — and the Algorithm Plan

**Date:** 2026-07-11  
**Standing:** ICDAR F1 **0.672** · TEDS **0.331** · row **0.382** · col **0.489** · rank **#4** · **0.58 s**  
**Bar:** Camelot lattice F1 **0.766** TEDS **0.784**; Camelot auto F1 **0.864** TEDS **0.786**  
**Constraint:** Keep release latency in the same class (target **≤2 s** full ICDAR set, **≪ Camelot auto’s 72 s**).

---

## 1. Hard truth: synthetic #1 ≠ competitive quality

| Track | pdfparser | Verdict |
|-------|-----------|---------|
| Hard / precision / sensing (owned) | cell F1 ≈ **1.0**, rank **#1** | Product regression is healthy |
| ICDAR-2013 (external) | F1 **0.67**, TEDS **0.33**, row/col **0.38/0.49** | **Not release-grade vs peers** |

### Why the improvements “don’t show up”

1. **Wrong distribution.** Synthetic ReportLab grids are clean vector strokes, controlled chrome, one layout family. ICDAR is multipage government/EU PDFs: faint rules, painted lines, huge borderless tables, multi-table pages, order-sensitive gold.
2. **We optimized engines in isolation; competitive score is a joint (detect × structure × order).**  
   - Detect wrong → F1 dies (OVER_DETECT on **36/67** docs).  
   - Detect right but shape wrong → TEDS/row/col die (**good bucket only 4/67**; even when `n_us==n_gt`, mean TEDS ≈ **0.39**).
3. **Camelot auto is not “run everything and NMS.”** It is **exclusive per-page routing**:
   - probe ruled lines on a **render** → enough H+V → **lattice combined (raster∪vector)** only  
   - else → **network** (text-alignment) only  
   We still often emit **lattice+stream soup** (`lattice+stream` on **22/67** docs) → systematic OVER_DETECT.
4. **Our “stream/hybrid” is not Camelot network.** On `us-017`/`us-018` we still emit **~3-row** slices vs gold **~31–58 rows**. That single class destroys mean TEDS.
5. **Vector-only lattice still misses real rules.** `us-012`: camelot lattice TEDS **0.91**, we **MISS_ALL**. Raster morphology finds lines our `RuleSegment` path never sees.

```
Synthetic win ──► knobs on vector lattice / stream gaps
                         │
                         ▼
              ICDAR still loses because:
              • wrong expert selected (or both)
              • weak expert on borderless
              • missing lines (no raster)
              • over-proposal of stream FPs
              • order-based scoring amplifies mess
```

**Conclusion:** More synthetic micro-wins without changing the **document-class → expert → exclusive emit** architecture will not make TEDS/row/col acceptable.

---

## 2. What the data says we must fix (priority order)

Latest ICDAR method mix (67 PDFs, current CLI):

| Method mix (docs) | Count |
|-------------------|------:|
| lattice+stream | **22** |
| lattice only | 15 |
| hybrid (± stream) | 16 |
| stream only | 5 |
| NONE | 4 |
| lattice present at all | **42** (was ~6 earlier — progress, not enough) |

Failure histogram (dominant):

| Mode | Docs | Role in score |
|------|-----:|---------------|
| OVER_DETECT | **36** | Kills F1 via FP tables |
| WRONG_SHAPE / ROW/COL | 36–47 | Kills TEDS/row/col |
| MULTI_PAGE / MULTI_TABLE | 17–28 | Order + fragment |
| UNDER / MISS_ALL | 11+4 | Recall |

**Bucket reality:** `good=4`, `over=36`, `bad_struct_ok_count=13`.  
Detection count exact on 20 docs — and even then TEDS ≈ **0.39**. So **structure is the larger quality cliff**, not only detection.

---

## 3. Design principle: one product, multiple experts, one decision

You asked: single algorithm vs classifier + specialists.

**Answer:** A **single pass of “run all detectors and hope NMS works” is what we have now and it is wrong.**  
Camelot’s win is **routing**, not magic. We need the same idea, but **latency-first**.

```
                    ┌─────────────────────┐
   page content ───►│  cheap page probe   │  (no heavy render unless needed)
                    └─────────┬───────────┘
                              │ class ∈ {Ruled, Borderless, Mixed, None}
              ┌───────────────┼───────────────┬────────────────┐
              ▼               ▼               ▼                ▼
         Expert L         Expert N       Expert H          emit ∅
        (lattice+)      (network/text)   (hybrid/partial)
              │               │               │
              └───────────────┴───────────────┘
                              │
                    exclusive tables for that page
                    (optional light second expert only on residual bands)
```

**Not** Camelot auto’s full-page OpenCV at 300 dpi by default (that is the 72 s path).  
**Yes** exclusive expert selection + much cheaper probes.

---

## 4. Target architecture (release-viable latency)

### Latency budget (67-page ICDAR class)

| Stage | Budget | Notes |
|-------|--------|-------|
| Vector parse + text (already paid) | ~0.4–0.6 s total set | Keep as base |
| Page probe (vector rules + text stats) | **+0–50 ms/page** | Always on |
| Expert L (vector + optional thin-fill) | already in budget | Default for Ruled |
| Expert N (network-class stream) | **+0–20 ms/page** when selected | Replace weak stream |
| Expert L-raster (optional) | **+5–15 ms/page** only if Ruled probe weak | Tiny render / low dpi / ROI |
| **Full set target** | **≤2 s** (comfort), **≤5 s** hard cap | Still ≪ auto |

### 4.1 Page classifier (cheap, mandatory)

Inputs (all already available after interpret):

| Feature | Signal |
|---------|--------|
| `n_h`, `n_v`, joint density, component count | Ruled lattice likely |
| Long-rule coverage fraction of page | Real grid vs chrome ticks |
| Multi-col text band count, col-sep score | Borderless table likely |
| Stream word-list score (2-col alpha) | Anti-table prose |
| Empty-frame / form-likeness | Reject |
| Page area of strongest joint CC | Table region proposal |

Output classes:

| Class | Primary expert | Secondary (only residual bands) |
|-------|----------------|----------------------------------|
| **Ruled** | Lattice (vector; densify Y from text when H sparse) | None by default |
| **Borderless** | Network (new) | None by default |
| **PartialBorder** | Hybrid | Lattice if residual joints |
| **None** | emit empty | — |
| **AmbiguousRuled** | Lattice vector; if fill/edge weak → **light raster lattice on ROI** | — |

**Critical policy change:**  
`Full` preset today runs lattice+hybrid+stream then NMS.  
**Ship `Auto` preset:** **exactly one primary expert per page** (Camelot auto semantics). Keep `Full` as debug/soup for research only.

This alone attacks OVER_DETECT (36 docs).

### 4.2 Expert L — Ruled tables (close gap to camelot lattice)

Must reach ~camelot lattice F1 **0.76** / TEDS **0.75+** on ruled subset.

| Gap | Algorithm work |
|-----|----------------|
| Missing lines (`us-012`) | **ROI raster line morph** when vector joints sparse but text is gridded; stamp into same contour/joint pipeline (not full-page 300 dpi always) |
| Row undercount | Text-band densify (done for 93) + **partial H reconstruction** from multi-col baselines |
| Col under/over | Joint-span filters keep; add **text column projection** only inside lattice bbox as soft anchors |
| Over-fragment | Contour/region-first: one table per joint-rich region; merge CCs that share x-range and small y-gap with same col anchors |
| Chrome FP | Whitespace/empty reject, min area, joint≥5 — already partial; tighten under Auto |
| Order | Stable sort y-desc, x-asc (affects F1 under order match) |

### 4.3 Expert N — Borderless / large statistical (close TEDS cliff)

This is the **missing Camelot network**. Current stream cannot recover 30-row tables.

| Requirement | Design |
|-------------|--------|
| Textlines | Aggregate runs → lines (reading order, baseline cluster) |
| Alignments | Left/right/center/column edges (Camelot network style) |
| Table area | Dense multi-col region grow by median row height |
| Rows | **One row per textline band** inside area (not gap-split into 3-row slices) |
| Cols | Projection of alignments + gap valleys |
| Continuity | Same-schema merge across small prose gaps (92); **different schema = split** (59) |
| Reject | Word-list 2-col FP (95); prose mean chars |

Success metric: on borderless ICDAR-like owned fixtures with 25–40 rows, shape within ±2 rows and cell F1 ≥ 0.9.

### 4.4 Expert H — Partial borders

Keep hybrid; only selected on PartialBorder class. Do not let hybrid compete with strong lattice on same bbox.

### 4.5 Global post (all experts)

1. **IoU-NMS within page** (not only containment) after exclusive routing residual.  
2. **Confidence calibration:** `conf = edge×fill×(1−empty)×structure_prior` — drop below threshold.  
3. **Multipage:** stitch only when Auto class is Ruled/Borderless **and** header+col anchors match; ICDAR gold is per-page so default stitch **off** for competitive eval.  
4. **Order:** deterministic geometric order for competitive harness.

---

## 5. Why not “just Camelot auto”?

| | Camelot auto | Our plan |
|--|--------------|----------|
| Strength | Raster+network quality | Same *routing idea* |
| Cost | ~72 s / 67 docs | Vector-first; raster **ROI only** |
| Expert N | Mature network | Must build/upgrade (biggest code investment) |
| Product fit | Python/OpenCV heavy | Rust, library-first, low latency |

Releasing something with TEDS 0.33 while claiming table SOTA is not honest.  
Releasing **Auto routing + Expert N + optional ROI raster** with TEDS→0.55–0.70 and F1→0.75–0.82 at **&lt;2–5 s** is a credible path.

---

## 6. Phased delivery (algorithm program, not fixture thrash)

### Phase A — Auto routing (1 week) — **F1 lever**

- Implement `TablePreset::Auto` (exclusive expert).  
- Probe features + class enum.  
- **Ban dual lattice+stream emit** under Auto.  
- Owned fixtures: multi-table ruled, prose+table, 95-class FP.  
- **Expect ICDAR:** OVER_DETECT down → F1 **+0.05–0.10**.

### Phase B — Expert N (network) (2–3 weeks) — **TEDS/row lever**

- Textline + alignment table builder.  
- Replace default stream in Auto/Borderless.  
- Owned large borderless multipage fixtures (30+ rows).  
- **Expect ICDAR:** TEDS **→0.45–0.55**, row **→0.50+** on borderless subset; overall TEDS **+0.10–0.15**.

### Phase C — ROI raster lattice (1–2 weeks) — **recall + lattice TEDS**

- Low-dpi render of joint-candidate ROI only when AmbiguousRuled.  
- Morph H/V + union with vector.  
- **Expect:** MISS_ALL/`us-012`-class recovery; lattice-subset TEDS up.

### Phase D — Structure polish (1 week)

- Col/row densify, footer strip (done), span, order.  
- Competitive: F1 **≥0.75**, TEDS **≥0.55**, row/col **≥0.55** as **minimum accept**.  
- Stretch: F1 **≥0.80**, TEDS **≥0.65**.

### Phase E — Continuous eval (ongoing)

- **Regression:** hard/precision/sensing must stay ≥0.99 cell F1.  
- **Competitive:** ICDAR black-box only after each phase, not for tuning knobs.  
- **Gap→fixture:** any remaining ICDAR failure class → new **owned** synthetic mode (no file copy).

---

## 7. Success criteria (honest release bar)

| Gate | Metric | Minimum | Stretch |
|------|--------|---------|---------|
| Product | hard+precision+sensing cell F1 | ≥0.99 | 1.0 |
| Competitive detect | ICDAR F1 | **≥0.75** | ≥0.82 |
| Competitive structure | ICDAR TEDS | **≥0.55** | ≥0.70 |
| Competitive shape | row / col | **≥0.55** | ≥0.70 |
| Latency | full ICDAR set | **≤2 s** | ≤1 s |
| vs plumber | F1 and TEDS | beat both | beat pymupdf F1 |

Below minimum: **do not market as competitive with Camelot on ruled+borderless quality.**

---

## 8. What we stop doing

1. Declaring victory from synthetic scoreboards alone.  
2. Running lattice+stream+hybrid soup as the default product path.  
3. Threshold thrash without a new expert or routing change.  
4. Expecting empty-neighbor span tweaks to fix 30-row borderless TEDS.  
5. Full-page high-dpi OpenCV as default (latency death).

---

## 9. One-paragraph thesis

Synthetic perfection means our **vector lattice and FP controls work on clean PDFs**. ICDAR still fails because we **select and combine experts wrong**, our **borderless expert is not network-class**, and we **lack cheap raster fallback for invisible rules**. The fix is a **latency-first Auto router** (like Camelot auto’s exclusivity, not its cost), a real **Expert N**, and **ROI raster** only when vector evidence is weak—measured by ICDAR F1/TEDS floors, while keeping sub-second-to-few-second latency so the library stays shippable.

---

## 10. Immediate next implementation step

**Phase A only:** `TablePreset::Auto` + exclusive routing + probe features + ICDAR black-box delta + no regression on hard/precision/sensing.

Do not start Phase B and C in parallel without Auto: dual-soup will keep polluting F1 and hide Expert N gains.
