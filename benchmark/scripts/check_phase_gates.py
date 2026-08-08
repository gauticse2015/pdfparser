#!/usr/bin/env python3
"""Hard phase gates for V3 gated development.

Merge bar (H8 / P0.6): ``--owned-only`` — discipline + fp_strict + g2 core +
nested doc 42. **Never loads ICDAR files.**

  python3 benchmark/scripts/check_phase_gates.py --phase 0
  python3 benchmark/scripts/check_phase_gates.py --owned-only

``--phase 1`` / ``--phase 2`` still exist for external ICDAR promotion checks
but are **not** a CI merge bar (H8).
"""
from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

BENCH = Path(__file__).resolve().parents[1]
RT = BENCH / "real_track"
REPO = BENCH.parent


def load(p: Path):
    if not p.is_file():
        return None
    return json.loads(p.read_text(encoding="utf-8"))


def ok(name, cond, detail=""):
    status = "PASS" if cond else "FAIL"
    print(f"  [{status}] {name}" + (f" — {detail}" if detail else ""))
    return bool(cond)


def gate0() -> bool:
    print("=== GATE-0 Measurement foundation ===")
    disc_m = load(RT / "manifests" / "detect_discipline_v1.json")
    fp_m = load(RT / "manifests" / "real_fp_smoke_v1_strict.json")
    structure_golds = list((RT / "gold").glob("*.json"))
    # exclude count/ subdir files already only in gold/*.json at top
    structure_golds = [p for p in structure_golds if p.parent.name == "gold"]
    count_golds = list((RT / "gold" / "count").glob("*.json"))
    results = []
    results.append(ok("G0.1 detect_discipline docs ≥25", disc_m is not None and disc_m.get("n_docs", 0) >= 25, f"n={disc_m.get('n_docs') if disc_m else None}"))
    results.append(ok("G0.2 fp_strict docs ≥12", fp_m is not None and fp_m.get("n_docs", 0) >= 12, f"n={fp_m.get('n_docs') if fp_m else None}"))
    results.append(ok("G0.3 structure T3 ≥15 (prefer 20)", len(structure_golds) >= 15, f"n={len(structure_golds)}"))
    results.append(ok("G0.3b count golds ≥25", len(count_golds) >= 25, f"n={len(count_golds)}"))
    # tags
    tagged = True
    if disc_m:
        for d in disc_m.get("documents") or []:
            if not (d.get("failure_families") or d.get("failure_classes")):
                tagged = False
    results.append(ok("G0.4 failure_families on discipline docs", tagged))
    results.append(ok("G0.5 runners exist", (BENCH / "scripts" / "run_detect_discipline.py").is_file() and (BENCH / "scripts" / "run_fp_strict.py").is_file()))
    # baseline should exist OR we allow generating
    base = load(RT / "freezes" / "baseline_pre_v3.json")
    results.append(ok("G0.6 baseline_pre_v3.json exists", base is not None, "run baseline first if missing"))
    if base:
        s = (base.get("detect_discipline") or {}).get("summary") or {}
        exact = s.get("exact_count_rate")
        # suite hard enough if exact < 0.70 OR over_doc_rate > 0.2
        hard = exact is not None and (exact < 0.70 or (s.get("over_doc_rate") or 0) > 0.20)
        results.append(ok("G0.7 baseline shows red on over-detect (exact<0.70 or over>0.20)", hard, f"exact={exact} over={s.get('over_doc_rate')}"))
    import subprocess

    icdar_rc = subprocess.call(
        ["python3", str(BENCH / "scripts" / "assert_no_icdar.py")],
        cwd=str(REPO),
    )
    results.append(ok("G0.8 no ICDAR in repo", icdar_rc == 0, f"assert_no_icdar exit={icdar_rc}"))
    # P0.5: freeze file + hardware + budget math (not a live probe; not GATE-5).
    scripts_dir = Path(__file__).resolve().parent
    if str(scripts_dir) not in sys.path:
        sys.path.insert(0, str(scripts_dir))
    import check_latency_freeze as clf

    lat_f = load(RT / "freezes" / "latency_fast_v0.json")
    results.append(ok("G0.9 latency_fast_v0.json exists", lat_f is not None))
    if lat_f:
        schema_errs = clf.validate_freeze_schema(lat_f)
        results.append(
            ok(
                "G0.9 latency freeze schema + budget=max(p95*1.5,max*1.2)",
                not schema_errs,
                "; ".join(schema_errs) if schema_errs else "ok",
            )
        )
        hw = lat_f.get("hardware") or {}
        results.append(
            ok(
                "G0.9 hardware runner_os + note recorded",
                bool(hw.get("runner_os")) and bool(hw.get("note")),
                f"os={hw.get('runner_os')}",
            )
        )
        nightly = REPO / ".github" / "workflows" / "nightly-latency-fast.yml"
        nt = nightly.read_text(encoding="utf-8") if nightly.is_file() else ""
        # Required: job must not set continue-on-error: (comments may mention the words).
        has_continue_on_error_key = any(
            line.split("#", 1)[0].strip().startswith("continue-on-error:")
            for line in nt.splitlines()
        )
        has_actions_write = any(
            line.split("#", 1)[0].strip() == "actions: write" for line in nt.splitlines()
        )
        results.append(
            ok(
                "G0.9 required nightly Fast latency workflow",
                nightly.is_file()
                and not has_continue_on_error_key
                and "check_latency_freeze.py" in nt
                and "run_latency_probe.py" in nt
                and "schedule:" in nt,
                str(nightly.name if nightly.is_file() else "missing"),
            )
        )
        results.append(
            ok(
                "G0.9 nightly permissions include actions: write (cache)",
                has_actions_write,
                "actions/cache + rust-cache need actions: write; contents stays read",
            )
        )
    wf = REPO / ".github" / "workflows"
    icdar_in_nightly = False
    if wf.is_dir():
        for p in wf.glob("*.yml"):
            txt = p.read_text(encoding="utf-8").lower()
            if "nightly" in p.name.lower() and (
                "run_icdar_competitive" in txt or "icdar2013" in txt
            ):
                icdar_in_nightly = True
    results.append(ok("G0.9 nightly does not run ICDAR", not icdar_in_nightly))
    return all(results)


def gate1(with_icdar: Path | None) -> bool:
    print("=== GATE-1 Over-detect / precision ===")
    disc = load(RT / "results" / "detect_discipline_latest.json")
    fp = load(RT / "results" / "real_fp_strict_latest.json")
    struct = load(RT / "results" / "real_structure_latest.json")
    freeze = load(RT / "freezes" / "g2.json") or load(RT / "freezes" / "baseline_pre_v3.json")
    results = []
    if not disc:
        print("  [FAIL] missing detect_discipline_latest.json — run suite")
        return False
    s = disc["summary"]
    results.append(ok("G1.1 exact_count_rate ≥ 0.88", s.get("exact_count_rate", 0) >= 0.88, f"{s.get('exact_count_rate')}"))
    results.append(ok("G1.2 over_doc_rate ≤ 0.12", s.get("over_doc_rate", 1) <= 0.12, f"{s.get('over_doc_rate')}"))
    ratio = s.get("pred_gt_ratio")
    results.append(ok("G1.3 pred/gt ≤ 1.15", ratio is not None and ratio <= 1.15, f"{ratio}"))
    if not fp:
        results.append(ok("G1.4 fp_strict present", False))
    else:
        results.append(ok("G1.4 fp_zero_rate ≥ 0.85", fp["summary"].get("fp_zero_rate", 0) >= 0.85, f"{fp['summary'].get('fp_zero_rate')}"))
    results.append(ok("G1.5 no severe over (delta≥3)", s.get("n_severe_over", 99) == 0, f"n_severe={s.get('n_severe_over')}"))
    # structure no-regress — score against freeze CORE doc ids only so expanded
    # T3 golds (Phase 2/3 tracking) do not dilute Phase-1 freeze comparison.
    if struct and freeze:
        fr = (freeze.get("summaries") or {}).get("auto") or {}
        fr_cell = fr.get("micro_cell_f1")
        fr_det = fr.get("micro_det_count_f1")
        core_ids = {d.get("id") for d in (freeze.get("documents_auto") or []) if d.get("id")}
        core = None
        if "runs" in struct and core_ids:
            docs = struct["runs"][0].get("documents") or []
            core_docs = [d for d in docs if d.get("id") in core_ids]
            if core_docs:
                # recompute micro means from core docs
                cells, dets, det_ious = [], [], []
                for d in core_docs:
                    m = d.get("metrics") or {}
                    if (m.get("cell") or {}).get("f1") is not None:
                        cells.append(m["cell"]["f1"])
                    dc = m.get("detection_count") or {}
                    if dc.get("f1") is not None:
                        dets.append(dc["f1"])
                    di = m.get("detection_iou") or {}
                    if di.get("f1") is not None:
                        det_ious.append(di["f1"])
                core = {
                    "n": len(core_docs),
                    "micro_cell_f1": sum(cells) / len(cells) if cells else None,
                    "micro_det_count_f1": sum(dets) / len(dets) if dets else None,
                    "micro_det_iou_f1": sum(det_ious) / len(det_ious) if det_ious else None,
                }
        if core is None and "runs" in struct:
            sm = struct["runs"][0].get("summary") or {}
            core = {
                "n": sm.get("n_docs"),
                "micro_cell_f1": sm.get("micro_cell_f1"),
                "micro_det_count_f1": sm.get("micro_det_count_f1"),
                "micro_det_iou_f1": sm.get("micro_det_iou_f1"),
            }
        if core:
            cell, det, det_iou = core.get("micro_cell_f1"), core.get("micro_det_count_f1"), core.get("micro_det_iou_f1")
            if cell is not None and fr_cell is not None:
                results.append(ok(
                    "G1.7 cell F1 no-regress core (freeze-0.03)",
                    cell >= fr_cell - 0.03,
                    f"{cell:.4f} vs freeze {fr_cell:.4f} (n_core={core.get('n')})",
                ))
            if det is not None:
                results.append(ok(
                    "G1.6 det count F1 core ≥ 0.88",
                    det >= 0.88,
                    f"{det:.4f} (freeze was {fr_det})",
                ))
            if det_iou is not None:
                results.append(ok("G1.6b det IoU F1 core ≥ 0.90", det_iou >= 0.90, f"{det_iou:.4f}"))
    # nested 42
    if struct and "runs" in struct:
        for d in struct["runs"][0].get("documents") or []:
            if d.get("id") == "42_real_insurance_italian":
                m = d.get("metrics") or {}
                # nested: n_pred/n_exp from detection
                dc = m.get("detection_count") or {}
                n_pred = dc.get("predicted", m.get("n_pred"))
                n_exp = dc.get("expected", m.get("n_exp"))
                results.append(ok("G1.8 nested 42 still 2 tables", n_pred == 2 and n_exp == 2, f"pred={n_pred} exp={n_exp}"))
                break
    # ICDAR hard-required (plan G1.10–G1.11). Never skip for phase ≥1.
    ic = load(BENCH / "results" / "icdar_failure_analysis.json")
    head = load(BENCH / "results" / "camelot_icdar_headtohead.json")
    if not ic or not (ic.get("per_doc") or []):
        results.append(ok("G1.10 ICDAR failure analysis present", False, "run run_icdar_competitive.py"))
        results.append(ok("G1.11 ICDAR over_detect metric present", False, "run run_icdar_competitive.py"))
    else:
        per = ic.get("per_doc") or []
        sp = sum(d["n_us"] for d in per)
        se = sum(d["n_gt"] for d in per)
        ratio_i = sp / se if se else 999
        over_rate = sum(1 for d in per if d["n_us"] > d["n_gt"]) / max(len(per), 1)
        n_docs = len(per)
        results.append(ok("G1.10 ICDAR pred/GT ≤ 1.50", ratio_i <= 1.50, f"{ratio_i:.3f} n={n_docs}"))
        results.append(ok("G1.11 ICDAR over_detect_doc_rate ≤ 0.35", over_rate <= 0.35, f"{over_rate:.3f}"))
        # Integrity: refuse restored/empty peer boards as "pass"
        us = ((head or {}).get("results") or {}).get("pdfparser") or {}
        if head and head.get("note") and "restored" in str(head.get("note")).lower():
            results.append(ok("G1.10b ICDAR board is live (not restored stub)", False, head.get("note")))
        if n_docs < 50:
            results.append(ok("G1.10c ICDAR n_docs ≥ 50", False, f"n={n_docs}"))
    return all(results)



def gate2(with_icdar: Path | None) -> bool:
    """Phase-2 completeness: keep Phase-1 precision + reduce under-detect."""
    print("=== GATE-2 Detection completeness ===")
    # Phase-1 must still pass
    if not gate1(with_icdar):
        print("  [FAIL] GATE-1 not green — cannot claim Phase-2")
        return False
    disc = load(RT / "results" / "detect_discipline_latest.json")
    results = []
    if not disc:
        return False
    s = disc["summary"]
    results.append(ok("G2.2 under_doc_rate ≤ 0.08", s.get("under_doc_rate", 1) <= 0.08, f"{s.get('under_doc_rate')}"))
    # multi_table exact among multi bucket
    multi = [d for d in disc.get("documents") or [] if d.get("bucket") == "multi" or (d.get("n_exp") or 0) >= 2]
    if multi:
        exact_m = sum(1 for d in multi if d.get("exact")) / len(multi)
        results.append(ok("G2.3 multi_table exact ≥ 0.85", exact_m >= 0.85, f"{exact_m:.3f} n={len(multi)}"))
    else:
        results.append(ok("G2.3 multi_table subset present", False))
    # expanded structure docs
    golds = list((RT / "gold").glob("*.json"))
    results.append(ok("G2.structure T3 golds ≥ 20", len(golds) >= 20, f"n={len(golds)}"))
    ic = load(BENCH / "results" / "icdar_failure_analysis.json")
    if not ic or not (ic.get("per_doc") or []):
        results.append(ok("G2.7 ICDAR under_doc metric present", False, "run ICDAR first"))
    else:
        per = ic.get("per_doc") or []
        under_rate = sum(1 for d in per if d["n_us"] < d["n_gt"]) / max(len(per), 1)
        results.append(ok("G2.7 ICDAR under_doc_rate ≤ 0.28", under_rate <= 0.28, f"{under_rate:.3f}"))
        # G2.6 pred/gt still ≤ 1.50 (same as G1.10)
        sp = sum(d["n_us"] for d in per)
        se = sum(d["n_gt"] for d in per)
        ratio_i = sp / se if se else 999
        results.append(ok("G2.6 ICDAR pred/GT ≤ 1.50", ratio_i <= 1.50, f"{ratio_i:.3f}"))
    return all(results)


def _core_structure_stats(struct, freeze):
    """Micro means on freeze core doc ids (Phase-1 no-regress set)."""
    if not struct or not freeze:
        return None
    core_ids = {d.get("id") for d in (freeze.get("documents_auto") or []) if d.get("id")}
    docs = []
    if "runs" in struct:
        docs = struct["runs"][0].get("documents") or []
    if core_ids:
        docs = [d for d in docs if d.get("id") in core_ids]
    if not docs:
        return None
    cells, dets, det_ious, shapes, rows, cols = [], [], [], [], [], []
    shape_zero = 0
    cell_zero = 0
    cell_lt_03 = 0
    for d in docs:
        m = d.get("metrics") or {}
        if (m.get("cell") or {}).get("f1") is not None:
            cf = m["cell"]["f1"]
            cells.append(cf)
            if cf == 0.0:
                cell_zero += 1
            if cf < 0.3:
                cell_lt_03 += 1
        dc = m.get("detection_count") or {}
        if dc.get("f1") is not None:
            dets.append(dc["f1"])
        di = m.get("detection_iou") or {}
        if isinstance(di, dict) and di.get("f1") is not None:
            det_ious.append(di["f1"])
        st = m.get("structure") or {}
        if st.get("shape_exact_rate") is not None:
            se = st["shape_exact_rate"]
            shapes.append(se)
            if se == 0.0:
                shape_zero += 1
        if st.get("row_accuracy") is not None:
            rows.append(st["row_accuracy"])
        if st.get("col_accuracy") is not None:
            cols.append(st["col_accuracy"])
        # per-table fallbacks
        for pt in m.get("per_table") or []:
            if pt.get("row_exact") is not None and st.get("row_accuracy") is None:
                pass
    def mean(xs):
        return sum(xs) / len(xs) if xs else None
    return {
        "n": len(docs),
        "micro_cell_f1": mean(cells),
        "micro_det_count_f1": mean(dets),
        "micro_det_iou_f1": mean(det_ious),
        "micro_shape_exact": mean(shapes),
        "micro_row_acc": mean(rows),
        "micro_col_acc": mean(cols),
        "n_shape_zero": shape_zero,
        "n_cell_zero": cell_zero,
        "n_cell_lt_03": cell_lt_03,
        "docs": docs,
    }


def gate3(with_icdar: Path | None) -> bool:
    """Phase-3 grid topology: shape exact + row/col accuracy; hold Phase 1–2."""
    print("=== GATE-3 Grid topology (shape) ===")
    if not gate2(with_icdar):
        print("  [FAIL] GATE-2 not green — cannot claim Phase-3")
        return False
    struct = load(RT / "results" / "real_structure_latest.json")
    freeze = load(RT / "freezes" / "g2.json") or load(RT / "freezes" / "baseline_pre_v3.json")
    core = _core_structure_stats(struct, freeze)
    results = []
    if not core:
        results.append(ok("G3 core structure stats", False, "missing structure or freeze"))
        return False
    # Prefer full T3 micro shape if ≥ core; Phase-3 plan targets structure suite.
    full_shape = None
    full_row = full_col = None
    if struct and "runs" in struct:
        sm = struct["runs"][0].get("summary") or struct.get("summary") or {}
        full_shape = sm.get("micro_shape_exact_rate")
        # recompute row/col from all docs
        docs = struct["runs"][0].get("documents") or []
        rows = [(d.get("metrics") or {}).get("structure", {}).get("row_accuracy") for d in docs]
        cols = [(d.get("metrics") or {}).get("structure", {}).get("col_accuracy") for d in docs]
        rows = [x for x in rows if x is not None]
        cols = [x for x in cols if x is not None]
        full_row = sum(rows) / len(rows) if rows else None
        full_col = sum(cols) / len(cols) if cols else None

    shape = core.get("micro_shape_exact")
    # Gate on core freeze (honest) AND report full T3
    results.append(ok(
        "G3.2 core shape_exact ≥ 0.70",
        shape is not None and shape >= 0.70,
        f"core={shape} full={full_shape} n_core={core['n']}",
    ))
    n_zero = core.get("n_shape_zero") or 0
    n_core = core.get("n") or 1
    results.append(ok(
        "G3.3 core docs shape_exact=0 ≤ 3",
        n_zero <= 3,
        f"n_zero={n_zero}/{n_core}",
    ))
    row_acc = core.get("micro_row_acc")
    col_acc = core.get("micro_col_acc")
    # If structure summary lacks row/col micro, derive from per_table
    if row_acc is None or col_acc is None:
        r_vals, c_vals = [], []
        for d in core.get("docs") or []:
            for pt in (d.get("metrics") or {}).get("per_table") or []:
                if pt.get("row_exact") is not None:
                    r_vals.append(1.0 if pt["row_exact"] else 0.0)
                if pt.get("col_exact") is not None:
                    c_vals.append(1.0 if pt["col_exact"] else 0.0)
            st = (d.get("metrics") or {}).get("structure") or {}
            if st.get("row_accuracy") is not None:
                r_vals.append(st["row_accuracy"])
            if st.get("col_accuracy") is not None:
                c_vals.append(st["col_accuracy"])
        if r_vals and row_acc is None:
            row_acc = sum(r_vals) / len(r_vals)
        if c_vals and col_acc is None:
            col_acc = sum(c_vals) / len(c_vals)
    results.append(ok(
        "G3.4 core row_accuracy ≥ 0.75",
        row_acc is not None and row_acc >= 0.75,
        f"{row_acc} (full={full_row})",
    ))
    results.append(ok(
        "G3.5 core col_accuracy ≥ 0.80",
        col_acc is not None and col_acc >= 0.80,
        f"{col_acc} (full={full_col})",
    ))
    cell = core.get("micro_cell_f1")
    fr = (freeze.get("summaries") or {}).get("auto") or {}
    fr_cell = fr.get("micro_cell_f1")
    if cell is not None and fr_cell is not None:
        results.append(ok(
            "G3.6 core cell F1 no-regress (freeze-0.02)",
            cell >= fr_cell - 0.02,
            f"{cell:.4f} vs freeze {fr_cell:.4f}",
        ))
    head = load(BENCH / "results" / "camelot_icdar_headtohead.json")
    us = ((head or {}).get("results") or {}).get("pdfparser") or {}
    if not us:
        results.append(ok("G3.7 ICDAR headtohead present", False, "run run_icdar_competitive.py"))
        results.append(ok("G3.8 ICDAR headtohead present", False, "run run_icdar_competitive.py"))
    else:
        if head and head.get("note") and "restored" in str(head.get("note")).lower():
            results.append(ok("G3.7b ICDAR board live (not restored)", False, head.get("note")))
        results.append(ok("G3.7 ICDAR row ≥ 0.50", (us.get("row") or 0) >= 0.50, f"{us.get('row')}"))
        results.append(ok("G3.8 ICDAR col ≥ 0.55", (us.get("col") or 0) >= 0.55, f"{us.get('col')}"))
    return all(results)


def gate4(with_icdar: Path | None) -> bool:
    """Phase-4 cell content quality; hold Phase 1–3."""
    print("=== GATE-4 Cell content ===")
    if not gate3(with_icdar):
        print("  [FAIL] GATE-3 not green — cannot claim Phase-4")
        return False
    struct = load(RT / "results" / "real_structure_latest.json")
    freeze = load(RT / "freezes" / "g2.json")
    core = _core_structure_stats(struct, freeze)
    results = []
    if not core:
        return False
    cell = core.get("micro_cell_f1")
    results.append(ok("G4.2 core cell F1 ≥ 0.78", cell is not None and cell >= 0.78, f"{cell}"))
    results.append(ok("G4.3 core docs cell F1=0 is 0", (core.get("n_cell_zero") or 0) == 0, f"n_zero={core.get('n_cell_zero')}"))
    results.append(ok("G4.4 core docs cell F1<0.3 ≤ 1", (core.get("n_cell_lt_03") or 0) <= 1, f"n={core.get('n_cell_lt_03')}"))
    # Named holdouts
    by_id = {d.get("id"): d for d in (core.get("docs") or [])}
    for hid in ("R010_bea_nipa_highlights", "32_real_census_table324"):
        d = by_id.get(hid)
        if d:
            cf = ((d.get("metrics") or {}).get("cell") or {}).get("f1")
            results.append(ok(f"G4.5 {hid} cell F1 ≥ 0.40", cf is not None and cf >= 0.40, f"{cf}"))
        else:
            # may be in full suite not core
            if struct and "runs" in struct:
                for d in struct["runs"][0].get("documents") or []:
                    if d.get("id") == hid:
                        cf = ((d.get("metrics") or {}).get("cell") or {}).get("f1")
                        results.append(ok(f"G4.5 {hid} cell F1 ≥ 0.40", cf is not None and cf >= 0.40, f"{cf}"))
                        break
    head = load(BENCH / "results" / "camelot_icdar_headtohead.json")
    us = ((head or {}).get("results") or {}).get("pdfparser") or {}
    if not us:
        results.append(ok("G4.6 ICDAR TEDS present", False, "run run_icdar_competitive.py"))
        results.append(ok("G4.7 ICDAR F1 present", False, "run run_icdar_competitive.py"))
    else:
        if head and head.get("note") and "restored" in str(head.get("note")).lower():
            results.append(ok("G4.6b ICDAR board live (not restored)", False, head.get("note")))
        # Peer parity goal: also report gap vs camelot_auto if present
        peers = (head or {}).get("results") or {}
        ca = peers.get("camelot_auto") or peers.get("camelot_lattice_vector") or {}
        results.append(ok("G4.6 ICDAR TEDS ≥ 0.50", (us.get("teds") or 0) >= 0.50, f"{us.get('teds')}"))
        results.append(ok("G4.7 ICDAR F1 ≥ 0.65", (us.get("f1") or 0) >= 0.65, f"{us.get('f1')}"))
        # Market target (aspirational hard bar for promotion to "peer-ready"):
        # F1 within 0.05 of best peer OR ≥ 0.75 absolute
        best_peer = max(
            (v.get("f1") or 0)
            for k, v in peers.items()
            if k != "pdfparser" and isinstance(v, dict)
        ) if any(k != "pdfparser" for k in peers) else None
        if best_peer is not None and best_peer > 0:
            gap = best_peer - (us.get("f1") or 0)
            results.append(
                ok(
                    "G4.8 ICDAR F1 peer-ready (gap≤0.05 or F1≥0.75)",
                    (us.get("f1") or 0) >= 0.75 or gap <= 0.05,
                    f"us={us.get('f1')} best_peer={best_peer} gap={gap:.3f}",
                )
            )
    return all(results)


def _count_tableoptions_product_fields() -> int | None:
    """Count PRODUCT_TABLE_OPTION_FIELDS from options.rs (G5.3)."""
    opts_rs = REPO / "crates" / "pdfparser-tables" / "src" / "options.rs"
    if not opts_rs.is_file():
        return None
    text = opts_rs.read_text(encoding="utf-8")
    # PRODUCT_TABLE_OPTION_FIELDS: &[&str] = &[ ... ];
    import re

    m = re.search(
        r"PRODUCT_TABLE_OPTION_FIELDS:\s*&\[&str\]\s*=\s*&\[(.*?)\]\s*;",
        text,
        re.S,
    )
    if not m:
        return None
    body = m.group(1)
    names = re.findall(r'"([a-z_]+)"', body)
    return len(names)


def _readme_maturity_ok() -> tuple[bool, str]:
    """G5.8: README must not claim false Production on OCR; ICDAR honesty present."""
    readme = REPO / "README.md"
    if not readme.is_file():
        return False, "missing README.md"
    t = readme.read_text(encoding="utf-8")
    # Must not claim full-page OCR as Production
    bad = False
    detail = []
    if "Full-page OCR" in t and "Production" in t:
        # allow "Not in product" / "Not planned"
        for line in t.splitlines():
            if "Full-page OCR" in line and "Production" in line and "Not" not in line:
                bad = True
                detail.append("OCR claimed Production")
    # Capability ladder: Text/tables labels present
    if "Production" not in t:
        return False, "no Production maturity labels"
    if "ICDAR" not in t:
        detail.append("missing ICDAR honesty section")
    # Phase-5 honesty: real-structure freeze path mentioned
    if "freezes/" not in t and "real_structure" not in t:
        detail.append("missing real_structure / freeze reference")
    ok_flag = not bad and not detail
    return ok_flag, "; ".join(detail) if detail else "maturity labels present"


def gate5(with_icdar: Path | None) -> bool:
    """Phase-5 polish: hold G1–4, API diet, CI, Fast, T3≥25, README."""
    print("=== GATE-5 Industry polish ===")
    if not gate4(with_icdar):
        print("  [FAIL] GATE-4 not green — cannot claim Phase-5")
        return False
    results = []

    # G5.2 HQ ≥ Auto on raster_needed subset (result artifact or equal when no render)
    hq_ab = load(RT / "results" / "hq_vs_auto_latest.json")
    if hq_ab:
        hq_cell = (hq_ab.get("summary") or {}).get("hq_micro_cell_f1")
        auto_cell = (hq_ab.get("summary") or {}).get("auto_micro_cell_f1")
        n = (hq_ab.get("summary") or {}).get("n_docs") or 0
        if hq_cell is not None and auto_cell is not None and n > 0:
            results.append(
                ok(
                    "G5.2 HQ cell F1 ≥ Auto on raster subset",
                    hq_cell + 1e-9 >= auto_cell,
                    f"hq={hq_cell:.4f} auto={auto_cell:.4f} n={n}",
                )
            )
        else:
            results.append(ok("G5.2 HQ vs Auto artifact valid", False, "missing cell scores"))
    else:
        # Soft path: when no external render, HQ ≡ Auto is acceptable if preset flags correct
        opts_rs = (REPO / "crates" / "pdfparser-tables" / "src" / "options.rs").read_text(
            encoding="utf-8"
        )
        has_hq = "HighQuality" in opts_rs and "enable_full_page_render: true" in opts_rs
        results.append(
            ok(
                "G5.2 HQ preset requests render (A/B artifact optional without tools)",
                has_hq,
                "run run_hq_vs_auto.py when pdftoppm/mutool available",
            )
        )

    # G5.3 public TableOptions product fields ≤ 12
    n_fields = _count_tableoptions_product_fields()
    results.append(
        ok(
            "G5.3 product TableOptions fields ≤ 12",
            n_fields is not None and n_fields <= 12,
            f"n={n_fields}",
        )
    )

    # G5.4 classic stream not in product Auto.
    # SSOT is the unit test auto_disables_classic_stream (cargo test). Product
    # page.rs gates StreamDetector on allow_classic_stream. Do not require
    # detect_stream_tables in page.rs — that symbol lives in stream.rs / detectors.rs.
    opts_rs = (REPO / "crates" / "pdfparser-tables" / "src" / "options.rs").read_text(
        encoding="utf-8"
    )
    page_rs = (REPO / "crates" / "pdfparser-tables" / "src" / "orchestrator" / "page.rs").read_text(
        encoding="utf-8"
    )
    results.append(
        ok(
            "G5.4 unit test auto_disables_classic_stream (SSOT)",
            "fn auto_disables_classic_stream" in opts_rs
            and "!o.allow_classic_stream" in opts_rs,
        )
    )
    results.append(
        ok(
            "G5.4 product path gates classic stream on allow_classic_stream",
            "allow_classic_stream" in page_rs,
        )
    )

    # G5.5 CI has freeze / gate job
    ci = REPO / ".github" / "workflows" / "ci.yml"
    ci_txt = ci.read_text(encoding="utf-8") if ci.is_file() else ""
    results.append(
        ok(
            "G5.5 CI workflow exists",
            ci.is_file(),
        )
    )
    results.append(
        ok(
            "G5.5 CI runs phase gates or real-track freeze job",
            "check_phase_gates" in ci_txt
            or "run_detect_discipline" in ci_txt
            or "phase-gates" in ci_txt
            or "real-track-gates" in ci_txt,
            "need real-track / phase-gates job",
        )
    )

    # G5.6 Fast preset: no full-page render
    results.append(
        ok(
            "G5.6 Fast preset exists",
            ("Fast," in opts_rs or "TablePreset::Fast" in opts_rs)
            and "Latency path" in opts_rs,
        )
    )
    # Look for Fast arm with both render flags false
    import re

    fast_arm = re.search(
        r"TablePreset::Fast\s*=>\s*\{.*?allow_auto_render:\s*false.*?\}",
        opts_rs,
        re.S,
    )
    if not fast_arm:
        fast_arm = re.search(
            r"TablePreset::Fast\s*=>\s*\{.*?enable_full_page_render:\s*false.*?allow_auto_render:\s*false",
            opts_rs,
            re.S,
        )
    results.append(
        ok(
            "G5.6 Fast disables full-page + opportunistic render",
            fast_arm is not None,
        )
    )
    lat = load(RT / "results" / "latency_probe_latest.json")
    if lat:
        p95 = (lat.get("summary") or {}).get("p95_ms")
        budget = (lat.get("summary") or {}).get("budget_p95_ms", 30_000)
        results.append(
            ok(
                "G5.6 latency probe p95 within budget",
                p95 is not None and p95 <= budget,
                f"p95={p95} budget={budget}",
            )
        )
        results.append(
            ok(
                "G5.6 Fast run had enable_full_page_render=false",
                (lat.get("summary") or {}).get("enable_full_page_render") is False,
                str((lat.get("summary") or {}).get("enable_full_page_render")),
            )
        )
    else:
        print("  [SKIP] G5.6 latency numbers (run run_latency_probe.py)")

    # G5.7 Structure T3 ≥ 25
    golds = list((RT / "gold").glob("*.json"))
    results.append(ok("G5.7 structure T3 golds ≥ 25", len(golds) >= 25, f"n={len(golds)}"))

    # G5.8 README maturity
    mat_ok, mat_detail = _readme_maturity_ok()
    results.append(ok("G5.8 README maturity labels honest", mat_ok, mat_detail))

    # Do not accept phase5 PASS freeze unless GATE-4 ICDAR already green on live board.
    g3 = load(RT / "freezes" / "g3_industry.json")
    head = load(BENCH / "results" / "camelot_icdar_headtohead.json")
    us = ((head or {}).get("results") or {}).get("pdfparser") or {}
    live = head and not (head.get("note") and "restored" in str(head.get("note")).lower())
    icdar_ok = live and (us.get("f1") or 0) >= 0.65 and (us.get("teds") or 0) >= 0.50
    if g3 and (g3.get("status") or {}).get("phase5") == "PASS" and not icdar_ok:
        results.append(
            ok(
                "G5 freeze phase5 PASS only with live ICDAR G4.6/G4.7",
                False,
                "revoke false PASS: ICDAR not peer-gate green",
            )
        )
    elif g3 is None:
        print("  [INFO] g3_industry.json not written yet (write only when GATE-5 truly green)")

    return all(results)


def _freeze_doc42_cell(freeze) -> float | None:
    for d in (freeze or {}).get("documents_auto") or []:
        if d.get("id") == "42_real_insurance_italian" and d.get("cell_f1") is not None:
            return float(d["cell_f1"])
    return None


def _owned_floor_cell(floors: dict, freeze) -> float:
    g2 = (freeze or {}).get("summaries", {}).get("auto") or {}
    band = float((floors.get("bands") or {}).get("micro_cell_f1", 0.03))
    if g2.get("micro_cell_f1") is not None:
        return float(g2["micro_cell_f1"]) - band
    return float((floors.get("floors") or {}).get("micro_cell_f1", 0.607))


def _owned_floor_det(floors: dict, freeze) -> float:
    g2 = (freeze or {}).get("summaries", {}).get("auto") or {}
    band = float((floors.get("bands") or {}).get("micro_det_count_f1", 0.02))
    if g2.get("micro_det_count_f1") is not None:
        return float(g2["micro_det_count_f1"]) - band
    return float((floors.get("floors") or {}).get("micro_det_count_f1", 0.944))


def _owned_floor_iou(floors: dict, freeze) -> float | None:
    g2 = (freeze or {}).get("summaries", {}).get("auto") or {}
    if g2.get("micro_det_iou_f1") is None:
        stored = (floors.get("floors") or {}).get("micro_det_iou_f1")
        return float(stored) if stored is not None else None
    band = float((floors.get("bands") or {}).get("micro_det_iou_f1", 0.03))
    return float(g2["micro_det_iou_f1"]) - band


def gate_owned_only(
    *,
    discipline_path: Path | None = None,
    fp_path: Path | None = None,
    structure_path: Path | None = None,
    freeze_path: Path | None = None,
    floors_path: Path | None = None,
) -> bool:
    """ICDAR-free merge bar (H8/H21). Must not load any ICDAR artifacts."""
    print("=== OWNED-ONLY (H21; ICDAR is not a gate) ===")
    floors = load(floors_path or (RT / "freezes" / "owned_gates_v0.json"))
    freeze = load(freeze_path or (RT / "freezes" / "g2.json"))
    disc = load(discipline_path or (RT / "results" / "detect_discipline_latest.json"))
    fp = load(fp_path or (RT / "results" / "real_fp_strict_latest.json"))
    struct = load(structure_path or (RT / "results" / "real_structure_latest.json"))
    results = []
    results.append(ok("owned_gates_v0.json present", floors is not None))
    if not floors:
        return False

    disc_cfg = floors.get("discipline") or {}
    exact_min = float(disc_cfg.get("exact_count_rate_min", (floors.get("floors") or {}).get("exact_count_rate", 0.88)))
    over_max = float(disc_cfg.get("over_doc_rate_max", (floors.get("floors") or {}).get("over_doc_rate", 0.12)))
    if not disc:
        results.append(ok("detect_discipline_latest.json present", False, "run run_detect_discipline.py"))
    else:
        s = disc.get("summary") or {}
        results.append(
            ok(
                f"discipline exact_count_rate >= {exact_min}",
                float(s.get("exact_count_rate") or 0) >= exact_min,
                f"{s.get('exact_count_rate')}",
            )
        )
        results.append(
            ok(
                f"discipline over_doc_rate <= {over_max}",
                float(s.get("over_doc_rate") if s.get("over_doc_rate") is not None else 1) <= over_max,
                f"{s.get('over_doc_rate')}",
            )
        )

    fp_cfg = floors.get("fp_strict") or {}
    zero_min = float(fp_cfg.get("zero_rate_min", (floors.get("floors") or {}).get("fp_strict_zero_rate", 1.0)))
    n_fp_req = int(fp_cfg.get("n", 12))
    if not fp:
        results.append(ok("real_fp_strict_latest.json present", False, "run run_fp_strict.py"))
    else:
        fs = fp.get("summary") or {}
        n_docs = int(fs.get("n_docs") or 0)
        results.append(
            ok(
                f"fp_strict n_docs >= {n_fp_req}",
                n_docs >= n_fp_req,
                f"n={n_docs}",
            )
        )
        results.append(
            ok(
                f"fp_strict zero_rate >= {zero_min}",
                float(fs.get("fp_zero_rate") or 0) >= zero_min,
                f"{fs.get('fp_zero_rate')}",
            )
        )

    if not freeze:
        results.append(ok("g2.json freeze present", False))
        return all(results)

    core = _core_structure_stats(struct, freeze)
    if not core:
        results.append(ok("real_structure core stats", False, "missing structure latest or freeze core ids"))
    else:
        cell_floor = _owned_floor_cell(floors, freeze)
        det_floor = _owned_floor_det(floors, freeze)
        cell = core.get("micro_cell_f1")
        det = core.get("micro_det_count_f1")
        results.append(
            ok(
                f"micro cell F1 >= g2 auto − band ({cell_floor:.4f})",
                cell is not None and cell >= cell_floor,
                f"{cell} vs floor {cell_floor:.4f} (n_core={core.get('n')})",
            )
        )
        results.append(
            ok(
                f"micro det count F1 >= g2 auto − band ({det_floor:.4f})",
                det is not None and det >= det_floor,
                f"{det} vs floor {det_floor:.4f}",
            )
        )
        iou_floor = _owned_floor_iou(floors, freeze)
        iou = core.get("micro_det_iou_f1")
        if iou_floor is not None and iou is not None:
            results.append(
                ok(
                    f"micro det IoU F1 >= g2 − band ({iou_floor:.4f})",
                    iou >= iou_floor,
                    f"{iou} vs floor {iou_floor:.4f}",
                )
            )
        elif iou_floor is not None and iou is None:
            print("  [SKIP] micro det IoU F1 (not present on live core docs)")
        else:
            print("  [SKIP] micro det IoU F1 (not present on g2 freeze)")

    nest = floors.get("nested_doc_42") or {}
    nest_id = nest.get("id") or "42_real_insurance_italian"
    nest_n = int(nest.get("n_tables", 2))
    found_42 = False
    if struct:
        docs = []
        if "runs" in struct:
            docs = struct["runs"][0].get("documents") or []
        if not docs:
            docs = struct.get("documents") or []
        for d in docs:
            if d.get("id") != nest_id:
                continue
            found_42 = True
            m = d.get("metrics") or {}
            dc = m.get("detection_count") or {}
            n_pred = dc.get("predicted", m.get("n_pred"))
            results.append(
                ok(
                    f"nested doc 42 still {nest_n} tables",
                    n_pred == nest_n,
                    f"pred={n_pred}",
                )
            )
            freeze_42 = nest.get("freeze_cell_f1")
            if freeze_42 is None:
                freeze_42 = _freeze_doc42_cell(freeze)
            if freeze_42 is not None:
                band42 = float(nest.get("cell_f1_band", 0.05))
                live_cell = (m.get("cell") or {}).get("f1")
                floor42 = float(freeze_42) - band42
                results.append(
                    ok(
                        f"nested doc 42 cell F1 >= freeze_42 − {band42}",
                        live_cell is not None and live_cell >= floor42,
                        f"{live_cell} vs floor {floor42:.4f} (freeze={freeze_42})",
                    )
                )
            break
    if not found_42:
        results.append(ok("nested doc 42 present in structure latest", False, nest_id))

    print("  [INFO] ICDAR is not a gate (H8)")
    return all(results)


def main() -> int:
    ap = argparse.ArgumentParser(
        description=(
            "Hard phase gates. CI merge bar is --owned-only (no ICDAR). "
            "--phase >=1 still loads ICDAR metrics for external promotion only."
        )
    )
    ap.add_argument(
        "--phase",
        type=int,
        default=None,
        help="Phase gate 0–5. Phase >=1 loads ICDAR JSON (not CI).",
    )
    ap.add_argument(
        "--owned-only",
        action="store_true",
        help="ICDAR-free merge bar: discipline + fp_strict + g2 core + doc 42.",
    )
    ap.add_argument(
        "--with-icdar",
        type=Path,
        default=None,
        help="Deprecated no-op: ICDAR always enforced from benchmark/results for phase≥1",
    )
    ap.add_argument("--discipline", type=Path, default=None, help="Override detect_discipline JSON")
    ap.add_argument("--fp-strict", type=Path, default=None, help="Override fp_strict JSON")
    ap.add_argument("--structure", type=Path, default=None, help="Override real_structure JSON")
    ap.add_argument("--freeze", type=Path, default=None, help="Override g2 freeze JSON")
    ap.add_argument("--owned-gates", type=Path, default=None, help="Override owned_gates_v0.json")
    args = ap.parse_args()
    if args.owned_only and args.phase is not None:
        ap.error("use either --owned-only or --phase, not both")
    if not args.owned_only and args.phase is None:
        ap.error("one of --owned-only or --phase is required")
    if args.owned_only:
        passed = gate_owned_only(
            discipline_path=args.discipline,
            fp_path=args.fp_strict,
            structure_path=args.structure,
            freeze_path=args.freeze,
            floors_path=args.owned_gates,
        )
        print("RESULT:", "PASS" if passed else "FAIL")
        return 0 if passed else 1
    # Always pass a non-None marker so any residual with_icdar branches still run.
    icdar_marker = Path("benchmark/results")
    if args.phase == 0:
        passed = gate0()
    elif args.phase == 1:
        passed = gate1(icdar_marker)
    elif args.phase == 2:
        passed = gate2(icdar_marker)
    elif args.phase == 3:
        passed = gate3(icdar_marker)
    elif args.phase == 4:
        passed = gate4(icdar_marker)
    elif args.phase == 5:
        passed = gate5(icdar_marker)
    else:
        print(f"Phase {args.phase} gate not fully automated yet")
        return 2
    print("RESULT:", "PASS" if passed else "FAIL")
    return 0 if passed else 1


if __name__ == "__main__":
    sys.exit(main())
