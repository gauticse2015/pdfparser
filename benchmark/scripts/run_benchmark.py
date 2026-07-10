#!/usr/bin/env python3
"""Competitive PDF parser benchmark harness.

Runs: pdfplumber, pdfminer.six, pypdf, PyMuPDF, pypdfium2
against corpus/*.pdf using ground_truth/*.json expectations.

Metrics:
  - success / error
  - wall_ms (open+extract)
  - peak_rss_delta_mb
  - page_count
  - text_chars
  - token_recall (must_contain)
  - reading_order_score (if applicable)
  - table_count_detected + cell token hits (where library supports tables)
  - image_count_detected (where supported)
  - form/link/outline probes (where supported)
  - encrypted open behavior
"""
from __future__ import annotations

import json
import re
import time
import traceback
from dataclasses import dataclass, asdict, field
from pathlib import Path
from typing import Any, Callable, Optional

import psutil

ROOT = Path(__file__).resolve().parents[1]
CORPUS = ROOT / "corpus"
GT_DIR = ROOT / "ground_truth"
RESULTS = ROOT / "results"
RESULTS.mkdir(parents=True, exist_ok=True)

proc = psutil.Process()


def rss_mb() -> float:
    return proc.memory_info().rss / (1024 * 1024)


def normalize_text(s: str) -> str:
    s = s.replace("\x00", " ")
    s = re.sub(r"[ \t]+", " ", s)
    s = re.sub(r"\n{3,}", "\n\n", s)
    return s


def token_recall(text: str, tokens: list[str]) -> dict[str, Any]:
    if not tokens:
        return {"recall": None, "hit": 0, "total": 0, "missing": [], "skipped": True}
    missing = [t for t in tokens if t not in text]
    hit = len(tokens) - len(missing)
    return {
        "recall": hit / len(tokens),
        "hit": hit,
        "total": len(tokens),
        "missing": missing,
        "skipped": False,
    }


def token_any(text: str, tokens: list[str]) -> dict[str, Any]:
    """At least one of the tokens should appear (for real PDFs with variable wording)."""
    if not tokens:
        return {"hit": True, "matched": None, "skipped": True}
    for t in tokens:
        if t in text:
            return {"hit": True, "matched": t, "skipped": False}
    return {"hit": False, "matched": None, "skipped": False, "missing": tokens}


def reading_order_score(text: str, ordered: list[str]) -> Optional[float]:
    if not ordered:
        return None
    positions = []
    for t in ordered:
        i = text.find(t)
        if i < 0:
            return 0.0
        positions.append(i)
    # score = fraction of adjacent pairs that are in order
    good = sum(1 for a, b in zip(positions, positions[1:]) if a < b)
    return good / max(len(positions) - 1, 1)


def cell_token_hits(tables: list[list[list[str]]], tokens: list[str]) -> dict[str, Any]:
    flat = []
    row_counts = []
    for table in tables:
        n = 0
        for row in table:
            for cell in row:
                if cell is None:
                    continue
                flat.append(str(cell))
                n += 1
        row_counts.append(len(table))
    blob = " | ".join(flat)
    base = {
        "table_count": len(tables),
        "cell_count": len(flat),
        "row_counts": row_counts,
        "max_rows": max(row_counts) if row_counts else 0,
        "avg_cell_len": (sum(len(x) for x in flat) / len(flat)) if flat else 0,
    }
    if not tokens:
        return {**base, "recall": None, "hit": 0, "total": 0, "missing": []}
    missing = [t for t in tokens if t not in blob]
    hit = len(tokens) - len(missing)
    return {
        **base,
        "recall": hit / len(tokens),
        "hit": hit,
        "total": len(tokens),
        "missing": missing,
    }


@dataclass
class ExtractResult:
    library: str
    doc_id: str
    success: bool
    error: Optional[str] = None
    wall_ms: float = 0.0
    rss_delta_mb: float = 0.0
    page_count: Optional[int] = None
    text: str = ""
    text_chars: int = 0
    token_recall: dict = field(default_factory=dict)
    reading_order_score: Optional[float] = None
    tables: list = field(default_factory=list)
    table_metrics: dict = field(default_factory=dict)
    image_count: Optional[int] = None
    form_fields: list = field(default_factory=list)
    links: list = field(default_factory=list)
    outline: list = field(default_factory=list)
    notes: list = field(default_factory=list)
    supports: dict = field(default_factory=dict)


# ─────────────────────────── adapters ───────────────────────────

def extract_pdfplumber(path: Path, gt: dict, password: Optional[str] = None) -> ExtractResult:
    import pdfplumber

    notes = []
    supports = {
        "text": True,
        "chars_geometry": True,
        "tables": True,
        "images_meta": True,
        "lines_rects": True,
        "annots_hyperlinks": True,
        "forms": True,  # limited
        "outline": False,
        "encrypted_password": True,
    }
    kwargs = {}
    if password:
        kwargs["password"] = password
    with pdfplumber.open(str(path), **kwargs) as pdf:
        pages = pdf.pages
        page_count = len(pages)
        texts = []
        tables_all = []
        image_count = 0
        links = []
        form_fields = []
        for page in pages:
            texts.append(page.extract_text() or "")
            # tables
            try:
                tabs = page.extract_tables() or []
                tables_all.extend(tabs)
            except Exception as e:
                notes.append(f"table_error:{e}")
            # images
            try:
                image_count += len(page.images or [])
            except Exception:
                pass
            # hyperlinks
            try:
                for hl in page.hyperlinks or []:
                    links.append(hl.get("uri") or hl.get("filename") or str(hl))
            except Exception:
                pass
            # annots (may include widgets)
            try:
                for a in page.annots or []:
                    if a.get("data"):
                        form_fields.append(str(a.get("data"))[:200])
            except Exception:
                pass
        text = normalize_text("\n".join(texts))
        # form values API if present
        try:
            # pdfplumber has experimental form extraction via page
            pass
        except Exception:
            pass
        return ExtractResult(
            library="pdfplumber",
            doc_id=gt["id"],
            success=True,
            page_count=page_count,
            text=text,
            text_chars=len(text),
            tables=tables_all,
            image_count=image_count,
            links=links,
            form_fields=form_fields,
            notes=notes,
            supports=supports,
        )


def extract_pdfminer(path: Path, gt: dict, password: Optional[str] = None) -> ExtractResult:
    from pdfminer.high_level import extract_text, extract_pages
    from pdfminer.layout import LTTextContainer, LTImage, LTFigure, LTAnno, LTChar
    from pdfminer.pdfparser import PDFParser
    from pdfminer.pdfdocument import PDFDocument
    from pdfminer.pdfpage import PDFPage
    from pdfminer.pdfinterp import PDFResourceManager, PDFPageInterpreter
    from pdfminer.converter import PDFPageAggregator
    from pdfminer.layout import LAParams

    supports = {
        "text": True,
        "chars_geometry": True,
        "tables": False,
        "images_meta": True,
        "lines_rects": True,
        "annots_hyperlinks": False,
        "forms": False,
        "outline": True,
        "encrypted_password": True,
    }
    notes = ["no_native_table_api"]
    laparams = LAParams()
    text = extract_text(str(path), password=password or "", laparams=laparams) or ""
    text = normalize_text(text)

    page_count = 0
    image_count = 0
    outline = []
    with open(path, "rb") as f:
        parser = PDFParser(f)
        try:
            doc = PDFDocument(parser, password=password or "")
        except Exception:
            doc = PDFDocument(parser)
        if doc.is_extractable is False:
            notes.append("not_extractable")
        try:
            outline_raw = doc.get_outlines()
            for level, title, dest, a, se in outline_raw:
                outline.append(title)
        except Exception as e:
            notes.append(f"outline:{type(e).__name__}")
        rsrc = PDFResourceManager()
        device = PDFPageAggregator(rsrc, laparams=laparams)
        interpreter = PDFPageInterpreter(rsrc, device)
        for page in PDFPage.create_pages(doc):
            page_count += 1
            interpreter.process_page(page)
            layout = device.get_result()
            for el in layout:
                if isinstance(el, LTFigure):
                    image_count += 1
                # count images inside figures loosely
                if isinstance(el, LTImage):
                    image_count += 1

    return ExtractResult(
        library="pdfminer.six",
        doc_id=gt["id"],
        success=True,
        page_count=page_count or None,
        text=text,
        text_chars=len(text),
        tables=[],
        image_count=image_count,
        outline=outline,
        notes=notes,
        supports=supports,
    )


def extract_pypdf(path: Path, gt: dict, password: Optional[str] = None) -> ExtractResult:
    from pypdf import PdfReader

    supports = {
        "text": True,
        "chars_geometry": False,
        "tables": False,
        "images_meta": True,
        "lines_rects": False,
        "annots_hyperlinks": True,
        "forms": True,
        "outline": True,
        "encrypted_password": True,
    }
    notes = ["no_native_table_api", "text_layout_limited"]
    reader = PdfReader(str(path))
    if reader.is_encrypted:
        if password:
            reader.decrypt(password)
        else:
            # try empty
            try:
                reader.decrypt("")
            except Exception:
                pass
    page_count = len(reader.pages)
    texts = []
    image_count = 0
    links = []
    for page in reader.pages:
        try:
            texts.append(page.extract_text() or "")
        except Exception as e:
            notes.append(f"text:{e}")
        try:
            if "/XObject" in page["/Resources"]:
                xobj = page["/Resources"]["/XObject"].get_object()
                for name in xobj:
                    o = xobj[name]
                    if o.get("/Subtype") == "/Image":
                        image_count += 1
        except Exception:
            pass
        try:
            if "/Annots" in page:
                for a in page["/Annots"]:
                    ann = a.get_object()
                    if ann.get("/Subtype") == "/Link":
                        action = ann.get("/A")
                        if action and action.get("/URI"):
                            links.append(str(action["/URI"]))
        except Exception:
            pass
    form_fields = []
    try:
        if reader.get_fields():
            for k, v in reader.get_fields().items():
                form_fields.append(k)
                if isinstance(v, dict) and v.get("/V") is not None:
                    form_fields.append(f"{k}={v.get('/V')}")
    except Exception as e:
        notes.append(f"forms:{e}")
    outline = []
    try:
        for item in reader.outline or []:
            if isinstance(item, list):
                continue
            title = getattr(item, "title", None) or (item.get("/Title") if isinstance(item, dict) else None)
            if title:
                outline.append(str(title))
    except Exception as e:
        notes.append(f"outline:{e}")

    text = normalize_text("\n".join(texts))
    return ExtractResult(
        library="pypdf",
        doc_id=gt["id"],
        success=True,
        page_count=page_count,
        text=text,
        text_chars=len(text),
        tables=[],
        image_count=image_count,
        form_fields=form_fields,
        links=links,
        outline=outline,
        notes=notes,
        supports=supports,
    )


def extract_pymupdf(path: Path, gt: dict, password: Optional[str] = None) -> ExtractResult:
    import fitz

    supports = {
        "text": True,
        "chars_geometry": True,
        "tables": True,  # find_tables in recent versions
        "images_meta": True,
        "lines_rects": True,
        "annots_hyperlinks": True,
        "forms": True,
        "outline": True,
        "encrypted_password": True,
    }
    notes = []
    doc = fitz.open(str(path))
    if doc.is_encrypted:
        if password:
            ok = doc.authenticate(password)
            if not ok:
                doc.close()
                raise RuntimeError("authenticate failed")
        else:
            # try empty
            doc.authenticate("")
    page_count = doc.page_count
    texts = []
    tables_all = []
    image_count = 0
    links = []
    form_fields = []
    for i in range(page_count):
        page = doc.load_page(i)
        texts.append(page.get_text("text") or "")
        try:
            image_count += len(page.get_images(full=True))
        except Exception:
            pass
        try:
            for l in page.get_links():
                if l.get("uri"):
                    links.append(l["uri"])
        except Exception:
            pass
        try:
            tabs = page.find_tables()
            if tabs and tabs.tables:
                for t in tabs.tables:
                    tables_all.append(t.extract())
        except Exception as e:
            notes.append(f"tables:{type(e).__name__}")
        try:
            for w in page.widgets() or []:
                form_fields.append(w.field_name or "widget")
                if w.field_value:
                    form_fields.append(f"{w.field_name}={w.field_value}")
        except Exception:
            pass
    outline = []
    try:
        toc = doc.get_toc()
        for lvl, title, _pno in toc:
            outline.append(title)
    except Exception:
        pass
    doc.close()
    text = normalize_text("\n".join(texts))
    return ExtractResult(
        library="pymupdf",
        doc_id=gt["id"],
        success=True,
        page_count=page_count,
        text=text,
        text_chars=len(text),
        tables=tables_all,
        image_count=image_count,
        form_fields=form_fields,
        links=links,
        outline=outline,
        notes=notes,
        supports=supports,
    )


def extract_pypdfium2(path: Path, gt: dict, password: Optional[str] = None) -> ExtractResult:
    import pypdfium2 as pdfium

    supports = {
        "text": True,
        "chars_geometry": True,
        "tables": False,
        "images_meta": False,
        "lines_rects": False,
        "annots_hyperlinks": False,
        "forms": False,
        "outline": False,
        "encrypted_password": True,
    }
    notes = ["pdfium_backend", "no_native_table_api", "limited_structure_api_in_adapter"]
    if password:
        pdf = pdfium.PdfDocument(str(path), password=password)
    else:
        pdf = pdfium.PdfDocument(str(path))
    page_count = len(pdf)
    texts = []
    for i in range(page_count):
        page = pdf[i]
        textpage = page.get_textpage()
        texts.append(textpage.get_text_bounded() or "")
        textpage.close()
        page.close()
    pdf.close()
    text = normalize_text("\n".join(texts))
    return ExtractResult(
        library="pypdfium2",
        doc_id=gt["id"],
        success=True,
        page_count=page_count,
        text=text,
        text_chars=len(text),
        tables=[],
        image_count=None,
        notes=notes,
        supports=supports,
    )




def extract_pdfparser(path: Path, gt: dict, password: Optional[str] = None) -> ExtractResult:
    """Native pdfparser CLI adapter (Phase T text + Phase V tables)."""
    import json
    import subprocess
    import shutil

    supports = {
        "text": True,
        "chars_geometry": True,
        "tables": True,
        "images_meta": True,
        "lines_rects": False,
        "annots_hyperlinks": True,
        "forms": True,
        "outline": True,
        "encrypted_password": False,
    }
    notes = ["phase_v_tables_phase_o_objects"]
    root = ROOT.parent
    bin_path = root / "target" / "release" / "pdfparser"
    if not bin_path.exists():
        bin_path = root / "target" / "debug" / "pdfparser"
    if not bin_path.exists():
        which = shutil.which("pdfparser")
        if which:
            bin_path = Path(which)
    if not bin_path.exists():
        raise RuntimeError(
            "pdfparser binary not found; build with: cargo build --release -p pdfparser-cli"
        )

    if gt.get("category") == "encrypted_password":
        # Phase T/U: no decrypt
        if password:
            raise RuntimeError("password decrypt not in Phase U")
        proc = subprocess.run(
            [str(bin_path), "extract", str(path)],
            capture_output=True,
            text=True,
            timeout=120,
        )
        if proc.returncode != 0:
            raise RuntimeError(proc.stderr.strip() or "encryption rejected")
        text = proc.stdout or ""
        return ExtractResult(
            library="pdfparser",
            doc_id=gt["id"],
            success=True,
            text=normalize_text(text),
            text_chars=len(text),
            tables=[],
            notes=notes + ["encrypted_open_unexpected"],
            supports=supports,
        )

    # Prefer JSON with tables for structured extract
    proc = subprocess.run(
        [str(bin_path), "extract", "--format", "json", "--tables", str(path)],
        capture_output=True,
        text=True,
        timeout=180,
    )
    if proc.returncode != 0:
        raise RuntimeError(proc.stderr.strip() or f"exit {proc.returncode}")

    try:
        payload = json.loads(proc.stdout or "{}")
    except json.JSONDecodeError as e:
        raise RuntimeError(f"invalid json: {e}") from e

    pages = payload.get("pages") or []
    texts = []
    tables_all = []

    def tab_to_grid(tab):
        rows = int(tab.get("rows") or 0)
        cols = int(tab.get("cols") or 0)
        grid = [["" for _ in range(cols)] for _ in range(rows)]
        for cell in tab.get("cells") or []:
            r = int(cell.get("row") or 0)
            c = int(cell.get("col") or 0)
            if 0 <= r < rows and 0 <= c < cols:
                grid[r][c] = str(cell.get("text") or "")
        return grid

    # Prefer document-level stitched tables (Phase V D1) when present
    root_tables = payload.get("tables")
    if isinstance(root_tables, list) and root_tables:
        for tab in root_tables:
            tables_all.append(tab_to_grid(tab))
        notes.append("stitched_root_tables")
    for page in pages:
        texts.append(page.get("text") or "")
        if not (isinstance(root_tables, list) and root_tables):
            for tab in page.get("tables") or []:
                tables_all.append(tab_to_grid(tab))

    text = normalize_text("\n".join(texts))
    page_count = payload.get("page_count")
    if page_count is None:
        page_count = len(pages)

    image_count = payload.get("image_count")
    if image_count is None and isinstance(payload.get("images"), list):
        image_count = len(payload.get("images") or [])
    links = list(payload.get("links") or [])
    form_fields = list(payload.get("form_fields") or [])
    outline = list(payload.get("outline") or [])
    if image_count is not None or links or form_fields or outline:
        notes.append("objects_from_json")

    return ExtractResult(
        library="pdfparser",
        doc_id=gt["id"],
        success=True,
        page_count=page_count,
        text=text,
        text_chars=len(text),
        tables=tables_all,
        image_count=image_count if image_count is not None else None,
        form_fields=form_fields,
        links=links,
        outline=outline,
        notes=notes,
        supports=supports,
    )



ADAPTERS: dict[str, Callable] = {
    "pdfparser": extract_pdfparser,
    "pdfplumber": extract_pdfplumber,
    "pdfminer.six": extract_pdfminer,
    "pypdf": extract_pypdf,
    "pymupdf": extract_pymupdf,
    "pypdfium2": extract_pypdfium2,
}


def run_one(lib: str, path: Path, gt: dict, with_password: bool = False) -> ExtractResult:
    fn = ADAPTERS[lib]
    password = gt.get("password") if with_password else None
    # For encrypted docs, default run is without password; second pass with password
    if gt.get("category") == "encrypted_password" and not with_password:
        password = None
    t0 = time.perf_counter()
    m0 = rss_mb()
    try:
        # force GC-ish baseline
        res = fn(path, gt, password=password)
        res.wall_ms = (time.perf_counter() - t0) * 1000.0
        res.rss_delta_mb = max(0.0, rss_mb() - m0)
        res.token_recall = token_recall(res.text, gt.get("must_contain") or [])
        any_m = token_any(res.text, gt.get("must_contain_any") or [])
        res.notes = list(res.notes or [])
        res.notes.append(f"must_contain_any={any_m}")
        if gt.get("must_contain_any") and not any_m.get("skipped") and not any_m.get("hit"):
            res.notes.append("FAIL_must_contain_any")
        # page expectations
        if gt.get("expected_pages_min") and res.page_count is not None:
            if res.page_count < gt["expected_pages_min"]:
                res.notes.append(
                    f"page_count_low:{res.page_count}<{gt['expected_pages_min']}"
                )
        res.reading_order_score = reading_order_score(
            res.text, gt.get("ideal_order_substrings") or []
        )
        cell_tokens = gt.get("table_cells_must_include") or []
        res.table_metrics = cell_token_hits(res.tables, cell_tokens)
        # table count expectation
        tmin = gt.get("expected_tables_min")
        tcount = res.table_metrics.get("table_count") or 0
        if tmin is not None and tmin > 0:
            res.table_metrics["expected_min"] = tmin
            res.table_metrics["meets_min"] = tcount >= tmin
            if tcount < tmin:
                res.notes.append(f"tables_below_min:{tcount}<{tmin}")
        # cell density signal
        if gt.get("expected_min_cells") and res.table_metrics.get("cell_count", 0) < gt["expected_min_cells"]:
            res.notes.append(
                f"cells_below_min:{res.table_metrics.get('cell_count')}<{gt['expected_min_cells']}"
            )
        # store challenge tags
        if gt.get("challenges"):
            res.notes.append("challenges=" + ",".join(gt["challenges"][:8]))
        if gt.get("tier"):
            res.notes.append(f"tier={gt['tier']}")
        # if encrypted without password and text empty, adjust
        if gt.get("category") == "encrypted_password" and not with_password:
            res.notes.append("open_without_password_attempt")
        return res
    except Exception as e:
        return ExtractResult(
            library=lib,
            doc_id=gt["id"],
            success=False,
            error=f"{type(e).__name__}: {e}",
            wall_ms=(time.perf_counter() - t0) * 1000.0,
            rss_delta_mb=max(0.0, rss_mb() - m0),
            notes=[traceback.format_exc(limit=3)],
            supports={},
        )


def summarize(all_rows: list[dict]) -> dict:
    by_lib: dict[str, list] = {}
    for r in all_rows:
        by_lib.setdefault(r["library"], []).append(r)

    summary = {}
    for lib, rows in by_lib.items():
        ok = [r for r in rows if r["success"]]
        recalls = [
            r["token_recall"].get("recall")
            for r in ok
            if r.get("token_recall") and r["token_recall"].get("recall") is not None
        ]
        times = [r["wall_ms"] for r in ok]
        summary[lib] = {
            "runs": len(rows),
            "successes": len(ok),
            "failures": len(rows) - len(ok),
            "mean_token_recall": sum(recalls) / len(recalls) if recalls else None,
            "mean_wall_ms": sum(times) / len(times) if times else None,
            "median_wall_ms": sorted(times)[len(times) // 2] if times else None,
            "total_wall_ms": sum(times) if times else 0,
        }
    return summary


def resolve_pdf(gt: dict) -> Path | None:
    doc_id = gt["id"]
    candidates = []
    if gt.get("path"):
        candidates.append(ROOT / gt["path"] if not Path(gt["path"]).is_absolute() else Path(gt["path"]))
        # also relative to benchmark root
        candidates.append(ROOT.parent / gt["path"])
    candidates.extend(
        [
            CORPUS / f"{doc_id}.pdf",
            CORPUS / "stress" / f"{doc_id}.pdf",
            CORPUS / "real" / f"{doc_id}.pdf",
            ROOT / "downloads" / f"{doc_id}.pdf",
        ]
    )
    for c in candidates:
        try:
            if c.exists():
                return c.resolve()
        except Exception:
            continue
    return None


def main() -> None:
    gts = []
    for p in sorted(GT_DIR.glob("*.json")):
        gts.append(json.loads(p.read_text(encoding="utf-8")))

    all_rows = []
    raw_for_report = []

    for gt in gts:
        doc_id = gt["id"]
        pdf_path = resolve_pdf(gt)
        if not pdf_path:
            print(f"MISSING pdf for {doc_id}")
            continue
        print(f"=== {doc_id} ({gt.get('tier','?')}/{gt.get('category')}) {pdf_path.name} ===")
        for lib in ADAPTERS:
            # standard run
            res = run_one(lib, pdf_path, gt, with_password=False)
            row = asdict(res)
            # don't dump full text into summary csv-like; keep truncated
            row_out = {**row, "text": row["text"][:2000], "text_full_chars": len(res.text)}
            all_rows.append(row_out)
            raw_for_report.append(row_out)
            status = "OK" if res.success else "FAIL"
            tr = res.token_recall.get("recall")
            print(
                f"  {lib:12} {status:4} {res.wall_ms:8.1f}ms  "
                f"recall={tr} tables={res.table_metrics.get('table_count')} "
                f"imgs={res.image_count} err={res.error}"
            )

            # encrypted with password second pass
            if gt.get("category") == "encrypted_password":
                res2 = run_one(lib, pdf_path, gt, with_password=True)
                row2 = asdict(res2)
                row2["doc_id"] = doc_id + "#with_password"
                row2["text"] = row2["text"][:2000]
                row2["text_full_chars"] = len(res2.text)
                row2["notes"] = list(row2.get("notes") or []) + ["with_password=true"]
                all_rows.append(row2)
                raw_for_report.append(row2)
                tr2 = res2.token_recall.get("recall")
                print(
                    f"  {lib:12} PW   {res2.wall_ms:8.1f}ms  recall={tr2} err={res2.error}"
                )

    summary = summarize(all_rows)
    out = {
        "benchmark": "pdfparser-competitive-v1",
        "corpus": "corpus/manifest.json",
        "libraries": list(ADAPTERS.keys()),
        "summary": summary,
        "results": all_rows,
    }
    out_path = RESULTS / "benchmark_results.json"
    out_path.write_text(json.dumps(out, indent=2), encoding="utf-8")
    print(f"\nWrote {out_path}")

    # compact CSV-like TSV
    tsv_path = RESULTS / "benchmark_results.tsv"
    cols = [
        "library",
        "doc_id",
        "success",
        "wall_ms",
        "rss_delta_mb",
        "page_count",
        "text_chars",
        "token_recall",
        "reading_order_score",
        "table_count",
        "table_cell_recall",
        "image_count",
        "error",
    ]
    lines = ["\t".join(cols)]
    for r in all_rows:
        lines.append(
            "\t".join(
                str(x)
                for x in [
                    r["library"],
                    r["doc_id"],
                    r["success"],
                    f"{r['wall_ms']:.2f}",
                    f"{r['rss_delta_mb']:.2f}",
                    r.get("page_count"),
                    r.get("text_chars"),
                    (r.get("token_recall") or {}).get("recall"),
                    r.get("reading_order_score"),
                    (r.get("table_metrics") or {}).get("table_count"),
                    (r.get("table_metrics") or {}).get("recall"),
                    r.get("image_count"),
                    (r.get("error") or "").replace("\t", " ")[:120],
                ]
            )
        )
    tsv_path.write_text("\n".join(lines) + "\n", encoding="utf-8")
    print(f"Wrote {tsv_path}")
    print("\nSUMMARY:")
    print(json.dumps(summary, indent=2))


if __name__ == "__main__":
    main()
