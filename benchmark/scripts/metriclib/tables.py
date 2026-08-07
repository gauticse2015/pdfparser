"""pdfparser benchmark metric primitives (split from metrics.py)."""
from __future__ import annotations

from typing import Any

from .text import normalize_cell, normalize_numeric_soft

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
