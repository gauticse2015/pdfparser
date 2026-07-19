#!/usr/bin/env python3
"""Strict FP smoke (tol=0, first page)."""
from __future__ import annotations

import argparse
import json
import subprocess
import sys
from datetime import datetime, timezone
from pathlib import Path

SCRIPTS = Path(__file__).resolve().parent
BENCH = SCRIPTS.parent
REPO = BENCH.parent
sys.path.insert(0, str(SCRIPTS))
from run_real_detect_smoke import find_binary, try_build_release, resolve_pdf  # noqa: E402

MANIFEST = BENCH / "real_track" / "manifests" / "real_fp_smoke_v1_strict.json"
OUT = BENCH / "real_track" / "results" / "real_fp_strict_latest.json"


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--build", action="store_true")
    ap.add_argument("--strict", action="store_true")
    ap.add_argument("--limit", type=int, default=None)
    args = ap.parse_args()
    man = json.loads(MANIFEST.read_text())
    docs = man["documents"]
    binary = find_binary()
    if binary is None or args.build:
        ok, msg = try_build_release()
        print(msg)
        if not ok:
            return 2
        binary = find_binary()
    if binary is None:
        return 2
    out_docs = []
    n_pass = 0
    for i, d in enumerate(docs):
        if args.limit is not None and i >= args.limit:
            break
        pdf = resolve_pdf(d["pdf"])
        exp = int(d.get("expected_table_count") or 0)
        tol = int(d.get("expected_table_count_tol") or 0)
        entry = {"id": d["id"], "n_exp": exp, "tol": tol}
        if not pdf.is_file():
            entry["status"] = "missing_pdf"
            out_docs.append(entry)
            continue
        proc = subprocess.run(
            [str(binary), "extract", "--tables", "--format", "json", "--no-stitch",
             "--page-tables", "--table-preset", "auto", "--pages", "1", str(pdf)],
            capture_output=True, text=True, timeout=180, cwd=str(REPO),
        )
        if proc.returncode != 0:
            entry["status"] = "extract_error"
            entry["error"] = (proc.stderr or "")[:200]
            out_docs.append(entry)
            continue
        payload = json.loads(proc.stdout, strict=False)
        n = 0
        for p in payload.get("pages") or []:
            n += len(p.get("tables") or [])
        if n == 0:
            n = len(payload.get("tables") or [])
        entry["n_pred"] = n
        ok = abs(n - exp) <= tol
        entry["pass"] = ok
        entry["status"] = "ok" if ok else "fail"
        if ok:
            n_pass += 1
        print(f"  {d['id']}: pred={n} exp={exp}±{tol} {'PASS' if ok else 'FAIL'}")
        out_docs.append(entry)
    summary = {
        "n_docs": len(out_docs),
        "n_pass": n_pass,
        "n_fail": len(out_docs) - n_pass,
        "fp_zero_rate": n_pass / max(len(out_docs), 1),
        "pass_rate": n_pass / max(len(out_docs), 1),
    }
    OUT.parent.mkdir(parents=True, exist_ok=True)
    OUT.write_text(json.dumps({
        "suite": "real_fp_smoke_strict",
        "generated_at": datetime.now(timezone.utc).isoformat(),
        "summary": summary,
        "documents": out_docs,
    }, indent=2) + "\n")
    print(f"wrote {OUT} zero_rate={summary['fp_zero_rate']:.3f}")
    if args.strict and summary["n_fail"]:
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
