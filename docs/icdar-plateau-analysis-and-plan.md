# Why ICDAR Progress Stalled — Root Cause Analysis & Single-Shot Plan

**Date:** 2026-07-11  
**Status:** Decision doc (stop knob-tuning; execute architecture plan)  
**Audience:** pdfparser table-engine owners  

---

## 0. Executive answer

We are not stuck because “spans are hard” or because we need more `joint_span_frac` knobs.
We are stuck because **on the ICDAR-2013 competitive set we almost never run the lattice engine that we have been optimizing**.

| Fact (measured 2026-07-11, 67 ICDAR PDFs) | Value |
|-------------------------------------------|------:|
| Docs where **any** lattice table is emitted | **6 / 67** |
| Docs that are **stream and/or hybrid only** | **57 / 67** |
| Total tables by method | stream **176**, hybrid **31**, lattice **8** |
| Docs with zero tables | 4 |

Meanwhile Camelot’s competitive win is **not** “better colspan on synthetic ReportLab grids.” It is:

1. **A different line-sensing stack** (raster morphology ± vector union → contour regions → joints → grid).
2. **A different borderless stack** (`network`, ~1k LOC text-alignment parser) used by `flavor=auto` when the page is not ruled.
3. **Hard precision gates** (contour ≥5 joints, min area fraction, whitespace ≥90% reject) calibrated on real pages.

Our last several phases optimized a **vector-rule lattice** that shines on **our synthetic corpus** (clean `GRID` strokes) and barely participates on ICDAR. That is why ICDAR moves by **+0.004 F1** while precision/hard synthetic scoreboards go to **~1.0**.

---

## 1. The continuous-improvement trap (why the loop feels endless)

```
Synthetic hard/precision (ReportLab vector grids)
        │
        ▼
  Tune lattice knobs (joints, spans, NMS, exclusive text assign…)
        │
        ▼
  Synthetic cell F1 → 0.99–1.00  ✓  (real progress on that distribution)
        │
        ▼
  Re-run ICDAR
        │
        ▼
  Almost all pages → stream/hybrid (lattice silent)
        │
        ▼
  Stream structure wrong → OVER_DETECT + ROW/COL_MISCOUNT + TEDS≈0.2
        │
        ▼
  Misread as “need better spans / better filters”
        │
        └──────────► another lattice micro-fix ──► loop
```

### What the loop optimizes vs what ICDAR scores

| Dimension | Synthetic regression | ICDAR-2013 competitive |
|-----------|----------------------|-------------------------|
| Line representation | Clean vector `RuleSegment`s | Often **no usable vector rules** (painted lines, thin rects, anti-aliased rules) |
| Dominant method (ours) | Lattice | **Stream/hybrid (~85% of docs)** |
| Failure modes | Spans, multi-region, phantom ticks | **Wrong method + weak stream structure** |
| Gold style | Dense cell grids we control | ICDAR `*-str.xml`, order match, difflib TEDS proxy |
| Knobs that help | joint filters, span merge, exclusive assign | **Line sensing, region proposal, network-class stream, FP gates** |

**Conclusion:** Further lattice span polish is **locally rational** for product regression and **globally useless** for ICDAR rank until lattice **fires** (or stream becomes network-class).

---

## 2. Evidence: failure modes are method-attribution failures

### 2.1 Method mix on worst TEDS-gap docs

| Doc | pdfparser methods | shapes (us) | Camelot path that wins | Camelot shapes |
|-----|-------------------|-------------|------------------------|----------------|
| `us-012` | **stream only** | 5×8 | lattice **raster**: 26 H + 7 V lines, 154 joints | **23×6** (gold ~21×6) |
| `us-007` | **stream only** (7 tabs) | 5×3, 19×7, … | lattice vector/raster | **2× (36×5, 36×6)** |
| `eu-014` | **stream only** (4 tabs) | small fragments | lattice | **1× (10×2)** |
| `us-017` | **hybrid only** (6 tabs) | all ~3×N | **auto → network** (lattice finds **0**) | **6× (~28–34 rows)** |
| `us-038` | **stream only** | 4×2 | lattice | 8×2 / 8×3 |
| `eu-001` | **stream only** | many small | lattice multi-region | matches gold counts |

On `us-012`, PlayA-style layout walk sees **0 LTLine / 0 LTRect**, yet OpenCV morphology on a render finds **26 horizontal** line segments and builds a correct grid. Our pipeline never gets those lines → lattice exits early → stream invents a 5×8 text layout.

On `us-017`, even Camelot **lattice** fails (0 tables). Camelot **auto** routes every page to **network** and recovers 30-row tables. Our hybrid emits 3-row header-ish slices. Comparing only lattice-vs-lattice understates how much of Camelot’s **auto** win is the **network** branch.

### 2.2 Failure histogram reinterpreted

| ICDAR mode | Count | Real driver (ours) |
|------------|------:|--------------------|
| OVER_DETECT | 37 | Stream/hybrid **fragmentation** + chrome-as-table; not “too many lattice CCs” on these docs |
| ROW_MISCOUNT | 50 | Stream row banding / hybrid partial rules; lattice silent |
| COL_MISCOUNT | 50 | Stream column projection errors |
| WRONG_SHAPE | 55 | Downstream of wrong method |
| UNDER_DETECT / MISS_ALL | 14+4 | No lattice lines + stream gates reject, or true empty |
| “Detect OK, TEDS bad” | 11 | Same table **count** as gold but **wrong grids** (e.g. 3-row hybrids vs 31-row network) |

Fragmentation pattern (max rows us ≪ max rows gold): `us-017`, `us-012`, `us-007`, `us-018`, `eu-027`, … — classic **weak text-table reconstruction**, not missing colspan.

### 2.3 Why Camelot lattice F1=0.77 while we “also have lattice”

Camelot lattice (especially `engine=combined` / default raster) is **not the same algorithm** as our `detect_lattice_tables`:

| Stage | Camelot lattice | pdfparser lattice (today) |
|-------|-----------------|---------------------------|
| Input | Render @ ~300dpi → adaptive threshold → morph open (`line_scale`) **and/or** layout vector lines stamped into masks | Content-stream **RuleSegment** only |
| Region proposal | `find_contours(V+H masks)` → bbox candidates | Connected components of crossing segments |
| Table gate | Contour needs **>4 joints**; min area fraction of page | `min_joints` on CC; fill/empty/tiny heuristics |
| Anchors | Joint x/y **inside contour**, then `merge_close_lines` | Cluster all joint+line coords, then **joint-span filter** (drops short lines) |
| FP reject | `whitespace >= 90` drop (ICDAR-calibrated) | form discriminator, tiny area, empty_frac |
| Spans | Edge presence + `shift_text` / `copy_text` | Dense edge merge + exclusive text assign |
| When no lines | (lattice empty) → **auto uses network** | stream/hybrid still run and **emit wrong tables** |

Our lattice is a credible **vector-grid** engine. Camelot’s is a **visual ruled-region** engine with a strong fallback. ICDAR is full of pages that need the latter.

---

## 3. How Camelot wins each problem we named

### 3.1 OVER_DETECT

Camelot reduces FPs by **architecture**, not by endless stream demotion:

1. **Contour = candidate table** — noise lines that do not form a closed joint-rich region never become tables.
2. **Joint count ≤4 reject** — form ticks / double underlines die here.
3. **Min contour area** (`_MIN_TABLE_AREA_FRACTION = 0.0005`) — glyph-scale noise out.
4. **Whitespace ≥ 90% reject** — comment in Camelot source: calibrated so ICDAR false positives (91–95% empty) die while real tables live.
5. **One contour → one grid** — avoids “7 stream tables” for 2 true tables (`us-007`).
6. **auto does not run lattice+stream union on every page** — ruled pages lattice-only; borderless pages network-only. We **always** can emit stream alongside weak lattice, which **adds** FPs.

We tried NMS, form scrub, list reject, joint filters — all **post-hoc** on a candidate soup. Camelot **never proposes** most of those candidates.

### 3.2 ROW_MISCOUNT (we usually have **too few** rows)

Measured pattern: count-ok pairings with rows_us ≪ rows_gt dominate over too-many-rows.

Camelot’s advantages:

1. **Morphology recovers full ruling sets** from pixels (us-012: 26 H lines). Vector-only sees 0–few.
2. **Row anchors = merged joint Y’s + bbox edges**, not “keep H lines only if joint span ≥ 22% of table width.” Our span filter is correct for **phantom ticks on synthetic multi-level headers** and **catastrophic** when intermediate rules are partial or when we should not be in lattice at all.
3. **`merge_close_lines`** collapses double rules into one row boundary — stable row counts.
4. For borderless pages, **network** builds rows from textline topology (min 6 textlines, column spread, header logic) — recovers 30+ rows where our stream/hybrid stops at header bands (~3 rows).

### 3.3 COL_MISCOUNT

Same story: joint X anchors inside a correct region beat stream gap clustering. Network uses alignment edges (`TextAlignments`) rather than simple whitespace peaks.

### 3.4 MULTI_TABLE_PAGE / MULTI_PAGE_DOC

Camelot: multiple contours → multiple tables; order by geometry.  
We: stream band splitting often **over-segments** one visual table into many, or **under-merges** multipage continuations. Stitch helps product bank statements; ICDAR gold is usually **per-page tables**, so over-stitch can also hurt if we fuse pages wrong — but current ICDAR pain is mostly **within-page stream fragmentation**.

### 3.5 Spans

Camelot is good enough on spans for ICDAR TEDS because **the base grid is right**. Our span work fixed synthetic 79/53; ICDAR barely uses that code path. **Do not confuse span success on hard_precision with ICDAR readiness.**

---

## 4. Metric honesty (so we do not chase ghosts)

| Issue | Effect |
|-------|--------|
| Match is **page order**, not IoU | Correct tables wrong order → false structure failure |
| “TEDS” is **difflib SequenceMatcher** on flattened cells × shape factor | Not tree-edit TEDS; empty-cell policies swing scores |
| Comparing to **camelot lattice** alone understates **camelot auto** (network) | us-017: lattice 0 tables, auto excellent |
| Synthetic scoreboard ≠ ICDAR | We already proved product lattice SOTA on synthetic; ICDAR is another product |

Development should use:

- Competitive: keep Camelot metrics for honesty.
- Engineering: **IoU table matching**, method tags, line-count diagnostics, per-doc lattice_fire boolean.

---

## 5. Why “more of the same” cannot produce a major jump

| Lever we keep turning | Why it cannot move ICDAR F1 by +0.15 |
|----------------------|--------------------------------------|
| joint_span_frac / min_joints | Lattice inactive on 61/67 docs |
| Span merge / exclusive assign | Only affects lattice masters |
| Stream gap thresholds | Local trade-off: fix one over-detect, break another under-detect; no contour gate |
| NMS / form scrub | Rearranges soup; does not create missing 26 H lines |
| Synthetic fixture enrichment | Improves regression; can **increase** overfitting to vector grids |

**Major jump requires changing the sensing + routing architecture**, then re-tuning gates once.

---

## 6. Policy: generic development, ICDAR never in the loop

**Hard rules (unchanged product policy):**

| Allowed | Forbidden |
|---------|-----------|
| Build algorithms that are generic (any native PDF) | Tuning thresholds on ICDAR docs or gold |
| Enrich **our** synthetic/real regression corpus with *modes* (painted rules, borderless multi-row, chrome FP) | Copying ICDAR PDFs/XML into `benchmark/corpus/` or tests |
| Optional **post-ship / post-phase** competitive ICDAR black-box run for honesty | Using ICDAR as a development oracle, per-doc fixture, or CI gate |
| Instrument **our** pipeline (method tags, rule counts on **our** docs) | “Instrument ICDAR” as a build step or regression harness |

ICDAR analysis in this doc was a **one-time diagnosis** of *why the competitive score plateaued*. It is **not** a development dataset and **not** a target to overfit.

Camelot’s own whitespace/joint gates were calibrated on real pages; we should derive **equivalent generic gates** from first principles + **our** precision/hard suites (e.g. empty grid reject, min joints, min area), not from ICDAR file names.

---

## 7. Single-shot development plan (generic architecture program)

**Goal:** Close the *capability* gap that competitive eval exposed — weak line sensing, weak borderless structure, weak region/FP gates — via **generic** engines. Success is measured on **our regression suites** (and optional real tier). Competitive ICDAR is a **late, external snapshot only**, never a day-to-day target.

**Non-goals:** Vision models, ICDAR-in-regression, named-doc product hacks, threshold search against competitive metrics.

### What “instrumentation” means (and does **not** mean)

| Do (generic) | Do **not** |
|--------------|------------|
| Tag each emitted table with `method` / notes already in the IR | Add ICDAR-specific counters in the product path |
| On **hard/precision/real** runs: report method mix, rule counts, shapes | Gate merges on ICDAR F1/TEDS |
| Unit tests for rule capture on **synthetic** “painted rule / thin rect” PDFs we generate | Check in external competition PDFs |
| Debug dumps (`dump_geom`, rule stats) for any user PDF | Build fixtures from `us-012.pdf` etc. |

The earlier “Phase 0 = instrument ICDAR” idea is **rejected**. It would couple the harness to the competition set and invite bias. Diagnosis already happened once; we do not need an ongoing ICDAR dashboard to build the right architecture.

### Phase 1 — Line sensing parity (generic lattice unlock)

**1a. Content-path rule completeness (vector)**

- Audit `RuleSegment` capture: thin filled rects as rules, sub-path strokes, `re` rectangles, dashes, clip-stack.
- Coalesce collinear segments (Camelot-like tol as a **generic** constant, not ICDAR-tuned).
- **Generate** synthetic PDFs that stress these modes (original content only) into `hard` / `hard_precision`.

**1b. Raster ruled-line engine (optional module, feature-flagged)**

Port the *ideas* behind Camelot’s lattice front-half (not a Python dependency, not trained on ICDAR):

1. Render page grayscale.
2. Adaptive threshold.
3. Morphological open with page-relative `line_scale` for H/V masks.
4. Optional: stamp vector rules into masks (**combined**).
5. Contours on V+H; joints = V∧H; reject sparse-joint regions; min area fraction of page.
6. Anchors = merge-close joint coords ∪ bbox edges.
7. Fill with existing exclusive text assign + span merge.

**Acceptance (phase 1) — only our corpus / unit tests:**

- New synthetic “visual rules only / thin-rect rules” fixtures: lattice recovers correct shape (not stream fallback).
- Existing hard + precision: cell F1 ≥ 0.98, detect F1 high, no new OVER_DETECT on precision suite.
- Method mix on **those new fixtures**: lattice (or combined), not stream-only.

### Phase 2 — Contour-first proposal + FP gates (over-detect)

Generic, on vector and/or raster masks:

1. Region proposal first (contour / dense joint CC), grid **inside** region only.
2. Gates from first principles + **our** precision suite (70–82 style chrome):
   - min joints per region
   - min area fraction
   - high empty-cell / whitespace reject for near-empty ruled chrome
3. **Routing:** strong lattice band → **hard exclude** stream in that band (not soft demote).

**Acceptance:**

- Precision suite: detect F1 = 1.0, cell F1 ≥ 0.98 (already near-perfect; no regression).
- New multi-chrome / multi-table synthetics: no extra tables from page borders / footer rules.
- Hard suite multi-table shapes still exact.

### Phase 3 — Network-class borderless parser (replace weak stream)

Implement a generic **text-alignment** table parser (`TableMethod::Network` or upgraded stream):

- Stable textlines; alignment edges; table areas; column anchors; min textline count; header spread limits.

**Acceptance (synthetic borderless stress, original content):**

- Multi-row borderless grids (dozens of rows, many cols) recover shape within tight tolerance vs gold.
- Current stream fixtures (07, 59, 82) still pass.
- No reliance on any external competition PDF.

### Phase 4 — Auto routing (generic)

Per page (or per band):

```
if ruled_line_probe (vector and/or morph: enough H and V):
    lattice (combined)
else:
    network
hard region ownership — no dual soup
```

**Acceptance:**

- Orchestration unit/integration tests on synthetic ruled vs borderless pages.
- hard + precision + basic/stress green; method provenance notes correct.

### Phase 5 — Structure polish (only after 1–4)

Spans, stitch policy, stable order — only when base grids are right on **our** suites.

### Optional: competitive snapshot (outside the program loop)

After a major architecture land, **optionally** run external ICDAR once for the honest leaderboard.  
- Black box only (`pdfparser extract --tables`).  
- No code change driven by a single competition file.  
- If the snapshot is still weak, map failures to **generic modes** and add **original** synthetic fixtures covering those modes — never the ICDAR files themselves.

---

## 8. Work breakdown (single program, parallelizable)

| Workstream | Owner skill | Depends on | Est. |
|------------|-------------|------------|------|
| W1 RuleSegment audit + synthetic rule fixtures | content VM | — | 2–4 d |
| W2 Raster lattice front-end | CV + geometry | — | 5–8 d |
| W3 Contour gates + routing | tables orchestrator | W1/W2 | 2–3 d |
| W4 Network parser + borderless synthetics | text layout | — | 6–10 d |
| W5 Auto + regression bakeoff | integration | W2–W4 | 2–3 d |
| W6 Optional external competitive snapshot | eval (manual) | W5 | 0.5 d |

**Critical path:** W1/W2 → W3 → W5. **Parallel:** W4 with W2.  
No “instrument ICDAR” workstream.

Rough calendar: **3–5 focused weeks** one engineer; **2–3 weeks** with two (CV + text).

---

## 9. Decision gates (kill criteria) — regression only

| Gate | Fail action |
|------|-------------|
| After W2, new “visual rule” synthetics still stream-only / wrong shape | Fix sensing before anything else |
| After W3, precision OVER_DETECT / chrome FPs return | Revisit gates; do not ICDAR-tune |
| After W4, large borderless synthetics still 3-row slices | Network incomplete; do not ship auto |
| hard cell F1 < 0.95 or precision cell F1 < 0.98 | Architecture regressed; fix before claiming progress |

Competitive ICDAR numbers are **not** kill criteria for day-to-day development.

---

## 10. What we should stop doing

1. **Stop** using ICDAR (or any competition set) as a development or CI signal.
2. **Stop** micro-tuning lattice knobs without new **generic** failure modes in our corpus.
3. **Stop** soft-demoting stream under lattice without hard region ownership.
4. **Stop** treating synthetic span wins as proof the full table stack is done — enrich corpus with *painted rules* and *large borderless* modes instead.
5. **Stop** copying or naming external competition documents in tests.

---

## 11. One-paragraph summary

Competitive ICDAR exposed a **capability** gap (vector-only lattice rarely applicable; stream too weak; no contour/FP architecture), but **must not become a training or test set**. Develop generically: better rule sensing, optional raster lattice, contour-first regions, network-class borderless, auto routing — all proven on **original synthetic/real regression**. Use ICDAR only as an occasional external honesty check after a phase lands, never as instrumentation, fixtures, or threshold oracles.

---

## Appendix A — Camelot source map (local checkout)

| Piece | Path |
|-------|------|
| Lattice parser | `camelot/parsers/lattice.py` |
| Morph lines / contours / joints | `camelot/image_processing.py` |
| Whitespace reject | `_GRID_WHITESPACE_REJECT = 90` in `lattice.py` |
| Auto flavor probe | `camelot/io.py` `_detect_flavor`, `_parse_auto` |
| Network (borderless) | `camelot/parsers/network.py` |
| Confidence | `accuracy/100 * (1 - whitespace/100)` in `core.py` |

## Appendix B — Baseline numbers to beat

| Tool | F1 | TEDS | Notes |
|------|---:|-----:|-------|
| camelot auto | 0.864 | 0.786 | lattice combined + network |
| camelot lattice/vector | 0.766 | 0.784 | no network |
| pdfparser now | 0.605 | 0.226 | stream-dominated |
| Target (this program) | ≥0.75 | ≥0.55 | min success |
| Stretch | ≥0.82 | ≥0.70 | approach auto |

