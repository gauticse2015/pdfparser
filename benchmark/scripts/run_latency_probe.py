#!/usr/bin/env python3
"""Latency probe for TablePreset::Fast (GATE-5 G5.6).

Times `pdfparser extract --tables --table-preset fast --no-stitch --page-tables`
on docs listed in real_track/manifests/latency_probe.json.

Writes real_track/results/latency_probe_latest.json with p50/p95 ms.
Budget is informative (default 30000ms p95) until rebaseline.
"""
from __future__ import annotations

import json
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


def main() -> int:
    if not BIN.is_file():
        print("missing release binary; run: cargo build --release -p pdfparser-cli", file=sys.stderr)
        return 2
    man = json.loads(MAN.read_text(encoding="utf-8"))
    docs = man.get("documents") or []
    times_ms = []
    per = []
    for doc_id in docs:
        pdf = find_pdf(doc_id)
        if not pdf:
            per.append({"id": doc_id, "error": "pdf_missing"})
            continue
        t0 = time.perf_counter()
        r = subprocess.run(
            [
                str(BIN),
                "extract",
                str(pdf),
                "--tables",
                "--table-preset",
                "fast",
                "--no-stitch",
                "--page-tables",
                "--format",
                "json",
            ],
            capture_output=True,
            text=True,
        )
        dt = (time.perf_counter() - t0) * 1000.0
        if r.returncode != 0:
            per.append({"id": doc_id, "error": r.stderr[-400:], "ms": dt})
            continue
        times_ms.append(dt)
        # Confirm Fast flags if dump present — not required
        per.append({"id": doc_id, "ms": dt, "ok": True})
        print(f"  {doc_id}: {dt:.1f} ms")

    if not times_ms:
        print("no successful latency samples", file=sys.stderr)
        return 1
    times_ms_sorted = sorted(times_ms)
    def pct(p):
        if not times_ms_sorted:
            return None
        k = (len(times_ms_sorted) - 1) * p / 100.0
        f = int(k)
        c = min(f + 1, len(times_ms_sorted) - 1)
        if f == c:
            return times_ms_sorted[f]
        return times_ms_sorted[f] + (times_ms_sorted[c] - times_ms_sorted[f]) * (k - f)

    summary = {
        "preset": "fast",
        "n_ok": len(times_ms),
        "n_docs": len(docs),
        "p50_ms": pct(50),
        "p95_ms": pct(95),
        "mean_ms": statistics.mean(times_ms),
        "max_ms": max(times_ms),
        "budget_p95_ms": float(man.get("budget_p95_ms") or 30_000),
        "enable_full_page_render": False,
        "allow_auto_render": False,
        "note": "Fast preset hard-disables full-page render (G5.6).",
    }
    out = {
        "generated_at": datetime.now(timezone.utc).isoformat(),
        "summary": summary,
        "documents": per,
    }
    OUT.parent.mkdir(parents=True, exist_ok=True)
    OUT.write_text(json.dumps(out, indent=2) + "\n", encoding="utf-8")
    print(
        f"wrote {OUT} p50={summary['p50_ms']:.1f} p95={summary['p95_ms']:.1f} "
        f"budget={summary['budget_p95_ms']}"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
