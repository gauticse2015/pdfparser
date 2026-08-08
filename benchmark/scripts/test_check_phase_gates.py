#!/usr/bin/env python3
"""P0.6 unit tests: --owned-only, dump-compare, --binary argparse.

No live binary required. Exit 0 on success.
Run: python3 benchmark/scripts/test_check_phase_gates.py
"""
from __future__ import annotations

import json
import subprocess
import sys
import tempfile
from pathlib import Path

SCRIPTS = Path(__file__).resolve().parent
BENCH = SCRIPTS.parent
REPO = BENCH.parent
sys.path.insert(0, str(SCRIPTS))

import check_phase_gates as gates  # noqa: E402
import compare_table_dumps as cmp  # noqa: E402

failed = 0
passed = 0


def check(name: str, cond: bool, detail: str = "") -> None:
    global failed, passed
    if cond:
        print(f"  OK  {name}")
        passed += 1
    else:
        print(f"  FAIL {name}: {detail}")
        failed += 1


def _help(script: str) -> str:
    r = subprocess.run(
        [sys.executable, str(SCRIPTS / script), "--help"],
        capture_output=True,
        text=True,
        cwd=str(REPO),
    )
    return (r.stdout or "") + (r.stderr or "")


def test_argparse_binary_and_owned() -> None:
    for script in (
        "run_detect_discipline.py",
        "run_fp_strict.py",
        "run_real_structure.py",
        "run_latency_probe.py",
    ):
        out = _help(script)
        check(f"{script} --binary", "--binary" in out, out[:200])
    out = _help("check_phase_gates.py")
    check("check_phase_gates --owned-only", "--owned-only" in out, out[:200])
    check("check_phase_gates --phase still listed", "--phase" in out)
    dump_h = _help("dump_product_tables.py")
    check("dump --binary", "--binary" in dump_h)
    check("dump --freeze", "--freeze" in dump_h)
    check("dump --structure-manifest", "--structure-manifest" in dump_h)
    check("dump --out", "--out" in dump_h)
    cmp_h = _help("compare_table_dumps.py")
    check("compare --before", "--before" in cmp_h)
    check("compare --after", "--after" in cmp_h)


def test_owned_gates_file() -> None:
    path = BENCH / "real_track" / "freezes" / "owned_gates_v0.json"
    check("owned_gates_v0 exists", path.is_file(), str(path))
    if not path.is_file():
        return
    data = json.loads(path.read_text(encoding="utf-8"))
    check("icdar not a gate", (data.get("policy") or {}).get("icdar") == "not_a_gate")
    floors = data.get("floors") or {}
    check("floor exact_count_rate 0.88", abs(float(floors.get("exact_count_rate", 0)) - 0.88) < 1e-9)
    check("floor over_doc_rate 0.12", abs(float(floors.get("over_doc_rate", 0)) - 0.12) < 1e-9)
    check("floor fp zero 1.0", abs(float(floors.get("fp_strict_zero_rate", 0)) - 1.0) < 1e-9)
    check("nested 42 n=2", int((data.get("nested_doc_42") or {}).get("n_tables", 0)) == 2)


def test_owned_only_no_icdar_load() -> None:
    orig = gates.load
    seen: list[str] = []

    def wrapped(p: Path):
        seen.append(str(p))
        low = str(p).lower()
        if "icdar" in low or "headtohead" in low:
            raise AssertionError(f"owned-only loaded ICDAR path: {p}")
        return orig(p)

    gates.load = wrapped  # type: ignore[assignment]
    try:
        ok = gates.gate_owned_only()
    finally:
        gates.load = orig  # type: ignore[assignment]
    check("owned-only PASS on committed JSON", ok is True, f"ok={ok}")
    check("owned-only loaded some JSON", len(seen) >= 3, f"n={len(seen)}")
    check(
        "owned-only never icdar/headtohead",
        all("icdar" not in s.lower() and "headtohead" not in s.lower() for s in seen),
        str(seen),
    )


def _mini_table(**kwargs) -> dict:
    t = {
        "page": 0,
        "method": "lattice",
        "rows": 2,
        "cols": 2,
        "header_rows": 1,
        "bbox": {"x0": 10.0, "y0": 20.0, "x1": 100.0, "y1": 80.0},
        "weak_edges": False,
        "text_row_recovery": False,
        "text_col_recovery": False,
        "multitable_stream_recovery": False,
        "stream_vs_overwide_hybrid": False,
        "continued_from_previous_page": False,
        "continued_to_next_page": False,
        "strategy_provenance": ["s2_lattice"],
        "confidence": 0.9,
        "notes": ["diag a"],
        "cells": [
            {"row": 0, "col": 0, "rowspan": 1, "colspan": 1, "text": "H1"},
            {"row": 0, "col": 1, "rowspan": 1, "colspan": 1, "text": "H2"},
        ],
    }
    t.update(kwargs)
    return t


def _dump(tables, *, notes_variant: str = "a", engine: str = "engine_v2") -> dict:
    tabs = []
    for t in tables:
        tt = dict(t)
        tt["notes"] = [f"diag {notes_variant}"]
        tabs.append(tt)
    return {
        "schema": "pdfparser_table_dump_v1",
        "binary": "target/release/pdfparser",
        "preset": "auto",
        "engine_path": engine,
        "stitch_multipage": False,
        "documents": [
            {
                "id": "30_real_ca_warn_report",
                "pdf": "corpus/real/30_real_ca_warn_report.pdf",
                "pages": [{"index": 0, "tables": tabs}],
            }
        ],
    }


def test_compare_dumps() -> None:
    a = _dump([_mini_table()], notes_variant="before")
    b = _dump([_mini_table()], notes_variant="after")
    diffs = cmp.compare_dumps(a, b)
    check("notes-only dumps identical", diffs == [], str(diffs))

    c = _dump([_mini_table(method="hybrid")])
    diffs = cmp.compare_dumps(a, c)
    check("method mismatch fails", any("method" in d for d in diffs), str(diffs))

    d = _dump([_mini_table(bbox={"x0": 10.0, "y0": 20.0, "x1": 100.0, "y1": 80.4})])
    diffs = cmp.compare_dumps(a, d)
    check("bbox within 0.5pt ok", diffs == [], str(diffs))

    e = _dump([_mini_table(bbox={"x0": 10.0, "y0": 20.0, "x1": 100.0, "y1": 81.2})])
    diffs = cmp.compare_dumps(a, e)
    check("bbox >0.5pt fails", any("bbox" in x for x in diffs), str(diffs))

    f = _dump([_mini_table(cells=[
        {"row": 0, "col": 0, "rowspan": 1, "colspan": 1, "text": "H1"},
        {"row": 0, "col": 1, "rowspan": 1, "colspan": 1, "text": "CHANGED"},
    ])])
    diffs = cmp.compare_dumps(a, f)
    check("cell text mismatch fails", any("text" in x for x in diffs), str(diffs))

    g = _dump([_mini_table(strategy_provenance=["s5_network", "s2_lattice"])])
    h = _dump([_mini_table(strategy_provenance=["s2_lattice", "s5_network"])])
    diffs = cmp.compare_dumps(g, h)
    check("provenance set order ignored", diffs == [], str(diffs))

    i = _dump([_mini_table()], engine="legacy")
    diffs = cmp.compare_dumps(a, i)
    check("engine_path mismatch fails", any("engine_path" in x for x in diffs), str(diffs))

    j = _dump([_mini_table(), _mini_table(page=0, bbox={"x0": 1, "y0": 1, "x1": 2, "y1": 2})])
    diffs = cmp.compare_dumps(a, j)
    check("count mismatch fails", any("count" in x for x in diffs), str(diffs))

    k = _dump([_mini_table(confidence=0.90005)])
    diffs = cmp.compare_dumps(a, k)
    check("confidence 1e-4 ok", diffs == [], str(diffs))


def test_compare_cli_roundtrip() -> None:
    a = _dump([_mini_table()], notes_variant="x")
    b = _dump([_mini_table()], notes_variant="y")
    with tempfile.TemporaryDirectory() as td:
        p1 = Path(td) / "before.json"
        p2 = Path(td) / "after.json"
        p1.write_text(json.dumps(a), encoding="utf-8")
        p2.write_text(json.dumps(b), encoding="utf-8")
        r = subprocess.run(
            [sys.executable, str(SCRIPTS / "compare_table_dumps.py"), "--before", str(p1), "--after", str(p2)],
            capture_output=True,
            text=True,
            cwd=str(REPO),
        )
        check("compare cli identical exit 0", r.returncode == 0, r.stdout + r.stderr)


def test_dump_dry_run() -> None:
    with tempfile.TemporaryDirectory() as td:
        out = Path(td) / "dump.json"
        r = subprocess.run(
            [
                sys.executable,
                str(SCRIPTS / "dump_product_tables.py"),
                "--freeze",
                str(BENCH / "real_track" / "freezes" / "g2.json"),
                "--structure-manifest",
                str(BENCH / "real_track" / "manifests" / "real_structure_v0.json"),
                "--out",
                str(out),
                "--dry-run",
            ],
            capture_output=True,
            text=True,
            cwd=str(REPO),
        )
        check("dump dry-run exit 0", r.returncode == 0, r.stdout + r.stderr)
        if not out.is_file():
            check("dump dry-run wrote file", False)
            return
        data = json.loads(out.read_text(encoding="utf-8"))
        freeze = json.loads((BENCH / "real_track" / "freezes" / "g2.json").read_text())
        ids = [d["id"] for d in freeze["documents_auto"]]
        got = [d["id"] for d in data.get("documents") or []]
        check("dump schema v1", data.get("schema") == "pdfparser_table_dump_v1")
        check("dump engine_path engine_v2", data.get("engine_path") == "engine_v2")
        check("dump freeze ids via manifest", got == ids, f"got={got}")
        check("no g2_core manifest created", not (BENCH / "real_track" / "manifests" / "g2_core.json").exists())


def test_check_phase_gates_cli_owned() -> None:
    r = subprocess.run(
        [sys.executable, str(SCRIPTS / "check_phase_gates.py"), "--owned-only"],
        capture_output=True,
        text=True,
        cwd=str(REPO),
    )
    out = r.stdout + r.stderr
    check("cli --owned-only exit 0", r.returncode == 0, out[-500:])
    check("cli RESULT PASS", "RESULT: PASS" in out, out[-200:])
    check("cli no ICDAR gate names", "G1.10" not in out and "G1.11" not in out and "G2.6" not in out)
    r0 = subprocess.run(
        [sys.executable, str(SCRIPTS / "check_phase_gates.py"), "--phase", "0"],
        capture_output=True,
        text=True,
        cwd=str(REPO),
    )
    check("cli --phase 0 still works", r0.returncode == 0, (r0.stdout + r0.stderr)[-300:])


def test_ci_yml_no_phase_12() -> None:
    ci = (REPO / ".github" / "workflows" / "ci.yml").read_text(encoding="utf-8")
    check("ci has --owned-only", "--owned-only" in ci)
    check("ci not hard --phase 1", "--phase 1" not in ci)
    check("ci not hard --phase 2", "--phase 2" not in ci)
    check("ci keeps assert_no_icdar", "assert_no_icdar" in ci)
    check("ci keeps --phase 0", "--phase 0" in ci)


def main() -> int:
    print("test_check_phase_gates: P0.6 owned contract...")
    test_argparse_binary_and_owned()
    test_owned_gates_file()
    test_owned_only_no_icdar_load()
    test_compare_dumps()
    test_compare_cli_roundtrip()
    test_dump_dry_run()
    test_check_phase_gates_cli_owned()
    test_ci_yml_no_phase_12()
    print(f"test_check_phase_gates: {passed} passed, {failed} failed")
    if failed:
        return 1
    print(f"test_check_phase_gates: OK ({passed} cases)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
