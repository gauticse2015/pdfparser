#!/usr/bin/env python3
"""Compare a live Fast latency probe against latency_fast_v0.json.

Two independent fail rules (do NOT wrap in min()):

  1. Absolute ceiling: fail if live p95 > freeze.budget_p95_ms
  2. Same-class drift: fail if live p95 > prev_commit_p95 * 1.10
     on the same machine class. First sample on a class: rule 1 only.

Do NOT gate live p95 against freeze recorded_p95 * 1.10.
Probe script budget_p95_ms=30000 stays informational; this checker uses the freeze.

Fast must never full-page-render (enable_full_page_render / allow_auto_render false).
"""
from __future__ import annotations

import argparse
import json
import platform
import sys
from pathlib import Path
from typing import Any

BENCH = Path(__file__).resolve().parents[1]
RT = BENCH / "real_track"
DEFAULT_FREEZE = RT / "freezes" / "latency_fast_v0.json"
DEFAULT_LIVE = RT / "results" / "latency_probe_latest.json"

DRIFT_FACTOR = 1.10
BUDGET_P95_MULT = 1.5
BUDGET_MAX_MULT = 1.2


def compute_budget_p95_ms(recorded_p95_ms: float, recorded_max_ms: float) -> float:
    """budget_p95_ms = max(recorded_p95 * 1.5, recorded_max * 1.2). Never min()."""
    return max(recorded_p95_ms * BUDGET_P95_MULT, recorded_max_ms * BUDGET_MAX_MULT)


def normalize_machine_class(raw: str | None) -> str:
    if not raw:
        return ""
    s = str(raw).strip().lower()
    if s in ("darwin", "macos", "osx", "mac", "macos-latest"):
        return "macos"
    if s in ("linux", "ubuntu", "ubuntu-latest", "debian"):
        return "linux"
    if s in ("windows", "win32", "win", "windows-latest"):
        return "windows"
    return s


def detect_machine_class() -> str:
    return normalize_machine_class(platform.system())


def load_json(path: Path) -> dict[str, Any]:
    return json.loads(path.read_text(encoding="utf-8"))


def live_summary(live: dict[str, Any]) -> dict[str, Any]:
    return live.get("summary") or live


def evaluate_rules(
    live_p95_ms: float,
    budget_p95_ms: float,
    prev_commit_p95_ms: float | None,
    same_machine_class: bool,
) -> list[tuple[str, bool, str]]:
    """Return [(name, passed, detail), ...] for the two independent rules.

    Rule 2 is omitted (not failed) when there is no prev sample on this class.
    Callers must not wrap the two ceilings in min().
    """
    results: list[tuple[str, bool, str]] = []
    results.append(
        (
            "absolute_ceiling",
            live_p95_ms <= budget_p95_ms,
            f"live_p95={live_p95_ms:.3f} budget_p95={budget_p95_ms:.3f}",
        )
    )
    if same_machine_class and prev_commit_p95_ms is not None:
        limit = prev_commit_p95_ms * DRIFT_FACTOR
        results.append(
            (
                "same_class_drift",
                live_p95_ms <= limit,
                f"live_p95={live_p95_ms:.3f} prev_commit_p95={prev_commit_p95_ms:.3f} "
                f"limit={limit:.3f} (x{DRIFT_FACTOR})",
            )
        )
    else:
        results.append(
            (
                "same_class_drift",
                True,
                "skipped (first sample on this machine class or class mismatch); rule 1 only",
            )
        )
    return results


def validate_freeze_schema(freeze: dict[str, Any]) -> list[str]:
    errors: list[str] = []
    for key in (
        "recorded_p50_ms",
        "recorded_p95_ms",
        "recorded_max_ms",
        "n_docs",
        "budget_p95_ms",
    ):
        if freeze.get(key) is None:
            errors.append(f"missing {key}")
    hw = freeze.get("hardware") or {}
    if not hw.get("runner_os"):
        errors.append("hardware.runner_os missing")
    if not hw.get("note"):
        errors.append("hardware.note missing")
    n_docs = freeze.get("n_docs")
    if n_docs is not None and int(n_docs) != 8:
        errors.append(f"n_docs={n_docs} expected 8")
    rec_p95 = freeze.get("recorded_p95_ms")
    rec_max = freeze.get("recorded_max_ms")
    budget = freeze.get("budget_p95_ms")
    if rec_p95 is not None and rec_max is not None and budget is not None:
        expected = compute_budget_p95_ms(float(rec_p95), float(rec_max))
        if abs(float(budget) - expected) > 1e-6:
            errors.append(
                f"budget_p95_ms={budget} != max(p95*1.5, max*1.2)={expected}"
            )
    if freeze.get("enable_full_page_render") is not False:
        errors.append("enable_full_page_render must be false (Fast never renders)")
    if freeze.get("allow_auto_render") is not False:
        errors.append("allow_auto_render must be false (Fast never renders)")
    rules = freeze.get("rules") or {}
    if rules.get("do_not_wrap_in_min") is False:
        errors.append("rules.do_not_wrap_in_min must not be false")
    if rules.get("do_not_gate_vs_freeze_recorded_p95_x_1_10") is False:
        errors.append("must not gate vs recorded_p95 * 1.10")
    return errors


def check_fast_never_renders(summary: dict[str, Any]) -> list[str]:
    errors: list[str] = []
    if summary.get("preset") not in (None, "fast"):
        errors.append(f"preset={summary.get('preset')!r} is not fast")
    if summary.get("enable_full_page_render") is not False:
        errors.append(
            f"enable_full_page_render={summary.get('enable_full_page_render')!r} "
            "(Fast must never full-page-render)"
        )
    if summary.get("allow_auto_render") is not False:
        errors.append(
            f"allow_auto_render={summary.get('allow_auto_render')!r} "
            "(Fast must never opportunistic-render)"
        )
    args = summary.get("cli_args") or []
    joined = " ".join(str(a) for a in args).lower()
    if "highquality" in joined or "high-quality" in joined:
        errors.append("cli_args request HighQuality (would full-page-render)")
    if "--table-preset" in joined and "fast" not in joined:
        errors.append(f"cli_args not Fast: {args}")
    return errors


def prev_p95_from_file(path: Path | None, live_class: str) -> float | None:
    if path is None or not path.is_file():
        return None
    prev = load_json(path)
    prev_class = normalize_machine_class(
        prev.get("machine_class") or (prev.get("hardware") or {}).get("machine_class")
    )
    if prev_class and live_class and prev_class != live_class:
        return None
    raw = prev.get("prev_commit_p95_ms")
    if raw is None:
        raw = prev.get("p95_ms")
    if raw is None:
        raw = (prev.get("summary") or {}).get("p95_ms")
    if raw is None:
        return None
    return float(raw)


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--freeze", type=Path, default=DEFAULT_FREEZE)
    ap.add_argument("--live", type=Path, default=DEFAULT_LIVE)
    ap.add_argument(
        "--machine-class",
        default=None,
        help="Normalized class (linux/macos/windows). Default: this host.",
    )
    ap.add_argument(
        "--prev-json",
        type=Path,
        default=None,
        help="Previous sample on this machine class (prev_commit_p95_ms).",
    )
    ap.add_argument(
        "--prev-p95",
        type=float,
        default=None,
        help="Override previous-commit p95 ms (same class).",
    )
    ap.add_argument(
        "--write-prev",
        type=Path,
        default=None,
        help="Write this run's p95 as prev_commit snapshot for the next nightly.",
    )
    args = ap.parse_args(argv)

    if not args.freeze.is_file():
        print(f"missing freeze: {args.freeze}", file=sys.stderr)
        return 2
    if not args.live.is_file():
        print(f"missing live probe JSON: {args.live}", file=sys.stderr)
        return 2

    freeze = load_json(args.freeze)
    live = load_json(args.live)
    summary = live_summary(live)

    schema_errs = validate_freeze_schema(freeze)
    if schema_errs:
        for e in schema_errs:
            print(f"  [FAIL] freeze schema — {e}")
        return 1

    render_errs = check_fast_never_renders(summary)
    live_p95 = summary.get("p95_ms")
    if live_p95 is None:
        render_errs.append("live summary missing p95_ms")

    live_class = normalize_machine_class(args.machine_class) or detect_machine_class()
    freeze_class = normalize_machine_class(
        (freeze.get("hardware") or {}).get("machine_class")
        or (freeze.get("hardware") or {}).get("runner_os")
    )

    prev_p95 = args.prev_p95
    if prev_p95 is None:
        prev_p95 = prev_p95_from_file(args.prev_json, live_class)

    # Drift is vs previous sample on THIS class, not vs freeze hardware class.
    # First sample on a class (no prev file / class mismatch) => rule 2 skipped.
    # Nightly ubuntu vs local-macos freeze is not a drift pair.
    drift_same_class = prev_p95 is not None

    print("=== latency_fast_v0 freeze ===")
    print(f"  freeze: {args.freeze}")
    print(f"  live:   {args.live}")
    print(f"  live machine_class={live_class or '?'} freeze_class={freeze_class or '?'}")
    print(f"  prev_commit_p95_ms={prev_p95}")
    print(
        "  note: do not gate vs freeze recorded_p95 * 1.10; "
        "do not wrap ceilings in min()"
    )

    failed = False
    for e in render_errs:
        print(f"  [FAIL] Fast never render — {e}")
        failed = True
    if not render_errs:
        print("  [PASS] Fast never full-page-render (flags + preset)")

    if live_p95 is None:
        print("RESULT: FAIL")
        return 1

    budget = float(freeze["budget_p95_ms"])
    for name, passed, detail in evaluate_rules(
        float(live_p95),
        budget,
        prev_p95,
        same_machine_class=drift_same_class,
    ):
        status = "PASS" if passed else "FAIL"
        print(f"  [{status}] {name} — {detail}")
        if not passed:
            failed = True

    # Explicitly refuse the rejected rule so a future min() wrap cannot hide.
    rec = float(freeze["recorded_p95_ms"])
    rejected_ceiling = rec * DRIFT_FACTOR
    print(
        f"  [INFO] freeze recorded_p95*1.10={rejected_ceiling:.3f} "
        f"is NOT a gate (live_p95={float(live_p95):.3f})"
    )

    if args.write_prev:
        args.write_prev.parent.mkdir(parents=True, exist_ok=True)
        payload = {
            "machine_class": live_class,
            "prev_commit_p95_ms": float(live_p95),
            "p50_ms": summary.get("p50_ms"),
            "max_ms": summary.get("max_ms"),
            "n_docs": summary.get("n_docs") or freeze.get("n_docs"),
            "preset": "fast",
            "enable_full_page_render": False,
            "allow_auto_render": False,
        }
        args.write_prev.write_text(json.dumps(payload, indent=2) + "\n", encoding="utf-8")
        print(f"  wrote prev snapshot {args.write_prev}")

    print("RESULT:", "FAIL" if failed else "PASS")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
