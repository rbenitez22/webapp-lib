use leptos::prelude::*;
use leptos::{component, view, IntoView};
use std::sync::Arc;
use web_sys::{KeyboardEvent, MouseEvent};

use crate::http::ResourcePath;

// ---------------------------------------------------------------------------
// AutocompleteInputModel<T>
//
// Owns the reactive loading state for AutocompleteInput.  Because it is a
// plain Rust struct (not a Leptos component), it can carry the
// `T: DeserializeOwned` bound needed to fetch data, while still making `T`
// visible to the Leptos #[component] macro through the prop type
// `model: AutocompleteInputModel<T>`.
//
// Usage:
//   let model = AutocompleteInputModel::<MyType>::new(
//       ResourcePath::new(MyEndpoint::ITEMS.to_uri()),
//       auth_token_signal,
//   );
// ---------------------------------------------------------------------------

#[derive(Clone, Copy)]
pub struct AutocompleteInputModel<T: 'static> {
    pub options:     RwSignal<Vec<T>>,
    pub error:       RwSignal<Option<String>>,
    pub loading:     RwSignal<bool>,
    refresh_trigger: RwSignal<u32>,
}

impl<T> AutocompleteInputModel<T>
where
    T: serde::de::DeserializeOwned + Clone + Send + Sync + 'static,
{
    pub fn new(path: ResourcePath, auth: Signal<Option<String>>) -> Self {
        let model = Self {
            options:         RwSignal::new(vec![]),
            error:           RwSignal::new(None),
            loading:         RwSignal::new(false),
            refresh_trigger: RwSignal::new(0u32),
        };
        let path_str = path.to_string();
        let options  = model.options;
        let error    = model.error;
        let loading  = model.loading;
        let refresh  = model.refresh_trigger;
        Effect::new(move |_| {
            let _ = refresh.get(); // tracked — re-runs on manual refresh
            let Some(token) = auth.get() else { return; };
            let path = path_str.clone();
            loading.set(true);
            leptos::task::spawn_local(async move {
                match crate::http::send_get::<Vec<T>>(&token, &path).await {
                    Ok(rows) => { options.set(rows); error.set(None); }
                    Err(e)   => error.set(Some(e.to_string())),
                }
                loading.set(false);
            });
        });
        model
    }

    pub fn refresh(&self) {
        self.refresh_trigger.update(|n| *n += 1);
    }
}

// ---------------------------------------------------------------------------
// AutocompleteInput<T>
//
// Props:
//   model       — created via AutocompleteInputModel::new(); owns loading state
//   item_key    — unique string key per item (keyboard navigation)
//   label       — display text per item (filtering and dropdown text)
//   input_value — the controlled text-input value
//   on_input    — called on every keystroke with the new text
//   on_select   — called with Some(T) on selection, None on blur with no match
// ---------------------------------------------------------------------------

#[component]
pub fn AutocompleteInput<T>(
    model: AutocompleteInputModel<T>,
    item_key: impl Fn(&T) -> String + Clone + Send + Sync + 'static,
    label: impl Fn(&T) -> String + Clone + Send + Sync + 'static,
    input_value: Signal<String>,
    on_input: impl Fn(String) + Clone + Send + Sync + 'static,
    on_select: impl Fn(Option<T>) + Clone + Send + Sync + 'static,
) -> impl IntoView
where
    T: Clone + Send + Sync + 'static,
{
    // Arc lets multiple closures — including those inside <Show> which re-invokes
    // its children closure on every toggle — share these without consuming them.
    let item_key: Arc<dyn Fn(&T) -> String + Send + Sync> = Arc::new(item_key);
    let label: Arc<dyn Fn(&T) -> String + Send + Sync> = Arc::new(label);
    let on_select: Arc<dyn Fn(Option<T>) + Send + Sync> = Arc::new(on_select);

    let options         = model.options;
    let loading         = model.loading;
    let refresh_trigger = model.refresh_trigger;

    let show_dropdown = RwSignal::new(false);
    let selected_key: RwSignal<Option<String>> = RwSignal::new(None);

    let label_filter = Arc::clone(&label);
    let filtered = Signal::derive(move || {
        let text = input_value.get().to_lowercase();
        if text.is_empty() { return vec![]; }
        options.get()
            .into_iter()
            .filter(|p| label_filter(p).to_lowercase().contains(&text))
            .collect::<Vec<_>>()
    });

    let ik_down = Arc::clone(&item_key);
    let on_sel_key = Arc::clone(&on_select);
    let handle_keydown = move |ev: KeyboardEvent| {
        let key = ev.key();
        let items = filtered.get_untracked();
        if key == "ArrowDown" {
            ev.prevent_default();
            if !items.is_empty() {
                show_dropdown.set(true);
                let cur = selected_key.get_untracked()
                    .and_then(|k| items.iter().position(|p| ik_down(p) == k));
                let next = match cur { None => 0, Some(i) => (i + 1).min(items.len() - 1) };
                selected_key.set(Some(ik_down(&items[next])));
            }
        } else if key == "ArrowUp" {
            ev.prevent_default();
            let cur = selected_key.get_untracked()
                .and_then(|k| items.iter().position(|p| ik_down(p) == k));
            selected_key.set(match cur {
                None | Some(0) => None,
                Some(i) => Some(ik_down(&items[i - 1])),
            });
        } else if key == "Enter" || key == " " {
            if let Some(k) = selected_key.get_untracked() {
                if let Some(item) = items.into_iter().find(|p| ik_down(p) == k) {
                    ev.prevent_default();
                    on_sel_key(Some(item));
                    show_dropdown.set(false);
                    selected_key.set(None);
                }
            }
        } else if key == "Escape" {
            show_dropdown.set(false);
            selected_key.set(None);
        }
    };

    let on_input_cb = on_input.clone();
    let label_blur = Arc::clone(&label);
    let on_sel_blur = Arc::clone(&on_select);

    view! {
        <div style="position: relative;">
            <div class="input-wrapper">
                <input
                    style="width: 100%; padding-right: 2rem; box-sizing: border-box;"
                    prop:value=move || input_value.get()
                    on:input=move |ev| {
                        let text = event_target_value(&ev);
                        let non_empty = !text.is_empty();
                        on_input_cb(text);
                        show_dropdown.set(non_empty);
                        selected_key.set(None);
                    }
                    on:keydown=handle_keydown
                    on:blur=move |_| {
                        let text = input_value.get_untracked().to_lowercase();
                        let matched = options.get_untracked()
                            .into_iter()
                            .find(|p| label_blur(p).to_lowercase() == text);
                        on_sel_blur(matched);
                        show_dropdown.set(false);
                        selected_key.set(None);
                    }
                    required
                />
                <span
                    class=move || if loading.get() {
                        "input-button input-refresh-spinning"
                    } else {
                        "clockwise-gapped-circle-arrow input-button"
                    }
                    title="Refresh"
                    on:click=move |_| {
                        if !loading.get_untracked() {
                            refresh_trigger.update(|n| *n += 1);
                        }
                    }
                />
            </div>
            <Show when=move || show_dropdown.get() && !filtered.get().is_empty()>
                <div class="autocomplete-dropdown"
                    on:mousedown=|ev: MouseEvent| ev.prevent_default()
                >
                    <For
                        each=move || filtered.get()
                        key={
                            let k = Arc::clone(&item_key);
                            move |p: &T| k(p)
                        }
                        children={
                            let ik = Arc::clone(&item_key);
                            let lb = Arc::clone(&label);
                            let on_sel = Arc::clone(&on_select);
                            move |item: T| {
                                let key_str = ik(&item);
                                let it = item.clone();
                                let display = lb(&item);
                                let cb = Arc::clone(&on_sel);
                                view! {
                                    <div
                                        class="autocomplete-item"
                                        class:autocomplete-item-selected=move || {
                                            selected_key.get().as_deref() == Some(key_str.as_str())
                                        }
                                        on:click=move |_| {
                                            cb(Some(it.clone()));
                                            show_dropdown.set(false);
                                            selected_key.set(None);
                                        }
                                    >
                                        {display}
                                    </div>
                                }
                            }
                        }
                    />
                </div>
            </Show>
        </div>
    }
}
