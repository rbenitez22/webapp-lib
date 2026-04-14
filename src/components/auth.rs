use leptos::html::Dialog;
use leptos::prelude::codee::string::JsonSerdeCodec;
use leptos::prelude::*;
use leptos::{component, view, IntoView};
use leptos_router::hooks::use_navigate;
use leptos_router::NavigateOptions;
use leptos_use::storage::use_local_storage;

use crate::auth::{
    paths, read_auth_token_from_local_storage, submit_login_request,
    use_auth_signal, use_auth_token,
    Auth, ChangePasswordRequest, InvitationRequest, LoginResponse,
    UpdateNameRequest, UserAccount, UserAccountRequest,
    STORAGE_AUTH_KEY, STORAGE_USER_KEY,
};
use crate::http::{send_request, ApiRequest, HttpMethod};
use crate::reactive::{create_delete_event, create_persist_event, load_list_component_model, ListComponentModel};
use crate::storage::{read_from_local_storage, write_to_local_storage};

use super::{
    open_dialog, close_dialog, BaseComponent, CheckBox, ComponentTitleBar,
    DeleteRowButton, DialogTitle, ListComponentView, Message, MessageModel,
    VerifiedPassword,
};

// ---------------------------------------------------------------------------
// Login
// ---------------------------------------------------------------------------

#[component]
pub fn Login() -> impl IntoView {
    let (email, set_email) = signal(String::new());
    let (password, set_password) = signal(String::new());
    let (show_password, set_show_password) = signal(false);
    let nav = use_navigate();
    let (_, set_auth, _) = use_local_storage::<Auth, JsonSerdeCodec>(STORAGE_AUTH_KEY);
    let (login_msg, set_login_msg) = signal(String::new());
    let (loading, set_loading) = signal(false);
    let on_submit = submit_login_request(email, password, nav, set_auth, set_login_msg, set_loading);

    view! {
        <div class="main">
            <Show when=move || loading.get()>
                <div class="spinner-overlay">
                    <div class="spinner"></div>
                </div>
            </Show>
            <div class="login-container">
                <h2 class="login-title">"Login"</h2>
                {move || {
                    let msg = login_msg.get();
                    if msg.is_empty() {
                        view! { <></> }.into_any()
                    } else {
                        view! {
                            <div style="color: red; margin-bottom: 1rem; padding: 0.5rem; background-color: #fee; border: 1px solid #fcc; border-radius: 4px;">
                                {msg}
                            </div>
                        }.into_any()
                    }
                }}
                <form on:submit=on_submit class="login-form">
                    <div style="margin-bottom: 1rem;">
                        <label for="email" style="display: block; margin-bottom: 0.5rem;">"E-mail:"</label>
                        <input
                            type="text"
                            id="email"
                            name="email"
                            autocomplete="off"
                            prop:value=email
                            on:input=move |ev| set_email.set(event_target_value(&ev))
                            style="width: 100%; padding: 0.5rem; font-size: 1rem; box-sizing: border-box;"
                            required
                        />
                    </div>
                    <div style="margin-bottom: 1rem;">
                        <label for="password" style="display: block; margin-bottom: 0.5rem;">"Password:"</label>
                        <div class="password-wrapper">
                            <input
                                type=move || if show_password.get() { "text" } else { "password" }
                                id="password"
                                name="password"
                                autocomplete="current-password"
                                prop:value=password
                                on:input=move |ev| set_password.set(event_target_value(&ev))
                                style="width: 100%; padding: 0.5rem 2.5rem 0.5rem 0.5rem; font-size: 1rem; box-sizing: border-box;"
                                required
                            />
                            <span
                                class=move || if show_password.get() {
                                    "open-lock password-toggle"
                                } else {
                                    "closed-lock-with-key password-toggle"
                                }
                                on:click=move |_| set_show_password.update(|v| *v = !*v)
                            />
                        </div>
                    </div>
                    <button class="button login-submit" type="submit">"Login"</button>
                    <p class="login-create-account">
                        "Do not have an account? "
                        <a href="/account/create">"Create One"</a>
                    </p>
                </form>
            </div>
        </div>
    }
}

// ---------------------------------------------------------------------------
// Logout
// ---------------------------------------------------------------------------

#[component]
pub fn Logout(
    #[prop(optional)] on_logout: Option<Callback<()>>,
) -> impl IntoView {
    let navigate = use_navigate();

    Effect::new(move |_| {
        if let Some(cb) = on_logout {
            cb.run(());
        }
        write_to_local_storage(STORAGE_AUTH_KEY, &Auth::default());
        crate::storage::dispatch_storage_event(STORAGE_AUTH_KEY);
        write_to_local_storage(STORAGE_USER_KEY, &UserAccount::default());
        crate::storage::dispatch_storage_event(STORAGE_USER_KEY);
        navigate(
            paths().login_page,
            NavigateOptions { replace: true, scroll: true, ..Default::default() },
        );
    });

    view! { <h2>"Logout"</h2> }
}

// ---------------------------------------------------------------------------
// Account
// ---------------------------------------------------------------------------

#[component]
pub fn Account() -> impl IntoView {
    let user: UserAccount =
        read_from_local_storage(STORAGE_USER_KEY).unwrap_or_default();
    let dialog_ref: NodeRef<Dialog> = NodeRef::new();

    view! {
        <BaseComponent is_authenticated=use_auth_signal()>
            <div class="main">
                <div class="title-bar">
                    <span>"Account"</span>
                </div>
                <div class="component-section">
                    <NameUpdateForm initial_name=user.display_name.clone() />
                    <div class="form-field">
                        <span class="value-label">"Email"</span>
                        <span>{user.email.clone()}</span>
                    </div>
                    <div class="form-field">
                        <span class="value-label">"Admin"</span>
                        <span>{if user.admin { "Yes" } else { "No" }}</span>
                    </div>
                    <div>
                        <button class="button" on:click=move |_| open_dialog(dialog_ref)>
                            "Change Password"
                        </button>
                    </div>
                </div>
            </div>
            <dialog node_ref=dialog_ref class="dialog">
                <DialogTitle
                    title="Change Password".to_string()
                    on_close=move |_| close_dialog(dialog_ref)
                />
                <PasswordUpdateForm on_saved=move || close_dialog(dialog_ref) />
            </dialog>
        </BaseComponent>
    }
}

#[component]
fn NameUpdateForm(initial_name: String) -> impl IntoView {
    let display_name = RwSignal::new(initial_name);
    let status = RwSignal::new(MessageModel::empty());
    let saving = RwSignal::new(false);

    let on_submit = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        let name = display_name.get_untracked();
        saving.set(true);
        leptos::task::spawn_local(async move {
            let Some(token) = read_auth_token_from_local_storage() else {
                saving.set(false);
                return;
            };
            let payload = UpdateNameRequest { display_name: name };
            let req = ApiRequest::new(&HttpMethod::PUT, Some(&token), paths().update_name, &payload);
            match send_request::<UpdateNameRequest, UserAccount>(req).await {
                Ok(updated) => {
                    saving.set(false);
                    write_to_local_storage(STORAGE_USER_KEY, &updated);
                    status.set(MessageModel::info("Name updated.".to_string()));
                }
                Err(e) => {
                    saving.set(false);
                    status.set(MessageModel::error(e.message));
                }
            }
        });
    };

    view! {
        <Message message=status />
        <form on:submit=on_submit autocomplete="off">
            <div class="form-field">
                <label for="display_name">"Display Name"</label>
                <input
                    id="display_name"
                    prop:value=move || display_name.get()
                    on:input=move |ev| display_name.set(event_target_value(&ev))
                    required
                />
            </div>
            <div>
                <button
                    class="button"
                    type="submit"
                    prop:disabled=move || saving.get()
                    class:saving=move || saving.get()
                >"Update Name"</button>
            </div>
        </form>
    }
}

#[component]
fn PasswordUpdateForm(on_saved: impl Fn() + Clone + 'static) -> impl IntoView {
    let current_password = RwSignal::new(String::new());
    let new_password = RwSignal::new(String::new());
    let password_valid = RwSignal::new(false);
    let status = RwSignal::new(MessageModel::empty());
    let saving = RwSignal::new(false);

    let on_submit = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        if !password_valid.get_untracked() { return; }
        let current_pw = current_password.get_untracked();
        let pw = new_password.get_untracked();
        let on_saved = on_saved.clone();
        saving.set(true);
        leptos::task::spawn_local(async move {
            let Some(token) = read_auth_token_from_local_storage() else {
                saving.set(false);
                return;
            };
            let payload = ChangePasswordRequest {
                current_password: current_pw,
                new_password: pw,
            };
            let req = ApiRequest::new(
                &HttpMethod::POST,
                Some(&token),
                paths().change_password,
                &payload,
            );
            match send_request::<ChangePasswordRequest, LoginResponse>(req).await {
                Ok(response) => {
                    saving.set(false);
                    write_to_local_storage(STORAGE_AUTH_KEY, &Auth::from(response.token));
                    current_password.set(String::new());
                    new_password.set(String::new());
                    on_saved();
                }
                Err(e) => {
                    saving.set(false);
                    status.set(MessageModel::error(e.message));
                }
            }
        });
    };

    view! {
        <form on:submit=on_submit autocomplete="off">
            <div class="form-field">
                <label for="current_password">"Current Password"</label>
                <input
                    id="current_password"
                    type="password"
                    autocomplete="current-password"
                    prop:value=move || current_password.get()
                    on:input=move |ev| current_password.set(event_target_value(&ev))
                    required
                />
            </div>
            <div class="new-password-separator">
                <span>"New Password"</span>
            </div>
            <VerifiedPassword
                password=new_password
                min_entropy=paths().min_password_entropy
                is_valid=password_valid
            />
            <Message message=status />
            <div>
                <button
                    class="button"
                    type="submit"
                    prop:disabled=move || !password_valid.get() || saving.get()
                    class:saving=move || saving.get()
                >"Change Password"</button>
            </div>
        </form>
    }
}

// ---------------------------------------------------------------------------
// NewAccount
// ---------------------------------------------------------------------------

#[component]
pub fn NewAccount() -> impl IntoView {
    let nav = use_navigate();
    let form_model = RwSignal::new(UserAccountRequest::default());
    let on_saved = move || {
        nav(
            paths().after_register,
            NavigateOptions { replace: true, scroll: true, ..Default::default() },
        );
    };

    view! {
        <div class="main">
            <div class="login-container">
                <h2 class="login-title">"Create Account"</h2>
                <AccountEditor form_model=form_model on_saved=on_saved />
                <p class="login-create-account">
                    "Already have an account? "
                    <a href="/login">"Login"</a>
                </p>
            </div>
        </div>
    }
}

#[component]
pub fn AccountEditor(
    form_model: RwSignal<UserAccountRequest>,
    on_saved: impl Fn() + Clone + 'static,
) -> impl IntoView {
    let status = RwSignal::new(MessageModel::empty());
    let password_signal = RwSignal::new(String::new());
    let password_valid = RwSignal::new(false);
    let saving = RwSignal::new(false);

    Effect::new(move |_| {
        form_model.update(|u| u.password = password_signal.get());
    });

    let on_submit_inner = create_account_submit_event(form_model, status, saving, on_saved);
    let on_submit = move |ev: leptos::ev::SubmitEvent| {
        if !password_valid.get_untracked() {
            ev.prevent_default();
            status.set(MessageModel::error(
                "Password too weak or passwords do not match.".to_string(),
            ));
            return;
        }
        on_submit_inner(ev);
    };

    view! {
        <Message message=status.clone() />
        <form on:submit=on_submit autocomplete="off">
            <div class="form-field">
                <label for="display_name">"Display Name"</label>
                <input
                    id="display_name"
                    prop:value=move || form_model.get().display_name.clone()
                    on:input=move |ev| form_model.update(|u| u.display_name = event_target_value(&ev))
                    required
                />
            </div>
            <div class="form-field">
                <label for="email">"Email"</label>
                <input
                    id="email"
                    type="email"
                    autocomplete="off"
                    prop:value=move || form_model.get().email.clone()
                    on:input=move |ev| form_model.update(|u| u.email = event_target_value(&ev))
                    required
                />
            </div>
            <VerifiedPassword
                password=password_signal
                min_entropy=paths().min_password_entropy
                is_valid=password_valid
            />
            <button
                class="button login-submit"
                type="submit"
                prop:disabled=move || saving.get()
                class:saving=move || saving.get()
            >"Create Account"</button>
        </form>
    }
}

fn create_account_submit_event(
    form_model: RwSignal<UserAccountRequest>,
    status: RwSignal<MessageModel>,
    saving: RwSignal<bool>,
    on_saved: impl Fn() + Clone + 'static,
) -> impl Fn(leptos::ev::SubmitEvent) {
    move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        let model = form_model.get_untracked();
        let on_saved = on_saved.clone();
        saving.set(true);
        leptos::task::spawn_local(async move {
            let req = ApiRequest::new(&HttpMethod::POST, None, paths().accounts, &model);
            match send_request::<UserAccountRequest, LoginResponse>(req).await {
                Ok(response) => {
                    saving.set(false);
                    write_to_local_storage(STORAGE_USER_KEY, &response.user_account);
                    write_to_local_storage(STORAGE_AUTH_KEY, &Auth::from(response.token));
                    status.set(MessageModel::info("Account created successfully".to_string()));
                    on_saved();
                }
                Err(e) => {
                    saving.set(false);
                    status.set(MessageModel::error(e.message));
                }
            }
        });
    }
}

// ---------------------------------------------------------------------------
// Invitations
// ---------------------------------------------------------------------------

#[component]
pub fn Invitations() -> impl IntoView {
    let is_admin = read_from_local_storage::<UserAccount>(STORAGE_USER_KEY)
        .unwrap_or_default()
        .admin;
    let is_new = RwSignal::new(false);
    let dialog_ref: NodeRef<Dialog> = NodeRef::new();
    let model: ListComponentModel<InvitationRequest> =
        ListComponentModel::new(paths().invitations, use_auth_token());
    let edit_form_model = RwSignal::new(InvitationRequest::new());

    let on_save = move |_: &InvitationRequest| {
        edit_form_model.set(InvitationRequest::new());
        close_dialog(dialog_ref);
    };

    let on_submit = create_persist_event(model.clone(), edit_form_model, on_save);
    let nav = use_navigate();
    let model_clone = model.clone();

    load_list_component_model(model.clone(), nav, None, paths().login_page);

    view! {
        <BaseComponent is_authenticated=use_auth_signal()>
            <div class="main">
                <ComponentTitleBar
                    title="Invitations".to_string()
                    show_add=is_admin
                    on_add=move |_| {
                        edit_form_model.set(InvitationRequest::new());
                        is_new.set(true);
                        open_dialog(dialog_ref);
                    }
                />
                <dialog node_ref=dialog_ref class="dialog">
                    <DialogTitle
                        title="Edit Invitation".to_string()
                        on_close=move |_| close_dialog(dialog_ref)
                    />
                    <InvitationEditForm
                        is_new=is_new
                        form_model=edit_form_model
                        saving=model.saving
                        on_submit=on_submit
                    />
                </dialog>
                <ListComponentView
                    loading=model.loading
                    error=model.error
                    data=model.data
                    cell_view=move |current: &InvitationRequest| {
                        let current_clone = current.clone();
                        let model_clone = model_clone.clone();
                        view! {
                            <InvitationView
                                model=current.clone()
                                list_model=model_clone
                                is_admin=is_admin
                                on_click=move || {
                                    edit_form_model.set(current_clone.clone());
                                    is_new.set(false);
                                    open_dialog(dialog_ref);
                                }
                            />
                        }
                    }
                />
            </div>
        </BaseComponent>
    }
}

#[component]
pub fn InvitationView(
    model: InvitationRequest,
    list_model: ListComponentModel<InvitationRequest>,
    is_admin: bool,
    on_click: impl Fn() + 'static,
) -> impl IntoView {
    let on_delete = create_delete_event(model.email.clone(), list_model);
    view! {
        <div class="component">
            <div style="display: flex; align-items: center; gap: 0.5rem;">
                {if is_admin { Some(view! { <DeleteRowButton on_delete=on_delete /> }) } else { None }}
                <div style="flex: 1;">
                    <div class="button" on:click=move |_| on_click()>
                        {model.display_name.clone()}
                    </div>
                    <div class="description">
                        {model.email.clone()}
                        {if model.is_admin {
                            Some(view! {
                                <span class="value-label" style="margin-left: 0.75rem;">"Admin"</span>
                            })
                        } else { None }}
                    </div>
                </div>
            </div>
        </div>
    }
}

#[component]
pub fn InvitationEditForm(
    is_new: RwSignal<bool>,
    form_model: RwSignal<InvitationRequest>,
    on_submit: impl Fn(leptos::ev::SubmitEvent) + 'static,
    saving: RwSignal<bool>,
) -> impl IntoView {
    view! {
        <div class="form-container">
            <form on:submit=on_submit>
                <div class="form-components">
                    <div class="form-field">
                        <label for="email">"E-mail"</label>
                        <input
                            type="email"
                            id="email"
                            autocomplete="off"
                            prop:value=move || form_model.get().email.clone()
                            prop:disabled=move || !is_new.get()
                            on:input=move |ev| form_model.update(|r| r.email = event_target_value(&ev))
                            required
                        />
                    </div>
                    <div class="form-field">
                        <label for="display_name">"Display Name"</label>
                        <input
                            type="text"
                            id="display_name"
                            autocomplete="off"
                            prop:value=move || form_model.get().display_name.clone()
                            on:input=move |ev| form_model.update(|r| r.display_name = event_target_value(&ev))
                            required
                        />
                    </div>
                    <div class="form-field">
                        <label style="display: inline; margin-right: 0.5rem;" for="is_admin">"Admin"</label>
                        <CheckBox
                            style="display: inline; width: 1.5rem; height: 1.5rem;"
                            id="is_admin"
                            value=move || form_model.get().is_admin
                            on_change=move |v| form_model.update(|r| r.is_admin = v)
                        />
                    </div>
                </div>
                <div class="form-actions">
                    <button
                        type="submit"
                        class="button"
                        prop:disabled=move || saving.get()
                        class:saving=move || saving.get()
                    >"Save"</button>
                </div>
            </form>
        </div>
    }
}