#!/usr/bin/env python3
"""HQ vs Auto A/B on real_structure suite (GATE-5 G5.2).

Runs real_structure for auto and high-quality presets, compares micro cell F1
on the freeze core set (or full suite if freeze missing).

When full-page render tools are absent, HQ ≈ Auto (fail-soft) — still valid:
HQ must not regress below Auto.
"""
from __future__ import annotations

import json
import subprocess
import sys
from datetime import datetime, timezone
from pathlib import Path

BENCH = Path(__file__).resolve().parents[1]
REPO = BENCH.parent
RT = BENCH / "real_track"
OUT = RT / "results" / "hq_vs_auto_latest.json"
SCRIPT = BENCH / "scripts" / "run_real_structure.py"


def run_preset(preset: str) -> dict:
    r = subprocess.run(
        [sys.executable, str(SCRIPT), "--preset", preset],
        cwd=str(REPO),
        capture_output=True,
        text=True,
    )
    if r.returncode != 0:
        raise RuntimeError(f"structure {preset} failed: {r.stderr[-500:] or r.stdout[-500:]}")
    data = json.loads((RT / "results" / "real_structure_latest.json").read_text(encoding="utf-8"))
    return data


def micro_cell(data: dict) -> float | None:
    if "runs" in data and data["runs"]:
        sm = data["runs"][0].get("summary") or {}
        return sm.get("micro_cell_f1")
    return (data.get("summary") or {}).get("micro_cell_f1")


def core_micro_cell(data: dict) -> tuple[float | None, int]:
    freeze = RT / "freezes" / "g2.json"
    if not freeze.is_file() or "runs" not in data:
        c = micro_cell(data)
        return c, 0
    fr = json.loads(freeze.read_text(encoding="utf-8"))
    core_ids = {d.get("id") for d in (fr.get("documents_auto") or []) if d.get("id")}
    docs = data["runs"][0].get("documents") or []
    cells = []
    for d in docs:
        if d.get("id") not in core_ids:
            continue
        cf = ((d.get("metrics") or {}).get("cell") or {}).get("f1")
        if cf is not None:
            cells.append(cf)
    if not cells:
        return micro_cell(data), 0
    return sum(cells) / len(cells), len(cells)


def main() -> int:
    print("=== HQ vs Auto A/B ===")
    print("running Auto…")
    auto_data = run_preset("auto")
    auto_cell, n_core = core_micro_cell(auto_data)
    # Save auto snapshot path
    auto_path = RT / "results" / "real_structure_auto_ab.json"
    auto_path.write_text(json.dumps(auto_data, indent=2) + "\n")

    print("running HighQuality…")
    hq_data = run_preset("high-quality")
    hq_cell, n_core_hq = core_micro_cell(hq_data)
    hq_path = RT / "results" / "real_structure_hq_ab.json"
    hq_path.write_text(json.dumps(hq_data, indent=2) + "\n")

    # Restore auto as latest (product default artifact)
    (RT / "results" / "real_structure_latest.json").write_text(
        json.dumps(auto_data, indent=2) + "\n"
    )

    n = n_core or n_core_hq
    summary = {
        "auto_micro_cell_f1": auto_cell,
        "hq_micro_cell_f1": hq_cell,
        "n_docs": n,
        "hq_ge_auto": (hq_cell is not None and auto_cell is not None and hq_cell + 1e-9 >= auto_cell),
        "note": "Without external render tools HQ fail-softs to Auto-equivalent quality.",
    }
    out = {
        "generated_at": datetime.now(timezone.utc).isoformat(),
        "summary": summary,
    }
    OUT.write_text(json.dumps(out, indent=2) + "\n")
    print(f"wrote {OUT} auto={auto_cell} hq={hq_cell} ge={summary['hq_ge_auto']}")
    return 0 if summary["hq_ge_auto"] else 1


if __name__ == "__main__":
    sys.exit(main())
