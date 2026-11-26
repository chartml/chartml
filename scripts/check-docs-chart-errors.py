#!/usr/bin/env python3
"""Fail when a built docs page shows a chart error box instead of a chart.

The docs render every `chartml` fence at build time (see the fence rule in
docs/docs/.vitepress/config.js). A fence that fails to render is replaced by a
`<div class="chartml-chart chartml-error">Chart error: ...</div>` box, so the
build succeeds and the published page quietly shows an error. This scans the
built HTML for those boxes and reports each one with its message.

Exit status is 1 on any error box, an unreadable file, a root that is not a
directory, a root with no HTML pages, or pages with no rendered chart at all
(so a broken build or a moved output folder cannot pass by scanning nothing).

Usage: check-docs-chart-errors.py [ROOT ...]
       (default: docs/docs/.vitepress/dist)
"""
import html
import os
import re
import sys

DEFAULT_ROOT = os.path.join("docs", "docs", ".vitepress", "dist")

# Two shapes reach the page, and both carry the `chartml-error` class:
#   - the box the fence rule in config.js writes when a fence fails to render;
#   - the element the renderer itself returns, inside a multi-chart fence, for
#     a chart that failed while the others rendered. It is serialised with an
#     xmlns attribute before the class and wraps the message in a span.
# So match the class wherever it sits in the tag, and drop nested tags from
# the message. The box ends at the first closing div; the message is escaped,
# so a literal closing tag cannot appear inside it.
ERROR_BOX = re.compile(
    r'<div\b[^>]*\bclass="[^"]*\bchartml-error\b[^"]*"[^>]*>(.*?)</div>', re.DOTALL
)
TAG = re.compile(r"<[^>]*>")
CHART_BOX = re.compile(r'<div class="chartml-chart"[^>]*>')


def chart_errors(text):
    """Return the message of each chart error box in a page."""
    messages = []
    for match in ERROR_BOX.finditer(text):
        message = html.unescape(TAG.sub("", match.group(1))).strip()
        messages.append(message.removeprefix("Chart error:").strip())
    return messages


def rendered_charts(text):
    return len(CHART_BOX.findall(text))


def html_files(root):
    for directory, _subdirs, files in os.walk(root):
        for name in sorted(files):
            if name.endswith(".html"):
                yield os.path.join(directory, name)


def main(roots):
    failures = 0
    for root in roots:
        if not os.path.isdir(root):
            print(f"::error::{root} is not a directory")
            failures += 1
            continue
        pages = 0
        charts = 0
        for path in html_files(root):
            pages += 1
            try:
                with open(path, encoding="utf-8") as handle:
                    text = handle.read()
            except (OSError, UnicodeDecodeError) as error:
                print(f"::error file={path}::cannot read file: {error}")
                failures += 1
                continue
            charts += rendered_charts(text)
            for message in chart_errors(text):
                print(f"::error file={path}::chart error: {message}")
                failures += 1
        if pages == 0:
            print(f"::error::no HTML pages found under {root}")
            failures += 1
        elif charts == 0:
            print(f"::error::no rendered charts found under {root}")
            failures += 1
        else:
            print(f"{root}: {pages} pages, {charts} charts rendered")
    if failures:
        return 1
    print("No chart errors in the built docs.")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:] or [DEFAULT_ROOT]))
