use leptos::prelude::*;

#[component]
pub fn YamlEditor(
    #[prop(into)]
    value: Signal<String>,
    on_change: impl Fn(String) + 'static + Send + Sync,
) -> impl IntoView {
    view! {
        <textarea
            class="yaml-editor"
            prop:value=move || value.get()
            on:input=move |ev| {
                let val = event_target_value(&ev);
                on_change(val);
            }
            spellcheck="false"
        />
    }
}
