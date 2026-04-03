//! webapp-lib — reusable building blocks for Leptos CSR/WASM web apps.
//!
//! # Features
//!
//! | Feature      | Contents |
//! |---|---|
//! | `http`       | `HttpMethod`, `ApiError`, `ResourcePath`, `ApiEndpoint`, `send_get/request/delete` |
//! | `storage`    | `read/write_to_local/session_storage`, `dispatch_storage_event` |
//! | `reactive`   | `HasId`, `HasName`, `ListComponentModel`, `load_list_component_model`, `create_persist_event`, `create_delete_event`, `update_record`, navigate helpers |
//! | `theme`      | `ThemeVars`, `init()`, `init_with()` — CSS injection into `<head>` |
//! | `components` | `DeleteRowButton`, `submit_form`, all generic UI components |
//!
//! Features are additive: `components` enables `reactive`, which enables `http` + `storage`.
//!
//! # Quickstart
//!
//! ```toml
//! # Cargo.toml
//! webapp-lib = { path = "...", features = ["components"] }
//! ```
//!
//! ```rust,ignore
//! // main.rs — call once before mounting
//! webapp_lib::http::set_base_url("http://localhost:3000");
//! ```

#[cfg(feature = "http")]
pub mod http;

#[cfg(feature = "storage")]
pub mod storage;

#[cfg(feature = "reactive")]
pub mod reactive;

#[cfg(feature = "theme")]
pub mod theme;

#[cfg(feature = "components")]
pub mod components;

#[cfg(feature = "auth")]
pub mod auth;