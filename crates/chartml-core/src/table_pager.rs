//! The contract between the table renderer and the browser controllers that
//! make its pager work (CHA-30).
//!
//! A table with more rows than `style.pageSize` renders one page at a time:
//! `RenderOptions::table_page` (from 0) picks the page and only that page's
//! rows are in the output, so its size does not depend on the row count. A
//! render with no `table_page` is static: page 0, and no button has a target. The
//! output also has a pager (Previous and Next buttons and a status line).
//! Rendering is a string, so nothing in it can run: a controller in the host
//! finds the buttons by the names below and calls the host back with the page
//! to show, and the host renders that page again. The names are defined here
//! once. The Rust renderer writes them; the Leptos controller
//! (`chartml-leptos`) uses these constants; the JavaScript controller
//! (`attachTablePager` in `@chartml/core`) repeats them and is tested against
//! the renderer's real output.
//!
//! - The table root (`.chartml-table`) carries [`ATTR_PAGE_COUNT`] and
//!   [`ATTR_CURRENT_PAGE`] when there is more than one page.
//! - Each pager button carries [`ATTR_ACTION`] ([`ACTION_PREV`] or
//!   [`ACTION_NEXT`]) and, when it has a page to go to, [`ATTR_TARGET_PAGE`]
//!   (from 0), but only in a render that has a `table_page`. Every button is
//!   written `disabled`, because a render with no controller (a static SVG or
//!   PNG) cannot move. A controller enables each
//!   button that has a target; the first page's Previous and the last page's
//!   Next have none and stay disabled.

/// On the table root: the number of pages, as a decimal integer.
pub const ATTR_PAGE_COUNT: &str = "data-chartml-page-count";
/// On the table root: the page shown, from 0.
pub const ATTR_CURRENT_PAGE: &str = "data-chartml-current-page";
/// On a pager button: what it does, [`ACTION_PREV`] or [`ACTION_NEXT`].
pub const ATTR_ACTION: &str = "data-chartml-pager-action";
/// On a pager button that has somewhere to go: the page it goes to, from 0.
pub const ATTR_TARGET_PAGE: &str = "data-chartml-target-page";

/// The Previous button's [`ATTR_ACTION`].
pub const ACTION_PREV: &str = "prev";
/// The Next button's [`ATTR_ACTION`].
pub const ACTION_NEXT: &str = "next";

/// The class of a table root.
pub const CLASS_TABLE: &str = "chartml-table";
