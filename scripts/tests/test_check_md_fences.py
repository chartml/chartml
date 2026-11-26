#!/usr/bin/env python3
"""Tests for scripts/check-md-fences.py."""
import importlib.util
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

SCRIPT = Path(__file__).resolve().parents[1] / "check-md-fences.py"
spec = importlib.util.spec_from_file_location("check_md_fences", SCRIPT)
checker = importlib.util.module_from_spec(spec)
spec.loader.exec_module(checker)


class UnclosedFenceTests(unittest.TestCase):
    def test_balanced_backtick_fence(self):
        self.assertEqual(checker.unclosed_fences("```yaml\na: 1\n```\n"), [])

    def test_stray_fence_after_a_closed_block_is_flagged(self):
        # The CHA-37 shape: a lone extra fence line right after a closed block.
        # Its pair is missing, so it stays open to the end of the file.
        text = "```yaml\na: 1\n```\n```\n\n5. **Dual-Axis**\n"
        self.assertEqual(checker.unclosed_fences(text), [(4, "```")])

    def test_unclosed_reports_opening_line(self):
        self.assertEqual(checker.unclosed_fences("a\n\n```js\ncode\n"), [(3, "```")])

    def test_tilde_fence(self):
        self.assertEqual(checker.unclosed_fences("~~~\nx\n~~~\n"), [])
        self.assertEqual(checker.unclosed_fences("~~~\nx\n"), [(1, "~~~")])

    def test_closer_must_use_same_character(self):
        self.assertEqual(checker.unclosed_fences("```\nx\n~~~\n"), [(1, "```")])
        self.assertEqual(checker.unclosed_fences("~~~\nx\n```\n"), [(1, "~~~")])

    def test_closer_must_be_at_least_as_long(self):
        self.assertEqual(checker.unclosed_fences("````\nx\n```\n"), [(1, "````")])
        self.assertEqual(checker.unclosed_fences("````\nx\n```\n````\n"), [])
        self.assertEqual(checker.unclosed_fences("```\nx\n`````\n"), [])

    def test_shorter_fence_inside_longer_is_content(self):
        text = "````md\n```yaml\na: 1\n```\n````\n"
        self.assertEqual(checker.unclosed_fences(text), [])

    def test_closer_cannot_have_info_string(self):
        self.assertEqual(checker.unclosed_fences("```\nx\n```js\n"), [(1, "```")])

    def test_backtick_opener_info_string_cannot_contain_backtick(self):
        self.assertEqual(checker.unclosed_fences("```a`b\n"), [])
        self.assertEqual(checker.unclosed_fences("~~~a`b\nx\n"), [(1, "~~~")])

    def test_indented_fences_in_lists(self):
        text = "1. step\n\n   ```yaml\n   a: 1\n   ```\n2. next\n"
        self.assertEqual(checker.unclosed_fences(text), [])
        self.assertEqual(checker.unclosed_fences("- item\n    ```\n    x\n"), [(2, "```")])

    def test_fences_in_blockquote(self):
        self.assertEqual(checker.unclosed_fences("> ```\n> x\n> ```\n"), [])
        self.assertEqual(checker.unclosed_fences("> ```\n> x\n"), [(1, "```")])

    def test_two_backticks_are_inline_code_not_a_fence(self):
        self.assertEqual(checker.unclosed_fences("``code``\n"), [])

    def test_closer_with_trailing_whitespace(self):
        self.assertEqual(checker.unclosed_fences("```\nx\n```   \n"), [])


class CommandTests(unittest.TestCase):
    def run_checker(self, root):
        return subprocess.run([sys.executable, str(SCRIPT), str(root)],
                              capture_output=True, text=True)

    def test_exit_status_and_exclusions(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / "ok.md").write_text("```\nx\n```\n")
            for excluded in ("node_modules/pkg", ".vitepress/dist", ".vitepress/cache", ".git/x"):
                (root / excluded).mkdir(parents=True)
                (root / excluded / "bad.md").write_text("```\nx\n")
            self.assertEqual(self.run_checker(root).returncode, 0)
            (root / "sub").mkdir()
            (root / "sub" / "bad.md").write_text("```\nx\n")
            result = self.run_checker(root)
            self.assertEqual(result.returncode, 1)
            self.assertIn("bad.md,line=1", result.stdout)

    def test_missing_or_empty_root_fails(self):
        with tempfile.TemporaryDirectory() as tmp:
            self.assertEqual(self.run_checker(Path(tmp) / "nope").returncode, 1)
            self.assertEqual(self.run_checker(tmp).returncode, 1)
            (Path(tmp) / "file.md").write_text("x\n")
            self.assertEqual(self.run_checker(Path(tmp) / "file.md").returncode, 1)

    def test_unreadable_file_fails_with_annotation(self):
        with tempfile.TemporaryDirectory() as tmp:
            (Path(tmp) / "bin.md").write_bytes(b"\xff\xfe```\n")
            result = self.run_checker(tmp)
            self.assertEqual(result.returncode, 1)
            self.assertIn("file=", result.stdout)
            self.assertNotIn("Traceback", result.stderr)

    def test_only_generated_dirs_are_excluded(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / "ok.md").write_text("x\n")
            (root / "dist").mkdir()
            (root / "dist" / "page.md").write_text("```\nx\n")
            self.assertEqual(self.run_checker(root).returncode, 1)

    def test_repository_docs_are_balanced(self):
        docs = SCRIPT.parents[1] / "docs"
        result = self.run_checker(docs)
        self.assertEqual(result.returncode, 0, result.stdout)


if __name__ == "__main__":
    unittest.main()
