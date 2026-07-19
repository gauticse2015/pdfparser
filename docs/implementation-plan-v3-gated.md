# Implementation Plan V3 — Phase-Gated Development

| Field | Value |
|-------|-------|
| **Status** | Binding process plan |
| **Date** | 2026-07-15 |
| **Architecture** | `docs/design-table-engine-v3-industry.md` |
| **Rule** | **No phase N+1 work until phase N hard gates pass.** Soft improvements do not unlock the next phase. |

---

## 0. Executive rules

1. **One failure family per phase.** Do not mix over-detect fixes with densify/shape work in the same PR train.
2. **Hard gates are automated** (script exit code 0/1). Human review only for gold quality, not for “looks better.”
3. **ICDAR is never CI and never used for threshold tuning**, but **is allowed as an external process gate** for Phase 1 promotion (pred/GT ratio only — not per-file hacks).
4. **Primary regression lock:** real_structure freeze must not regress on metrics *outside* the phase’s allowed movement band.
5. **If measurement cannot see the bug, expand gold before coding.**

---

## 1. Benchmark assessment — is current real track good enough?

### 1.1 What we have

| Suite | n | What it measures | Current signal |
|-------|--:|------------------|----------------|
| **real_structure** (T3 gold) | **15** | det count/IoU + shape + cell F1 | det F1 **0.964**, cell **0.637**, shape exact **0.533** |
| **real_fp_smoke** | **6** | expect ~0 tables | **6/6 pass** but tols are loose (1–4); still emits stream FPs within tol |
| **real_detect_smoke** | ~20+ | soft count | Directional only; not a hard gate today |
| **Owned multi-lib** | 33–81 | synthetic/compete_hard | Near-perfect; **does not capture ICDAR over-detect** |
| **ICDAR external** | 67 | F1/TEDS/count | **pred 432 / gt 158**; OVER_DETECT 52/67 — true pain |

### 1.2 Freeze residual (real_structure Auto)

| Issue | Docs | Notes |
|-------|------|-------|
| **Over-count** | only **2/15** | `35_fuel` 2→1, `44_spanning` 3→2 |
| **Under-count** | **0/15** | |
| **Shape exact = 0** | **7/15** | topology/content wrong even when count OK |
| **cell F1 = 0** | **2/15** | `32_census`, `R010_nipa` |
| **cell F1 &lt; 0.3** | **3/15** | + `37_liabilities` |

### 1.3 Verdict: **NOT good enough to gate Phase 1 (over-detect) alone**

| Requirement for Phase 1 | Current coverage | Gap |
|-------------------------|------------------|-----|
| Many docs where Auto **over-emits** tables | Only 2 mild over-counts on structure core | **Critical gap** — suite already “green” on det |
| Prose/chrome **must emit 0** with tight tol | FP smoke passes with pred=1 and tol≥1 | **Too soft** — cannot prove stream kill |
| Multi-table exact counts | Few docs (36, 42, 32) | Thin |
| ICDAR-class stream explosion | **Absent from in-repo primary** (policy) | Need **external gate** + **in-repo FP expansion** |
| Wrong shape measurement | 15 docs OK-ish | Usable for Phase 2 **after** det fixed |
| Encoding failures | R010 only | Thin for Phase 4 |

**Conclusion:**  
- Do **not** start engine changes yet.  
- **Phase 0 = measurement + gold expansion** is mandatory.  
- real_structure remains the **no-regress lock** and later **shape/cell gates**.  
- Over-detect needs a **new Detect Discipline suite** + **stricter FP smoke** + **external ICDAR count ratio**.

### 1.4 What “good enough” measurement looks like before Phase 1 code

| Suite | Min size | Role |
|-------|---------:|------|
| **detect_discipline** (NEW) | ≥ **25** docs | Strict table **count** (tol=0) + method_mix logging |
| **fp_smoke_strict** (tighten existing) | ≥ **12** docs | expect 0 tables, **tol=0** (or 0 with documented waiver max 1 on ≤2 docs) |
| **real_structure** | keep 15; grow to **25** before Phase 2 | Shape/cell; no-regress always |
| **nested_holdout** | ≥ 1 (doc 42) | Always required |
| **ICDAR external count board** | 67 | pred/GT + OVER_DETECT rate — promotion only |

---

## 2. Phase map (strict order)

```text
Phase 0  Measurement foundation + gold expansion     ── HARD GATE 0
Phase 1  Over-detection / precision (count only)       ── HARD GATE 1
Phase 2  Detection completeness (under / multi-table) ── HARD GATE 2
Phase 3  Grid topology (rows/cols / shape)             ── HARD GATE 3
Phase 4  Cell content / structure quality              ── HARD GATE 4
Phase 5  Sensing ladder + encoding + API polish        ── HARD GATE 5
```

**Forbidden:** Starting Phase 3 densify/row work while Gate 1 is red.  
**Forbidden:** “Drive-by” shape fixes inside Phase 1 PRs.

---

## 3. Phase 0 — Measurement foundation & goldens

**Goal:** Make over-detect, under-detect, shape, and cell progress **observable and gateable**.  
**No product algorithm changes** except harness/scripts/manifests/gold.

### 3.1 Work items

| ID | Task | Output |
|----|------|--------|
| P0-1 | Define suite `detect_discipline_v1` manifest | `benchmark/real_track/manifests/detect_discipline_v1.json` |
| P0-2 | Runner `run_detect_discipline.py` | Metrics: n_pred, n_exp, exact, FP, FN, method_mix, over_rate |
| P0-3 | Tighten FP smoke → `real_fp_smoke_v1` | tol=0 for prose/chrome; separate form waiver list |
| P0-4 | Expand **count gold** (T0/T1) for discipline suite | ≥25 docs with human-confirmed `expected_table_count` |
| P0-5 | Expand **T3 structure gold** toward 25 | Priority struggle modes (see §3.3) |
| P0-6 | Tag every gold with `failure_families`: `over_detect`, `under_detect`, `wrong_shape`, `bad_cells`, `fp_prose`, `fp_form`, `nested` | Filterable gates |
| P0-7 | Baseline freeze `freezes/baseline_pre_v3.json` | Snapshot Auto metrics on all new suites |
| P0-8 | Gate script `check_phase_gates.py --phase N` | Exit 1 if any criterion fails |
| P0-9 | ICDAR external **count-only** report hook | `pred_gt_ratio`, `over_detect_doc_rate` (no tuning) |
| P0-10 | CI: run detect_discipline + fp_smoke_strict + real_structure (report); gates optional until Phase 1 | Artifacts on PR |

### 3.2 Detect discipline suite composition (target ≥25)

**Bucket A — Over-detect / FP (count should be low)** ≥10  

| Source | Examples | expected_count |
|--------|----------|----------------|
| Existing FP | 38 IRS, 39 Beige, 40 arxiv, 41 NIST notice, R001, R007 | **0** (strict) |
| Chrome multi-page narrative | R006 BeigeBook sample, more prose PDFs | 0 |
| Fuel-like mild over | 35_fuel | **1** (not 2) |
| Spanning over-fragment | 44_spanning | **2** (not 3) |
| New: multi-col prose / report | add 4–6 licensed PDFs | 0 or exact small |

**Bucket B — Multi-table exact count** ≥6  

| Source | expected |
|--------|----------|
| 36_two_tables | 2 |
| 42_insurance nested | 2 |
| 32_census | 2 |
| R018 health | 1 or exact |
| Promote R008/R009/R016 with count gold | exact |

**Bucket C — Single-table exact (sanity)** ≥8  

From existing structure core with n_exp=1 (should stay exact).

**Bucket D — Under-detect risk** ≥3  

Docs where lattice/stream might miss second table (stacked multi-table). May require new PDFs if inventory thin.

### 3.3 T3 structure expansion (before Phase 2/3) — priority list

Promote from `compete_real` + existing downloads (human T3, **not** pdfparser self-gold):

| Priority | Doc | Why |
|----------|-----|-----|
| 1 | R016_tabula_12s0324 | Classic lattice structure |
| 2 | R017_tabula_argentina | Votes/stream structure |
| 3 | R019_camelot_foo | Ruled baseline |
| 4 | R005_census_acs | Dense numeric (pair with 32) |
| 5 | R008_cdc_mmwr | Multi-table report |
| 6 | R009_sec_10q | Financial partial / multi |
| 7 | R020_irs_schedule_d | Form-adjacent table |
| 8–10 | 2–3 new US-report style licensed PDFs | ICDAR-US-like hardness without ICDAR files |

**Process for each T3 gold:** peer draft (Camelot/plumber) → human edit cells → `review_status=reviewed` → add SOURCES.md → coverage_matrix tag.

### 3.4 Phase 0 hard gate (GATE-0) — **must pass before any Phase 1 code**

| # | Criterion | Threshold |
|---|-----------|-----------|
| G0.1 | `detect_discipline_v1` docs | **≥ 25** with `expected_table_count` + human review |
| G0.2 | FP strict docs | **≥ 12** with expect 0 and **tol=0** (≤2 waived form docs documented) |
| G0.3 | Structure T3 docs | **≥ 20** reviewed (from 15) **or** written plan+calendar with **≥5 new T3 in progress** *and* discipline suite already ≥25 — **prefer full 20** |
| G0.4 | Every discipline doc tagged `failure_families` | 100% |
| G0.5 | Baseline freeze written | `baseline_pre_v3.json` exists |
| G0.6 | `check_phase_gates.py --phase 0` | exit 0 |
| G0.7 | Coverage matrix updated | over_detect, fp_prose, multi_table modes non-empty with structure or discipline refs |
| G0.8 | No ICDAR files in repo | `assert_no_icdar.py` green |

**GATE-0 pass definition:** all G0.* true.  
**On fail:** only gold/harness work continues.

### 3.5 Success criteria for “measurement quality”

After Phase 0, running **current Auto** baseline must show **visible red** on discipline suite for over-detect (otherwise gold is still too easy):

| Metric on baseline Auto | Expected before Phase 1 |
|-------------------------|-------------------------|
| detect_discipline exact-count rate | **&lt; 70%** (room to improve) |
| fp_strict zero-table rate | **&lt; 80%** (prose/chrome currently leak) |
| real_structure det F1 | ~0.96 (already high — lock no-regress) |

If baseline already ≥95% exact on discipline suite, **gold is too easy** — add harder FP/over-detect docs before coding.

---

## 4. Phase 1 — Over-detection only (detection precision)

**Goal:** Stop inventing extra tables. **Do not** change densify, cell assign, or encoding except as required to keep freeze cell F1 from collapsing &gt;ε.

### 4.1 Allowed code changes

- Ruled-owns-page (skip network/stream when strong ruled)  
- Stream/network confidence floors, prose reject, min textlines  
- Form discriminator **hard** reject  
- Per-page table budget  
- Disable classic stream product path  
- Whitespace reject for **empty** grids (count FP), not row-collapse logic  
- Router: drop weak borderless under ruled — **not** new densify  

### 4.2 Forbidden in Phase 1

- Text densify changes aimed at shape  
- Span merge / cell fill rewrites  
- Encoding / font work  
- Contour-primary rewrite (unless required solely for “is_strong_ruled” probe — prefer simple existing lattice strength)  
- New synthetic corpus generators  

### 4.3 Metrics (definitions)

```text
exact_count_rate = mean(1[n_pred == n_exp]) on detect_discipline
over_doc_rate    = mean(1[n_pred > n_exp]) on detect_discipline
fp_zero_rate     = mean(1[n_pred == 0]) on fp_strict (expect 0)
pred_gt_ratio    = sum(n_pred)/sum(n_exp) on ICDAR external (process gate)
over_detect_icdar_docs = fraction of ICDAR docs with n_pred > n_gt
micro_det_f1_structure = real_structure micro det count F1
micro_cell_f1_structure = real_structure micro cell F1  # no-regress band
```

### 4.4 Phase 1 hard gate (GATE-1)

| # | Criterion | Threshold | Suite |
|---|-----------|-----------|-------|
| G1.1 | exact_count_rate | **≥ 0.88** | detect_discipline |
| G1.2 | over_doc_rate | **≤ 0.12** | detect_discipline |
| G1.3 | sum(n_pred)/sum(n_exp) | **≤ 1.15** | detect_discipline |
| G1.4 | fp_zero_rate | **≥ 0.85** | fp_strict |
| G1.5 | No discipline doc with n_pred ≥ n_exp + 3 | **0 such docs** | detect_discipline |
| G1.6 | real_structure micro det F1 | **≥ freeze − 0.02** | real_structure |
| G1.7 | real_structure micro cell F1 | **≥ freeze − 0.03** | no-regress band |
| G1.8 | Nested doc 42 | still **2** tables, cell F1 ≥ freeze_42 − 0.05 | holdout |
| G1.9 | Owned hard suite overall | **still green** (or documented intentional) | accuracy hard |
| G1.10 | ICDAR external pred/GT | **≤ 1.50** (from ~2.73) | external only |
| G1.11 | ICDAR OVER_DETECT doc rate | **≤ 0.35** (from ~0.78) | external only |
| G1.12 | `check_phase_gates.py --phase 1` | exit 0 | |

**Promotion rule:** G1.1–G1.12 all pass on same binary SHA.  
**If G1.10–G1.11 fail but in-repo gates pass:** Phase 1 **not done** — exclusivity not strong enough for wild pages; continue Phase 1 only.

### 4.5 Phase 1 exit freeze

Write `freezes/phase1_detect_precision.json` with suite digests + git SHA.

---

## 5. Phase 2 — Detection completeness (under-detect / multi-table)

**Goal:** Recover missing tables **without** re-introducing over-detect.

Only starts after GATE-1 green.

### 5.1 Allowed work

- Multi-region lattice merge (careful)  
- Nested keep gates refinement  
- Borderless **second region** residual (area engine) only if Phase 1 gates stay green  
- Multi-table page association  

### 5.2 Forbidden

- Lowering FP floors “to find more tables” without dual metrics  
- Re-enabling classic stream globally  

### 5.3 Phase 2 hard gate (GATE-2)

| # | Criterion | Threshold |
|---|-----------|-----------|
| G2.1 | All GATE-1 metrics still pass | hard |
| G2.2 | under_doc_rate on discipline | **≤ 0.08** |
| G2.3 | multi_table subset exact_count_rate | **≥ 0.85** |
| G2.4 | Nested 42 | 2 tables hold |
| G2.5 | real_structure det F1 | ≥ phase1 freeze − 0.01 |
| G2.6 | ICDAR pred/GT | still **≤ 1.50** (no regression to soup) |
| G2.7 | ICDAR under-detect docs | no worse than baseline + 2 docs |

---

## 6. Phase 3 — Grid topology (wrong shape / row / col)

**Goal:** Correct **rows×cols** on matched tables. Cell text quality secondary (shape exact + row/col accuracy).

### 6.1 Prerequisites

- GATE-2 green  
- Structure T3 **≥ 20** docs (if not met in Phase 0, finish gold here **before** densify PRs)

### 6.2 Allowed work

- Joint-only anchors; densify **inside frame only**  
- Decorative H/V suppress  
- Contour-primary ruled regions  
- Empty column collapse (shape-affecting)  
- Span topology (blank covered cells)  

### 6.3 Forbidden

- Broad confidence drops that re-open over-detect  
- Encoding-only fixes (Phase 4/5)  

### 6.4 Metrics

```text
shape_exact_rate = real_structure micro shape exact
row_acc, col_acc = from structure metrics (wire if not micro-aggregated)
matched_shape_rate on discipline docs that have T3 gold
```

### 6.5 Phase 3 hard gate (GATE-3)

| # | Criterion | Threshold |
|---|-----------|-----------|
| G3.1 | GATE-1 and GATE-2 still pass | hard |
| G3.2 | real_structure shape_exact_rate | **≥ 0.70** (from ~0.53) |
| G3.3 | Docs with shape_exact=0 | **≤ 3 / n** (from 7/15) |
| G3.4 | Per matched table row_accuracy micro | **≥ 0.75** |
| G3.5 | Per matched table col_accuracy micro | **≥ 0.80** |
| G3.6 | cell F1 | ≥ phase2 freeze − 0.02 (no big cell regress) |
| G3.7 | ICDAR row metric | **≥ 0.50** (from ~0.33) external honesty |
| G3.8 | ICDAR col metric | **≥ 0.55** (from ~0.45) external honesty |

---

## 7. Phase 4 — Cell content / structure quality

**Goal:** Cell F1 / content alignment (TEDS-like quality).

### 7.1 Allowed work

- Cell assignment, multi-run merge, glued numeric  
- Span text flow  
- Encoding / ToUnicode / Differences (R010)  
- Dense numeric (census 32)  

### 7.2 Phase 4 hard gate (GATE-4)

| # | Criterion | Threshold |
|---|-----------|-----------|
| G4.1 | GATE-1–3 still pass | hard |
| G4.2 | real_structure micro cell F1 | **≥ 0.78** (from ~0.64) |
| G4.3 | Docs with cell F1 = 0 | **0** |
| G4.4 | Docs with cell F1 &lt; 0.3 | **≤ 1** |
| G4.5 | R010 and 32 cell F1 | each **≥ 0.40** |
| G4.6 | ICDAR TEDS external | **≥ 0.50** (from ~0.32) |
| G4.7 | ICDAR F1 external | **≥ 0.65** (from ~0.50) |

---

## 8. Phase 5 — Sensing ladder, latency, API, industry polish

**Goal:** HQ render path, knob diet, CI freezes, honest README.

### 8.1 Phase 5 hard gate (GATE-5)

| # | Criterion | Threshold |
|---|-----------|-----------|
| G5.1 | GATE-1–4 hold on Auto | hard |
| G5.2 | HQ vs Auto A/B on raster_needed subset | HQ ≥ Auto on that subset |
| G5.3 | Public TableOptions fields | **≤ 12** |
| G5.4 | Classic stream | not in product Auto |
| G5.5 | CI | detect_discipline + fp_strict + real_structure freeze job |
| G5.6 | Latency Fast preset | no full-page render; p95 within budget on latency_probe |
| G5.7 | Structure T3 | **≥ 25** docs |
| G5.8 | README maturity labels | match capability ladder (no false Production) |

---

## 9. Gate automation

### 9.1 Script contract

```bash
# After each phase, same command:
python3 benchmark/scripts/check_phase_gates.py --phase 0
python3 benchmark/scripts/check_phase_gates.py --phase 1 --binary target/release/pdfparser
python3 benchmark/scripts/check_phase_gates.py --phase 1 --with-icdar /path/to/icdar2013-dataset
```

Exit codes:

| Code | Meaning |
|-----:|---------|
| 0 | All hard criteria green |
| 1 | One or more gates failed (print table) |
| 2 | Missing suite / gold / binary |

### 9.2 Freeze files

| File | When |
|------|------|
| `freezes/baseline_pre_v3.json` | End Phase 0 |
| `freezes/phase1_detect_precision.json` | End Phase 1 |
| `freezes/phase2_detect_recall.json` | End Phase 2 |
| `freezes/phase3_topology.json` | End Phase 3 |
| `freezes/phase4_cells.json` | End Phase 4 |
| `freezes/g3_industry.json` | End Phase 5 (new steady) |

### 9.3 PR policy

```text
label: phase-0 | phase-1 | ... | phase-5
CI must run check_phase_gates.py --phase <current>
Merging phase-N+1 code requires phase-N freeze file present on main
```

---

## 10. Gold creation playbook (Phase 0 execution)

### 10.1 Count gold (T0) — fast path for discipline suite

1. Open PDF, list tables per page (human).  
2. Set `expected_table_count`, `tol=0`.  
3. Tag `failure_families`.  
4. No cells required.  

### 10.2 Structure gold (T3) — slower path

1. Peer extract draft (Camelot lattice + stream, pdfplumber).  
2. Human corrects grid.  
3. Validate with `validate_structure_gold.py`.  
4. `review_status=reviewed`.  
5. Never accept raw pdfparser output as gold.

### 10.3 Minimum Phase 0 delivery checklist

- [ ] 25+ discipline docs  
- [ ] 12+ fp_strict docs  
- [ ] 5+ new T3 golds (20 total structure)  
- [ ] baseline freeze  
- [ ] gate script  
- [ ] baseline shows red on over-detect metrics (discipline exact &lt; 0.70)

---

## 11. Risk: “real det is already 0.96 — Phase 1 looks done”

**Mitigation baked into this plan:**

1. Phase 1 is gated on **detect_discipline + fp_strict + ICDAR ratio**, not on real_structure det alone.  
2. real_structure det is only a **no-regress** constraint in Phase 1.  
3. Phase 0 must create a suite where **current Auto fails** on over-detect.

---

## 12. Timeline (indicative, 1 engineer)

| Phase | Duration | Depends on |
|-------|----------|------------|
| **0 Measurement + gold** | **1.5–3 weeks** | Humans for T3 |
| **1 Over-detect** | 1–2 weeks | GATE-0 |
| **2 Completeness** | 1 week | GATE-1 |
| **3 Topology** | 2–3 weeks | GATE-2 + T3≥20 |
| **4 Cells + encoding** | 2–3 weeks | GATE-3 |
| **5 Polish** | 1–2 weeks | GATE-4 |

**Critical path:** Phase 0 gold quality. Do not compress Phase 0.

---

## 13. Immediate next actions (ordered)

1. **Approve this gate plan** (thresholds).  
2. Implement `detect_discipline` harness + `check_phase_gates.py` (no engine changes).  
3. Author count golds for FP strict (tighten tols on 38–41, R001, R007 first).  
4. Add 10+ over-detect/prose count golds from compete_real + new licensed PDFs.  
5. Add ≥5 T3 structure golds.  
6. Write `baseline_pre_v3.json`.  
7. Run GATE-0.  
8. Only then: Phase 1 orchestrator exclusivity PR.

---

## 14. Summary table — phase → problem → gate headline

| Phase | Problem family | Headline gate |
|------:|----------------|---------------|
| 0 | Cannot measure | ≥25 discipline docs; baseline red on over-detect |
| 1 | Over-detect | exact≥0.88; ICDAR pred/GT≤1.50 |
| 2 | Under / multi-table | under≤0.08; multi exact≥0.85; keep Phase 1 |
| 3 | Wrong shape r/c | shape_exact≥0.70; keep Phase 1–2 |
| 4 | Bad cells / TEDS | cell F1≥0.78; zero cell-F1=0 docs |
| 5 | Industry polish | API≤12 knobs; CI freezes; T3≥25 |

---

*This plan intentionally blocks “interesting” topology work until over-detect is measurably fixed on a suite hard enough to see it.*
