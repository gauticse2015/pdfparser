# Table Engine V3 — Industry-Ready Architecture

| Field | Value |
|-------|-------|
| **Title** | Table Engine V3 — Exclusive Page Strategy for Industry-Ready Quality |
| **Date** | 2026-07-15 |
| **Status** | Proposed (supersedes product path of design-table-engine-v2 for *routing & detection discipline*) |
| **Informs** | P0/P1 fix backlog from ICDAR + Camelot analysis |
| **Non-goals** | ICDAR as CI gate; OCR product path; filename hacks |

---

## 1. Diagnosis (why V2 is not enough)

Engine V2 added an exclusive **router on top of candidate soup**. ICDAR still shows:

| Symptom | Value | Root cause |
|---------|------:|------------|
| Pred / GT tables | 432 / 158 (2.73×) | Parallel lattice+network+stream |
| Stream share of emissions | 79% | Borderless path unconstrained |
| OVER_DETECT docs | 52 / 67 | Late FP control |
| TEDS | 0.32 vs Camelot 0.79 | Wrong region + densify + cell fill |
| Rank | #5 | Not a knob problem |

**Camelot auto wins** with: full-page combined sensing → **exclusive page flavor** (lattice XOR network) → contour+joints → precision gates → careful text fill.

**V3 thesis:** Move exclusivity **before** detectors run, make **PageEvidence real**, and put **precision gates at proposal time** — not post-hoc scrub of 400 candidates.

---

## 2. Product goals

### Goals

1. **Industry-ready Auto** for born-digital PDFs: competitive with Camelot/pdfplumber on wild multi-table pages without becoming OpenCV-dependent by default.
2. **Stable detection counts** (pred/GT ≈ 1) before chasing TEDS.
3. **Preserve strengths:** nested multi-table (form+grid), pure-Rust Core tier, security governor, unified IR.
4. **Honest capability ladder** (Core vs Auto+Render vs HQ).
5. **Maintainable:** delete dual paths; ≤12 public options; one orchestrator.

### Non-goals

| Non-goal | Why |
|----------|-----|
| Full-page OCR | Separate product |
| ICDAR #1 as release gate | External honesty only |
| Per-file ICDAR tuning | Policy |
| Replacing text extract quality work | Parallel track (encoding) |
| Becoming Camelot (Python+OpenCV required) | Pure-Rust Core remains default |

### Success metrics (gates)

| Gate | Metric | Target |
|------|--------|--------|
| **G-Det** | real_structure det F1 | ≥ freeze g2 − ε (no regress) |
| **G-Cell** | real_structure cell F1 | ≥ freeze; improve residuals 32/R010 over time |
| **G-Nested** | doc 42 | still 2 tables, cell F1 high |
| **G-Owned** | main/hard multi-lib | stay green |
| **G-ICDAR-ext** | pred/GT ratio (external) | ≤ 1.3× (from 2.73×); F1 ≥ 0.70 aspirational |
| **G-API** | public TableOptions fields | ≤ 12 (+ nested policy struct private) |

---

## 3. Architecture overview

### 3.1 Principle: **Sense → Classify → Build once → Fill → Gate**

```text
PDF page (post-/Rotate text + rules + optional rasters)
        │
        ▼
┌───────────────────┐
│  S0  PageEvidence │  sole algorithm input (text, lines, page stats)
└─────────┬─────────┘
          │
          ▼
┌───────────────────┐
│  S1  Line sensing │  vector ∪ embedded morph ∪ optional full-page render
│      + LineProbe  │  v_count, h_count, joint_proxy, coverage
└─────────┬─────────┘
          │
          ▼
┌───────────────────┐
│  S2  PageStrategy │  Exclusive: Ruled | Partial | Borderless | None
│      (classifier) │  NOT multi-detector soup
└─────────┬─────────┘
          │
          ▼
┌───────────────────┐
│  S3  Builder      │  Exactly one primary builder for strategy
│      (single)     │  + optional nested secondary under hard gates
└─────────┬─────────┘
          │
          ▼
┌───────────────────┐
│  S4  Cell fill    │  Geometry-first grid; text densify NEVER invents
│      + quality    │  structure outside accepted frame
└─────────┬─────────┘
          │
          ▼
┌───────────────────┐
│  S5  Precision    │  whitespace, min cells, prose reject, budget
│      gates        │  nested keep only if complete child grid
└─────────┬─────────┘
          │
          ▼
     Tables (emit order −y1, x0) → optional multipage stitch
```

### 3.2 Module map (keep crate, reshape internals)

```text
pdfparser-tables/
  evidence/     # PageEvidence, LineEvidence  — REAL inputs (not diagnostics-only)
  sensing/      # probe, morph, combined stamp, contour regions  (from raster/)
  strategy/     # PageStrategy classifier + exclusive dispatch
  builders/
    ruled.rs    # contour/joint grid (Camelot-class region model)
    partial.rs  # incomplete frames (today hybrid, gated)
    borderless.rs  # area-first network (replace fragment soup)
  fill/         # cell assign, spans, densify-inside-frame only
  gates/        # whitespace, prose, budget, nested, overlap suppress
  stitch.rs     # unchanged product multipage
  options.rs    # presets only at public surface
```

**Delete / feature-gate after characterization:**

- Classic `stream.rs` product path  
- Legacy soup NMS as default (keep one release as `legacy_router`)  
- Parallel “run all detectors then partition” orchestrator  

---

## 4. Stage designs

### S0 — PageEvidence (sole input)

```rust
// Conceptual API
struct PageEvidence {
    page_index: u32,
    width: f32,
    height: f32,
    text: TextLayer,          // runs + derived textlines + page stats
    lines: LineEvidence,     // oriented segments + provenance
    rasters: Vec<RasterPage>,// embedded and/or full-page
    probe: LineProbe,        // filled in S1
    diagnostics: EvidenceDiagnostics,
}

struct LineProbe {
    n_h: u32,
    n_v: u32,
    n_joints_proxy: u32,     // rough crossings or morph joint samples
    ruled_coverage: f32,     // fraction of page area near H∪V ink
    multi_col_text: bool,    // ≥2 strong column bands
    vector_weak: bool,       // few vector rules relative to text columns
}
```

**Invariant:** builders take `&PageEvidence` only — never re-open PDF, never re-scan raw runs ad hoc.

### S1 — Line sensing ladder

| Tier | Input | Output | Default |
|------|-------|--------|---------|
| **T0 Core** | Vector rules + thin-fill + Form + dash/clip + embedded Image morph | LineEvidence | Always |
| **T1 Render** | Full-page gray (external CLI or optional native later) → morph → **stamp vector into mask** | Stronger LineEvidence | Auto if probe weak **or** HQ preset |
| **T2 OCR** | — | — | Out of scope |

**Normative Auto render decision (industry):**

```text
function need_full_page_render(probe, opts, feature_on) -> bool:
  if opts.enable_full_page_render: return feature_on
  if not feature_on or not opts.allow_auto_render: return false
  # Industry default: render when ruled evidence is ambiguous
  if probe.vector_weak and probe.multi_col_text: return true
  if probe.n_h < 2 or probe.n_v < 2:
      if probe.multi_col_text: return true   # may be painted grid
  return false
```

**Contour regions (normative when any raster mask exists):**

```text
mask = morph_H ∨ morph_V
regions = external contours of mask with area ≥ 0.05% page
for each region:
  joints = crossings inside ROI
  if joints < 5: discard region
  else: RuledRegionProposal(bbox, joints, segments)
```

Contours are **primary region seeds**, not diagnostic notes.

### S2 — PageStrategy (the Camelot-class exclusive classifier)

```text
enum PageStrategy {
  Ruled,        // exclusive: ruled builder only (+ nested ruled children)
  Partial,      // exclusive: partial builder (incomplete frames)
  Borderless,   // exclusive: borderless area engine
  None,         // no tables (or only after explicit user force)
}
```

**Classifier (defaults calibrated on real_structure + geom unit, NOT ICDAR gold):**

```text
function classify(probe, evidence) -> PageStrategy:
  # Strong ruled signal
  if probe.n_h >= 2 and probe.n_v >= 2 and probe.n_joints_proxy >= 5:
      return Ruled
  if has_contour_region_with_joints_ge_5(evidence):
      return Ruled

  # Partial: outer frame or one-direction rules + multi-col text
  if (probe.n_h >= 3 or probe.n_v >= 3) and probe.multi_col_text:
      if incomplete_frame_score(evidence) >= τ_partial:
          return Partial

  # Borderless: multi-col text with 2D alignment support
  if probe.multi_col_text and borderless_prefilter(evidence):
      return Borderless

  return None
```

**Hard rule:** On `Ruled` pages, **do not run** borderless/stream builders at all (except nested keep, §S5).

This single rule is the largest expected ICDAR F1 lift (kills eu-024-class stream FPs).

### S3 — Builders (one primary)

#### S3.1 Ruled builder (rewrite of lattice path)

```text
input: PageEvidence with LineEvidence (+ rasters)
1. Obtain regions:
   - prefer contour seeds when mask present
   - else joint-CC on vector segments (today's lattice core)
2. Per region with joints ≥ 5:
   col_anchors = merge_close(joint.x ∪ bbox L/R)
   row_anchors = merge_close(joint.y ∪ bbox T/B)
3. set_edges from real segments (joint_tol)
4. DO NOT text-densify anchors unless:
   densify_mode == InsideFrameOnly AND missing edge cover < τ
5. emit RuledTable skeleton
```

**Mastered from Camelot:** region → joints → anchors → edges.  
**Kept from pdfparser:** multi-region, exterior stub only when multi-row text sits just outside frame with column consistency.

#### S3.2 Partial builder (today hybrid, constrained)

- Frame + text bands for rows when H sparse  
- **No page-union of all rules** (already removed; keep banned)  
- May promote to Ruled if joints ≥ min after gated densify  
- Exclusive: does not co-emit borderless on same page unless non-overlapping and Partial confidence low

#### S3.3 Borderless builder (replace network soup)

**Area-first algorithm (Camelot network class, Rust-native):**

```text
1. Build textlines; alignment graph (L/C/R × top/mid — at least left+center)
2. Drop nodes with only 1D connectivity (max_h≤1 or max_v≤1)
3. While textline pool non-empty:
   a. seed = most 2D-connected remaining textline
   b. gaps = 2 × P75 strides along seed's alignments
   c. grow bbox; require ≥ MIN_TEXTLINES (default 6)
   d. columns from column boundaries of TLS in body
   e. optional header expand constrained by col anchors
   f. REMOVE consumed textlines from pool
4. Nested/overlap suppress: if area_overlap(A,B) ≥ 0.6 keep larger
5. Prose reject: mean_chars, alpha_ratio, col_sep score floors
```

**Ban product emission of 3×2 sentence grids** unless user preset `AggressiveBorderless`.

### S4 — Cell fill & structure quality

```text
1. Assign textlines to cells by geometry (reading order −y, x)
2. Spanning: shift into master cell (L/T default); blank covered cells
3. Multi-run merge inside cell; split multi-token tabular runs when col grid known
4. Densify: ONLY propose missing H/V *inside* accepted ruled frame when edge cover low
5. Empty-column collapse with colspan preserve
6. Quality scores:
   accuracy ≈ f(assignment residual)
   whitespace = empty_cell_fraction
   confidence = (accuracy/100) * (1 - whitespace/100)   # Camelot-class composite
```

### S5 — Precision gates (proposal-time, mandatory)

| Gate | Rule | Kills |
|------|------|-------|
| **G-joints** | Ruled region joints < 5 → drop | Chrome corners |
| **G-whitespace** | whitespace ≥ 0.90 → drop | Empty frames |
| **G-min-cells** | filled cells < 4 → drop | Noise grids |
| **G-prose** | borderless mean_chars high & numeric_density low → drop | Paragraph FPs |
| **G-budget** | max N tables/page (default 8–12); keep by confidence | eu-013 explosion |
| **G-overlap** | containment ≥ 0.6 → keep larger (except nested keep) | Duplicate fragments |
| **G-nested** | child kept only if: area_ratio ∈ [ρ_min, ρ_max], child joints≥5 or child complete grid, ≥ min_rows | Tiny corner FPs; keep insurance 42 |
| **G-ruled-owns** | On Ruled strategy: zero borderless emissions | ICDAR over-detect |

**Nested multi-table (pdfparser strength, keep):**

```text
if parent Ruled and child Ruled and is_nested_table_pair(parent, child):
  if child passes G-nested completeness:
    keep BOTH
  else:
    drop child
```

---

## 5. End-to-end page algorithm (normative pseudocode)

```text
function extract_tables_page(page_inputs, opts) -> Vec<Table>:
  if not opts.detect_tables: return []

  ev = build_page_evidence(page_inputs)           # S0
  ev = sense_lines(ev, opts)                      # S1 (may render)
  strategy = classify_page(ev)                    # S2

  tables = []
  match strategy:
    Ruled:
      tables = build_ruled(ev, opts)
      tables = apply_nested_ruled_children(tables, ev, opts)
    Partial:
      tables = build_partial(ev, opts)
    Borderless:
      tables = build_borderless(ev, opts)
    None:
      tables = []

  tables = precision_gates(tables, strategy, opts) # S5
  tables = sort_emit_order(tables)                 # (−y1, x0)
  return tables

function extract_tables_document(...):
  per_page = map extract_tables_page
  if opts.stitch_multipage:
    return stitch(per_page)
  return per_page
```

**Contrast with today:** no lattice+hybrid+network+stream all feeding one NMS.

---

## 6. Public API (industry surface)

### Presets only

| Preset | Strategy bias | Render | Use |
|--------|---------------|--------|-----|
| **Off** | — | — | Default tables off |
| **Auto** | S2 classifier | Opportunistic T1 | Product default |
| **LatticeOnly** | Force Ruled | Off unless needed | Batch ruled corpora |
| **HighQuality** | Auto | Always T1 if feature on | Best quality |
| **Fast** | Auto, no render, lower budgets | Never | Latency |

### Public options (target ≤ 12)

```text
detect_tables, preset,
enable_full_page_render, allow_auto_render,
stitch_multipage,
max_tables_per_page,
min_confidence,          # single floor
aggressive_borderless,   # opt-in weak grids
legacy_router,           # one minor deprecation only
```

All lattice/stream/raster numeric knobs → **private `DetectionPolicy`** with versioned defaults. Changing policy requires real_structure A/B artifact.

---

## 7. Migration plan (no big-bang rewrite)

### Phase A — Detection discipline (1–2 weeks) — **largest ROI**

Implement **without** full module rename:

1. **Ruled-owns-page hard switch** in orchestrator: if strong lattice (≥1 table meeting joint/conf gates), **do not run** network/stream (except nested child lattice).  
2. **Raise borderless floors:** min textlines, prose reject, drop conf &lt; 0.65.  
3. **Whitespace gate** on lattice emit.  
4. **Per-page budget** after route.  
5. Disable classic stream by default (`feature = "classic-stream"`).  

**Exit:** ICDAR external pred/GT ≤ 1.5×; real freeze holds; nested 42 holds.

### Phase B — Sensing + contours (2–3 weeks)

1. Contour seeds enter ruled region proposals (not diagnostic-only).  
2. Combined stamp always when raster present.  
3. Auto render probe when vector weak (feature-on builds).  
4. Joints &lt; 5 drop.

**Exit:** lattice fire rate up on painted pages; HQ path documented.

### Phase C — Borderless rewrite (2–3 weeks)

1. Area-grow + textline consumption + nested suppress.  
2. Delete product dual stream path.  
3. Partial builder isolation.

**Exit:** borderless ICDAR pages fewer FPs; real stream docs (donors, etc.) hold.

### Phase D — Fill quality + encoding (parallel)

1. Cell fill / spans / densify-inside-frame only.  
2. Font/encoding path for R010-class.  
3. Dense numeric (doc 32).

### Phase E — API cleanup

1. Knob diet; remove EngineV2/Full/LatticeStream redundancy.  
2. Delete legacy router after one minor.  
3. PageEvidence-only builder signatures.

---

## 8. Industry readiness checklist (beyond tables)

Tables alone do not make an industry PDF library. Parallel tracks:

| Track | Minimum for “industry ready” |
|-------|------------------------------|
| **Tables V3** | This doc Phases A–C |
| **Text** | Encoding/CID robustness; no silent mojibake on common gov PDFs |
| **Security** | Governor + encrypted open (password) for enterprise |
| **API** | Stable presets; semver; no 57 knobs |
| **Eval** | real_structure n≥50; freeze in CI; ICDAR quarterly external |
| **Ops** | Latency tiers (Fast/Auto/HQ); no surprise multi-second render |
| **Docs** | Honest capability ladder; no false “Production” on partial features |

---

## 9. Risk register

| Risk | Mitigation |
|------|------------|
| Ruled-owns-page drops valid borderless beside lattice | Nested + non-overlap exception only if borderless passes high gates |
| Render dependency surprises embedders | Feature + allow_auto_render hard off; Fast preset |
| Owned synthetic regresses | Keep geom_unit CI; don’t optimize only ICDAR |
| Nested 42 breaks | Explicit G-nested tests in CI |
| Over-fitting classifier to real n=15 | Expand gold before freezing strategy constants |

---

## 10. Comparison summary

| | Camelot auto | pdfparser today | pdfparser V3 (this) |
|--|--------------|-----------------|---------------------|
| Page strategy | Exclusive flavor | Parallel soup + router | **Exclusive strategy** |
| Sensing | Always full-page combined | Optional / weak | **Ladder T0+opportunistic T1** |
| Regions | Contour + joints≥5 | Joint-CC + densify | **Contour-primary + joints** |
| Borderless | Area grow, min 6 TLS | Fragment stream | **Area engine + gates** |
| FP control | Early reject | Late scrub | **Proposal-time gates** |
| Nested forms | Weak | Strong | **Keep + stricter child** |
| Pure Rust default | No | Yes | **Yes (T0)** |

---

## 11. Decision record

| ID | Decision |
|----|----------|
| V3-K1 | Exclusivity **before** builders, not only after candidates |
| V3-K2 | PageEvidence is sole builder input |
| V3-K3 | Contours primary when raster exists |
| V3-K4 | Densify never primary structure outside frame |
| V3-K5 | Classic stream not in product Auto |
| V3-K6 | Whitespace ≥ 90% and joints &lt; 5 reject ruled noise |
| V3-K7 | Nested keep retained with completeness gates |
| V3-K8 | ICDAR external honesty only; CI = real_structure + owned geom |
| V3-K9 | Public API = presets + ≤12 fields |
| V3-K10 | Phase A (ruled-owns + stream kill) ships before any “V3 rewrite” branding |

---

## 12. Immediate next implementation tickets (Wave 1)

1. `orchestrator`: if `has_strong_ruled(page)` skip network/stream entirely  
2. `gates`: whitespace reject on lattice tables  
3. `network`: min textlines + prose hard reject  
4. `options`: default disable classic stream; budget cap  
5. External ICDAR re-run (pred count dashboard)  
6. real_structure freeze CI job  

---

*This document is the architectural answer to: “How do we stop losing to Camelot on discipline while staying a Rust-native product?” Implement Phase A before larger refactors.*
