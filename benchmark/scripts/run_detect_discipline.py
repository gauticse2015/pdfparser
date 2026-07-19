#!/usr/bin/env python3
"""Detect discipline suite — strict table COUNT gates (Phase 0/1).

Reads manifests/detect_discipline_v1.json.
CLI: pdfparser extract --tables --no-stitch --page-tables --format json
Page scope per doc: first_page | gold_pages | all_pages.
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

from run_real_detect_smoke import find_binary, try_build_release, resolve_pdf  # noqa: E402

MANIFEST_DEFAULT = BENCH / "real_track" / "manifests" / "detect_discipline_v1.json"
OUT_DEFAULT = BENCH / "real_track" / "results" / "detect_discipline_latest.json"


def load_gold(doc: dict) -> dict:
    gpath = doc.get("gold")
    if not gpath:
        return {
            "expected_table_count": doc.get("expected_table_count"),
            "page_scope": doc.get("page_scope", "all_pages"),
            "gold_pages": doc.get("gold_pages"),
            "failure_families": doc.get("failure_families") or [],
        }
    for base in (BENCH / "real_track", BENCH, REPO):
        p = base / gpath
        if p.is_file():
            return json.loads(p.read_text(encoding="utf-8"))
        p2 = BENCH / "real_track" / gpath
        if p2.is_file():
            return json.loads(p2.read_text(encoding="utf-8"))
    return {
        "expected_table_count": doc.get("expected_table_count"),
        "page_scope": doc.get("page_scope"),
        "gold_pages": doc.get("gold_pages"),
    }


def collect_tables(payload: dict) -> list[dict]:
    out: list[dict] = []
    pages = payload.get("pages") or []
    if pages:
        for i, page in enumerate(pages):
            if not isinstance(page, dict):
                continue
            pidx = int(page.get("index", i))
            for t in page.get("tables") or []:
                if isinstance(t, dict):
                    tt = dict(t)
                    tt.setdefault("page", pidx)
                    out.append(tt)
        if out:
            return out
    for t in payload.get("tables") or []:
        if isinstance(t, dict):
            out.append(t)
    return out


def filter_by_scope(tables: list[dict], gold: dict, doc: dict) -> tuple[list[dict], dict]:
    scope = gold.get("page_scope") or doc.get("page_scope") or "all_pages"
    pages = gold.get("gold_pages") or doc.get("gold_pages")
    meta = {"scope": scope, "pages": pages, "n_all": len(tables)}
    if scope == "all_pages":
        return tables, meta
    if scope == "first_page":
        pages = [0]
    if scope == "gold_pages":
        if not pages:
            # infer from structure gold
            sid = gold.get("id") or doc.get("id")
            sg = BENCH / "real_track" / "gold" / f"{sid}.json"
            if sg.is_file():
                g = json.loads(sg.read_text())
                pages = sorted({int(t.get("page", 0)) for t in g.get("expected_tables") or []}) or [0]
            else:
                pages = [0]
    pages_set = set(int(p) for p in (pages or [0]))
    meta["pages"] = sorted(pages_set)
    filtered = []
    for t in tables:
        p = t.get("page")
        if p is None or int(p) in pages_set:
            filtered.append(t)
    meta["n_filtered"] = len(filtered)
    return filtered, meta


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--manifest", type=Path, default=MANIFEST_DEFAULT)
    ap.add_argument("--out", type=Path, default=OUT_DEFAULT)
    ap.add_argument("--build", action="store_true")
    ap.add_argument("--strict", action="store_true")
    ap.add_argument("--limit", type=int, default=None)
    ap.add_argument("--dry-run", action="store_true")
    args = ap.parse_args()

    man = json.loads(args.manifest.read_text(encoding="utf-8"))
    docs = man.get("documents") or []
    print(f"detect_discipline n_docs={len(docs)}")
    if args.dry_run:
        for d in docs[: args.limit or len(docs)]:
            pdf = resolve_pdf(d["pdf"])
            print(f"  {d['id']}: pdf={'OK' if pdf.is_file() else 'MISS'} exp={d.get('expected_table_count')} scope={d.get('page_scope')}")
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
    n_exact = n_over = n_under = 0
    sum_pred = sum_exp = 0
    for i, d in enumerate(docs):
        if args.limit is not None and i >= args.limit:
            break
        gold = load_gold(d)
        exp = int(gold.get("expected_table_count") if gold.get("expected_table_count") is not None else d.get("expected_table_count") or 0)
        pdf = resolve_pdf(d["pdf"])
        entry: dict[str, Any] = {
            "id": d["id"],
            "pdf": d["pdf"],
            "n_exp": exp,
            "failure_families": gold.get("failure_families") or d.get("failure_families") or [],
            "bucket": d.get("bucket"),
        }
        if not pdf.is_file():
            entry["status"] = "missing_pdf"
            out_docs.append(entry)
            print(f"  {d['id']}: MISSING")
            continue
        proc = subprocess.run(
            [
                str(binary), "extract", "--format", "json", "--tables",
                "--table-preset", "auto", "--no-stitch", "--page-tables", str(pdf),
            ],
            capture_output=True, text=True, timeout=180, cwd=str(REPO),
        )
        if proc.returncode != 0:
            entry["status"] = "extract_error"
            entry["error"] = (proc.stderr or "")[:300]
            out_docs.append(entry)
            print(f"  {d['id']}: ERR")
            continue
        try:
            payload = json.loads(proc.stdout, strict=False)
        except json.JSONDecodeError as e:
            entry["status"] = "json_error"
            entry["error"] = str(e)
            out_docs.append(entry)
            continue
        tables = collect_tables(payload)
        tables, scope_meta = filter_by_scope(tables, gold, d)
        n_pred = len(tables)
        methods = {}
        for t in tables:
            m = t.get("method") or "?"
            methods[m] = methods.get(m, 0) + 1
        entry.update({
            "n_pred": n_pred,
            "exact": n_pred == exp,
            "over": n_pred > exp,
            "under": n_pred < exp,
            "delta": n_pred - exp,
            "method_mix": methods,
            "scope_meta": scope_meta,
            "status": "ok",
        })
        sum_pred += n_pred
        sum_exp += exp
        if n_pred == exp:
            n_exact += 1
        elif n_pred > exp:
            n_over += 1
        else:
            n_under += 1
        flag = "OK" if n_pred == exp else ("OVER" if n_pred > exp else "UNDER")
        print(f"  {d['id']}: pred={n_pred} exp={exp} {flag} mix={methods}")
        out_docs.append(entry)

    n = len([d for d in out_docs if d.get("status") == "ok"])
    summary = {
        "n_docs": len(out_docs),
        "n_scored": n,
        "exact_count_rate": n_exact / max(n, 1),
        "over_doc_rate": n_over / max(n, 1),
        "under_doc_rate": n_under / max(n, 1),
        "n_exact": n_exact,
        "n_over": n_over,
        "n_under": n_under,
        "sum_pred": sum_pred,
        "sum_exp": sum_exp,
        "pred_gt_ratio": (sum_pred / sum_exp) if sum_exp else None,
        "n_severe_over": sum(1 for d in out_docs if d.get("status") == "ok" and d.get("delta", 0) >= 3),
    }
    result = {
        "suite": "detect_discipline",
        "version": 1,
        "generated_at": datetime.now(timezone.utc).isoformat(),
        "binary": str(binary),
        "summary": summary,
        "documents": out_docs,
    }
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
    print(
        f"wrote {args.out} exact={summary['exact_count_rate']:.3f} "
        f"over={summary['over_doc_rate']:.3f} ratio={summary['pred_gt_ratio']}"
    )
    if args.strict and (n_exact < n or summary["n_severe_over"]):
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
