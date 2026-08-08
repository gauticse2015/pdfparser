# Zero-Regression Hardening + Architecture Repair

| Field | Value |
|-------|-------|
| **Title** | Production hardening and architecture repair (no quality/perf regression) |
| **Author** | pdfparser contributors |
| **Date** | 2026-08-08 |
| **Status** | Draft (rev 3.1 — open questions decided 2026-08-08) |
| **Workspace HEAD context** | After stacked PRs #29–#33 (standards / SOLID hygiene). As-built, not prior-session file sizes. |
| **Living path** | `docs/design-zero-regression-hardening.md` |
| **Diagrams** | ASCII only (no Mermaid) — `docs/README.md` |
| **Does not supersede** | `docs/design-table-engine-v2.md` (capability ladder, K-decisions, ICDAR policy) |
| **Informs / sequences** | `docs/design-table-engine-v3-industry.md` exclusive-first; `docs/implementation-plan-v3-gated.md` GATE-0..5 (quality) |
| **Hard constraints** | No product Auto behavior change in mechanical PRs. No perf regression. No ICDAR in CI/tuning. Do not claim GATE-3/4 green. |
| **P0.1 (2026-08-08)** | Landed. A1.12 / A3.23 / A4.8 / B Deref *evidence* columns describe the **pre-P0.1** tree. Living gate board is `docs/STATUS.md`. Do not flip the whole inventory in this file. |
| **ASCII** | `docs/STATUS.md` and `docs/ARCHITECTURE.md` are strict ASCII. This inventory may keep typographic unicode (dashes, arrows). Diagrams stay ASCII. |

---

## Overview

pdfparser's table product is already a **detection-strong** Engine V2 stack: `TablePreset::Auto` / `Full` run lattice + residual hybrid + network, then `finalize_engine_v2` (K26 merge + partition + nested keep + exclusive cleanup). That is **not** the V3 exclusive-first architecture (`Sense -> Classify -> Build once`). Detectors still all run; `PageEvidence` is diagnostics-only; `route_proposals` is not on the product call graph. Pre-P0.1 status files disagreed about which gates are green; living board is now `docs/STATUS.md`.

This document is a **production hardening + architecture repair bible**, not a TEDS-chasing plan. It (1) inventories every A1–A5 / B item against **today's tree**, (2) ranks a phased fix order that cannot silently change product Auto, (3) gives low-level design so each stacked PR is independently mergeable, and (4) defines a **No-Regression Contract**. Until **P0.6** lands, the contract's *intent* is binding but several commands are **not yet executable** (see that section). After P0.6 the reprinted command block is copy-pasteable.

Quality holes (census 34x2 vs 34x10, NIPA 4x3 vs 51x22, ICDAR TEDS) stay **Phase 5**, freeze-gated, and must not drop detection discipline / `fp_strict` / nested keep (doc 42). Correctness bugs that change outputs (stroke width, StandardEncoding leftovers, MissingToUnicode) land behind **shadow compare or explicit opt-in**, then default-flip only after no-drop. **DCT JPEG decode already exists** (A2.2a); do not reopen it as a "fix."

**Why not start with census/TEDS or a big exclusive-router rewrite:** those change Auto tables. We cannot measure "no regression" while the status plane lies, and we cannot flip exclusivity until soup vs shadow is comparable on owned freezes. Architecture-first-without-bit-identity is how we already got status corruption (CHANGELOG GATE-5 PASS vs `g3_industry.json` INVALID).

---

## Key Decisions

| ID | Decision | Rationale |
|----|----------|-----------|
| H1 | **Product Auto stays soup-then-`finalize_engine_v2` through Phase 4.** Exclusive-first is shadow-only in Phase 3 (never returned). **H-Flip is H19**, not "after Phase 3." | V2 already flipped Auto to Engine V2 *router*. Exclusive-first is additive. |
| H2 | **Mechanical PRs are bit-identical** on freeze dumps (count, method, bbox snap, cell texts, provenance enums). Diagnostics strings may change. | Prevents silent score moves. |
| H3 | **Correctness bugs that change extract/tables are isolated PRs** with dump-compare or typed opt-in; default flip is a separate PR. | A2.6 / A2.16 / A2.18 must never hide inside splits. **A2.2a DCT is Fixed — do not reopen.** |
| H4 | **Borrowed `PageEvidence` on the diagnostics path in P1.2.** Product Auto/Fast **must not construct** `PageEvidence` / `LineEvidence` until P3.2. Dump clone only if `shadow_diagnostics && dump requested`. | No extra work vs today on product detect. |
| H5 | **Exclusive shadow is only `advanced.shadow_exclusive_first`** (default **false on all presets**, including EngineV2). Distinct from `shadow_diagnostics`. CLI `--tables-shadow-exclusive`. Never default-enable 2x work. | Shadow = soup + exclusive. |
| H6 | **Densify not flipped in a move/split PR.** Add `DensifyMode` default Primary in **P1.8**. Demote in P5.2. | A1.4 is quality-coupled. |
| H7 | **Do not claim GATE-3/4/5 green.** Lock is `g2.json` + `owned_gates_v0.json` + `--owned-only`. `g3_industry.json` INVALID. | README honesty. |
| H8 | **ICDAR never in CI, never a tuning target, never a PR merge bar.** Merge entrypoint is `check_phase_gates.py --owned-only` (P0.6). ICDAR-only script is optional/external. | Today's `--phase 1` loads ICDAR JSON — that is a P0.6 bugfix, not policy. |
| H9 | **Do not mix tracks.** Owned synthetic != real_structure T3 != detect_discipline != external ICDAR. | A1.11. |
| H10 | **`TableOptions::default()` stays tables-off / legacy.** Product is `from_preset(Auto)`. **Do not flip Default==Auto in 0.1.x.** Decided user 2026-08-08. | Embedders; semver. |
| H11 | **A5 is product scope**, not architecture debt. | Hardening != expanding product. |
| H12 | **As-built Auto == Engine V2.** V2 env `PDFPARSER_TABLE_LEGACY` / `--tables-legacy` are **unimplemented**. This doc does **not** re-endorse them (H22). | V2 migration text is stale. |
| H13 | V3 exclusive page flavor is the **post-H-Flip target**; today's `ruled_owns_page` is a post-lattice skip. | — |
| H14 | options-map Deref claim is stale. **`impl Deref` is gone.** | — |
| H15 | **Detector PRs: author runs live owned suites; nightly on main is required.** PR CI = unit + `assert_no_icdar` + `--owned-only` on committed JSON (after P0.6). Do not claim PASS from committed JSON alone. | Nightly is not optional if H15 is real. |
| H16 | Empty-user-password encryption stays hard-error in 0.1 (incl. empty user pw). **Decided user 2026-08-08.** | Native K15; not this rail. |
| H17 | Census 10-col assert only after Phase 5 algorithm exists. Shadow metric first. | A4.2. |
| H18 | **`merge_then_partition` (no sort)** is the shared primitive. `finalize_engine_v2` keeps today's emit-walk order. `route_proposals` may sort after for tests. P1.3 is **not** a drop-in `route_proposals` call. | Extra sort can change emit when `source_indices` overlap. |
| H19 | **Single H-Flip sequence:** soup through Phase 4 → Phase 3 shadow in parallel (not returned) → re-run shadow on **post-Phase-4 soup** → H-Flip only if shadow diff = 0 on g2 core ids (count/method/bbox snap) and owned-gate floors hold. | Honest scores change survivors; do not flip exclusive then retune joints. |
| H20 | **All new flags on `TableAdvancedOptions`** (serde flatten). No 13th top-level field. CLI maps to advanced. Rollback stays top-level `legacy_router`. | Surface already exactly 12. |
| H21 | **One owned-gate floor table** (`freezes/owned_gates_v0.json` = this section = `--owned-only` checker). Cell band freeze − **0.03** (today's G1.7). No `live-main - 0.02` relative floors. | Contract and checker must agree. |
| H22 | **Rollback = `--legacy-router` / `legacy_router=true` only.** After H-Flip also `advanced.exclusive_first_live=false`. | Env/alias are V2 fiction (zero `*.rs` reads). |

---

## Still-open vs already-fixed inventory (as-built 2026-08-08)

Inventory is against the current tree under `crates/` + `benchmark/` + `.github/workflows/ci.yml`. **Do not open PRs for Fixed rows.** Partial = leftover work listed.

### Legend

| Status | Meaning |
|--------|---------|
| **Fixed** | Behavior/code matches the intended fix; no PR. |
| **Partial** | Hygiene or subset landed; residual listed. |
| **Open** | Still present as described (or worse, if noted). |
| **Out of scope** | A5 / explicit product cut-line. Do not treat as architecture debt. |

### A1 Architectural faults

| ID | Issue | Status | Evidence today | Residual / notes |
|----|-------|--------|----------------|------------------|
| A1.1 | Soup-then-filter; exclusive page flavor never landed | **Open** | `orchestrator/page.rs` always runs lattice, then hybrid, then network; optional classic; overwide re-invoke; after-form re-invoke; then `finalize_engine_v2`. `ruled_owns_page` only *filters* borderless, does not skip hybrid/lattice work. | V3 S2 classify-first = Phase 3. |
| A1.2 | `PageEvidence` / `LineEvidence` unused as algorithm input | **Open** | `page_evidence_from_inputs` clones runs/rules/rasters; `proposals: Vec::new()`. Builders take raw slices. Product detect does **not** build evidence. | P1.2: borrow on diagnostics only. P3.2: product constructs borrowed evidence for classifier. |
| A1.3 | V2 router is NMS 2.0; `route_proposals` not called | **Open** | Product: `finalize_engine_v2` calls `vertical_merge` + `partition` directly (no pre-emit sort). `route_proposals` = merge+partition+**sort proposals**. | P1.3: extract `merge_then_partition` (no sort); do not drop in `route_proposals`. |
| A1.4 | Densify-as-primary on sparse-rule pages | **Open** | `lattice_text_densify: true`; `builders/ruled/mod.rs` densify X/Y from text before grid emit. | P1.8 add `DensifyMode` default Primary (no math). P5.2 flip A/B. |
| A1.5 | Triple borderless stack | **Open** | Network primary + classic if `allow_classic_stream` + overwide hybrid recovery (re-run network) + after-form recovery (re-run network). Auto has classic **off**. | Still 2–3 network invocations possible. |
| A1.6 | Hybrid single global outer frame | **Partial** | K34: `union_rules_frame` removed from detect path (dead helper remains). `find_outer_frames` can emit **multiple** frames. | Still cannot emit two partial tables without closed frames; one grid per frame. |
| A1.7 | Text path != table path for forms | **Fixed** | `TextOptions.expand_forms` default **true**; table path `capture_rules \|\| expand_forms`. CLI `expand_forms: true`. | — |
| A1.8 | Knob explosion behind fake <=12 diet | **Partial** | 12 top-level fields (`PRODUCT_TABLE_OPTION_FIELDS`). Advanced + tuning + policy + raster still dozens. **Deref removed.** | H20: new flags on `advanced` only. |
| A1.9 | Notes-string control plane | **Partial** | Typed flags exist. Only product `notes.iter().any` is `contour_seed_match` write-once uniqueness in `finalize_engine_v2` (not routing). Notes still **written** as telemetry. | P1.4: replace that guard with a bool; CI grep; keep note writes. |
| A1.10 | Keyword / threshold fitting (IRS, NIST, Schedule D, width>=140) | **Open** | `lexicon.rs` tax/notice phrases; `STRONG_LATTICE_2COL_MIN_WIDTH = 140.0`; `only_weak_lattice` uses `bbox.width() < 140.0`. Generic phrases, not ICDAR filenames — still corpus-shaped. | Freeze-gated if thresholds move (K35). |
| A1.11 | Eval != product | **Open** | real_structure: stitch off + page-local gold (`page_filter`). ICDAR: order match. R010 structure n_pred=1 after filter vs discipline n_pred=2. R005 discipline `n_exp=0` vs structure `n_exp=1`. | Harness honesty Phase 0; do not "fix" by changing product stitch default. |
| A1.12 | Status plane corrupted | **Open** | README: do not claim GATE-3/4; core cell 0.738. CHANGELOG Unreleased: GATE-4/5 PASS. `g3_industry.json`: phase4/5 **INVALID** + `revoked_at`. `freezes/README.json`: phase5 PASS. `AUTONOMOUS_PROGRESS.md`: GATE-3 PASS, core 0.820. `real_track/results/STATUS.md`: GATE-3 FAIL. `phase-structure-gates.md`: Phase 3 FAIL. options map: Deref. V2 design: Auto still legacy. | Phase 0 single status plane. |
| A1.13 | God files | **Partial** | Splits landed (`orchestrator/*`, `builders/ruled/*`, `network/*`, `vm/{path,state,text}`, `densify.rs`). Remaining LOC: `ruled/mod.rs` 1240, `densify.rs` 1219, `raster/morph.rs` 1537, `network/mod.rs` 936, `stream.rs` 986, `vm/mod.rs` 973, `hybrid.rs` 823, `form.rs` 747, `page.rs` 731. | Continue splits bit-identically. |
| A1.14 | Dead design surface | **Open** | `TableModeSet.structure` unused by detectors. `PipelineId::S1Structure` unused as a builder. Contour seeds computed in `finalize_engine_v2` then **not** entered into partition (notes only). | Delete or wire behind flag; default keep discard (current Auto). |
| A1.15 | `detect_tables_document` vs façade page_size/rasters | **Open** | `detect_tables_document*` pass `page_size: None` (letter stand-in). Façade `document_tables` / `page_tables` pass media box + rasters. | Fix is **not** bit-identical if letter != media. Phase 2/3 with freeze compare. |
| A1.16 | Policy `min_area_frac` uses letter; proposal `area_frac` may use real page | **Open** | `ProposalPolicy::from_options` divides `lattice_min_table_area` by `LETTER_PAGE_AREA`. `table_to_proposal` uses `page_area(page_size)`. | Same PR family as A1.15; freeze-gated. |

### A2 Parse-stack bugs / incomplete

| ID | Issue | Status | Evidence today | Residual / notes |
|----|-------|--------|----------------|------------------|
| A2.1 | Text extract does not expand Form XObjects | **Fixed** | `expand_forms` default true; `DocFormResolver`. | — |
| A2.2a | JPEG / `DCTDecode` unsupported | **Fixed** | `filters.rs`: DCT passthrough; `raster_images.rs` `image` crate decode. | Do not reopen a DCT "fix." Painted-grid miss is A3.21 sensing. |
| A2.2b | JPX / CCITTFax / JBIG2 unsupported | **Open / later** | `StreamFilter::parse` None -> Unsupported; raster fail-soft skip. | Opt-in later; not H3 critical path. |
| A2.3 | Curve ops c/v/y clear whole path | **Fixed** | `vm/mod.rs`: consume operands only; do not `path.clear()`. | — |
| A2.4 | Close-and-stroke s/b does not close | **Fixed** | `s`/`b`/`b*` call `path.close()` before stroke. | — |
| A2.5 | `"` operator does not pop Tw/Tc | **Fixed** | Sets `char_spacing` / `word_spacing` then show. | — |
| A2.6 | Stroke width `w` ignored | **Open** | `"w"` grouped with ignored ops (`stack.clear()`). Fat bars become 1-D centerlines. | Opt-in then default flip; changes lattice inputs. |
| A2.7 | Clip is AABB only; not applied to text | **Open** | `W`/`W*` intersect AABB into `gs.clip_rect`; rules clipped; `show_text` ignores clip. | **P2.4c** opt-in text clip, default off. |
| A2.8 | Many ISO ops ignored; no typed warning | **Open** | Colors/`gs`/BMC/BDC/EMC ignored silently. **No `BI`/`EI`**. Unknown ops -> string `"unknown_op"`. | Typed `VmWarning`; do not change skip set in same PR. |
| A2.9 | Form `/BBox` not clipped; form fonts = page font map | **Open** | `FormXObject.b_box` parsed; `try_expand_form` does not clip. Fonts: page `load_page_fonts` only. | Behavior change; isolate. |
| A2.10 | VM never returns `Err`; missing numbers -> 0.0 | **Open** | `interpret_*` -> `InterpretResult`. `pop_num` underflow -> 0.0 + warning. | Keep fail-soft for product; typed warnings first. Hard-err is opt-in. |
| A2.11 | All VM warnings mapped to `WarningCode::UnknownOperator` | **Open** | `extract.rs` maps every interpret warning to `UnknownOperator`. `MissingToUnicode` exists on IR, unused. | Map by prefix; no table change expected. |
| A2.12 | `ResourceLimits.max_nesting_depth` dead; q/Q unbounded | **Partial** | Page tree walk uses `hard_max::MAX_NESTING_DEPTH`. VM `gstack` unbounded. Form depth capped (`MAX_FORM_DEPTH=4`). | Cap q/Q to `max_nesting_depth`; fail-soft warn. |
| A2.13 | Governor charges then checks | **Fixed** | `filters.rs`: `check_expand_ratio` then `charge_expanded`. | — |
| A2.14 | Inline page `/Resources` dict not stored on `PageInfo` | **Open** | `page_tree.rs` stores `Resources` only if `Object::Reference`. Inline dicts dropped (raster/form walk page dict directly as workaround). | Store owned snapshot or inline flag. |
| A2.15 | LZW `EarlyChange` ignored; DecodeParms not per-filter indexed | **Open** | `decode_lzw` weezl default EarlyChange=1. Flate uses last DecodeParms dict if array. | Isolate; may change rare streams. |
| A2.16 | Type0 without ToUnicode: CID->Unicode conf 0.3 | **Open** | `LoadedFont::to_unicode` Identity-H BMP guess, conf 0.3. | Warning + optional no-guess flag. |
| A2.17 | `MissingToUnicode` warning never emitted | **Open** | Code exists; extract never sets it. | Emit with A2.11/A2.16. |
| A2.18 | MacRoman/Standard high bytes = WinAnsi/Latin-1 | **Partial** | MacRoman 0x80-FF table correct; MacExpert distinct. StandardEncoding high bytes still `char::from_u32(code)` (Latin-1-ish). | Standard high-byte table; freeze text compare. |
| A2.19 | ToUnicode bfrange array form skipped; parse always Ok | **Open** | `parse_bfrange_region` skips `[...]` destinations. `parse` returns `Ok` even if empty. | Implement array form; return err on zero maps when tokens seen. |
| A2.20 | Type0 encoding stream CMaps unimplemented; CID always 2-byte BE | **Open** | `codes_from_bytes` Identity 2-byte. Encoding name parsed, not interpreted as CMap. | Large; isolate; not required to "fix architecture." |
| A2.21 | Type3 charprocs not run | **Out of scope** | Widths used; no charproc VM. | Product later (native design P3). |
| A2.22 | Font load failure -> silent Helvetica-ish fallback | **Open** | `load_page_fonts` `Err(_) => simple_latin`. | Warning + conf drop; same glyphs until opt-in fail. |
| A2.23 | ToUnicode decode uses a fresh governor | **Open** | `to_unicode_stream` `ResourceGovernor::new(default)`. | Thread document governor; security. |
| A2.24 | Reading order: 2 columns only | **Open** | `reading_order_text` detects a single gutter. | Later; text-only freeze. |
| A2.25 | Rotate applied to bbox only, not `TextRun.transform` | **Open** | `apply_page_rotate_to_runs` mutates bbox only. | Bit-identical for bbox-based tables; transform consumers change. |
| A2.26 | IR `Element` is text-only; mcid / from_actual_text never set | **Open** | `Element::Text` only. VM always `mcid: None`, `from_actual_text: false`. | Non-goal for table hardening; optional IR PR. |
| A2.27 | `extract_document` ignores `ExtractOptions.tables` | **Open** | Comment: tables via `Document::tables`. `ExtractOptions.tables` unused. Public API takes `&PdfDocument` (private). | Wire optional tables into IR **off by default** (Default tables off). |
| A2.28 | `pdfparser-export` unused by CLI | **Partial** | Façade re-exports `to_json`. CLI `--format json` uses ad-hoc `serde_json`. | CLI can call export for text IR; not blocking. |
| A2.29 | Encryption always hard-error (incl. empty user pw) | **Out of scope** | `PdfDocument::from_bytes` any `/Encrypt` -> `Error::Encryption`. Intentional 0.1 (A5.2 / K15). | 0.2+ crypto track. |
| A2.30 | Objects API: no pixels/bbox, no GoTo, no XFA, outline titles only | **Out of scope** | `DocumentObjects` metadata-only. | Product later. |
| A2.31 | `from_bytes` cannot full-page-render (no `source_path`) | **Open** | Documented; HQ/K25 fail-soft. | Optional temp-file adapter; not required for Auto/Fast. |

### A3 Table engine / quality holes

Live source unless noted: `benchmark/real_track/results/real_structure_latest.json` (preset **auto**, stitch off), `detect_discipline_latest.json`, `docs/icdar-competitive-report.md`, `benchmark/results/camelot_icdar_headtohead.json`.

| ID | Issue | Status | Live signal | Residual |
|----|-------|--------|-------------|----------|
| A3.1 | Census 34x2 vs 34x10, cell F1 ~0.006 | **Open** | Per-table: `[34,2]` vs gold `[34,10]` cell F1 **0.006**; second table `[31,6]` vs `[33,6]` F1 **0.93**; doc cell F1 **0.493**; det count exact 2. | Phase 5 glued/col assign. Do not add 10-col assert yet. |
| A3.2 | NIPA R010 4x3 vs 51x22 | **Open** | Structure (page 7 filter): 1 table `[4,3]` vs `[51,22]`, cell F1 **0.002**, det count exact. Discipline: **2 vs 1** (over). g3 freeze *claimed* R010 cell 0.565 — **status lie**. | Phase 0 gold/discipline consistency; Phase 5 quality. |
| A3.3 | R005 ACS miss 0 vs 1 | **Open** | Structure: n_pred=0 n_exp=1 gold shape **5x1**; `n_pred_all_pages=99` then page_filter. Discipline: n_exp=**0**. Likely page_filter / stub gold, not a clean ACS miss. | P0.3: open both golds + PDF; do **not** blindly copy structure 5x1 into discipline. |
| A3.4 | R009 10-Q: 2 vs 3 | **Open** | Now **1 vs 3** (worse under-detect). IoU F1 0. | Phase 5 after discipline lock. |
| A3.5 | R008 MMWR count OK, IoU 0, cell 0 | **Open** | Count exact 1; IoU F1 0; cell 0; shape `[4,5]` vs `[47,2]`. | Phase 5. |
| A3.6 | R016 cell F1 0.20 | **Open** | cell 0.20; count exact 2; shapes wrong. | Phase 5. |
| A3.7 | Schools shape exact, cell ~0.47 | **Open** | shape exact `[46,10]`; cell F1 **0.466**. | Phase 5 cell assign. |
| A3.8 | ICDAR WRONG_SHAPE 47 / ROW 41 / COL 33 / TEDS ~0.46 vs Camelot ~0.79 | **Open** | External report. **Not CI.** | Honesty snapshot only. |
| A3.9 | Detect-OK / TEDS<0.35: 12 ICDAR docs | **Open** | Report section. | External. |
| A3.10 | OVER_DETECT 22/67 ICDAR | **Open** | Report. In-repo discipline exact **0.941**, over_doc **0.029**. | Do not retune on ICDAR. |
| A3.11 | Nested keep over-emits corner+full | **Open** | Nested gates exist (`nested_min_area_ratio` etc.). Risk remains on competition layouts. | Phase 4 gates; must keep doc 42 = 2. |
| A3.12 | Densify wrap-explode vs sparse statistical | **Open** | Stream 07/59 cargo tests vs ICDAR col under-count tension. | Phase 5; 07/59 stay green always. |
| A3.13 | Empty-interior-col collapse drops gutters under spans | **Partial** | `collapse_sparse_interior_columns` + colspan preserve; skip empty image lattices. | Residual Phase 5. |
| A3.14 | Hybrid `edge_score=0`, fabricated `joint_count` | **Open** | `hybrid.rs` `edge_score: 0.0`. `table_to_proposal`: Hybrid joints = `rows+cols` if unknown; line_score falls back to confidence. | Phase 4 honesty. |
| A3.15 | `whitespace_est` capped so chrome gate barely fires | **Open** | Cap at `whitespace_reject - 0.01`. | Phase 4; freeze-gated. |
| A3.16 | Unknown lattice `joint_count` set to `min_joints` | **Open** | `table_to_proposal` Lattice unknown -> `policy.min_joints_ruled` so ruled gate always passes. | Phase 4. |
| A3.17 | K26 y-gap is a global **12pt** constant, not page pitch | **Partial** | `ROUTER_MEDIAN_LINE_GAP = 12.0` in `constants.rs` (not 18pt). | P4.2: median from text bands. |
| A3.18 | `method_rank` Hybrid > Stream | **Open** | `geom_util.rs`: Hybrid 3, DenseNumeric 2, Stream 1. Wrong survivor after merge. | Phase 4 freeze-gated. |
| A3.19 | Classic stream mega-fallback still in tree | **Open** | `stream.rs` ~986 LOC; gated `allow_classic_stream` (Auto **false**). | **Decided user 2026-08-08:** keep gated; delete only after M4 + call-site zero. |
| A3.20 | Stitch silent skip on col mismatch; unconstrained if `page_h<=1` | **Open** | `merge_fragments` `continue` on col mismatch. `h<=1` -> `infer_page_height`. | Phase 4; product stitch default true — eval uses `--no-stitch`. |
| A3.21 | JPEG painted grids often undetected | **Open** | Decode path exists; detect still weak without full-page render. Fast never renders. | Sensing+HQ track; not Auto mandatory render. |
| A3.22 | Taxonomy harness: real_track all "unknown" | **Open** | `structure_error_taxonomy_latest.json` `mode_counts.unknown: 23` (null pred/gold shapes). | Phase 0 harness fix. |
| A3.23 | Latest ICDAR head-to-head pdfparser-only but ranks #1 | **Open** | `camelot_icdar_headtohead.json` ranking = `[pdfparser]`. `docs/icdar-competitive-report.md` ranks pdfparser #1. README multi-peer table is more honest (#2). | Phase 0: mark incomplete; never claim #1 from solo board. |
| A3.24 | Truncated rustdoc in `densify.rs` | **Partial** | Module rustdoc exists; references `TableOptions::lattice_text_densify` (actual field is `advanced.lattice_text_densify`). | One-line docs PR. |

### A4 Tests / CI / process

| ID | Issue | Status | Evidence | Residual |
|----|-------|--------|----------|----------|
| A4.1 | Soft-pass tests 93/94/95; missing fixtures skip | **Fixed** | Fixtures exist under `benchmark/corpus/hard_sensing/`. `phase_v_tables.rs` `assert!(path.is_file())` on 90–95. | No remaining test debt. |
| A4.2 | Census e2e locks count/disjoint, not 10-col cell F1 | **Open** | `phase15_census.rs` only `len>=2` + vertical gap. | Shadow metric Phase 0; assert Phase 5. |
| A4.3 | Phase V uses Full, not product Auto | **Fixed** | `table_opts()` = `from_preset(Auto)`. | — |
| A4.4 | CI phase 3–5 `continue-on-error` | **Open** | `ci.yml` real-track-gates. Honest until green; **must not** be labeled PASS. | Keep continue-on-error until live green; fix docs. |
| A4.5 | Real-track gates read committed JSON, not live binary | **Open** | `ci.yml` explicit. `check_phase_gates.py` has **no `--binary`**; loads `*_latest.json`. `--phase 1/2` also **hard-require ICDAR JSON** today. | P0.6: `--owned-only` + `--binary` on runners; nightly live; PR CI never ICDAR metrics. |
| A4.6 | G5.4 classic-stream check is source substring grep | **Open** | G5.4 requires `"detect_stream_tables" in page.rs` — **false** (symbol is in `stream.rs` / `detectors.rs`; page uses `StreamDetector`). Gate already lying FAIL. | P0.4: unit test + `allow_classic_stream` in page.rs; **remove** `detect_stream_tables` substring. |
| A4.7 | G0.8 no ICDAR hardcoded True | **Fixed** | `assert_no_icdar.py` in CI python-unit + dedicated job. | — |
| A4.8 | CHANGELOG G4/G5 PASS vs `g3_industry.json` INVALID | **Open** | As A1.12. | Phase 0 doc PR. |
| A4.9 | T3 golds 23 vs claimed G5 >=25 | **Partial** | `real_track/gold/*.json` count **25**. README full suite **n=23**. Freeze g3 claims T3>=25. | Pick one number; README vs gold glob. |
| A4.10 | "No ICDAR in CI" is **false for metrics** | **Open (policy intent; enforcement broken)** | `assert_no_icdar.py` bans PDFs. `check_phase_gates --phase 1/2` in CI **hard-requires** `icdar_failure_analysis.json` + headtohead. Committed ICDAR JSON is already a merge bar. | P0.6 `--owned-only`; CI never runs ICDAR metric checks; external script only. |
| A4.11 | Four overlapping design docs; no as-built ARCHITECTURE.md | **Open** | V2 + V3 + implementation-plan + options-map + this doc. | Phase 0: this doc is as-built hardening SSOT; add `docs/ARCHITECTURE.md` stub pointing here + V2 ladder. |

### A5 Product limits (not bugs)

| ID | Limit | Classification | Track |
|----|-------|----------------|-------|
| A5.1 | No OCR / scans | **Out of scope** | Separate OCR product; Tier 2 in V2 ladder. |
| A5.2 | No encrypted open | **Out of scope** | 0.2+ crypto; empty-user-pw stays refuse in 0.1 (H16). |
| A5.3 | No tagged-PDF tables | **Out of scope** | `TableModeSet.structure` reserved; do not fake it. |
| A5.4 | No crates.io publish / 0.1.0 unstable API | **Out of scope** | Process, not architecture. |
| A5.5 | Cell / TEDS beta | **Honest label** | Keep README Beta; quality = Phase 5. |
| A5.6 | Full-page render depends on external CLIs | **Accepted** | Fast never; Auto opportunistic fail-soft; HQ explicit. |
| A5.7 | English / US-tax form keywords | **Accepted residual** | `lexicon.rs` is generic-phrase not filename; moving it is freeze-gated (A1.10). |

### B Major SOLID / code issues

| Item | Status | Evidence / residual |
|------|--------|---------------------|
| God files/functions | **Partial** | See A1.13; continue splits. |
| No real Detector/Router use | **Partial** | Traits exist (`TableDetector`, `TableRouter`); page.rs uses detector structs; router trait not used by finalize. |
| Deref mega-options | **Fixed** | No `impl Deref`. options-map stale. |
| Notes-as-control | **Partial** | See A1.9. |
| Default != Auto | **Keep (decided)** | H10: Default tables-off/legacy. User 2026-08-08: no Default==Auto in 0.1.x. |
| Magic numbers | **Partial** | `constants.rs` has letter + 140 + K26 gap. Many remain in densify/form. |
| Silent errors | **Open** | Font fallback, pop_num 0.0, unsupported filters -> None rasters. |
| Crate-wide clippy allows | **Fixed** | Workspace clippy `-D warnings` in CI. File-level allows remain (`needless_range_loop`, etc.). |
| Duplication (stack-merge / numeric / keywords / IoU) | **Partial** | `lexicon.rs`, `stack_merge.rs`, `geom.rs` / `policy` IoU still overlap slightly. |
| Tests that lie | **Partial** | A4.1 Fixed. Residual: taxonomy all-unknown; census doesn't lock cells; G5.4 false grep. |
| `metrics.py` honesty | **Partial** | Façade over `metriclib/`. `run_accuracy_benchmark.py` still 824 LOC. |
| Python god scripts | **Open** | Split later; not product behavior. |
| External render timeout doesn't kill | **Fixed** | `run_timed` `child.kill()` + `wait`. |
| Governor / security | **Partial** | Charge-then-check fixed; ToUnicode fresh gov open (A2.23); q/Q unbound (A2.12). |
| Unused evidence clones | **Open** | `runs.to_vec()` / `raster_pages.to_vec()` on diagnostics path. Product `detect_tables_page_with_raster` does **not** build evidence. |

---

## Background & Motivation

### Current product Auto (as-built)

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

This is **exclusive routing of already-built tables**, not exclusive **page flavor**. It preserves nested keep and detection discipline on owned suites. It is also why ICDAR still over-detects when lattice is weak: network/hybrid already ran.

### Pain points that hardening must remove without moving Auto

1. **Cannot trust freeze claims** (A1.12) — no-regression is undefined.
2. **Cannot review** remaining god files / dual router entry points.
3. **Parse bugs** silently corrupt rules/text; fixing them *will* move tables if mixed into refactors.
4. **V3 exclusive-first** is the right architecture but is a **behavior change**. Shadow first.
5. **Quality work** on census/NIPA will fight densify + form keywords + method_rank if done first.

### Existing design docs — what we keep vs contradict

| Doc | Keep | Conflict (this doc wins on as-built) |
|-----|------|--------------------------------------|
| `design-table-engine-v2.md` | K1–K36 capability ladder, ICDAR ban, Fast never render, Form resolver, nested keep, K25/K26/K27 | Migration "Auto still legacy until G1" — **Auto already Engine V2**. |
| `design-table-engine-v3-industry.md` | S0–S5 exclusive-first target, precision gates, densify inside frame | Numbers (ICDAR pred 432) stale; do not retune to them. Implementation is **not** S2 today. |
| `implementation-plan-v3-gated.md` | One failure family per phase; ICDAR never CI; GATE metric defs | Do not claim GATE-3/4/5 PASS. This hardening plan is a **prerequisite rail** beside that quality train. |
| `options-deprecation-map.md` | 12-field product surface; presets | Deref claim stale; `use_engine_v2` still a top-level field. |
| `phase-structure-gates.md` | Phase 1 framework PASS; ICDAR not in engine | Phase 2/3 status disagrees with AUTONOMOUS_PROGRESS. |
| README / CONTRIBUTING | ICDAR policy; Auto = Engine V2; honesty labels | README ICDAR #2 table vs `icdar-competitive-report.md` #1 solo board. |

---

## Goals & Non-Goals

### Goals

1. Single honest status plane + live No-Regression Contract.
2. Bit-identical SOLID completion: remaining splits, borrowed evidence, single router entry, notes diagnostic-only.
3. Parse/runtime correctness behind compare, then opt-in default flips.
4. Exclusive-first **shadow in Phase 3**, honest scores on soup in Phase 4, **H-Flip (H19)** only if shadow diff = 0 on core; Fast/Auto never slower after flip.
5. Honest detector scores (joints, whitespace, method_rank, stitch) freeze-gated.
6. Quality algorithms last, gated, no discipline/fp/nested regression.
7. ASCII-only docs; as-built ARCHITECTURE pointer.

### Non-Goals

| Non-goal | Why |
|----------|-----|
| ICDAR #1 / TEDS CI gate | Policy |
| Filename-keyed thresholds | Policy |
| OCR, encryption open, tagged tables, crates.io | A5 |
| Type3 charprocs, XFA, GoTo, image pixels in Objects API | Product cut-line |
| Flipping `Default` to Auto | Semver / embedders |
| Mandatory full-page render on Auto | Perf; Fast contract |
| Deleting classic `stream.rs` in this program | After M4 + zero call sites |
| Claiming GATE-3/4/5 green | Freeze INVALID / live disagree |

---

## Strategy (ranked phases)

H19 is the only flip order. ASCII:

```text
Phase 0  Measure + freeze honesty + executable owned gates + dump compare
Phase 1  Bit-identical architecture (parallel slices; dump-compare required)
Phase 2  Parse/runtime behind compare  (P2 warnings/governor may start after P0)
Phase 3  Exclusive-first SHADOW only   (not returned; not H-Flip)
Phase 4  Score honesty on SOUP Auto    (changes survivors; freeze A/B)
         |
         v
      Re-run Phase 3 shadow on post-Phase-4 soup binary
         |
         v
      H-Flip  exclusive-first becomes product Auto
              iff shadow diff = 0 on g2 core ids
              AND owned-gate floors hold
Phase 5  Quality algorithms            (after H-Flip or still on soup if H-Flip waits)
```

**Minimum mergeable increment (ship without H-Flip):** P0.1 + **P0.6** (harness) + P1.3 (`merge_then_partition`) + dump-compare green on core. That is a complete honesty + bit-identity rail.

### Why this order

| If we started with… | What breaks |
|---------------------|-------------|
| Census / TEDS (A3.1–A3.12) | Auto cells move; densify fights 07/59; no honest baseline. |
| Big exclusive-router rewrite | Recall holes; nested 42 risk; no shadow diff. |
| H-Flip then Phase 4 score honesty | Different experiment: exclusive on fabricated joints, then joints change survivors. |
| Parse correctness in a split PR | Fat-bar / encoding blamed on "refactor". |
| Letter page_size as "mechanical" | Area gates move on non-letter pages. |

### Phase 0 — Measure & freeze honesty + executable contract

**Goal:** One status plane. `--owned-only` ICDAR-free merge bar. `--binary` on runners. Dump-compare tool. Taxonomy classifies. ICDAR solo board cannot claim rank #1. Latency freeze file with hardware.

**Allowed:** docs, harness, freeze metadata, gold *consistency* review (R005: open PDF; do not blindly take 5x1). **No detector math.**

**Exit:** `docs/STATUS.md` SSOT; CHANGELOG corrected; `g3_industry` not cited PASS; taxonomy not all-unknown; `check_phase_gates.py --owned-only` exists and is what CI/PR contract invokes; `compare_table_dumps.py` exists; `owned_gates_v0.json` + `latency_fast_v0.json` exist.

### Phase 1 — Bit-identical architecture / SOLID

**Goal:** Same tables on freeze dumps. Parallel slices (no false deps). `merge_then_partition` shared. Diagnostics `PageEvidence` borrowed. `DensifyMode` added with no math.

**Forbidden:** densify default change; skipping detectors; constructing `PageEvidence` on Auto detect; page_size letter->media; encoding/VM paint.

### Phase 2 — Parse / runtime correctness behind compare

**Goal:** One concern per PR. Typed warnings + shared ToUnicode governor may start **after P0** (do not wait for P1 exit). Stroke width / form BBox / encodings isolated.

**Rule:** Default flip only after dump no-drop **or** intentional sensing changelog.

### Phase 3 — Exclusive-first shadow (not H-Flip)

**Goal:** Cheap `LineProbe` + `PageClassifier`. Shadow runs soup || exclusive, logs table diffs **and detector invocation counts**. Product Auto still soup+finalize. Incremental **invocation-skip** (Alt 4) behind dump-compare.

**Perf:** Shadow 2x — `advanced.shadow_exclusive_first` default false on **all** presets.

### Phase 4 — Detector discipline / honesty of scores (still soup)

**Goal:** Real joints, honest whitespace, method_rank, K26 page median, stitch telemetry. **On product soup Auto.** Nested 42 + fp_strict + discipline hold.

### Phase 5 — Quality algorithms

**Goal:** Census 10-col, NIPA, R008 IoU, schools cell — **separate PRs**. Densify `InsideFrameOnly` default flip as its own PR.

**Guards:** owned-gate floors; 07/59; doc 42; no ICDAR filenames.

---

## Proposed Design

### Target product Auto **after H-Flip** (H19; not after Phase 3)

```text
                    +------------------+
                    |  extract.rs      |
                    |  Form resolver   |
                    |  rasters (no     |
                    |  render on Fast) |
                    +--------+---------+
                             |
                             v
                    +------------------+
                    | PageEvidence<'a> |   borrowed runs/rules/rasters
                    | LineEvidence     |   no clone of all runs
                    | LineProbe        |   counts + joint proxy
                    +--------+---------+
                             |
                             v
                    +------------------+
                    | PageClassifier   |   Ruled | Partial | Borderless | None
                    | (S2)             |   uses probe + cheap text stats
                    +--------+---------+
                             |
           +-----------------+-----------------+----------------+
           v                 v                 v                v
      Ruled only       Partial only     Borderless only        None
      + nested         (hybrid frames)  (network once)         []
      ruled children
           |
           v
      precision gates (whitespace, min cells, prose, budget, nested)
           |
           v
      emit order (-y1, x0) -> optional stitch
```

**Today vs target:** today all builders run then gates. After H-Flip: **one primary builder + at most one residual** (nested remainder lattice, or after-form network if form wiped all solid grids). Never three network invokes. Flavor is **primary + optional residual**, not a single enum that silently drops recovery.

### Today vs after Phase 1 (still same Auto tables)

```text
Phase 1 (bit-identical):

  detect_tables_page_with_raster     # still NO PageEvidence construct
       |
       +-- LatticeDetector / HybridDetector / NetworkDetector
       |
       v
  finalize_engine_v2
       tables_to_proposals
       merge_then_partition(...)     # NEW shared fn: vertical_merge + partition, NO sort
       emit_tables_from_accepted     # same walk order as today
       engine_v2_exclusive_cleanup
       sort_tables_by_emit_order     # existing K27 after emit

  route_proposals()                  # tests / trait: merge_then_partition THEN sort proposals
```

### Module map (target; keep crate, reshape internals)

```text
crates/pdfparser-tables/src/
  lib.rs                 # re-exports only
  types.rs / options.rs / constants.rs / tuning.rs / lexicon.rs / geom.rs
  stats.rs               # KEEP (CellStats / form numeric density)
  lattice.rs             # KEEP thin re-export of builders/ruled (P1.6: keep adapter)
  evidence/
    mod.rs               # borrowed from_inputs (diagnostics P1.2)
    page.rs              # PageEvidence<'a> + PageEvidenceOwned
    line.rs              # LineEvidence
    probe.rs             # LineProbe (P3.2 only on product path)
  detectors.rs           # TableDetector / TableRouter traits (keep)
  builders/
    densify.rs           # DensifyMode in P1.8; default Primary
    ruled/               # split further when touching
    hybrid.rs / network/
  orchestrator/
    page.rs              # soup until H-Flip
    engine_v2.rs         # finalize via merge_then_partition
    classify.rs          # today strong/solid + P3 PageClassifier
    prefer.rs / geom_util.rs
  router/mod.rs          # merge_then_partition + route_proposals
  policy/ form.rs stitch.rs split.rs stack_merge.rs
  stream.rs              # experimental only
  raster/morph.rs providers/

crates/pdfparser-content/src/vm/
  mod.rs / path.rs / state.rs / text.rs
```

### Trait sketches (extend current; do not reinvent)

```rust
// P1.2 diagnostics type only -- product detect does not construct this yet
pub struct PageEvidence<'a> {
    pub page_index: u32,
    pub page_width: f32,
    pub page_height: f32,
    pub runs: &'a [pdfparser_ir::TextRun],
    pub rules: &'a [pdfparser_content::RuleSegment],
    pub lines: LineEvidence,             // diagnostics path MAY build from rules
    pub raster_pages: &'a [RasterPage],  // borrow pixels; do not clone
    pub proposals: Vec<RegionProposal>,
    pub diagnostics: EvidenceDiagnostics,
}

// Dump clone -- only if shadow_diagnostics && dump requested (--dump-evidence)
pub struct PageEvidenceOwned {
    pub page_index: u32,
    pub page_width: f32,
    pub page_height: f32,
    pub runs: Vec<pdfparser_ir::TextRun>,
    pub rules: Vec<pdfparser_content::RuleSegment>,
    pub lines: LineEvidence,
    // raster_pages: omit pixel buffers in dump by default (width/height/scale only)
    pub raster_meta: Vec<(u32, u32)>,
    pub proposals: Vec<RegionProposal>,
    pub diagnostics: EvidenceDiagnostics,
}

// P3.2 -- classify-time plan only. Residual after-form network is NOT a probe bit
// (only knowable after lattice + form discriminator; see build_plan step 3).
pub struct PagePlan {
    pub primary: PageFlavor,           // Ruled | Partial | Borderless | None
    pub nested_remainder_lattice: bool,
}

pub struct LineProbe {
    pub n_h: u32,              // axis H rules len >= 8
    pub n_v: u32,
    pub n_joints_proxy: u32,   // H-V near-crossings; O(n_h*n_v) segments, NOT lattice CC
    pub multi_col_text: bool,  // left-edge clusters >= 2 and n_runs >= 12; NO alignment peaks
    pub incomplete_frame: bool // 2+ long H and 2+ long V nearly a rect; NOT HybridDetector
}

// TableDetector signature unchanged.
```

`ExclusiveAutoRouter::route` stays a thin wrapper around `route_proposals` (tests). Product finalize uses `merge_then_partition` (H18).

### PageEvidence without cloning (P1.2 vs P3.2)

| Path | Today | After P1.2 | After P3.2 |
|------|-------|------------|------------|
| `detect_tables_page_with_raster` | no evidence | **still no construct** | borrowed evidence for classifier only; no run/pixel clone |
| `detect_tables_page_with_diagnostics` | clones runs+rasters always | borrow; `LineEvidence` allowed | same |
| `--dump-evidence` | diagnostics clone | `PageEvidenceOwned` only if dump requested | same |
| Builders | `&[TextRun]` | unchanged | unchanged |

**Forbidden on Auto/Fast detect:** `runs.to_vec()`, raster pixel clone, alignment peaks over all runs, `LineEvidence` build before P3.2.

### Exclusive-first + shadow mode

Flavor = **primary + optional nested remainder** (classify-time). After-form network is a **build_plan post-condition** on Ruled pages, not a probe bit.

```text
function detect_page(inputs, opts):
  if opts.legacy_router: return soup_nms(...)

  soup = soup_then_finalize(inputs, opts)     # product through Phase 4

  if opts.advanced.shadow_exclusive_first:    # default FALSE all presets
      plan = classify_probe(inputs)           # cheap probe; NO LatticeDetector
      excl = build_plan(plan, inputs, opts)
      excl = finalize_engine_v2(excl, ...)
      log_shadow_diff(soup, excl)             # tables + detector_invocation_counts
      if plan.primary == None and soup nonempty:
          log FAIL "classify None but soup emitted"   # must not ship []
      # still return soup

  if opts.advanced.exclusive_first_live:      # H-Flip only
      return build_plan(classify_probe(inputs), ...)

  return soup
```

**Cheap probe (no LatticeDetector, no alignment peaks, no densify):**

```text
function classify_probe(rules, runs, opts) -> PagePlan:
  probe = LineProbe from rules + left-edge cluster stats on runs
  plan = { primary: None, nested_remainder_lattice: false }

  if probe.n_h >= 2 and probe.n_v >= 2 and probe.n_joints_proxy >= 5:
      plan.primary = Ruled
      plan.nested_remainder_lattice = true   # second lattice on remainder only
      return plan
  if probe.incomplete_frame and probe.multi_col_text:
      plan.primary = Partial
      return plan
  if borderless_prefilter_cheap(runs, probe, opts):
      plan.primary = Borderless
      return plan
  return plan

# Existing product knobs only. No new ICDAR-derived numbers.
# Left-edge cluster gap = 12pt (same as extract.rs want_full_page_render).
# stream_min_body_bands default 3 (network/mod.rs already uses this floor).
function borderless_prefilter_cheap(runs, probe, opts) -> bool:
  if probe.incomplete_frame:
      return false                    # Partial already claimed
  if not probe.multi_col_text:
      return false                    # n_runs >= 12 and >= 2 left-edge clusters
  n_clusters = probe left-edge cluster count (gap > 12pt)
  min_bands = max(opts.advanced.stream_min_body_bands, 3)
  if n_clusters < min_bands:
      return false                    # 2-col prose stays None, not Borderless
  return true
```

**`build_plan` (H-Flip live path and shadow exclusive path):**

```text
Ruled:
  1. LatticeDetector once on full page
  2. Nested remainder: if >=1 solid lattice emitted, run LatticeDetector
     AGAIN only on runs+rules OUTSIDE padded bbox(es) of those solids
     (bbox mask). Max one extra lattice pass. This is doc-42-class
     (today both survivors are lattice 7x2 + 4x2).
  3. Residual network (POST-CONDITION, not PagePlan bit): ONLY if form
     discriminator wiped ALL solid lattices (same predicate as today's
     after-form recovery). NetworkDetector once. Never also do overwide
     hybrid re-invoke (cap 1 network). Shadow logs residual_network_ran.
Partial:
  HybridDetector only. No lattice.
Borderless:
  NetworkDetector once. Classic stream only if allow_classic_stream.
None:
  []
```

**If classify says None but soup emitted tables:** shadow **fails** (must not silently ship []). H-Flip blocked.

**Cost model after H-Flip (probe is O(rules)+O(runs stats), not a builder):**

| Page flavor | Builders today (typical) | After H-Flip builders |
|-------------|--------------------------|------------------------|
| Strong ruled, no form wipe | lattice+hybrid+network (+maybe after-form) = 2-4 | lattice + optional remainder lattice = 1-2 |
| Ruled + form wipe residual | 3-5 | lattice + 1 network = 2 |
| Partial | lattice+hybrid+network = 3 | hybrid = 1 |
| Borderless | 3-6 | network = 1 |
| None / chrome | 3 | 0 builders (+ cheap probe) |

Partial/Borderless do **not** run LatticeDetector.

**Shadow:** soup + exclusive = 2x; `advanced.shadow_exclusive_first` default false on **all** presets including EngineV2. Log `detector_invocations` per path.

**Alt 4 incremental (Phase 3, still soup Auto):** skip `NetworkDetector.detect` when `ruled_owns_page && !recovery predicates` (today still *runs* then filters). Skip `HybridDetector.detect` when probe says no exterior frame AND `ruled_owns_page`. Dump-compare required. Safer bit-identity perf than full dispatch.

**H-Flip gate (after Phase 4 soup is baseline):**
- Shadow re-run on that binary: **diff = 0** on g2 **core ids** for count, method, bbox snap (0.5 pt)
- Owned-gate floors (`owned_gates_v0.json`) hold
- Doc 42 = 2 tables
- 07/59 cargo tests
- Fast latency: see Latency section
- Then `advanced.exclusive_first_live = true` on Auto preset
- Rollback: `legacy_router=true` or `exclusive_first_live=false`

### Densify demotion (not same PR as refactor)

```text
enum DensifyMode { Primary, InsideFrameOnly, Off }

# P1.8: add enum, map lattice_text_densify false=>Off true=>Primary; NO math change
# P5.2: Auto may switch default to InsideFrameOnly after freeze A/B
```

Inside-frame-only: only propose missing H/V when `edge_cover < tau` **and** anchors stay within accepted joint/frame bbox. No exterior X expand in that mode (today `expand_xs_exterior_text_cols` is a known NIPA/census lever — flipping it is quality, not hygiene).

### Options story

```text
TableOptions::default()
  detect_tables = false
  legacy_router = true
  use_engine_v2 = false
  allow_classic_stream = false
  # KEEP this. Product is from_preset(Auto).

TableOptions::from_preset(Auto|Full)
  detect_tables = true
  use_engine_v2 = true
  legacy_router = false
  allow_classic_stream = false
  allow_auto_render = true     # Fast: false; never render if Fast
  enable_full_page_render = false
  shadow_diagnostics = false   # EngineV2 preset true (telemetry only; NOT exclusive shadow)
  advanced = TableAdvancedOptions (serde flatten)

  # H20 -- NEW FLAGS ON advanced ONLY (not a 13th top-level field):
  advanced.exclusive_first_live = false      # H-Flip
  advanced.shadow_exclusive_first = false    # all presets including EngineV2
  advanced.densify_mode = Primary            # P1.8

Do NOT add Deref.
Do NOT grow past 12 top-level fields.
CLI: --tables-shadow-exclusive / --exclusive-first map to advanced.
Rollback stays existing top-level legacy_router / --legacy-router.
```

### Notes: diagnostic only

| Today | Target |
|-------|--------|
| typed flags + note clones | keep notes as telemetry writes |
| `contour_seed_match` `notes.iter().any` uniqueness | P1.4: use a `bool` on the table/loop; keep note write |
| form `form_likeness=0.xx` note | keep diagnostic |
| Product `notes.iter().any` except tests | CI grep after P1.4 |

### VM / filter LLD

**Typed warnings (Phase 2a — no math change):**

```rust
pub enum VmWarning {
    UnknownOperator(&'static str),
    StackUnderflowNumeric,
    MaxPageOps,
    FormCycle,
    FormDepth,
    FormExpansions,
    ClipAabbOnly,
    IgnoredOperator(&'static str), // gs, color, BMC, ...
    StrokeWidthIgnored,            // until A2.6 lands
}
```

Map in `extract.rs`: `UnknownOperator` / `Other` / `LimitSoft` / `MissingToUnicode` (fonts).

**A2.6 stroke width (Phase 2, opt-in `InterpretOptions::stroke_width_rules`):**

```text
if capture_rules and gs.line_width * ctm_scale >= thin_fill_rule_max:
    emit axis-aligned stroke as a thin rectangle (two long edges or
    filled bar of width w), not a 1-D centerline.
default false until freeze no-drop or intentional sensing changelog
```

**DCT vs unsupported (as-built keep; document):**

```text
Flate/AHx/A85/RL/LZW -> decode
DCT -> pass bytes through; image crate JPEG decode in raster_images
JPX / CCITTFax / JBIG2 -> Error::Unsupported at filter; raster fail-soft skip
Do not pretend DCT is unimplemented.
```

**q/Q cap:** `gstack.len() >= limits.max_nesting_depth` -> warn `LimitSoft`, ignore extra `q`. Page tree already errors. Default 64.

**ToUnicode governor:** `load_page_fonts(doc, refs, &doc.governor)` — pass `&ResourceGovernor` from `PdfDocument`.

**Curves:** already keep path; optional later chord approximation is **sensing change**, not Phase 1.

### Eval harness LLD

**Today (pre-P0.6) — do not invent flags:**

- Runners (`run_detect_discipline.py`, `run_fp_strict.py`, `run_real_structure.py`) have **no `--binary`**; they call `find_binary()` / hardcode `target/release/pdfparser`.
- `run_real_structure.py` already forces stitch off internally (no `--no-stitch` argparse). No `--dump-json`.
- `run_latency_probe.py` hardcodes `BIN = target/release/pdfparser`.
- `check_phase_gates.py` accepts only `--phase` and deprecated `--with-icdar`. It **never runs a binary**. `--phase >= 1` **hard-requires ICDAR JSON** in `benchmark/results/`.
- No dump-compare script exists.

**P0.6 must add (then contract below is copy-pasteable):**

1. `--binary PATH` on discipline, fp_strict, real_structure, latency_probe (default = today's `find_binary()`).
2. `check_phase_gates.py --owned-only` : discipline + fp_strict + g2 core cell/det + doc 42 + (optional committed JSON paths). **No ICDAR files, no G1.10/G1.11/G2.6/G2.7.** `--phase 1` without `--owned-only` stays external-only (not CI).
3. `dump_product_tables.py` + `compare_table_dumps.py` (schema below; `--freeze g2.json --structure-manifest real_structure_v0.json`; one CLI extract per PDF).
4. `freezes/owned_gates_v0.json` (H21 floors).
5. CI: replace hard `--phase 1` / `--phase 2` with `--owned-only`. Never run ICDAR metrics in CI.

**Taxonomy (P0.2):** read `metrics.per_table[].pred_shape/gold_shape`. Default invocation = real_structure only. `--icdar-analysis` stays **external optional**; not CI; not Phase 0 exit.

**G5.4 (P0.4):** unit test `auto_disables_classic_stream` + `"allow_classic_stream"` in `page.rs`. **Remove** `"detect_stream_tables" in page.rs` substring.

**Do not add failing 10-col census assert before Phase 5.**

### Performance budget

| Constraint | Rule |
|------------|------|
| Fast | `allow_auto_render=false`, `enable_full_page_render=false`. No render spawn. |
| Auto | No new mandatory full-page render. |
| Asymptotics | Probe is O(n_h*n_v) on **rule segments** + O(runs) left-edge clusters. **No** LatticeDetector preview. **No** all-pairs runs x rules. |
| Alloc | No clone of all `TextRun`s / raster pixels on Auto/Fast. |
| Detector count | After H-Flip: match cost table (Partial/Borderless = 1 builder; no lattice). |
| Shadow | Excluded from Fast/Auto latency; default off all presets. |

### Dump-compare LLD (`dump_product_tables.py` + `compare_table_dumps.py`)

**`g2_core.json` does not exist.** P0.6 does **not** invent it. Resolve core docs as:

```text
ids = freeze g2.json documents_auto[].id
pdf  = real_structure_v0.json documents[id].pdf   # e.g. corpus/real/30_....pdf
```

Wrapper flags (after P0.6):

```text
python3 benchmark/scripts/dump_product_tables.py \
  --binary target/release/pdfparser \
  --freeze benchmark/real_track/freezes/g2.json \
  --structure-manifest benchmark/real_track/manifests/real_structure_v0.json \
  --out /tmp/after.json
```

**One CLI invocation per PDF** (never two Auto extracts — K25 render could diverge):

```text
pdfparser extract --tables --no-stitch --page-tables \
  --format json --dump-evidence --table-preset auto PATH
  # stdout: CLI JSON (pages[].tables = serde Table)
  # stderr: evidence JSON (optional; engine_path is options-derived)
```

`engine_path` in the dump is **filled from preset flags** (`use_engine_v2 && !legacy_router` => `engine_v2`), not by parsing stderr. `--dump-evidence` stays on the same process for humans; compare does not require it.

Merged dump schema (`after.json` / `before.json`) — `compare_table_dumps.py` reads only this:

```text
{
  "schema": "pdfparser_table_dump_v1",
  "binary": "target/release/pdfparser",
  "preset": "auto",
  "engine_path": "engine_v2",
  "stitch_multipage": false,
  "documents": [
    {
      "id": "30_real_ca_warn_report",
      "pdf": "benchmark/corpus/real/30_real_ca_warn_report.pdf",
      "pages": [
        {
          "index": 0,
          "tables": [ /* serde Table, sorted (-bbox.y1, bbox.x0) */ ]
        }
      ]
    }
  ]
}
```

Compare sort key (named): **`(-bbox.y1, bbox.x0, page)`** (K27).

Must-match: count, method, rows, cols, bbox abs<=0.5pt, cell indices/spans/text, header_rows, weak_edges, typed flags, strategy_provenance **set**, document-level `engine_path`. **Not** per-table EnginePath.

Allowed: notes text, confidence <=1e-4, elapsed, diagnostics pretty-print.

---

## API / Interface Changes

| Change | Phase | Semver / behavior |
|--------|-------|-------------------|
| `PageEvidence<'a>` borrowed | P1.2 diagnostics only | 0.1.x unstable. `PageEvidenceOwned` for dump. |
| `VmWarning` | P2.1a after P0 | Additive. |
| `TableAdvancedOptions.densify_mode` | **P1.8** add / P5.2 flip | Default Primary = today. |
| `advanced.shadow_exclusive_first` / `exclusive_first_live` | P3.3 / H-Flip | Default false **all presets**. |
| `extract_document` honors `opts.tables` if detect_tables | P2.8b | Default still off. |
| `TableOptions::default()` | — | **Unchanged.** |
| Public field count | — | Stay <= 12 top-level (H20). |

No Deref. No new crate.

---

## Data Model Changes

| Item | Change | Migration |
|------|--------|-----------|
| `PageInfo.resources` | Enum None / Ref / Inline | P2.2c additive. |
| `Table.joint_count` | Real or 0 (unknown). Stop stuffing min_joints. | Phase 4. |
| Freeze JSON | `owned_gates_v0.json`, `latency_fast_v0.json`; g3 revoked | P0.5 / P0.6 |
| Gold | R005/R010: open both golds + PDF (P0.3). If 5x1 is not a real table, fix structure gold. Do not blindly copy structure into discipline. | User 2026-08-08. |

---

## No-Regression Contract

Until **P0.6** lands, treat this section as **intent**. Commands in "After P0.6" are the merge bar. Do not call pre-P0.6 invented flags normative.

### Commands that run on today's tree (pre-P0.6)

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
python3 benchmark/scripts/assert_no_icdar.py
python3 benchmark/scripts/check_phase_gates.py --phase 0

cargo build --release -p pdfparser-cli
# runners discover target/release/pdfparser (NO --binary yet)
python3 benchmark/scripts/run_detect_discipline.py
python3 benchmark/scripts/run_fp_strict.py
python3 benchmark/scripts/run_real_structure.py
python3 benchmark/scripts/run_latency_probe.py

cargo test -p pdfparser --test phase_v_tables --test phase_u_tables \
  --test phase15_census --test phase14_multitable --test form_table_rules
```

**Do not** run `check_phase_gates.py --phase 1` or `--phase 2` as a PR merge bar today: they load ICDAR metrics (H8). CI must stop doing that in P0.6.

Mechanical dump today (manual):

```bash
./target/release/pdfparser extract --tables --no-stitch --page-tables \
  --format json --table-preset auto PATH > /tmp/after.json
```

No automated comparator until P0.6.

### Commands after P0.6 (copy-paste merge bar)

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
python3 benchmark/scripts/assert_no_icdar.py
python3 benchmark/scripts/check_phase_gates.py --phase 0
python3 benchmark/scripts/check_phase_gates.py --owned-only

cargo build --release -p pdfparser-cli
BIN=target/release/pdfparser
python3 benchmark/scripts/run_detect_discipline.py --binary "$BIN"
python3 benchmark/scripts/run_fp_strict.py --binary "$BIN"
python3 benchmark/scripts/run_real_structure.py --binary "$BIN"
python3 benchmark/scripts/run_latency_probe.py --binary "$BIN"
python3 benchmark/scripts/check_phase_gates.py --owned-only

cargo test -p pdfparser --test phase_v_tables --test phase_u_tables \
  --test phase15_census --test phase14_multitable --test form_table_rules

# mechanical PRs:
python3 benchmark/scripts/dump_product_tables.py --binary "$BIN" \
  --freeze benchmark/real_track/freezes/g2.json \
  --structure-manifest benchmark/real_track/manifests/real_structure_v0.json \
  --out /tmp/after.json
python3 benchmark/scripts/compare_table_dumps.py --before /tmp/before.json --after /tmp/after.json
```

Doc-only P0.1 may skip live runners. Detector PRs: author runs live `--binary` locally; **nightly on main required** (H15).

### Owned-gate floors (H21) — `freezes/owned_gates_v0.json`

SSOT for this doc and `--owned-only`. Core ids = `g2.json` `documents_auto[].id`. g2 auto cell F1 = **0.637**.

| Metric | Floor | Suite |
|--------|-------|-------|
| micro cell F1 | g2 auto − **0.03** (G1.7) | real_structure core |
| micro det count F1 | g2 auto − **0.02** (G1.6) | core |
| micro det IoU F1 | g2 − 0.03 when present | core |
| discipline exact_count_rate | **>= 0.88** | detect_discipline_v1 |
| over_doc_rate | **<= 0.12** | discipline |
| fp_strict zero_rate | **>= 1.0** (n=12; freeze current live) | real_fp_smoke_v1_strict |
| nested doc 42 | **2** tables; cell F1 >= freeze_42 − 0.05 | real_structure |
| stream 07 / 59 | cargo pass | owned synthetic |
| ICDAR | **not a gate** | external script only |

No `live-main − 0.02`. Do not use `g3_industry.json` as PASS. Do not mix n=23 micro (~0.656) with core n=15.

### Latency (unambiguous)

P0.5 writes `freezes/latency_fast_v0.json` from a **recorded** `run_latency_probe.py` sample:

```text
recorded_p50_ms, recorded_p95_ms, recorded_max_ms, n_docs=8
hardware: runner_os / note (e.g. "local M-series" or "ubuntu-latest nightly TBD")
budget_p95_ms = max(recorded_p95_ms * 1.5, recorded_max_ms * 1.2)
  # current latest: p95~409, max~604 => budget = max(613.5, 724.8) = 724.8 ms
```

**Two independent fail rules. Do not wrap in `min()`** (`p95×1.10` is always < budget when p95 ≤ max, so `min` would make budget dead):

1. **Absolute ceiling:** fail if live p95 > `budget_p95_ms` (this sample ~725ms).
2. **Same-class drift:** fail if live p95 > `prev_commit_p95_ms × 1.10` on **that same machine class** (nightly stores prev p95 per class; first sample on a class has no drift check, only rule 1).

Do **not** gate live p95 against freeze `recorded_p95 × 1.10` (that is the flaky n=8 ceiling Issue 11 rejected). Do **not** claim 30s→0.6s tightening is done until a live ubuntu-latest sample exists; until then `budget_p95_ms=30000` in the probe script stays informational only — freeze file still records the real ~725ms budget for nightly/local.

Fast: no render. Shadow exclusive excluded. Auto: no new asymptotic / no mandatory render.

### Rollback

| Unit | Mechanism |
|------|-----------|
| One PR | `git revert` of that PR |
| Router | `TableOptions.legacy_router = true` / CLI **`--legacy-router`** |
| Exclusive-first (post H-Flip) | `advanced.exclusive_first_live = false` |

**Not implemented / not this program:** `PDFPARSER_TABLE_LEGACY`, `--tables-legacy`.

---

## Mapping (every A/B item)

| ID | Phase | PR (see plan) | Status | Risk | Mitigation |
|----|-------|---------------|--------|------|------------|
| A1.1 | 3 / H19 | P3.2–P3.4, H-Flip | Open | Behavior / recall | Shadow; H-Flip after P4 soup; diff=0 on core |
| A1.2 | 1+3 | P1.2 then P3.2 | Open | Perf if clone | P1.2 diagnostics only; product construct at P3.2 |
| A1.3 | 1 | P1.3 (deps P0.6 only) | Open | Bit-identity | `merge_then_partition` no sort |
| A1.4 | 1+5 | P1.8 then P5.2 | Open | Shape | Enum first; flip later |
| A1.5 | 3 | P3.4 / H-Flip residual | Open | Recall | At most one after-form network |
| A1.6 | 5 | P5.4 | Partial | Count | Multi-frame already; no union restore |
| A1.7 | — | — | **Fixed** | — | — |
| A1.8 | 1,3 | P1.5 | Partial | API | No new top-level; no Deref |
| A1.9 | 1 | P1.4 | Partial | Control | Bool for contour_seed; keep note writes |
| A1.10 | 4–5 | P4.4 / later | Open | FP | K35 A/B; no ICDAR names |
| A1.11 | 0 | P0.3 | Open | Eval honesty | Document page_filter; don't change product stitch default |
| A1.12 | 0 | P0.1 | Open | Process | STATUS SSOT |
| A1.13 | 1 | P1.1 | Partial | Review | Bit-identical splits |
| A1.14 | 1,3 | P1.6 | Open | Dead code | Contour stay diagnostic until Phase 3 flag |
| A1.15 | 2–3 | P2.6 | Open | Scores | Not mechanical; freeze compare |
| A1.16 | 2–3 | P2.6 | Open | Scores | Same PR as A1.15 |
| A2.1 | — | — | **Fixed** | — | — |
| A2.2a | — | — | **Fixed** | — | DCT path exists |
| A2.2b | later | P2.5b | Open | Sensing | JPX/CCITT/JBIG2 opt-in |
| A2.3 | — | — | **Fixed** | — | — |
| A2.4 | — | — | **Fixed** | — | — |
| A2.5 | — | — | **Fixed** | — | — |
| A2.6 | 2 | P2.3 | Open | Lattice inputs | Flag then flip |
| A2.7 | 2 | P2.4c | Open | Text extract | Opt-in clip text; default off; dump-compare |
| A2.8 | 2 | P2.1a | Open | Diagnostics | Typed warnings; BI/EI later |
| A2.9 | 2 | P2.4a/b | Open | Rules/text | Split form bbox vs form fonts |
| A2.10 | 2 | P2.1a | Open | Layout | Keep 0.0 fail-soft; typed warn |
| A2.11 | 2 | P2.1b | Open | Diagnostics | Map codes |
| A2.12 | 2 | P2.2b | Partial | Pathological PDF | Cap q/Q warn |
| A2.13 | — | — | **Fixed** | — | — |
| A2.14 | 2 | P2.2c | Open | Resources | Additive PageInfo |
| A2.15 | 2 | P2.5a | Open | Decode | Isolate; rare |
| A2.16 | 2 | P2.7a | Open | Text | Warning + freeze |
| A2.17 | 2 | P2.7b | Open | Diagnostics | Emit code |
| A2.18 | 2 | P2.7c | Partial | Text | StandardEncoding table + compare |
| A2.19 | 2 | P2.7d | Open | Text | Array bfrange |
| A2.20 | later | — | Open | CID | Out of hardening critical path |
| A2.21 | — | — | **Out of scope** | — | Type3 later |
| A2.22 | 2 | P2.7e | Open | Text | Warning only first |
| A2.23 | 2 | P2.2a | Open | Security | Shared governor; after P0 |
| A2.24 | later | — | Open | Text | Not table-blocking |
| A2.25 | 2 | P2.8a | Open | Transform consumers | Bbox already rotated |
| A2.26 | later | — | Open | IR | Not required |
| A2.27 | 2 | P2.8b | Open | API | Default tables still off |
| A2.28 | 1 | P1.7 | Partial | CLI | Optional use export |
| A2.29 | — | — | **Out of scope** | — | A5.2 |
| A2.30 | — | — | **Out of scope** | — | Objects later |
| A2.31 | later | — | Open | HQ only | Temp path optional |
| A3.1 | 5 | P5.1a | Open | Cell | Census glued/cols only |
| A3.2 | 0+5 | P0.3, P5.1c | Open | Cell + gold | NIPA separate PR |
| A3.3 | 0 | P0.3 | Open | Gold | Open PDF; do not freeze 5x1 stub blindly |
| A3.4 | 5 | P5.3 | Open | Count | After discipline lock |
| A3.5 | 5 | P5.3 | Open | IoU/cell | After region quality |
| A3.6 | 5 | P5.3 | Open | Cell | — |
| A3.7 | 5 | P5.1b | Open | Cell assign | Schools separate; shape already exact |
| A3.8–A3.10 | ext | — | Open | ICDAR | Honesty only |
| A3.11 | 4 | P4.3 | Open | Count | Nested gates; hold 42 |
| A3.12 | 5 | P5.2 | Open | Shape | 07/59 guard |
| A3.13 | 5 | P5.2 | Partial | Cols | Span-aware collapse |
| A3.14 | 4 | P4.1 | Open | Route | Honest scores |
| A3.15 | 4 | P4.1 | Open | Route | Uncap when fill known |
| A3.16 | 4 | P4.1 | Open | Route | Unknown != min_joints |
| A3.17 | 4 | P4.2 | Partial | Merge | Page median gap |
| A3.18 | 4 | P4.2 | Open | Survivor | Freeze-gated rank |
| A3.19 | 3/M4 | P3.1 | Open | Dead path | Keep gated (user 2026-08-08); delete after M4 |
| A3.20 | 4 | P4.5 | Open | Stitch | Telemetry + height infer |
| A3.21 | 5 / HQ | P5.5 | Open | Detect | No Auto mandatory render |
| A3.22 | 0 | P0.2 | Open | Harness | Fix taxonomy |
| A3.23 | 0 | P0.1 | Open | Docs | Incomplete banner |
| A3.24 | 1 | P1.1 | Partial | Docs | Rustdoc path |
| A4.1 | — | — | **Fixed** | — | Fixtures + is_file asserts exist |
| A4.2 | 0+5 | P0.2, P5.1a | Open | Tests | Shadow then assert |
| A4.3 | — | — | **Fixed** | — | — |
| A4.4 | 0 | P0.1 | Open | CI label | Stay continue-on-error; don't call PASS |
| A4.5 | 0 | P0.6 | Open | CI | `--owned-only` + nightly live |
| A4.6 | 0 | P0.4 | Open | Gate | Fix false `detect_stream_tables` grep |
| A4.7 | — | — | **Fixed** | — | PDF ban only |
| A4.8 | 0 | P0.1 | Open | Docs | Changelog |
| A4.9 | 0 | P0.1 | Partial | Count | Align README n=25 vs 23 |
| A4.10 | 0 | P0.6 | Open | CI metrics | Split ICDAR out of merge bar |
| A4.11 | 0 | P0.1 | Open | Docs | ARCHITECTURE stub |
| A5.1–A5.7 | — | — | **Out of scope** | — | Labels only |
| B Deref | — | — | **Fixed** | — | Update options-map |
| B timeout kill | — | — | **Fixed** | — | — |
| B clippy crate-wide | — | — | **Fixed** | — | File allows OK |
| B Default!=Auto | — | — | **Keep** | — | H10 |
| B traits unused router | 1 | P1.3 | Partial | — | `merge_then_partition`; trait keeps sort wrapper |
| B evidence clone | 1 | P1.2 | Open | Perf | Borrow |
| B metrics.py / god scripts | 0–1 | P0.2 | Partial | Process | Taxonomy + optional split |
| B silent errors | 2 | P2.x | Open | — | Warnings |
| B duplication | 1 | P1.1 | Partial | — | Further extract if touching |

---

## Alternatives Considered

### Alt 1 — Big-bang exclusive-first rewrite (V3 S2 now)

| Pro | Con |
|-----|-----|
| Fastest path to Camelot-class ownership | Unmeasurable no-regression; nested 42 and borderless recall likely drop; mixes A1.1 with A3 |
| Deletes soup | Cannot bisect; violates "each PR mergeable" |

**Reject** until Phase 3 shadow is green.

### Alt 2 — Quality-first (census/NIPA/TEDS immediately)

| Pro | Con |
|-----|-----|
| Customer-visible cell F1 | Status plane still lies; densify fights 07/59; ICDAR temptation; architecture still soup |
| Matches AUTONOMOUS_PROGRESS impulse | Contradicts "no TEDS-chasing" + implementation-plan "one failure family" |

**Reject** as the *start*. It is Phase 5.

### Alt 3 — Freeze Auto forever; only EngineV2 opt-in for new arch

| Pro | Zero product risk |
|-----|-------------------|
| Con | README already ships Auto = Engine V2; two eternal stacks; soup never dies |

**Reject** as end state.

### Alt 4 — Invocation-skip when today's filter would drop all outputs (migration tactic)

| Pro | Con |
|-----|-----|
| Closer to bit-identical perf than classify-first | Does not stop hybrid/network from *existing* on weak-lattice ICDAR pages (they still ran) |
| Example: do not call `NetworkDetector` when `ruled_owns_page && !recovery`; still run hybrid if non-overlapping frames can emit | Still soup architecture |

**Accept as Phase 3 incremental (P3.4)** behind dump-compare. **Full PageFlavor + residual plan remains the H-Flip end state** because ICDAR-class over-detect happens when lattice is *weak*: hybrid/network already ran (Overview). Invocation-skip cannot fix that; only not running those builders can.

### Chosen: honesty -> bit-identical -> isolated correctness -> shadow exclusive + invocation-skip -> honest scores on soup -> H-Flip -> quality

Matches H1–H22 without pretending Auto is still legacy.

---

## Security & Privacy Considerations

| Threat | Severity | Mitigation |
|--------|----------|------------|
| ToUnicode / font streams bypass document governor (A2.23) | **High** | Phase 2: shared `ResourceGovernor` |
| Unbounded `q` stack (A2.12) | **Med** | Cap to `max_nesting_depth`; warn |
| Expansion bomb via filters | **Low** (fixed charge/check) | Keep `check_expand_ratio` then `charge` |
| Encryption bypass / empty password | **N/A this program** | Stay hard-error (H16) |
| External render CLI | **Med** | Fast never; timeout kill already; no new tools |
| Shadow exclusive doubles work | **Low** | Default false on **all** presets; only `--tables-shadow-exclusive` |
| Keyword lexicon PII | **None** | Phrases are form chrome, not user data |

Do not weaken governor or encryption gate (CONTRIBUTING).

---

## Observability

| Signal | Where | Use |
|--------|-------|-----|
| `EnginePath` | dump-evidence | legacy vs engine_v2 |
| `MethodMix` | dump-evidence | lattice/hybrid/stream counts |
| `PageFlavor` (new) | shadow dump | classify vs soup survivor methods |
| Shadow diff JSON | `shadow_exclusive_first` | count/bbox/method deltas per page |
| `VmWarning` histogram | extract warnings | unknown vs ignored vs underflow |
| Freeze scripts exit codes | CI / nightly | 0/1/2 per `check_phase_gates.py` |
| Latency probe p50/p95 | `latency_probe_latest.json` | Fast budget |
| Taxonomy mode_counts | must not be all `unknown` after P0.2 | harness health |

**Alerting (process):** nightly **owned-only live** fail = block detector merges. Never alert on ICDAR JSON. Phase 3–5 quality red = report only until honest green.

Logging: prefer structured fields on `EvidenceDiagnostics` (V2 K14); no new required `tracing` dep.

---

## Rollout Plan

```text
P0 docs + P0.6 harness     -> merge anytime after tests
P1 mechanical (parallel)   -> merge if dump-compare identical
P2 correctness (after P0)  -> flag off -> dump compare -> default flip PR
P3 shadow + invocation-skip -> advanced.shadow_exclusive_first default false
P4 score honesty on SOUP   -> freeze A/B each PR
Re-run shadow on post-P4 soup
H-Flip (H19)               -> advanced.exclusive_first_live on Auto
P5 quality                 -> freeze A/B; owned-gate hold
M4 later                   -> delete soup adapters (>=1 minor after H-Flip)
```

Flags: top-level `legacy_router`; advanced `exclusive_first_live`, `shadow_exclusive_first`, `densify_mode`; `InterpretOptions::stroke_width_rules`.

CLI: existing `--legacy-router`; add `--tables-shadow-exclusive` (maps to advanced).

Rollback: revert one PR; or `--legacy-router`.

---

## Open Questions

None remaining. Product calls below are **final** (user 2026-08-08). Latency rule was decided in rev 3 (design, not a product fork).

| Q | Decision | Binding |
|---|----------|---------|
| Empty-user-password encryption in 0.1? | **Decided (user 2026-08-08): Keep hard-error** (H16). Any `/Encrypt` stays `Error::Encryption`. | No 0.1 crypto track. |
| `TableOptions::default() == Auto`? | **Decided (user 2026-08-08): No** in 0.1.x (H10). Default stays tables-off / `legacy_router=true`. Product path is `from_preset(Auto)`. | No semver flip. |
| `detect_tables_document` page_size? | **Decided (user 2026-08-08): Additive overload** `detect_tables_document_with_page_sizes`; old fn stays letter stand-in + changelog (P2.6 / A1.15). | Not a silent score change on old callers. |
| Delete classic `stream.rs` now? | **Decided (user 2026-08-08): Keep gated** (`allow_classic_stream`; Auto false). **Delete only after M4** + call-site zero. | P3.1 assert Auto never calls it; no delete PR in this rail. |
| R005 discipline n_exp=0 vs structure n_exp=1 (5x1)? | **Decided (user 2026-08-08): Open both golds + PDF in P0.3.** If 5x1 is not a real table, **fix structure gold** (possibly n_exp=0). Do not blindly copy structure into discipline. Likely page_filter vs first-page scope (`n_pred_all_pages=99`), not ACS miss. | Measurement before detector work. |
| Fast p95 30s vs ~409ms? | **Decided (rev 3 design):** two independent fail rules — live p95 > `budget_p95`; live p95 > prev_commit_p95×1.10 on same class. No `min()`. | P0.5 freeze file. |

---

## PR Plan

**Minimum mergeable increment:** P0.1 + **P0.6** + P1.3 + dump-compare green on g2 core. Ship without waiting for H-Flip.

False deps removed: P1.1 // P1.3 // P1.5 // P1.7 // P1.8 are parallel after P0.6. P2.1a / P2.2a may start after P0 (not after P1 exit). Phase 5 quality is split.

### Phase 0 — Honesty + executable contract

| PR | Title | Files | Deps | Changes |
|----|-------|-------|------|---------|
| **P0.1** | docs: single status plane | CHANGELOG, README, `docs/STATUS.md`, ARCHITECTURE stub, options-map (no Deref), V2 note Auto flipped, freeze README | — | G3/G4/G5 not claimed. ICDAR report peers=1. |
| **P0.2** | taxonomy from `per_table` shapes + census shadow metric | `structure_error_taxonomy.py` | P0.1 | Default = real_structure only. `--icdar-analysis` external; **not CI**. |
| **P0.3** | gold: R005 / R010 after opening PDF | gold + discipline manifest | P0.1 | Open both golds + PDF. If 5x1 is not a real table, fix **structure** gold (possibly n_exp=0). Do not copy structure into discipline blindly. |
| **P0.4** | G5.4: unit test SSOT; remove `detect_stream_tables` in page.rs grep | `check_phase_gates.py` | — | `allow_classic_stream` in page.rs OK. |
| **P0.5** | `latency_fast_v0.json` with hardware + budget math | freeze + nightly workflow **required** (not optional) | P0.1 | See Latency section. |
| **P0.6** | **executable contract** | see deliverables below | P0.1 | After this, reprint commands are copy-pasteable. |

P0.6 deliverables (complete list):

- `--binary PATH` on `run_detect_discipline.py`, `run_fp_strict.py`, `run_real_structure.py`, `run_latency_probe.py`
- `check_phase_gates.py --owned-only` (no ICDAR files)
- `benchmark/scripts/dump_product_tables.py` — `--freeze g2.json --structure-manifest real_structure_v0.json --out dump.json`; **one** CLI extract per PDF (`--format json --dump-evidence`); writes dump schema `pdfparser_table_dump_v1`
- `benchmark/scripts/compare_table_dumps.py --before --after`
- `freezes/owned_gates_v0.json`
- CI: hard `--phase 1/2` replaced by `--owned-only`; never load ICDAR metrics

Do **not** create `manifests/g2_core.json`.

**Phase 0 exit:** `--phase 0` green; `--owned-only` exists and is CI merge bar; dump-compare exists; taxonomy not all-unknown (local re-run); nightly scheduled.

### Phase 1 — Bit-identical SOLID (parallel after P0.6)

| PR | Title | Files | Deps | Changes |
|----|-------|-------|------|---------|
| **P1.1** | split ruled emit / densify rustdoc | ruled/*, densify rustdoc | P0.6 | Move-only; dump identical. |
| **P1.2** | borrowed `PageEvidence<'a>` + `PageEvidenceOwned` | evidence/*, lib.rs diagnostics | P0.6 | **Diagnostics wrapper only.** Product detect does not construct evidence. Clone only if dump requested. |
| **P1.3** | extract `merge_then_partition` (no sort); finalize uses it | engine_v2.rs, router/mod.rs | **P0.6 only** | Keep emit-walk order. `route_proposals` = merge_then_partition + sort (tests). |
| **P1.4** | contour_seed uniqueness bool; grep note reads | engine_v2.rs | P1.3 | Notes still written. |
| **P1.5** | options-map + no Deref test | options.rs | P0.1 | — |
| **P1.6** | document reserved structure / keep lattice.rs adapter | comments | P0.6 | Keep `lattice.rs` + `stats.rs`. |
| **P1.7** | optional CLI text IR via export | cli | P0.6 | Independent. |
| **P1.8** | `DensifyMode::{Primary,InsideFrameOnly,Off}` map from `lattice_text_densify` | options advanced, densify.rs | P0.6 | Default Primary; **no math**. |

**Phase 1 exit:** dump-compare identical on g2 core; owned-only green; Fast latency hold.

### Phase 2 — Parse / runtime (one concern per PR; may start after P0)

| PR | Title | Deps | Changes |
|----|-------|------|---------|
| **P2.1a** | typed `VmWarning` only | P0 | No skip-set change. |
| **P2.1b** | extract `WarningCode` map from VmWarning | P2.1a | Diagnostics. |
| **P2.2a** | ToUnicode shared document governor | P0 | Security. |
| **P2.2b** | q/Q nest cap | P2.1a | Fail-soft warn. |
| **P2.2c** | PageInfo inline Resources | P0 | Additive. |
| **P2.3** | stroke width rules **opt-in** | P2.1a | Default off; flip P2.3b later. |
| **P2.4a** | form BBox clip opt-in | P2.1a | Default off. |
| **P2.4b** | form font map opt-in | P2.1a | Separate from BBox. |
| **P2.4c** | clip applied to text (A2.7) **opt-in** | P2.1a | Default off; dump-compare; not bundled with P2.4a. |
| **P2.5a** | DecodeParms per-filter + LZW EarlyChange | P2.2a | Isolate. |
| **P2.5b** | JPX/CCITT/JBIG2 later opt-in | — | Not DCT. |
| **P2.6** | additive `detect_tables_document_with_page_sizes` | P0.6 dump | Old `detect_tables_document` stays letter + changelog. New fn used by façade only after dump no-drop. |
| **P2.7a** | Type0 missing ToUnicode warning + conf | P2.1b | — |
| **P2.7b** | emit `MissingToUnicode` | P2.7a | — |
| **P2.7c** | StandardEncoding high-byte table | P2.1b | Text compare. |
| **P2.7d** | bfrange array form | P2.1b | — |
| **P2.7e** | font load failure warning | P2.1b | Same glyphs. |
| **P2.8a** | rotate `TextRun.transform` | P0.6 | Bbox already rotated. |
| **P2.8b** | `extract_document` tables if detect_tables | P0 | Default still off. |

Default-flip PRs (`*b` after opt-in) only after dump no-drop.

### Phase 3 — Shadow + invocation-skip (still return soup)

| PR | Title | Deps | Changes |
|----|-------|------|---------|
| **P3.1** | assert Auto never calls StreamDetector | P1.5 | Same behavior. |
| **P3.2** | `LineProbe` + `PagePlan` (no dispatch change) | P1.2 | Cheap probe; dump flavor. Product may construct borrowed evidence here. |
| **P3.3** | `advanced.shadow_exclusive_first` build_plan path | P3.2 | Returns soup; logs table diff + invocation counts. **Default false all presets.** |
| **P3.4** | **invocation-skip** (Alt 4) behind dump-compare | P0.6 + P3.2 | Skip Network detect when ruled_owns && !recovery; skip Hybrid detect when probe says no exterior frame && ruled_owns. Freeze-gated Auto perf, not shadow-only. |

**Phase 3 exit:** shadow artifact; Auto dumps unchanged unless P3.4 dump-identical skip landed; Fast unchanged (shadow off).

### Phase 4 — Score honesty on soup (before H-Flip)

| PR | Title | Deps | Changes |
|----|-------|------|---------|
| **P4.1** | honest joints + whitespace | P1.3 | Freeze A/B on soup Auto. |
| **P4.2** | K26 page median; method_rank | P4.1 | Freeze A/B. |
| **P4.3** | nested completeness (keep 42) | P4.1 | — |
| **P4.4** | lexicon/tuning only | P4.1 | Optional. |
| **P4.5** | stitch skip telemetry | P1 | Eval stitch off. |

**Phase 4 exit:** owned-gates hold; scores not fabricated.

**H-Flip PR:** re-run shadow on post-P4 binary; require **shadow diff = 0 on g2 core ids** (count/method/bbox); set `advanced.exclusive_first_live=true` on Auto. Rollback `--legacy-router`.

### Phase 5 — Quality (split)

| PR | Title | Deps | Changes |
|----|-------|------|---------|
| **P5.1a** | census 34x10 glued/col assign | P4, P0.3 | Then e2e assert. |
| **P5.1b** | schools cell assign (shape exact) | P4 | Separate. |
| **P5.1c** | NIPA / R010 structure | P4, P0.3 | Separate. |
| **P5.2** | DensifyMode InsideFrameOnly default A/B | P1.8, 07/59 | Flip only. |
| **P5.3** | R008/R009/R016 region+IoU | P5.2 | Discipline hold. |
| **P5.4** | two partial tables | P5.3 | No union restore. |
| **P5.5** | JPEG painted grids via existing morph + HQ | A2.2a already | Fast no render. |

**Phase 5 exit:** core cell up vs g2; still **no GATE-4 claim** unless external ICDAR process (not CI) and STATUS honest.

---

## Feasibility

| Item | Estimate |
|------|----------|
| Phase 0 + P0.6 harness | **2–4 engineer-days** (docs + scripts + CI split) |
| Phase 1 mechanical (parallel) | **3–5 days** after dump-compare exists |
| Minimum increment (P0.1+P0.6+P1.3) | **~1 week** one person |
| Live `run_real_structure` n=23 | **minutes**; R005 alone **~50.5s** in latest JSON |
| Discipline + fp_strict | typically **< 1 min** each on release binary |
| Full live suite every PR in GitHub Actions | **heavy**; do **not** put full T3 live in PR CI |
| PR CI | unit + clippy + `assert_no_icdar` + `--owned-only` on **committed** JSON (after ICDAR split) |
| Detector PR merge | author runs live `--binary` locally; **nightly on main required** or H15 is fiction |
| H-Flip + Phase 5 | multi-week; not in minimum increment |

One person can execute Phase 0–1. Full exclusive-first + quality is a team quarter, not a weekend.

---

## Risks

| Risk | Sev | Mitigation |
|------|-----|------------|
| Status docs keep drifting after P0 | Med | `docs/STATUS.md` only place that may say PASS; CI comment links it |
| Borrowed PageEvidence breaks external 0.1 users | Low | Unstable 0.1; `PageEvidenceOwned` dump type |
| A1.15 page_size fix moves Auto | Med | Not mechanical; additive API |
| Exclusive-first drops network recovery pages | High | Residual after-form network once; shadow diff=0 on core; None+soup nonempty fails shadow |
| Phase 4 honest joints drops weak lattices | High | Freeze band on **soup**; unknown != auto-pass |
| H-Flip vs P4 order confusion | High | H19 single sequence; soup through P4 |
| `check_phase_gates --phase 1` ICDAR merge bar | High | P0.6 `--owned-only`; CI never ICDAR metrics |
| Encoding default flip mojibake | Med | Opt-in; phase_t_text + dumps |
| Latency freeze `min()` dead budget | Med | Two independent rules: budget ceiling + same-class +10% vs prev commit |
| Nightly optional undercuts H15 | High | Nightly **required** for detector merges |
| Contributors retune on ICDAR | High | H8 + `--owned-only` + assert_no_icdar PDFs |

---

## References

- `docs/design-table-engine-v2.md` — capability ladder, K1–K36, migration invariant scope
- `docs/design-table-engine-v3-industry.md` — exclusive page strategy target
- `docs/implementation-plan-v3-gated.md` — GATE-0..5 metric definitions (quality train)
- `docs/options-deprecation-map.md` — product field list (Deref claim stale)
- `docs/phase-structure-gates.md` — structure phase criteria
- `docs/README.md` — ASCII diagrams only
- `README.md` — customer honesty, ICDAR policy
- `CONTRIBUTING.md` — no ICDAR in CI/tuning
- `benchmark/scripts/assert_no_icdar.py`
- `benchmark/scripts/check_phase_gates.py` — add `--owned-only` in P0.6
- `benchmark/real_track/freezes/g2.json` — steady no-regress lock
- `benchmark/real_track/freezes/owned_gates_v0.json` — P0.6 owned floors (planned)
- `benchmark/real_track/freezes/g3_industry.json` — INVALID / revoked
- `benchmark/real_track/results/real_structure_latest.json`
- `benchmark/real_track/results/detect_discipline_latest.json`
- `benchmark/real_track/results/latency_probe_latest.json`
- `.github/workflows/ci.yml`

---

## Revision Summary

- Initial draft 2026-08-08: as-built inventory; ranked Phase 0–5; No-Regression Contract; PR slices. ASCII only.
- Rev 2 (review 29910549): executable contract + P0.6; `--owned-only`; H19; cheap probe; `merge_then_partition`; H5/H20/H21/H22; split P2; dump fields; owned floors; P1.8; min increment; A4.1 Fixed; A2.2a/b; Alt 4; feasibility.
- Rev 3: latency = two independent rules (budget ceiling + same-class +10% vs prev commit), no `min()`; dump = one CLI invoke + freeze/manifest resolve (no fictional `g2_core.json`) + `pdfparser_table_dump_v1` schema; P2.4c A2.7 text clip; `PagePlan` drops `residual_network_after_form` (build_plan post-condition); `borderless_prefilter_cheap` spelled from existing `stream_min_body_bands` + 12pt cluster gap.
- Rev 3.1 (user 2026-08-08): Open Questions **decided** — H16 hard-error encryption; H10 no Default==Auto; P2.6 additive `with_page_sizes`; classic `stream.rs` keep until M4; P0.3 R005 open both golds+PDF and fix structure gold if 5x1 is not real. No unresolved product calls.
