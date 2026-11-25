//! ChartML `table` chart — tabular renderer that emits an HTML table
//! via `ChartElement::Div`/`Span` nodes, styled from `Theme` table tokens
//! and tagged with stable `chartml-table-*` class names so a consumer's CSS can
//! restyle it.
//!
//! Drop-in: changing `type: bar` to `type: table` on an existing cartesian
//! spec renders the same `columns` + `rows` fields as a table. Column
//! labels come from `FieldSpec.label`, cell formatting from `FieldSpec.format`,
//! and pagination from `visualize.style.pageSize`.

use std::borrow::Cow;
use std::collections::HashMap;

use chartml_core::data::DataTable;
use chartml_core::element::*;
use chartml_core::error::ChartError;
use chartml_core::format::NumberFormatter;
use chartml_core::plugin::{ChartConfig, ChartRenderer};
use chartml_core::spec::{
    ColumnAlign, ColumnWidth, FieldRef, FieldRefItem, FieldSpec, VisualizeSpec,
};
use chartml_core::theme::{TextTransform, Theme};

const DEFAULT_PAGE_SIZE: usize = 50;
const DEFAULT_HEIGHT: f64 = 400.0;

/// A column's minimum text width, in `em` of the table font, comes from its
/// header label and values (see `ColumnText`) and is at most this, so one
/// very long value (a URL, an ID) cannot make the table scroll in a wide
/// container; past it the value wraps. The widest test value
/// ("Northwind Traders International Ltd II") estimates at 19.96em after the
/// slack (19.01em of advances), so 20em lets it fit one line, while a URL or
/// an ID still wraps instead of widening the table without bound.
const MAX_MIN_TEXT_EM: f64 = 20.0;

/// Safety margin over the measured per-character advances
/// (`TABLE_CHAR_MILLI_EM`).
/// The estimator sums advances, while a browser shapes the text (kerning,
/// hinting and subpixel rounding) and a font may run a little wider than the
/// measured faces. 5% is the margin over the widest measured face, and it is
/// spare: the measured advances are themselves the widest of the six sources
/// (four Linux faces, Chrome's macOS `system-ui`, and AppKit), so a glyph has to
/// be wider than every one of them to need it.
///
/// `slack_and_cap_deliver_their_guarantees` pins this, because a corpus test
/// alone cannot: for a measured character the estimate *is* the measurement, so
/// it would pass at any slack at or above 1.0.
const TEXT_WIDTH_SLACK: f64 = 1.05;

/// Width of the right-edge fade that signals more columns off to the right
/// (see `fade_cell`).
const FADE_WIDTH_PX: u32 = 32;

/// Resolved column: the source field name + display label + optional format,
/// width and alignment.
struct Column {
    field: String,
    label: String,
    format: Option<String>,
    width: Option<ColumnWidth>,
    align: Option<ColumnAlign>,
}

impl Column {
    fn plain(field: &str) -> Self {
        Self {
            field: field.to_string(),
            label: field.to_string(),
            format: None,
            width: None,
            align: None,
        }
    }

    /// Returns `None` for range-mark specs, which have no single field name
    /// and don't correspond to a column in a tabular view.
    fn from_field_spec(spec: &FieldSpec) -> Option<Self> {
        let field = spec.field.clone()?;
        Some(Self {
            label: spec.label.clone().unwrap_or_else(|| field.clone()),
            field,
            format: spec.format.clone(),
            width: spec.width,
            align: spec.align,
        })
    }
}

#[derive(Default)]
pub struct TableRenderer;

impl TableRenderer {
    pub fn new() -> Self {
        Self
    }
}

/// Collect columns from `visualize.columns` and `visualize.rows`. The category
/// field (columns) comes first, then each measure (rows) in order. If neither
/// is set, fall back to all fields in the DataTable schema.
fn resolve_columns(viz: &VisualizeSpec, data: &DataTable) -> Vec<Column> {
    let mut out: Vec<Column> = Vec::new();

    if let Some(cols) = &viz.columns {
        append_field_ref(&mut out, cols);
    }
    if let Some(rows) = &viz.rows {
        append_field_ref(&mut out, rows);
    }

    if out.is_empty() {
        // No spec — show every field in the table, in schema order.
        for name in data.field_names() {
            out.push(Column::plain(&name));
        }
    }

    out
}

fn append_field_ref(out: &mut Vec<Column>, field_ref: &FieldRef) {
    match field_ref {
        FieldRef::Simple(name) => out.push(Column::plain(name)),
        FieldRef::Detailed(spec) => {
            if let Some(col) = Column::from_field_spec(spec) {
                out.push(col);
            }
        }
        FieldRef::Multiple(items) => {
            for item in items {
                match item {
                    FieldRefItem::Simple(name) => out.push(Column::plain(name)),
                    FieldRefItem::Detailed(spec) => {
                        if let Some(col) = Column::from_field_spec(spec) {
                            out.push(col);
                        }
                    }
                }
            }
        }
    }
}

/// Whether a column holds numbers, from the Arrow data type of its field.
/// Numeric columns are right-aligned, header included. A field missing from
/// the data is not numeric.
fn is_numeric_column(data: &DataTable, field: &str) -> bool {
    data.schema()
        .field_with_name(field)
        .map(|f| f.data_type().is_numeric())
        .unwrap_or(false)
}

/// Format a cell value for display.
///
/// When an explicit format string is present, numeric values go through
/// `NumberFormatter`. Otherwise `get_string` is used — it returns the
/// type-aware representation for every Arrow type (ISO timestamps, dates,
/// formatted numbers, etc.) so temporal columns are never shown as raw
/// epoch millis.
fn format_cell(data: &DataTable, row: usize, col: &Column) -> String {
    if let Some(fmt) = &col.format {
        if let Some(n) = data.get_f64(row, &col.field) {
            return NumberFormatter::new(fmt).format(n);
        }
    }
    match data.get_string(row, &col.field) {
        Some(s) => s,
        None => "\u{2014}".to_string(),
    }
}

/// Text widths of a column in `em`, from its header label (measured as the
/// header draws it, see `HeaderText`) and its values on the page, each
/// widened by `TEXT_WIDTH_SLACK` and rounded up to 0.01em so output is
/// stable.
#[derive(Debug, Clone, Copy, PartialEq)]
struct ColumnText {
    /// The widest label or value on one line, at most `MAX_MIN_TEXT_EM`.
    line: f64,
    /// The widest single word (a run without spaces, e.g. a number): the
    /// narrowest the column can be without breaking a word, also at most
    /// `MAX_MIN_TEXT_EM` (a longer word breaks).
    word: f64,
}

/// Advance width of one character in the table font, in thousandths of an
/// `em`: `(character, regular, bold)`. Each value is the widest advance
/// measured for that character across the common UI sans fonts — DejaVu Sans,
/// Liberation Sans, Noto Sans and Roboto in headless Chrome at 13px; Chrome's
/// own `system-ui` on macOS in headless Chrome at 13px; and the macOS system
/// font through AppKit at 13px — at weight 400 for values and the wider of 600
/// and 700 for bold headers, rounded up to the thousandth of an em. Each entry
/// is the widest measured face for that glyph.
///
/// The macOS Chrome pass is not redundant beside AppKit: AppKit falls back to
/// New York Regular for glyphs SF Pro lacks, while Chrome falls back to a face
/// with a real bold, so `U+A7AE` paints 397.8 milli-em bold in Chrome against
/// AppKit's 391.7. Measuring only AppKit under-estimated it.
///
/// Column minimums are sized for the fonts the table is drawn with
/// (`system-ui, sans-serif`, `Theme::label_font_family`), not the 12px
/// axis-label estimate in `chartml_core::layout::labels`: on Linux that stack
/// resolves to DejaVu Sans, whose caps, digits and `%` run wider than the
/// macOS-tuned character table. At 13px, bold "NPS" paints 29.77px in DejaVu
/// Sans (the widest of the measured faces), while base's estimate for it — its
/// 7px-per-character table, `* 1.15` and `* 1.10` for bold — was 2.2138em, or
/// 28.78px, so the header did not fit its own minimum and broke mid-word.
/// Taking the widest measured
/// face per glyph keeps every measured font inside the estimate;
/// `TEXT_WIDTH_SLACK` covers shaping and rounding on top.
///
/// Sorted by character for the binary search in `measured_char_em`.
const TABLE_CHAR_MILLI_EM: &[(char, u32, u32)] = &[
    (' ', 318, 349),
    ('!', 401, 457),
    ('"', 472, 563),
    ('#', 838, 838),
    ('$', 637, 696),
    ('%', 951, 1030),
    ('&', 780, 873),
    ('\'', 292, 342),
    ('(', 391, 458),
    (')', 391, 458),
    ('*', 552, 546),
    ('+', 838, 838),
    (',', 318, 380),
    ('-', 466, 479),
    ('.', 318, 380),
    ('/', 413, 414),
    ('0', 637, 696),
    ('1', 637, 696),
    ('2', 637, 696),
    ('3', 637, 696),
    ('4', 638, 696),
    ('5', 637, 696),
    ('6', 637, 696),
    ('7', 637, 696),
    ('8', 637, 696),
    ('9', 637, 696),
    (':', 337, 400),
    (';', 337, 400),
    ('<', 838, 838),
    ('=', 838, 838),
    ('>', 838, 838),
    ('?', 557, 611),
    ('@', 1016, 1000),
    ('A', 685, 774),
    ('B', 687, 763),
    ('C', 723, 736),
    ('D', 771, 831),
    ('E', 667, 684),
    ('F', 611, 684),
    ('G', 778, 821),
    ('H', 752, 837),
    ('I', 339, 390),
    ('J', 552, 596),
    ('K', 667, 775),
    ('L', 563, 638),
    ('M', 908, 996),
    ('N', 761, 837),
    ('O', 788, 851),
    ('P', 667, 733),
    ('Q', 788, 851),
    ('R', 723, 771),
    ('S', 667, 721),
    ('T', 628, 683),
    ('U', 732, 813),
    ('V', 685, 774),
    ('W', 989, 1104),
    ('X', 686, 771),
    ('Y', 667, 725),
    ('Z', 686, 726),
    ('[', 391, 458),
    ('\\', 411, 423),
    (']', 391, 458),
    ('^', 838, 838),
    ('_', 578, 622),
    ('`', 500, 500),
    ('a', 613, 675),
    ('b', 635, 716),
    ('c', 554, 593),
    ('d', 635, 716),
    ('e', 616, 679),
    ('f', 356, 436),
    ('g', 635, 716),
    ('h', 634, 712),
    ('i', 278, 343),
    ('j', 278, 343),
    ('k', 580, 666),
    ('l', 278, 343),
    ('m', 975, 1042),
    ('n', 634, 712),
    ('o', 612, 688),
    ('p', 635, 716),
    ('q', 635, 716),
    ('r', 414, 494),
    ('s', 521, 596),
    ('t', 393, 479),
    ('u', 634, 712),
    ('v', 592, 652),
    ('w', 818, 924),
    ('x', 592, 646),
    ('y', 592, 652),
    ('z', 534, 583),
    ('{', 637, 712),
    ('|', 552, 552),
    ('}', 637, 712),
    ('~', 838, 838),
    ('\u{a0}', 318, 349),
    ('\u{a1}', 401, 457),
    ('\u{a2}', 637, 696),
    ('\u{a3}', 637, 696),
    ('\u{a4}', 714, 693),
    ('\u{a5}', 637, 696),
    ('\u{a6}', 552, 552),
    ('\u{a7}', 614, 667),
    ('\u{a8}', 581, 608),
    ('\u{a9}', 1000, 1000),
    ('\u{aa}', 472, 564),
    ('\u{ab}', 656, 693),
    ('\u{ac}', 838, 838),
    ('\u{ad}', 0, 0),
    ('\u{ae}', 1000, 1000),
    ('\u{af}', 553, 553),
    ('\u{b0}', 500, 500),
    ('\u{b1}', 838, 838),
    ('\u{b2}', 401, 438),
    ('\u{b3}', 401, 438),
    ('\u{b4}', 500, 500),
    ('\u{b5}', 637, 736),
    ('\u{b6}', 656, 667),
    ('\u{b7}', 318, 380),
    ('\u{b8}', 500, 500),
    ('\u{b9}', 401, 438),
    ('\u{ba}', 472, 564),
    ('\u{bb}', 656, 693),
    ('\u{bc}', 970, 1036),
    ('\u{bd}', 970, 1036),
    ('\u{be}', 970, 1036),
    ('\u{bf}', 611, 611),
    ('\u{c0}', 685, 774),
    ('\u{c1}', 685, 774),
    ('\u{c2}', 685, 774),
    ('\u{c3}', 685, 774),
    ('\u{c4}', 685, 774),
    ('\u{c5}', 685, 774),
    ('\u{c6}', 1000, 1085),
    ('\u{c7}', 723, 736),
    ('\u{c8}', 667, 684),
    ('\u{c9}', 667, 684),
    ('\u{ca}', 667, 684),
    ('\u{cb}', 667, 684),
    ('\u{cc}', 339, 390),
    ('\u{cd}', 339, 390),
    ('\u{ce}', 339, 390),
    ('\u{cf}', 339, 390),
    ('\u{d0}', 775, 838),
    ('\u{d1}', 761, 837),
    ('\u{d2}', 788, 851),
    ('\u{d3}', 788, 851),
    ('\u{d4}', 788, 851),
    ('\u{d5}', 788, 851),
    ('\u{d6}', 788, 851),
    ('\u{d7}', 838, 838),
    ('\u{d8}', 788, 851),
    ('\u{d9}', 732, 813),
    ('\u{da}', 732, 813),
    ('\u{db}', 732, 813),
    ('\u{dc}', 732, 813),
    ('\u{dd}', 667, 725),
    ('\u{de}', 667, 738),
    ('\u{df}', 632, 720),
    ('\u{e0}', 613, 675),
    ('\u{e1}', 613, 675),
    ('\u{e2}', 613, 675),
    ('\u{e3}', 613, 675),
    ('\u{e4}', 613, 675),
    ('\u{e5}', 613, 675),
    ('\u{e6}', 982, 1048),
    ('\u{e7}', 554, 593),
    ('\u{e8}', 616, 679),
    ('\u{e9}', 616, 679),
    ('\u{ea}', 616, 679),
    ('\u{eb}', 616, 679),
    ('\u{ec}', 278, 343),
    ('\u{ed}', 278, 343),
    ('\u{ee}', 278, 343),
    ('\u{ef}', 278, 343),
    ('\u{f0}', 612, 688),
    ('\u{f1}', 634, 712),
    ('\u{f2}', 612, 688),
    ('\u{f3}', 612, 688),
    ('\u{f4}', 612, 688),
    ('\u{f5}', 612, 688),
    ('\u{f6}', 612, 688),
    ('\u{f7}', 838, 838),
    ('\u{f8}', 612, 688),
    ('\u{f9}', 634, 712),
    ('\u{fa}', 634, 712),
    ('\u{fb}', 634, 712),
    ('\u{fc}', 634, 712),
    ('\u{fd}', 592, 652),
    ('\u{fe}', 635, 716),
    ('\u{ff}', 592, 652),
    ('\u{100}', 685, 774),
    ('\u{101}', 613, 675),
    ('\u{102}', 685, 774),
    ('\u{103}', 613, 675),
    ('\u{104}', 685, 774),
    ('\u{105}', 613, 675),
    ('\u{106}', 723, 736),
    ('\u{107}', 554, 593),
    ('\u{108}', 723, 736),
    ('\u{109}', 554, 593),
    ('\u{10a}', 723, 736),
    ('\u{10b}', 554, 593),
    ('\u{10c}', 723, 736),
    ('\u{10d}', 554, 593),
    ('\u{10e}', 771, 831),
    ('\u{10f}', 697, 772),
    ('\u{110}', 775, 838),
    ('\u{111}', 637, 716),
    ('\u{112}', 667, 684),
    ('\u{113}', 616, 679),
    ('\u{114}', 667, 684),
    ('\u{115}', 616, 679),
    ('\u{116}', 667, 684),
    ('\u{117}', 616, 679),
    ('\u{118}', 667, 684),
    ('\u{119}', 616, 679),
    ('\u{11a}', 667, 684),
    ('\u{11b}', 616, 679),
    ('\u{11c}', 778, 821),
    ('\u{11d}', 635, 716),
    ('\u{11e}', 778, 821),
    ('\u{11f}', 635, 716),
    ('\u{120}', 778, 821),
    ('\u{121}', 635, 716),
    ('\u{122}', 778, 821),
    ('\u{123}', 635, 716),
    ('\u{124}', 752, 837),
    ('\u{125}', 634, 712),
    ('\u{126}', 917, 974),
    ('\u{127}', 695, 791),
    ('\u{128}', 339, 390),
    ('\u{129}', 278, 343),
    ('\u{12a}', 339, 390),
    ('\u{12b}', 278, 343),
    ('\u{12c}', 339, 390),
    ('\u{12d}', 278, 343),
    ('\u{12e}', 339, 390),
    ('\u{12f}', 278, 343),
    ('\u{130}', 339, 390),
    ('\u{131}', 278, 343),
    ('\u{132}', 824, 908),
    ('\u{133}', 556, 686),
    ('\u{134}', 552, 596),
    ('\u{135}', 278, 343),
    ('\u{136}', 667, 775),
    ('\u{137}', 580, 666),
    ('\u{138}', 580, 666),
    ('\u{139}', 563, 638),
    ('\u{13a}', 278, 343),
    ('\u{13b}', 563, 638),
    ('\u{13c}', 278, 343),
    ('\u{13d}', 563, 638),
    ('\u{13e}', 375, 480),
    ('\u{13f}', 563, 638),
    ('\u{140}', 356, 557),
    ('\u{141}', 563, 643),
    ('\u{142}', 313, 372),
    ('\u{143}', 761, 837),
    ('\u{144}', 634, 712),
    ('\u{145}', 761, 837),
    ('\u{146}', 634, 712),
    ('\u{147}', 761, 837),
    ('\u{148}', 634, 712),
    ('\u{149}', 814, 983),
    ('\u{14a}', 761, 837),
    ('\u{14b}', 634, 712),
    ('\u{14c}', 788, 851),
    ('\u{14d}', 612, 688),
    ('\u{14e}', 788, 851),
    ('\u{14f}', 612, 688),
    ('\u{150}', 788, 851),
    ('\u{151}', 612, 688),
    ('\u{152}', 1130, 1167),
    ('\u{153}', 1023, 1095),
    ('\u{154}', 723, 771),
    ('\u{155}', 414, 494),
    ('\u{156}', 723, 771),
    ('\u{157}', 414, 494),
    ('\u{158}', 723, 771),
    ('\u{159}', 414, 494),
    ('\u{15a}', 667, 721),
    ('\u{15b}', 521, 596),
    ('\u{15c}', 667, 721),
    ('\u{15d}', 521, 596),
    ('\u{15e}', 667, 721),
    ('\u{15f}', 521, 596),
    ('\u{160}', 667, 721),
    ('\u{161}', 521, 596),
    ('\u{162}', 628, 683),
    ('\u{163}', 393, 479),
    ('\u{164}', 628, 683),
    ('\u{165}', 393, 480),
    ('\u{166}', 628, 683),
    ('\u{167}', 393, 479),
    ('\u{168}', 732, 813),
    ('\u{169}', 634, 712),
    ('\u{16a}', 732, 813),
    ('\u{16b}', 634, 712),
    ('\u{16c}', 732, 813),
    ('\u{16d}', 634, 712),
    ('\u{16e}', 732, 813),
    ('\u{16f}', 634, 712),
    ('\u{170}', 732, 813),
    ('\u{171}', 634, 712),
    ('\u{172}', 732, 813),
    ('\u{173}', 634, 712),
    ('\u{174}', 989, 1104),
    ('\u{175}', 818, 924),
    ('\u{176}', 667, 725),
    ('\u{177}', 592, 652),
    ('\u{178}', 667, 725),
    ('\u{179}', 686, 726),
    ('\u{17a}', 534, 583),
    ('\u{17b}', 686, 726),
    ('\u{17c}', 534, 583),
    ('\u{17d}', 686, 726),
    ('\u{17e}', 534, 583),
    ('\u{17f}', 356, 436),
    ('\u{180}', 635, 716),
    ('\u{181}', 765, 824),
    ('\u{182}', 687, 763),
    ('\u{183}', 635, 716),
    ('\u{184}', 687, 763),
    ('\u{185}', 635, 716),
    ('\u{186}', 722, 734),
    ('\u{187}', 708, 734),
    ('\u{188}', 607, 593),
    ('\u{189}', 775, 838),
    ('\u{18a}', 834, 879),
    ('\u{18b}', 687, 757),
    ('\u{18c}', 635, 716),
    ('\u{18d}', 612, 688),
    ('\u{18e}', 632, 684),
    ('\u{18f}', 788, 850),
    ('\u{190}', 615, 696),
    ('\u{191}', 590, 684),
    ('\u{192}', 557, 557),
    ('\u{193}', 775, 821),
    ('\u{194}', 687, 793),
    ('\u{195}', 984, 1045),
    ('\u{196}', 354, 436),
    ('\u{197}', 352, 398),
    ('\u{198}', 746, 775),
    ('\u{199}', 580, 666),
    ('\u{19a}', 341, 390),
    ('\u{19b}', 592, 616),
    ('\u{19c}', 1067, 1118),
    ('\u{19d}', 761, 837),
    ('\u{19e}', 634, 712),
    ('\u{19f}', 788, 851),
    ('\u{1a0}', 914, 874),
    ('\u{1a1}', 617, 688),
    ('\u{1a2}', 1076, 1132),
    ('\u{1a3}', 846, 930),
    ('\u{1a4}', 731, 794),
    ('\u{1a5}', 635, 716),
    ('\u{1a6}', 695, 771),
    ('\u{1a7}', 667, 721),
    ('\u{1a8}', 521, 596),
    ('\u{1a9}', 646, 684),
    ('\u{1aa}', 528, 570),
    ('\u{1ab}', 393, 479),
    ('\u{1ac}', 628, 717),
    ('\u{1ad}', 393, 479),
    ('\u{1ae}', 611, 683),
    ('\u{1af}', 858, 886),
    ('\u{1b0}', 675, 761),
    ('\u{1b1}', 783, 851),
    ('\u{1b2}', 732, 814),
    ('\u{1b3}', 744, 798),
    ('\u{1b4}', 731, 779),
    ('\u{1b5}', 686, 726),
    ('\u{1b6}', 525, 583),
    ('\u{1b7}', 667, 773),
    ('\u{1b8}', 667, 773),
    ('\u{1b9}', 578, 641),
    ('\u{1ba}', 534, 583),
    ('\u{1bb}', 637, 696),
    ('\u{1bc}', 667, 773),
    ('\u{1bd}', 578, 641),
    ('\u{1be}', 512, 573),
    ('\u{1bf}', 635, 716),
    ('\u{1c0}', 337, 373),
    ('\u{1c1}', 556, 659),
    ('\u{1c2}', 517, 544),
    ('\u{1c3}', 296, 373),
    ('\u{1c4}', 1422, 1556),
    ('\u{1c5}', 1299, 1413),
    ('\u{1c6}', 1155, 1298),
    ('\u{1c7}', 1095, 1185),
    ('\u{1c8}', 803, 980),
    ('\u{1c9}', 517, 686),
    ('\u{1ca}', 1269, 1358),
    ('\u{1cb}', 1019, 1180),
    ('\u{1cc}', 877, 1055),
    ('\u{1cd}', 685, 774),
    ('\u{1ce}', 613, 675),
    ('\u{1cf}', 339, 390),
    ('\u{1d0}', 278, 343),
    ('\u{1d1}', 788, 851),
    ('\u{1d2}', 612, 688),
    ('\u{1d3}', 732, 813),
    ('\u{1d4}', 634, 712),
    ('\u{1d5}', 732, 813),
    ('\u{1d6}', 634, 712),
    ('\u{1d7}', 732, 813),
    ('\u{1d8}', 634, 712),
    ('\u{1d9}', 732, 813),
    ('\u{1da}', 634, 712),
    ('\u{1db}', 732, 813),
    ('\u{1dc}', 634, 712),
    ('\u{1dd}', 616, 679),
    ('\u{1de}', 685, 774),
    ('\u{1df}', 613, 675),
    ('\u{1e0}', 685, 774),
    ('\u{1e1}', 613, 675),
    ('\u{1e2}', 975, 1085),
    ('\u{1e3}', 982, 1048),
    ('\u{1e4}', 775, 821),
    ('\u{1e5}', 635, 716),
    ('\u{1e6}', 775, 821),
    ('\u{1e7}', 635, 716),
    ('\u{1e8}', 656, 775),
    ('\u{1e9}', 580, 666),
    ('\u{1ea}', 788, 851),
    ('\u{1eb}', 612, 688),
    ('\u{1ec}', 788, 851),
    ('\u{1ed}', 612, 688),
    ('\u{1ee}', 667, 773),
    ('\u{1ef}', 583, 583),
    ('\u{1f0}', 278, 343),
    ('\u{1f1}', 1422, 1556),
    ('\u{1f2}', 1299, 1413),
    ('\u{1f3}', 1155, 1298),
    ('\u{1f4}', 775, 821),
    ('\u{1f5}', 635, 716),
    ('\u{1f6}', 1113, 1289),
    ('\u{1f7}', 683, 788),
    ('\u{1f8}', 761, 837),
    ('\u{1f9}', 634, 712),
    ('\u{1fa}', 685, 774),
    ('\u{1fb}', 613, 675),
    ('\u{1fc}', 1000, 1085),
    ('\u{1fd}', 982, 1048),
    ('\u{1fe}', 788, 851),
    ('\u{1ff}', 612, 688),
    ('\u{200}', 685, 774),
    ('\u{201}', 613, 675),
    ('\u{202}', 685, 774),
    ('\u{203}', 613, 675),
    ('\u{204}', 632, 684),
    ('\u{205}', 616, 679),
    ('\u{206}', 632, 684),
    ('\u{207}', 616, 679),
    ('\u{208}', 339, 390),
    ('\u{209}', 278, 343),
    ('\u{20a}', 339, 390),
    ('\u{20b}', 278, 343),
    ('\u{20c}', 788, 851),
    ('\u{20d}', 612, 688),
    ('\u{20e}', 788, 851),
    ('\u{20f}', 612, 688),
    ('\u{210}', 695, 771),
    ('\u{211}', 414, 494),
    ('\u{212}', 695, 771),
    ('\u{213}', 414, 494),
    ('\u{214}', 732, 813),
    ('\u{215}', 634, 712),
    ('\u{216}', 732, 813),
    ('\u{217}', 634, 712),
    ('\u{218}', 667, 721),
    ('\u{219}', 521, 596),
    ('\u{21a}', 628, 683),
    ('\u{21b}', 393, 479),
    ('\u{21c}', 664, 691),
    ('\u{21d}', 664, 664),
    ('\u{21e}', 752, 837),
    ('\u{21f}', 634, 712),
    ('\u{220}', 742, 837),
    ('\u{221}', 921, 995),
    ('\u{222}', 771, 833),
    ('\u{223}', 650, 659),
    ('\u{224}', 686, 726),
    ('\u{225}', 574, 593),
    ('\u{226}', 685, 774),
    ('\u{227}', 613, 675),
    ('\u{228}', 632, 684),
    ('\u{229}', 616, 679),
    ('\u{22a}', 788, 851),
    ('\u{22b}', 612, 688),
    ('\u{22c}', 788, 851),
    ('\u{22d}', 612, 688),
    ('\u{22e}', 788, 851),
    ('\u{22f}', 612, 688),
    ('\u{230}', 788, 851),
    ('\u{231}', 612, 688),
    ('\u{232}', 650, 725),
    ('\u{233}', 592, 652),
    ('\u{234}', 569, 661),
    ('\u{235}', 906, 999),
    ('\u{236}', 573, 668),
    ('\u{237}', 278, 343),
    ('\u{238}', 999, 1088),
    ('\u{239}', 999, 1088),
    ('\u{23a}', 685, 774),
    ('\u{23b}', 710, 736),
    ('\u{23c}', 554, 593),
    ('\u{23d}', 611, 667),
    ('\u{23e}', 611, 683),
    ('\u{23f}', 603, 603),
    ('\u{240}', 603, 603),
    ('\u{241}', 604, 782),
    ('\u{242}', 487, 615),
    ('\u{243}', 724, 764),
    ('\u{244}', 847, 833),
    ('\u{245}', 726, 774),
    ('\u{246}', 667, 684),
    ('\u{247}', 616, 679),
    ('\u{248}', 599, 602),
    ('\u{249}', 341, 377),
    ('\u{24a}', 860, 861),
    ('\u{24b}', 635, 792),
    ('\u{24c}', 781, 773),
    ('\u{24d}', 414, 494),
    ('\u{24e}', 772, 767),
    ('\u{24f}', 614, 661),
    ('\u{1e00}', 685, 774),
    ('\u{1e01}', 613, 675),
    ('\u{1e02}', 687, 763),
    ('\u{1e03}', 635, 716),
    ('\u{1e04}', 687, 763),
    ('\u{1e05}', 635, 716),
    ('\u{1e06}', 687, 763),
    ('\u{1e07}', 635, 716),
    ('\u{1e08}', 710, 736),
    ('\u{1e09}', 554, 593),
    ('\u{1e0a}', 771, 831),
    ('\u{1e0b}', 635, 716),
    ('\u{1e0c}', 771, 831),
    ('\u{1e0d}', 635, 716),
    ('\u{1e0e}', 771, 831),
    ('\u{1e0f}', 635, 716),
    ('\u{1e10}', 771, 831),
    ('\u{1e11}', 635, 716),
    ('\u{1e12}', 771, 831),
    ('\u{1e13}', 635, 716),
    ('\u{1e14}', 632, 684),
    ('\u{1e15}', 616, 679),
    ('\u{1e16}', 632, 684),
    ('\u{1e17}', 616, 679),
    ('\u{1e18}', 632, 684),
    ('\u{1e19}', 616, 679),
    ('\u{1e1a}', 632, 684),
    ('\u{1e1b}', 616, 679),
    ('\u{1e1c}', 632, 684),
    ('\u{1e1d}', 616, 679),
    ('\u{1e1e}', 576, 684),
    ('\u{1e1f}', 356, 436),
    ('\u{1e20}', 775, 821),
    ('\u{1e21}', 635, 716),
    ('\u{1e22}', 752, 837),
    ('\u{1e23}', 634, 712),
    ('\u{1e24}', 752, 837),
    ('\u{1e25}', 634, 712),
    ('\u{1e26}', 752, 837),
    ('\u{1e27}', 634, 712),
    ('\u{1e28}', 752, 837),
    ('\u{1e29}', 634, 712),
    ('\u{1e2a}', 752, 837),
    ('\u{1e2b}', 634, 712),
    ('\u{1e2c}', 355, 390),
    ('\u{1e2d}', 351, 376),
    ('\u{1e2e}', 354, 390),
    ('\u{1e2f}', 330, 391),
    ('\u{1e30}', 656, 775),
    ('\u{1e31}', 580, 666),
    ('\u{1e32}', 656, 775),
    ('\u{1e33}', 580, 666),
    ('\u{1e34}', 656, 775),
    ('\u{1e35}', 580, 666),
    ('\u{1e36}', 563, 638),
    ('\u{1e37}', 288, 343),
    ('\u{1e38}', 563, 638),
    ('\u{1e39}', 288, 343),
    ('\u{1e3a}', 563, 638),
    ('\u{1e3b}', 278, 343),
    ('\u{1e3c}', 563, 638),
    ('\u{1e3d}', 278, 343),
    ('\u{1e3e}', 908, 996),
    ('\u{1e3f}', 975, 1042),
    ('\u{1e40}', 908, 996),
    ('\u{1e41}', 975, 1042),
    ('\u{1e42}', 908, 996),
    ('\u{1e43}', 975, 1042),
    ('\u{1e44}', 761, 837),
    ('\u{1e45}', 634, 712),
    ('\u{1e46}', 761, 837),
    ('\u{1e47}', 634, 712),
    ('\u{1e48}', 761, 837),
    ('\u{1e49}', 634, 712),
    ('\u{1e4a}', 761, 837),
    ('\u{1e4b}', 634, 712),
    ('\u{1e4c}', 788, 851),
    ('\u{1e4d}', 612, 688),
    ('\u{1e4e}', 788, 851),
    ('\u{1e4f}', 612, 688),
    ('\u{1e50}', 788, 851),
    ('\u{1e51}', 612, 688),
    ('\u{1e52}', 788, 851),
    ('\u{1e53}', 612, 688),
    ('\u{1e54}', 631, 733),
    ('\u{1e55}', 635, 716),
    ('\u{1e56}', 631, 733),
    ('\u{1e57}', 635, 716),
    ('\u{1e58}', 695, 771),
    ('\u{1e59}', 414, 494),
    ('\u{1e5a}', 695, 771),
    ('\u{1e5b}', 414, 494),
    ('\u{1e5c}', 695, 771),
    ('\u{1e5d}', 414, 494),
    ('\u{1e5e}', 695, 771),
    ('\u{1e5f}', 414, 494),
    ('\u{1e60}', 635, 721),
    ('\u{1e61}', 521, 596),
    ('\u{1e62}', 635, 721),
    ('\u{1e63}', 521, 596),
    ('\u{1e64}', 635, 721),
    ('\u{1e65}', 521, 596),
    ('\u{1e66}', 635, 721),
    ('\u{1e67}', 521, 596),
    ('\u{1e68}', 635, 721),
    ('\u{1e69}', 521, 596),
    ('\u{1e6a}', 628, 683),
    ('\u{1e6b}', 393, 479),
    ('\u{1e6c}', 628, 683),
    ('\u{1e6d}', 393, 479),
    ('\u{1e6e}', 628, 683),
    ('\u{1e6f}', 393, 479),
    ('\u{1e70}', 628, 683),
    ('\u{1e71}', 393, 479),
    ('\u{1e72}', 732, 813),
    ('\u{1e73}', 634, 712),
    ('\u{1e74}', 732, 813),
    ('\u{1e75}', 634, 712),
    ('\u{1e76}', 732, 813),
    ('\u{1e77}', 634, 712),
    ('\u{1e78}', 732, 813),
    ('\u{1e79}', 634, 712),
    ('\u{1e7a}', 732, 813),
    ('\u{1e7b}', 634, 712),
    ('\u{1e7c}', 685, 774),
    ('\u{1e7d}', 592, 652),
    ('\u{1e7e}', 685, 774),
    ('\u{1e7f}', 592, 652),
    ('\u{1e80}', 989, 1104),
    ('\u{1e81}', 818, 924),
    ('\u{1e82}', 989, 1104),
    ('\u{1e83}', 818, 924),
    ('\u{1e84}', 989, 1104),
    ('\u{1e85}', 818, 924),
    ('\u{1e86}', 989, 1104),
    ('\u{1e87}', 818, 924),
    ('\u{1e88}', 989, 1104),
    ('\u{1e89}', 818, 924),
    ('\u{1e8a}', 686, 771),
    ('\u{1e8b}', 592, 646),
    ('\u{1e8c}', 686, 771),
    ('\u{1e8d}', 592, 646),
    ('\u{1e8e}', 650, 725),
    ('\u{1e8f}', 592, 652),
    ('\u{1e90}', 686, 726),
    ('\u{1e91}', 534, 583),
    ('\u{1e92}', 686, 726),
    ('\u{1e93}', 534, 583),
    ('\u{1e94}', 686, 726),
    ('\u{1e95}', 534, 583),
    ('\u{1e96}', 634, 712),
    ('\u{1e97}', 393, 479),
    ('\u{1e98}', 818, 924),
    ('\u{1e99}', 592, 652),
    ('\u{1e9a}', 613, 675),
    ('\u{1e9b}', 356, 436),
    ('\u{1e9c}', 362, 436),
    ('\u{1e9d}', 353, 436),
    ('\u{1e9e}', 769, 896),
    ('\u{1e9f}', 612, 688),
    ('\u{1ea0}', 685, 774),
    ('\u{1ea1}', 613, 675),
    ('\u{1ea2}', 685, 774),
    ('\u{1ea3}', 613, 675),
    ('\u{1ea4}', 685, 774),
    ('\u{1ea5}', 613, 675),
    ('\u{1ea6}', 685, 774),
    ('\u{1ea7}', 613, 675),
    ('\u{1ea8}', 685, 774),
    ('\u{1ea9}', 613, 675),
    ('\u{1eaa}', 685, 774),
    ('\u{1eab}', 613, 675),
    ('\u{1eac}', 685, 774),
    ('\u{1ead}', 613, 675),
    ('\u{1eae}', 685, 774),
    ('\u{1eaf}', 613, 675),
    ('\u{1eb0}', 685, 774),
    ('\u{1eb1}', 613, 675),
    ('\u{1eb2}', 685, 774),
    ('\u{1eb3}', 613, 675),
    ('\u{1eb4}', 685, 774),
    ('\u{1eb5}', 613, 675),
    ('\u{1eb6}', 685, 774),
    ('\u{1eb7}', 613, 675),
    ('\u{1eb8}', 632, 684),
    ('\u{1eb9}', 616, 679),
    ('\u{1eba}', 632, 684),
    ('\u{1ebb}', 616, 679),
    ('\u{1ebc}', 632, 684),
    ('\u{1ebd}', 616, 679),
    ('\u{1ebe}', 632, 684),
    ('\u{1ebf}', 616, 679),
    ('\u{1ec0}', 632, 684),
    ('\u{1ec1}', 616, 679),
    ('\u{1ec2}', 632, 684),
    ('\u{1ec3}', 616, 679),
    ('\u{1ec4}', 632, 684),
    ('\u{1ec5}', 616, 679),
    ('\u{1ec6}', 632, 684),
    ('\u{1ec7}', 616, 679),
    ('\u{1ec8}', 339, 390),
    ('\u{1ec9}', 278, 343),
    ('\u{1eca}', 339, 390),
    ('\u{1ecb}', 278, 343),
    ('\u{1ecc}', 788, 851),
    ('\u{1ecd}', 612, 688),
    ('\u{1ece}', 788, 851),
    ('\u{1ecf}', 612, 688),
    ('\u{1ed0}', 788, 851),
    ('\u{1ed1}', 612, 688),
    ('\u{1ed2}', 788, 851),
    ('\u{1ed3}', 612, 688),
    ('\u{1ed4}', 788, 851),
    ('\u{1ed5}', 612, 688),
    ('\u{1ed6}', 788, 851),
    ('\u{1ed7}', 612, 688),
    ('\u{1ed8}', 788, 851),
    ('\u{1ed9}', 612, 688),
    ('\u{1eda}', 914, 874),
    ('\u{1edb}', 617, 688),
    ('\u{1edc}', 914, 874),
    ('\u{1edd}', 617, 688),
    ('\u{1ede}', 914, 874),
    ('\u{1edf}', 617, 688),
    ('\u{1ee0}', 914, 874),
    ('\u{1ee1}', 617, 688),
    ('\u{1ee2}', 914, 874),
    ('\u{1ee3}', 617, 688),
    ('\u{1ee4}', 732, 813),
    ('\u{1ee5}', 634, 712),
    ('\u{1ee6}', 732, 813),
    ('\u{1ee7}', 634, 712),
    ('\u{1ee8}', 858, 886),
    ('\u{1ee9}', 675, 761),
    ('\u{1eea}', 858, 886),
    ('\u{1eeb}', 675, 761),
    ('\u{1eec}', 858, 886),
    ('\u{1eed}', 675, 761),
    ('\u{1eee}', 858, 886),
    ('\u{1eef}', 675, 761),
    ('\u{1ef0}', 858, 886),
    ('\u{1ef1}', 675, 761),
    ('\u{1ef2}', 667, 725),
    ('\u{1ef3}', 592, 652),
    ('\u{1ef4}', 650, 725),
    ('\u{1ef5}', 592, 652),
    ('\u{1ef6}', 650, 725),
    ('\u{1ef7}', 592, 652),
    ('\u{1ef8}', 650, 725),
    ('\u{1ef9}', 592, 652),
    ('\u{1efa}', 937, 953),
    ('\u{1efb}', 590, 644),
    ('\u{1efc}', 734, 749),
    ('\u{1efd}', 593, 601),
    ('\u{1efe}', 657, 705),
    ('\u{1eff}', 559, 612),
    ('\u{2002}', 510, 510),
    ('\u{2003}', 1021, 1021),
    ('\u{2009}', 205, 205),
    ('\u{2012}', 637, 696),
    ('\u{2013}', 657, 632),
    ('\u{2014}', 1001, 1001),
    ('\u{2015}', 1001, 1556),
    ('\u{2018}', 318, 380),
    ('\u{2019}', 318, 380),
    ('\u{201c}', 519, 658),
    ('\u{201d}', 519, 658),
    ('\u{2022}', 590, 640),
    ('\u{2026}', 1000, 1000),
    ('\u{202f}', 249, 250),
    ('\u{2030}', 1342, 1459),
    ('\u{2031}', 1736, 1887),
    ('\u{20ac}', 637, 696),
    ('\u{2116}', 1155, 1217),
    ('\u{2122}', 1000, 1000),
    ('\u{2212}', 838, 838),
    ('\u{2260}', 838, 838),
    ('\u{2264}', 838, 838),
    ('\u{2265}', 838, 838),
    ('\u{2c60}', 611, 667),
    ('\u{2c61}', 351, 477),
    ('\u{2c62}', 633, 638),
    ('\u{2c63}', 691, 733),
    ('\u{2c64}', 722, 771),
    ('\u{2c65}', 613, 675),
    ('\u{2c66}', 393, 479),
    ('\u{2c67}', 786, 957),
    ('\u{2c68}', 634, 712),
    ('\u{2c69}', 723, 778),
    ('\u{2c6a}', 580, 666),
    ('\u{2c6b}', 686, 726),
    ('\u{2c6c}', 525, 583),
    ('\u{2c6d}', 898, 898),
    ('\u{2c6e}', 896, 996),
    ('\u{2c6f}', 685, 774),
    ('\u{2c70}', 898, 898),
    ('\u{2c71}', 735, 779),
    ('\u{2c72}', 1128, 1221),
    ('\u{2c73}', 962, 1057),
    ('\u{2c74}', 592, 652),
    ('\u{2c75}', 660, 698),
    ('\u{2c76}', 568, 565),
    ('\u{2c77}', 726, 793),
    ('\u{2c78}', 689, 722),
    ('\u{2c79}', 603, 603),
    ('\u{2c7a}', 612, 688),
    ('\u{2c7b}', 514, 559),
    ('\u{2c7c}', 603, 603),
    ('\u{2c7d}', 603, 603),
    ('\u{2c7e}', 635, 721),
    ('\u{2c7f}', 686, 726),
    ('\u{2e3a}', 1921, 1921),
    ('\u{2e3b}', 2841, 2841),
    ('\u{a720}', 665, 627),
    ('\u{a721}', 665, 627),
    ('\u{a722}', 420, 444),
    ('\u{a723}', 372, 391),
    ('\u{a724}', 533, 541),
    ('\u{a725}', 472, 541),
    ('\u{a726}', 752, 837),
    ('\u{a727}', 634, 712),
    ('\u{a728}', 903, 1032),
    ('\u{a729}', 809, 930),
    ('\u{a72a}', 615, 696),
    ('\u{a72b}', 572, 582),
    ('\u{a72c}', 556, 572),
    ('\u{a72d}', 538, 550),
    ('\u{a72e}', 675, 675),
    ('\u{a72f}', 674, 674),
    ('\u{a730}', 491, 559),
    ('\u{a731}', 521, 596),
    ('\u{a732}', 1352, 1352),
    ('\u{a733}', 985, 1052),
    ('\u{a734}', 1229, 1285),
    ('\u{a735}', 990, 1064),
    ('\u{a736}', 1183, 1216),
    ('\u{a737}', 981, 1055),
    ('\u{a738}', 1048, 1080),
    ('\u{a739}', 818, 922),
    ('\u{a73a}', 1048, 1080),
    ('\u{a73b}', 818, 922),
    ('\u{a73c}', 1053, 1053),
    ('\u{a73d}', 818, 922),
    ('\u{a73e}', 704, 699),
    ('\u{a73f}', 550, 550),
    ('\u{a740}', 727, 727),
    ('\u{a741}', 602, 689),
    ('\u{a742}', 628, 665),
    ('\u{a743}', 535, 621),
    ('\u{a744}', 727, 727),
    ('\u{a745}', 602, 621),
    ('\u{a746}', 691, 850),
    ('\u{a747}', 393, 542),
    ('\u{a748}', 584, 683),
    ('\u{a749}', 427, 532),
    ('\u{a74a}', 909, 918),
    ('\u{a74b}', 792, 814),
    ('\u{a74c}', 937, 959),
    ('\u{a74d}', 756, 756),
    ('\u{a74e}', 1358, 1407),
    ('\u{a74f}', 1019, 1106),
    ('\u{a750}', 668, 733),
    ('\u{a751}', 667, 716),
    ('\u{a752}', 744, 949),
    ('\u{a753}', 784, 938),
    ('\u{a754}', 889, 927),
    ('\u{a755}', 902, 924),
    ('\u{a756}', 804, 851),
    ('\u{a757}', 667, 716),
    ('\u{a758}', 982, 982),
    ('\u{a759}', 815, 815),
    ('\u{a75a}', 606, 611),
    ('\u{a75b}', 475, 544),
    ('\u{a75c}', 645, 712),
    ('\u{a75d}', 626, 660),
    ('\u{a75e}', 776, 776),
    ('\u{a75f}', 622, 622),
    ('\u{a760}', 941, 968),
    ('\u{a761}', 836, 857),
    ('\u{a762}', 564, 604),
    ('\u{a763}', 573, 600),
    ('\u{a764}', 642, 738),
    ('\u{a765}', 655, 716),
    ('\u{a766}', 641, 738),
    ('\u{a767}', 651, 716),
    ('\u{a768}', 573, 703),
    ('\u{a769}', 521, 588),
    ('\u{a76a}', 563, 563),
    ('\u{a76b}', 546, 546),
    ('\u{a76c}', 550, 650),
    ('\u{a76d}', 548, 641),
    ('\u{a76e}', 605, 652),
    ('\u{a76f}', 605, 618),
    ('\u{a770}', 474, 474),
    ('\u{a771}', 793, 842),
    ('\u{a772}', 439, 513),
    ('\u{a773}', 1105, 1194),
    ('\u{a774}', 801, 869),
    ('\u{a775}', 503, 650),
    ('\u{a776}', 755, 813),
    ('\u{a777}', 706, 862),
    ('\u{a778}', 549, 562),
    ('\u{a779}', 778, 797),
    ('\u{a77a}', 600, 620),
    ('\u{a77b}', 584, 581),
    ('\u{a77c}', 434, 455),
    ('\u{a77d}', 612, 643),
    ('\u{a77e}', 612, 620),
    ('\u{a77f}', 597, 597),
    ('\u{a780}', 558, 638),
    ('\u{a781}', 278, 343),
    ('\u{a782}', 800, 837),
    ('\u{a783}', 688, 712),
    ('\u{a784}', 488, 547),
    ('\u{a785}', 416, 455),
    ('\u{a786}', 668, 670),
    ('\u{a787}', 566, 566),
    ('\u{a788}', 591, 591),
    ('\u{a789}', 337, 400),
    ('\u{a78a}', 603, 603),
    ('\u{a78b}', 401, 457),
    ('\u{a78c}', 275, 307),
    ('\u{a78d}', 738, 809),
    ('\u{a78e}', 547, 694),
    ('\u{a78f}', 1101, 1101),
    ('\u{a790}', 773, 928),
    ('\u{a791}', 667, 769),
    ('\u{a792}', 706, 717),
    ('\u{a793}', 631, 631),
    ('\u{a794}', 1101, 1101),
    ('\u{a795}', 1101, 1101),
    ('\u{a796}', 1101, 1101),
    ('\u{a797}', 1101, 1101),
    ('\u{a798}', 1101, 1101),
    ('\u{a799}', 1101, 1101),
    ('\u{a79a}', 1101, 1101),
    ('\u{a79b}', 1101, 1101),
    ('\u{a79c}', 1101, 1101),
    ('\u{a79d}', 1101, 1101),
    ('\u{a79e}', 1101, 1101),
    ('\u{a79f}', 1101, 1101),
    ('\u{a7a0}', 775, 821),
    ('\u{a7a1}', 716, 716),
    ('\u{a7a2}', 734, 775),
    ('\u{a7a3}', 580, 666),
    ('\u{a7a4}', 803, 837),
    ('\u{a7a5}', 639, 712),
    ('\u{a7a6}', 695, 771),
    ('\u{a7a7}', 442, 494),
    ('\u{a7a8}', 712, 721),
    ('\u{a7a9}', 585, 629),
    ('\u{a7aa}', 863, 888),
    ('\u{a7ab}', 1101, 1101),
    ('\u{a7ac}', 1101, 1101),
    ('\u{a7ad}', 1101, 1101),
    ('\u{a7ae}', 351, 398),
    ('\u{a7af}', 633, 671),
    ('\u{a7b0}', 1101, 1101),
    ('\u{a7b1}', 1101, 1101),
    ('\u{a7b2}', 1101, 1101),
    ('\u{a7b3}', 1101, 1101),
    ('\u{a7b4}', 1101, 1101),
    ('\u{a7b5}', 632, 661),
    ('\u{a7b6}', 1101, 1101),
    ('\u{a7b7}', 1101, 1101),
    ('\u{a7b8}', 1101, 1101),
    ('\u{a7b9}', 1101, 1101),
    ('\u{a7ba}', 1101, 1101),
    ('\u{a7bb}', 1101, 1101),
    ('\u{a7bc}', 1101, 1101),
    ('\u{a7bd}', 1101, 1101),
    ('\u{a7be}', 1101, 1101),
    ('\u{a7bf}', 1101, 1101),
    ('\u{a7c2}', 1101, 1139),
    ('\u{a7c3}', 1101, 1101),
    ('\u{a7c4}', 1101, 1101),
    ('\u{a7c5}', 1101, 1101),
    ('\u{a7c6}', 1101, 1101),
    ('\u{a7c7}', 1101, 1101),
    ('\u{a7c8}', 1101, 1101),
    ('\u{a7c9}', 1101, 1101),
    ('\u{a7ca}', 1101, 1101),
    ('\u{a7f5}', 1101, 1101),
    ('\u{a7f6}', 1101, 1101),
    ('\u{a7f7}', 1101, 1101),
    ('\u{a7f8}', 625, 614),
    ('\u{a7f9}', 1013, 1013),
    ('\u{a7fa}', 916, 1063),
    ('\u{a7fb}', 656, 684),
    ('\u{a7fc}', 656, 733),
    ('\u{a7fd}', 991, 996),
    ('\u{a7fe}', 399, 399),
    ('\u{a7ff}', 1208, 1326),
];

/// Advance width of `ch` in `em` of the table font, if the measured Latin table
/// has one (see `TABLE_CHAR_MILLI_EM`). `None` means the character is outside the
/// measured Latin scope, and `text_em` then sizes the whole string with
/// `legacy_text_em`.
fn measured_char_em(ch: char, bold: bool) -> Option<f64> {
    let i = TABLE_CHAR_MILLI_EM.binary_search_by_key(&ch, |&(measured, _, _)| measured).ok()?;
    let (_, regular, bold_milli) = TABLE_CHAR_MILLI_EM[i];
    Some(f64::from(if bold { bold_milli } else { regular }) / 1000.0)
}

/// Advance width of a character the measured Latin table does not cover, in
/// `px` of the legacy 12px calibration: a typical glyph 7px, a narrow one 4px,
/// `fr t` 5px and `MWmw` 9px. `legacy_text_em` composes these into the width base
/// charged, which is what text outside this ticket's Latin scope keeps.
fn legacy_char_px(ch: char) -> f64 {
    match ch {
        'M' | 'W' | 'm' | 'w' => 9.0,
        'i' | 'l' | 'j' | '!' | '|' | '.' | ',' | ':' | ';' | '\'' => 4.0,
        'f' | 'r' | 't' => 5.0,
        ' ' => 4.0,
        _ => 7.0,
    }
}

/// The legacy estimator's finished width for `text`, in `em`, composed in base's
/// exact operation order and including base's own 1.15 widening.
///
/// Base's chain, step for step (`chartml_core::layout::labels::measure_text` at a
/// 12px font, then the table's `column_text`):
///
/// 1. sum the 12px character table, in px;
/// 2. `*= 1.10` when the theme uppercases;
/// 3. `+= n * (letter_spacing_em * 12.0)` — base formed `letter_spacing_px`
///    first and multiplied by the character count second, and f64 addition is
///    not associative, so the order matters at the 1e-16 level;
/// 4. `*= 1.10` for a bold header (base's `weight_scale`);
/// 5. `w / 12.0 * 1.15` — base's `em` without the `* 100.0` and ceiling, which
///    `column_text` does once for every route.
///
/// This is written as base wrote it rather than algebraically simplified for
/// exactly that reason: it yields bit-identical values, and a 0.01em boundary in
/// the ceiling below turns any floating-point difference into a visible one.
///
/// A string that contains any character outside the measured Latin table is
/// charged this, so text outside this ticket's scope lays out exactly as it did
/// before — proven in `non_latin_text_keeps_the_legacy_estimate`.
fn legacy_text_em(text: &str, header: &HeaderText) -> f64 {
    let transformed = transform(text, &header.transform);
    let chars: Vec<char> = transformed.chars().collect();
    let mut width: f64 = chars.iter().map(|c| legacy_char_px(*c)).sum();
    if matches!(header.transform, TextTransform::Uppercase) {
        width *= LEGACY_UPPERCASE_SCALE;
    }
    width += chars.len() as f64 * (header.letter_spacing_em * LEGACY_CALIBRATION_PX);
    if header.bold {
        width *= LEGACY_BOLD_SCALE;
    }
    width / LEGACY_CALIBRATION_PX * LEGACY_SLACK
}

/// Apply a header's CSS `text-transform`.
fn transform<'a>(text: &'a str, transform: &TextTransform) -> Cow<'a, str> {
    match transform {
        TextTransform::None => Cow::Borrowed(text),
        TextTransform::Uppercase => Cow::Owned(text.to_uppercase()),
        TextTransform::Lowercase => Cow::Owned(text.to_lowercase()),
    }
}

/// The legacy estimator's calibration and widening (see `legacy_text_em`).
const LEGACY_CALIBRATION_PX: f64 = 12.0;
const LEGACY_SLACK: f64 = 1.15;
const LEGACY_BOLD_SCALE: f64 = 1.10;
const LEGACY_UPPERCASE_SCALE: f64 = 1.10;

fn column_text<'a>(label: &str, header: &HeaderText, values: impl Iterator<Item = &'a str>) -> ColumnText {
    let body = HeaderText::body();
    let em = |w: f64| (w * 100.0).ceil() / 100.0;
    // A string's words are sized with the same estimator as the string itself
    // (see `word_em`), so a word never uses a different estimate from the line
    // it came from.
    let label_line = em(text_em(label, header));
    let label_word = label.split_whitespace().map(|w| em(word_em(w, label, header))).fold(0.0, f64::max);
    let mut line = label_line;
    let mut word = label_word;
    for v in values {
        line = line.max(em(text_em(v, &body)));
        word = v
            .split_whitespace()
            .map(|w| em(word_em(w, v, &body)))
            .fold(word, f64::max);
    }
    ColumnText { line: line.min(MAX_MIN_TEXT_EM), word: word.min(MAX_MIN_TEXT_EM) }
}

/// How header labels are drawn, for measuring them: the theme's CSS
/// `text-transform` and `letter-spacing` (`normal`, `<n>em`, or `<n>px`
/// with a px table font size; any other form adds no spacing) are applied to
/// the estimate, and a bold font weight (`600` and up, `bold`, `bolder`)
/// selects the bold advances in `TABLE_CHAR_MILLI_EM`.
struct HeaderText {
    transform: TextTransform,
    letter_spacing_em: f64,
    bold: bool,
}

impl HeaderText {
    /// Values in the body: regular weight, no transform, no letter spacing.
    fn body() -> Self {
        Self {
            transform: TextTransform::None,
            letter_spacing_em: 0.0,
            bold: false,
        }
    }
}

fn header_text(theme: &Theme) -> HeaderText {
    let transform = match theme.table_header_text_transform.trim() {
        "uppercase" => TextTransform::Uppercase,
        "lowercase" => TextTransform::Lowercase,
        _ => TextTransform::None,
    };
    let px = |v: &str| v.trim().strip_suffix("px").and_then(|n| n.trim().parse::<f64>().ok());
    let spacing = theme.table_header_letter_spacing.trim();
    let letter_spacing_em = if let Some(n) = spacing.strip_suffix("em").and_then(|n| n.trim().parse::<f64>().ok()) {
        n
    } else if let (Some(n), Some(font)) = (px(spacing), px(&theme.table_font_size)) {
        n / font
    } else {
        0.0
    };
    let weight = theme.table_header_font_weight.trim();
    let bold = matches!(weight, "bold" | "bolder") || weight.parse::<f64>().is_ok_and(|w| w >= 600.0);
    HeaderText { transform, letter_spacing_em, bold }
}

/// Width of `text` in `em` of the table font, margin included, with the header's
/// transform and letter spacing applied (`HeaderText::body()` applies neither).
/// `column_text` only rounds this up to 0.01em and applies the cap.
///
/// A string is sized by one estimator as a whole: the measured table when every
/// character of the text *and* of its transform has a measured advance, and
/// base's legacy arithmetic otherwise. Routing on the text and its transform
/// together keeps a theme from switching estimators — `U+026A` has no
/// measurement of its own while its uppercase form does, so transforming first
/// would make `uppercase` narrower than `none`. See
/// `sizes_with_measured_table` and `word_em`, which applies the same decision to
/// every word of the string.
///
/// The two routes carry their own margin, because they answer different
/// questions: the measured route multiplies by `TEXT_WIDTH_SLACK` over advances
/// that are already the widest measured face, while the legacy route is base's
/// own `* 1.15` and must not be widened again or it would stop equalling base.
///
/// Base had no special handling for emoji sequences, keycaps, flags or
/// zero-width joiners, and neither does this: a joined sequence is charged its
/// members' advances, which over-estimates a composed glyph (wider column, never
/// broken text) and is exactly base's behaviour, so no cell regresses against it.
fn text_em(text: &str, header: &HeaderText) -> f64 {
    if sizes_with_measured_table(text, header) {
        measured_text_em(text, header) * TEXT_WIDTH_SLACK
    } else {
        legacy_text_em(text, header)
    }
}

/// Whether `text` is sized from the measured table rather than base's legacy
/// arithmetic: true only when every character of the text, and every character
/// after the theme's transform, has a measured advance.
///
/// This is decided per *string*, not per word. A word takes its width from the
/// same answer as the string it came from, so a Latin-only word inside non-Latin
/// text (`(¥)` in `売上 (¥)`) keeps base's estimate instead of shrinking to its
/// measured advance. Base sized the whole cell and each of its words with one
/// estimator, and the column minimum uses both the line and the widest word, so
/// splitting the decision would make the word minimum narrower than base and wrap
/// text that base kept on one line.
fn sizes_with_measured_table(text: &str, header: &HeaderText) -> bool {
    text.chars().all(|c| measured_char_em(c, header.bold).is_some())
        && transform(text, &header.transform)
            .chars()
            .all(|c| measured_char_em(c, header.bold).is_some())
}

/// Width of a fully measured string in `em`: the sum of its measured advances
/// plus the theme's letter-spacing. Only valid when `sizes_with_measured_table`
/// is true for the string it was called with.
fn measured_text_em(text: &str, header: &HeaderText) -> f64 {
    let transformed = transform(text, &header.transform);
    let width: f64 = transformed
        .chars()
        .map(|c| measured_char_em(c, header.bold).expect("every character is measured"))
        .sum();
    width + transformed.chars().count() as f64 * header.letter_spacing_em
}

/// Width in `em` of one whitespace-separated word of `text`, sized with the same
/// estimator as `text` itself (see `sizes_with_measured_table`).
fn word_em(word: &str, text: &str, header: &HeaderText) -> f64 {
    if sizes_with_measured_table(text, header) {
        measured_text_em(word, header) * TEXT_WIDTH_SLACK
    } else {
        legacy_text_em(word, header)
    }
}

/// A column's explicit CSS `text-align` value, if it sets one.
fn align_css(align: ColumnAlign) -> &'static str {
    match align {
        ColumnAlign::Left => "left",
        ColumnAlign::Center => "center",
        ColumnAlign::Right => "right",
    }
}

/// Left and right components of a CSS `padding` shorthand (1–4 values).
fn horizontal_padding(shorthand: &str) -> (String, String) {
    let parts: Vec<&str> = shorthand.split_whitespace().collect();
    match parts.as_slice() {
        [] => ("0px".into(), "0px".into()),
        [all] => ((*all).into(), (*all).into()),
        [_, x] | [_, x, _] => ((*x).into(), (*x).into()),
        [_, right, _, left, ..] => ((*left).into(), (*right).into()),
    }
}

fn style(pairs: &[(&str, &str)]) -> HashMap<String, String> {
    pairs
        .iter()
        .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
        .collect()
}

/// A header or row background that is always opaque: its own colour (which
/// a theme may make transparent) layered over the opaque colour of the
/// surface the table sits on. The fade takes its row's background
/// (`inherit`), so it is opaque too and scrolled cells never show through
/// it. A consumer rule that replaces a row's background (e.g. a hover
/// colour) carries through to it.
fn opaque_bg(colour: &str, theme: &Theme) -> String {
    format!(
        "linear-gradient({colour}, {colour}), var(--chartml-table-surface, {})",
        theme.table_surface
    )
}

/// Grid placement of a header or body cell. Each row is its own grid with
/// the same column tracks, so header and body always line up. The last
/// column also spans the fade's track (see `fade_cell`).
struct CellPlacement {
    /// 1-based grid column.
    column: usize,
    last: bool,
    /// CSS `text-align` value.
    align: &'static str,
}

impl CellPlacement {
    fn grid_column(&self) -> String {
        if self.last {
            format!("{} / span 2", self.column)
        } else {
            self.column.to_string()
        }
    }
}

/// Styles common to header and body cells.
///
/// Cell padding comes from `--chartml-table-cell-padding`, with the theme's
/// `table_cell_padding` as its default — the same way every other
/// `--chartml-table-*` token is read. The last cell is as wide as its column
/// plus the fade's strip (so its border runs to the row's end); a transparent
/// right border of the strip's width keeps its content clear of the strip
/// without overriding the padding token.
fn cell_style(
    place: &CellPlacement,
    theme: &Theme,
    border_bottom: &str,
    fade_width: &str,
) -> HashMap<String, String> {
    let padding = format!("var(--chartml-table-cell-padding, {})", theme.table_cell_padding);
    let mut map = style(&[
        ("grid-column", &place.grid_column()),
        ("grid-row", "1"),
        ("min-width", "0"),
        ("box-sizing", "border-box"),
        ("padding", &padding),
        ("text-align", place.align),
        ("border-bottom", border_bottom),
    ]);
    if place.last {
        map.insert("border-right".into(), format!("{fade_width} solid transparent"));
    }
    map
}

/// Build a header cell. The column is sized to fit the label; should the
/// font run wider than estimated, the label wraps (as before) rather than
/// being cut off.
fn header_cell(label: &str, place: &CellPlacement, theme: &Theme, fade_width: &str) -> ChartElement {
    let header_text_var = format!("var(--chartml-table-header-text, {})", theme.table_header_text);
    let weight_var = format!(
        "var(--chartml-table-header-font-weight, {})",
        theme.table_header_font_weight,
    );
    let spacing_var = format!(
        "var(--chartml-table-header-letter-spacing, {})",
        theme.table_header_letter_spacing,
    );
    let transform_var = format!(
        "var(--chartml-table-header-text-transform, {})",
        theme.table_header_text_transform,
    );
    let mut cell_style = cell_style(place, theme, &header_border(theme), fade_width);
    cell_style.extend(style(&[
        ("font-weight", &weight_var),
        ("letter-spacing", &spacing_var),
        ("text-transform", &transform_var),
        ("color", &header_text_var),
        ("overflow-wrap", "anywhere"),
    ]));
    ChartElement::Span {
        class: "chartml-table-header-cell".to_string(),
        style: cell_style,
        content: label.to_string(),
    }
}

/// Build a body cell. Every value stays readable: the column is sized to
/// fit it on one line, and a value wider than the column (beyond
/// `MAX_MIN_TEXT_EM`, wider than the visible area, or when the font runs
/// wider than estimated) wraps onto more lines instead of being cut off.
fn body_cell(content: &str, place: &CellPlacement, theme: &Theme, fade_width: &str) -> ChartElement {
    let text_var = format!("var(--chartml-table-text, {})", theme.table_text);
    let mut cell_style = cell_style(place, theme, &body_border(theme), fade_width);
    cell_style.extend(style(&[
        ("color", &text_var),
        ("overflow-wrap", "anywhere"),
    ]));
    ChartElement::Span {
        class: "chartml-table-cell".to_string(),
        style: cell_style,
        content: content.to_string(),
    }
}

/// The right-edge fade of a header or body row: a layer in the row's last
/// track, held at the right edge of the visible area (sticky) while columns
/// are hidden to the right. It takes the row's current, opaque background
/// (`inherit`, so a consumer's hover shows) masked from transparent to
/// solid, and stops 1px short of the row's bottom so the cells' border lines
/// are never faded.
///
/// The track is `fade_width` wide: `FADE_WIDTH_PX` while the columns at
/// their minimum are wider than the scroll area (the table scrolls), 0
/// otherwise (nothing overflows, nothing is drawn and no width is taken
/// from the columns). Scrolled to the right end, the fade sits over its own
/// strip, which holds only the last cell's extra padding, so it covers no
/// content.
fn fade_cell(column: usize) -> ChartElement {
    ChartElement::Span {
        class: "chartml-table-fade".to_string(),
        style: style(&[
            ("grid-column", &column.to_string()),
            ("grid-row", "1"),
            ("position", "sticky"),
            ("right", "0"),
            ("z-index", "1"),
            ("background", "inherit"),
            ("mask-image", "linear-gradient(to right, transparent, #000)"),
            ("pointer-events", "none"),
            ("margin-bottom", "1px"),
        ]),
        content: String::new(),
    }
}

fn header_border(theme: &Theme) -> String {
    let header_border_color = if theme.table_header_border.is_empty() {
        &theme.table_border
    } else {
        &theme.table_header_border
    };
    format!(
        "1px solid var(--chartml-table-header-border, var(--chartml-table-border, {}))",
        header_border_color,
    )
}

fn body_border(theme: &Theme) -> String {
    format!("1px solid var(--chartml-table-border, {})", theme.table_border)
}

impl ChartRenderer for TableRenderer {
    fn render(&self, data: &DataTable, config: &ChartConfig) -> Result<ChartElement, ChartError> {
        let viz = &config.visualize;
        let theme = &config.theme;

        let columns = resolve_columns(viz, data);
        if columns.is_empty() {
            return Err(ChartError::DataError(
                "Table has no columns to render".into(),
            ));
        }
        let numeric: Vec<bool> = columns
            .iter()
            .map(|c| is_numeric_column(data, &c.field))
            .collect();

        // Pagination: style.pageSize → how many rows per page, page 0.
        // Clamp to >= 1 so a malformed `pageSize: 0` cannot divide-by-zero
        // in the pager footer's `div_ceil`.
        let page_size = viz
            .style
            .as_ref()
            .and_then(|s| s.page_size)
            .unwrap_or(DEFAULT_PAGE_SIZE)
            .max(1);
        let total_rows = data.num_rows();
        let end = total_rows.min(page_size);

        let cells: Vec<Vec<String>> = (0..end)
            .map(|row| columns.iter().map(|c| format_cell(data, row, c)).collect())
            .collect();

        // ── Column tracks ──
        // A column with a fixed `width` keeps that width. Every other column
        // is at least as wide as its header label or widest value on the page
        // (plus padding) and otherwise takes an equal share of the width. No
        // column is pinned: when the columns don't all fit, the whole table
        // scrolls sideways (header and body rows are at least the sum of the
        // minimums plus the fade).
        //
        // The expressions are set once, as custom properties on the scroll
        // area, and used by every row. Everything is relative to the
        // table's own container and font, never the viewport: `em` resolves
        // where a property is used (rows and the body carry the table
        // font) and `100cqw` is the scroll area's visible width (it is an
        // inline-size container for everything inside it).
        let (pad_left, pad_right) = horizontal_padding(&theme.table_cell_padding);
        let header = header_text(theme);
        let texts: Vec<ColumnText> = columns
            .iter()
            .enumerate()
            .map(|(i, c)| column_text(&c.label, &header, cells.iter().map(|r| r[i].as_str())))
            .collect();
        let text_width = |em: f64| format!("calc({em:.2}em + {pad_left} + {pad_right})");
        let min_var = |i: usize| format!("--chartml-table-min-{}", i + 1);
        let mut sizing: Vec<(String, String)> = Vec::new();
        // Each column's minimum in column order, plus its grid track: a fixed
        // `width` column is exactly that wide; every other column is at least
        // its measured content width and shares the remaining space.
        let mut tracks: Vec<String> = Vec::with_capacity(columns.len());
        let mut column_mins: Vec<String> = Vec::with_capacity(columns.len());
        for (i, column) in columns.iter().enumerate() {
            let fixed = match column.width {
                Some(ColumnWidth::Pixels(px)) => {
                    if !(px.is_finite() && px > 0.0) {
                        return Err(ChartError::InvalidSpec(format!(
                            "table column '{}': width must be a positive number of pixels or \"auto\"",
                            column.field
                        )));
                    }
                    Some(format!("{px:.2}px"))
                }
                _ => None,
            };
            match fixed {
                Some(px) => {
                    column_mins.push(px.clone());
                    tracks.push(format!("minmax({px}, {px})"));
                }
                None => {
                    let text = &texts[i];
                    let content_min = if columns.len() > 1 {
                        // Every content-sized column fits in the visible area
                        // (less the fade) so its values can be seen whole —
                        // wrapping at spaces if they must — but is never
                        // narrower than its widest word.
                        format!(
                            "max({}, min({}, calc(100cqw - {FADE_WIDTH_PX}px)))",
                            text_width(text.word),
                            text_width(text.line),
                        )
                    } else {
                        text_width(texts[0].line)
                    };
                    sizing.push((min_var(i), content_min));
                    column_mins.push(format!("var({})", min_var(i)));
                    tracks.push(format!("minmax(var({}), 1fr)", min_var(i)));
                }
            }
        }
        // The fade: `FADE_WIDTH_PX` once the columns at their minimum are
        // wider than the scroll area (the step completes within 0.04px),
        // otherwise 0. Rows are at least the columns plus the fade, so when
        // the table scrolls its right end shows the fade's own empty strip.
        sizing.push(("--chartml-table-columns-min".into(), format!("calc({})", column_mins.join(" + "))));
        sizing.push((
            "--chartml-table-fade".into(),
            format!("clamp(0px, calc((var(--chartml-table-columns-min) - 100cqw) * 1000), {FADE_WIDTH_PX}px)"),
        ));
        let fade_width = "var(--chartml-table-fade)".to_string();
        let row_min = "calc(var(--chartml-table-columns-min) + var(--chartml-table-fade))".to_string();
        tracks.push(fade_width.clone());
        let template = tracks.join(" ");
        let fade_column = columns.len() + 1;
        let place = |i: usize| CellPlacement {
            column: i + 1,
            last: i + 1 == columns.len(),
            align: columns[i]
                .align
                .map_or_else(|| if numeric[i] { "right" } else { "left" }, align_css),
        };

        // The table font: the size the `em` minimums resolve against and the
        // size the npm stylesheet gives cell text, from the same variable.
        let font_size_var = format!("var(--chartml-table-font-size, {})", theme.table_font_size);
        let header_bg_var = format!("var(--chartml-table-header-bg, {})", theme.table_header_bg);
        let row_bg_var = format!("var(--chartml-table-row-bg, {})", theme.table_row_bg);
        let row_bg_alt_var = format!("var(--chartml-table-row-bg-alt, {})", theme.table_row_bg_alt);

        // ── Header row (sticky to the top of the scroll area, above the
        // body rows; opaque, so rows scrolling under it never show
        // through) ──
        let mut header_children: Vec<ChartElement> = columns
                .iter()
                .enumerate()
                .map(|(i, c)| header_cell(&c.label, &place(i), theme, &fade_width))
                .collect();
        header_children.push(fade_cell(fade_column));
        let header_row = ChartElement::Div {
            class: "chartml-table-header".to_string(),
            style: style(&[
                ("display", "grid"),
                ("grid-template-columns", &template),
                ("min-width", &row_min),
                ("position", "sticky"),
                ("top", "0"),
                ("z-index", "3"),
                ("background", &opaque_bg(&header_bg_var, theme)),
                ("font-size", &font_size_var),
                ("font-family", &theme.label_font_family),
            ]),
            children: header_children,
        };

        // ── Body rows ──
        let mut body_children: Vec<ChartElement> = Vec::with_capacity(end);
        for (row_idx, row_cells) in cells.iter().enumerate() {
            let is_alt = row_idx % 2 == 1;
            let bg = if is_alt { &row_bg_alt_var } else { &row_bg_var };
            let class = if is_alt {
                "chartml-table-row chartml-table-row-alt"
            } else {
                "chartml-table-row"
            };
            let mut children: Vec<ChartElement> = row_cells
                .iter()
                .enumerate()
                .map(|(i, text)| body_cell(text, &place(i), theme, &fade_width))
                .collect();
            children.push(fade_cell(fade_column));
            body_children.push(ChartElement::Div {
                class: class.to_string(),
                style: style(&[
                    ("display", "grid"),
                    ("grid-template-columns", &template),
                    ("background", &opaque_bg(bg, theme)),
                    ("font-size", &font_size_var),
                    ("font-family", &theme.label_font_family),
                ]),
                children,
            });
        }
        let body = ChartElement::Div {
            class: "chartml-table-body".to_string(),
            style: style(&[
                ("display", "flex"),
                ("flex-direction", "column"),
                // `em` in `row_min` resolves against this element's font,
                // which must be the table font as on the header and rows.
                ("min-width", &row_min),
                ("font-size", &font_size_var),
            ]),
            children: body_children,
        };

        // ── Scroll area: header + body scroll together on both axes, so the
        // header stays aligned with its columns when scrolled sideways and
        // stays on top (sticky) when the rows scroll vertically. `isolation`
        // keeps the sticky layers' z-indexes inside the table. ──
        let mut scroll_style = style(&[
            ("flex", "1 1 0"),
            ("min-height", "0"),
            ("overflow", "auto"),
            ("isolation", "isolate"),
            ("container-type", "inline-size"),
        ]);
        scroll_style.extend(sizing);
        let scroll = ChartElement::Div {
            class: "chartml-table-scroll".to_string(),
            style: scroll_style,
            children: vec![header_row, body],
        };

        // ── Pager footer (shown when there are more rows than one page),
        // outside the scroll area so it is always visible. ──
        let pager_text_var = format!("var(--chartml-text-secondary, {})", theme.text_secondary);
        let pager_border_var = format!("1px solid var(--chartml-table-border, {})", theme.table_border);
        let mut root_children: Vec<ChartElement> = vec![scroll];
        if total_rows > page_size {
            let page_count = total_rows.div_ceil(page_size);
            let info = ChartElement::Span {
                class: "chartml-table-pager-info".to_string(),
                style: style(&[
                    ("color", &pager_text_var),
                    ("font-size", &theme.table_font_size),
                    ("font-family", &theme.label_font_family),
                ]),
                content: format!(
                    "Showing {}–{} of {} · Page 1 of {}",
                    1, end, total_rows, page_count
                ),
            };
            root_children.push(ChartElement::Div {
                class: "chartml-table-pager".to_string(),
                style: style(&[
                    ("display", "flex"),
                    ("justify-content", "flex-end"),
                    ("padding", &theme.table_cell_padding),
                    ("border-top", &pager_border_var),
                    ("background", &header_bg_var),
                ]),
                children: vec![info],
            });
        }

        let root_text_var = format!("var(--chartml-table-text, {})", theme.table_text);
        let root_border_var = format!("1px solid var(--chartml-table-border, {})", theme.table_border);
        let radius_var = format!(
            "var(--chartml-table-border-radius, {})",
            theme.table_border_radius,
        );
        Ok(ChartElement::Div {
            class: "chartml-table".to_string(),
            style: style(&[
                ("display", "flex"),
                ("flex-direction", "column"),
                ("width", "100%"),
                ("height", "100%"),
                ("background", &row_bg_var),
                ("color", &root_text_var),
                ("border", &root_border_var),
                ("border-radius", &radius_var),
                ("overflow", "hidden"),
                ("box-sizing", "border-box"),
            ]),
            children: root_children,
        })
    }

    fn default_dimensions(&self, _spec: &VisualizeSpec) -> Option<Dimensions> {
        Some(Dimensions::new(DEFAULT_HEIGHT))
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;
    use chartml_core::data::Row;
    use chartml_core::element::count_elements;
    use chartml_core::spec::VisualizeSpec;
    use serde_json::json;

    fn data() -> DataTable {
        let rows: Vec<Row> = vec![
            [
                ("month".to_string(), json!("Jan")),
                ("revenue".to_string(), json!(1200.0)),
                ("cost".to_string(), json!(400.0)),
            ]
            .into_iter()
            .collect(),
            [
                ("month".to_string(), json!("Feb")),
                ("revenue".to_string(), json!(1500.5)),
                ("cost".to_string(), json!(450.0)),
            ]
            .into_iter()
            .collect(),
            [
                ("month".to_string(), json!("Mar")),
                ("revenue".to_string(), json!(1800.0)),
                ("cost".to_string(), json!(500.0)),
            ]
            .into_iter()
            .collect(),
        ];
        DataTable::from_rows(&rows).unwrap()
    }

    fn cfg(viz_yaml: &str) -> ChartConfig {
        let visualize: VisualizeSpec = serde_yaml::from_str(viz_yaml).unwrap();
        ChartConfig {
            visualize,
            title: None,
            width: 600.0,
            height: 400.0,
            colors: vec![],
            theme: Theme::default(),
        }
    }

    fn find_contents(el: &ChartElement, out: &mut Vec<String>) {
        match el {
            ChartElement::Span { content, .. } => out.push(content.clone()),
            ChartElement::Div { children, .. } => {
                for c in children {
                    find_contents(c, out);
                }
            }
            _ => {}
        }
    }

    #[test]
    fn renders_drop_in_bar_spec() {
        // Same spec a bar chart would use — just type: table.
        let config = cfg(
            r#"
type: table
columns: month
rows:
  - field: revenue
    label: Revenue
    format: "$,.0f"
  - field: cost
    label: Cost
    format: "$,.0f"
"#,
        );
        let element = TableRenderer::new().render(&data(), &config).unwrap();
        let mut contents = Vec::new();
        find_contents(&element, &mut contents);
        // Header labels present
        assert!(contents.iter().any(|c| c == "month"));
        assert!(contents.iter().any(|c| c == "Revenue"));
        assert!(contents.iter().any(|c| c == "Cost"));
        // Formatted revenue cell present
        assert!(
            contents.iter().any(|c| c == "$1,200"),
            "expected formatted revenue cell, got: {contents:?}"
        );
        // Category cell present
        assert!(contents.iter().any(|c| c == "Jan"));
    }

    #[test]
    fn falls_back_to_all_fields_when_no_spec() {
        let config = cfg("type: table");
        let element = TableRenderer::new().render(&data(), &config).unwrap();
        let mut contents = Vec::new();
        find_contents(&element, &mut contents);
        // All three schema fields appear as headers (DataTable sorts schema alphabetically).
        for name in ["cost", "month", "revenue"] {
            assert!(
                contents.iter().any(|c| c == name),
                "missing header {name}: {contents:?}"
            );
        }
    }

    #[test]
    fn page_size_zero_is_clamped_to_one() {
        // Malformed `pageSize: 0` must not divide-by-zero in the pager.
        let config = cfg(
            r#"
type: table
columns: month
rows:
  - field: revenue
style:
  pageSize: 0
"#,
        );
        let element = TableRenderer::new().render(&data(), &config).unwrap();
        let mut contents = Vec::new();
        find_contents(&element, &mut contents);
        // Page size clamped to 1 → only Jan visible, pager shows page 1 of 3.
        assert!(contents.iter().any(|c| c == "Jan"));
        assert!(!contents.iter().any(|c| c == "Feb"));
        assert!(contents.iter().any(|c| c.contains("Page 1 of 3")));
    }

    #[test]
    fn pagination_limits_rows() {
        let config = cfg(
            r#"
type: table
columns: month
rows:
  - field: revenue
style:
  pageSize: 2
"#,
        );
        let element = TableRenderer::new().render(&data(), &config).unwrap();
        let mut contents = Vec::new();
        find_contents(&element, &mut contents);
        // Only first two month values present
        assert!(contents.iter().any(|c| c == "Jan"));
        assert!(contents.iter().any(|c| c == "Feb"));
        assert!(
            !contents.iter().any(|c| c == "Mar"),
            "Mar should be paginated out: {contents:?}"
        );
        // Pager info span should be present
        assert!(
            contents.iter().any(|c| c.contains("Page 1 of")),
            "missing pager info: {contents:?}"
        );
    }

    #[test]
    fn empty_columns_errors() {
        let config = cfg("type: table");
        let empty = DataTable::from_rows(&Vec::<Row>::new()).unwrap();
        // Empty DataTable → no fields → no columns → error.
        assert!(TableRenderer::new().render(&empty, &config).is_err());
    }

    #[test]
    fn null_cell_renders_em_dash() {
        // A JSON null in a numeric column must render "—" (U+2014) rather than
        // an empty string, so the table always has visible, informative content.
        let rows: Vec<Row> = vec![
            [
                ("month".to_string(), json!("Jan")),
                ("revenue".to_string(), serde_json::Value::Null),
            ]
            .into_iter()
            .collect(),
        ];
        let data = DataTable::from_rows(&rows).unwrap();
        let config = cfg(
            r#"
type: table
columns: month
rows:
  - field: revenue
    label: Revenue
    format: "$,.0f"
"#,
        );
        let element = TableRenderer::new().render(&data, &config).unwrap();
        let mut contents = Vec::new();
        find_contents(&element, &mut contents);
        assert!(
            contents.iter().any(|c| c == "\u{2014}"),
            "null cell must render em dash, got: {contents:?}"
        );
    }

    #[test]
    fn null_string_cell_renders_em_dash() {
        // A JSON null in a string column must also render "—".
        let rows: Vec<Row> = vec![
            [
                ("month".to_string(), serde_json::Value::Null),
                ("revenue".to_string(), json!(1200.0)),
            ]
            .into_iter()
            .collect(),
        ];
        let data = DataTable::from_rows(&rows).unwrap();
        let config = cfg("type: table");
        let element = TableRenderer::new().render(&data, &config).unwrap();
        let mut contents = Vec::new();
        find_contents(&element, &mut contents);
        assert!(
            contents.iter().any(|c| c == "\u{2014}"),
            "null string cell must render em dash, got: {contents:?}"
        );
    }

    #[test]
    fn root_has_table_class() {
        let config = cfg("type: table");
        let element = TableRenderer::new().render(&data(), &config).unwrap();
        match &element {
            ChartElement::Div { class, .. } => assert_eq!(class, "chartml-table"),
            _ => panic!("root must be a Div"),
        }
    }

    #[test]
    fn theme_applied_to_cells() {
        let mut config = cfg("type: table");
        config.theme = Theme::dark();
        let element = TableRenderer::new().render(&data(), &config).unwrap();
        // Header bg should be the dark-theme header color somewhere in the tree.
        let header_bg = Theme::dark().table_header_bg;
        let mut found = false;
        fn walk(el: &ChartElement, needle: &str, out: &mut bool) {
            match el {
                ChartElement::Div { style, children, .. } => {
                    if style.values().any(|v| v.contains(needle)) {
                        *out = true;
                    }
                    for c in children {
                        walk(c, needle, out);
                    }
                }
                ChartElement::Span { style, .. } if style.values().any(|v| v.contains(needle)) => {
                    *out = true;
                }
                _ => {}
            }
        }
        walk(&element, &header_bg, &mut found);
        assert!(found, "dark theme header bg {header_bg} not found in tree");
    }

    #[test]
    fn default_dimensions() {
        let dims = TableRenderer::new().default_dimensions(
            &serde_yaml::from_str::<VisualizeSpec>("type: table").unwrap(),
        );
        assert_eq!(dims.unwrap().height, 400.0);
    }

    /// Collect every Div/Span whose class list contains `class`, in document order.
    fn find_by_class<'a>(el: &'a ChartElement, class: &str, out: &mut Vec<&'a ChartElement>) {
        let (own, children): (&str, Option<&Vec<ChartElement>>) = match el {
            ChartElement::Div { class, children, .. } => (class, Some(children)),
            ChartElement::Span { class, .. } => (class, None),
            _ => ("", None),
        };
        if own.split_whitespace().any(|c| c == class) {
            out.push(el);
        }
        if let Some(children) = children {
            for c in children {
                find_by_class(c, class, out);
            }
        }
    }

    fn by_class<'a>(el: &'a ChartElement, class: &str) -> Vec<&'a ChartElement> {
        let mut out = Vec::new();
        find_by_class(el, class, &mut out);
        out
    }

    fn style_of(el: &ChartElement) -> &HashMap<String, String> {
        match el {
            ChartElement::Div { style, .. } | ChartElement::Span { style, .. } => style,
            _ => panic!("not a Div/Span"),
        }
    }

    fn children_of(el: &ChartElement) -> &Vec<ChartElement> {
        match el {
            ChartElement::Div { children, .. } => children,
            _ => panic!("not a Div"),
        }
    }

    fn class_of(el: &ChartElement) -> &str {
        match el {
            ChartElement::Div { class, .. } | ChartElement::Span { class, .. } => class,
            _ => panic!("not a Div/Span"),
        }
    }

    fn get<'a>(el: &'a ChartElement, key: &str) -> Option<&'a str> {
        style_of(el).get(key).map(|s| s.as_str())
    }

    /// Twelve columns: one text category, a text measure, nine numeric
    /// measures and a trailing text measure — the narrow-container case.
    fn wide_data() -> DataTable {
        let rows: Vec<Row> = (0..4)
            .map(|i| {
                let mut row: Row = [
                    ("region".to_string(), json!(format!("Region {i}"))),
                    ("manager".to_string(), json!(format!("Manager {i}"))),
                    ("tier".to_string(), json!("Gold")),
                ]
                .into_iter()
                .collect();
                for m in ["m1", "m2", "m3", "m4", "m5", "m6", "m7", "m8", "m9"] {
                    row.insert(m.to_string(), json!(f64::from(i) * 10.5));
                }
                row
            })
            .collect();
        DataTable::from_rows(&rows).unwrap()
    }

    const WIDE_SPEC: &str = r#"
type: table
columns: region
rows: [manager, m1, m2, m3, m4, m5, m6, m7, m8, m9, tier]
style:
  pageSize: 3
"#;

    fn wide() -> ChartElement {
        TableRenderer::new().render(&wide_data(), &cfg(WIDE_SPEC)).unwrap()
    }

    /// The table's own cells in a header or body row (not the fade).
    fn real_cells(row: &ChartElement) -> Vec<&ChartElement> {
        children_of(row)
            .iter()
            .filter(|c| matches!(class_of(c), "chartml-table-cell" | "chartml-table-header-cell"))
            .collect()
    }

    fn header_and_rows(el: &ChartElement) -> Vec<&ChartElement> {
        let mut rows = by_class(el, "chartml-table-header");
        rows.extend(by_class(el, "chartml-table-row"));
        rows
    }

    #[test]
    fn column_text_follows_label_and_values_up_to_the_cap() {
        let plain = HeaderText::body();
        // Short content gets a short column: "NPS" / "44" needs about 2em,
        // no wider.
        let nps = column_text("NPS", &plain, ["44", "-15"].into_iter()).line;
        let expected = (text_em("NPS", &plain) * 100.0).ceil() / 100.0;
        assert_eq!(nps, expected);
        assert!(nps < 3.0, "{nps}");
        // The widest word: a multi-word value can wrap at its spaces, a
        // number cannot.
        let t = column_text("Region", &plain, ["Northwind Traders International", "$1,234,567"].into_iter());
        let em = |w: &str| (text_em(w, &plain) * 100.0).ceil() / 100.0;
        assert_eq!(t.word, em("International").max(em("$1,234,567")));
        assert!(t.word < t.line);
        // A single word longer than the cap (a URL, an ID) is capped too, so
        // it cannot force sideways scrolling in a wide container; it breaks.
        let url = format!("https://example.com/{}", "a".repeat(120));
        let t = column_text("Link", &plain, [url.as_str()].into_iter());
        assert_eq!((t.line, t.word), (MAX_MIN_TEXT_EM, MAX_MIN_TEXT_EM));
        // Nothing to show: no text width (the column is its padding).
        assert_eq!(column_text("", &plain, ["", ""].into_iter()).line, 0.0);
        // A long value: its measured advance in em, widened by the slack.
        let v = "Sub-Saharan Africa";
        let expected = text_em(v, &plain);
        let got = column_text("region", &plain, [v, "Oceania"].into_iter()).line;
        assert!((got - expected).abs() < 0.011, "{got} vs {expected}");
        // A label wider than every value sets the width.
        let label = "Customer lifetime value";
        assert!(column_text(label, &plain, ["1"].into_iter()).line > column_text("x", &plain, ["1"].into_iter()).line);
        // Very long values are capped (beyond it they wrap).
        let long = "x".repeat(200);
        assert_eq!(column_text("n", &plain, [long.as_str()].into_iter()).line, MAX_MIN_TEXT_EM);
        // Latin characters carry their measured advance instead of the old
        // uniform estimate; a character outside the Latin scope keeps exactly
        // what the legacy estimator charged it.
        let measured = |ch: char, bold: bool| measured_char_em(ch, bold).expect("measured");
        assert!(measured('W', false) > measured('i', false));
        assert!(measured('%', false) > measured('n', false));
        assert!(measured('W', true) > measured('W', false));
        // Non-Latin parity with base: a CJK ideograph keeps the legacy 7px of 12
        // widened by 1.15, i.e. 0.6389em before this estimator's own slack.
        assert!(measured_char_em('\u{4e2d}', false).is_none());
        assert!((legacy_char_px('\u{4e2d}') / 12.0 * 1.15 / TEXT_WIDTH_SLACK - 7.0 / 12.0 * 1.15 / TEXT_WIDTH_SLACK).abs() < 1e-9);
        // The table is sorted for the binary search and covers every
        // printable ASCII character.
        assert!(TABLE_CHAR_MILLI_EM.windows(2).all(|w| w[0].0 < w[1].0));
        for c in ' '..='~' {
            assert!(TABLE_CHAR_MILLI_EM.binary_search_by_key(&c, |&(ch, _, _)| ch).is_ok(), "{c:?}");
        }
        // Bold headers (the default theme's weight 600) use the bold
        // advances; weight 500 uses the regular ones.
        let mut theme = Theme::default();
        let bold = header_text(&theme);
        assert!(bold.bold);
        assert!(column_text("NPS", &bold, ["44"].into_iter()).line > nps);
        theme.table_header_font_weight = "500".into();
        let regular = header_text(&theme);
        assert!(!regular.bold);
        theme.table_header_font_weight = "bold".into();
        assert!(header_text(&theme).bold);
        // Uppercase, letter-spaced headers measure wider.
        theme.table_header_font_weight = "500".into();
        theme.table_header_text_transform = "uppercase".into();
        theme.table_header_letter_spacing = "0.08em".into();
        let kyomi = header_text(&theme);
        assert!(matches!(kyomi.transform, TextTransform::Uppercase));
        assert!((kyomi.letter_spacing_em - 0.08).abs() < 1e-9);
        assert!(column_text("Customers", &kyomi, [].into_iter()).line > column_text("Customers", &regular, [].into_iter()).line);
        theme.table_header_letter_spacing = "1.3px".into();
        theme.table_font_size = "13px".into();
        assert!((header_text(&theme).letter_spacing_em - 0.1).abs() < 1e-9);
    }

    /// The owner's ruling: text outside the Latin scope must lay out exactly as
    /// it did at base 86a87a1. This compares the estimator against base's own
    /// per-character arithmetic for a sample of each non-Latin script, at both
    /// weights, and fails if any of them moved at all.
    #[test]
    fn non_latin_text_keeps_the_legacy_estimate() {
        // One representative sample per script, plus multi-character strings,
        // characters whose case mapping crosses the measured boundary, and
        // emoji/symbols. Single characters sample the scripts; the multi-character
        // entries are what exercise word minimums.
        let samples = [
            ("\u{4e2d}", "CJK"), ("\u{65e5}", "CJK"), ("\u{ac00}", "Hangul"),
            ("\u{3042}", "kana"), ("\u{0416}", "Cyrillic"), ("\u{0434}", "Cyrillic"),
            ("\u{03a9}", "Greek"), ("\u{03bb}", "Greek"),
            ("\u{05e9}", "Hebrew"), ("\u{0645}", "Arabic"), ("\u{0e01}", "Thai"),
            ("\u{0915}", "Devanagari"), ("\u{0d15}", "Malayalam"), ("\u{0b95}", "Tamil"),
            ("\u{10a0}", "Georgian"), ("\u{0531}", "Armenian"), ("\u{fb17}", "Armenian ligature"),
            // Multi-character strings, including ones with a space, because a
            // space is Latin and so mixed strings exercise the boundary.
            ("\u{041c}\u{043e}\u{0441}\u{043a}\u{0432}\u{0430}", "Cyrillic word"),
            ("\u{041f}\u{0440}\u{0438}\u{0432}\u{0435}\u{0442} \u{043c}\u{0438}\u{0440}", "Cyrillic with space"),
            ("\u{05e9}\u{05dc}\u{05d5}\u{05dd}", "Hebrew word"),
            ("\u{4e2d}\u{6587}\u{8868}\u{683c}", "CJK run"),
            ("\u{03a3}\u{03b5}\u{03c0}\u{03c4}\u{03ad}\u{03bc}\u{03b2}\u{03c1}\u{03b9}\u{03bf}\u{03c2}", "Greek word"),
            ("\u{1f600}", "emoji"), ("\u{2191}", "arrow"), ("\u{2605}", "symbol"),
            // Case mappings that cross the measured/legacy boundary: these have no
            // measurement of their own, but their uppercase forms do, so routing
            // on the transformed string would let `uppercase` shrink a column.
            ("\u{26a}", "small capital I"), ("\u{269}", "iota"), ("\u{fb01}", "fi ligature"),
            ("\u{283}", "esh"), ("\u{266}", "h with hook"),
            ("\u{1626}", "Mongolian"), ("\u{1200}", "Ethiopic"),
            // Mixed strings whose Latin-only word is separately measured: the word
            // minimum must stay at base, or a column that kept this text whole
            // starts wrapping it. `(¥)` is the case the coordinator proved.
            ("\u{58f2}\u{4e0a} (\u{a5})", "CJK + Latin word"),
            ("Sales \u{58f2}\u{4e0a}", "Latin word + CJK"),
            ("NPS \u{0416}", "Latin acronym + Cyrillic"),
            ("Gross margin \u{4e2d}", "Latin words + CJK"),
            ("\u{6771}\u{4eac} 2024 (24%)", "CJK + digits and per-cent"),
            // Strings whose legacy width lands on a 0.01em ceiling boundary, where
            // a floating-point difference in the composition becomes a whole
            // hundredth of an em. Base formed `n * (spacing * 12)` and this once
            // formed `n * spacing * 12`; f64 addition is not associative.
            //
            // `Northwind\u{3a9}` and `Growth\u{416}\u{416}\u{416}\u{416}` are the two
            // that actually expose it: 10 characters, uppercase and 0.08em
            // spacing gives 8.509999999999998 grouped against
            // 8.510000000000002 ungrouped, so the ceiling yields 8.51 against
            // 8.52. `Customers\u{394} Region\u{2605}` sits at 14.833083... under
            // both groupings and is here as a long mixed string, not as a
            // boundary case.
            ("Customers\u{394} Region\u{2605}", "10 characters, mixed"),
            ("Northwind\u{3a9}", "10 characters, ceiling boundary"),
            ("Growth\u{416}\u{416}\u{416}\u{416}", "10 characters, ceiling boundary"),
        ];
        // Base composed a string as: sum of the 12px table, x1.10 if the theme
        // uppercases, + letter-spacing per character, x1.10 for bold, then its
        // 1.15 slack. Reproduce that whole chain, not just the per-character
        // part, so every theme state is compared.
        fn base_text_em(text: &str, transform_upper: bool, spacing_em: f64, bold: bool) -> f64 {
            let t: String = if transform_upper { text.to_uppercase() } else { text.to_string() };
            let n = t.chars().count() as f64;
            let mut w: f64 = t.chars().map(base_em_char).sum();
            if transform_upper {
                w *= 1.10;
            }
            // Base formed `letter_spacing_px = spacing_em * 12.0` first and
            // multiplied by the character count second. The opposite grouping
            // differs in the last bits and turns into 0.01em at the ceiling.
            w += n * (spacing_em * 12.0);
            if bold {
                w *= 1.10;
            }
            w / 12.0 * 1.15
        }
        fn base_em_char(ch: char) -> f64 {
            match ch {
                'M' | 'W' | 'm' | 'w' => 9.0,
                'i' | 'l' | 'j' | '!' | '|' | '.' | ',' | ':' | ';' | '\'' => 4.0,
                'f' | 'r' | 't' => 5.0,
                ' ' => 4.0,
                _ => 7.0,
            }
        }
        // Every combination of the theme states that change the estimate. The
        // third element is the raw CSS `letter-spacing` value, and the fourth the
        // equivalent em for the base oracle, so `spaced-px` really is px.
        let themes: [(&str, bool, &str, f64); 5] = [
            ("plain", false, "normal", 0.0),
            ("uppercase", true, "normal", 0.0),
            ("spaced", false, "0.08em", 0.08),
            ("uppercase+spaced", true, "0.08em", 0.08),
            ("spaced-px", false, "1.3px", 1.3 / 13.0),
        ];
        for (label, upper, spacing_css, spacing) in themes {
            for (text, script) in samples {
                for bold in [false, true] {
                    let mut theme = Theme::default();
                    theme.table_header_text_transform =
                        if upper { "uppercase".into() } else { "none".into() };
                    theme.table_header_letter_spacing = spacing_css.into();
                    theme.table_header_font_weight =
                        if bold { "600".into() } else { "400".into() };
                    let header = header_text(&theme);
                    let ours = text_em(text, &header);
                    let base = base_text_em(text, upper, spacing, bold);
                    assert!(
                        (ours - base).abs() < 1e-9,
                        "{script} {text:?} [{label}] bold={bold}: estimate {ours:.6}em != base {base:.6}em"
                    );
                    // The column minimum is the larger of the line and the widest
                    // word, so every word must match base too. A word is sized with
                    // its string's estimator: routing per word instead would let a
                    // Latin-only word inside non-Latin text (the `(¥)` in
                    // `売上 (¥)`) shrink to its measured advance, make the word
                    // minimum narrower than base, and wrap text base kept whole.
                    let column = column_text(text, &header, std::iter::empty());
                    // Base: the line is the whole string; the word is the widest
                    // whitespace-separated word (0 for a string with no words).
                    let base_line = base_text_em(text, upper, spacing, bold);
                    let base_word = text
                        .split_whitespace()
                        .map(|w| base_text_em(w, upper, spacing, bold))
                        .fold(0.0_f64, f64::max);
                    let base_line = (base_line * 100.0).ceil() / 100.0;
                    let base_word = (base_word * 100.0).ceil() / 100.0;
                    assert!(
                        (column.line - base_line.min(MAX_MIN_TEXT_EM)).abs() < 1e-9,
                        "{script} {text:?} [{label}] bold={bold}: line {:.2}em != base {base_line:.2}em",
                        column.line
                    );
                    assert!(
                        (column.word - base_word.min(MAX_MIN_TEXT_EM)).abs() < 1e-9,
                        "{script} {text:?} [{label}] bold={bold}: word minimum {:.2}em != base {base_word:.2}em",
                        column.word
                    );
                }
            }
        }
    }

    fn measured_font_scan() -> Vec<(char, f64, f64)> {
        let raw = include_str!("../tests/fixtures/font_scan.txt");
        raw.lines()
            .filter_map(|line| {
                let line = line.trim();
                if line.is_empty() || line.starts_with('#') {
                    return None;
                }
                let mut parts = line.split_whitespace();
                let cp = u32::from_str_radix(parts.next()?, 16).ok()?;
                let regular = parts.next()?.parse::<f64>().ok()?;
                let bold = parts.next()?.parse::<f64>().ok()?;
                Some((char::from_u32(cp)?, regular / 1000.0, bold / 1000.0))
            })
            .collect()
    }

    /// The two constants the estimator's safety rests on are pinned by the
    /// behaviour they drive, because both could otherwise be weakened with every
    /// test still green: for a measured character the estimate *is* the
    /// measurement, so a corpus comparison passes at any slack at or above 1.0,
    /// and no test exercises a capped column.
    #[test]
    fn slack_and_cap_deliver_their_guarantees() {
        // The slack must actually widen the estimate. For a measured glyph the
        // estimate before slack is exactly the measurement, so estimate x slack
        // is the only headroom a wider font gets — at slack 1.0 there is none.
        let body = HeaderText::body();
        let measured = text_em("W", &body);
        let widened = measured * TEXT_WIDTH_SLACK;
        let ratio = widened / measured;
        assert!(
            (1.05..=1.10).contains(&ratio),
            "slack gives {ratio:.3}x over the measured advance, outside the 5-10% the evidence supports"
        );
        // Where the fixture records a value, the estimate must be at or above it
        // and must not exceed it by more than that slack.
        let scan = measured_font_scan();
        for (ch, regular_em, _) in scan.iter() {
            let estimate = text_em(&ch.to_string(), &body);
            assert!(
                estimate + 1e-9 >= *regular_em,
                "U+{:04X} estimate {estimate:.4}em < measured {regular_em:.4}em",
                *ch as u32
            );
            assert!(
                estimate <= regular_em * 1.10 + 1e-9,
                "U+{:04X} estimate {estimate:.4}em is far above the measured {regular_em:.4}em",
                *ch as u32
            );
        }
        // The cap must sit just above the longest common table value: high enough
        // that the value fits on one line, and low enough that one long value
        // cannot take over the table. Asserting both sides pins it to a narrow
        // band; asserting only "the value fits" would pass for any cap up to the
        // point a 120-character URL stops being capped (88.7em), so a silent
        // 20 -> 30em edit would ship. The value estimates 19.97em.
        let region = "Northwind Traders International Ltd II";
        let fits = column_text("Region", &body, std::iter::once(region));
        assert!(fits.line < MAX_MIN_TEXT_EM, "the longest common value must not be capped ({:.2}em)", fits.line);
        assert!(
            (MAX_MIN_TEXT_EM - fits.line).abs() <= 0.1,
            "cap {MAX_MIN_TEXT_EM:.2}em should sit just above the longest common value's {:.2}em",
            fits.line
        );
        let url = "https://example.com/".to_string() + &"a".repeat(120);
        let capped = column_text("Link", &body, std::iter::once(url.as_str()));
        assert_eq!(capped.line, MAX_MIN_TEXT_EM, "a long URL must be capped");
    }

    /// A theme must never make a cell narrower than base sized it, and never
    /// narrower than the text paints. Routing is decided from the text *and* its
    /// transform (see `text_em`), so a character whose uppercase form happens to
    /// be measured cannot switch to the measured path because the theme
    /// uppercases it: `U+026A` has no measurement of its own, and without that
    /// rule it measured 0.4179em uppercase against base's 0.7379em at weight 400
    /// (0.8117em at 600 and 700, where base's bold scale applies).
    ///
    /// This does not claim uppercase is always at least as wide as plain. For a
    /// *measured* glyph it can be narrower, because the font is: the widest of
    /// the five measured faces gives `m` 0.9750em and `M` 0.9080em, and the
    /// fixture records only that per-character maximum, so no per-face ordering
    /// is claimed. That is the font speaking, not the routing, so the invariant
    /// is base parity and painted width.
    #[test]
    fn a_theme_never_makes_a_cell_narrower_than_base_or_than_it_paints() {
        // Unmeasured characters whose uppercase form is measured, measured
        // characters whose uppercase form is not, a lowercase glyph that is
        // wider than its uppercase form, and multi-word strings.
        let samples = [
            "\u{26a}", "\u{269}", "\u{fb01}", "\u{283}", "\u{266}", "\u{b5}",
            "m", "M", "w", "W", "\u{4e2d}", "\u{0416}",
            "nps", "Northwind Traders International Ltd II", "Umsatz",
        ];
        for text in samples {
            for weight in ["400", "600", "700"] {
                for transform in ["none", "uppercase", "lowercase"] {
                    let mut theme = Theme::default();
                    theme.table_header_font_weight = weight.into();
                    theme.table_header_text_transform = transform.into();
                    let header = header_text(&theme);
                    let ours = text_em(text, &header);
                    // Base's full arithmetic for the same text and theme state,
                    // in base's operation order, including the 1.10 bold scale:
                    // omitting it would compare a bold estimate against a value
                    // about 10% low and make this assertion vacuous at 600/700.
                    let base = {
                        let transformed = transform_text(text, transform);
                        let upper = transform == "uppercase";
                        let bold = weight != "400";
                        let mut w: f64 = transformed.chars().map(base_char_px).sum();
                        if upper {
                            w *= 1.10;
                        }
                        w += transformed.chars().count() as f64 * (header.letter_spacing_em * 12.0);
                        if bold {
                            w *= 1.10;
                        }
                        w / 12.0 * 1.15
                    };
                    // The contract differs by scope, and this is exactly where the
                    // two meet:
                    //  * inside the measured scope the estimate must cover what the
                    //    widest measured face paints. It may be *narrower* than
                    //    base there, because base was wrong in both directions —
                    //    `µ` measures 0.6370em where base charged 0.6708em, and the
                    //    measurement is the correct number.
                    //  * outside it the contract is base parity, so it must never be
                    //    narrower than base. `\u{266}` uppercase is a measured
                    //    `U+A7AA`, so legacy's 0.7379em under-estimates the
                    //    0.8630em it paints — base did exactly the same, and
                    //    widening it is CHA-42.
                    let transformed = transform_text(text, transform);
                    let in_scope = text.chars().all(|c| font_scan_lookup(c, weight != "400").is_some())
                        && transformed
                            .chars()
                            .all(|c| font_scan_lookup(c, weight != "400").is_some());
                    if in_scope {
                        let painted: f64 = transformed
                            .chars()
                            .map(|c| font_scan_lookup(c, weight != "400").expect("in scope"))
                            .sum();
                        assert!(
                            ours + 1e-9 >= painted,
                            "{text:?} [{transform}] weight {weight}: estimate {ours:.4}em is narrower than the widest measured face paints ({painted:.4}em)"
                        );
                    } else {
                        assert!(
                            ours + 1e-9 >= base,
                            "{text:?} [{transform}] weight {weight}: estimate {ours:.4}em is narrower than base {base:.4}em for text outside the measured scope"
                        );
                    }
                }
            }
        }
    }

    /// `text` with the theme's CSS `text-transform` applied, for the base oracle
    /// in `a_theme_never_makes_a_cell_narrower_than_base_or_than_it_paints`.
    fn transform_text(text: &str, transform: &str) -> String {
        match transform {
            "uppercase" => text.to_uppercase(),
            "lowercase" => text.to_lowercase(),
            _ => text.to_string(),
        }
    }

    /// Base's 12px character table, in px.
    fn base_char_px(ch: char) -> f64 {
        match ch {
            'M' | 'W' | 'm' | 'w' => 9.0,
            'i' | 'l' | 'j' | '!' | '|' | '.' | ',' | ':' | ';' | '\'' => 4.0,
            'f' | 'r' | 't' => 5.0,
            ' ' => 4.0,
            _ => 7.0,
        }
    }

    /// The measured advance for `ch` in em, from the fixture, or `None` when the
    /// fixture has no row for it.
    fn font_scan_lookup(ch: char, bold: bool) -> Option<f64> {
        measured_font_scan()
            .into_iter()
            .find(|(c, _, _)| *c == ch)
            .map(|(_, regular, bold_em)| if bold { bold_em } else { regular })
    }

    /// Letter-spacing must be charged on the *measured* route too, once per
    /// character. Nothing else covers it: the parity test only exercises text
    /// outside the measured table, and no golden uses a letter-spaced header
    /// theme, so deleting the term from `measured_text_em` used to leave every
    /// test green while a letter-spaced uppercase header measured 7.59em against
    /// the 8.02em it paints — an in-scope under-estimate of exactly the kind this
    /// ticket exists to remove.
    #[test]
    fn measured_text_charges_letter_spacing_once_per_character() {
        for spacing_css in ["normal", "0.08em", "0.2em", "1.3px"] {
            let mut theme = Theme::default();
            theme.table_header_font_weight = "600".into();
            theme.table_header_text_transform = "uppercase".into();
            theme.table_header_letter_spacing = spacing_css.into();
            let header = header_text(&theme);
            let text = "Umsatz";
            let transformed = transform(text, &header.transform);
            let n = transformed.chars().count() as f64;
            let advances: f64 = transformed
                .chars()
                .map(|c| measured_char_em(c, true).expect("measured"))
                .sum();
            let expected = (advances + n * header.letter_spacing_em) * TEXT_WIDTH_SLACK;
            let actual = text_em(text, &header);
            assert!(
                (actual - expected).abs() < 1e-12,
                "[{spacing_css}] measured estimate {actual:.6}em != advances {advances:.6} + {n} x {:.6} spacing, widened = {expected:.6}em",
                header.letter_spacing_em
            );
        }
        // And the term must actually be non-zero for a spaced theme, so the
        // assertion above cannot pass by comparing two spacing-free numbers.
        let mut spaced = Theme::default();
        spaced.table_header_letter_spacing = "0.2em".into();
        assert!(header_text(&spaced).letter_spacing_em > 0.19);
    }

    /// AC 1 and 3: the estimate for every measured Latin code point, regular and
    /// bold, must be at least the width the browser paints. This carries the
    /// whole measured scope, not samples, so any character the estimator
    /// under-estimates fails here.
    #[test]
    fn estimate_covers_measured_font_scan() {
        let scan = measured_font_scan();
        assert_eq!(scan.len(), 1_020, "{} scanned code points", scan.len());
        // Every character table entry is a recorded measurement, not an invented
        // number: each must appear in the fixture with the same values, so the
        // table and the fixture cannot drift apart.
        //
        // Both measured tables are binary-searched, so their ordering is part of
        // the contract: an unsorted row would become silently unreachable.
        assert!(
            TABLE_CHAR_MILLI_EM.windows(2).all(|w| w[0].0 < w[1].0),
            "TABLE_CHAR_MILLI_EM must be sorted by character, without duplicates"
        );
        // The fixture is measured data, so it is pinned as a whole: an exact row
        // count and a content hash. Without this, lowering a recorded value would
        // silently weaken every check below, and deleting a row the table no
        // longer contains would go unnoticed.
        assert_eq!(scan.len(), 1_020, "the measured fixture lost or gained rows");
        let raw = include_str!("../tests/fixtures/font_scan.txt");
        let digest = raw.bytes().fold(0xcbf2_9ce4_8422_2325_u64, |hash, byte| {
            (hash ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3)
        });
        assert_eq!(
            digest, 0x4d14_cb31_484d_1981,
            "the measured fixture changed; re-run scripts/measure-table-fonts.mjs and scripts/measure-table-fonts-macos.swift, then update this pin"
        );

        // Compare in milli-em, rounding the f64 round-trip to nearest so a
        // consistent pair never fails on binary representation (for 11 of the
        // values below 3000, `ceil` of the round-trip overshoots by 1).
        let to_milli = |em: f64| (em * 1000.0).round() as u32;
        // The character table must cover the fixture in both directions: every
        // table row is a recorded measurement, and every recorded measurement has
        // a table row. Without the second direction, deleting a row from the
        // table would silently return that character to base's legacy estimate —
        // `U+00B5` would go from its 0.637em measurement back to base's 0.6708em,
        // which is *wider*, so the under-estimate check below would still pass.
        for (scanned, _, _) in &scan {
            assert!(
                TABLE_CHAR_MILLI_EM.iter().any(|(ch, _, _)| ch == scanned),
                "U+{:04X} is measured in the fixture but missing from the character table",
                *scanned as u32
            );
        }
        // The character table must agree with the fixture, so the two cannot
        // drift from the recorded measurements.
        for (ch, regular, bold) in TABLE_CHAR_MILLI_EM {
            let recorded = scan
                .iter()
                .find(|(scanned, _, _)| scanned == ch)
                .unwrap_or_else(|| panic!("U+{:04X}: character table entry has no measured width", *ch as u32));
            assert_eq!(
                (to_milli(recorded.1), to_milli(recorded.2)),
                (*regular, *bold),
                "U+{:04X}: character table disagrees with the recorded measurement",
                *ch as u32
            );
        }
        let bold_header = header_text(&Theme::default());
        let body = HeaderText::body();
        let mut under = Vec::new();
        for (ch, regular_em, bold_em) in &scan {
            let text = ch.to_string();
            for (header, measured_em) in [(&body, regular_em), (&bold_header, bold_em)] {
                let estimate_em = text_em(&text, header);
                if estimate_em + 1e-9 < *measured_em {
                    under.push(format!(
                        "U+{:04X} {text:?} bold={}: estimate {:.3}em < measured {:.3}em",
                        *ch as u32,
                        header.bold,
                        estimate_em,
                        measured_em
                    ));
                }
            }
        }
        assert!(under.is_empty(), "{} code points under-estimated:\n{}", under.len(), under.join("\n"));
    }

    #[test]
    fn padding_shorthand_left_and_right() {
        assert_eq!(horizontal_padding("8px"), ("8px".into(), "8px".into()));
        assert_eq!(horizontal_padding("8px 12px"), ("12px".into(), "12px".into()));
        assert_eq!(horizontal_padding("8px 12px 4px"), ("12px".into(), "12px".into()));
        assert_eq!(horizontal_padding("1px 2px 3px 4px"), ("4px".into(), "2px".into()));
        assert_eq!(horizontal_padding(""), ("0px".into(), "0px".into()));
    }

    #[test]
    fn numeric_columns_right_aligned_from_data_type() {
        // Point 3: alignment comes from each column's data type: Float64
        // measures (formatted or not) are right-aligned, header included;
        // text columns, digit-only text and a field missing from the data
        // stay left.
        let el = wide();
        let expected = [
            "left", "left", "right", "right", "right", "right", "right", "right", "right",
            "right", "right", "left",
        ];
        for row in header_and_rows(&el) {
            let got: Vec<&str> = real_cells(row).iter().map(|c| get(c, "text-align").unwrap()).collect();
            assert_eq!(got, expected);
        }

        let rows: Vec<Row> = vec![[
            ("code".to_string(), json!("007")),
            ("amount".to_string(), json!(5)),
        ]
        .into_iter()
        .collect()];
        let data = DataTable::from_rows(&rows).unwrap();
        let config = cfg("type: table\ncolumns: code\nrows: [amount, missing]\n");
        let el = TableRenderer::new().render(&data, &config).unwrap();
        let header = by_class(&el, "chartml-table-header")[0];
        let got: Vec<&str> = real_cells(header).iter().map(|c| get(c, "text-align").unwrap()).collect();
        assert_eq!(got, vec!["left", "right", "left"]);
    }

    #[test]
    fn numeric_alignment_covers_arrow_integer_and_decimal_types() {
        // Point 3, typed sources (e.g. a warehouse query): Int64 and Decimal128
        // are numeric and right-aligned; Date32 and Utf8 are not.
        use arrow::array::{ArrayRef, Date32Array, Decimal128Array, Int64Array, StringArray};
        use arrow::datatypes::{DataType, Field, Schema};
        use arrow::record_batch::RecordBatch;
        use std::sync::Arc;
        let schema = Arc::new(Schema::new(vec![
            Field::new("name", DataType::Utf8, true),
            Field::new("count", DataType::Int64, true),
            Field::new("price", DataType::Decimal128(10, 2), true),
            Field::new("day", DataType::Date32, true),
        ]));
        let arrays: Vec<ArrayRef> = vec![
            Arc::new(StringArray::from(vec!["a"])),
            Arc::new(Int64Array::from(vec![3])),
            Arc::new(Decimal128Array::from(vec![1234]).with_precision_and_scale(10, 2).unwrap()),
            Arc::new(Date32Array::from(vec![19000])),
        ];
        let data = DataTable::from_record_batch(RecordBatch::try_new(schema, arrays).unwrap());
        let config = cfg("type: table\ncolumns: name\nrows: [count, price, day]\n");
        let el = TableRenderer::new().render(&data, &config).unwrap();
        for row in header_and_rows(&el) {
            let got: Vec<&str> = real_cells(row).iter().map(|c| get(c, "text-align").unwrap()).collect();
            assert_eq!(got, vec!["left", "right", "right", "left"]);
        }
    }

    #[test]
    fn column_align_overrides_type_based_default() {
        // Schema `align` is read for columns and rows: it replaces the
        // type-based default on both the header and every body cell (here a
        // text category forced right and numeric measures forced left/center).
        let config = cfg(
            r#"
type: table
columns:
  - field: month
    align: right
rows:
  - field: revenue
    label: Revenue
    align: left
  - field: cost
    label: Cost
    align: center
"#,
        );
        let el = TableRenderer::new().render(&data(), &config).unwrap();
        let rows = header_and_rows(&el);
        assert_eq!(rows.len(), 4, "header + three body rows");
        for row in rows {
            let got: Vec<&str> = real_cells(row).iter().map(|c| get(c, "text-align").unwrap()).collect();
            assert_eq!(got, vec!["right", "left", "center"]);
        }
    }

    #[test]
    fn fixed_column_width_sets_the_track_and_keeps_the_scroll_layout() {
        // Schema `width` is read: a number is that column's exact grid track
        // (header and body cells included); `auto` keeps the measured
        // content minimum. Fixed widths join the existing scroll machinery,
        // so the table still scrolls sideways and no column is pinned.
        let config = cfg(
            r#"
type: table
columns:
  - field: month
    width: 120
rows:
  - field: revenue
    label: Revenue
    width: auto
  - field: cost
    label: Cost
    width: 90.5
"#,
        );
        let el = TableRenderer::new().render(&data(), &config).unwrap();
        let scroll = by_class(&el, "chartml-table-scroll")[0];
        // Fixed columns need no measured minimum; the auto column keeps one.
        assert_eq!(get(scroll, "--chartml-table-min-1"), None);
        assert!(get(scroll, "--chartml-table-min-2").unwrap().starts_with("max("));
        assert_eq!(get(scroll, "--chartml-table-min-3"), None);
        assert_eq!(
            get(scroll, "--chartml-table-columns-min"),
            Some("calc(120.00px + var(--chartml-table-min-2) + 90.50px)")
        );
        let template = "minmax(120.00px, 120.00px) minmax(var(--chartml-table-min-2), 1fr) minmax(90.50px, 90.50px) var(--chartml-table-fade)";
        for row in header_and_rows(&el) {
            assert_eq!(get(row, "grid-template-columns"), Some(template));
            for cell in real_cells(row) {
                for key in ["position", "left", "z-index"] {
                    assert_eq!(get(cell, key), None, "{key} must stay unset");
                }
            }
        }
        let rows = header_and_rows(&el);
        let body = by_class(&el, "chartml-table-body")[0];
        assert_eq!(get(rows[0], "min-width"), get(body, "min-width"));
        assert_eq!(
            get(rows[0], "min-width"),
            Some("calc(var(--chartml-table-columns-min) + var(--chartml-table-fade))")
        );
    }

    /// Render a chart spec through the public path so parse errors surface
    /// exactly as consumers see them.
    fn render_yaml(yaml: &str) -> Result<ChartElement, ChartError> {
        use chartml_core::ChartML;
        let mut chartml = ChartML::new();
        chartml.register_renderer("table", TableRenderer::new());
        chartml.render_from_yaml(yaml)
    }

    #[test]
    fn invalid_column_width_errors_like_other_mistyped_fields() {
        // A width that is neither a number nor "auto" is a type error, as any
        // other mistyped FieldSpec value is; a non-positive pixel width is
        // rejected when rendering instead of emitting invalid CSS.
        let spec = |width: &str| {
            format!(
                r#"
type: chart
version: 1
data:
  provider: inline
  rows:
    - {{ month: "Jan", revenue: 1200 }}
visualize:
  type: table
  columns:
    - field: month
      width: {width}
  rows: revenue
"#
            )
        };
        let err = render_yaml(&spec("\"100%\"")).expect_err("a percentage is not documented");
        assert!(matches!(err, ChartError::YamlParse(_)), "got {err:?}");
        let err = render_yaml(&spec("0")).expect_err("zero pixels is not a usable width");
        assert!(
            matches!(&err, ChartError::InvalidSpec(msg) if msg.contains("positive number of pixels")),
            "got {err:?}"
        );
    }

    #[test]
    fn cell_padding_reads_the_css_variable_with_the_theme_default() {
        // The published --chartml-table-cell-padding token has no effect when
        // the renderer hardcodes the theme's padding inline. It is now read
        // like every other --chartml-table-* token, with the theme value as
        // the fallback, so a host stylesheet can override it.
        let config = cfg("type: table\ncolumns: month\nrows: [revenue]");
        let el = TableRenderer::new().render(&data(), &config).unwrap();
        for row in header_and_rows(&el) {
            let cells = real_cells(row);
            for cell in &cells {
                assert_eq!(get(cell, "padding"), Some("var(--chartml-table-cell-padding, 8px 12px)"));
                assert_eq!(get(cell, "padding-right"), None, "the token must control every side");
            }
            // The fade clearance is a transparent border on the last cell, so
            // the padding token still applies to its right side.
            assert_eq!(
                get(cells.last().unwrap(), "border-right"),
                Some("var(--chartml-table-fade) solid transparent")
            );
        }
        // A host theme that sets its own padding provides that fallback.
        let mut config = cfg("type: table\ncolumns: month\nrows: [revenue]");
        config.theme.table_cell_padding = "4px 6px".into();
        let el = TableRenderer::new().render(&data(), &config).unwrap();
        for row in header_and_rows(&el) {
            assert_eq!(
                get(real_cells(row)[0], "padding"),
                Some("var(--chartml-table-cell-padding, 4px 6px)")
            );
        }
    }

    const SCHEMA: &str = include_str!("../../../docs/docs/public/chartml_schema.json");

    #[test]
    fn schema_documents_only_table_properties_the_renderer_reads() {
        // Every property the published schema documents for table columns
        // must be read by the renderer. If a documented property is added,
        // this fails until the renderer (and the render proof below) covers it.
        let schema: serde_json::Value = serde_json::from_str(SCHEMA).expect("schema is JSON");
        let mut documented: Vec<&str> = schema["definitions"]["ColumnsObject"]["properties"]
            .as_object()
            .expect("ColumnsObject.properties")
            .keys()
            .map(String::as_str)
            .collect();
        documented.sort_unstable();
        assert_eq!(documented, vec!["align", "field", "format", "label", "width"]);

        let page_size = &schema["definitions"]["Style"]["properties"]["pageSize"];
        assert_eq!(page_size["type"], "integer");
        assert_eq!(page_size["minimum"], 1);
        assert_eq!(page_size["default"], 50);

        // Render proof for the documented column properties and pageSize: a
        // spec setting every one produces a header label, a fixed track, an
        // alignment, a formatted cell and a paginated row count.
        let config = cfg(
            r#"
type: table
columns:
  - field: month
    label: Month
    width: 100
    align: center
rows:
  - field: revenue
    label: Revenue
    format: "$,.0f"
    width: auto
style:
  pageSize: 2
"#,
        );
        let el = TableRenderer::new().render(&data(), &config).unwrap();
        let mut contents = Vec::new();
        find_contents(&el, &mut contents);
        assert!(contents.iter().any(|c| c == "Month"), "{contents:?}");
        assert!(contents.iter().any(|c| c == "Revenue"), "{contents:?}");
        assert!(contents.iter().any(|c| c == "$1,200"), "{contents:?}");
        assert!(!contents.iter().any(|c| c == "Mar"), "pageSize 2 must hide Mar: {contents:?}");
        let scroll = by_class(&el, "chartml-table-scroll")[0];
        assert_eq!(
            get(scroll, "--chartml-table-columns-min"),
            Some("calc(100.00px + var(--chartml-table-min-2))")
        );
        let rows = header_and_rows(&el);
        assert_eq!(rows.len(), 3, "header + two body rows");
        for row in rows {
            let cells = real_cells(row);
            assert_eq!(get(cells[0], "grid-column"), Some("1"));
            assert_eq!(get(cells[0], "text-align"), Some("center"));
        }
    }

    #[test]
    fn existing_class_names_kept() {
        // Point 6: consumer CSS overrides keep applying.
        let el = wide();
        for class in [
            "chartml-table",
            "chartml-table-header",
            "chartml-table-header-cell",
            "chartml-table-body",
            "chartml-table-row",
            "chartml-table-row-alt",
            "chartml-table-cell",
            "chartml-table-pager",
            "chartml-table-pager-info",
        ] {
            assert!(!by_class(&el, class).is_empty(), "missing {class}");
        }
    }

    #[test]
    fn header_and_body_share_one_scroll_area_and_column_grid() {
        // Point 1: header and body sit in one scroll container and every
        // row is a grid with the same column template, so they scroll
        // sideways together and stay aligned; each column sits in the same
        // grid column in the header and in every row.
        let el = wide();
        let scroll = by_class(&el, "chartml-table-scroll");
        assert_eq!(scroll.len(), 1);
        let scroll = scroll[0];
        assert_eq!(get(scroll, "overflow"), Some("auto"));
        assert_eq!(get(scroll, "container-type"), Some("inline-size"));
        let kids: Vec<&str> = children_of(scroll).iter().map(class_of).collect();
        assert_eq!(kids, vec!["chartml-table-header", "chartml-table-body"]);
        let rows = header_and_rows(&el);
        let template = get(rows[0], "grid-template-columns");
        assert!(template.is_some());
        for row in &rows {
            assert_eq!(get(row, "display"), Some("grid"));
            assert_eq!(get(row, "grid-template-columns"), template);
            let cols: Vec<Option<&str>> = real_cells(row).iter().map(|c| get(c, "grid-column")).collect();
            assert_eq!(cols, real_cells(rows[0]).iter().map(|c| get(c, "grid-column")).collect::<Vec<_>>());
        }
        // Header and body are at least as wide as the columns' minimums
        // (the same expression on both), so every row spans the scroll.
        let body = by_class(&el, "chartml-table-body")[0];
        assert!(get(rows[0], "min-width").is_some());
        assert_eq!(get(rows[0], "min-width"), get(body, "min-width"));
    }

    #[test]
    fn measured_widths_reach_every_column_track() {
        // Point 1: each column's track minimum is built from that column's
        // measured one-line width and widest word (never zero), every row
        // uses the same tracks, in column order, ending with the fade.
        let data = wide_data();
        let el = TableRenderer::new().render(&data, &cfg(WIDE_SPEC)).unwrap();
        let scroll = by_class(&el, "chartml-table-scroll")[0];
        let header = header_text(&Theme::default());
        let columns = resolve_columns(&cfg(WIDE_SPEC).visualize, &data);
        let rows = header_and_rows(&el);
        let body_rows = &rows[1..];
        for (i, c) in columns.iter().enumerate() {
            let values: Vec<String> = (0..body_rows.len()).map(|r| format_cell(&data, r, c)).collect();
            let t = column_text(&c.label, &header, values.iter().map(String::as_str));
            assert!(t.line > 0.0, "{}", c.label);
            let min = get(scroll, &format!("--chartml-table-min-{}", i + 1)).unwrap();
            assert!(min.contains(&format!("calc({:.2}em + 12px + 12px)", t.line)), "{}: {min}", c.label);
            assert!(min.contains(&format!("calc({:.2}em + 12px + 12px)", t.word)), "{}: {min}", c.label);
        }
        let expected: Vec<String> = (1..=columns.len())
            .map(|i| format!("minmax(var(--chartml-table-min-{i}), 1fr)"))
            .chain(std::iter::once("var(--chartml-table-fade)".to_string()))
            .collect();
        for row in &rows {
            assert_eq!(get(row, "grid-template-columns"), Some(expected.join(" ").as_str()));
        }
    }

    #[test]
    fn fade_turns_on_only_when_the_columns_overflow() {
        // Point 4: the fade's width is 0 unless the sum of all column
        // minimums is wider than the scroll area (`100cqw`), when it is
        // `FADE_WIDTH_PX`; rows are at least the columns plus the fade.
        let el = wide();
        let scroll = by_class(&el, "chartml-table-scroll")[0];
        let all: Vec<String> = (1..=12).map(|i| format!("var(--chartml-table-min-{i})")).collect();
        assert_eq!(get(scroll, "--chartml-table-columns-min"), Some(format!("calc({})", all.join(" + ")).as_str()));
        assert_eq!(
            get(scroll, "--chartml-table-fade"),
            Some("clamp(0px, calc((var(--chartml-table-columns-min) - 100cqw) * 1000), 32px)")
        );
        for row in by_class(&el, "chartml-table-header").into_iter().chain(by_class(&el, "chartml-table-body")) {
            assert_eq!(
                get(row, "min-width"),
                Some("calc(var(--chartml-table-columns-min) + var(--chartml-table-fade))")
            );
        }
    }

    #[test]
    fn rows_and_header_are_opaque_over_the_table_surface() {
        // Point 2: header and body rows lay their colour over the table
        // surface, so they are opaque even when a theme makes them
        // transparent.
        let mut config = cfg(WIDE_SPEC);
        config.theme.table_header_bg = "transparent".into();
        config.theme.table_row_bg = "transparent".into();
        config.theme.table_surface = "#24201E".into();
        let el = TableRenderer::new().render(&wide_data(), &config).unwrap();
        for row in header_and_rows(&el) {
            let bg = get(row, "background").unwrap();
            assert!(bg.starts_with("linear-gradient("), "{bg}");
            assert!(bg.ends_with(", var(--chartml-table-surface, #24201E)"), "{bg}");
        }
        let header = by_class(&el, "chartml-table-header")[0];
        assert_eq!(get(header, "position"), Some("sticky"));
        assert_eq!(get(header, "top"), Some("0"));
    }

    #[test]
    fn no_column_is_sticky_and_each_fits_the_visible_area() {
        // No column is pinned: no header or body cell is sticky, takes its
        // row's background or covers a gutter beside it, so the whole table
        // scrolls sideways. Every column, the first included, fits in the
        // visible area less the fade (so each can be scrolled fully into
        // view), but is never narrower than its widest word.
        let table = wide_data();
        let el = TableRenderer::new().render(&table, &cfg(WIDE_SPEC)).unwrap();
        for row in header_and_rows(&el) {
            for (i, c) in real_cells(row).iter().enumerate() {
                for key in ["position", "left", "z-index", "background", "margin-right", "mask-image"] {
                    assert_eq!(get(c, key), None, "cell {i}: {key}");
                }
            }
        }
        let scroll = by_class(&el, "chartml-table-scroll")[0];
        assert!(!style_of(scroll).keys().any(|k| k.contains("frozen")), "{:?}", style_of(scroll).keys());
        let header = header_text(&Theme::default());
        for (i, c) in resolve_columns(&cfg(WIDE_SPEC).visualize, &table).iter().enumerate() {
            let values: Vec<String> = (0..3).map(|r| format_cell(&table, r, c)).collect();
            let t = column_text(&c.label, &header, values.iter().map(String::as_str));
            assert_eq!(
                get(scroll, &format!("--chartml-table-min-{}", i + 1)),
                Some(
                    format!(
                        "max(calc({:.2}em + 12px + 12px), min(calc({:.2}em + 12px + 12px), calc(100cqw - 32px)))",
                        t.word, t.line
                    )
                    .as_str()
                ),
                "{}",
                c.label
            );
        }
    }

    #[test]
    fn fade_is_the_last_cell_of_every_row() {
        // Point 4: every header and body row ends with a fade in its own
        // grid track: sticky at the right edge, the row's background masked
        // into a gradient, no border of its own, ignoring the pointer. The
        // last real cell reaches under that track so its border runs to the
        // row's end.
        let el = wide();
        for row in header_and_rows(&el) {
            let fade = children_of(row).last().unwrap();
            assert_eq!(class_of(fade), "chartml-table-fade");
            assert_eq!(get(fade, "grid-column"), Some("13"));
            assert_eq!(get(fade, "position"), Some("sticky"));
            assert_eq!(get(fade, "right"), Some("0"));
            assert_eq!(get(fade, "background"), Some("inherit"));
            assert_eq!(get(fade, "mask-image"), Some("linear-gradient(to right, transparent, #000)"));
            assert_eq!(get(fade, "pointer-events"), Some("none"));
            assert_eq!(get(fade, "border-bottom"), None);
            let last = *real_cells(row).last().unwrap();
            assert_eq!(get(last, "grid-column"), Some("12 / span 2"));
        }
    }

    #[test]
    fn sizes_from_container_not_viewport() {
        // Point 5: no viewport units anywhere; sizes are em of the table
        // font, px, percentages of the table's own box and `cqw` of its
        // scroll area.
        fn walk(el: &ChartElement) {
            if let ChartElement::Div { .. } | ChartElement::Span { .. } = el {
                for (k, v) in style_of(el) {
                    for tok in v.split(|c: char| !c.is_ascii_alphanumeric() && c != '.') {
                        let unit = tok.trim_start_matches(|c: char| c.is_ascii_digit() || c == '.');
                        let numeric = tok.starts_with(|c: char| c.is_ascii_digit());
                        assert!(
                            !(numeric && ["vw", "vh", "vmin", "vmax", "dvw", "svw", "lvw", "dvh", "svh", "lvh"].contains(&unit)),
                            "viewport unit in {k}: {v}"
                        );
                    }
                }
            }
            if let ChartElement::Div { children, .. } = el {
                children.iter().for_each(walk);
            }
        }
        let el = wide();
        walk(&el);
        let scroll = by_class(&el, "chartml-table-scroll")[0];
        assert!(get(scroll, "--chartml-table-fade").unwrap().contains("100cqw"));
    }

    #[test]
    fn vertical_scroll_and_pager_unchanged() {
        // Point 6: the rows scroll vertically inside a flex-filling area
        // under the sticky header; the pager stays outside it, below.
        let el = wide();
        let kids: Vec<&str> = children_of(&el).iter().map(class_of).collect();
        assert_eq!(kids, vec!["chartml-table-scroll", "chartml-table-pager"]);
        let scroll = &children_of(&el)[0];
        assert_eq!(get(scroll, "flex"), Some("1 1 0"));
        assert_eq!(get(scroll, "min-height"), Some("0"));
        let mut contents = Vec::new();
        find_contents(&el, &mut contents);
        assert!(contents.iter().any(|c| c == "Showing 1–3 of 4 · Page 1 of 2"));
    }

    #[test]
    fn element_counts_reasonable() {
        let config = cfg(
            r#"
type: table
columns: month
rows:
  - field: revenue
"#,
        );
        let element = TableRenderer::new().render(&data(), &config).unwrap();
        let span_count =
            count_elements(&element, &|e| matches!(e, ChartElement::Span { .. }));
        // 2 header cells + (3 rows * 2 cells) = 8 spans minimum
        assert!(span_count >= 8, "got {span_count} spans");
    }
}
