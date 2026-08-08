#!/usr/bin/env python3
"""Unit tests for latency_fast_v0 freeze math and the two independent fail rules.

No binary / ICDAR / full-page render. Exit 0 on success.
"""
from __future__ import annotations

import json
import sys
import tempfile
from pathlib import Path

SCRIPTS = Path(__file__).resolve().parent
ROOT = SCRIPTS.parents[1]
sys.path.insert(0, str(SCRIPTS))

import check_latency_freeze as C  # noqa: E402


def _approx(a: float, b: float, eps: float = 1e-6) -> bool:
    return abs(a - b) <= eps


def test_budget_is_max_not_min() -> None:
    # Design sample: p95~409, max~604 => max(613.5, 724.8) = 724.8-class
    p95 = 409.03888345346803
    mx = 604.4711249996908
    budget = C.compute_budget_p95_ms(p95, mx)
    assert budget == max(p95 * 1.5, mx * 1.2)
    assert budget != min(p95 * 1.5, mx * 1.2)
    assert budget > p95 * 1.5
    assert _approx(budget, mx * 1.2)


def test_committed_freeze_matches_formula() -> None:
    path = ROOT / "benchmark" / "real_track" / "freezes" / "latency_fast_v0.json"
    freeze = json.loads(path.read_text(encoding="utf-8"))
    errs = C.validate_freeze_schema(freeze)
    assert errs == [], errs
    expected = C.compute_budget_p95_ms(
        float(freeze["recorded_p95_ms"]), float(freeze["recorded_max_ms"])
    )
    assert _approx(float(freeze["budget_p95_ms"]), expected)
    assert freeze["n_docs"] == 8
    assert freeze["enable_full_page_render"] is False
    assert freeze["allow_auto_render"] is False
    assert freeze["hardware"]["runner_os"]
    assert freeze["hardware"]["note"]
    assert freeze["rules"]["do_not_wrap_in_min"] is True
    assert freeze["rules"]["do_not_gate_vs_freeze_recorded_p95_x_1_10"] is True
    # Honesty: freeze must not claim ubuntu-latest tightening is done.
    note = freeze["hardware"]["note"].lower()
    assert "ubuntu-latest" in note
    assert "tbd" in note or "not an ubuntu-latest" in note


def test_rule1_fails_over_budget() -> None:
    rows = C.evaluate_rules(
        live_p95_ms=800.0,
        budget_p95_ms=725.365,
        prev_commit_p95_ms=None,
        same_machine_class=False,
    )
    by = {n: (ok, d) for n, ok, d in rows}
    assert by["absolute_ceiling"][0] is False
    assert by["same_class_drift"][0] is True  # skipped / first sample


def test_rule2_independent_of_budget() -> None:
    """Drift can fail even when live p95 is under budget (no min())."""
    rows = C.evaluate_rules(
        live_p95_ms=700.0,
        budget_p95_ms=725.365,
        prev_commit_p95_ms=600.0,
        same_machine_class=True,
    )
    by = {n: (ok, d) for n, ok, d in rows}
    assert by["absolute_ceiling"][0] is True  # 700 <= 725
    assert by["same_class_drift"][0] is False  # 700 > 600 * 1.10 = 660


def test_rule1_independent_when_drift_would_pass() -> None:
    """Budget fail still fires when drift vs prev would pass (no min())."""
    rows = C.evaluate_rules(
        live_p95_ms=800.0,
        budget_p95_ms=725.365,
        prev_commit_p95_ms=780.0,
        same_machine_class=True,
    )
    by = {n: (ok, d) for n, ok, d in rows}
    assert by["absolute_ceiling"][0] is False  # 800 > 725
    assert by["same_class_drift"][0] is True  # 800 <= 780 * 1.10 = 858


def test_do_not_gate_vs_recorded_p95_x_110() -> None:
    """Live p95 above recorded*1.10 but under budget must PASS both rules.

    Wrapping min(budget, recorded_p95*1.10) would incorrectly fail this case
    (Issue 11 / design Latency section).
    """
    recorded_p95 = 409.03888345346803
    budget = C.compute_budget_p95_ms(recorded_p95, 604.4711249996908)
    live = recorded_p95 * 1.15  # ~470 > 450 but << 725
    assert live > recorded_p95 * 1.10
    assert live < budget
    rows = C.evaluate_rules(
        live_p95_ms=live,
        budget_p95_ms=budget,
        prev_commit_p95_ms=None,
        same_machine_class=False,
    )
    assert all(ok for _n, ok, _d in rows), rows


def test_first_sample_on_class_skips_drift() -> None:
    rows = C.evaluate_rules(
        live_p95_ms=700.0,
        budget_p95_ms=725.365,
        prev_commit_p95_ms=600.0,
        same_machine_class=False,
    )
    by = {n: (ok, d) for n, ok, d in rows}
    assert by["absolute_ceiling"][0] is True
    assert by["same_class_drift"][0] is True
    assert "skipped" in by["same_class_drift"][1]


def test_fast_never_renders_rejects_hq() -> None:
    errs = C.check_fast_never_renders(
        {
            "preset": "fast",
            "enable_full_page_render": False,
            "allow_auto_render": False,
            "cli_args": ["extract", "--tables", "--table-preset", "highquality"],
        }
    )
    assert errs, "HighQuality CLI must fail Fast never-render check"


def test_fast_never_renders_rejects_true_flags() -> None:
    errs = C.check_fast_never_renders(
        {
            "preset": "fast",
            "enable_full_page_render": True,
            "allow_auto_render": False,
        }
    )
    assert any("enable_full_page_render" in e for e in errs)
    errs2 = C.check_fast_never_renders(
        {
            "preset": "fast",
            "enable_full_page_render": False,
            "allow_auto_render": True,
        }
    )
    assert any("allow_auto_render" in e for e in errs2)


def test_normalize_machine_class() -> None:
    assert C.normalize_machine_class("Darwin") == "macos"
    assert C.normalize_machine_class("ubuntu-latest") == "linux"
    assert C.normalize_machine_class("Windows") == "windows"


def test_prev_json_class_mismatch_is_first_sample() -> None:
    with tempfile.TemporaryDirectory() as td:
        p = Path(td) / "prev.json"
        p.write_text(
            json.dumps({"machine_class": "macos", "prev_commit_p95_ms": 400.0}),
            encoding="utf-8",
        )
        assert C.prev_p95_from_file(p, "linux") is None
        assert C.prev_p95_from_file(p, "macos") == 400.0


def test_cli_end_to_end_pass_and_fail() -> None:
    freeze_path = ROOT / "benchmark" / "real_track" / "freezes" / "latency_fast_v0.json"
    freeze = json.loads(freeze_path.read_text(encoding="utf-8"))
    budget = float(freeze["budget_p95_ms"])
    with tempfile.TemporaryDirectory() as td:
        td_p = Path(td)
        live_ok = {
            "summary": {
                "preset": "fast",
                "n_docs": 8,
                "p50_ms": 12.0,
                "p95_ms": budget - 10.0,
                "max_ms": 600.0,
                "budget_p95_ms": 30000.0,
                "enable_full_page_render": False,
                "allow_auto_render": False,
                "cli_args": ["extract", "--tables", "--table-preset", "fast"],
            }
        }
        live_fail = {
            "summary": {
                **live_ok["summary"],
                "p95_ms": budget + 50.0,
            }
        }
        ok_path = td_p / "ok.json"
        fail_path = td_p / "fail.json"
        ok_path.write_text(json.dumps(live_ok), encoding="utf-8")
        fail_path.write_text(json.dumps(live_fail), encoding="utf-8")
        prev = td_p / "prev.json"
        rc = C.main(
            [
                "--freeze",
                str(freeze_path),
                "--live",
                str(ok_path),
                "--machine-class",
                "linux",
                "--write-prev",
                str(prev),
            ]
        )
        assert rc == 0, "first linux sample under budget must pass (rule 1 only)"
        snap = json.loads(prev.read_text(encoding="utf-8"))
        assert snap["machine_class"] == "linux"
        assert _approx(snap["prev_commit_p95_ms"], budget - 10.0)

        rc2 = C.main(
            [
                "--freeze",
                str(freeze_path),
                "--live",
                str(fail_path),
                "--machine-class",
                "linux",
            ]
        )
        assert rc2 == 1, "over-budget must fail rule 1"


def main() -> int:
    tests = [
        test_budget_is_max_not_min,
        test_committed_freeze_matches_formula,
        test_rule1_fails_over_budget,
        test_rule2_independent_of_budget,
        test_rule1_independent_when_drift_would_pass,
        test_do_not_gate_vs_recorded_p95_x_110,
        test_first_sample_on_class_skips_drift,
        test_fast_never_renders_rejects_hq,
        test_fast_never_renders_rejects_true_flags,
        test_normalize_machine_class,
        test_prev_json_class_mismatch_is_first_sample,
        test_cli_end_to_end_pass_and_fail,
    ]
    failed = 0
    for fn in tests:
        try:
            fn()
            print(f"  OK  {fn.__name__}")
        except Exception as e:  # noqa: BLE001 — surface assertion text
            failed += 1
            print(f"  FAIL {fn.__name__}: {e}")
    print(f"{len(tests) - failed}/{len(tests)} passed")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
