use serde::de::DeserializeOwned;
use serde::Serialize;
use wasm_bindgen::JsValue;

pub fn write_to_local_storage<T: Serialize>(key: &str, value: &T) {
    if let Ok(serialized) = serde_json::to_string(value) {
        if let Some(window) = web_sys::window() {
            if let Ok(Some(ls)) = window.local_storage() {
                let _ = ls.set_item(key, &serialized);
            }
        }
    }
}

pub fn read_from_local_storage<T: DeserializeOwned>(key: &str) -> Option<T> {
    web_sys::window()
        .and_then(|w| w.local_storage().ok().flatten())
        .and_then(|ls| ls.get_item(key).ok().flatten())
        .and_then(|s| serde_json::from_str::<T>(&s).ok())
}

pub fn write_to_session_storage<T: Serialize>(key: &str, value: &T) {
    if let Ok(serialized) = serde_json::to_string(value) {
        if let Some(window) = web_sys::window() {
            if let Ok(Some(ss)) = window.session_storage() {
                let _ = ss.set_item(key, &serialized);
            }
        }
    }
}

pub fn read_from_session_storage<T: DeserializeOwned>(key: &str) -> Option<T> {
    web_sys::window()
        .and_then(|w| w.session_storage().ok().flatten())
        .and_then(|ss| ss.get_item(key).ok().flatten())
        .and_then(|s| serde_json::from_str::<T>(&s).ok())
}

pub fn read_from_session_storage_or<T>(key: &str, on_not_found: impl Fn() -> T) -> T
where
    T: DeserializeOwned,
{
    read_from_session_storage::<T>(key).unwrap_or_else(on_not_found)
}

/// Dispatches the leptos-use-storage CustomEvent so reactive signals from
/// use_local_storage update when you write directly via write_to_local_storage.
pub fn dispatch_storage_event(key: &str) {
    if let Some(window) = web_sys::window() {
        let init = web_sys::CustomEventInit::new();
        init.set_detail(&JsValue::from_str(key));
        if let Ok(event) =
            web_sys::CustomEvent::new_with_event_init_dict("leptos-use-storage", &init)
        {
            let _ = window.dispatch_event(&event);
        }
    }
}