mod app;
mod editor;
mod examples;
mod gallery;
// `provider_example` uses `wasm_bindgen_futures::JsFuture` to back its mock
// provider's artificial latency, which isn't `Send` and therefore can't
// satisfy the trait bound on native `cargo check`s. The demo is a
// browser-only Trunk app, so gating the module + its router branch to
// `wasm32` keeps the workspace native build green without losing the
// page in the actual user-facing artifact.
#[cfg(target_arch = "wasm32")]
mod provider_example;

fn main() {
    console_error_panic_hook::set_once();
    leptos::mount::mount_to_body(app::App);
}
