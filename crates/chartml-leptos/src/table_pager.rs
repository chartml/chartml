//! The browser controller that makes a table's pager work (CHA-30).
//!
//! The table renderer writes Previous and Next buttons that start disabled and
//! carry the page they go to (see `chartml_core::table_pager` for the contract,
//! whose names this module uses). [`TablePager::attach`] listens on a host
//! container for clicks on those buttons and calls the host back with the page
//! to show; the host renders that page again, which replaces the table. The
//! controller does not move anything itself.
//!
//! The click listener is delegated to the container, and a `MutationObserver`
//! prepares every table that is mounted into it later: it enables each button
//! that has a target page (the first page's Previous and the last page's Next
//! have none and stay disabled), and, after a page change, puts focus back on
//! the button that was pressed (or on the other one when the pressed one has no
//! page left to go to). So a chart that re-renders (a page change, a resize, a
//! refresh, new data) is set up again without the host doing anything.
//! Dropping the [`TablePager`] removes the listener and disconnects the
//! observer.

use std::cell::RefCell;
use std::rc::Rc;

use chartml_core::table_pager::{ACTION_NEXT, ACTION_PREV, ATTR_ACTION, ATTR_TARGET_PAGE};
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

/// A table pager controller attached to one container element.
pub struct TablePager {
    container: web_sys::Element,
    click: Closure<dyn Fn(web_sys::Event)>,
    observer: web_sys::MutationObserver,
    _observe: Closure<dyn Fn(js_sys::Array, web_sys::MutationObserver)>,
}

impl TablePager {
    /// Drive every table in `container`, now and whenever one is mounted into
    /// it later. `on_page` is called with the (0-based) page a pressed button
    /// goes to.
    pub fn attach(
        container: &web_sys::Element,
        on_page: impl Fn(usize) + 'static,
    ) -> Result<Self, JsValue> {
        // Which button was pressed, until the table has been re-rendered.
        let pending_focus: Rc<RefCell<Option<String>>> = Rc::new(RefCell::new(None));

        let for_click = container.clone();
        let pending_for_click = pending_focus.clone();
        let click = Closure::<dyn Fn(web_sys::Event)>::new(move |event: web_sys::Event| {
            let Some(target) = event.target().and_then(|t| t.dyn_into::<web_sys::Element>().ok())
            else {
                return;
            };
            let Ok(Some(button)) = target.closest(&format!("[{ATTR_TARGET_PAGE}]")) else {
                return;
            };
            if !for_click.contains(Some(&button)) || button.has_attribute("disabled") {
                return;
            }
            let Some(page) = button
                .get_attribute(ATTR_TARGET_PAGE)
                .and_then(|p| p.parse::<usize>().ok())
            else {
                return;
            };
            *pending_for_click.borrow_mut() = button.get_attribute(ATTR_ACTION);
            on_page(page);
        });
        container.add_event_listener_with_callback("click", click.as_ref().unchecked_ref())?;

        let for_observer = container.clone();
        let observe = Closure::<dyn Fn(js_sys::Array, web_sys::MutationObserver)>::new(
            move |_records: js_sys::Array, _observer: web_sys::MutationObserver| {
                prepare(&for_observer, &pending_focus);
            },
        );
        let observer = web_sys::MutationObserver::new(observe.as_ref().unchecked_ref())?;
        let init = web_sys::MutationObserverInit::new();
        init.set_child_list(true);
        init.set_subtree(true);
        observer.observe_with_options(container, &init)?;

        prepare(container, &Rc::new(RefCell::new(None)));
        Ok(Self {
            container: container.clone(),
            click,
            observer,
            _observe: observe,
        })
    }
}

impl Drop for TablePager {
    fn drop(&mut self) {
        self.observer.disconnect();
        let _ = self
            .container
            .remove_event_listener_with_callback("click", self.click.as_ref().unchecked_ref());
    }
}

/// Enable every pager button in `container` that has a page to go to, and
/// restore focus after a page change.
fn prepare(container: &web_sys::Element, pending_focus: &Rc<RefCell<Option<String>>>) {
    let Ok(buttons) = container.query_selector_all(&format!("[{ATTR_ACTION}]")) else {
        return;
    };
    for i in 0..buttons.length() {
        let Some(button) = buttons.item(i).and_then(|n| n.dyn_into::<web_sys::Element>().ok())
        else {
            continue;
        };
        if button.has_attribute(ATTR_TARGET_PAGE) {
            let _ = button.remove_attribute("disabled");
        }
    }
    let Some(pressed) = pending_focus.borrow_mut().take() else {
        return;
    };
    let other = if pressed == ACTION_PREV { ACTION_NEXT } else { ACTION_PREV };
    // The pressed button again if it still has somewhere to go, else the other.
    for action in [pressed.as_str(), other] {
        let selector = format!("[{ATTR_ACTION}=\"{action}\"][{ATTR_TARGET_PAGE}]");
        if let Ok(Some(el)) = container.query_selector(&selector) {
            if let Ok(el) = el.dyn_into::<web_sys::HtmlElement>() {
                let _ = el.focus();
                return;
            }
        }
    }
}
