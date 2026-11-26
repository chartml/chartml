#!/usr/bin/env python3
"""Fail when a markdown file has a code fence that is never closed.

An unclosed fence turns the rest of the page into one literal code block, so
the published docs render raw markdown instead of headings and examples.

Fence rules follow CommonMark: an opener is three or more backticks or tildes
(a backtick opener's info string may not contain a backtick); a closer uses the
same character, is at least as long as the opener, and has nothing after it but
whitespace. Fences may be indented (inside list items) and may sit inside
blockquotes, so any amount of leading whitespace and any `>` markers are ignored.
That is a deliberate approximation of CommonMark's container rules: it does not
model indented code blocks, and it does not end a fence when a blockquote ends.
An open fence at the end of the file is reported, as the unclosed-fence defect
this check exists to catch.

Exit status is 1 on any unclosed fence, an unreadable file, a root that is not a
directory, or a root with no markdown files (so a moved docs folder cannot make
the check pass by scanning nothing).

Usage: check-md-fences.py [ROOT ...]   (default: docs)
"""
import os
import re
import sys

EXCLUDED_DIRS = {"node_modules", ".git"}
# VitePress output lives under .vitepress; a real page folder named `dist` or
# `cache` elsewhere in docs/ must still be checked.
GENERATED_UNDER_VITEPRESS = {"dist", "cache"}

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
        generated = os.path.basename(directory) == ".vitepress"
        subdirs[:] = [d for d in subdirs if d not in EXCLUDED_DIRS
                      and not (generated and d in GENERATED_UNDER_VITEPRESS)]
        for name in sorted(files):
            if name.endswith(".md"):
                yield os.path.join(directory, name)


def main(roots):
    failures = 0
    for root in roots:
        if not os.path.isdir(root):
            print(f"::error::{root} is not a directory")
            failures += 1
            continue
        scanned = 0
        for path in markdown_files(root):
            scanned += 1
            try:
                with open(path, encoding="utf-8") as handle:
                    text = handle.read()
            except (OSError, UnicodeDecodeError) as error:
                print(f"::error file={path}::cannot read file: {error}")
                failures += 1
                continue
            for line, fence in unclosed_fences(text):
                print(f"::error file={path},line={line}::unclosed code fence {fence}")
                failures += 1
        if scanned == 0:
            print(f"::error::no markdown files found under {root}")
            failures += 1
    if failures:
        return 1
    print("All markdown code fences are closed.")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:] or ["docs"]))
