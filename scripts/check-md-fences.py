#!/usr/bin/env python3
"""Fail when a markdown file has a code fence that is never closed.

An unclosed fence turns the rest of the page into one literal code block, so
the published docs render raw markdown instead of headings and examples.

Fence rules follow CommonMark: an opener is three or more backticks or tildes
(a backtick opener's info string may not contain a backtick); a closer uses the
same character, is at least as long as the opener, and has nothing after it but
whitespace. Fences may be indented (inside list items) and may sit inside
blockquotes, so any leading whitespace and `>` markers are ignored.

Usage: check-md-fences.py [ROOT ...]   (default: docs)
"""
import os
import re
import sys

EXCLUDED_DIRS = {"node_modules", "dist", "cache", ".git"}

PREFIX = re.compile(r"^(?:[ \t]*>)*[ \t]*")
OPENER = re.compile(r"^(`{3,}|~{3,})(.*)$")


def unclosed_fences(text):
    """Return (line_number, fence) for each fence still open at end of file."""
    open_char = None
    open_len = 0
    open_line = 0
    for number, raw in enumerate(text.splitlines(), start=1):
        line = raw[PREFIX.match(raw).end():]
        match = OPENER.match(line)
        if open_char is None:
            if match and not (match.group(1)[0] == "`" and "`" in match.group(2)):
                open_char = match.group(1)[0]
                open_len = len(match.group(1))
                open_line = number
        elif match and match.group(1)[0] == open_char \
                and len(match.group(1)) >= open_len and not match.group(2).strip():
            open_char = None
    if open_char is None:
        return []
    return [(open_line, open_char * open_len)]


def markdown_files(root):
    for directory, subdirs, files in os.walk(root):
        subdirs[:] = [d for d in subdirs if d not in EXCLUDED_DIRS]
        for name in sorted(files):
            if name.endswith(".md"):
                yield os.path.join(directory, name)


def main(roots):
    failures = 0
    for root in roots:
        for path in markdown_files(root):
            with open(path, encoding="utf-8") as handle:
                text = handle.read()
            for line, fence in unclosed_fences(text):
                print(f"::error file={path},line={line}::unclosed code fence {fence}")
                failures += 1
    if failures:
        return 1
    print("All markdown code fences are closed.")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:] or ["docs"]))
