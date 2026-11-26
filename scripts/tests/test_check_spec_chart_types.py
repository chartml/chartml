#!/usr/bin/env python3
"""Tests for scripts/check-spec-chart-types.py."""
import importlib.util
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

SCRIPT = Path(__file__).resolve().parents[1] / "check-spec-chart-types.py"
spec = importlib.util.spec_from_file_location("check_spec_chart_types", SCRIPT)
checker = importlib.util.module_from_spec(spec)
spec.loader.exec_module(checker)

REPO_DOCS = Path(__file__).resolve().parents[2] / "docs" / "docs"
TYPES = ["bar", "line", "table"]


def make_docs(root, spec_text, quick="  type: bar | line | table\n", schema_page=None):
    (root / "public").mkdir(parents=True)
    schema = {"definitions": {"Visualize": {"properties": {"type": {"enum": TYPES}}}}}
    (root / "public" / "chartml_schema.json").write_text(json.dumps(schema))
    (root / "spec.md").write_text(spec_text)
    (root / "quick-reference.md").write_text(quick)
    (root / "schema.md").write_text(
        schema_page or "visualize.type must be one of:\n  bar, line, table\n"
    )


class TypeListTests(unittest.TestCase):
    def test_pipe_list_and_error_list_are_found(self):
        text = "x\n  type: bar | line\nvisualize.type must be one of:\n  bar, line, table\n"
        self.assertEqual(
            checker.type_lists(text), [(2, {"bar", "line"}), (4, {"bar", "line", "table"})]
        )

    def test_single_type_line_is_not_a_list(self):
        self.assertEqual(checker.type_lists("  type: bar\n  type: source\n"), [])

    def test_matching_lists_pass(self):
        with tempfile.TemporaryDirectory() as tmp:
            make_docs(Path(tmp), "  type: table | bar | line\n")
            self.assertEqual(checker.problems(tmp), [])

    def test_type_missing_from_the_spec_is_reported(self):
        with tempfile.TemporaryDirectory() as tmp:
            make_docs(Path(tmp), "  type: bar | line\n")
            found = checker.problems(tmp)
            self.assertEqual(len(found), 1)
            self.assertIn("spec.md:1", found[0])
            self.assertIn("'table'", found[0])

    def test_type_not_in_the_schema_is_reported(self):
        with tempfile.TemporaryDirectory() as tmp:
            make_docs(Path(tmp), "  type: bar | line | table | radar\n")
            self.assertIn("'radar'", checker.problems(tmp)[0])

    def test_every_list_in_a_page_is_checked(self):
        with tempfile.TemporaryDirectory() as tmp:
            make_docs(Path(tmp), "  type: bar | line | table\n  type: bar | line\n")
            self.assertEqual(len(checker.problems(tmp)), 1)

    def test_page_without_a_list_is_reported(self):
        with tempfile.TemporaryDirectory() as tmp:
            make_docs(Path(tmp), "no lists here\n")
            self.assertIn("no chart-type list found", checker.problems(tmp)[0])

    def test_empty_schema_enum_is_reported(self):
        with tempfile.TemporaryDirectory() as tmp:
            make_docs(Path(tmp), "  type: bar | line | table\n")
            schema = {"definitions": {"Visualize": {"properties": {"type": {"enum": []}}}}}
            (Path(tmp) / "public" / "chartml_schema.json").write_text(json.dumps(schema))
            self.assertIn("enum is empty", checker.problems(tmp)[0])

    def test_error_message_list_is_checked(self):
        with tempfile.TemporaryDirectory() as tmp:
            make_docs(
                Path(tmp),
                "  type: bar | line | table\n",
                schema_page="visualize.type must be one of:\n  bar, line\n",
            )
            self.assertIn("schema.md:2", checker.problems(tmp)[0])


class CommandTests(unittest.TestCase):
    def run_checker(self, root):
        return subprocess.run([sys.executable, str(SCRIPT), str(root)],
                              capture_output=True, text=True)

    def test_exit_status(self):
        with tempfile.TemporaryDirectory() as tmp:
            self.assertEqual(self.run_checker(tmp).returncode, 1)  # no schema
            make_docs(Path(tmp), "  type: bar | line | table\n")
            self.assertEqual(self.run_checker(tmp).returncode, 0)
            (Path(tmp) / "spec.md").write_text("  type: bar | line\n")
            result = self.run_checker(tmp)
            self.assertEqual(result.returncode, 1)
            self.assertIn("table", result.stdout)

    def test_repository_docs_match_the_schema(self):
        self.assertEqual(checker.problems(REPO_DOCS), [])


if __name__ == "__main__":
    unittest.main()
