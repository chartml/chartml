#!/usr/bin/env python3
"""Tests for scripts/check-docs-chart-errors.py."""
import importlib.util
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

SCRIPT = Path(__file__).resolve().parents[1] / "check-docs-chart-errors.py"
spec = importlib.util.spec_from_file_location("check_docs_chart_errors", SCRIPT)
checker = importlib.util.module_from_spec(spec)
spec.loader.exec_module(checker)

# Exactly what the fence rule in docs/docs/.vitepress/config.js emits.
OK_BOX = '<div class="chartml-chart"><svg></svg></div>'
ERROR_BOX = (
    '<div class="chartml-chart chartml-error" style="color: #dc3545;">'
    "Chart error: Data error: Named data source &#39;q1_sales&#39; not found &amp; more</div>"
)


class ChartErrorTests(unittest.TestCase):
    def test_page_with_only_charts_has_no_errors(self):
        self.assertEqual(checker.chart_errors(OK_BOX + OK_BOX), [])
        self.assertEqual(checker.rendered_charts(OK_BOX + OK_BOX), 2)

    def test_error_box_is_reported_with_unescaped_message(self):
        self.assertEqual(
            checker.chart_errors(OK_BOX + ERROR_BOX),
            ["Data error: Named data source 'q1_sales' not found & more"],
        )

    def test_error_box_is_not_counted_as_a_chart(self):
        self.assertEqual(checker.rendered_charts(ERROR_BOX), 0)

    def test_every_error_box_is_reported(self):
        self.assertEqual(len(checker.chart_errors(ERROR_BOX * 3)), 3)

    def test_error_class_is_found_in_any_position(self):
        reordered = '<div class="chartml-error chartml-chart" id="x">Chart error: boom</div>'
        self.assertEqual(checker.chart_errors(reordered), ["boom"])

    def test_error_inside_a_multi_chart_fence_is_reported(self):
        # What the renderer serialises (svg.rs, ChartElement::Div and Span) for
        # one failed chart among several: xmlns before class, message in a span.
        multi = (
            '<div class="chartml-chart"><svg><foreignObject>'
            '<div xmlns="http://www.w3.org/1999/xhtml" class="chartml-multi-chart">'
            '<div xmlns="http://www.w3.org/1999/xhtml" class="chartml-error">'
            "<span>Chart error: boom &amp; more</span></div></div></foreignObject></svg></div>"
        )
        self.assertEqual(checker.chart_errors(multi), ["boom & more"])

    def test_prose_mentioning_chart_error_is_not_a_box(self):
        self.assertEqual(checker.chart_errors("<p>Chart error: shown on failure</p>"), [])


class CommandTests(unittest.TestCase):
    def run_checker(self, root):
        return subprocess.run([sys.executable, str(SCRIPT), str(root)],
                              capture_output=True, text=True)

    def test_exit_status(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            self.assertEqual(self.run_checker(root).returncode, 1)  # no pages
            (root / "a.html").write_text("<p>no charts</p>")
            self.assertEqual(self.run_checker(root).returncode, 1)  # nothing rendered
            (root / "a.html").write_text(OK_BOX)
            self.assertEqual(self.run_checker(root).returncode, 0)
            (root / "sub").mkdir()
            (root / "sub" / "b.html").write_text(OK_BOX + ERROR_BOX)
            result = self.run_checker(root)
            self.assertEqual(result.returncode, 1)
            self.assertIn("b.html", result.stdout)
            self.assertIn("q1_sales", result.stdout)

    def test_missing_root_fails(self):
        self.assertEqual(self.run_checker("/nonexistent-docs-dist").returncode, 1)


if __name__ == "__main__":
    unittest.main()
