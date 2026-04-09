use std::cmp::Ordering;

use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_router::NavigateOptions;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::http::{ApiError, ApiRequest, HttpMethod, send_delete, send_get, send_request};

// Re-export the traits so webapp-lib consumers only need one import.
pub use ferrox_traits::{HasId, HasName, HasParentId};
// Derive macros in the macro namespace — needed for #[derive(HasId, HasName)] below.
use ferrox_webapp_macros::{HasId, HasName};

// ---------------------------------------------------------------------------
// ListComponentModel — standard model for list pages
//
// auth: Signal<Option<String>> — the current JWT token (None = not logged in).
// The app is responsible for deriving this signal from its own auth storage
// and passing it in. This keeps the model free of any app-specific auth type.
//
// Example in consuming app:
//   let (auth, _, _) = use_local_storage::<Auth, JsonSerdeCodec>(STORAGE_AUTH_KEY);
//   let token = Signal::derive(move || auth.get().token);
//   let model = ListComponentModel::new("users", token);
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct ListComponentModel<D> {
    pub auth: Signal<Option<String>>,
    pub loading: RwSignal<bool>,
    pub saving: RwSignal<bool>,
    pub error: RwSignal<Option<String>>,
    pub data: RwSignal<Vec<D>>,
    pub form_action: String,
}

impl<D: Send + Sync + 'static> ListComponentModel<D> {
    pub fn new(form_action: impl Into<String>, auth: Signal<Option<String>>) -> Self {
        Self {
            auth,
            loading: RwSignal::new(true),
            saving: RwSignal::new(false),
            error: RwSignal::new(None),
            data: RwSignal::new(Vec::new()),
            form_action: form_action.into(),
        }
    }

    pub fn set_error(&self, err: String) {
        self.error.set(Some(err));
    }

    pub fn clear_error(&self) {
        self.error.set(None);
    }
}

// ---------------------------------------------------------------------------
// Navigation helpers
// ---------------------------------------------------------------------------

pub fn navigate(nav: impl Fn(&str, NavigateOptions) + Clone, uri: &str) {
    nav(
        uri,
        NavigateOptions { replace: true, scroll: true, ..Default::default() },
    );
}

pub fn navigate_push(nav: impl Fn(&str, NavigateOptions) + Clone, uri: &str) {
    nav(
        uri,
        NavigateOptions { replace: false, scroll: true, ..Default::default() },
    );
}

pub fn navigate_back() {
    if let Some(win) = web_sys::window() {
        if let Ok(history) = win.history() {
            let _ = history.back();
        }
    }
}

// ---------------------------------------------------------------------------
// load_list_component_model
//
// login_path: where to redirect on 401 (e.g. "/login")
// ---------------------------------------------------------------------------

pub fn load_list_component_model<D>(
    model: ListComponentModel<D>,
    nav: impl Fn(&str, NavigateOptions) + Clone + Send + Sync + 'static,
    sorter: Option<fn(&D, &D) -> Ordering>,
    login_path: &'static str,
) where
    D: HasName + DeserializeOwned + Clone + Send + Sync + 'static,
{
    let sorter = sorter.unwrap_or(|a: &D, b: &D| a.get_name().cmp(&b.get_name()));

    Effect::new(move || {
        let model_clone = model.clone();
        let nav_clone = nav.clone();
        if let Some(token) = model.auth.get() {
            model.loading.set(true);
            let path = model.form_action.clone();
            spawn_local(async move {
                match send_get::<Vec<D>>(&token, &path).await {
                    Ok(mut records) => {
                        records.sort_by(sorter);
                        model_clone.data.set(records);
                        model_clone.loading.set(false);
                    }
                    Err(e) => {
                        if e.is_unauthorized() {
                            navigate(nav_clone, login_path);
                        } else {
                            model_clone.set_error(e.to_string());
                        }
                        model_clone.loading.set(false);
                    }
                }
            });
        } else {
            model_clone.set_error("Please log in first".to_string());
            navigate(nav_clone, login_path);
        }
    });
}

// ---------------------------------------------------------------------------
// create_persist_event
//
// Handles both create (POST) and update (PUT) based on whether get_id()
// returns an empty string.
//
// T — form model (what gets serialized and sent)
// D — list item type (what the API returns and what lives in model.data)
// ---------------------------------------------------------------------------

pub fn create_persist_event<D, T>(
    model: ListComponentModel<D>,
    form_model: RwSignal<T>,
    on_save_success: impl FnOnce() + Clone + 'static,
) -> impl Fn(leptos::ev::SubmitEvent)
where
    T: HasId + Send + Sync + Serialize + Clone + 'static,
    D: HasId + HasName + Send + Sync + Clone + DeserializeOwned + 'static,
{
    move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();

        let form_data = form_model.get().clone();
        let is_new = form_data.get_id().is_empty();
        let method = if is_new { HttpMethod::POST } else { HttpMethod::PUT };
        let path = if is_new {
            model.form_action.clone()
        } else {
            format!("{}/{}", model.form_action, form_data.get_id())
        };

        if let Some(token) = model.auth.get() {
            let model_clone = model.clone();
            let on_success = on_save_success.clone();
            let form_id = form_data.get_id();

            model.saving.set(true);
            spawn_local(async move {
                let req = ApiRequest::new(&method, Some(token.as_str()), &path, &form_data);
                match send_request::<T, D>(req).await {
                    Ok(persisted) => {
                        model_clone.saving.set(false);
                        model_clone.data.update(|rows| {
                            if let Some(idx) = rows.iter().position(|r| r.get_id() == form_id) {
                                rows[idx] = persisted;
                            } else {
                                rows.push(persisted);
                                rows.sort_by(|a, b| a.get_name().cmp(&b.get_name()));
                            }
                        });
                        on_success();
                    }
                    Err(e) => {
                        model_clone.saving.set(false);
                        model_clone.set_error(e.to_string());
                    }
                }
            });
        } else {
            model.set_error("Not authorized".to_string());
        }
    }
}

// ---------------------------------------------------------------------------
// create_delete_event
// ---------------------------------------------------------------------------

pub fn create_delete_event<D>(
    id: String,
    list_model: ListComponentModel<D>,
) -> impl Fn(leptos::ev::SubmitEvent)
where
    D: HasId + Send + Sync + Clone + 'static,
{
    move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();

        let token = list_model.auth.get().unwrap_or_default();
        let path = format!("{}/{}", list_model.form_action, id);
        let id = id.clone();
        let model_clone = list_model.clone();

        spawn_local(async move {
            match send_delete(&token, &path).await {
                Ok(_) => {
                    model_clone.clear_error();
                    model_clone.data.update(|rows| {
                        if let Some(idx) = rows.iter().position(|r| r.get_id() == id) {
                            rows.remove(idx);
                        }
                    });
                }
                Err(e) => {
                    model_clone.set_error(e.to_string());
                }
            }
        });
    }
}

// ---------------------------------------------------------------------------
// update_record — for detail/edit pages that don't use a ListComponentModel
// ---------------------------------------------------------------------------

pub fn update_record<D>(
    form_action: String,
    form_model: D,
    auth: Signal<Option<String>>,
    on_success: impl FnOnce(D) + Clone + 'static,
    on_failure: impl Fn(ApiError) + Clone + 'static,
) where
    D: HasId + Send + Sync + Clone + DeserializeOwned + Serialize + 'static,
{
    if let Some(token) = auth.get_untracked() {
        let path = format!("{}/{}", form_action, form_model.get_id());
        spawn_local(async move {
            let req = ApiRequest::new(&HttpMethod::PUT, Some(token.as_str()), &path, &form_model);
            match send_request::<D, D>(req).await {
                Ok(saved) => on_success(saved),
                Err(e) => on_failure(e),
            }
        });
    } else {
        on_failure(ApiError::new("Not authorized".to_string(), 401));
    }
}

// ---------------------------------------------------------------------------
// update_resource
//
// Sends a PUT request with `body` serialised as JSON to the given ResourcePath
// and invokes one of two callbacks depending on the outcome.
//
// on_success receives the deserialised response body (same type T).
// on_fail receives the error as a String.
// ---------------------------------------------------------------------------

pub fn update_resource<T>(
    path: crate::http::ResourcePath,
    body: T,
    auth: Signal<Option<String>>,
    on_success: impl Fn(T) + 'static,
    on_fail: impl Fn(String) + 'static,
) where
    T: Serialize + DeserializeOwned + Send + Sync + 'static,
{
    update_resource_as(path, body, auth, on_success, on_fail);
}

/// Like [`update_resource`] but allows the response type `R` to differ from
/// the request body type `B`.  Use this when the API accepts one shape and
/// returns a richer one (e.g. sending `UpdatePermissionRequest`, receiving
/// `StorePermission`).
pub fn update_resource_as<B, R>(
    path: crate::http::ResourcePath,
    body: B,
    auth: Signal<Option<String>>,
    on_success: impl Fn(R) + 'static,
    on_fail: impl Fn(String) + 'static,
) where
    B: Serialize + Send + Sync + 'static,
    R: DeserializeOwned + Send + Sync + 'static,
{
    let path_str = path.to_string();
    let Some(token) = auth.get_untracked() else {
        on_fail("Not authenticated".to_string());
        return;
    };
    spawn_local(async move {
        let req = ApiRequest::new(&HttpMethod::PUT, Some(token.as_str()), &path_str, &body);
        match send_request::<B, R>(req).await {
            Ok(saved) => on_success(saved),
            Err(e)    => on_fail(e.to_string()),
        }
    });
}

// ---------------------------------------------------------------------------
// load_reference_data_list
//
// Loads a Vec<LookupData> for a given endpoint key, with local-storage caching.
// The key is used both as the API path and the local-storage cache key.
//
// auth: Signal<Option<String>> — current JWT token.  When None the load is
//       skipped and `loaded` is set to true (no-op / unauthenticated).
// force_refresh: bypass the cache and always hit the network.
// ---------------------------------------------------------------------------

pub fn load_reference_data_list(
    list_name: &'static str,
    auth: Signal<Option<String>>,
    options: RwSignal<Vec<LookupData>>,
    loaded: RwSignal<bool>,
    force_refresh: bool,
) {
    use crate::storage::{read_from_local_storage, write_to_local_storage};

    Effect::new(move |_| {
        let cached = read_from_local_storage::<Vec<LookupData>>(list_name).unwrap_or_default();
        if !cached.is_empty() && !force_refresh {
            options.set(cached);
            loaded.set(true);
            return;
        }

        let Some(token) = auth.get() else {
            loaded.set(true);
            return;
        };

        spawn_local(async move {
            match send_get::<Vec<LookupData>>(&token, list_name).await {
                Ok(rows) => {
                    write_to_local_storage(list_name, &rows);
                    options.set(rows);
                    loaded.set(true);
                }
                Err(e) => {
                    leptos::logging::error!("load_reference_data_list({}): {}", list_name, e);
                    loaded.set(true);
                }
            }
        });
    });
}

// ---------------------------------------------------------------------------
// load_resource_list
//
// Fetches a Vec<T> from an arbitrary ResourcePath without any caching.
// Useful for data that must always be fresh (e.g. permissions, user lists).
//
// auth: Signal<Option<String>> — current JWT token.  When None the load is
//       skipped and `error` is left as None.
// data:  populated on success.
// error: set to Some(message) on failure; None while loading or on success.
// ---------------------------------------------------------------------------

pub fn load_resource_list<T>(
    path: crate::http::ResourcePath,
    auth: Signal<Option<String>>,
    data: RwSignal<Vec<T>>,
    error: RwSignal<Option<String>>,
    loading: Option<RwSignal<bool>>,
) where
    T: DeserializeOwned + Clone + Send + Sync + 'static,
{
    let path_str = path.to_string();
    Effect::new(move |_| {
        let Some(token) = auth.get() else { return; };
        let path = path_str.clone();
        if let Some(l) = loading { l.set(true); }
        spawn_local(async move {
            match send_get::<Vec<T>>(&token, &path).await {
                Ok(rows) => {
                    data.set(rows);
                    if let Some(l) = loading { l.set(false); }
                }
                Err(e) => {
                    error.set(Some(e.to_string()));
                    if let Some(l) = loading { l.set(false); }
                }
            }
        });
    });
}

// ---------------------------------------------------------------------------
// load_resource
//
// Fetches a single T from an arbitrary ResourcePath (e.g. a get-by-id call)
// without any caching.
//
// auth: Signal<Option<String>> — current JWT token.  When None the load is
//       skipped and `error` is left as None.
// data:  set to Some(T) on success.
// error: set to Some(message) on failure; None while loading or on success.
// ---------------------------------------------------------------------------

pub fn load_resource<T>(
    path: crate::http::ResourcePath,
    auth: Signal<Option<String>>,
    data: RwSignal<Option<T>>,
    error: RwSignal<Option<String>>,
) where
    T: DeserializeOwned + Clone + Send + Sync + 'static,
{
    let path_str = path.to_string();
    Effect::new(move |_| {
        let Some(token) = auth.get() else { return; };
        let path = path_str.clone();
        spawn_local(async move {
            match send_get::<T>(&token, &path).await {
                Ok(item) => data.set(Some(item)),
                Err(e)   => error.set(Some(e.to_string())),
            }
        });
    });
}

// ---------------------------------------------------------------------------
// create_nav_up_event
//
// Returns a MouseEvent handler that navigates to the parent resource when one
// exists, or to the base `path` when at the root.
//
// Requires T to implement HasParentId.  The navigate function is typically
// obtained from `use_navigate()`.
//
// Example:
//   let nav_up = create_navigate_up_event(current_store, "/stores", navigate);
// ---------------------------------------------------------------------------

pub fn create_navigate_up_event<T, N>(
    resource: RwSignal<Option<T>>,
    path: &'static str,
    navigate: N,
) -> impl Fn(web_sys::MouseEvent)
where
    T: HasParentId + Clone + Send + Sync + 'static,
    N: Fn(&str, NavigateOptions) + Clone + 'static,
{
    move |_| {
        let parent = resource.get_untracked().and_then(|s| s.get_parent_id());
        match parent {
            Some(pid) => navigate(&format!("{path}/{pid}"), Default::default()),
            None      => navigate(path, Default::default()),
        }
    }
}

// ---------------------------------------------------------------------------
// sync_page_title
//
// Reactively mirrors the name of an Option<T: HasName> resource into the
// page-title signal provided via context (RwSignal<String>).  Resets the
// title to `default` when the component is torn down.
//
// Typical call site:
//   sync_page_title(current_store, "Les Magasins");
// ---------------------------------------------------------------------------

pub fn sync_page_title<T: HasName + Clone + Send + Sync + 'static>(
    resource: RwSignal<Option<T>>,
    default: &'static str,
) {
    if let Some(title) = use_context::<RwSignal<String>>() {
        Effect::new(move |_| {
            if let Some(item) = resource.get() {
                title.set(item.get_name().to_string());
            }
        });
        on_cleanup(move || title.set(default.into()));
    }
}

// ---------------------------------------------------------------------------
// delete_resource
//
// Sends a DELETE request to the given ResourcePath and invokes one of two
// callbacks depending on the outcome.  No caching is touched.
//
// auth: Signal<Option<String>> — current JWT token.  When None, on_fail is
//       called immediately.
// ---------------------------------------------------------------------------

/// Returns a plain `Fn()` closure that calls [`delete_resource`] when invoked.
/// Use this when you need to pass a delete action as a callback prop rather
/// than executing it immediately.
///
/// ```ignore
/// let on_delete = create_delete_callback(
///     Endpoint::Items.path().id(&item_id),
///     auth,
///     move || items.update(|is| is.retain(|i| i.id != item_id)),
///     move |e| error.set(Some(e)),
/// );
/// // on_delete can now be passed as: on_delete=on_delete
/// ```
pub fn create_delete_callback(
    path: crate::http::ResourcePath,
    auth: Signal<Option<String>>,
    on_success: impl Fn() + Clone + 'static,
    on_fail: impl Fn(String) + Clone + 'static,
) -> impl Fn() {
    move || delete_resource(path.clone(), auth, on_success.clone(), on_fail.clone())
}

pub fn delete_resource(
    path: crate::http::ResourcePath,
    auth: Signal<Option<String>>,
    on_success: impl Fn() + 'static,
    on_fail: impl Fn(String) + 'static,
) {
    let path_str = path.to_string();
    let Some(token) = auth.get_untracked() else {
        on_fail("Not authenticated".to_string());
        return;
    };
    spawn_local(async move {
        match send_delete(&token, &path_str).await {
            Ok(_)  => on_success(),
            Err(e) => on_fail(e.to_string()),
        }
    });
}

// ---------------------------------------------------------------------------
// LookupData — the common {id, name, description?} shape for reference/lookup
// tables (categories, units, tags, etc.).  Clients that need a different shape
// should define their own type and derive HasId / HasName as appropriate.
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Clone, PartialEq, Default, Debug, HasId, HasName)]
pub struct LookupData {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
}

/// Request body for creating or updating a `LookupData` record.
/// An empty `id` means "create"; a non-empty `id` means "update".
#[derive(Serialize, Deserialize, Clone, Default, Debug, HasId, HasName)]
pub struct LookupDataRequest {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
}

impl LookupDataRequest {
    pub fn new() -> Self { Self::default() }

    pub fn from_data(data: LookupData) -> Self {
        Self { id: data.id, name: data.name, description: data.description }
    }
}