//! Interactive parameter controls for ChartML.
//!
//! Renders native HTML form controls (select, checkbox, input) that write to
//! a shared `RwSignal<ParamValues>`. Charts reading the same signal re-render
//! automatically when values change.

use leptos::prelude::*;
use chartml_core::params::ParamValues;
use chartml_core::spec::ParamDef;

/// Render interactive parameter controls from a list of param definitions.
/// Writes to the shared `param_values` signal — any ChartMLChart reading
/// the same signal will re-render with updated param resolution.
#[component]
pub fn ParamsControls(
    /// Parameter definitions from the spec
    params: Vec<ParamDef>,
    /// Shared reactive param values signal
    param_values: RwSignal<ParamValues>,
    /// Block name prefix (e.g., "dashboard_filters" for named params). Empty for chart-level.
    #[prop(optional)]
    block_name: String,
) -> impl IntoView {
    view! {
        <div class="chartml-params">
            {params.into_iter().map(|param| {
                let block_name = block_name.clone();
                view! { <ParamControl param=param param_values=param_values block_name=block_name /> }
            }).collect::<Vec<_>>()}
        </div>
    }
}

/// Single parameter control — dispatches by type.
#[component]
fn ParamControl(
    param: ParamDef,
    param_values: RwSignal<ParamValues>,
    block_name: String,
) -> impl IntoView {
    let label = param.label.clone();

    // The key used in ParamValues: "blockname.param_id" or just "param_id"
    let param_key = if block_name.is_empty() {
        param.id.clone()
    } else {
        format!("{}.{}", block_name, param.id)
    };

    let control = match param.param_type.as_str() {
        "select" => render_select(param, param_key, param_values),
        "multiselect" => render_multiselect(param, param_key, param_values),
        "number" => render_number(param, param_key, param_values),
        "text" => render_text(param, param_key, param_values),
        "daterange" => render_daterange(param, param_key, param_values),
        _ => view! { <span class="chartml-param-error">"Unknown param type"</span> }.into_any(),
    };

    view! {
        <div class="chartml-param-item">
            <label class="chartml-param-label">{label}</label>
            {control}
        </div>
    }
}

fn render_select(
    param: ParamDef,
    key: String,
    param_values: RwSignal<ParamValues>,
) -> AnyView {
    let options = param.options.clone().unwrap_or_default();
    let default_val = param.default
        .as_ref()
        .and_then(|d| d.as_str())
        .unwrap_or("")
        .to_string();

    view! {
        <select
            class="chartml-param-control"
            on:change=move |ev| {
                let val = event_target_value(&ev);
                param_values.update(|pv| {
                    pv.insert(key.clone(), serde_json::Value::String(val));
                });
            }
        >
            {options.iter().map(|opt| {
                let selected = *opt == default_val;
                let opt_for_val = opt.clone();
                let opt_for_text = opt.clone();
                view! { <option value=opt_for_val selected=selected>{opt_for_text}</option> }
            }).collect::<Vec<_>>()}
        </select>
    }.into_any()
}

fn render_multiselect(
    param: ParamDef,
    key: String,
    param_values: RwSignal<ParamValues>,
) -> AnyView {
    let options = param.options.clone().unwrap_or_default();
    let defaults: Vec<String> = param.default
        .as_ref()
        .and_then(|d| d.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
        .unwrap_or_default();

    // Local state tracking which options are selected
    let selected = RwSignal::new(defaults.clone());

    view! {
        <div class="chartml-param-control chartml-param-multiselect-options">
            {options.iter().map(|opt| {
                let opt_val = opt.clone();
                let opt_display = opt.clone();
                let key = key.clone();
                let is_checked = defaults.contains(opt);

                view! {
                    <label class="chartml-param-checkbox">
                        <input
                            type="checkbox"
                            checked=is_checked
                            on:change=move |ev| {
                                let checked = event_target_checked(&ev);
                                selected.update(|s| {
                                    if checked {
                                        if !s.contains(&opt_val) { s.push(opt_val.clone()); }
                                    } else {
                                        s.retain(|v| v != &opt_val);
                                    }
                                });
                                let vals: Vec<serde_json::Value> = selected.get()
                                    .iter()
                                    .map(|v| serde_json::Value::String(v.clone()))
                                    .collect();
                                param_values.update(|pv| {
                                    pv.insert(key.clone(), serde_json::Value::Array(vals));
                                });
                            }
                        />
                        {opt_display}
                    </label>
                }
            }).collect::<Vec<_>>()}
        </div>
    }.into_any()
}

fn render_number(
    param: ParamDef,
    key: String,
    param_values: RwSignal<ParamValues>,
) -> AnyView {
    let default_val = param.default
        .as_ref()
        .and_then(|d| d.as_f64())
        .unwrap_or(0.0)
        .to_string();

    view! {
        <input
            type="number"
            class="chartml-param-control"
            value=default_val
            on:input=move |ev| {
                let val_str = event_target_value(&ev);
                let num = val_str.parse::<f64>().unwrap_or(0.0);
                param_values.update(|pv| {
                    pv.insert(key.clone(), serde_json::json!(num));
                });
            }
        />
    }.into_any()
}

fn render_text(
    param: ParamDef,
    key: String,
    param_values: RwSignal<ParamValues>,
) -> AnyView {
    let default_val = param.default
        .as_ref()
        .and_then(|d| d.as_str())
        .unwrap_or("")
        .to_string();
    let placeholder = param.placeholder.unwrap_or_default();

    view! {
        <input
            type="text"
            class="chartml-param-control"
            value=default_val
            placeholder=placeholder
            on:input=move |ev| {
                let val = event_target_value(&ev);
                param_values.update(|pv| {
                    pv.insert(key.clone(), serde_json::Value::String(val));
                });
            }
        />
    }.into_any()
}

fn render_daterange(
    param: ParamDef,
    key: String,
    param_values: RwSignal<ParamValues>,
) -> AnyView {
    let default_start = param.default
        .as_ref()
        .and_then(|d| d.get("start"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let default_end = param.default
        .as_ref()
        .and_then(|d| d.get("end"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let start_val = RwSignal::new(default_start.clone());
    let end_val = RwSignal::new(default_end.clone());

    let key_start = key.clone();
    let key_end = key;

    view! {
        <div class="chartml-param-control chartml-param-daterange-inputs">
            <input
                type="date"
                value=default_start
                on:change=move |ev| {
                    let val = event_target_value(&ev);
                    start_val.set(val.clone());
                    param_values.update(|pv| {
                        pv.insert(key_start.clone(), serde_json::json!({
                            "start": val,
                            "end": end_val.get()
                        }));
                    });
                }
            />
            <span class="chartml-param-daterange-separator">"→"</span>
            <input
                type="date"
                value=default_end
                on:change=move |ev| {
                    let val = event_target_value(&ev);
                    end_val.set(val.clone());
                    param_values.update(|pv| {
                        pv.insert(key_end.clone(), serde_json::json!({
                            "start": start_val.get(),
                            "end": val
                        }));
                    });
                }
            />
        </div>
    }.into_any()
}
