#!/usr/bin/env python3
"""Quantitative accuracy metrics for PDF extraction benchmarks.

Metrics designed so future `pdfparser` scores are comparable to competitors.

TEXT
  - token_recall / token_precision / token_f1  (must_contain tokens)
  - must_contain_any_hit (binary)
  - cer  (character error rate vs reference_text, if provided)
  - wer  (word error rate vs reference_text, if provided)
  - normalized_similarity (1 - cer clipped)

TABLES
  - detection: precision/recall/f1 from predicted table count vs expected count
    (with optional tolerance). Also exact-match rate when expected_tables list given.
  - structure: row_accuracy, col_accuracy, shape_exact (vs gold grids)
  - cell: cell_recall, cell_precision, cell_f1 on normalized cell texts
    (micro over all gold cells after best table alignment)
  - content_token_recall: legacy substring tokens inside any cell

OBJECTS
  - images: count_error, count_exact, count_recall (min(pred,exp)/exp)
  - links: set precision/recall/f1
  - forms: set precision/recall/f1
  - outline: set precision/recall/f1

Aggregate score helpers for scoreboard columns.
"""
from __future__ import annotations

import re
import unicodedata
from typing import Any, Optional


# ─────────────────────────── normalization ───────────────────────────

def normalize_text(s: str) -> str:
    if s is None:
        return ""
    s = unicodedata.normalize("NFKC", str(s))
    s = s.replace("\x00", " ")
    s = s.replace("\r", "\n")
    s = re.sub(r"[ \t]+", " ", s)
    s = re.sub(r"\n{3,}", "\n\n", s)
    return s.strip()


def normalize_cell(s: str) -> str:
    """Aggressive normalize for cell matching."""
    s = normalize_text(s)
    s = s.replace("\n", " ")
    s = re.sub(r"\s+", " ", s)
    # collapse common MuPDF underscore corruption: "TOKEN SIDE L" vs TOKEN_SIDE_L
    s = s.replace(" _", "_").replace("_ ", "_")
    s = s.lower().strip()
    # strip currency/comma noise for numeric soft match handled separately
    return s


def normalize_numeric_soft(s: str) -> str:
    s = normalize_cell(s)
    s = s.replace(",", "").replace("$", "").replace("%", "")
    return s


# ─────────────────────────── text metrics ───────────────────────────

def levenshtein(a: str, b: str) -> int:
    if a == b:
        return 0
    if not a:
        return len(b)
    if not b:
        return len(a)
    if len(a) < len(b):
        a, b = b, a
    prev = list(range(len(b) + 1))
    for i, ca in enumerate(a, 1):
        cur = [i]
        for j, cb in enumerate(b, 1):
            ins = cur[j - 1] + 1
            delete = prev[j] + 1
            sub = prev[j - 1] + (ca != cb)
            cur.append(min(ins, delete, sub))
        prev = cur
    return prev[-1]


def character_error_rate(ref: str, hyp: str) -> Optional[float]:
    ref_n = normalize_text(ref)
    hyp_n = normalize_text(hyp)
    if not ref_n:
        return None
    dist = levenshtein(ref_n, hyp_n)
    return dist / max(len(ref_n), 1)


def word_error_rate(ref: str, hyp: str) -> Optional[float]:
    ref_w = normalize_text(ref).split()
    hyp_w = normalize_text(hyp).split()
    if not ref_w:
        return None
    # classic WER via edit distance on tokens
    dist = _seq_levenshtein(ref_w, hyp_w)
    return dist / max(len(ref_w), 1)


def _seq_levenshtein(a: list, b: list) -> int:
    if a == b:
        return 0
    if not a:
        return len(b)
    if not b:
        return len(a)
    prev = list(range(len(b) + 1))
    for i, ca in enumerate(a, 1):
        cur = [i]
        for j, cb in enumerate(b, 1):
            ins = cur[j - 1] + 1
            delete = prev[j] + 1
            sub = prev[j - 1] + (ca != cb)
            cur.append(min(ins, delete, sub))
        prev = cur
    return prev[-1]


def token_set_metrics(text: str, tokens: list[str]) -> dict[str, Any]:
    """Precision/recall/F1 treating must_contain tokens as bag (each token once)."""
    if not tokens:
        return {
            "precision": None,
            "recall": None,
            "f1": None,
            "hit": 0,
            "total": 0,
            "missing": [],
            "skipped": True,
        }
    text_n = text  # substring match on raw-ish text (tokens are designed as exact)
    missing = [t for t in tokens if t not in text_n]
    hit = len(tokens) - len(missing)
    recall = hit / len(tokens)
    # precision for required-token protocol is same as recall when all tokens are required
    # (we do not invent false tokens). Optional: soft precision = 1.0 if no garbage check.
    precision = 1.0 if hit == len(tokens) else hit / len(tokens)
    f1 = (2 * precision * recall / (precision + recall)) if (precision + recall) else 0.0
    return {
        "precision": precision,
        "recall": recall,
        "f1": f1,
        "hit": hit,
        "total": len(tokens),
        "missing": missing,
        "skipped": False,
    }


def text_accuracy(pred_text: str, gold: dict) -> dict[str, Any]:
    tokens = gold.get("must_contain") or []
    any_tokens = gold.get("must_contain_any") or []
    ref = gold.get("reference_text")

    tok = token_set_metrics(pred_text, tokens)
    any_hit = None
    any_matched = None
    if any_tokens:
        for t in any_tokens:
            if t in pred_text:
                any_hit = True
                any_matched = t
                break
        if any_hit is None:
            any_hit = False

    cer = character_error_rate(ref, pred_text) if ref else None
    wer = word_error_rate(ref, pred_text) if ref else None
    sim = (1.0 - min(cer, 1.0)) if cer is not None else None

    # composite text score 0-100
    parts = []
    weights = []
    if tok.get("f1") is not None and not tok.get("skipped"):
        parts.append(tok["f1"])
        weights.append(0.5 if ref else 1.0)
    if sim is not None:
        parts.append(sim)
        weights.append(0.5)
    if any_hit is True:
        parts.append(1.0)
        weights.append(0.15)
    elif any_hit is False:
        parts.append(0.0)
        weights.append(0.15)

    if parts and sum(weights) > 0:
        score = 100.0 * sum(p * w for p, w in zip(parts, weights)) / sum(weights)
    else:
        score = None

    return {
        "token": tok,
        "must_contain_any_hit": any_hit,
        "must_contain_any_matched": any_matched,
        "cer": cer,
        "wer": wer,
        "normalized_similarity": sim,
        "score_0_100": score,
        "has_reference_text": bool(ref),
    }


# ─────────────────────────── table metrics ───────────────────────────

def _pred_tables_normalized(tables: list) -> list[list[list[str]]]:
    out = []
    for t in tables or []:
        grid = []
        for row in t:
            grid.append([normalize_cell(c if c is not None else "") for c in row])
        if grid:
            out.append(grid)
    return out


def _gold_tables_normalized(gold_tables: list) -> list[list[list[str]]]:
    out = []
    for t in gold_tables or []:
        if isinstance(t, dict):
            cells = t.get("cells") or []
        else:
            cells = t
        grid = []
        for row in cells:
            grid.append([normalize_cell(c if c is not None else "") for c in row])
        if grid:
            out.append(grid)
    return out


def table_detection_metrics(n_pred: int, n_exp: int, tol: int = 0) -> dict[str, Any]:
    """Count-based detection metrics.

    Treat as multi-set of identical 'table' instances:
      TP = min(pred, exp)
      FP = max(pred - exp, 0)
      FN = max(exp - pred, 0)
    """
    if n_exp is None:
        return {"skipped": True}
    tp = min(n_pred, n_exp)
    fp = max(n_pred - n_exp, 0)
    fn = max(n_exp - n_pred, 0)
    # tolerance: if |pred-exp| <= tol, treat as exact for scoring bonus
    within_tol = abs(n_pred - n_exp) <= tol
    prec = tp / (tp + fp) if (tp + fp) else (1.0 if n_exp == 0 and n_pred == 0 else 0.0)
    rec = tp / (tp + fn) if (tp + fn) else (1.0 if n_exp == 0 else 0.0)
    f1 = (2 * prec * rec / (prec + rec)) if (prec + rec) else 0.0
    return {
        "skipped": False,
        "expected": n_exp,
        "predicted": n_pred,
        "tp": tp,
        "fp": fp,
        "fn": fn,
        "precision": prec,
        "recall": rec,
        "f1": f1,
        "exact": n_pred == n_exp,
        "within_tolerance": within_tol,
        "tolerance": tol,
    }


def _shape(grid: list[list[str]]) -> tuple[int, int]:
    rows = len(grid)
    cols = max((len(r) for r in grid), default=0)
    return rows, cols


def _pad_grid(grid: list[list[str]], rows: int, cols: int) -> list[list[str]]:
    out = []
    for i in range(rows):
        if i < len(grid):
            row = list(grid[i]) + [""] * (cols - len(grid[i]))
            out.append(row[:cols])
        else:
            out.append([""] * cols)
    return out


def _cell_match(g: str, p: str) -> bool:
    if g == p:
        return True
    if not g and not p:
        return True
    # soft numeric
    if normalize_numeric_soft(g) == normalize_numeric_soft(p) and normalize_numeric_soft(g) != "":
        return True
    # containment for wrapped cells (gold short token in pred long cell or vice versa)
    if g and p and (g in p or p in g) and min(len(g), len(p)) >= 4:
        return True
    return False


def _align_and_cell_f1(gold: list[list[str]], pred: list[list[str]]) -> dict[str, Any]:
    gr, gc = _shape(gold)
    pr, pc = _shape(pred)
    # align to gold shape by padding/truncating pred
    rows, cols = gr, gc
    g = _pad_grid(gold, rows, cols)
    p = _pad_grid(pred, max(pr, rows), max(pc, cols))
    p = [r[:cols] for r in p[:rows]]
    # if pred has fewer rows, pad
    p = _pad_grid(p, rows, cols)

    tp = fp = fn = 0
    matched = 0
    total_gold_nonzero = 0
    for i in range(rows):
        for j in range(cols):
            gv, pv = g[i][j], p[i][j]
            g_empty = gv == ""
            p_empty = pv == ""
            if g_empty and p_empty:
                continue
            if not g_empty:
                total_gold_nonzero += 1
            if not g_empty and not p_empty and _cell_match(gv, pv):
                tp += 1
                matched += 1
            elif not g_empty and (p_empty or not _cell_match(gv, pv)):
                fn += 1
            elif g_empty and not p_empty:
                fp += 1
            else:
                # both non-empty but no match
                fp += 1
                fn += 1

    prec = tp / (tp + fp) if (tp + fp) else 0.0
    rec = tp / (tp + fn) if (tp + fn) else 0.0
    f1 = (2 * prec * rec / (prec + rec)) if (prec + rec) else 0.0
    return {
        "gold_shape": [gr, gc],
        "pred_shape": [pr, pc],
        "row_exact": gr == pr,
        "col_exact": gc == pc,
        "shape_exact": gr == pr and gc == pc,
        "cell_precision": prec,
        "cell_recall": rec,
        "cell_f1": f1,
        "cells_matched": matched,
        "gold_nonzero_cells": total_gold_nonzero,
        "tp": tp,
        "fp": fp,
        "fn": fn,
    }


def _best_table_assignment(
    gold_tables: list[list[list[str]]], pred_tables: list[list[list[str]]]
) -> list[tuple[int, int, dict]]:
    """Greedy match gold→pred by highest cell_f1."""
    used_pred = set()
    pairs = []
    for gi, g in enumerate(gold_tables):
        best = None
        best_score = -1.0
        best_pj = None
        for pj, p in enumerate(pred_tables):
            if pj in used_pred:
                continue
            m = _align_and_cell_f1(g, p)
            score = m["cell_f1"]
            # slight bonus for shape match
            if m["shape_exact"]:
                score += 0.05
            if score > best_score:
                best_score = score
                best = m
                best_pj = pj
        if best is not None and best_pj is not None:
            used_pred.add(best_pj)
            pairs.append((gi, best_pj, best))
        else:
            pairs.append((gi, -1, {
                "gold_shape": list(_shape(g)),
                "pred_shape": [0, 0],
                "row_exact": False,
                "col_exact": False,
                "shape_exact": False,
                "cell_precision": 0.0,
                "cell_recall": 0.0,
                "cell_f1": 0.0,
                "cells_matched": 0,
                "gold_nonzero_cells": sum(1 for row in g for c in row if c),
                "tp": 0,
                "fp": 0,
                "fn": sum(1 for row in g for c in row if c),
                "unmatched_gold": True,
            }))
    return pairs


def table_accuracy(pred_tables: list, gold: dict) -> dict[str, Any]:
    """Full table accuracy block."""
    pred_n = len(pred_tables or [])
    gold_list = gold.get("expected_tables")  # list of {rows, cols, cells}
    exp_count = gold.get("expected_table_count")
    if exp_count is None and gold_list is not None:
        exp_count = len(gold_list)
    if exp_count is None and gold.get("expected_tables_min") is not None:
        # weak count: if pred >= min, treat expected as pred for detection soft-pass
        exp_count = gold["expected_tables_min"]
        weak = True
    else:
        weak = False

    # detection
    tol = int(gold.get("table_count_tolerance") or 0)
    det = table_detection_metrics(pred_n, exp_count, tol=tol) if exp_count is not None else {"skipped": True}
    if weak and not det.get("skipped"):
        # soft: if pred >= min, detection recall forced 1.0 for min-only gold
        if pred_n >= gold["expected_tables_min"]:
            det = {
                **det,
                "weak_min_only": True,
                "recall": 1.0,
                "precision": 1.0 if pred_n == gold["expected_tables_min"] else max(0.0, gold["expected_tables_min"] / pred_n),
                "f1": None,  # recompute
                "note": "expected_tables_min only (no exact count gold)",
            }
            p, r = det["precision"], det["recall"]
            det["f1"] = (2 * p * r / (p + r)) if (p + r) else 0.0

    # structure + cell against full grids
    structure = {"skipped": True}
    cell = {"skipped": True}
    per_table = []
    if gold_list:
        g_norm = _gold_tables_normalized(gold_list)
        p_norm = _pred_tables_normalized(pred_tables)
        pairs = _best_table_assignment(g_norm, p_norm)
        per_table = [
            {"gold_index": gi, "pred_index": pj, **m} for gi, pj, m in pairs
        ]
        # micro aggregate
        tp = sum(m["tp"] for *_, m in pairs)
        fp = sum(m["fp"] for *_, m in pairs)
        fn = sum(m["fn"] for *_, m in pairs)
        prec = tp / (tp + fp) if (tp + fp) else 0.0
        rec = tp / (tp + fn) if (tp + fn) else 0.0
        f1 = (2 * prec * rec / (prec + rec)) if (prec + rec) else 0.0
        row_acc = sum(1 for *_, m in pairs if m.get("row_exact")) / max(len(pairs), 1)
        col_acc = sum(1 for *_, m in pairs if m.get("col_exact")) / max(len(pairs), 1)
        shape_acc = sum(1 for *_, m in pairs if m.get("shape_exact")) / max(len(pairs), 1)
        structure = {
            "skipped": False,
            "row_accuracy": row_acc,
            "col_accuracy": col_acc,
            "shape_exact_rate": shape_acc,
            "n_gold_tables": len(g_norm),
            "n_pred_tables": len(p_norm),
        }
        cell = {
            "skipped": False,
            "precision": prec,
            "recall": rec,
            "f1": f1,
            "tp": tp,
            "fp": fp,
            "fn": fn,
        }

    # content token recall inside cells (legacy)
    content_tokens = gold.get("table_cells_must_include") or []
    flat = []
    for t in pred_tables or []:
        for row in t:
            for c in row:
                if c is not None:
                    flat.append(str(c))
    blob = " | ".join(flat)
    if content_tokens:
        missing = [t for t in content_tokens if t not in blob]
        hit = len(content_tokens) - len(missing)
        content = {
            "recall": hit / len(content_tokens),
            "hit": hit,
            "total": len(content_tokens),
            "missing": missing,
        }
    else:
        content = {"recall": None, "skipped": True}

    # composite table score 0-100
    parts, weights = [], []
    if not det.get("skipped") and det.get("f1") is not None:
        parts.append(det["f1"])
        weights.append(0.35)
    if not structure.get("skipped"):
        parts.append(structure["shape_exact_rate"])
        weights.append(0.25)
    if not cell.get("skipped"):
        parts.append(cell["f1"])
        weights.append(0.40)
    elif content.get("recall") is not None:
        parts.append(content["recall"])
        weights.append(0.40)

    score = 100.0 * sum(p * w for p, w in zip(parts, weights)) / sum(weights) if parts else None

    return {
        "detection": det,
        "structure": structure,
        "cell": cell,
        "content_tokens": content,
        "per_table": per_table,
        "predicted_table_count": pred_n,
        "predicted_cell_count": len(flat),
        "predicted_row_counts": [len(t) for t in (pred_tables or [])],
        "score_0_100": score,
        "has_grid_gold": bool(gold_list),
    }


# ─────────────────────────── object metrics ───────────────────────────

def _count_metrics(pred: Optional[int], exp: Optional[int]) -> dict[str, Any]:
    if exp is None:
        return {"skipped": True, "predicted": pred}
    if pred is None:
        # Library adapter has no image/object API — do not score as 0 (would bias overall).
        return {
            "skipped": True,
            "expected": exp,
            "predicted": None,
            "unavailable": True,
        }
    exact = pred == exp
    abs_error = abs(pred - exp)
    recall = min(pred, exp) / exp if exp > 0 else (1.0 if pred == 0 else 0.0)
    # score: 1 if exact else decay by relative error
    if exp == 0:
        score = 1.0 if pred == 0 else 0.0
    else:
        score = max(0.0, 1.0 - abs_error / max(exp, 1))
    return {
        "skipped": False,
        "expected": exp,
        "predicted": pred,
        "exact": exact,
        "abs_error": abs_error,
        "recall": recall,
        "score": score,
    }


def _set_metrics(pred: list[str], exp: list[str]) -> dict[str, Any]:
    if exp is None:
        return {"skipped": True}
    exp_set = {normalize_cell(x) for x in exp if x}
    pred_set = {normalize_cell(x) for x in (pred or []) if x}
    if not exp_set and not pred_set:
        return {"precision": 1.0, "recall": 1.0, "f1": 1.0, "tp": 0, "fp": 0, "fn": 0, "skipped": False}
    tp = len(exp_set & pred_set)
    fp = len(pred_set - exp_set)
    fn = len(exp_set - pred_set)
    prec = tp / (tp + fp) if (tp + fp) else 0.0
    rec = tp / (tp + fn) if (tp + fn) else 0.0
    f1 = (2 * prec * rec / (prec + rec)) if (prec + rec) else 0.0
    return {
        "skipped": False,
        "precision": prec,
        "recall": rec,
        "f1": f1,
        "tp": tp,
        "fp": fp,
        "fn": fn,
        "expected": sorted(exp_set),
        "predicted": sorted(pred_set),
    }


def objects_accuracy(
    *,
    image_count: Optional[int],
    links: list,
    form_fields: list,
    outline: list,
    gold: dict,
) -> dict[str, Any]:
    images = _count_metrics(image_count, gold.get("expected_images"))
    # links: gold may be list of URI substrings or full URIs
    exp_links = gold.get("expected_links")
    if exp_links is None and gold.get("expected_link_uri_contains"):
        exp_links = [gold["expected_link_uri_contains"]]
    # soft link match: pred hits if any pred contains gold or equals
    link_metrics = {"skipped": True}
    if exp_links is not None:
        pred_l = [str(x) for x in (links or [])]
        tp = 0
        missing = []
        for g in exp_links:
            if any(g in p or p in g for p in pred_l):
                tp += 1
            else:
                missing.append(g)
        rec = tp / len(exp_links) if exp_links else 1.0
        # precision soft: if we have preds and all gold found, 1.0; else tp/len(pred)
        prec = 1.0 if not pred_l and not exp_links else (tp / max(len(pred_l), 1) if pred_l else 0.0)
        # better: precision = 1 if every pred matches some gold loosely when gold exists
        if exp_links and pred_l:
            matched_preds = sum(1 for p in pred_l if any(g in p or p in g for g in exp_links))
            prec = matched_preds / len(pred_l)
        f1 = (2 * prec * rec / (prec + rec)) if (prec + rec) else 0.0
        link_metrics = {
            "skipped": False,
            "precision": prec,
            "recall": rec,
            "f1": f1,
            "missing": missing,
            "predicted": pred_l,
            "expected": exp_links,
        }

    exp_forms = gold.get("expected_form_fields") or gold.get("expected_form_field_names")
    # forms: extract names before '='
    pred_form_names = []
    for f in form_fields or []:
        s = str(f)
        pred_form_names.append(s.split("=")[0].strip())
    forms = _set_metrics(pred_form_names, exp_forms) if exp_forms is not None else {"skipped": True}

    exp_outline = gold.get("expected_outline_titles")
    outline_m = _set_metrics(outline or [], exp_outline) if exp_outline is not None else {"skipped": True}

    parts, weights = [], []
    if not images.get("skipped") and images.get("score") is not None:
        parts.append(images["score"])
        weights.append(1.0)
    if not link_metrics.get("skipped") and link_metrics.get("f1") is not None:
        parts.append(link_metrics["f1"])
        weights.append(1.0)
    if not forms.get("skipped") and forms.get("f1") is not None:
        parts.append(forms["f1"])
        weights.append(1.0)
    if not outline_m.get("skipped") and outline_m.get("f1") is not None:
        parts.append(outline_m["f1"])
        weights.append(1.0)
    score = 100.0 * sum(p * w for p, w in zip(parts, weights)) / sum(weights) if parts else None

    return {
        "images": images,
        "links": link_metrics,
        "forms": forms,
        "outline": outline_m,
        "score_0_100": score,
    }


def overall_accuracy(text_m: dict, table_m: dict, obj_m: dict, gold: dict) -> dict[str, Any]:
    """Weighted overall 0-100 for scoreboard."""
    # weights by what's gold-available and category emphasis
    tw = gold.get("weight_text", 0.40)
    tbw = gold.get("weight_tables", 0.40)
    ow = gold.get("weight_objects", 0.20)

    scores = []
    weights = []
    if text_m.get("score_0_100") is not None:
        scores.append(text_m["score_0_100"])
        weights.append(tw)
    if table_m.get("score_0_100") is not None:
        scores.append(table_m["score_0_100"])
        weights.append(tbw)
    if obj_m.get("score_0_100") is not None:
        scores.append(obj_m["score_0_100"])
        weights.append(ow)

    if not scores:
        return {"score_0_100": None, "components_used": []}
    overall = sum(s * w for s, w in zip(scores, weights)) / sum(weights)
    return {
        "score_0_100": overall,
        "components_used": {
            "text": text_m.get("score_0_100"),
            "tables": table_m.get("score_0_100"),
            "objects": obj_m.get("score_0_100"),
        },
        "weights": {"text": tw, "tables": tbw, "objects": ow},
    }
