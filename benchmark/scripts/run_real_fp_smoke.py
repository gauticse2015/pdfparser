#!/usr/bin/env python3
"""real_fp_smoke — false-positive table detection on forms/prose/chrome.

Expects expected_table_count near 0 with tolerance. Uses same CLI extract path as
detect smoke (product Auto). Not structure F1.
"""
from __future__ import annotations

import argparse
import json
import subprocess
import sys
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Optional

SCRIPTS = Path(__file__).resolve().parent
BENCH = SCRIPTS.parent
REPO = BENCH.parent
sys.path.insert(0, str(SCRIPTS))

from run_real_detect_smoke import (  # noqa: E402
    find_binary,
    try_build_release,
    resolve_pdf,
    count_tables_from_payload,
)

MANIFEST = BENCH / "real_track" / "manifests" / "real_fp_smoke_v0.json"
OUT = BENCH / "real_track" / "results" / "real_fp_smoke_latest.json"


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--dry-run", action="store_true")
    ap.add_argument("--build", action="store_true")
    ap.add_argument("--strict", action="store_true")
    ap.add_argument("--limit", type=int, default=None)
    args = ap.parse_args()
    man = json.loads(MANIFEST.read_text(encoding="utf-8"))
    docs = man.get("documents") or []
    print(f"real_fp_smoke n_docs={len(docs)}")
    if args.dry_run:
        for d in docs[: args.limit or len(docs)]:
            pdf = resolve_pdf(d["pdf"])
            print(f"  {d['id']}: pdf={'OK' if pdf.is_file() else 'MISS'} exp={d.get('expected_table_count')}±{d.get('expected_table_count_tol',0)}")
        print("dry-run OK")
        return 0
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
        entry: dict[str, Any] = {"id": d["id"], "pdf": d["pdf"], "n_exp": exp, "tol": tol}
        if not pdf.is_file():
            entry["status"] = "missing_pdf"
            out_docs.append(entry)
            continue
        # First-page only: multipage specs (NIST AES etc.) have real tables later;
        # FP smoke measures chrome/prose over-detect on page 1.
        proc = subprocess.run(
            [
                str(binary), "extract", "--tables", "--format", "json",
                "--no-stitch", "--page-tables", "--pages", "1", str(pdf),
            ],
            capture_output=True, text=True, timeout=180, cwd=str(REPO),
        )
        if proc.returncode != 0:
            entry["status"] = "extract_error"
            entry["error"] = (proc.stderr or "")[:300]
            out_docs.append(entry)
            continue
        payload = json.loads(proc.stdout)
        n_pred, src, mix, _ = count_tables_from_payload(payload)
        entry["n_pred"] = n_pred
        entry["source"] = src
        entry["method_mix"] = mix[:12]
        ok = abs(n_pred - exp) <= tol
        entry["status"] = "ok" if ok else "over_fp"
        entry["pass"] = ok
        if ok:
            n_pass += 1
        out_docs.append(entry)
        print(f"  {d['id']}: n_pred={n_pred} exp={exp}±{tol} {'PASS' if ok else 'FAIL'}")
    summary = {
        "n_docs": len(out_docs),
        "n_pass": n_pass,
        "n_fail": len(out_docs) - n_pass,
        "pass_rate": n_pass / max(len(out_docs), 1),
    }
    result = {
        "suite": "real_fp_smoke",
        "generated_at": datetime.now(timezone.utc).isoformat(),
        "policy": man.get("policy"),
        "summary": summary,
        "documents": out_docs,
    }
    OUT.parent.mkdir(parents=True, exist_ok=True)
    OUT.write_text(json.dumps(result, indent=2), encoding="utf-8")
    print(f"wrote {OUT} pass_rate={summary['pass_rate']:.2f}")
    if args.strict and summary["n_fail"]:
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
