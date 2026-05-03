//! Authentication primitives for Leptos CSR/WASM web apps.
//!
//! Call [`init`] once in `main()` to configure API paths and options.
//!
//! ```rust,ignore
//! fn main() {
//!     // Override only what differs from the defaults:
//!     webapp_lib::auth::init(webapp_lib::auth::AuthPaths {
//!         after_login: "/dashboard",
//!         ..Default::default()
//!     });
//! }
//! ```

use leptos::prelude::codee::string::JsonSerdeCodec;
use leptos::prelude::*;
use leptos_use::storage::use_local_storage;
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::JsCast;

use crate::reactive::{HasId, HasName};
use ferrox_webapp_macros::{HasId, HasName};

// ---------------------------------------------------------------------------
// Storage keys
// ---------------------------------------------------------------------------

pub const STORAGE_AUTH_KEY: &str = "auth";
pub const STORAGE_USER_KEY: &str = "user_account";

// ---------------------------------------------------------------------------
// AuthPaths — configurable API endpoints and redirect destinations
// ---------------------------------------------------------------------------

pub struct AuthPaths {
    pub login: &'static str,
    pub refresh: &'static str,
    pub accounts: &'static str,
    pub update_name: &'static str,
    pub change_password: &'static str,
    pub invitations: &'static str,
    pub login_page: &'static str,
    pub after_login: &'static str,
    pub after_register: &'static str,
    pub min_password_entropy: f64,
}

impl Default for AuthPaths {
    fn default() -> Self {
        Self {
            login:                "login",
            refresh:              "refresh",
            accounts:             "accounts",
            update_name:          "accounts/update_name",
            change_password:      "accounts/change_passwd",
            invitations:          "accounts/invitations",
            login_page:           "/login",
            after_login:          "/lists",
            after_register:       "/",
            min_password_entropy: 80.0,
        }
    }
}

static AUTH_PATHS: OnceLock<AuthPaths> = OnceLock::new();

/// Configure authentication paths once at app startup (before mounting).
/// If not called, [`AuthPaths::default`] is used.
pub fn init(paths: AuthPaths) {
    let _ = AUTH_PATHS.set(paths);
}

pub(crate) fn paths() -> &'static AuthPaths {
    AUTH_PATHS.get_or_init(AuthPaths::default)
}

// ---------------------------------------------------------------------------
// Models
// ---------------------------------------------------------------------------

#[derive(Debug, Default, Serialize, Deserialize, Clone, PartialEq)]
pub struct Auth {
    pub token: Option<String>,
}

impl Auth {
    pub fn from(token: String) -> Self {
        Auth { token: Some(token) }
    }

    pub fn is_authenticated(&self) -> bool {
        self.token.is_some()
    }
}

#[derive(Serialize, Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Serialize, Deserialize)]
pub struct LoginResponse {
    pub token: String,
    pub user_account: UserAccount,
}

#[derive(Deserialize)]
pub struct RefreshResponse {
    pub token: String,
}

#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize, HasId, HasName)]
#[has_name(field = "display_name")]
pub struct UserAccount {
    pub id: String,
    pub display_name: String,
    pub email: String,
    pub auth_type: Option<String>,
    pub admin: bool,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize, HasId, HasName)]
#[has_id(field = "email")]
#[has_name(field = "display_name")]
pub struct UserAccountRequest {
    pub display_name: String,
    pub email: String,
    pub password: String,
}

impl UserAccountRequest {
    pub fn from_account(account: &UserAccount) -> Self {
        Self {
            display_name: account.display_name.clone(),
            email: account.email.clone(),
            password: String::new(),
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct UpdateNameRequest {
    pub display_name: String,
}

#[derive(Serialize, Deserialize)]
pub struct ChangePasswordRequest {
    pub current_password: String,
    pub new_password: String,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize, HasId, HasName)]
#[has_name(field = "display_name")]
pub struct InvitationRequest {
    pub id: String,
    pub email: String,
    pub display_name: String,
    pub is_admin: bool,
}

impl InvitationRequest {
    pub fn new() -> Self {
        Self::default()
    }
}

// ---------------------------------------------------------------------------
// Reactive helpers
// ---------------------------------------------------------------------------

/// Returns a reactive signal containing the current JWT token (`None` = logged out).
/// Must be called inside a reactive context (component or effect).
pub fn use_auth_token() -> Signal<Option<String>> {
    let (auth, _, _) = use_local_storage::<Auth, JsonSerdeCodec>(STORAGE_AUTH_KEY);
    Signal::derive(move || auth.get().token)
}

/// Returns a reactive signal containing the current user account (default when logged out).
/// Must be called inside a reactive context (component or effect).
pub fn use_user_account() -> Signal<UserAccount> {
    let (account, _, _) = use_local_storage::<UserAccount, JsonSerdeCodec>(STORAGE_USER_KEY);
    Signal::derive(move || account.get())
}

/// Returns a reactive signal that is `true` when the user is authenticated.
/// Must be called inside a reactive context (component or effect).
pub fn use_auth_signal() -> Signal<bool> {
    let (auth, _, _) = use_local_storage::<Auth, JsonSerdeCodec>(STORAGE_AUTH_KEY);
    Signal::derive(move || auth.get().is_authenticated())
}

/// Reads the current JWT token directly from localStorage (non-reactive).
pub fn read_auth_token_from_local_storage() -> Option<String> {
    crate::storage::read_from_local_storage::<Auth>(STORAGE_AUTH_KEY)
        .and_then(|a| a.token)
}

// ---------------------------------------------------------------------------
// Token refresh timer
// ---------------------------------------------------------------------------

/// Starts a background timer that silently refreshes the JWT token.
/// Call once in `main()` after `set_base_url`.
pub fn start_token_refresh_timer() {
    const REFRESH_INTERVAL_MS: i32 = 3_000_000; // 50 minutes

    let callback = Closure::wrap(Box::new(move || {
        leptos::reactive::spawn_local(async move {
            if let Some(token) = read_auth_token_from_local_storage() {
                match refresh_auth_token(&token).await {
                    Ok(new_token) => {
                        // Silently write — no dispatch_storage_event, avoids re-renders
                        crate::storage::write_to_local_storage(
                            STORAGE_AUTH_KEY,
                            &Auth::from(new_token),
                        );
                        leptos::logging::log!("Auth token refreshed successfully");
                    }
                    Err(e) => leptos::logging::error!("Token refresh failed: {}", e.message),
                }
            }
        });
    }) as Box<dyn Fn()>);

    if let Some(window) = web_sys::window() {
        let _ = window.set_interval_with_callback_and_timeout_and_arguments_0(
            callback.as_ref().unchecked_ref(),
            REFRESH_INTERVAL_MS,
        );
    }
    // Leak so the closure remains valid for the app lifetime
    callback.forget();
}

/// Sends a POST to the refresh endpoint with only the Authorization header.
pub async fn refresh_auth_token(token: &str) -> Result<String, crate::http::ApiError> {
    let req = crate::http::ApiRequest::new(
        &crate::http::HttpMethod::POST,
        Some(token),
        paths().refresh,
        &(),
    );
    let response: RefreshResponse = crate::http::send_request(req).await?;
    Ok(response.token)
}

// ---------------------------------------------------------------------------
// Login helpers
// ---------------------------------------------------------------------------

pub async fn submit_login(
    email: &str,
    password: &str,
) -> Result<LoginResponse, crate::http::ApiError> {
    let payload = LoginRequest {
        email: email.to_string(),
        password: password.to_string(),
    };
    let req = crate::http::ApiRequest::new(
        &crate::http::HttpMethod::POST,
        None,
        paths().login,
        &payload,
    );
    crate::http::send_request(req).await
}

pub fn submit_login_request(
    email: ReadSignal<String>,
    password: ReadSignal<String>,
    nav: impl Fn(&str, leptos_router::NavigateOptions) + Clone + Send + Sync + 'static,
    set_auth: WriteSignal<Auth>,
    set_login_msg: WriteSignal<String>,
    set_loading: WriteSignal<bool>,
) -> impl Fn(leptos::ev::SubmitEvent) {
    move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        let email_val = email.get();
        let password_val = password.get();
        let set_auth = set_auth;
        let nav = nav.clone();
        set_loading.set(true);
        leptos::reactive::spawn_local(async move {
            match submit_login(&email_val, &password_val).await {
                Ok(response) => {
                    set_auth.set(Auth::from(response.token));
                    crate::storage::write_to_local_storage(
                        STORAGE_USER_KEY,
                        &response.user_account,
                    );
                    crate::storage::dispatch_storage_event(STORAGE_USER_KEY);
                    nav(
                        paths().after_login,
                        leptos_router::NavigateOptions {
                            replace: true,
                            scroll: true,
                            ..Default::default()
                        },
                    );
                }
                Err(e) => {
                    leptos::logging::error!("Login failed: {:?}", e);
                    set_login_msg.set("Login failed. Please try again.".to_string());
                    set_loading.set(false);
                }
            }
        });
    }
}