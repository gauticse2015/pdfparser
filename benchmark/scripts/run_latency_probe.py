#!/usr/bin/env python3
"""Latency probe for TablePreset::Fast (G5.6 / P0.5).

Times `pdfparser extract --tables --table-preset fast --no-stitch --page-tables`
on docs listed in real_track/manifests/latency_probe.json.

Writes real_track/results/latency_probe_latest.json with p50/p95 ms.
budget_p95_ms here is informational (default 30000). Freeze ceiling + fail
rules live in real_track/freezes/latency_fast_v0.json. Fast never full-page-renders.
"""
from __future__ import annotations

import argparse
import json
import os
import platform
import statistics
import subprocess
import sys
import time
from datetime import datetime, timezone
from pathlib import Path

BENCH = Path(__file__).resolve().parents[1]
REPO = BENCH.parent
RT = BENCH / "real_track"
MAN = RT / "manifests" / "latency_probe.json"
OUT = RT / "results" / "latency_probe_latest.json"
BIN = REPO / "target" / "release" / "pdfparser"

# Informational only until an ubuntu-latest sample exists (P0.5). Do not treat
# this 30s figure as the freeze ceiling — see freezes/latency_fast_v0.json.
INFORMATIONAL_BUDGET_P95_MS = 30_000.0

# Map probe doc ids to PDF paths under corpus/
PDF_CANDIDATES = [
    BENCH / "corpus" / "real",
    BENCH / "corpus" / "compete_real",
]


def find_pdf(doc_id: str) -> Path | None:
    for root in PDF_CANDIDATES:
        p = root / f"{doc_id}.pdf"
        if p.is_file():
            return p
    return None


def hardware_block() -> dict:
    runner_os = os.environ.get("RUNNER_OS") or platform.system()
    note = os.environ.get("LATENCY_HARDWARE_NOTE") or (
        f"{platform.platform()} {platform.machine()}"
    )
    return {
        "runner_os": runner_os,
        "machine_class": (os.environ.get("LATENCY_MACHINE_CLASS") or runner_os).lower(),
        "machine": platform.machine(),
        "platform": platform.platform(),
        "note": note,
    }


def fast_cli(pdf: Path, binary: Path = BIN, dump_evidence: bool = False) -> list[str]:
    cmd = [
        str(binary),
        "extract",
        str(pdf),
        "--tables",
        "--table-preset",
        "fast",
        "--no-stitch",
        "--page-tables",
        "--format",
        "json",
    ]
    if dump_evidence:
        cmd.append("--dump-evidence")
    return cmd


def parse_dump_evidence_flags(stderr: str) -> dict | None:
    text = (stderr or "").strip()
    if not text:
        return None
    try:
        return json.loads(text)
    except json.JSONDecodeError:
        start = text.find("{")
        end = text.rfind("}")
        if start < 0 or end <= start:
            return None
        try:
            return json.loads(text[start : end + 1])
        except json.JSONDecodeError:
            return None


def verify_fast_never_renders(
    pdf: Path, binary: Path = BIN
) -> tuple[bool, bool | None, bool | None, str]:
    """Untimed dump-evidence probe. Fast must hard-disable both render flags."""
    r = subprocess.run(
        fast_cli(pdf, binary=binary, dump_evidence=True),
        capture_output=True,
        text=True,
    )
    if r.returncode != 0:
        return False, None, None, f"dump-evidence exit={r.returncode} {r.stderr[-300:]}"
    ev = parse_dump_evidence_flags(r.stderr)
    if not ev:
        return False, None, None, "dump-evidence stderr was not JSON"
    enable = ev.get("enable_full_page_render")
    allow = ev.get("allow_auto_render")
    preset = str(ev.get("preset") or "").lower()
    ok = enable is False and allow is False and preset in ("fast", "")
    return ok, enable, allow, f"preset={ev.get('preset')!r}"


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument(
        "--out",
        type=Path,
        default=OUT,
        help="Write probe JSON here (default: real_track/results/latency_probe_latest.json)",
    )
    ap.add_argument(
        "--binary",
        type=Path,
        default=None,
        help="pdfparser binary (default: discover target/release/pdfparser)",
    )
    args = ap.parse_args()
    binary = args.binary.expanduser() if args.binary is not None else BIN
    if not binary.is_file():
        print(
            f"missing binary: {binary}; run: cargo build --release -p pdfparser-cli",
            file=sys.stderr,
        )
        return 2
    man = json.loads(MAN.read_text(encoding="utf-8"))
    docs = man.get("documents") or []
    times_ms = []
    per = []
    first_ok_pdf: Path | None = None
    for doc_id in docs:
        pdf = find_pdf(doc_id)
        if not pdf:
            per.append({"id": doc_id, "error": "pdf_missing"})
            print(f"  {doc_id}: MISSING", file=sys.stderr)
            continue
        t0 = time.perf_counter()
        r = subprocess.run(
            fast_cli(pdf, binary=binary, dump_evidence=False),
            capture_output=True,
            text=True,
        )
        dt = (time.perf_counter() - t0) * 1000.0
        if r.returncode != 0:
            per.append({"id": doc_id, "error": r.stderr[-400:], "ms": dt})
            print(f"  {doc_id}: FAIL rc={r.returncode} ({dt:.1f} ms)", file=sys.stderr)
            continue
        times_ms.append(dt)
        if first_ok_pdf is None:
            first_ok_pdf = pdf
        per.append({"id": doc_id, "ms": dt, "ok": True})
        print(f"  {doc_id}: {dt:.1f} ms")

    n_docs = len(docs)
    n_ok = len(times_ms)
    incomplete = n_ok != n_docs or any(d.get("error") for d in per)

    def pct(p):
        if not times_ms:
            return None
        times_ms_sorted = sorted(times_ms)
        k = (len(times_ms_sorted) - 1) * p / 100.0
        f = int(k)
        c = min(f + 1, len(times_ms_sorted) - 1)
        if f == c:
            return times_ms_sorted[f]
        return times_ms_sorted[f] + (times_ms_sorted[c] - times_ms_sorted[f]) * (k - f)

    enable_flag: bool | None = False
    allow_flag: bool | None = False
    render_ok = False
    render_detail = "no successful pdf for dump-evidence"
    if first_ok_pdf is not None:
        render_ok, enable_flag, allow_flag, render_detail = verify_fast_never_renders(
            first_ok_pdf, binary=binary
        )
        print(f"  fast_never_render: ok={render_ok} {render_detail}")

    info_budget = float(man.get("budget_p95_ms") or INFORMATIONAL_BUDGET_P95_MS)
    summary = {
        "preset": "fast",
        "n_ok": n_ok,
        "n_docs": n_docs,
        "p50_ms": pct(50),
        "p95_ms": pct(95),
        "mean_ms": statistics.mean(times_ms) if times_ms else None,
        "max_ms": max(times_ms) if times_ms else None,
        "budget_p95_ms": info_budget,
        "budget_p95_informational": True,
        "enable_full_page_render": False if enable_flag is False else enable_flag,
        "allow_auto_render": False if allow_flag is False else allow_flag,
        "fast_never_render_verified": render_ok,
        "cli_preset": "fast",
        "cli_args": ["extract", "--tables", "--table-preset", "fast"],
        "note": (
            "Invoked TablePreset::Fast; Fast hard-disables full-page render (G5.6). "
            "budget_p95_ms here is informational (default 30000). Freeze ceiling is "
            "freezes/latency_fast_v0.json - do not claim 30s->0.6s tightening until "
            "ubuntu-latest sample exists. Incomplete n_ok!=n_docs is a hard fail."
        ),
    }
    out = {
        "generated_at": datetime.now(timezone.utc).isoformat(),
        "hardware": hardware_block(),
        "summary": summary,
        "documents": per,
    }
    out_path = args.out
    out_path.parent.mkdir(parents=True, exist_ok=True)
    out_path.write_text(json.dumps(out, indent=2) + "\n", encoding="utf-8")
    p95 = summary["p95_ms"]
    p95_s = f"{p95:.1f}" if isinstance(p95, (int, float)) else "None"
    print(
        f"wrote {out_path} n_ok={n_ok}/{n_docs} p95={p95_s} "
        f"informational_budget={summary['budget_p95_ms']}"
    )
    if incomplete:
        print(
            f"incomplete Fast probe: n_ok={n_ok} n_docs={n_docs} "
            "(every listed doc must succeed; crashing the slow doc must not green p95)",
            file=sys.stderr,
        )
        return 1
    if not render_ok:
        print(
            f"Fast preset must never full-page-render ({render_detail})",
            file=sys.stderr,
        )
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
