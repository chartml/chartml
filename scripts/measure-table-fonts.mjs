#!/usr/bin/env node
// Measure the advance widths the table renderer sizes its Latin columns from,
// and write `crates/chartml-chart-table/tests/fixtures/font_scan.txt`.
//
// The fixture is the widest advance of the common UI faces per character. Three
// passes contribute, and each one takes the maximum of what it measured and what
// the file already holds, so the passes are order-independent and re-runnable:
//
//   # 1. The Linux faces, in headless Chromium (Debian/Ubuntu container)
//   node scripts/measure-table-fonts.mjs --write
//
//   # 2. macOS: Chrome's own `system-ui` resolution at 400/600/700. This is not
//   #    the same font AppKit picks, so it catches glyphs Chrome falls back for.
//   node scripts/measure-table-fonts.mjs --write --families system-ui
//
//   # 3. macOS: the system font through AppKit at 400/600/700
//   swiftc -O scripts/measure-table-fonts-macos.swift -o /tmp/measure-macos
//   /tmp/measure-macos crates/chartml-chart-table/tests/fixtures/font_scan.txt --write
//
// Then update the row-count and hash pins in `estimate_covers_measured_font_scan`;
// the last pass prints both. Without --write the rows are printed and nothing is
// written. Both the count and the hash are printed either way.
//
// Requires a browser with the UI fonts installed. On Debian/Ubuntu:
//
//   apt-get install -y chromium fonts-dejavu fonts-liberation fonts-noto-core
//
// Chrome must run headless and, on macOS, with --use-mock-keychain
// --password-store=basic so it never prompts for keychain access. On macOS prefer
// `chrome-headless-shell`, which does not open a window:
//
//   CHROME="$HOME/Library/Caches/ms-playwright/chromium_headless_shell-1217/\
//   chrome-headless-shell-mac-arm64/chrome-headless-shell" \
//   node scripts/measure-table-fonts.mjs --write --families system-ui

import { readFileSync, writeFileSync } from "node:fs";
import { spawn } from "node:child_process";
import { createServer } from "node:http";

const FIXTURE = new URL(
  "../crates/chartml-chart-table/tests/fixtures/font_scan.txt",
  import.meta.url,
);
const DEFAULT_FAMILIES = ["DejaVu Sans", "Liberation Sans", "Noto Sans", "Roboto"];
const familyArg = process.argv.indexOf("--families");
const FAMILIES = familyArg === -1
  ? DEFAULT_FAMILIES
  : process.argv[familyArg + 1].split(",").map((f) => f.trim()).filter(Boolean);
const WEIGHTS = { regular: "400", semibold: "600", bold: "700" };

// The Latin scope this ticket sizes columns for: Basic Latin, Latin-1, Latin
// Extended-A/B, Latin Extended Additional, and the common Latin punctuation and
// symbols (dashes, quotes, per-mille, currency, vulgar fractions).
//
// Code points outside this scope are deliberately not measured: they keep the
// legacy estimator's advance, so non-Latin table text lays out exactly as it did
// before this change. Widening it is a separate piece of work.
const RANGES = [
  [0x20, 0x7e], // Basic Latin
  [0xa0, 0xff], // Latin-1 Supplement
  [0x100, 0x24f], // Latin Extended-A and -B
  [0x1e00, 0x1eff], // Latin Extended Additional
  [0x2c60, 0x2c7f], // Latin Extended-C
  [0xa720, 0xa7ff], // Latin Extended-D
];
// Latin Extended-D has unassigned holes; skip code points with no character.

// Common Latin punctuation and symbols outside the letter blocks. This list is
// exactly the non-letter code points in the committed fixture, so running this
// script reproduces that file rather than a superset of it.
const SYMBOLS = [
  0x2002, 0x2003, 0x2009, 0x202f, // General Punctuation spaces
  0x2012, 0x2013, 0x2014, 0x2015, 0x2e3a, 0x2e3b, // dashes
  0x2018, 0x2019, 0x201c, 0x201d, 0x2022, 0x2026, // quotes and bullet
  0x2030, 0x2031, // per-mille, per-ten-thousand
  0x00a2, 0x00a3, 0x00a5, 0x20ac, // currency
  0x00a7, 0x00a9, 0x00ae, 0x00b0, 0x00b1, 0x00b2, 0x00b3, 0x00b6,
  0x00b7, 0x00b9, 0x00ba, 0x00bc, 0x00bd, 0x00be, 0x00d7, 0x00f7,
  0x2116, 0x2122, 0x2212, // numero, trademark, minus sign
  0x2260, 0x2264, 0x2265, // comparison operators
];

// Code points in Unicode 15 with no assigned character, which no font can paint.
const UNASSIGNED = [
  0xa7c0, 0xa7c1, 0xa7cb, 0xa7cc, 0xa7cd, 0xa7ce, 0xa7cf, 0xa7d0, 0xa7d1, 0xa7d2,
  0xa7d3, 0xa7d4, 0xa7d5, 0xa7d6, 0xa7d7, 0xa7d8, 0xa7d9, 0xa7da, 0xa7db, 0xa7dc,
  0xa7dd, 0xa7de, 0xa7df, 0xa7e0, 0xa7e1, 0xa7e2, 0xa7e3, 0xa7e4, 0xa7e5, 0xa7e6,
  0xa7e7, 0xa7e8, 0xa7e9, 0xa7ea, 0xa7eb, 0xa7ec, 0xa7ed, 0xa7ee, 0xa7ef, 0xa7f0,
  0xa7f1, 0xa7f2, 0xa7f3, 0xa7f4,
];

function codePoints() {
  const out = new Set(SYMBOLS);
  const skip = new Set(UNASSIGNED);
  for (const [lo, hi] of RANGES) {
    for (let cp = lo; cp <= hi; cp += 1) {
      if (!skip.has(cp)) out.add(cp);
    }
  }
  return [...out].sort((a, b) => a - b);
}

function tableSymbol(cp) {
  const ch = String.fromCodePoint(cp);
  if (ch === '"') return '\\"';
  if (ch === "\\") return "\\\\";
  if (cp >= 0x20 && cp < 0x7f) return ch;
  return `\\u{${cp.toString(16)}}`;
}

const flags = ["--headless=new", "--no-sandbox", "--disable-gpu", "--disable-dev-shm-usage"];
if (process.platform === "darwin") flags.push("--use-mock-keychain", "--password-store=basic");
const browser = process.env.CHROME || "chromium";

const cps = codePoints();

const page = `<!doctype html><meta charset="utf-8"><script>
const cps = ${JSON.stringify(cps)};
const families = ${JSON.stringify(FAMILIES)};
const weights = ${JSON.stringify(WEIGHTS)};
const out = {};
for (const family of families) {
  out[family] = {};
  for (const [name, weight] of Object.entries(weights)) {
    const ctx = document.createElement("canvas").getContext("2d");
    ctx.font = weight + ' 13px "' + family + '"';
    const row = {};
    for (const cp of cps) row[cp] = ctx.measureText(String.fromCodePoint(cp)).width;
    out[family][name] = row;
  }
}
fetch("/", { method: "POST", body: JSON.stringify(out) });
</script>`;

// Serve the page and let the browser post its measurements back to a local
// server. `--dump-dom` returns before a deferred script has run on some
// builds, which would silently capture a placeholder.
let child;
const posted = await new Promise((resolve, reject) => {
  const server = createServer((req, res) => {
    if (req.method === "POST") {
      let body = "";
      req.on("data", (chunk) => (body += chunk));
      req.on("end", () => {
        res.writeHead(204).end();
        server.close();
        try {
          resolve({ data: JSON.parse(body), child });
        } catch (error) {
          reject(error);
        }
      });
      return;
    }
    res.writeHead(200, { "content-type": "text/html; charset=utf-8" }).end(page);
  });
  server.listen(0, "127.0.0.1", () => {
    const { port } = server.address();
    child = spawn(browser, [...flags, `http://127.0.0.1:${port}/`], { stdio: "ignore" });
    child.on("error", reject);
    setTimeout(() => {
      server.close();
      reject(new Error("the browser did not report measurements within 120s"));
    }, 120_000).unref();
  });
});

// The browser is finished once it has posted; stop it so the script exits.
child?.kill();
const measurements = posted.data;

// Start from what the fixture already holds, so running one pass after another
// takes the maximum across all of them rather than overwriting the earlier ones.
const existing = new Map();
try {
  for (const line of readFileSync(FIXTURE, "utf8").split("\n")) {
    const trimmed = line.trim();
    if (!trimmed || trimmed.startsWith("#")) continue;
    const [hex, regular, bold] = trimmed.split(/\s+/);
    const cp = Number.parseInt(hex, 16);
    if (Number.isFinite(cp)) existing.set(cp, [Number(regular), Number(bold)]);
  }
} catch {
  // No fixture yet: this pass is the first, and starts from zero.
}

const rows = [];
for (const cp of cps) {
  // Header text is drawn at 600 by default but a theme may set 700, so the bold
  // column is the widest of both.
  const widest = (...weights) =>
    Math.max(...FAMILIES.flatMap((family) => weights.map((w) => measurements[family]?.[w]?.[cp] ?? 0)));
  const milli = (px) => Math.max(0, Math.ceil((px / 13) * 1000 - 1e-9));
  const [wasRegular, wasBold] = existing.get(cp) ?? [0, 0];
  const regular = Math.max(wasRegular, milli(widest("regular")));
  const bold = Math.max(wasBold, milli(widest("semibold", "bold")));
  rows.push(`${cp.toString(16)} ${regular} ${bold}`);
}

const header = `# Measured advance widths for the Latin text the table estimator sizes columns
# from: Basic Latin, Latin-1, Latin Extended-A/B/C/D, Latin Extended Additional,
# and the common Latin punctuation and symbols (dashes, quotes, per-mille, minus
# sign, numero, the General Punctuation spaces, currency, fractions).
#
# Format: one code point per line, "<hex codepoint> <regular milli-em> <bold milli-em>",
# where a milli-em is 1/1000 em of the 13px table font, rounded up.
#
# Each value is the widest advance measured for that character across DejaVu Sans,
# Liberation Sans, Noto Sans and Roboto in headless Chromium at 13px, Chrome's
# \`system-ui\` on macOS in headless Chromium at 13px, and the macOS system font
# through AppKit NSFont at 13px: weight 400 for the regular column, the wider of
# 600 and 700 for bold headers. Code points outside this Latin scope are
# deliberately absent: they keep the legacy estimator's estimate (see CHA-42).
#
# Regenerate by running, in any order, scripts/measure-table-fonts.mjs --write,
# the same with --families system-ui, and
# scripts/measure-table-fonts-macos.swift --write: each takes the maximum of its
# own measurement and the file's current value. The last pass prints the row
# count and FNV-1a hash pinned in \`estimate_covers_measured_font_scan\`.
`;
const text = header + rows.join("\n") + "\n";

const fnv = (s) => {
  let h = 0xcbf29ce484222325n;
  for (const byte of Buffer.from(s, "utf8")) {
    h ^= BigInt(byte);
    h = (h * 0x100000001b3n) & 0xffffffffffffffffn;
  }
  return "0x" + h.toString(16).padStart(16, "0");
};

if (process.argv.includes("--write")) {
  writeFileSync(FIXTURE, text, "utf8");
  console.log(`wrote ${FIXTURE.pathname}`);
} else {
  console.log(text);
}
console.log(`rows: ${rows.length}`);
console.log(`hash: ${fnv(text)}`);
