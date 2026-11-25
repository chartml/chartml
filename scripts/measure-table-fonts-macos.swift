// Widen the Latin measurement fixture with the macOS system font's advance
// widths, and optionally write it back.
//
// This script only *raises* cells the fixture already has; it cannot discover the
// code points to measure. So the `.mjs` pass has to run first. The full
// reproduction of the committed fixture is three passes:
//
//   # 1. the four Linux faces, in headless Chromium (Debian/Ubuntu container)
//   node scripts/measure-table-fonts.mjs --write
//   # 2. Chrome's own macOS `system-ui`, in headless Chromium (on a Mac)
//   node scripts/measure-table-fonts.mjs --write --families system-ui
//   # 3. the macOS system font through AppKit (on a Mac)
//   swiftc -O scripts/measure-table-fonts-macos.swift -o /tmp/measure-macos
//   /tmp/measure-macos crates/chartml-chart-table/tests/fixtures/font_scan.txt --write
//
// Each pass takes the maximum of its own measurement and the file's current
// value, so re-running any of them changes nothing. Pass 1 or 2 must come first
// because those are the passes that create the rows for a code point; this one
// only raises rows the file already has, and exits 1 if there are none.
//
// Then update the row-count and hash pins in `estimate_covers_measured_font_scan`;
// the last pass prints both. Without --write the rows are printed and nothing is
// written.
//
// Headers are drawn at weight 600 by default but a theme may set 700, so the bold
// column is the wider of the two.

import AppKit
import Foundation

var arguments = CommandLine.arguments
let write = arguments.contains("--write")
arguments.removeAll { $0 == "--write" }
let fixturePath = arguments.count > 1
    ? arguments[1]
    : "crates/chartml-chart-table/tests/fixtures/font_scan.txt"

// A missing file must fail with a message rather than a Swift runtime trap, and
// so must an empty one: this pass only raises cells that already exist, so either
// means the `.mjs` pass has not run.
guard FileManager.default.fileExists(atPath: fixturePath),
      let contents = try? String(contentsOfFile: fixturePath, encoding: .utf8)
else {
    FileHandle.standardError.write(Data("\(fixturePath) not found: run scripts/measure-table-fonts.mjs --write first, because this pass only raises cells that already exist\n".utf8))
    exit(1)
}
var order: [Int] = []
var rows: [Int: (Int, Int)] = [:]
var header: [String] = []
for line in contents.split(separator: "\n", omittingEmptySubsequences: false) {
    let trimmed = line.trimmingCharacters(in: .whitespaces)
    if trimmed.isEmpty || trimmed.hasPrefix("#") {
        header.append(String(line))
        continue
    }
    let parts = trimmed.split(separator: " ")
    guard parts.count == 3,
          let cp = Int(parts[0], radix: 16),
          let regular = Int(parts[1]),
          let bold = Int(parts[2])
    else { continue }
    order.append(cp)
    rows[cp] = (regular, bold)
}

if order.isEmpty {
    FileHandle.standardError.write(Data("no rows in \(fixturePath): run scripts/measure-table-fonts.mjs --write first, because this pass only raises cells that already exist\n".utf8))
    exit(1)
}

let regular = NSFont.systemFont(ofSize: 13)
let semibold = NSFont.systemFont(ofSize: 13, weight: .semibold)
let bold = NSFont.systemFont(ofSize: 13, weight: .bold)

func milliEm(_ cp: Int, _ font: NSFont) -> Int {
    guard let scalar = Unicode.Scalar(cp) else { return 0 }
    let text = String(Character(scalar))
    let attrs: [NSAttributedString.Key: Any] = [.font: font]
    let px = Double((text as NSString).size(withAttributes: attrs).width)
    return Int((px / 13.0 * 1000.0).rounded(.up))
}

for cp in order {
    let measuredRegular = milliEm(cp, regular)
    let measuredBold = max(milliEm(cp, semibold), milliEm(cp, bold))
    let existing = rows[cp]!
    rows[cp] = (max(existing.0, measuredRegular), max(existing.1, measuredBold))
    if !write {
        print(String(format: "%x %d %d", cp, rows[cp]!.0, rows[cp]!.1))
    }
}

if write {
    var out = header.joined(separator: "\n")
    if !out.hasSuffix("\n") { out += "\n" }
    out += order.map { String(format: "%x %d %d", $0, rows[$0]!.0, rows[$0]!.1) }.joined(separator: "\n")
    out += "\n"
    try out.write(toFile: fixturePath, atomically: true, encoding: .utf8)
    var hash: UInt64 = 0xcbf2_9ce4_8422_2325
    for byte in Array(out.utf8) {
        hash ^= UInt64(byte)
        hash = hash &* 0x0000_0100_0000_01b3
    }
    print("wrote \(fixturePath)")
    print(String(format: "rows: %d", order.count))
    print(String(format: "hash: 0x%016llx", hash))
}
