#!/usr/bin/env python3
"""Quantitative accuracy metrics for PDF extraction benchmarks.

Implementation lives in [`metriclib`] (text / tables / objects / geom).
This module is a stable import façade for existing `import metrics` callers.
"""
from __future__ import annotations

from metriclib import *  # noqa: F403
from metriclib import __all__ as __all__  # re-export

