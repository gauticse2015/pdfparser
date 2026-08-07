"""Shared synthetic-corpus PDF helpers.

Generator scripts (`generate_*.py`) stay responsible for fixture content;
this package holds page-size and font constants so they do not drift.
"""
from reportlab.lib.pagesizes import letter
from reportlab.lib.units import inch

LETTER = letter
INCH = inch
BODY_FONT = "Helvetica"
BODY_FONT_BOLD = "Helvetica-Bold"
