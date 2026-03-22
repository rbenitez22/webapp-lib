mod autocomplete;
pub use autocomplete::{AutocompleteInput, AutocompleteInputModel};

#[cfg(feature = "auth")]
mod auth;
#[cfg(feature = "auth")]
pub use auth::{
    Account, AccountEditor, Invitations, InvitationEditForm, InvitationView,
    Login, Logout, NewAccount,
};

use leptos::html::Dialog;
use leptos::prelude::*;
use leptos::{component, view, IntoView};
use leptos_router::components::A;
use leptos_router::hooks::use_navigate;
use leptos_router::NavigateOptions;
use serde::de::DeserializeOwned;
use wasm_bindgen::JsCast;
use web_sys::{HtmlInputElement, MouseEvent};

use crate::reactive::{DataRow, LookupData, LookupDataRequest};

// ---------------------------------------------------------------------------
// MessageType / MessageModel  (generic enough to live in the library)
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub enum MessageType {
    Info,
    Warning,
    Error,
}

impl MessageType {
    pub fn get_icon_class(&self) -> &'static str {
        match self {
            MessageType::Info => "circle-latin-small-letter-i",
            MessageType::Warning => "warning-sign",
            MessageType::Error => "heavy-multiplication-x",
        }
    }
    pub fn get_style_class(&self) -> &'static str {
        match self {
            MessageType::Info => "info",
            MessageType::Warning => "warning",
            MessageType::Error => "error",
        }
    }
}

#[derive(Clone)]
pub struct MessageModel {
    pub message: String,
    pub message_type: MessageType,
}

impl MessageModel {
    pub fn empty() -> Self {
        Self { message: String::new(), message_type: MessageType::Info }
    }
    pub fn info(message: String) -> Self {
        Self { message, message_type: MessageType::Info }
    }
    pub fn error(message: String) -> Self {
        Self { message, message_type: MessageType::Error }
    }
}

// ---------------------------------------------------------------------------
// Dialog helpers
// ---------------------------------------------------------------------------

pub fn open_dialog(dialog_ref: NodeRef<Dialog>) {
    if let Some(dialog) = dialog_ref.get() {
        let _ = dialog.show_modal();
    }
}

pub fn close_dialog(dialog_ref: NodeRef<Dialog>) {
    if let Some(dialog) = dialog_ref.get_untracked() {
        dialog.close();
    }
}

// ---------------------------------------------------------------------------
// submit_form — finds the nearest <form> ancestor and submits it
// ---------------------------------------------------------------------------

pub fn submit_form(ev: MouseEvent) {
    if let Some(target) = ev.target() {
        if let Some(form) = target
            .dyn_ref::<web_sys::HtmlElement>()
            .and_then(|el| el.closest("form").ok())
            .flatten()
            .and_then(|f| f.dyn_into::<web_sys::HtmlFormElement>().ok())
        {
            let _ = form.request_submit();
        }
    }
}

// ---------------------------------------------------------------------------
// render_options — datalist-style <option> list from a signal
// ---------------------------------------------------------------------------

pub fn render_options<D>(options: RwSignal<Vec<D>>, loading: RwSignal<bool>) -> impl IntoView
where
    D: DataRow + DeserializeOwned + Clone + Send + Sync + 'static,
{
    view! {
        {move || {
            if loading.get() {
                view! { <option value="" disabled selected hidden>"Loading..."</option> }.into_any()
            } else {
                options.get().iter().map(|o| {
                    let name = o.get_name();
                    view! { <option value=name /> }
                }).collect_view().into_any()
            }
        }}
    }
}

// ---------------------------------------------------------------------------
// DeleteRowButton
// ---------------------------------------------------------------------------

#[component]
pub fn DeleteRowButton(on_delete: impl Fn(leptos::ev::SubmitEvent) + 'static) -> impl IntoView {
    let deleting = RwSignal::new(false);
    view! {
        <form on:submit=on_delete>
            <div
                class="delete-button"
                class:delete-button--deleting=move || deleting.get()
                on:click=move |ev: MouseEvent| {
                    if deleting.get() { return; }
                    deleting.set(true);
                    submit_form(ev);
                }
            >
                {move || if deleting.get() {
                    view! { <div class="delete-spinner" /> }.into_any()
                } else {
                    view! { <></> }.into_any()
                }}
            </div>
        </form>
    }
}

// ---------------------------------------------------------------------------
// DialogTitle
// ---------------------------------------------------------------------------

#[component]
pub fn DialogTitle(title: String, on_close: impl Fn(MouseEvent) + 'static) -> impl IntoView {
    view! {
        <div class="title-bar">
            <span>{title}</span>
            <div class="button close-dialog multiply" on:click=on_close />
        </div>
    }
}

// ---------------------------------------------------------------------------
// ComponentTitleBar
// ---------------------------------------------------------------------------

#[component]
pub fn ComponentTitleBar(
    title: String,
    on_add: impl Fn(MouseEvent) + 'static,
    #[prop(default = true)] show_add: bool,
    #[prop(optional)] children: Option<Children>,
) -> impl IntoView {
    view! {
        <div class="title-bar">
            {show_add.then(|| view! {
                <div class="button left-button plus-sign" on:click=on_add />
            })}
            {children.map(|c| c())}
            <span>{title}</span>
        </div>
    }
}

// ---------------------------------------------------------------------------
// BaseComponent — redirects to login_path when not authenticated
// ---------------------------------------------------------------------------

#[component]
pub fn BaseComponent(
    children: Children,
    #[prop(into)] is_authenticated: Signal<bool>,
    #[prop(default = "/login")] login_path: &'static str,
) -> impl IntoView {
    let navigate = use_navigate();
    Effect::new(move |_| {
        if !is_authenticated.get() {
            navigate(
                login_path,
                NavigateOptions { replace: true, scroll: true, ..Default::default() },
            );
        }
    });
    children()
}

// ---------------------------------------------------------------------------
// Spinner
// ---------------------------------------------------------------------------

#[component]
pub fn Spinner(
    #[prop(default = "Loading…".to_string())] message: String,
) -> impl IntoView {
    view! {
        <div class="spinner-overlay">
            <div class="spinner"></div>
            <p class="loading-text">{message}</p>
        </div>
    }
}

// ---------------------------------------------------------------------------
// DataGrid — generic scrollable list
// ---------------------------------------------------------------------------

#[component]
pub fn DataGrid<T, F, IV>(
    data: impl Fn() -> Vec<T> + Clone + Send + Sync + 'static,
    cell: F,
) -> impl IntoView
where
    F: Fn(&T) -> IV + Send + Sync + Clone + 'static,
    IV: IntoView + 'static,
{
    let render = cell.clone();
    view! {
        <div class="scrollable-reverse data-grid">
            {move || {
                let cell = render.clone();
                data().iter().map(move |e| view! {
                    <div class="grid-cell snap-to-bottom">{cell(e)}</div>
                }).collect_view()
            }}
        </div>
    }
}

// ---------------------------------------------------------------------------
// ListComponentView — loading/error/data switcher backed by individual signals
// ---------------------------------------------------------------------------

#[component]
pub fn ListComponentView<D, F, IV>(
    #[prop(into)] loading: Signal<bool>,
    #[prop(into)] error: Signal<Option<String>>,
    #[prop(into)] data: Signal<Vec<D>>,
    cell_view: F,
) -> impl IntoView
where
    D: Clone + Send + Sync + 'static,
    F: Fn(&D) -> IV + Send + Sync + Clone + 'static,
    IV: IntoView + 'static,
{
    view! {
        {move || {
            if loading.get() {
                view! { <Spinner /> }.into_any()
            } else if let Some(err) = error.get() {
                view! { <p style="color: red;">{err}</p> }.into_any()
            } else {
                view! {
                    <DataGrid data=move || data.get() cell=cell_view.clone() />
                }.into_any()
            }
        }}
    }
}

// ---------------------------------------------------------------------------
// CheckBox
// ---------------------------------------------------------------------------

#[component]
pub fn CheckBox(
    id: &'static str,
    value: impl Fn() -> bool + Send + Sync + 'static,
    on_change: impl Fn(bool) + Send + Sync + 'static,
    #[prop(optional)] style: Option<&'static str>,
    #[prop(optional)] class: Option<&'static str>,
) -> impl IntoView {
    view! {
        <input
            id=id
            type="checkbox"
            style=style.unwrap_or_default()
            class=class.unwrap_or_default()
            prop:checked=move || value()
            on:change=move |ev| {
                let checked = ev.target()
                    .and_then(|t| t.dyn_into::<HtmlInputElement>().ok())
                    .map(|i| i.checked())
                    .unwrap_or(false);
                on_change(checked);
            }
        />
    }
}

// ---------------------------------------------------------------------------
// Message
// ---------------------------------------------------------------------------

#[component]
pub fn Message(message: RwSignal<MessageModel>) -> impl IntoView {
    view! {
        {move || {
            let model = message.get();
            if model.message.is_empty() {
                view! { <></> }.into_any()
            } else {
                let class_name = format!(
                    "message {}-message {}",
                    model.message_type.get_style_class(),
                    model.message_type.get_icon_class(),
                );
                view! { <div class=class_name>{model.message.clone()}</div> }.into_any()
            }
        }}
    }
}

// ---------------------------------------------------------------------------
// DeleteConfirmDialog
// ---------------------------------------------------------------------------

#[component]
pub fn DeleteConfirmDialog(
    dialog_ref: NodeRef<Dialog>,
    message: String,
    on_confirm: impl Fn() + 'static,
) -> impl IntoView {
    let on_cancel = move |_| close_dialog(dialog_ref);
    let on_delete = move |_| {
        on_confirm();
        close_dialog(dialog_ref);
    };
    view! {
        <dialog node_ref=dialog_ref class="dialog">
            <div class="title-bar">
                <span>"Confirm Delete"</span>
            </div>
            <div style="padding: 1rem;">
                <p>{message}</p>
                <div class="form-actions" style="justify-content: space-between;">
                    <button class="button" style="margin-right: 0.25rem" on:click=on_cancel>
                        "Cancel"
                    </button>
                    <button style="background: var(--error-color); color: white;" on:click=on_delete>
                        "Delete"
                    </button>
                </div>
            </div>
        </dialog>
    }
}

// ---------------------------------------------------------------------------
// ConfirmDialog
// ---------------------------------------------------------------------------

#[component]
pub fn ConfirmDialog(
    dialog_ref: NodeRef<Dialog>,
    is_visible: RwSignal<bool>,
    on_confirm: impl Fn() + 'static,
    on_cancel: impl Fn() + 'static,
) -> impl IntoView {
    view! {
        <dialog node_ref=dialog_ref class="confirm-dialog" open=is_visible.get()>
            <p>"Are you sure you want to proceed?"</p>
            <button on:click=move |_| {
                on_confirm();
                if let Some(d) = dialog_ref.get_untracked() { d.close(); }
            }>"Confirm"</button>
            <button on:click=move |_| {
                on_cancel();
                if let Some(d) = dialog_ref.get_untracked() { d.close(); }
            }>"Cancel"</button>
        </dialog>
    }
}

// ---------------------------------------------------------------------------
// SideMenu + MenuLink
//
// SideMenu manages the open/close state and provides it via context so that
// MenuLink children can close the drawer without needing the signal passed
// explicitly.
//
// Usage:
//   <SideMenu title="My App" user_name=name_signal>
//       <MenuLink href="/home">"Home"</MenuLink>
//       <MenuLink href="/logout">"Logout"</MenuLink>
//   </SideMenu>
// ---------------------------------------------------------------------------

#[component]
pub fn SideMenu(
    children: Children,
    #[prop(optional, into)] title: Option<String>,
    /// Reactive username shown as a badge in the toolbar and at the bottom of
    /// the drawer. Pass `Signal::derive(move || user.get().display_name)`.
    #[prop(optional, into)] user_name: Option<Signal<String>>,
    /// When provided, the entire menu is hidden while the user is logged out.
    #[prop(optional, into)] is_authenticated: Option<Signal<bool>>,
) -> impl IntoView {
    let open = RwSignal::new(false);
    provide_context(open);

    view! {
        <div>
            <div class="top-toolbar">
                <button
                    class="trigram-for-heaven hamburger-btn"
                    on:click=move |_| open.set(true)
                    aria-label="Open menu"
                />
                {title.map(|t| view! { <span style="color: var(--white);">{t}</span> })}
                {user_name.map(|name| view! {
                    <Show when=move || is_authenticated.map(|a| a.get()).unwrap_or(true)>
                        <span class="user-badge">{move || name.get()}</span>
                    </Show>
                })}
            </div>

            <div
                class="side-menu-overlay"
                class:visible=move || open.get()
                on:click=move |_| open.set(false)
            />

            <nav class="side-menu" class:open=move || open.get()>
                <button class="side-menu-close multiply button" on:click=move |_| open.set(false) />
                {children()}
                {user_name.map(|name| view! {
                    <span class="side-menu-username">{move || name.get()}</span>
                })}
            </nav>
        </div>
    }
}

/// A nav link that automatically closes the SideMenu drawer when clicked.
/// Must be used inside a `<SideMenu>`.
#[component]
pub fn MenuLink(href: &'static str, children: Children) -> impl IntoView {
    let open = use_context::<RwSignal<bool>>()
        .expect("MenuLink must be used inside SideMenu");
    view! {
        <A href=href on:click=move |_| open.set(false)>
            {children()}
        </A>
    }
}

// ---------------------------------------------------------------------------
// Password entropy helpers
// ---------------------------------------------------------------------------

pub fn calc_password_entropy(password: &str) -> f64 {
    if password.is_empty() { return 0.0; }
    let mut charset: u32 = 0;
    if password.chars().any(|c| c.is_ascii_lowercase()) { charset += 26; }
    if password.chars().any(|c| c.is_ascii_uppercase()) { charset += 26; }
    if password.chars().any(|c| c.is_ascii_digit())     { charset += 10; }
    if password.chars().any(|c| c.is_ascii_punctuation() || c.is_ascii_whitespace()) { charset += 33; }
    if password.chars().any(|c| !c.is_ascii())           { charset += 64; }
    if charset == 0 { return 0.0; }
    password.len() as f64 * (charset as f64).log2()
}

#[component]
pub fn EntropyIndicator(
    password: impl Fn() -> String + Send + Sync + Clone + 'static,
    min_entropy: f64,
    is_valid: RwSignal<bool>,
) -> impl IntoView {
    let pw_for_effect = password.clone();
    Effect::new(move |_| {
        let pw = pw_for_effect();
        is_valid.set(!pw.is_empty() && calc_password_entropy(&pw) >= min_entropy);
    });

    view! {
        <div class="entropy-indicator">
            {move || {
                let pw = password();
                if pw.is_empty() {
                    return view! { <span></span> }.into_any();
                }
                let bits = calc_password_entropy(&pw);
                let (label, color) = if bits < 25.0 {
                    ("Very Weak", "var(--error-color)")
                } else if bits < 40.0 {
                    ("Weak", "var(--warning-color)")
                } else if bits < min_entropy {
                    ("Fair", "#a9ea12")
                } else {
                    ("Strong", "#1a7a10")
                };
                view! {
                    <span style=format!("color: {}; font-size: 0.8rem;", color)>
                        {format!("Strength: {} ({:.0} bits)", label, bits)}
                    </span>
                }.into_any()
            }}
        </div>
    }
}

// ---------------------------------------------------------------------------
// VerifiedPassword
// ---------------------------------------------------------------------------

#[component]
pub fn VerifiedPassword(
    password: RwSignal<String>,
    is_valid: RwSignal<bool>,
    min_entropy: f64,
) -> impl IntoView {
    let show_password = RwSignal::new(false);
    let show_confirm = RwSignal::new(false);
    let password_confirm = RwSignal::new(String::new());
    let entropy_display = RwSignal::new(false);

    let entropy_passes = Memo::new(move |_| {
        let pw = password.get();
        !pw.is_empty() && calc_password_entropy(&pw) >= min_entropy
    });
    let passwords_match = Memo::new(move |_| {
        let confirm = password_confirm.get();
        !confirm.is_empty() && password.get() == confirm
    });

    Effect::new(move |_| {
        is_valid.set(entropy_passes.get() && passwords_match.get());
    });

    view! {
        <div class="form-field">
            <label for="password">"Password"</label>
            <div class="password-wrapper">
                <input
                    id="password"
                    class="password-input"
                    type=move || if show_password.get() { "text" } else { "password" }
                    autocomplete="new-password"
                    prop:value=move || password.get()
                    on:input=move |ev| password.set(event_target_value(&ev))
                    required
                />
                <span
                    class=move || if show_password.get() { "open-lock password-toggle" } else { "closed-lock-with-key password-toggle" }
                    on:click=move |_| show_password.update(|v| *v = !*v)
                />
            </div>
            <EntropyIndicator
                password=move || password.get()
                min_entropy=min_entropy
                is_valid=entropy_display
            />
        </div>
        <div class="form-field">
            <label for="password_confirm">"Confirm Password"</label>
            <div class="password-wrapper">
                <input
                    id="password_confirm"
                    class="password-input"
                    type=move || if show_confirm.get() { "text" } else { "password" }
                    autocomplete="new-password"
                    prop:value=move || password_confirm.get()
                    on:input=move |ev| password_confirm.set(event_target_value(&ev))
                    required
                />
                <span
                    class=move || if show_confirm.get() { "open-lock password-toggle" } else { "closed-lock-with-key password-toggle" }
                    on:click=move |_| show_confirm.update(|v| *v = !*v)
                />
            </div>
            <div class="entropy-indicator">
                {move || {
                    let confirm = password_confirm.get();
                    if confirm.is_empty() {
                        view! { <span></span> }.into_any()
                    } else if passwords_match.get() {
                        view! { <span style="color: #2b9e1f; font-size: 0.8rem;">"✓ Passwords match"</span> }.into_any()
                    } else {
                        view! { <span style="color: var(--error-color); font-size: 0.8rem;">"Passwords do not match"</span> }.into_any()
                    }
                }}
            </div>
        </div>
    }
}

// ---------------------------------------------------------------------------
// LookupData components
// ---------------------------------------------------------------------------

/// A read-only row for a `LookupData` record inside a `ListComponentView`.
///
/// `on_delete` should be created by the caller via the reactive layer's
/// `create_delete_event`, allowing this component to stay decoupled from
/// whichever `ListComponentModel` variant the consuming app uses.
#[component]
pub fn LookupDataView(
    model: LookupData,
    on_delete: impl Fn(leptos::ev::SubmitEvent) + 'static,
    on_click: impl Fn() + 'static,
) -> impl IntoView {
    view! {
        <div class="component">
            <div>
                <DeleteRowButton on_delete=on_delete />
            </div>
            <div class="button" on:click=move |_| on_click()>
                {model.name.clone()}
            </div>
            <div class="description">
                {model.description.clone()}
            </div>
        </div>
    }
}

/// A create/edit form for a `LookupData` record.
#[component]
pub fn LookupDataEditForm(
    request_model: RwSignal<LookupDataRequest>,
    on_submit: impl Fn(leptos::ev::SubmitEvent) + 'static,
    saving: RwSignal<bool>,
) -> impl IntoView {
    view! {
        <div class="form-container">
            <form on:submit=on_submit>
                <div class="form-components">
                    <div>
                        <label for="name">Name</label>
                        <input
                            type="text"
                            name="name"
                            id="name"
                            placeholder="Name"
                            prop:value=move || request_model.get().name
                            on:input=move |ev| request_model.update(|req| req.name = event_target_value(&ev))
                            required
                            style="width:200px;margin:2px"
                        />
                        <label for="description">Description</label>
                        <input
                            id="description"
                            name="description"
                            type="text"
                            placeholder="Description"
                            prop:value=move || request_model.get().description.clone().unwrap_or_default()
                            on:input=move |ev| request_model.update(|req| req.description = Some(event_target_value(&ev)))
                            style="width:400px;margin:2px;margin-left:4px"
                        />
                    </div>
                    <div class="form-actions">
                        <button
                            type="submit"
                            class="button"
                            prop:disabled=move || saving.get()
                            class:saving=move || saving.get()
                        >
                            " Save "
                        </button>
                    </div>
                </div>
            </form>
        </div>
    }
}

/// A `<select>` dropdown that loads its options from `source_name` via the API.
///
/// Requires the auth token to be provided as context (`Signal<Option<String>>`)
/// somewhere above in the component tree — call `provide_context(token_signal)`
/// in your root component.  The `source_name` is used as both the API path and
/// the local-storage cache key.
#[component]
pub fn SelectMenu(
    source_name: &'static str,
    field_name: &'static str,
    value: impl Fn() -> Option<String> + Send + Sync + Clone + 'static,
    on_change: impl Fn(String) + Send + Sync + Clone + 'static,
    #[prop(default = true)] required: bool,
    #[prop(default = false)] force_refresh: bool,
) -> impl IntoView {
    let auth = use_context::<Signal<Option<String>>>().unwrap_or(Signal::derive(|| None));
    let options = RwSignal::new(Vec::<LookupData>::new());
    let loaded = RwSignal::new(false);

    crate::reactive::load_reference_data_list(source_name, auth, options, loaded, force_refresh);

    view! {
        {move || loaded.get().then(|| {
            let on_change_clone = on_change.clone();
            let current_value = value().unwrap_or_default();
            view! {
                <select
                    id=field_name
                    name=field_name
                    prop:value=current_value.clone()
                    on:change=move |ev| { on_change_clone(event_target_value(&ev)); }
                    required=required
                >
                    <option value="" disabled=required selected=required hidden=required>
                        "Select a Value"
                    </option>
                    {options.get().iter().map(|option| {
                        let is_selected = option.id == current_value;
                        view! {
                            <option value=option.id.clone() selected=is_selected>
                                {option.name.clone()}
                            </option>
                        }
                    }).collect_view()}
                </select>
            }
        })}
    }
}

/// Resolves a `LookupData` id to its display name, loading options from
/// `source_name` via the API.
///
/// Requires the auth token in context — see [`SelectMenu`].
/// Renders the resolved name, or the raw id if not found.
#[component]
pub fn LookupDataDisplay(
    source_name: &'static str,
    value: impl Fn() -> String + Send + Sync + Clone + 'static,
) -> impl IntoView {
    let auth = use_context::<Signal<Option<String>>>().unwrap_or(Signal::derive(|| None));
    let options = RwSignal::new(Vec::<LookupData>::new());
    let loaded = RwSignal::new(false);

    crate::reactive::load_reference_data_list(source_name, auth, options, loaded, false);

    view! {
        {move || {
            if !loaded.get() { return String::new(); }
            let id = value();
            options.get().iter()
                .find(|item| item.get_id() == id)
                .map(|item| item.name.clone())
                .unwrap_or(id)
        }}
    }
}