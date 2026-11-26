#!/usr/bin/env python3
"""Fail when a docs page's list of chart types differs from the schema's enum.

The schema (docs/docs/public/chartml_schema.json, `Visualize.properties.type`)
is the source of truth for which `visualize.type` values exist. The docs repeat
that list in prose; a type missing from one of those lists is a type readers
(and Kyomi's AI) never learn about. This check compares every such list with the
enum.

Lists recognised, per page:
  * a YAML line `type: a | b | c` (two or more alternatives), as in the spec and
    the quick reference;
  * the line after `visualize.type must be one of:` (a comma-separated list).

A list that shares no value with the enum is another kind of list (the spec's
parameter types are one) and is skipped.

Each page named below must contain at least one chart-type list (one that
shares a value with the enum), so a reworded page cannot pass by having nothing
left to check. Order is not compared, only the set.

Usage: check-spec-chart-types.py [DOCS_DIR]   (default: docs/docs)
"""
import json
import re
import sys
from pathlib import Path

SCHEMA = "public/chartml_schema.json"
PAGES = ("spec.md", "quick-reference.md", "schema.md")

PIPE_LIST = re.compile(r"^\s*type:\s*([a-z]+(?:\s*\|\s*[a-z]+)+)\s*$")
ERROR_INTRO = re.compile(r"visualize\.type must be one of:\s*$")
COMMA_LIST = re.compile(r"^\s*([a-z]+(?:\s*,\s*[a-z]+)*)\s*$")


def schema_types(schema_text):
    schema = json.loads(schema_text)
    return set(schema["definitions"]["Visualize"]["properties"]["type"]["enum"])


def type_lists(text):
    """Return [(line_number, set_of_types)] for every chart-type list in text."""
    lines = text.splitlines()
    found = []
    for number, line in enumerate(lines, 1):
        match = PIPE_LIST.match(line)
        if match:
            found.append((number, {t.strip() for t in match.group(1).split("|")}))
        elif ERROR_INTRO.search(line) and number < len(lines):
            following = COMMA_LIST.match(lines[number])
            if following:
                found.append((number + 1, {t.strip() for t in following.group(1).split(",")}))
    return found


def problems(docs_dir):
    docs = Path(docs_dir)
    expected = schema_types((docs / SCHEMA).read_text(encoding="utf-8"))
    if not expected:
        return [f"{SCHEMA}: the visualize.type enum is empty, so nothing can be checked"]
    out = []
    for page in PAGES:
        lists = type_lists((docs / page).read_text(encoding="utf-8"))
        # A list that shares no value with the enum is another kind of list
        # (the spec's parameter types are one) and is not checked.
        chart_lists = [(n, listed) for n, listed in lists if listed & expected]
        if not chart_lists:
            out.append(f"{page}: no chart-type list found, so nothing was checked")
        for number, listed in chart_lists:
            missing = sorted(expected - listed)
            extra = sorted(listed - expected)
            if missing or extra:
                out.append(
                    f"{page}:{number}: chart-type list differs from the schema enum"
                    f" (missing: {missing or 'none'}, not in schema: {extra or 'none'})"
                )
    return out


def main(argv):
    docs_dir = argv[1] if len(argv) > 1 else "docs/docs"
    try:
        found = problems(docs_dir)
    except (OSError, KeyError, ValueError) as err:
        print(f"check-spec-chart-types: {err!r}")
        return 1
    for problem in found:
        print(problem)
    return 1 if found else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
