# pdfparser Table Engine Re-Architecture

| Field | Value |
|-------|-------|
| **Title** | Table Engine Re-Architecture — Production-Ready Path to Real Quality |
| **Author** | pdfparser contributors |
| **Date** | 2026-07-12 |
| **Status** | Draft (rev 2.3 — gap closure: capability ladder, contour/combined sensing, borderless areas, cell quality, updated plan) |
| **Supersedes** | `docs/icdar-plateau-analysis-and-plan.md`, `docs/table-orchestrator-architecture.md` (orchestration section), phase-*-report table notes |
| **Living path** | `docs/design-table-engine-v2.md` |
| **Diagrams** | **ASCII only** (no Mermaid) — same convention as `design-native-pdf-parser.md` / `docs/README.md` |
| **Workspace** | `/Users/gautamkumar/Desktop/pdfParser` |

---

## Overview

pdfparser's table stack is a capable **vector-grid product** that scores near-perfectly on synthetic ReportLab fixtures (owned multi-lib harness: cell F1 ~0.98 main, ~0.83 compete_hard) yet plateaus on real, ICDAR-like PDFs (external competitive F1 ~0.58, TEDS ~0.33, rank #5). Diagnosis is architectural, not knob-level: lattice is silent on most real pages (no full-page line sensing; Form XObjects and painted rules incomplete); borderless network is **left-edge–primary** (limited center fallback); exclusive routing exists **only when strong lattice exists**, so weak/missing ruled evidence falls through to parallel weak detectors + post-hoc scrub; and the evaluation corpus co-evolved with the algorithms.

This design proposes a **SOLID, trait-based table engine** with a **capability ladder** (Core vs Auto+Render), **contour-first ruled regions** when raster exists, **combined vector∪raster sensing**, a true **borderless table-area engine**, exclusive routing with **vertical proposal merge**, a **cell-quality** pass, and a **corpus + evaluation program** (struggle-first real gold, synthetic demoted). Gaps that kept the product non-market-ready are closed as **normative requirements** (rev 2.3), not optional polish. Diagrams are **ASCII only**.

**Migration invariant (orchestrator/builders/router only):** product `TablePreset::Auto` keeps today's **legacy orchestrator, builders, and routing** until a **named real_structure gate** (G1) passes; new builders run in **shadow** then **opt-in `EngineV2`** only. Auto flips only after that gate—not when algorithm PRs merge. **Sensing completeness** (Form/dash/clip rules, optional full-page render) may still change Auto table outputs under that frozen orchestrator; that is intentional and not a violation of the invariant (see Migration Stages).

---

## Background & Motivation

### Current architecture (facts)

Workspace crates: `pdfparser-core`, `pdfparser-content`, `pdfparser-fonts`, `pdfparser-layout`, `pdfparser-tables`, `pdfparser-export`, `pdfparser-ir`, `pdfparser` façade, `pdfparser-cli`.

| Component | Path | ~LOC | Role today |
|-----------|------|-----:|------------|
| Content VM | `crates/pdfparser-content/src/vm.rs` | ~726 | Strokes + thin fills as `RuleSegment`; curves discarded; `Do` records image placements only; **no Form XObject content expansion for rules**; dash op ignored |
| Extract | `crates/pdfparser/src/extract.rs` | ~232 | Page interpret → runs + rules + image placements → tables; applies `/Rotate` to runs and rules |
| Raster images | `crates/pdfparser/src/raster_images.rs` | ~205 | Decode **embedded Image XObjects** only → `RasterPage` (walks Form **resources** for nested images, does **not** interpret Form content streams) |
| Lattice | `crates/pdfparser-tables/src/lattice.rs` | ~2171 | Multi-region **joint-CC** of H∩V segments; densify X/Y from text as primary structure fill-in; joint-span filters; exclusive text assign |
| Network | `crates/pdfparser-tables/src/network.rs` | ~984 | Textlines + **left-edge–primary** column anchors (limited center snap fallback); gap/schema region split |
| Stream | `crates/pdfparser-tables/src/stream.rs` | ~938 | Classic whitespace stream (dual path; product uses network when `modes.stream`) |
| Hybrid | `crates/pdfparser-tables/src/hybrid.rs` | ~656 | Outer frames + **union-of-all-rules frame fallback** (`union_rules_frame`) + text |
| Raster morph | `crates/pdfparser-tables/src/raster/` | ~1517 | Camelot-class morph on embedded images |
| Orchestrator | `crates/pdfparser-tables/src/lib.rs` | ~426 | lattice → hybrid outside strong lattice → network outside strong lattice → side-by-side split → form scrub + NMS |
| Options | `crates/pdfparser-tables/src/options.rs` | ~271 | **~50 public fields** on `TableOptions` (re-exported by façade) |

### Product Auto path (today)

```text
page rules + text + embedded Image XObjects
    → raster morph (Image XObject → H/V rules, grid-validated)
    → lattice (vector + thin-fill + raster rules)
    → hybrid only outside strong lattice
    → network borderless only outside strong lattice
    → side-by-side empty-gutter split
    → form scrub + NMS
```

**Existing exclusive model** (`exclusive_under_strong_lattice`, default true): hard-drop network when a table is `is_strong_lattice` (Lattice method, ≥2×2, conf ≥ `strong_lattice_min_conf`, not `weak_edges`); hybrid only outside those bboxes (overlap ≥0.40 or IoU ≥0.35). This is **not** always-on dual soup when lattice is strong.

What fails on real pages:

1. **Strong lattice rarely exists** (~6/67 on external ICDAR snapshot) → network/hybrid still invent wrong grids; exclusive never engages.
2. Hybrid **union_rules_frame** page-union fallback invents grids from chrome rules.
3. FP control is largely **post-hoc** (form scrub, NMS, soft demotion) rather than proposal-time region ownership.

### Competitive reality (external only — not a development oracle)

| Fact | Value |
|------|------:|
| ICDAR docs with any lattice emission | ~6 / 67 |
| Stream/hybrid dominated docs | ~57 / 67 |
| pdfparser competitive F1 / TEDS | 0.584 / 0.333 |
| Regression after product improvements | F1 0.672 → 0.584 (overfit signal) |
| Dominant failures | OVER_DETECT, ROW_MISCOUNT, wrong shape from wrong method |

These numbers motivate architecture; **they are not CI gates, tuning targets, or corpus contents.** Implementers must not re-open ICDAR as a development loop (see CONTRIBUTING + `assert_no_icdar.py`).

Camelot wins with: full-page morph ± vector stamp → contour regions → joint gates → exclusive auto (lattice **or** network).

### Root causes (not "need more knobs")

1. **No full-page line sensing** — only embedded images + content-stream rules; painted grids without Image XObjects never enter lattice.
2. **Weak borderless** — left-edge–primary textlines ≠ multi-alignment network (L/C/R) with table-area proposals and structure quality scores.
3. **Exclusive only when strong lattice exists** — weak/missing ruled evidence falls through to parallel weak detectors + post-hoc scrub (`exclusive_under_strong_lattice` / `is_strong_lattice` is the incomplete model to replace with proposal-time region ownership).
4. **Synthetic corpus co-evolved** with algorithms; hard suite circular; real gold soft (`accuracy_mode: weak_real`, `soft_gold: true`, empty cell grids on real tracks).
5. **~50 public magic knobs**; densify-from-text as **primary** structure source is double-edged.

### Pain points for maintainers

- Tuning one suite regresses another; ICDAR F1 fell while synthetic rose.
- `lattice.rs` is a monolith mixing sensing merge, densify, spans, gates.
- Dead/dual paths: classic `stream.rs` still `pub use` while production uses network under `modes.stream`.
- Docs sprawl; no real-PDF structure-quality track that is licensed and in-repo for CI.
- CI (`.github/workflows/ci.yml`) is `cargo test` + CLI text smoke only—**no accuracy job**.

---

## Goals & Non-Goals

### Goals

1. **Re-architect** the table engine around SOLID traits: evidence layers, builders, exclusive router.
2. **Unify line evidence** (vector + optional raster); joint graph; proposal-time gates; joint-CC refactor first, optional raster contours later.
3. **Rewrite borderless** as multi-alignment network with structure quality gates.
4. **Exclusive Auto routing** with hard region ownership—**after** real_structure gate; product Auto unchanged until then.
5. **Optional full-page raster** behind trait + compile feature + **runtime** opt-in (never auto from compile alone).
6. **Corpus program (K23):** demote/archive happy-path and solved synthetic bulk; grow **real / hard-real** PDFs with T1–T3 gold and mode coverage matrix; synthetic only as small `geom_unit`.
7. **Phased real-PDF evaluation track** as the **only** primary quality signal for ship claims; synthetic multi-lib not merge-blocking.
8. **Doc cleanup** + honest README scoreboard policy; **ASCII diagrams only**.
9. **Migration stages:** shadow → `EngineV2` opt-in → Auto flip only after named real gate.
10. **Knob diet** with explicit deprecation/compat plan for façade `TableOptions`.

### Non-Goals

| Non-goal | Rationale |
|----------|-----------|
| Vision models / VLM as v1 table core | Latency, deps, determinism |
| ICDAR files in repo, CI, or tuning | Hard product policy |
| Per-file or named-doc accuracy hacks | Violates CONTRIBUTING |
| Full-page OCR for scans as v1 | Optional later |
| Beating Camelot on ICDAR as a CI gate | External honesty snapshot only |
| Instant breakage of every `TableOptions` field | 0.1.x allows evolution with **documented deprecation** (see API stability) |

### Hard constraints

1. Free re-architecture; break free from fine-tuning trap.
2. New test suite; clean docs; keep performance-measurement docs.
3. Focus on **real PDFs** for performance measurement.
4. **NO ICDAR** in pipeline testing / corpus / tuning / file-oriented code.
5. Generic, expandable, SOLID, production-ready algorithms.
6. README honest.
7. No accuracy-oriented / file-oriented code changes.

---

## Key Decisions

| # | Decision | Rationale |
|---|----------|-----------|
| K1 | **Keep workspace crate graph**; restructure *inside* `pdfparser-tables` (`evidence/`, `builders/`, `router/`, `policy/`). | Cargo churn avoidance; SRP via modules |
| K2 | **`PageEvidence` sole algorithm input** (text + lines + regions + diagnostics). | Testable; sensing decoupled from builders |
| K3 | **`LineEvidence` unifies vector + raster** with provenance; merge once. | One joint graph |
| K4 | **Capability ladder** (see § Capability Ladder): pure-Rust **Core** (vector+Form+embedded morph) always; **full-page render** is feature-gated but **post-G1 Auto may opportunistically render** when vector probe is weak and feature is compiled (K25). Compile feature alone never renders. | Market ceiling without always-on native deps |
| K5 | **Exclusive AutoRouter** (Ruled > Partial > Borderless) with scored proposals + partition + **cross-kind vertical merge** (K26). | Replace incomplete strong-lattice-only exclusive |
| K6 | **Ruled region proposal is dual-mode:** (A) joint-CC on vector segments; (B) when raster mask exists, **ink contour / morph-CC is primary region seed**, joints/anchors inside contour (Camelot-class). Joint-CC alone is not enough for painted pages. | Close contour-first gap |
| K7 | **Borderless = table-area engine** (area proposal + L/C/R + multi-line cells + fragment re-merge), not left-edge textlines with a score. Densify-from-text remains gated secondary on ruled only. | Close network-class gap |
| K8 | **Public knobs → presets + policy**; old fields retained deprecated with mapping for one minor (see API stability). | Compat for embedders |
| K9 | **Real-PDF eval primary**; synthetic geom only; ICDAR external never CI. | Break co-evolution |
| K10 | **Classic stream** deprecated in PR1; feature-gated; delete only after call-site zero + characterization. | Safety net |
| K11 | **No ICDAR** in repo/CI/tuning; mode tags not file names; early `assert_no_icdar.py`. | Policy enforcement |
| K12 | **Migration stages:** (1) shadow diagnostics, (2) `TablePreset::EngineV2` opt-in, (3) flip Auto **only** after real gate G1. | Product Auto preserved |
| K13 | **CI policy:** cargo tests always; `assert_no_icdar` early; real detect smoke after PR6a; structure cell metrics after core≥15; ε freezes versioned. | Concrete gates |
| K14 | **`tracing` optional** in `pdfparser-tables` behind feature `trace` (default off) **or** diagnostics-only structs without new dep in PR1; façade may log. Prefer **no new required dep** in PR1—structured fields on `Table` / `EvidenceDiagnostics` first. | Keep tables lean |
| K15 | **Stitch after per-page detect** remains **product** default; exclusive ownership is **per-page**. **Eval path** (`real_structure`, competitive scripts) forces `stitch_multipage=false` (K27). | Product vs eval decoupling |
| K16 | **PageEvidence is post-`/Rotate`** page space (same as today's extract rotation of runs/rules). | Matches current IR |
| K17 | **`TableMethod::PartialRuled` additive**; keep `Hybrid` as serde alias / deprecated synonym until 0.2. | Avoid silent JSON break |
| K18 | **Auto flip gate G1** (named): real_structure core ≥ **15** docs with cell grids; detect smoke ≥ **20** docs; IoU det F1 and cell F1 ≥ frozen baseline − ε; shadow comparison artifact reviewed; `EngineV2` default for Auto then. | Explicit flip criteria |
| K19 | **Form expansion via `FormContentResolver`** injected from façade; VM never opens the PDF graph alone. | Matches interpret_page limits today |
| K20 | **Latency:** probe set + p50/p95 from early timers; README latency claims only after rebaseline post-PR5; not pass/fail Success Criteria until baselined. | Methodology before claims |
| K21 | **Migration freeze scope** = orchestrator + builders + router identity, **not** full sensing bit-identity. PR2/PR3 may change Auto outputs under legacy routing. | Avoid false rejections or gating Form behind EngineV2 |
| K22 | **G1 freeze bootstrap:** first write of `g2.json` is a reviewed EngineV2 snapshot (Boot); later PRs regress vs freeze−ε (Steady); intentional rebaseline is manual + changelog. | Avoid tautological self-compare / undefined first baseline |
| K23 | **Corpus is product infrastructure:** demote/remove happy-path synthetic from *primary* quality claims; grow **real / hard-real** PDFs with mode-tagged gold as the only path to Gate G1 and README scoreboards. Synthetic stays unit/regression only. | Break self-approval loop |
| K24 | **Diagrams in this doc are ASCII only** — no Mermaid (does not render reliably in this repo’s IDE/review path). | Match existing design docs |
| K25 | **Opportunistic full-page render (post-G1 Auto, feature-on builds):** if `vector_line_probe` weak AND multi-col text strong AND `full-page-render` feature compiled, Auto may set one-shot render for that page (fail-soft). Embedders can force off via `enable_full_page_render=false` + `allow_auto_render=false`. Pure-Rust builds skip. | B1 market gap |
| K26 | **Proposal vertical merge:** before partition finalize, merge proposals with same kind (or Ruled+Partial) when x-IoU≥0.6, y-gap ≤ 1.5×median_line_gap, compatible col schema. Prevents header/body split. | C1 |
| K27 | **Emit order normative:** sort final tables by (−y1, x0) page space (top-to-bottom, then left). real_structure harness disables stitch. | C2/C3 |
| K28 | **Combined sensing:** when raster page present, stamp vector H/V into binary mask (OR) before morph/contours. | B3 |
| K29 | **Full-page morph FP policy:** suppress segments collinear with text baselines (tol 0.35×fs); min_seg ≥ max(8px, 0.08×region_width); chart-axis reject retained. | C5 |
| K30 | **Cell quality program (PR5c):** multi-run merge into cells, span blanks, boundary exclusive assign; not free with lattice extract. | B5 |
| K31 | **Structure-core selection = struggle-first:** prefer docs where **current Legacy Auto** fails shape or count; forbid cherry-picking only easy wins for G1. | D self-approval |
| K32 | **Peer honesty on real_structure:** optional bakeoff Camelot/plumber on same real core (not ICDAR); report in README secondary. | D |
| K33 | **G1 algorithm bar (in addition to C5):** EngineV2 must include PR2a + (PR2b **or** K25 render path available) + PR4 contour-when-raster + PR5 table-area borderless + K26 merge unit tests. | Avoid flip on skeleton only |
| K34 | **PartialRule:** row model = frame + text bands (not densify-as-primary); no page-union; may promote to Ruled if joints≥min after densify-gated. | B6 |
| K35 | **Policy constant changes** require real_track A/B artifact (method_mix + det/cell delta); no silent threshold churn. | Avoid knob soup 2.0 |
| K36 | **Pre-G1 product messaging:** recommend `EngineV2` (+ HighQuality when render built) for quality; Auto remains legacy until G1; publish real smoke numbers for EngineV2. | B7 |

---

## Capability Ladder (market claims)

Product quality is **not** one binary. Claims and defaults must match what sensing can do.

```text
  ┌─────────────────────────────────────────────────────────────────────────┐
  │ Tier 0 — Core (always; pure Rust; product Auto base sensing)            │
  │   Vector rules + thin-fill + Form expansion + dash/clip (PR2)           │
  │   + embedded Image XObject morph                                        │
  │   Claim: "Strong on born-digital PDFs with real vector/image rules"     │
  └─────────────────────────────────────────────────────────────────────────┘
                                    │
                    vector probe weak + multi-col text?
                                    │ yes, if feature compiled (K25)
                                    ▼
  ┌─────────────────────────────────────────────────────────────────────────┐
  │ Tier 1 — Auto+Render (post-G1 Auto opportunistic OR HighQuality)        │
  │   Tier 0 + full-page gray render → combined mask morph → contours       │
  │   Claim: "Competitive on painted/faint ruled pages when render enabled" │
  └─────────────────────────────────────────────────────────────────────────┘
                                    │
                         scanned / no extractable text
                                    ▼
  ┌─────────────────────────────────────────────────────────────────────────┐
  │ Tier 2 — OCR (explicit non-goal for v1 table core)                      │
  │   Full-page scan tables; image text without PDF text operators          │
  │   Claim: out of scope until separate OCR track                          │
  └─────────────────────────────────────────────────────────────────────────┘
```

| Preset | Router | Render | Notes |
|--------|--------|--------|-------|
| `Fast` | EngineV2 lite / lattice-prefer | never | Latency first |
| `Auto` (post-G1) | Exclusive AutoRouter | **Opportunistic** (K25) if feature on | Product default |
| `Auto` (pre-G1) | **Legacy** soup | never (unless user opts) | Frozen migration |
| `EngineV2` | Exclusive AutoRouter | off unless `enable_full_page_render` | Opt-in pre-G1 |
| `HighQuality` | Exclusive AutoRouter | **on** if feature compiled | Best effort |

**README must never claim Tier-1 quality for pure Core-only builds.**

### Opportunistic render probe (normative, K25)

```text
function may_auto_render(page, opts, feature_on) -> bool:
  if not feature_on: return false
  if opts.allow_auto_render == false: return false   # embedder hard off
  if opts.enable_full_page_render: return true       # explicit on
  # opportunistic (Auto post-G1 only):
  n_h, n_v = count axis rules with len >= min_seg
  vector_weak = (n_h < 3 or n_v < 3) and joint_count_page < min_joints_ruled
  text_strong = multi_col_textlines >= 6 and text_score_page >= 0.45
  return vector_weak and text_strong
```

Fail-soft: render error → diagnostics only; continue Core sensing.

---

## Closed Design Gaps (rev 2.3)

Gaps from pre-development review are **normative requirements**, not backlog ideas.

| ID | Gap | Resolution in this design |
|----|-----|---------------------------|
| B1 | Full-page optional → Auto still fails painted rules | K25 opportunistic Auto+Render; ladder; HQ |
| B2 | Joint-CC only, not contour-first | K6: raster contour primary when mask exists |
| B3 | No combined vector∪raster mask | K28 combined stamp before morph |
| B4 | Borderless still left-edge network | § Borderless table-area engine |
| B5 | No cell-assign / span program | K30 PR5c |
| B6 | Partial = recycled hybrid | K34 partial row model |
| B7 | Auto legacy long time / no visible quality path | K36 EngineV2 messaging + real smoke numbers |
| C1 | Header/body dual proposals | K26 vertical merge |
| C2 | Table order | K27 emit order |
| C3 | Stitch vs per-page gold | K15/K27 eval stitch off |
| C4 | Dash/clip slip | G1 bar K33 needs PR2a + (2b or render) |
| C5 | Full-page false lines | K29 morph FP policy |
| C6 | Text quality | Structure docs require extractable-text bar (§ Corpus) |
| C7 | Knob soup 2.0 | K35 A/B for policy changes |
| C8 | Nested/rotated | Explicit non-goal + README |
| C9 | OCR | Tier 2 non-goal |
| D | Easy core selection | K31 struggle-first |
| D | No peer check | K32 real_structure bakeoff |

---

## Migration Stages (product Auto preserved)

```text
  [start]
     │
     ▼
 ┌──────────────────┐
 │ M0  Legacy Auto  │  TablePreset::Auto == today's soup
 │     (default)    │
 └────────┬─────────┘
          │  PR1+: shadow diagnostics only (no builder swap)
          ▼
 ┌──────────────────┐
 │ M1  Shadow       │  Legacy Auto frozen (orchestrator/builders/router)
 │                  │  Evidence + method_mix dumps parallel
 └────────┬─────────┘
          │  PR4b/PR5 + PR6a ready
          ▼
 ┌──────────────────┐
 │ M2  EngineV2     │  TablePreset::EngineV2 opt-in only
 │     opt-in       │  Auto still Legacy
 └────────┬─────────┘
          │  Gate G1 green (real_structure core ≥ 15)
          ▼
 ┌──────────────────┐         rollback: legacy_router /
 │ M3  Auto == V2   │◄─────── env PDFPARSER_TABLE_LEGACY=1 /
 │     product      │         CLI --tables-legacy
 └────────┬─────────┘
          │  ≥1 minor + characterization
          ▼
 ┌──────────────────┐
 │ M4  Delete legacy│  soup adapters removed
 │     adapters     │
 └──────────────────┘
```

| Stage | When | Orchestrator / builders / router | Sensing inputs | New builders |
|-------|------|----------------------------------|----------------|--------------|
| **M0 Legacy** | today → PR1 | Current `lib.rs` orchestrator | As today | — |
| **M1 Shadow** | PR1 onward (until M3) | **Frozen** legacy orchestrator path (same call graph as M0 soup) | May improve via PR2/PR3 (see below) | Parallel only; diagnostics / dump JSON; **no** builder/router substitution on Auto |
| **M2 EngineV2 opt-in** | After PR4b/PR5 + PR6a | Auto still legacy orchestrator | Shared evidence/sensing | `TablePreset::EngineV2` uses exclusive router + new builders |
| **M3 Auto flip** | Gate **G1** green on main | Auto == EngineV2 policies | Shared | Legacy behind `legacy_router` |
| **M4 Delete legacy** | ≥1 minor after M3 + characterization | Auto only | Shared | Remove soup adapters |

### Migration invariant scope (normative)

**What is frozen until G1 (M0–M2 Auto):**

- Detector **orchestration**: order and exclusivity rules of the **legacy** path (`legacy/orchestrator.rs` / today's `detect_tables_page_with_raster` soup).
- **Builder implementations** wired into Auto (legacy lattice / hybrid / network adapters)—no Auto switch to Ruled/Borderless/Partial EngineV2 builders.
- **Router**: no exclusive AutoRouter partition as the product Auto path.

**What is *not* frozen (sensing may change Auto outputs pre-G1):**

- Vector rule completeness: Form XObject expansion, dash, clip, optional curve chords (PR2a–d).
- Optional full-page raster when runtime-enabled (PR3); default Auto still has render off.
- Embedded morph knobs only if intentionally changed (prefer not; keep morph behavior stable unless separate changelog).

Form expansion is **not** gated behind `EngineV2`. Prefer:

```text
InterpretOptions::expand_form_xobjects = true  // when capture_rules / table path
// optional one-PR bake: default false → true with changelog; not EngineV2-only
```

**Sensing PR acceptance (PR2a–d, PR3 when enabled on a path):**

1. **Must not** change orchestrator/builder/router wiring for Auto (invariant holds).
2. **May** change table lists / digests because `RuleSegment` / raster inputs grew—**intentional sensing improvement**.
3. On intentional Auto delta: **update** `legacy_parity_fixtures` digests in the same PR (or a stacked digest-refresh commit), **changelog** the behavior, and attach **shadow method-mix / real_detect_smoke** comparison (pre vs post sensing)—do **not** reject solely because Auto outputs moved.
4. Unit tests still cover Form depth/cycle and “rules appear from Form content.”

**Builder/router PR acceptance (PR4/PR5):** still requires Auto **orchestrator path** parity on freeze fixtures when sensing inputs are held constant (same rules fixture injection / golden rules dumps). Prefer fixture digests that pin **post-interpret rules** where possible so PR4a is not conflated with PR2.

### Rollback API (normative)

| Mechanism | Binding | Notes |
|-----------|---------|-------|
| `TableOptions.legacy_router: bool` | default `false` after M3; **ignored before M3** (Auto always legacy) | Primary library switch |
| Env `PDFPARSER_TABLE_LEGACY=1` | Read once in façade `extract` / CLI when building options | Overrides preset to force legacy orchestrator |
| CLI `--tables-legacy` | Sets `legacy_router` | Documented in CLI help |
| `TablePreset::EngineV2` | Opt-in before M3 | Experimental; not default |
| Deprecation window | **one minor release** after M3 (e.g. 0.1.x → next 0.1.y or 0.2) | Then remove env/CLI if desired |

### Parity suite (day-to-day product freeze)

While Auto is legacy (M0–M2):

- **Freeze artifact:** `benchmark/real_track/legacy_parity_fixtures.json` — synthetic + a few real detect-only (**not** ICDAR). Digests = table count, method tags, shape hashes **for a fixed orchestrator path**.
- **PR1 / PR4a / PR5 (Auto path):** bit-identical or digest-identical vs baseline **when rule/raster inputs are unchanged**. These PRs must not substitute EngineV2 builders into Auto.
- **PR2/PR3 sensing:** digests **may** be refreshed intentionally (see invariant scope); cargo geom unit tests for pure geometry remain green; real smoke + method-mix are the quality signal.
- EngineV2 quality judged by **shadow comparison JSON** + real smoke metrics—not synthetic cell F1 leaderboards.

---

## Proposed Design

### High-level architecture

```text
  ┌──────────────────────────────────────────────────────────────────────────┐
  │  pdfparser façade (extract.rs)                                           │
  │                                                                          │
  │   page_content  ──▶  FormContentResolver (PR2)                           │
  │        │                   │                                             │
  │        │                   └── form streams → extra rules/text           │
  │        ▼                                                                 │
  │   runs + RuleSegments + ImagePlacements                                  │
  │        │                                                                 │
  │        ├── RasterLineProvider ──▶ EmbeddedImageProvider (default)        │
  │        │                      └── FullPageRenderProvider (optional)      │
  │        ▼                                                                 │
  │   PageEvidence assembly (post-/Rotate page space)                        │
  └───────────────────────────────────┬──────────────────────────────────────┘
                                      │
                                      ▼
  ┌──────────────────────────────────────────────────────────────────────────┐
  │  PageEvidence                                                            │
  │                                                                          │
  │   TextLayer (runs / textlines / L·C·R peaks)                             │
  │   VectorLineLayer  ──┐                                                   │
  │   RasterLineLayer  ──┼──▶ LineEvidence (merge, snap, provenance)         │
  │                      │                                                   │
  │   RegionProposer ◄───┘   joint-CC · partial bands · borderless bands     │
  └───────────────────────────────────┬──────────────────────────────────────┘
                                      │ proposals (may overlap)
                                      ▼
  ┌──────────────────────────────────────────────────────────────────────────┐
  │  TableEngine                                                             │
  │                                                                          │
  │   AutoRouter  ── partition + ownership (Ruled > Partial > Borderless)    │
  │        │                                                                 │
  │        ├── RuledContour   ──▶ RuledTableBuilder                          │
  │        ├── PartialRuled   ──▶ PartialRuleBuilder                         │
  │        └── BorderlessText ──▶ BorderlessTableBuilder                     │
  │                                                                          │
  │   Exactly one builder per owned region — no dual soup                    │
  └───────────────────────────────────┬──────────────────────────────────────┘
                                      │ per-page tables
                                      ▼
  ┌──────────────────────────────────────────────────────────────────────────┐
  │  Post (document)                                                         │
  │   side-by-side split residual · form scrub residual · NMS · stitch       │
  └──────────────────────────────────────────────────────────────────────────┘

  Product Auto today (until G1): legacy/orchestrator.rs soup path only.
  EngineV2 path above is opt-in until Gate G1 flips Auto.
```

### SOLID module layout (`pdfparser-tables`)

```text
crates/pdfparser-tables/src/
  lib.rs
  types.rs
  options.rs              # TablePreset + TableOptions (compat fields)
  policy/
    lattice_policy.rs
    borderless_policy.rs
    partial_rule_policy.rs
    proposal_policy.rs
    raster_policy.rs
  evidence/
    page_evidence.rs
    text_layer.rs
    line_evidence.rs
    region_proposal.rs
  providers/
    line_source.rs
    raster_provider.rs
    embedded_images.rs    # trait consumer; decode stays in façade
  builders/
    table_builder.rs
    ruled.rs
    borderless.rs
    partial_rule.rs
  router/
    auto.rs
    probe.rs
    shadow.rs             # M1 parallel run
  post/
    form.rs
    stitch.rs
    split.rs
    nms.rs
  geom.rs
  raster/
  legacy/
    orchestrator.rs       # today's lib.rs soup, frozen
    lattice_adapter.rs
    stream_experimental.rs
```

| Crate | Responsibility |
|-------|----------------|
| `pdfparser-content` | VM + **resolver callbacks**; Form streams interpreted only when resolver provides bytes |
| `pdfparser` | Façade: Form resolver, image decode, optional render provider, rotate, assemble inputs |
| `pdfparser-tables` | Evidence, builders, router, policy; **no PDF I/O** |
| `pdfparser-cli` | Presets, `--tables-legacy`, `--tables-preset engine-v2`, `--dump-evidence` |

### Core types

```rust
pub struct LineSegment {
    pub x0: f32, pub y0: f32, pub x1: f32, pub y1: f32,
    pub thickness: f32,
    pub source: LineSourceKind, // VectorStroke | VectorThinFill | VectorForm | RasterEmbedded | RasterFullPage
    pub confidence: f32,
}

pub struct Joint { pub x: f32, pub y: f32, pub h_id: u32, pub v_id: u32 }

pub struct LineEvidence {
    pub h: Vec<LineSegment>,
    pub v: Vec<LineSegment>,
    pub joints: Vec<Joint>,
    pub page_width: f32,
    pub page_height: f32,
}

pub struct AlignmentPeak {
    pub x: f32,           // or y for horizontal bands
    pub support: u32,     // textlines hitting this peak
    pub kind: AlignKind,  // Left | Center | Right
}

pub struct TextLayer {
    pub runs: Vec<TextRun>,
    pub textlines: Vec<TextLine>,
    pub left_edges: Vec<AlignmentPeak>,
    pub center_edges: Vec<AlignmentPeak>,
    pub right_edges: Vec<AlignmentPeak>,
    pub median_font: f32,
}

pub struct RegionProposal {
    pub id: u32,
    pub bbox: Rect,
    pub kind: RegionKind, // RuledContour | PartialRuled | BorderlessText | Residual
    pub joint_count: u32,
    pub area_frac: f32,
    pub whitespace_est: f32,
    pub line_score: f32,
    pub text_score: f32,
    pub structure_prior: f32, // pre-build estimate
}

pub struct EvidenceDiagnostics {
    pub vector_rule_count: u32,
    pub form_rule_count: u32,
    pub raster_embedded_rule_count: u32,
    pub raster_fullpage_rule_count: u32,
    pub joint_count: u32,
    pub regions_proposed: u32,
    pub regions_rejected_gate: u32,
    pub router_stage: RouterStage, // Legacy | Shadow | EngineV2
    pub densify_applied: bool,
    pub render_attempted: bool,
    pub render_fallback: bool,
    pub elapsed_evidence_ms: f32,
    pub elapsed_detect_ms: f32,
}

pub struct PageEvidence {
    pub page_index: u32,
    pub page_box: Rect, // post-rotate user space
    pub text: TextLayer,
    pub lines: LineEvidence,
    pub regions: Vec<RegionProposal>, // raw proposals before partition
    pub diagnostics: EvidenceDiagnostics,
}
```

### Traits

```rust
pub trait LineSource {
    fn collect(&self, ctx: &PageContext) -> Vec<LineSegment>;
}

/// Implemented in façade; tables crate only consumes resolved segments / pages.
pub trait RasterLineProvider {
    fn pages(&self, page_index: u32) -> Result<Vec<RasterPage>, ProviderError>;
}

pub trait RegionProposer {
    fn propose(&self, text: &TextLayer, lines: &LineEvidence, policy: &ProposalPolicy)
        -> Vec<RegionProposal>;
}

pub trait TableBuilder {
    fn build(&self, evidence: &PageEvidence, region: &RegionProposal, policy: &dyn PolicyView)
        -> Option<Table>;
}

pub trait PageRenderer {
    fn render_gray(&self, page_index: u32, dpi: u32) -> Result<RasterPage, ProviderError>;
}

/// Injected into content VM from façade (PR2).
pub trait FormContentResolver {
    /// Resolve Form XObject by resource name under current resource stack.
    fn resolve_form(&mut self, name: &str) -> Option<FormXObject>;
}

pub struct FormXObject {
    pub id: ObjectId,           // for cycle detection
    pub stream: Vec<u8>,
    pub matrix: Matrix3x2,      // /Matrix or identity
    pub b_box: Option<Rect>,
    // Resources handled by pushing resolver scope in façade
}
```

---

## AutoRouter algorithm (normative)

### `ProposalPolicy` defaults (geometry-justified)

| Constant | Default | Justification |
|----------|---------|----------------|
| `min_joints_ruled` | **5** | ≥2×2 cells need 4 corners; +1 margin filters tick noise. **Migration:** code today uses `lattice_min_joints=4`; EngineV2 starts at 5; flag `policy.min_joints` A/B on real tags (see Densify & joints migration). |
| `min_area_frac` | `5e-4` | Page-relative; chrome ticks below this die (order of Camelot min area; validated on owned precision chrome modes—not ICDAR files) |
| `min_seg_len_font_mult` | `0.35` | `max(3pt, mult * median_font)` |
| `joint_gap_font_mult` | `0.25` | `max(2pt, mult * median_font)` |
| `line_snap_font_mult` | `0.15` | `max(1.5pt, mult * median_font)` |
| `ownership_pad_font_mult` | `0.5` | pad = `max(2pt, mult * median_font)` |
| `ownership_iou_tau` | `0.15` | max IoU with owned ruled region for non-ruled claim |
| `ownership_contain_frac` | `0.85` | containment ⇒ owned |
| `whitespace_reject` | `0.90` | empty-cell / whitespace estimate reject for ruled chrome |
| `line_score_ruled_enter` | `0.55` | enter Ruled builder |
| `line_score_partial_enter` | `0.25` | enter Partial if not ruled |
| `text_score_borderless_enter` | `0.50` | enter Borderless |
| `min_structure_score_emit` | `0.45` | post-build reject |
| `partition_nms_iou` | `0.60` | suppress duplicate proposals same kind |

### Scoring formulas

Let `page_area = page_width * page_height` (post-rotate media/crop box used by extract).

**`area_frac`** = `bbox.area() / page_area`.

**`joint_count`** = number of H∩V joints with endpoints within `joint_gap` of both segments inside expanded bbox.

**`line_score`** ∈ [0,1]:

```text
n_h = count H segs intersecting bbox with len >= min_seg
n_v = count V segs similarly
span_h = union coverage of H along vertical extent / bbox.height   # 0..1
span_v = union coverage of V along horizontal extent / bbox.width
joint_term = min(1, joint_count / 12)
line_score = 0.25 * min(1, n_h/4) + 0.25 * min(1, n_v/4)
           + 0.20 * span_h + 0.20 * span_v + 0.10 * joint_term
```

**`whitespace_est`** ∈ [0,1] (ruled regions):

```text
# Prefer rule-ink estimate when raster mask available:
whitespace_est = 1 - (ink_pixels_in_bbox / bbox_pixels)
# Else vector proxy:
cell_grid_guess from joint x/y anchors → empty_cell_frac after text assign dry-run
whitespace_est = empty_cell_frac
```

**`text_score`** ∈ [0,1]:

```text
L = support-weighted left peaks with support >= min_align_support (default 3)
C, R similarly
multi = textlines in bbox with >=2 tokens
align_div = |unique peak classes with support>=3 among L,C,R| / 3
col_sep = column_separation_score(textlines)   # existing geom helper
schema_stab = fraction of multi-col lines whose L-edge schema matches region mode
text_score = 0.30 * min(1, multi/8)
           + 0.25 * min(1, L.len()/3)
           + 0.15 * align_div
           + 0.15 * col_sep
           + 0.15 * schema_stab
```

**`structure_prior`** = `0.6 * line_score + 0.4 * text_score` for ordering only.

### Proposal generators (may overlap)

1. **Ruled — raster contour primary (when mask exists, K6):**  
   Combined binary mask (K28) → morph H/V → connected components / contours on (H∨V) → each component with joints ≥ `min_joints_ruled` and `area_frac ≥ min_area_frac` → `RuledContour`. Anchors = joints **inside** contour ∪ bbox edges; merge-close lines.
2. **Ruled — vector joint-CC (always, fallback/complement):**  
   Connected components of H∩V segments (today’s lattice clustering). Same gates. Used alone when no raster mask; merged/NMS’d with contour proposals when both exist (prefer higher joint_count / line_score).
3. **Partial rule bands:** some H or V, `joint_count < min_joints_ruled`, `line_score ≥ line_score_partial_enter`, multi-col text → `PartialRuled`. **No page-union frame.**
4. **Borderless table areas (not raw network split alone):**  
   See § Borderless — area proposal first, then `text_score ≥ text_score_borderless_enter` → `BorderlessText`.
5. **Residual:** only bands left after partition with high text_score and no owner — rare second pass; still **bounded** (not free page bbox).

### Vertical merge before partition finalize (K26)

```text
function vertical_merge(proposals, policy):
  sort proposals by -y1 (top first)
  repeat until fixed point:
    for each pair (a,b) with a above b:
      if kind compatible (same kind OR {Ruled,Partial}):
        if x_iou(a,b) >= 0.60
           and y_gap(a,b) <= 1.5 * median_textline_gap
           and col_schema_compatible(a,b):
          merge into union bbox; keep higher-priority kind
  return proposals
```

Prevents classic **header lattice + body stream** split of one visual table.

### Conflict resolution & non-overlapping partition

```text
function partition(proposals, policy):
  proposals = vertical_merge(proposals, policy)   # K26 first

  # 1. Gate each proposal independently
  for p in proposals:
    if p.kind == RuledContour and (p.joint_count < min_joints or p.area_frac < min_area_frac
        or p.whitespace_est >= whitespace_reject): mark reject
    if p.kind == BorderlessText and p.text_score < text_score_borderless_enter: mark reject

  # 2. Sort survivors by priority then score then smaller area
  priority = { RuledContour: 3, PartialRuled: 2, BorderlessText: 1, Residual: 0 }
  sort by (priority desc, structure_prior desc, area_frac asc)

  owned = empty list of (bbox_padded, kind)
  accepted = []
  for p in sorted:
    if overlaps_owned(p.bbox, owned, policy): continue
    if any IoU(p,q) >= partition_nms_iou for q in accepted with q.kind==p.kind: continue
    accepted.append(p)
    owned.append(pad(p.bbox, ownership_pad), p.kind)
  return accepted
```

**Ownership test** (bbox-based, not pixel mask in v1):

```text
overlaps_owned(b, owned, policy):
  for (ob, ok) in owned:
    if ok is RuledContour or ok is PartialRuled:  # hard owners
      if IoU(b, ob) >= ownership_iou_tau: return true
      if intersection(b,ob)/area(b) >= ownership_contain_frac: return true
      if intersection(b,ob)/area(ob) >= ownership_contain_frac: return true
  return false
```

### Build dispatch + emit order (K27)

```text
tables = []
for region in partition(...):
  match region.kind:
    RuledContour → RuledTableBuilder
    PartialRuled → PartialRuleBuilder
    BorderlessText → BorderlessTableBuilder
  if table is None or table.structure_score < min_structure_score_emit: drop
  else tables.append(table)

# Normative product order (also used by real_structure harness):
sort tables by (-bbox.y1, bbox.x0)   # top-to-bottom, then left-to-right
return tables
```

**Priority rule:** Ruled > Partial > Borderless by sort + ownership; never two builders on same owned region.

### Unit-test fixtures (synthetic geometry only)

| Fixture | Expect |
|---------|--------|
| Clean 3×4 joint grid | one Ruled region; no Borderless |
| Chrome ticks + real grid | ticks fail joints/area; one Ruled |
| No rules, multi-col text | one Borderless; zero Ruled |
| Partial outer frame + text | Partial or Ruled if joints suffice; not dual |
| Nested: small grid inside text columns | Ruled owns inner; Borderless only outside pad |

---

## RuledTableBuilder

### Algorithm delta vs today's lattice

| Stage | Today (`lattice.rs`) | EngineV2 |
|-------|----------------------|----------|
| Line merge | Inside lattice + raster | Pre-built `LineEvidence` + **combined mask** (K28) |
| Regions | Joint-CC of H∩V only | **Contour/morph-CC when raster** (primary) + vector joint-CC (fallback) |
| Gates | Mixed mid-build | Proposal-time + post-build structure_score |
| Anchors | Joints + lines; densify primary | Joints **inside region** + bbox edges; densify **gated secondary** |
| Raster | Morph → segments only | Morph → **region contours** → joints inside → segments |

### Combined sensing (K28) — normative

```text
when RasterPage (embedded or full-page) available:
  1. adaptive_threshold(gray) → ink
  2. stamp_vector_rules(ink): for each vector H/V seg, paint 1px (or stroke width) into mask
  3. morph close (dashed) + directional open H/V   # existing morph ideas
  4. apply K29 FP suppress (text-baseline, min_seg, chart reject)
  5. contours / CC on (H_mask ∨ V_mask) → region bboxes
  6. joints = H_mask ∧ V_mask inside each region
  7. emit RuledContour proposals + LineEvidence segments for builder
```

### Full-page morph FP policy (K29)

| Rule | Default |
|------|---------|
| Text-baseline suppress | Drop H segs within `0.35 * median_font` of ≥3 textline baselines spanning ≥50% of seg |
| Min segment | `max(8px, 0.08 * region_width)` for H; analogous for V vs height |
| Chart axes | Existing joint-graph + regularity gate; reject 2-line L shapes without grid |
| Max segs/page | Cap (e.g. 2000) then increase min_seg — fail-soft |

### PR4 split (updated)

| Sub | Work | Behavior |
|-----|------|----------|
| **4a** | Extract joint graph + `table_from_component` → `RuledTableBuilder` adapter | **Parity** with legacy lattice on freeze fixtures |
| **4b** | Proposal-time gates; densify behind `LatticePolicy`; min_joints=5 EngineV2 | EngineV2 only until M3 |
| **4c** | **Required for G1 (K33):** contour/morph-CC region seeds when raster present + K28 stamp | Not optional polish |
| **4d** | K29 full-page FP policy wired into morph | With PR3 / full-page path |

### Densify & `min_joints` migration flags

| Flag | Legacy Auto (M0–M2) | EngineV2 initial | Target after A/B |
|------|---------------------|------------------|------------------|
| `allow_text_densify` | **on** (today) | **gated secondary**: only if `ruled_anchor_sparsity > 0.4` AND multi-col band agreement AND empty_frac after densify ≤ `whitespace_reject` | May stay gated |
| `min_joints` | **4** | **5** in `ProposalPolicy` | Keep 5 if precision improves without ruled_fire collapse |

**A/B requirement (K35):** method_mix + row exactness on real tags `partial_rules` / `missing_rules` / `visible_rules`.

### Post-build structure_score (ruled)

```text
structure_score = 0.35 * edge_score + 0.25 * fill_rate
                + 0.20 * min(1, joint_count/12)
                + 0.20 * grid_regularity_score
```

---

## BorderlessTableBuilder (table-area engine)

**Not** “left-edge network with a score.” Pipeline:

```text
  textlines
      → table area proposals (gaps + schema + rejection of prose columns)
      → L/C/R alignment peaks inside each area
      → column fusion
      → row model (textlines + multi-line cell merge)
      → structure_score + list/prose gates
      → emit or drop
```

### Table area proposal (normative — closes B4)

```text
1. Build multi-col textlines (tokens ≥ 2 after whitespace split).
2. Hard split: y-gap ≥ 3 × soft_gap (font-scaled) → separate areas always.
3. Soft split: soft_gap < gap < hard_gap → split only if col schema incompatible
   (count or left-peak match fail) OR numeric-density delta > 0.4.
4. Reject area as non-table if:
   - multi_col_lines < min_body_lines (default 4), OR
   - mean_chars ≥ prose_mean_chars_reject and n_cols ≤ 2, OR
   - numbered-list marker ratio ≥ 0.7 with long right column, OR
   - x-span < 0.15 * page_width (narrow sidebar lists) without strong numeric dens.
5. Interior whitespace: if projected empty row bands dominate without column alignment, reject.
```

Do **not** use classic stream “full page body” fallback that invents one mega-table.

### Alignment peaks + column fusion

```text
bin = max(2pt, 0.20 * median_font)
for each token in area:
  hist on x0 (L), (x0+x1)/2 (C), x1 (R)
peaks = NMS support >= min_align_support (3)

1. Prefer Left peaks (support-filtered).
2. Center peaks only refine mid-gap; invent columns from Center only if Left < 2
   and Center ≥ 3 with stable pitch.
3. Right peaks refine right edge + prose variance reject.
4. Bipartite match L/C/R within bin to collapse duplicates.
```

### Row model + multi-line cells (feeds PR5c)

```text
- Default: one multi-col textline → one row.
- Merge consecutive lines into same row if:
    same column occupancy pattern AND y-gap < 0.55 * fs AND
    not both non-empty in the same column (conflict).
- Wrapped cell: multiple runs assigned to same (row,col) → join with space
  (PR5c exclusive assign on union bbox after row merge).
```

### Fragment re-merge (K26 at borderless level)

After building candidate tables, re-merge vertically if same n_cols, col-center delta < 0.5×median_col_width, y-gap small — **before** emit. Complements router K26.

### `structure_score` (borderless)

```text
structure_score =
  0.25 * min(1, multi_col_lines / max(6, min_body_lines))
+ 0.20 * min(1, n_cols / 3)
+ 0.20 * column_separation_score
+ 0.15 * schema_stability
+ 0.10 * (1 - prose_penalty)
+ 0.10 * align_diversity
```

Emit only if `structure_score ≥ min_structure_score_emit` and list/prose gates fail-closed.

### Map current options → `BorderlessPolicy`

| Current field | Policy field |
|---------------|--------------|
| `stream_max_prose_mean_chars` | `prose_mean_chars_reject` |
| `stream_min_col_sep` | `min_col_sep` |
| `stream_min_body_bands` | `min_body_lines` |
| `stream_region_gap_font_mult` | `region_gap_font_mult` |
| `stream_region_gap_min` | `region_gap_min` |
| `min_confidence_stream` | `min_emit_confidence` |

---

## PartialRuleBuilder (K34)

| Hybrid today | EngineV2 PartialRule |
|--------------|----------------------|
| `find_outer_frames` | Keep **inside region bbox only** |
| `union_rules_frame` page-wide | **Deleted** |
| Text columns/rows | **Text-band row model** (multi-col bands), not densify-primary |
| Sparse rules | Use partial H/V as **hints** for band edges; missing edges from text |
| Promote | If after build joints ≥ min_joints → retag Ruled |

Partial may not claim Ruled-owned regions. Confidence floor higher than Borderless when `line_score < line_score_ruled_enter`.

---

## Cell quality program (K30, PR5c)

Independent of which builder produced the grid:

| Step | Spec |
|------|------|
| Exclusive assign | One run → one master cell (existing geom); prefer interior over boundary |
| Multi-run merge | Runs in same cell bbox → join `" "` / hyphen rules from layout |
| Span blanks | Covered slots empty string; text on master only (ICDAR-style) |
| Span merge | Keep edge-based colspan/rowspan; add tests on real financial-ish geom_unit |
| Superscripts | Optional later: small-font runs near top of cell stay in cell text |
| Out of scope v1 | Vertical text, nested tables, OCR glyphs |

**Acceptance:** real_structure cell F1 measured; geom_unit span fixtures; not “free with 4a extract.”

---

## Line evidence & Form XObject expansion

### Unification

Vector (page stream + Form-expanded) + embedded morph + optional full-page morph → snap/coalesce → joint graph → `LineEvidence`.

### Why Form expansion cannot live only in `pdfparser-content`

Today: `interpret_page(content: &[u8], fonts, opts)` has **no** document graph, Form stream bytes, `/Matrix`, or Form `/Resources`. `Do` only pushes `ImagePlacement`. Nested image walk in `raster_images.rs` does not interpret Form **content**.

### Normative PR2 design: resolver API

```rust
// pdfparser-content
pub fn interpret_page_with_resolver(
    content: &[u8],
    fonts: &HashMap<String, LoadedFont>,
    opts: &InterpretOptions,
    resolver: Option<&mut dyn FormContentResolver>,
) -> InterpretResult;

// On Do:
// 1) try image placement (existing)
// 2) else if resolver.some: resolve_form(name)
// 3) cycle check on Form id; depth < max_form_depth
// 4) push CTM' = CTM * form.matrix; optional clip to form BBox
// 5) interpret form.stream with remaining op budget; rules get LineSourceKind::VectorForm
// 6) pop resource/font scope (façade resolver stack)
```

| Limit | Default |
|-------|---------|
| `max_form_depth` | 4 |
| `max_form_expansions_per_page` | 32 |
| `max_ops` (shared) | existing governor |
| `per_form_max_ops` | `max_ops / 4` floor 50_000 |
| Cycle detection | set of `ObjectId` on stack |
| Interpret mode | rules+text for forms (or rules-only flag if text duplicated—default both, dedupe later) |

**PR2 dependency:** does **not** require PR1 provenance for landing; can emit `RuleSegment` only. Provenance tags merge when LineEvidence lands.

### Content VM sequencing (not one mega-PR)

| Step | PR | Scope |
|------|-----|--------|
| Form expansion + resolver | **PR2a** | Primary vector completeness |
| Dash pattern expansion | **PR2b** | Expand `d` into segments or stamp intervals |
| Clip-respecting rule emission | **PR2c** | Intersect segments with clip polygon (axis-aligned clip first) |
| Near-axis curve chords | **PR2d** (optional) | Chord if `|angle| < 8°` from axis and chord len ≥ min_seg; else discard |

---

## Full-page raster backend (Tier 1)

### When render runs

```text
render_used =
  cfg!(feature = "full-page-render")
  && renderer.is_some()
  && (
       opts.enable_full_page_render                    // explicit / HighQuality
    || (preset == Auto && post_G1 && may_auto_render) // K25 opportunistic
  )
// Cargo feature alone never renders.
// allow_auto_render=false disables K25 even on Auto.
```

### PR3 spike checklist (must document in PR)

| Item | Requirement |
|------|-------------|
| Candidates | pdfium bindings vs external pre-render CLI vs pure-Rust — pick with license note |
| License | Compatible with MIT/Apache-2.0; native lib redistrib notes |
| Binary size | Report Δ for feature-on |
| Platforms | macOS/Linux x86_64 (+ arm64 if feasible); Windows optional |
| RCE / attack surface | Sandbox guidance for multi-tenant servers |
| Hard caps | **max_dpi = 200**; **max_pixels = 40e6**; timeout **5000 ms**/page |
| Fail-soft | `diagnostics.render_fallback=true`; continue Core; **no hard fail** |
| CI | Feature-off always green; feature-on **nightly optional** |
| Acceptance | `cargo tree` without feature must not pull native render |
| Integration | Output feeds K28 combined mask + K6 contours + K29 FP policy |

### Security caps (normative)

```rust
pub struct RenderSafety {
    pub max_dpi: u32,              // 200
    pub max_pixels: u64,           // 40_000_000
    pub timeout_ms: u64,           // 5000
    pub max_pages_rendered: u32,   // per document call, default 50
}

pub struct RenderOptions {
    pub enable_full_page_render: bool, // explicit on
    pub allow_auto_render: bool,       // default true post-G1 Auto; embedder may false
}
```

---

## API stability & knob diet

### Slim public surface (target after deprecation window)

| Field | Purpose |
|-------|---------|
| `detect_tables` | Master |
| `preset` | Off / LatticeOnly / Auto / EngineV2 / HighQuality / Fast |
| `stitch_multipage` | bool (product default true; **false** in real_structure harness) |
| `enable_raster_embedded` | bool default true |
| `enable_full_page_render` | bool default false (HQ true if feature) |
| `allow_auto_render` | bool default true — K25 opportunistic gate |
| `max_tables_per_page` | cap |
| `min_table_confidence` | global floor |
| `legacy_router` | rollback |
| `policy_overrides: Option<PolicyBundle>` | advanced (0.1: advanced module OK) |

### Compatibility plan (0.1.x)

1. **Retain** all existing `TableOptions` fields with `#[deprecated]` + `#[serde(default)]` for one minor after M3.
2. Deprecated fields **map into** `LatticePolicy` / `BorderlessPolicy` / `ProposalPolicy` when building EngineV2; on legacy path, fields keep current meaning.
3. `TableOptions::from_legacy(self) -> TableOptions` identity for now; later strips dead fields.
4. Feature `legacy-table-options` (default **on** in 0.1.x) keeps full struct; off builds slim struct for embedders who opt in.
5. `detect_stream_tables`: `#[deprecated]` in PR1; `pub use` behind `experimental-stream` feature after PR7; remove default export when call sites zero.
6. `TableModeSet { lattice, stream, hybrid }`: during M1–M2 maps to which **legacy** detectors run; on EngineV2 maps to probe enables (stream→borderless builder, hybrid→partial).
7. **Changelog** entry every PR that deprecates or remaps a field.
8. **Open Q #3 decided (K17):** add `TableMethod::PartialRuled`; `Hybrid` remains serde/`rename` alias until 0.2.

### Façade freeze

- `pdfparser` continues re-exporting `TableOptions`, `TablePreset`, `TableMethod`, `TableModeSet`.
- No silent removal of `pub use detect_stream_tables` without deprecation cycle.

---

## Corpus Strategy (real-primary, anti self-approval)

This section is **as important as the algorithm rewrite**. Architecture alone cannot break the
self-approval loop if the primary scoreboard stays on ReportLab grids the engine was co-evolved with.

### Why the current corpus fails as a quality bar

Approximate inventory (workspace, 2026-07):

| Bucket | ~PDF count | Gold quality | Role today | Problem |
|--------|-----------:|--------------|------------|---------|
| `corpus/compete_synthetic` (C001+) | **~146** | Full cell grids | compete_hard wins | Co-evolved with densify/lattice; peers look weak by construction |
| basic + stress (01–27) | ~20 | Mixed / generator gold | Multi-lib #1 tables | **Happy path** digital text + clean tables |
| hard 50–62 + precision 70–82 + sensing 90–95 | ~32 | Full grids | “Hard” regression | Mostly **solved** after tuning (cell F1 ≈ 1.0) — no longer hard |
| `corpus/real` + `compete_real` | ~27 | **Soft** (counts/tokens; **~0 full cell grids**) | Smoke only | **Cannot** measure TEDS/row/col the way ICDAR does |
| ICDAR-2013 | 67 external | Official XML | Competitive only | Correct stressor — **forbidden** in CI/corpus |

```text
  Self-approval loop (must break)

  ┌────────────────────┐     score ~1.0      ┌────────────────────┐
  │ Synthetic / solved │ ─────────────────▶ │ Claim product win  │
  │  ReportLab grids   │                    │  on owned harness  │
  └─────────▲──────────┘                    └─────────┬──────────┘
            │                                         │
            │ tune knobs / densify                    │ external real / ICDAR
            │                                         ▼
            │                               ┌────────────────────┐
            └────────────────────────────── │ F1/TEDS collapse   │
              (no real structure gold)      │ wrong method mix   │
                                            └────────────────────┘
```

**Rule:** A suite on which pdfparser already scores ≥0.95 cell F1 is **regression insurance**, not a development target and **never** a market claim.

### Goals for the corpus program

1. **Primary quality signal** = real (or real-derived) PDFs with honest gold — not synthetic multi-lib leaderboard.
2. **Coverage of struggle modes** (Appendix B) with **≥2 real docs per mode** where licensing allows; synthetic only fills geometric unit gaps no real PDF supplies.
3. **Cull / demote** bulk that only re-proves happy paths.
4. **Never ICDAR files** in corpus — but **always** mode tags inspired by ICDAR *failure classes*.
5. **Gold independence:** human-reviewed; never “export from pdfparser and commit as gold.”

### Target suite layout (after corpus PRs)

```text
  benchmark/
  ├── real_track/                    ◄── PRIMARY (CI + README)
  │   ├── manifests/
  │   │   ├── real_detect_smoke_vN.json     ≥20–40 real docs
  │   │   ├── real_structure_core_vN.json   5 → 15 → 25+ cell grids
  │   │   ├── real_fp_smoke_vN.json         forms / chrome / prose (expect 0 or low count)
  │   │   └── latency_probe.json
  │   ├── gold/                      structure + detect gold (schema v1)
  │   ├── freezes/
  │   └── SOURCES.md                 license + URL per doc
  │
  ├── corpus/
  │   ├── real/                      licensed public PDFs (growing)
  │   ├── geom_unit/                 SMALL synthetic for unit geometry only
  │   │                              (merge of few basic lattice/stream fixtures)
  │   └── archive/                   moved-out bulk (not scored by default)
  │       └── compete_synthetic/     C001–C180 etc. after demotion
  │
  └── ground_truth/                  legacy; migrate structure gold → real_track/gold
```

| Suite id | Purpose | Merge-blocking? | Contents |
|----------|---------|-----------------|----------|
| `real_detect_smoke` | Detection F1 (IoU when bbox exists) | Yes (after freeze Steady) | Real PDFs only |
| `real_structure` | Cell F1 / shape / row / col | Yes at G1 (≥15) | Real + full cells |
| `real_fp_smoke` | Over-detect / form FP | Yes (det precision floors) | Forms, chrome, prose |
| `geom_unit` | Pure geometry unit tests | cargo + optional light CI | ≤20 synthetic PDFs |
| `legacy_synthetic_full` | Optional historical multi-lib | **No** | Archived compete_* |
| `competitive_icdar` | External honesty | **No** | Outside repo |

### What to remove, demote, or keep

#### A. Demote to `archive/` (not deleted from git history; **out of default scoreboards**)

| Set | Action | Why |
|-----|--------|-----|
| `compete_synthetic` C001–C068 (~64) “coverage wave” | Archive | Already solved; inflates #1 |
| Most C100–C180 after modes are covered by **real** docs | Archive gradually | Keep only until real substitutes exist for that mode |
| hard 50–62 once real multi-table/span covered | Demote from primary | Perfect 1.0 scores = dead target |
| precision 70–82 | Keep **one** chrome FP synthetic in `geom_unit` or `real_fp_smoke` seed; archive rest | FP control unit |
| basic 01–12 text fixtures | Keep **minimal** text smoke elsewhere; tables not primary | Text path already covered |

#### B. Delete from active generators / stop growing

| Stop | Reason |
|------|--------|
| Parametric “more C### variants of same mode” | Redundant density without new geometry |
| New ReportLab grids used to claim multi-lib SOTA | Re-enters self-approval |
| Soft-gold real docs with no plan to promote to structure | Clutter; prefer fewer real docs with real gold |

#### C. Keep small as `geom_unit` (algorithm unit tests, not product SOTA)

Target **≤ 15–20 PDFs**, each mapped to **one** pure geometric unit:

| geom_unit fixture role | Example existing id (may rename) |
|------------------------|----------------------------------|
| Clean vector lattice | `06_table_lattice` |
| Borderless stream skeleton | `07_table_stream` |
| Partial frame | `08_table_partial_border` |
| Painted thin rect / image rules | sensing 90 / C10x image set (1–2 only) |
| False underlines overdense H | one precision underline doc |
| Side-by-side two grids | one hard side-by-side |

Acceptance for geom_unit: cargo integration + optional fast Python run. **Not** published as market rank.

#### D. Grow aggressively: real / hard-real PDFs

**Sources (licensed only; document in `SOURCES.md`):**

| Source class | Examples already nearby | Use |
|--------------|-------------------------|-----|
| US gov statistical | Census ACS, BLS, Fed Beige Book, NIST notices | Multi-col stats, multipage |
| Tax / forms | IRS schedules (as **FP** or true tables carefully) | form_not_table vs data tables |
| Tabula / Camelot **sample** docs (not ICDAR package) | schools, argentina votes, fuel, health | Borderless + lattice |
| Academic papers (arXiv PDF) | tensorflow paper already | sparse tables, mixed layout |
| International open data PDFs | EU stats agencies with open license | multi-table pages |
| Invoices / bank-like **public** samples | only if license clear | footer rows, partial rules |

**Hard forbid:** ICDAR-2013 competition PDFs/XML; any “copy eu-0xx.pdf into corpus.”

**Add rate (program):**

| Horizon | real_detect_smoke | real_structure (cell gold) | real_fp_smoke |
|---------|------------------:|---------------------------:|--------------:|
| Now | 13 soft | 0 | few ad hoc |
| +2 weeks | ≥20 | 5 | ≥5 |
| Gate G1 | ≥20 | **≥15** | ≥8 |
| Steady product | ≥30–40 | **25–40** | ≥12 |

Prefer **hard real** (multi-table, multipage, partial rules, borderless 20+ rows) over easy single clean lattice.

### Failure-mode coverage matrix (corpus must close gaps)

Every mode must have **≥1 real doc** in detect smoke and **≥1 structure-gold doc** when cells are annotatable. Synthetic geom_unit only if no licensed real PDF found after search.

| Mode tag | What to capture | Prefer real source type | Structure gold? |
|----------|-----------------|-------------------------|-----------------|
| `visible_rules` | Vector or painted ruled grids | Camelot samples, gov tables | Yes |
| `missing_rules` / `raster_needed` | Lines only after render/image | Image-only grids, Excel-to-PDF oddball | Yes if text extractable |
| `partial_rules` | Incomplete H/V | Financial partial frames | Yes |
| `borderless_multicol` | Whitespace columns, 10–40 rows | Schools / votes / ACS | Yes |
| `row_fragment` | Engine splits one table into many | Large borderless + section notes | Yes |
| `header_slice` | Only header band recovered | Multi-level headers | Yes |
| `col_collapse` | Under-count columns | Wide stats, partial V | Yes |
| `over_detect` | Chrome / ticks / deco lines | Pages with borders + one table | Detect + FP smoke |
| `multi_table` | 2–6 tables/page | Reports with stacked tables | Yes |
| `multi_page_continuation` | Continued tables | Multipage stats | Detect + stitch policy |
| `form_not_table` | Form chrome ≠ table | IRS f1040 class | FP smoke only |
| `chrome_rules_fp` | Page border as lattice | Deco frames | FP smoke |
| `borderless_prose_fp` | Multi-col prose | Magazines / 2-col articles | FP smoke |
| `span_complex` | Colspan/rowspan | Financial / category stubs | Yes if clear |
| `empty_grid` / sparse | Many empty cells | Wide census | Yes |
| `mixed_lattice_stream` | Ruled + borderless same page | Mixed reports | Yes |

**Coverage gate:** PR6b/c and Gate G1 require a checked matrix file  
`benchmark/real_track/manifests/coverage_matrix.json` with mode → doc_ids.  
Missing modes = **explicit waiver** with reason (license / no public PDF), not silence.

### Gold quality tiers (stop “soft forever”)

| Tier | Contents | Used for |
|------|----------|----------|
| **T0 Soft detect** | `expected_table_count` ± tol; optional page counts | real_detect_smoke entry only |
| **T1 BBox detect** | Per-table `page` + `bbox` | IoU detection F1 |
| **T2 Shape** | rows × cols exact | Structure weak |
| **T3 Full cells** | Complete cell grid + optional spans | real_structure primary |
| **T4 Tokens only** | must_contain in cells | Legacy; do not use alone for structure claims |

**Promotion path:** T0 → T1 within 1 sprint of adding a real doc; **structure core** requires T3.  
Docs stuck at T0 for >1 release are **candidates for removal** from smoke (noise).

**Annotation process (anti self-gold):**

```text
  1. Add PDF + SOURCES.md (license, URL, retrieved date)
  2. Semi-auto draft: Camelot / pdfplumber / pdfparser shadow export
  3. Human edits cells / bboxes / counts  ── never accept auto output
  4. Second reviewer spot-check (100% for first 15 structure docs)
  5. Tag failure_classes[] from Appendix B
  6. Land gold PR separate from algorithm PR when possible
```

### Anti self-approval process (normative)

| Rule | Enforcement |
|------|-------------|
| README primary numbers only from `real_structure` / `real_detect_smoke` | PR8 + review checklist |
| Synthetic multi-lib boards labeled **regression only** | README language ban |
| EngineV2 / Auto flip blocked without G1 real core ≥15 | PR7 checklist |
| New synthetic fixture requires: (a) mode not covered by real, (b) geom_unit only, (c) not for scoreboard | CONTRIBUTING + review |
| Algorithm PR must attach real smoke method_mix delta | PR template |
| “We beat Camelot on compete_hard” is **not** a ship criterion | Docs |

### Corpus PR workstream (parallel to algorithm)

| PR | Title | Deliverable |
|----|-------|-------------|
| **C0** | Policy + inventory | `coverage_matrix.json` skeleton; SOURCES.md template; demotion list signed |
| **C1** | Archive bulk synthetic | Move compete_synthetic + solved hard into `corpus/archive/`; suites stop scoring them by default |
| **C2** | geom_unit slim set | ≤20 synthetic; generators only for those |
| **C3** | Real detect smoke ≥20 | New licensed PDFs + T0/T1 gold; IoU where possible |
| **C4** | Structure core 5 | First T3 golds; nightly metrics |
| **C5** | Structure core 15 | **Unlocks Gate G1** with algorithm track |
| **C6** | FP smoke suite | Forms / prose / chrome |
| **C7** | Ongoing real growth | 25–40 structure; matrix complete |

Corpus PRs are **mergeable without algorithm changes**. Algorithm PRs that only improve synthetic archive scores are **rejectable**.

### Early ICDAR refuse

`benchmark/scripts/assert_no_icdar.py` (landed with PR1):

- Scan `benchmark/corpus/**`, `benchmark/real_track/**`, `benchmark/ground_truth/**` for ICDAR basenames / path fragments.
- CI job every PR.
- Competitive ICDAR remains external black-box only.

### Harness redesign (metrics)

Current `metrics.py`: greedy **cell-F1** assignment; detection by **counts**. Gold often **lacks bboxes**.

| Sub-PR | Deliverable |
|--------|-------------|
| **6a** | Gold schema v1 with optional `bbox`; **IoU ≥ 0.5 greedy bipartite** matching; geom_unit order fallback; detect smoke scaffolding; latency_probe; method_mix; assert_no_icdar |
| **6b** | Detect smoke ≥**20** real; coverage_matrix started; FP smoke split |
| **6c** | Structure core **5 → 15 → 25** T3 grids on **real** docs |

### Phased structure gold (operational)

| Phase | Core size | Est. effort | CI strength |
|-------|-----------|-------------|-------------|
| G0 | harness only | 2–4 eng-days | IoU unit tests |
| G1 prep | **5** real T3 grids | ~2–4 h/doc + reviewer | Nightly non-blocking |
| G2 | **15** real T3 | +1–2 weeks | **Merge-blocking Auto flip (G1)** |
| G3 | **25–40** real T3 | ongoing | Primary README scoreboard |

**Struggle-first selection (K31):** ≥60% of structure-core docs must be ones where **Legacy Auto** currently fails exact shape **or** detect count (probe before gold commit). Easy lattice wins may fill ≤40%. Document probe in gold PR.

**Extractable-text bar (C6):** structure-core docs must have median non-empty cell text recoverable by pdfparser text path (spot-check); pure image-text tables go to Tier-2 OCR backlog, not T3 core.

**Harness:** `real_structure` runs with `stitch_multipage=false` and K27 table order; optional peer bakeoff (K32).

### Metrics

| Metric | Role |
|--------|------|
| IoU≥0.5 bipartite match | Primary engineering pairing on real track |
| Detection F1 (IoU-aware) | Primary detect |
| Row/col/shape exact, cell F1 | Structure core only |
| Method mix, ruled_fire_rate, render_used_rate | Telemetry |
| Latency p50/p95 | Informative until rebaseline |
| Peer cell/det on real core | Secondary honesty (K32) |
| Synthetic cell F1 | **Regression only** — not primary |

### CI concrete (K13)

| Job | When | Command / notes |
|-----|------|-----------------|
| `cargo-test` | every PR | existing |
| `assert-no-icdar` | every PR | `python benchmark/scripts/assert_no_icdar.py` |
| `real-detect-smoke` | after 6a + C3 | Fail Steady if det F1 < freeze − ε |
| `real-structure` | core≥15 + freeze Steady | cell F1 / shape vs `g2.json` |
| `real-fp-smoke` | after C6 | Precision floor / max FP count |
| `geom-unit` | optional fast | ≤20 synthetic |
| `render-nightly` | optional | feature-on |
| multi-lib synthetic archive | **not** merge-blocking | optional nightly |

**Synthetic multi-lib leaderboard: not merge-blocking and not README primary.**

### Freeze file policy (Boot → Steady)

Applies to `benchmark/real_track/freezes/g2.json` (structure core) and, once created, `.../freezes/detect_smoke_v0.json`.

#### Boot (first G1 attempt / first freeze write)

1. Prerequisites: structure core ≥15 with human-reviewed gold; detect smoke ≥20; EngineV2 opt-in runnable; shadow JSON (EngineV2 vs legacy method_mix + det counts) **human-reviewed**.
2. Run EngineV2 (release CLI) on the structure core; write freeze file from **that run** after review. Fields at minimum:

```json
{
  "schema_version": 1,
  "suite_id": "real_structure_g2",
  "engine": "engine_v2",
  "created_utc": "YYYY-MM-DD",
  "git_sha": "...",
  "doc_count": 15,
  "metrics": {
    "det_f1": 0.0,
    "cell_f1": 0.0,
    "shape_exact": 0.0,
    "row_exact": 0.0,
    "col_exact": 0.0
  },
  "epsilon": {
    "det_f1": 0.05,
    "cell_f1": 0.03,
    "shape_exact": 0.03
  },
  "mode": "boot_snapshot",
  "notes": "First G1 baseline; human-reviewed shadow JSON attached in PR"
}
```

3. **No CI auto-fail on self-compare** during Boot: the inequality “≥ freeze − ε” is **not** applied against the same run that authors the freeze (would be tautological).
4. Optional **absolute floors** (product judgment, not required for architecture): e.g. refuse Boot if det F1 &lt; 0.40 or cell F1 &lt; 0.35 on the 15-doc core—set only if maintainers agree; document chosen floors in freeze `notes`.
5. Shadow item “no silent det collapse vs legacy” is a **PR7 review checklist**, **not** a hard gate that EngineV2 cell F1 ≥ legacy cell F1 (method mix differs; not one-to-one).

#### Steady (after freeze committed on main)

1. Subsequent PRs fail CI if metrics drop **below freeze − ε** (ε from freeze file).
2. **Intentional rebaseline:** manual PR that updates freeze metrics + `git_sha` + `created_utc`, sets `mode: "rebaseline"`, and **CHANGELOG** note explaining why (gold fix, sensing jump, agreed quality trade). Not silent.
3. Gold additions (15→25) may keep the same freeze metrics thresholds until a rebaseline; expanding core without rebaseline uses the same metric definitions on the **growing** set only after an explicit freeze update (prefer rebaseline when core composition changes materially).

### Gate G1 (Auto flip) — named minimum

#### Corpus bar

1. Structure core **≥ 15** T3 real grids (phase G2) with **K31 struggle-first** ratio.
2. Detect smoke **≥ 20** real docs.
3. Coverage matrix: required modes filled or waived with reason.
4. FP smoke suite exists (C6) with at least baseline numbers.

#### Algorithm bar (K33) — EngineV2 must include

| Capability | Min PR |
|------------|--------|
| Form rule expansion | **PR2a** |
| Dash **or** full-page render path available | **PR2b** or **PR3** feature-on build in CI nightly |
| Ruled contour-when-raster + combined stamp | **PR4c** + **K28** |
| Full-page FP policy if render used | **PR4d** / **K29** |
| Borderless table-area + fragment merge | **PR5** (+ **K26** unit tests) |
| Cell multi-run merge / exclusive assign | **PR5c** |
| Vertical proposal merge | **PR5b** or router PR with K26 tests |
| Exclusive AutoRouter | Wired for EngineV2 preset |

#### Process bar

1. Freeze file **Boot-written and committed** (no self-compare fail on Boot).
2. Shadow JSON EngineV2 vs legacy method_mix **human-reviewed**.
3. `legacy_router` rollback tested.
4. Densify/min_joints A/B note (K35).
5. README draft of capability ladder (Tier 0 vs 1).

**PR7 Auto flip is blocked until G1.** EngineV2 may ship earlier as opt-in (K36).

---

## Dead-code deletion safety

### Inventory before delete (PR7 prerequisite report)

| API / module | Action timeline |
|--------------|-----------------|
| `detect_stream_tables` | Deprecate PR1; feature-gate; delete when zero call sites + tests moved to experimental |
| `detect_network_tables` | Becomes BorderlessTableBuilder; keep wrapper |
| `detect_hybrid_tables` / `union_rules_frame` | PartialRuleBuilder; delete page-union after EngineV2 characterization |
| `detect_lattice_tables` | Ruled adapter; internal |
| Legacy orchestrator in `legacy/orchestrator.rs` | Keep until M4 |

### Characterization before delete

1. Baseline `real_detect_smoke` method_mix + det counts (legacy).
2. EngineV2 same suite.
3. Kill classic stream only if product path never calls it and experimental tests cover it.
4. **Form discriminator:** keep residual post-pass until Borderless gates prove equivalent on FP smoke.

### Form scrub split

| Heuristic class | Destination |
|-----------------|-------------|
| 2-col prose / numbered list | BorderlessBuilder gates (already partially in network options) |
| Tiny / caption grids | Proposal gates min_area + post |
| Form-like chrome high empty | Ruled whitespace_reject + form residual |
| Overseg scrub / max logical tables | post/form residual until router partition proven |

---

## Alternatives Considered

### Alt A — More post-hoc filters on soup

Reject: filters fight; exclusive only when strong lattice.

### Alt B — Mandatory full-page render

Reject as default; optional HQ only (K4).

### Alt C — Immediate multi-crate split

Reject for v1 (K1).

### Alt D — VLM table core

Reject for v1.

### Alt E — Detect-only track alone

Reject as sole quality signal; need structure core for row/col/cell.

### Alt F — Fix sensing only (Form + full-page); keep lattice/network as-is

| Pros | Cons |
|------|------|
| Smaller change; may raise ruled_fire_rate | Left-edge network + union hybrid + strong-lattice-only exclusive remain; densify still primary; knobs remain |
| | ICDAR-like borderless failures (network depth) untouched |

**Reject as end state.** Sensing is necessary but not sufficient—K5–K7 still required. May **sequence** sensing PRs before builder rewrites (already PR2/PR3 before Auto flip).

### Alt G — Camelot FFI / shell-out for HighQuality

| Pros | Cons |
|------|------|
| Fast HQ quality | Python/native dep; non-library-first; license/ops; not pure-Rust |
| | Diverges IR; hard to solidify our engine |

**Reject** for product core and default HQ. Optional research spike only—not committed architecture.

---

## Security & Privacy

| Concern | Mitigation |
|---------|------------|
| Render RCE/memory | Feature + runtime opt-in; RenderSafety caps; fail-soft; nightly-only CI |
| Form recursion | depth, expansions/page, op budgets, cycle IDs |
| Image bombs | existing stream governor |
| Gold license | SOURCES.md; public-domain preference |
| ICDAR leakage | assert_no_icdar in CI |
| Customer PDFs | not in CI |

---

## Observability

### Early (PR1) — required for success criteria

- `EvidenceDiagnostics` + per-table method/region/joint counts on every detect path (legacy fills best-effort).
- Timers: `elapsed_evidence_ms`, `elapsed_detect_ms`.
- Shadow mode JSON: `{page, legacy_tables, v2_tables, method_mix}`.
- Optional `tracing` behind feature `trace`—not required.

### PR6 harness

- `method_mix`, `ruled_fire_rate`, `densify_applied_rate` in accuracy JSON.

### PR9 expansion

- CLI `--dump-evidence`, richer spans, probe dashboards—not the first time diagnostics exist.

### Latency methodology (K20)

| Item | Spec |
|------|------|
| Probe list | `benchmark/real_track/latency_probe.json` (doc_id + page indices), created in PR6a |
| Build | `cargo build --release -p pdfparser-cli` |
| Measure | wall time per page inside CLI or harness; exclude process startup from p95 page stats when possible; report both cold-start doc and per-page |
| Warm | discard first page or first doc optional flag |
| Includes | embedded morph when images present; full-page render only if enabled |
| Report | p50/p95 in accuracy JSON from 6a timers |
| README | no latency claims until post-PR5 rebaseline on this probe |
| Success Criteria | “instrumentation exists” early; numeric budgets **informative only** until baseline freeze |

Initial budgets (informative): Fast ≤15 / Auto ≤40 / HQ ≤200 ms/page—**rebaseline required**.

---

## Rollout Plan

### Feature flags

| Flag | Default | Role |
|------|---------|------|
| `full-page-render` | off | Compile render (Tier 1) |
| `experimental-stream` | off (after PR7) | Classic stream |
| `legacy-table-options` | on in 0.1.x | Full TableOptions fields |
| `trace` | off | tracing in tables |
| Runtime `enable_full_page_render` | false | Explicit render on |
| Runtime `allow_auto_render` | true | K25 opportunistic (post-G1 Auto) |
| Runtime `legacy_router` / env / CLI | see Migration | Rollback |

### PR plan (gap-closed order)

```text
Algorithm                                      Corpus (parallel)
─────────                                      ─────────────────
PR1  evidence + legacy freeze                  C0 inventory + matrix
PR6a IoU + stitch-off eval + assert_no_icdar   C1 archive synthetic bulk
PR2a Form rules                                C2 geom_unit ≤20
PR2b Dash (G1 path A)                          C3 real smoke ≥20 struggle tags
PR3  Full-page render provider (G1 path B)     C4 structure 5 struggle-first
PR4a Ruled extract parity
PR4b Gates + densify flags
PR4c Contour regions + K28 combined   ◄── req for G1 when raster
PR4d K29 morph FP policy
PR5  Borderless table-area + K26 merge
PR5b Router exclusive + vertical merge
PR5c Cell quality (multi-run / spans)
PR2c Clip (can trail slightly)
PR2d Curves optional
                                               C5 structure 15 ──┐
PR7  Auto = V2 ONLY IF G1 ◄── K33 algo + C5 ──┘
PR8  README capability ladder + real primary   C6 FP smoke; C7 grow 25–40
PR9  dump-evidence
```

**G1 needs:** C5 + PR2a + (PR2b **or** PR3) + PR4c + PR5 + PR5b + PR5c unit/integration green on EngineV2.

### PR1 — Traits + LineEvidence + shadow + deprecations

| | |
|--|--|
| **Title** | `refactor(tables): PageEvidence, shadow diagnostics, legacy orchestrator extract` |
| **Depends** | — |
| **Acceptance** | Orchestrator digest-identical when sensing inputs fixed; shadow JSON; assert_no_icdar CI; `allow_auto_render` field present (default true, unused until G1) |

### PR2a — Form XObject resolver + rule expansion

| | |
|--|--|
| **Title** | `feat(content): FormContentResolver + form rule expansion` |
| **Depends** | PR1 optional |
| **Acceptance** | Rules from Form content; depth/cycle; digest refresh + method_mix delta; **G1 required** |

### PR2b — Dash patterns

| | |
|--|--|
| **Title** | `feat(content): dash pattern → rule segments` |
| **Depends** | PR2a optional |
| **Acceptance** | Dashed lines become segments or stampable; geom_unit dashed fixture; **G1 path A** |

### PR2c / PR2d — Clip / curves

| | |
|--|--|
| **Title** | Clip-respecting emission; optional near-axis curve chords |
| **Depends** | PR2a |
| **Acceptance** | Not hard G1 blockers; should land before claiming Tier-0 completeness |

### PR3 — Full-page render provider

| | |
|--|--|
| **Title** | `feat(render): full-page RasterLineProvider (feature-gated)` |
| **Depends** | PR1 |
| **Acceptance** | Spike checklist; safety caps; fail-soft; feature-off tree clean; feeds K28/K6; **G1 path B** (alternative to PR2b for K33) |
| **Note** | Not “optional polish” — required for Tier-1 market claim; opportunistic Auto uses it post-G1 |

### PR4a — Ruled extract parity

| | |
|--|--|
| **Title** | `refactor(tables): RuledTableBuilder joint-CC extract (legacy parity)` |
| **Depends** | PR1 |
| **Acceptance** | Digest parity on freeze fixtures with fixed rules |

### PR4b — Proposal gates + densify flags

| | |
|--|--|
| **Title** | `feat(tables): ruled proposal gates + densify migration flags` |
| **Depends** | PR4a, PR6a |
| **Acceptance** | EngineV2 only; A/B note template; shadow JSON on real smoke |

### PR4c — Contour regions + combined sensing (K6/K28)

| | |
|--|--|
| **Title** | `feat(tables): raster contour region seeds + vector stamp into mask` |
| **Depends** | PR4a; PR3 and/or embedded raster |
| **Acceptance** | **G1 required.** When raster present, regions from contours; unit tests stamp+joint; real smoke ruled_fire_rate up on image/painted tags |

### PR4d — Full-page morph FP policy (K29)

| | |
|--|--|
| **Title** | `feat(tables): text-baseline suppress + min_seg region fraction` |
| **Depends** | PR4c |
| **Acceptance** | FP smoke does not explode when render on; chart reject retained |

### PR5 — Borderless table-area engine

| | |
|--|--|
| **Title** | `feat(tables): BorderlessTableBuilder table areas + L/C/R + row model` |
| **Depends** | PR1, PR6a |
| **Acceptance** | **G1 required.** Area proposal rejects prose; multi-col 20+ row geom/real improves vs legacy network; no full-page mega-table fallback |

### PR5b — Exclusive router + K26 vertical merge

| | |
|--|--|
| **Title** | `feat(tables): AutoRouter partition + vertical proposal merge` |
| **Depends** | PR4b, PR5 |
| **Acceptance** | **G1 required.** Unit fixtures: header+body merge; ruled owns over borderless; emit order K27 |

### PR5c — Cell quality

| | |
|--|--|
| **Title** | `feat(tables): multi-run cell merge + span blanks + exclusive assign polish` |
| **Depends** | PR4a, PR5 |
| **Acceptance** | **G1 required.** Cell F1 on structure core moves with content quality, not only detect |

### PR6a/b/c — Harness

| | |
|--|--|
| **Title** | `test(bench): IoU metrics, real smoke, structure harness (stitch off)` |
| **Depends** | — day 0; pairs with C3–C5 |
| **Acceptance** | 6a: IoU, method_mix, latency, stitch=false for structure, assert_no_icdar. 6b: smoke≥20. 6c: support core ramp metrics |

### PR7 — Auto flip

| | |
|--|--|
| **Title** | `feat(tables): exclusive AutoRouter as product Auto (post-G1)` |
| **Depends** | **Full G1** (corpus + K33 algorithm bar), characterization |
| **Acceptance** | Appendix D checklist; capability ladder in README; K25 active if feature on; rollback works |

### PR8 — Docs + honest README

| | |
|--|--|
| **Title** | `docs: real-primary scoreboard + Tier 0/1 claims; archive sprawl` |
| **Depends** | G1 numbers |
| **Acceptance** | No synthetic SOTA as market claim; ladder documented; peer secondary optional |

### PR9 — Observability

| | |
|--|--|
| **Title** | `feat(tables): dump-evidence CLI + render/method telemetry` |
| **Depends** | PR1, PR7 preferred |
| **Acceptance** | CLI dump includes proposals, render_used, method_mix |

### Max reviewable diff guideline

Prefer ≤~800–1200 net LOC algorithmic change per PR. Split epics as above. **Do not mark PR4c/PR5/PR5c optional** relative to G1.

---

## Doc Cleanup Plan

### Keep

- This design → `docs/design-table-engine-v2.md`
- README honest scoreboard
- `benchmark/README.md` (real track)
- CONTRIBUTING
- `docs/design-native-pdf-parser.md` (trimmed links)
- CHANGELOG

### Archive after migration (PR8)

phase-*-report, duplicate accuracy-scoreboard copies, plateau/struggle/icdar competitive (→ `docs/archive/competitive/`), sensing essays, old orchestrator body.

### PR1 stub (Issue 23)

At top of `docs/table-orchestrator-architecture.md` and `docs/icdar-plateau-analysis-and-plan.md`:

```markdown
> **Superseded for implementation** by `docs/design-table-engine-v2.md` (table engine re-architecture). Historical context only.
```

---

## Risks

| Risk | Sev | Mitigation |
|------|-----|------------|
| Gold annotation bottleneck | High | Phased 5→15→25 real T3; G1 at 15; detect smoke first; corpus PRs parallel |
| Staying on synthetic #1 (self-approval) | High | K23; C1 archive; README bans; G1 requires real core |
| Auto flip before gate | High | K12/K18; PR7 blocked on G1 **and C5** |
| Form resolver complexity | Med | PR2a only Forms first; limits |
| Render deps/security | Med | Caps; runtime opt-in; nightly CI |
| Densify/min_joints silent shift | Med | Migration flags + A/B artifacts |
| Synthetic drop on EngineV2 | Med | Accept; not merge gate |
| Dead-code delete regression | Med | Characterization + deprecation |
| ICDAR leakage | High | assert_no_icdar early |
| Over-claim README | High | PR8 after G1; language bans |

---

## Success Criteria

1. **Harness:** IoU + real_detect_smoke CI + assert_no_icdar; structure with stitch off.
2. **Corpus:** synthetic demoted; geom_unit ≤20; struggle-first real T3 core 5/15/25; coverage matrix.
3. **Anti self-approval:** README primary = real track; capability ladder Tier 0/1 honest.
4. **Gate G1** = C5 + **K33 algorithm bar** (Form, dash|render, contour+combined, borderless area, router merge, cell quality).
5. **Ruled:** contour-when-raster + K28 combined; not joint-CC-only.
6. **Borderless:** table-area engine, not left-edge soup.
7. **K26** vertical merge unit tests; **K27** emit order.
8. **Cell quality PR5c** contributes to cell F1, not only detection.
9. **Method mix / ruled_fire_rate / render_used_rate** instrumented.
10. **Exclusive routing** + no dual ownership on EngineV2.
11. **Latency** methodology; budgets informative until rebaseline.
12. **K35** policy changes need real_track A/B.
13. **No ICDAR** in corpus/CI; peer bakeoff optional on real core.
14. **Legacy Auto frozen** until G1; EngineV2 recommended pre-G1 (K36).
15. **G1 freeze Boot→Steady** documented.

---

## Honest Communication Plan (README)

Same principles as rev1, plus:

- Until G1: README may say EngineV2 experimental; Auto = legacy orchestrator.
- After G1: primary scoreboard = real_structure suite id + date.
- Latency numbers only from latency_probe methodology.
- Language bans unchanged (no mixed synthetic #1 as market SOTA).

---

## Open Questions

1. ~~Render backend choice~~ → **Decided:** spike in PR3; pure-Rust default without feature.
2. **Gold tooling path** — one entrypoint under `real_track/` tools vs extend `build_gold_standards.py`.
3. ~~Hybrid rename~~ → **K17** PartialRuled additive.
4. ~~Stitch~~ → **K15/K27** product on, eval off.
5. ~~G1 size~~ → **K18** core ≥15.
6. ~~Dash path~~ → **PR2b** in VM preferred; morph also helps dashed after render.
7. ~~policy_overrides public?~~ → **Decided:** advanced module / `Option<PolicyBundle>`; not required for basic embedders.
8. ~~Auto opportunistic render?~~ → **K25** yes post-G1 when feature compiled.
9. ~~Contour vs joint-CC primary?~~ → **K6** contour when raster; joint-CC always.
10. Remaining: default DPI for full-page (150 vs 200) — set in PR3 spike with K29 FP tradeoff.

---

## References

- Orchestrator: `crates/pdfparser-tables/src/lib.rs`
- Lattice densify / joints: `lattice.rs` (`lattice_min_joints` default 4)
- Network left-edge: `network.rs`
- Hybrid union: `hybrid.rs` `union_rules_frame`
- Options ~50 fields: `options.rs`
- VM Do/images: `pdfparser-content/src/vm.rs`
- Embedded raster: `raster_images.rs`
- Soft real gold: `benchmark/ground_truth/*real*`, compete_real soft_gold
- CI: `.github/workflows/ci.yml` (no accuracy job today)
- Plateau diagnosis: `docs/icdar-plateau-analysis-and-plan.md` (historical)

---

## PR Plan (summary table)

### Algorithm (gap-closed)

| PR | Title | G1? | Depends |
|----|-------|:---:|---------|
| 1 | Evidence + legacy freeze + shadow | | — |
| 6a | IoU harness, stitch-off eval, assert_no_icdar | | — |
| 2a | FormContentResolver | **yes** | —/1 |
| 2b | Dash rules | path A | 2a |
| 2c/d | Clip / curves | no | 2a |
| 3 | Full-page render provider | path B | 1 |
| 4a | Ruled joint-CC extract parity | | 1 |
| 4b | Gates + densify flags | | 4a, 6a |
| 4c | Contour regions + K28 combined | **yes** | 4a, raster |
| 4d | K29 morph FP policy | yes if render | 4c |
| 5 | Borderless table-area engine | **yes** | 1, 6a |
| 5b | AutoRouter + K26 vertical merge | **yes** | 4b, 5 |
| 5c | Cell quality multi-run/spans | **yes** | 4a, 5 |
| 7 | Auto = V2 flip | needs full G1 | K33 + C5 |
| 8 | README ladder + real primary | | G1 |
| 9 | dump-evidence | | 1, 7 |

### Corpus (required — K23 / K31)

| PR | Title | Depends | Deliverable |
|----|-------|---------|-------------|
| **C0** | Inventory + coverage matrix | — | matrix, demotion list, SOURCES |
| **C1** | Archive bulk synthetic | C0 | off default scoreboards |
| **C2** | geom_unit ≤20 | C1 | unit suite only |
| **C3** | Real detect smoke ≥20 | C0, 6a | T0/T1; tag struggles |
| **C4** | Structure core **5** struggle-first | C3 | T3 golds |
| **C5** | Structure core **15** struggle-first | C4 | **Unlocks G1** with algo |
| **C6** | real_fp_smoke | C3 | forms/prose/chrome |
| **C7** | Grow 25–40 + matrix complete | C5 | steady product bar |

### Suggested merge order

```text
PR1 ── PR6a ──┬── PR2a ── PR2b ──────────────┐
              ├── PR3 (render) ──────────────┤
              ├── PR4a ── PR4b ── PR4c ── PR4d
              ├── PR5 ── PR5b ── PR5c ───────┼── G1 (K33 + C5) ── PR7 ── PR8 ── PR9
              └── PR2c/d (trail) ────────────┘
                                             ▲
C0 ── C1 ── C2 ── C3 ── C4 ── C5 ────────────┘
                    └── C6 ── C7
```

---

## Appendix A — File mapping

| Current | Target |
|---------|--------|
| `lib.rs` soup | `legacy/orchestrator.rs` + thin `lib.rs` |
| `lattice.rs` | `builders/ruled.rs` + densify helper |
| `network.rs` | `builders/borderless.rs` + `text_layer.rs` |
| `hybrid.rs` | `builders/partial_rule.rs` (no page-union) |
| `stream.rs` | `legacy/stream_experimental.rs` |
| `options.rs` | slim + deprecated fields |
| `form.rs` | `post/form.rs` residual |
| `extract.rs` | resolver + evidence assembly |

## Appendix B — Failure-class tags

`over_detect`, `under_detect`, `row_fragment`, `col_collapse`, `partial_rules`, `missing_rules`, `raster_needed`, `multi_table`, `multi_page_continuation`, `borderless_prose_fp`, `chrome_rules_fp`, `form_not_table`, `span_complex`, `empty_grid`, `visible_rules`

Migrate compete gold `C4_ROW_FRAGMENT_*` → these tags over time (non-blocking).

## Appendix C — Latency (informative)

| Preset | p50 target (rebaseline) |
|--------|------------------------:|
| Fast | ≤ 15 ms/page |
| Auto | ≤ 40 ms/page |
| HighQuality | ≤ 200 ms/page |

Methodology: K20 / Observability section.

## Appendix D — Gate G1 checklist (copy into PR7)

### Corpus

- [ ] real_structure core ≥ 15 T3 (human-reviewed)  
- [ ] **≥60% struggle-first** (Legacy Auto fail shape or count) — K31  
- [ ] real_detect_smoke ≥ 20  
- [ ] Coverage matrix complete or waived  
- [ ] real_fp_smoke baseline exists  
- [ ] Extractable-text bar checked on structure core  

### Algorithm (K33)

- [ ] PR2a Form expansion landed  
- [ ] PR2b dash **or** PR3 render feature available in nightly  
- [ ] PR4c contour-when-raster + K28 combined stamp  
- [ ] PR4d / K29 if render used in quality path  
- [ ] PR5 borderless table-area engine  
- [ ] PR5b AutoRouter + K26 vertical merge tests  
- [ ] PR5c cell multi-run / span blanks  
- [ ] Emit order K27; structure harness stitch=false  

### Process

- [ ] EngineV2 run on core; Boot freeze `g2.json` written  
- [ ] No CI self-compare fail on Boot run  
- [ ] Shadow method_mix JSON human-reviewed  
- [ ] `legacy_router` + env + CLI rollback verified  
- [ ] Densify/min_joints A/B note (K35)  
- [ ] Characterization report for stream/hybrid demotion  
- [ ] README capability ladder (Tier 0/1) drafted  

### Steady (after freeze on main)

- [ ] Metrics ≥ freeze − ε  
- [ ] Rebaseline only via intentional PR + CHANGELOG  

---

## Appendix E — Page pipeline (gap-closed, normative)

```text
  page_content (+ Form expand PR2)
       │
       ├─► vector LineEvidence
       │
       ├─► embedded morph ──┐
       │                    │
       └─► full-page render? (K25 / HQ / explicit)
                │           │
                ▼           ▼
         combined mask (K28 stamp vector ∪ raster ink)
                │
                ▼
         morph H/V + K29 FP suppress
                │
         ┌──────┴──────┐
         ▼             ▼
   contour regions   segment joints
   (primary if       (always)
    raster)
         │             │
         └──────┬──────┘
                ▼
   borderless table-area proposals (parallel)
                │
                ▼
   K26 vertical_merge → partition (Ruled>Partial>Borderless)
                │
         ┌──────┼──────────┐
         ▼      ▼          ▼
      Ruled  Partial   Borderless
         │      │          │
         └──────┼──────────┘
                ▼
         PR5c cell assign / spans
                ▼
         sort K27 (−y1, x0)
                ▼
         stitch? (product yes / eval no)
```

---

*End of design document (rev 2.3).*
