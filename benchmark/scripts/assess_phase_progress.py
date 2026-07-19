#!/usr/bin/env python3
"""Assess ICDAR + real_structure vs a baseline floors file. Exit 0 if no-regress + optional gates."""
from __future__ import annotations
import argparse, json, sys
from pathlib import Path

BENCH = Path(__file__).resolve().parents[1]
RT = BENCH / "real_track"

def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--baseline", type=Path, default=RT/"results"/"phase_ab_baseline.json")
    ap.add_argument("--require-g3", action="store_true")
    ap.add_argument("--require-g4-teds", action="store_true")
    args = ap.parse_args()
    base = json.loads(args.baseline.read_text())
    floors = base["floors"]
    h = json.loads((BENCH/"results"/"camelot_icdar_headtohead.json").read_text())
    us = h["results"]["pdfparser"]
    # refuse restored
    if h.get("note") and "restored" in str(h.get("note")).lower():
        print("FAIL: ICDAR board is restored stub")
        return 1
    s = json.loads((RT/"results"/"real_structure_latest.json").read_text())
    g2 = json.loads((RT/"freezes"/"g2.json").read_text())
    core_ids = {d["id"] for d in g2["documents_auto"]}
    by_id = {d["id"]: d for d in s["runs"][0]["documents"]}
    cells = []
    reg = []
    for cid in core_ids:
        d = by_id.get(cid)
        if not d: continue
        nc = d["metrics"]["cell"]["f1"]
        cells.append(nc)
        old = next(x["cell_f1"] for x in g2["documents_auto"] if x["id"]==cid)
        if nc < old - 0.03:
            reg.append((cid, old, nc))
    core = sum(cells)/len(cells) if cells else 0

    def ok(name, cond, detail=""):
        print(f"  [{'PASS' if cond else 'FAIL'}] {name}" + (f" — {detail}" if detail else ""))
        return cond

    print("=== Phase progress assess ===")
    results = []
    results.append(ok("ICDAR F1 no-drop", us["f1"] + 1e-9 >= floors["icdar_f1_min"], f"{us['f1']:.4f} >= {floors['icdar_f1_min']:.4f}"))
    results.append(ok("ICDAR TEDS no-drop", us["teds"] + 1e-9 >= floors["icdar_teds_min"], f"{us['teds']:.4f} >= {floors['icdar_teds_min']:.4f}"))
    results.append(ok("ICDAR row no-drop", us["row"] + 1e-9 >= floors["icdar_row_min"], f"{us['row']:.4f} >= {floors['icdar_row_min']:.4f}"))
    results.append(ok("ICDAR col no-drop", us["col"] + 1e-9 >= floors["icdar_col_min"], f"{us['col']:.4f} >= {floors['icdar_col_min']:.4f}"))
    results.append(ok("real core cell no-drop", core + 1e-9 >= floors["real_core_min"], f"{core:.4f} >= {floors['real_core_min']:.4f}"))
    results.append(ok("g2 doc regressions (-0.03) none", len(reg)==0, str(reg)))
    if args.require_g3:
        results.append(ok("G3.7 row ≥ 0.50", us["row"] >= 0.50, f"{us['row']:.4f}"))
        results.append(ok("G3.8 col ≥ 0.55", us["col"] >= 0.55, f"{us['col']:.4f}"))
    if args.require_g4_teds:
        results.append(ok("G4.6 TEDS ≥ 0.50", us["teds"] >= 0.50, f"{us['teds']:.4f}"))
        results.append(ok("G4.7 F1 ≥ 0.65", us["f1"] >= 0.65, f"{us['f1']:.4f}"))
    # improvement report
    b = base["icdar"]
    print(f"  [INFO] ΔF1={us['f1']-b['f1']:+.4f} ΔTEDS={us['teds']-b['teds']:+.4f} Δrow={us['row']-b['row']:+.4f} Δcol={us['col']-b['col']:+.4f}")
    print("RESULT:", "PASS" if all(results) else "FAIL")
    return 0 if all(results) else 1

if __name__ == "__main__":
    sys.exit(main())
