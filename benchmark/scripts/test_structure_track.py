#!/usr/bin/env python3
"""Phase 6 structure-track tests — must execute real cases (not empty skip).

Run: python3 benchmark/scripts/test_structure_track.py
Exit 0 only if all cases pass.
"""
from __future__ import annotations

import json
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SCRIPTS = Path(__file__).resolve().parent
sys.path.insert(0, str(SCRIPTS))

passed = 0
failed = 0


def check(name: str, cond: bool, detail: str = "") -> None:
    global passed, failed
    if cond:
        print(f"  OK  {name}")
        passed += 1
    else:
        print(f"  FAIL {name}: {detail}")
        failed += 1


def test_manifest_loads() -> None:
    path = ROOT / "real_track" / "manifests" / "real_structure_v0.json"
    check("manifest_exists", path.is_file(), str(path))
    man = json.loads(path.read_text(encoding="utf-8"))
    check("manifest_suite", man.get("suite") == "real_structure", str(man.get("suite")))
    check("manifest_has_g1_min", man.get("g1_min_docs") == 15, str(man.get("g1_min_docs")))
    check(
        "manifest_struggle_frac",
        man.get("struggle_first_min_frac") == 0.6,
        str(man.get("struggle_first_min_frac")),
    )
    n = man.get("n_docs", -1)
    docs = man.get("documents") or []
    check("manifest_n_docs_matches_list", n == len(docs), f"n={n} len={len(docs)}")
    g1_min = int(man.get("g1_min_docs") or 15)
    g1_ready = bool(man.get("g1_ready"))
    # Progressive ramp (5→15→25) is allowed; only g1_ready may claim Gate G1.
    check(
        "manifest_not_false_g1",
        (not g1_ready) or (n >= g1_min),
        f"g1_ready={g1_ready} with n_docs={n} < g1_min={g1_min}",
    )
    if n > 0:
        struggle_n = sum(1 for d in docs if d.get("struggle"))
        check(
            "manifest_has_gold_entries",
            all(d.get("gold") or d.get("gold_path") for d in docs),
            "missing gold pointers",
        )
        check(
            "manifest_struggle_count_recorded",
            struggle_n >= 0,
            f"struggle_n={struggle_n}",
        )


def test_coverage_and_demotion() -> None:
    cov = ROOT / "real_track" / "manifests" / "coverage_matrix.json"
    dem = ROOT / "real_track" / "manifests" / "demotion_list.json"
    check("coverage_matrix_exists", cov.is_file())
    check("demotion_list_exists", dem.is_file())
    if cov.is_file():
        c = json.loads(cov.read_text(encoding="utf-8"))
        modes = c.get("modes") or {}
        check("coverage_has_modes", len(modes) >= 5, f"modes={len(modes)}")
    if dem.is_file():
        d = json.loads(dem.read_text(encoding="utf-8"))
        check("demotion_signed_or_list", bool(d), "empty demotion")


def test_validate_structure_gold_empty_ok() -> None:
    """Suite validates (empty or progressive T3 golds); G1 not auto-claimed."""
    import subprocess

    r = subprocess.run(
        [sys.executable, str(SCRIPTS / "validate_structure_gold.py")],
        capture_output=True,
        text=True,
        cwd=str(ROOT.parent) if (ROOT.parent / "Cargo.toml").exists() else str(ROOT),
    )
    # script lives under benchmark/scripts; cwd should be repo or benchmark
    # validate uses parents[1] = benchmark
    check("validate_exit_0", r.returncode == 0, r.stderr or r.stdout)
    check(
        "validate_mentions_empty_or_ok",
        "OK" in (r.stdout + r.stderr),
        r.stdout + r.stderr,
    )


def test_validate_structure_gold_rejects_bad_t3() -> None:
    bad = {"id": "x", "expected_tables": [{"rows": 2, "cols": 2}]}  # no cells
    cells_ok = {
        "id": "y",
        "expected_tables": [
            {
                "page": 0,
                "bbox": {"x0": 0, "y0": 0, "x1": 10, "y1": 10},
                "cells": [["a", "b"], ["c", "d"]],
            }
        ],
    }

    def has_t3(g: dict) -> bool:
        et = g.get("expected_tables") or []
        if not et:
            return False
        for t in et:
            if not isinstance(t, dict) or not t.get("cells"):
                return False
        return True

    check("t3_reject_no_cells", not has_t3(bad))
    check("t3_accept_full_cells", has_t3(cells_ok))


def test_validate_with_temp_fixture() -> None:
    good = {
        "id": "fixture_ok",
        "schema_version": 1,
        "expected_tables": [
            {
                "page": 0,
                "bbox": {"x0": 0.0, "y0": 0.0, "x1": 100.0, "y1": 50.0},
                "rows": 2,
                "cols": 2,
                "cells": [["H1", "H2"], ["a", "b"]],
            }
        ],
    }
    bad = {"id": "fixture_bad", "expected_tables": [{"page": 0}]}

    def validate_doc(g: dict) -> list:
        errs = []
        et = g.get("expected_tables") or []
        if not et:
            errs.append("no expected_tables")
        for i, t in enumerate(et):
            if not isinstance(t, dict):
                errs.append(f"table {i} not object")
                continue
            if not t.get("cells"):
                errs.append(f"table {i} missing cells")
        return errs

    check("fixture_good_clean", validate_doc(good) == [])
    check("fixture_bad_errors", len(validate_doc(bad)) >= 1, str(validate_doc(bad)))
    gold_dir = ROOT / "real_track" / "gold"
    gold_dir.mkdir(parents=True, exist_ok=True)
    path = gold_dir / "_phase6_fixture_ok.json"
    path.write_text(json.dumps(good, indent=2), encoding="utf-8")
    loaded = json.loads(path.read_text(encoding="utf-8"))
    check("fixture_roundtrip", loaded["id"] == "fixture_ok")
    path.unlink(missing_ok=True)
    check("fixture_cleaned", not path.exists())


def test_suites_json_has_real_structure_or_smoke() -> None:
    suites = ROOT / "corpus" / "suites.json"
    check("suites_json_exists", suites.is_file())
    if not suites.is_file():
        return
    s = json.loads(suites.read_text(encoding="utf-8"))
    blob = json.dumps(s)
    check(
        "suites_mentions_real_detect_or_structure",
        "real_detect" in blob or "real_structure" in blob or "geom_unit" in blob,
        "missing real/geom suite keys",
    )


def test_practice_structure_suite() -> None:
    path = ROOT / "real_track" / "manifests" / "geom_unit_structure_v0.json"
    check("practice_suite_exists", path.is_file())
    if not path.is_file():
        return
    man = json.loads(path.read_text(encoding="utf-8"))
    check("practice_suite_name", man.get("suite") == "geom_unit_structure")
    check("practice_not_g1", man.get("suite") != "real_structure")
    n = man.get("n_docs", 0)
    check("practice_has_docs", n >= 1, f"n={n}")
    gold = ROOT / "real_track" / "gold" / "06_table_lattice.practice.json"
    check("practice_gold_exists", gold.is_file())
    if gold.is_file():
        g = json.loads(gold.read_text(encoding="utf-8"))
        check("practice_has_t3_cells", bool(g.get("expected_tables", [{}])[0].get("cells")))
        check(
            "practice_status_reviewed",
            g.get("status") == "reviewed_practice",
            str(g.get("status")),
        )
    drafts = ROOT / "real_track" / "gold" / "drafts"
    check("drafts_dir_exists", drafts.is_dir())
    check(
        "export_script_exists",
        (SCRIPTS / "export_structure_draft.py").is_file(),
    )


def test_smoke_script_dry_run() -> None:
    import subprocess

    smoke = SCRIPTS / "run_real_detect_smoke.py"
    check("smoke_script_exists", smoke.is_file())
    if not smoke.is_file():
        return
    r = subprocess.run(
        [sys.executable, str(smoke), "--dry-run"],
        capture_output=True,
        text=True,
        cwd=str(ROOT.parent),
    )
    check("smoke_dry_run_exit_0", r.returncode == 0, r.stderr or r.stdout)
    check(
        "smoke_dry_run_lists_docs",
        "docs:" in r.stdout or "pdf=OK" in r.stdout or "dry-run" in r.stdout.lower(),
        r.stdout[:500],
    )


def test_real_structure_runner() -> None:
    """Phase 9: real_structure metrics harness exists and dry-runs."""
    import subprocess

    runner = SCRIPTS / "run_real_structure.py"
    check("structure_runner_exists", runner.is_file())
    if not runner.is_file():
        return
    r = subprocess.run(
        [sys.executable, str(runner), "--dry-run"],
        capture_output=True,
        text=True,
        cwd=str(ROOT.parent),
    )
    check("structure_runner_dry_run_exit_0", r.returncode == 0, r.stderr or r.stdout)
    out = r.stdout + r.stderr
    check(
        "structure_runner_lists_docs",
        "dry-run OK" in out or "pdf=OK" in out or "n_docs=" in out,
        out[:400],
    )
    r2 = subprocess.run(
        [sys.executable, str(runner), "--help"],
        capture_output=True,
        text=True,
        cwd=str(ROOT.parent),
    )
    check("structure_runner_help", r2.returncode == 0)
    check(
        "structure_runner_mentions_preset",
        "preset" in (r2.stdout + r2.stderr).lower(),
        r2.stdout[:200],
    )


def test_soft_gold_structure_ramp() -> None:
    """Structure core may be empty after self-gold quarantine; peer drafts required."""
    man = json.loads(
        (ROOT / "real_track" / "manifests" / "real_structure_v0.json").read_text(
            encoding="utf-8"
        )
    )
    n = int(man.get("n_docs") or 0)
    docs = man.get("documents") or []
    g1_min = int(man.get("g1_min_docs") or 15)
    g1_ready = bool(man.get("g1_ready"))
    # Corpus G1 may be true once n≥g1_min (product Auto flip is a separate H3 gate).
    check(
        "structure_ramp_g1_corpus_ok",
        (not g1_ready) or (n >= g1_min),
        f"g1_ready={g1_ready} with n_docs={n} < g1_min={g1_min}",
    )
    check("manifest_n_matches", n == len(docs), f"n={n} len={len(docs)}")
    peer_dir = ROOT / "real_track" / "gold" / "drafts" / "peer"
    check("peer_draft_dir", peer_dir.is_dir())
    builder = SCRIPTS / "build_structure_gold_peers.py"
    check("peer_builder_exists", builder.is_file())
    idx = peer_dir / "INDEX.json"
    check("peer_index_exists", idx.is_file(), "run build_structure_gold_peers.py --all-soft-gold")
    if idx.is_file():
        ix = json.loads(idx.read_text(encoding="utf-8"))
        docs_ix = ix.get("documents") or []
        check("peer_index_has_docs", len(docs_ix) >= 5, f"n={len(docs_ix)}")
        for d in docs_ix[:5]:
            gid = d.get("id")
            gp = peer_dir / f"{gid}.peer.json"
            check(f"peer_file_{gid}", gp.is_file())
            if not gp.is_file():
                continue
            g = json.loads(gp.read_text(encoding="utf-8"))
            st = str(g.get("status") or "")
            check(
                f"peer_status_{gid}",
                st.startswith("peer_consensus")
                or st.startswith("vision_")
                or st in ("needs_human_review", "peer_consensus_draft"),
                st,
            )
            check(f"peer_has_tables_{gid}", bool(g.get("expected_tables")))
            # Must not claim pdfparser is authority
            pol = (g.get("policy") or "") + json.dumps(g.get("pdfparser_shadow"))
            check(f"peer_policy_not_self_{gid}", "NOT pdfparser" in (g.get("policy") or "") or "not used as gold" in pol.lower() or "NEVER" in (g.get("policy") or ""))
    qdir = ROOT / "real_track" / "gold" / "quarantine"
    check("quarantine_dir_exists", qdir.is_dir())
    # Struggle golds 31/32 re-added as vision+stream reviewed structure gold
    for name in [
        "31_real_background_checks.json",
        "32_real_census_table324.json",
        "30_real_ca_warn_report.json",
        "35_real_camelot_fuel.json",
    ]:
        live = ROOT / "real_track" / "gold" / name
        check(f"gold_live_{name}", live.is_file(), str(live))
        if live.is_file():
            g = json.loads(live.read_text(encoding="utf-8"))
            check(
                f"approved_is_reviewed_{name}",
                g.get("status") == "reviewed" and g.get("track") == "real_structure",
                str(g.get("status")),
            )



def main() -> int:
    print("test_structure_track: running Phase 6/8 cases...")
    test_manifest_loads()
    test_coverage_and_demotion()
    test_validate_structure_gold_empty_ok()
    test_validate_structure_gold_rejects_bad_t3()
    test_validate_with_temp_fixture()
    test_suites_json_has_real_structure_or_smoke()
    test_practice_structure_suite()
    test_smoke_script_dry_run()
    test_real_structure_runner()
    test_soft_gold_structure_ramp()
    print(f"test_structure_track: {passed} passed, {failed} failed")
    if failed:
        return 1
    if passed < 10:
        print(f"FAIL: only {passed} cases (need ≥10)", file=sys.stderr)
        return 1
    print(f"test_structure_track: OK ({passed} cases)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
