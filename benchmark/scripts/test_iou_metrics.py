#!/usr/bin/env python3
"""Unit tests for table_detection_metrics_iou / rect_iou (PR6a).

No PDF or binary required. Exit 0 on success, 1 on assertion failure.
"""
from __future__ import annotations

import sys
from pathlib import Path

SCRIPTS = Path(__file__).resolve().parent
sys.path.insert(0, str(SCRIPTS))
import metrics as M  # noqa: E402


def _approx(a: float, b: float, eps: float = 1e-6) -> bool:
    return abs(a - b) <= eps


def test_rect_iou_identical() -> None:
    r = {"x0": 0, "y0": 0, "x1": 10, "y1": 10}
    assert _approx(M.rect_iou(r, r), 1.0)


def test_rect_iou_disjoint() -> None:
    a = {"x0": 0, "y0": 0, "x1": 10, "y1": 10}
    b = {"x0": 20, "y0": 20, "x1": 30, "y1": 30}
    assert _approx(M.rect_iou(a, b), 0.0)


def test_rect_iou_partial() -> None:
    # 10x10 and shift by 5 in x → inter 5x10=50, union=100+100-50=150 → 1/3
    a = {"x0": 0, "y0": 0, "x1": 10, "y1": 10}
    b = {"x0": 5, "y0": 0, "x1": 15, "y1": 10}
    assert _approx(M.rect_iou(a, b), 50.0 / 150.0)


def test_rect_iou_missing_keys() -> None:
    assert M.rect_iou({}, {"x0": 0, "y0": 0, "x1": 1, "y1": 1}) == 0.0
    assert M.rect_iou(None, {"x0": 0, "y0": 0, "x1": 1, "y1": 1}) == 0.0  # type: ignore[arg-type]


def test_iou_perfect_match() -> None:
    gold = [{"page": 0, "bbox": {"x0": 0, "y0": 0, "x1": 100, "y1": 50}}]
    pred = [{"page": 0, "bbox": {"x0": 0, "y0": 0, "x1": 100, "y1": 50}}]
    m = M.table_detection_metrics_iou(pred, gold, iou_thresh=0.5)
    assert m["match_mode"] == "iou"
    assert m["tp"] == 1 and m["fp"] == 0 and m["fn"] == 0
    assert _approx(m["f1"], 1.0)
    assert len(m["pairs"]) == 1
    assert _approx(m["pairs"][0]["iou"], 1.0)


def test_iou_below_threshold_is_fn_fp() -> None:
    gold = [{"page": 0, "bbox": {"x0": 0, "y0": 0, "x1": 100, "y1": 100}}]
    # tiny overlap
    pred = [{"page": 0, "bbox": {"x0": 90, "y0": 90, "x1": 110, "y1": 110}}]
    m = M.table_detection_metrics_iou(pred, gold, iou_thresh=0.5)
    assert m["tp"] == 0
    assert m["fp"] == 1 and m["fn"] == 1
    assert _approx(m["f1"], 0.0)


def test_iou_page_must_match() -> None:
    gold = [{"page": 0, "bbox": {"x0": 0, "y0": 0, "x1": 10, "y1": 10}}]
    pred = [{"page": 1, "bbox": {"x0": 0, "y0": 0, "x1": 10, "y1": 10}}]
    m = M.table_detection_metrics_iou(pred, gold, iou_thresh=0.5)
    assert m["tp"] == 0
    assert m["fp"] == 1 and m["fn"] == 1


def test_iou_greedy_one_to_one() -> None:
    # two gold; one pred overlaps both — only one TP
    gold = [
        {"page": 0, "bbox": {"x0": 0, "y0": 0, "x1": 100, "y1": 40}},
        {"page": 0, "bbox": {"x0": 0, "y0": 50, "x1": 100, "y1": 90}},
    ]
    pred = [{"page": 0, "bbox": {"x0": 0, "y0": 0, "x1": 100, "y1": 40}}]
    m = M.table_detection_metrics_iou(pred, gold, iou_thresh=0.5)
    assert m["tp"] == 1
    assert m["fp"] == 0
    assert m["fn"] == 1
    assert _approx(m["precision"], 1.0)
    assert _approx(m["recall"], 0.5)


def test_iou_extra_pred_fp() -> None:
    gold = [{"page": 0, "bbox": {"x0": 0, "y0": 0, "x1": 10, "y1": 10}}]
    pred = [
        {"page": 0, "bbox": {"x0": 0, "y0": 0, "x1": 10, "y1": 10}},
        {"page": 0, "bbox": {"x0": 50, "y0": 50, "x1": 60, "y1": 60}},
    ]
    m = M.table_detection_metrics_iou(pred, gold, iou_thresh=0.5)
    assert m["tp"] == 1 and m["fp"] == 1 and m["fn"] == 0
    assert _approx(m["precision"], 0.5)
    assert _approx(m["recall"], 1.0)


def test_iou_empty_both() -> None:
    m = M.table_detection_metrics_iou([], [], iou_thresh=0.5)
    assert m["expected"] == 0 and m["predicted"] == 0
    assert _approx(m["f1"], 1.0)


def test_count_fallback_missing_gold_bbox() -> None:
    gold = [{"page": 0}]  # no bbox
    pred = [{"page": 0, "bbox": {"x0": 0, "y0": 0, "x1": 1, "y1": 1}}]
    m = M.table_detection_metrics_iou(pred, gold, iou_thresh=0.5)
    assert m["match_mode"] == "count_fallback_missing_gold_bbox"
    assert m["expected"] == 1
    assert m["predicted"] == 1


def test_gold_none_skipped() -> None:
    m = M.table_detection_metrics_iou([], None)  # type: ignore[arg-type]
    assert m.get("skipped") is True


def main() -> int:
    tests = [
        test_rect_iou_identical,
        test_rect_iou_disjoint,
        test_rect_iou_partial,
        test_rect_iou_missing_keys,
        test_iou_perfect_match,
        test_iou_below_threshold_is_fn_fp,
        test_iou_page_must_match,
        test_iou_greedy_one_to_one,
        test_iou_extra_pred_fp,
        test_iou_empty_both,
        test_count_fallback_missing_gold_bbox,
        test_gold_none_skipped,
    ]
    failed = 0
    for t in tests:
        try:
            t()
            print(f"  OK  {t.__name__}")
        except AssertionError as e:
            failed += 1
            print(f"FAIL  {t.__name__}: {e}", file=sys.stderr)
        except Exception as e:
            failed += 1
            print(f"ERR   {t.__name__}: {type(e).__name__}: {e}", file=sys.stderr)
    if failed:
        print(f"test_iou_metrics: {failed}/{len(tests)} failed", file=sys.stderr)
        return 1
    print(f"test_iou_metrics: OK ({len(tests)} cases)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
