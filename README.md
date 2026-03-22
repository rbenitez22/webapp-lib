# webapp-lib

A reusable Leptos 0.8 CSR/WASM component library for Rust web applications. Provides HTTP utilities, browser storage helpers, reactive list/resource patterns, CSS theming, and a set of generic UI components.

## Feature Flags

Features are additive. Each feature enables the ones it depends on.

| Feature | Enables | Description |
|---|---|---|
| `http` | — | HTTP client, `ResourcePath`, `ApiError` |
| `storage` | — | `localStorage` / `sessionStorage` helpers |
| `reactive` | `http`, `storage` | `ListComponentModel`, data-loading functions |
| `theme` | — | CSS injection at runtime |
| `components` | `reactive`, `theme` | All Leptos UI components |

```toml
# Cargo.toml
[dependencies]
webapp-lib = { path = "…/webapp-lib", features = ["components"] }
```

---

## Startup

Call these once in `main()` before `mount_to_body`:

```rust
fn main() {
    webapp_lib::http::set_base_url("https://api.example.com");
    webapp_lib::theme::init();          // or init_with(&custom_vars)
    mount_to_body(App);
}
```

If pages use `SelectMenu` or `LookupDataDisplay`, provide the auth token via context in your root component:

```rust
#[component]
pub fn App() -> impl IntoView {
    provide_context(use_auth_token()); // Signal<Option<String>>
    view! { <Router>…</Router> }
}
```

---

## `http` — HTTP Client

### `ResourcePath`

Fluent path builder. Avoids scattering string literals throughout the codebase.

```rust
use webapp_lib::http::ResourcePath;

let path = ResourcePath::new("lists").id(list_id).child("permissions");
// → "lists/42/permissions"
```

| Method | Description |
|---|---|
| `new(base)` | Start a path from a base segment |
| `.id(id)` | Append an id segment |
| `.child(name)` | Append a sub-resource segment |
| `.build()` | Return `String` |
| `Display` | Same as `.build()` |

### `ApiEndpoint` trait

Implement on your endpoint enum so `.path()` returns a `ResourcePath`. Then add
methods for compound sub-resource paths — this keeps all path strings in one
place and eliminates repeated string literals at call sites:

```rust
use webapp_lib::http::{ApiEndpoint, ResourcePath};

pub enum EndPoint { Lists, Products }

impl ApiEndpoint for EndPoint {
    fn base(&self) -> &str {
        match self {
            EndPoint::Lists    => "lists",
            EndPoint::Products => "products",
        }
    }
}

// Add methods for sub-resources so no string literal leaks to call sites.
impl EndPoint {
    pub fn items(&self, id: impl ToString) -> ResourcePath {
        self.path().id(id).child("items")
    }
    pub fn permissions(&self, id: impl ToString) -> ResourcePath {
        self.path().id(id).child("permissions")
    }
}

// Call sites are clean and compiler-navigable:
EndPoint::Lists.path()               // → "lists"
EndPoint::Lists.path().id(id)        // → "lists/42"
EndPoint::Lists.items(id)            // → "lists/42/items"
EndPoint::Lists.permissions(id)      // → "lists/42/permissions"
```

### HTTP functions

```rust
use webapp_lib::http::{send_get, send_request, send_delete, ApiRequest, HttpMethod};

// GET
let result: Result<Vec<MyType>, ApiError> = send_get(&token, "lists").await;

// POST / PUT
let req = ApiRequest::new(&HttpMethod::POST, Some(&token), "lists", &payload);
let result: Result<MyType, ApiError> = send_request(req).await;

// DELETE
send_delete(&token, "lists/42").await?;
```

### `ApiError`

```rust
pub struct ApiError { pub message: String, pub status: u16 }

error.is_unauthorized() // status == 401
```

---

## `storage` — Browser Storage

```rust
use webapp_lib::storage::{
    write_to_local_storage, read_from_local_storage,
    write_to_session_storage, read_from_session_storage,
    read_from_session_storage_or, dispatch_storage_event,
};

// Write
write_to_local_storage("auth", &auth_value);

// Read
let auth: Option<Auth> = read_from_local_storage("auth");

// Read with fallback
let item = read_from_session_storage_or("current-list", || ShoppingList::default());

// Trigger reactive leptos-use signals to update
dispatch_storage_event("auth");
```

> Write + `dispatch_storage_event` updates reactive signals.
> Write alone (omitting dispatch) performs a silent update — useful for token refresh where you don't want a re-render.

---

## `reactive` — Reactive Patterns

### `DataRow` trait

Implement on any type used in a list:

```rust
use webapp_lib::reactive::DataRow;

impl DataRow for ShoppingList {
    fn get_id(&self)   -> String { self.id.clone() }
    fn get_name(&self) -> String { self.name.clone() }
}
```

### `ListComponentModel<D>`

Bundles all signals for a standard list page.

```rust
use webapp_lib::reactive::ListComponentModel;

let model = ListComponentModel::new("lists", auth_token); // auth: Signal<Option<String>>

// Signals:
model.loading  // RwSignal<bool>
model.saving   // RwSignal<bool>
model.error    // RwSignal<Option<String>>
model.data     // RwSignal<Vec<D>>
```

### `load_list_component_model`

Fetches the list on mount, redirects to login on 401:

```rust
load_list_component_model(model.clone(), use_navigate(), Some(sorter_fn), "/login");
```

### `create_persist_event` / `create_delete_event`

Return submit-event handlers for create/update/delete:

```rust
// POST when id is empty, PUT when id is set
let on_submit = create_persist_event(model.clone(), form_model, move || close_dialog(dialog_ref));

// DELETE by id, removes the row from model.data on success
let on_delete = create_delete_event(item.get_id(), model.clone());
```

### Resource loading functions

#### `load_resource_list` — fetch a `Vec<T>` (no cache)

```rust
use webapp_lib::reactive::load_resource_list;
use webapp_lib::http::ResourcePath;

let data:  RwSignal<Vec<Permission>>    = RwSignal::new(vec![]);
let error: RwSignal<Option<String>>     = RwSignal::new(None);

load_resource_list(
    ResourcePath::new("lists").id(list_id).child("permissions"),
    auth_token,
    data,
    error,
);

// In view — loading: data empty + no error; error: error.get().is_some()
```

#### `load_resource` — fetch a single item by id (no cache)

```rust
use webapp_lib::reactive::load_resource;

let data:  RwSignal<Option<MyRecord>> = RwSignal::new(None);
let error: RwSignal<Option<String>>   = RwSignal::new(None);

load_resource(
    ResourcePath::new("lists").id(list_id),
    auth_token,
    data,
    error,
);
```

#### `load_reference_data_list` — fetch `Vec<LookupData>` with localStorage cache

```rust
use webapp_lib::reactive::load_reference_data_list;

let options: RwSignal<Vec<LookupData>> = RwSignal::new(vec![]);
let loaded:  RwSignal<bool>            = RwSignal::new(false);

load_reference_data_list("categories", auth_token, options, loaded, false);
// force_refresh: true bypasses cache
```

### `update_resource` — PUT with callbacks

```rust
use webapp_lib::reactive::update_resource;

update_resource(
    ResourcePath::new("lists").id(list_id),
    my_list_value,
    auth_token,
    move |saved| { data.set(Some(saved)); },
    move |err|   { error.set(Some(err)); },
);
```

### `delete_resource` — DELETE with callbacks

```rust
use webapp_lib::reactive::delete_resource;

delete_resource(
    ResourcePath::new("lists").id(list_id).child("permissions").id(perm_id),
    auth_token,
    move || { /* success — e.g. reload list */ },
    move |err| { error.set(Some(err)); },
);
```

### `update_record`

PUT for detail/edit pages that don't use `ListComponentModel`:

```rust
update_record(
    "lists".to_string(),
    form_model_value,         // D: DataRow + Serialize + DeserializeOwned
    auth_token,
    move |saved| { … },
    move |api_err| { … },
);
```

### Navigation helpers

```rust
navigate(nav, "/lists");       // replace current history entry
navigate_push(nav, "/lists");  // push new history entry
navigate_back();               // history.back()
```

### `LookupData` / `LookupDataRequest`

Standard shape for reference tables (categories, units, tags, etc.):

```rust
pub struct LookupData {
    pub id:          String,
    pub name:        String,
    pub description: Option<String>,
}

LookupDataRequest::new()              // blank for create
LookupDataRequest::from_data(lookup)  // populated for edit
```

---

## `theme` — CSS Theming

Injects vars, base, icons, and loading CSS into `<head>` at runtime.

```rust
// Default palette
webapp_lib::theme::init();

// Custom palette
webapp_lib::theme::init_with(&ThemeVars {
    primary:   "--my-brand-blue",
    secondary: "#334455",
    ..ThemeVars::default()
});
```

Override any variable in your own stylesheet:

```css
:root {
    --primary: #1a73e8;
}
```

### Default CSS variables

| Variable | Role |
|---|---|
| `--text` | Body text colour |
| `--background` | Page background |
| `--primary` | Primary action colour |
| `--secondary` | Secondary / muted colour |
| `--accent` | Highlight / focus colour |
| `--white` | White (toolbar text, etc.) |
| `--error-color` | Error messages |
| `--warning-color` | Warning messages |

---

## `components` — UI Components

### Dialog helpers

```rust
open_dialog(dialog_ref);   // calls showModal()
close_dialog(dialog_ref);  // calls close()
submit_form(mouse_event);  // finds nearest <form> and calls requestSubmit()
```

### `BaseComponent` — auth guard

```rust
view! {
    <BaseComponent is_authenticated=use_auth_signal()>
        // redirects to /login if not authenticated
    </BaseComponent>
}
```

Props: `is_authenticated: Signal<bool>`, `login_path: &'static str` (default `"/login"`).

### `SideMenu` + `MenuLink`

```rust
view! {
    <SideMenu title="My App".to_string() user_name=user_name_signal>
        <MenuLink href="/home">"Home"</MenuLink>
        <MenuLink href="/settings">"Settings"</MenuLink>
        <MenuLink href="/logout">"Logout"</MenuLink>
    </SideMenu>
}
```

`SideMenu` props: `title: Option<String>`, `user_name: Option<Signal<String>>`, `children`.
`MenuLink` auto-closes the drawer on click. Must be a descendant of `SideMenu`.

### `ListComponentView`

Three-state wrapper: spinner → error message → data grid.

```rust
view! {
    <ListComponentView
        loading=model.loading
        error=model.error
        data=model.data
        cell_view=move |item: &MyType| view! { … }
    />
}
```

### `ComponentTitleBar`

```rust
view! {
    <ComponentTitleBar
        title="Shopping Lists".to_string()
        on_add=move |_| open_dialog(dialog_ref)
        // show_add=false  (hide the + button)
    />
}
```

### `DialogTitle`

```rust
view! {
    <dialog node_ref=dialog_ref class="dialog">
        <DialogTitle title="Edit Item".to_string() on_close=move |_| close_dialog(dialog_ref)/>
        // form…
    </dialog>
}
```

### `DataGrid`

```rust
view! {
    <DataGrid
        data=move || items.get()
        cell=move |item: MyType| view! { <div>{item.name}</div> }
    />
}
```

### `CheckBox`

```rust
view! {
    <CheckBox
        id="can_edit"
        value=move || form.get().can_edit
        on_change=move |v| form.update(|m| m.can_edit = v)
    />
}
```

### `Message`

```rust
let msg = RwSignal::new(MessageModel::empty());
// …
msg.set(MessageModel::error("Something went wrong".to_string()));

view! { <Message message=msg /> }
```

### `DeleteRowButton`

```rust
let on_delete = create_delete_event(item.get_id(), model.clone());
view! { <DeleteRowButton on_delete=on_delete /> }
```

### `DeleteConfirmDialog`

```rust
view! {
    <DeleteConfirmDialog
        dialog_ref=dialog_ref
        message="Delete this item?".to_string()
        on_confirm=move || { /* perform delete */ }
    />
}
```

### `SelectMenu`

Loads options from the API (cached in localStorage). Requires auth token in context.

```rust
view! {
    <SelectMenu
        source_name="categories"
        field_name="category_id"
        value=move || form.get().category_id.clone()
        on_change=move |val| form.update(|m| m.category_id = val)
        // force_refresh=true  — bypass cache
    />
}
```

### `LookupDataDisplay`

Resolves a stored id to its display name. Requires auth token in context.

```rust
view! {
    <LookupDataDisplay
        source_name="categories"
        value=move || item.get().category_id.clone()
    />
}
```

### `LookupDataView` / `LookupDataEditForm`

Standard CRUD row + form pair for reference tables:

```rust
// Row
let on_delete = create_delete_event(item.id.clone(), model.clone());
view! {
    <LookupDataView
        model=item.clone()
        on_delete=on_delete
        on_click=move || { request.set(LookupDataRequest::from_data(item.clone())); open_dialog(dialog_ref); }
    />
}

// Form (inside <dialog>)
view! {
    <LookupDataEditForm
        request_model=request
        saving=model.saving
        on_submit=on_submit
    />
}
```

### `VerifiedPassword` + `EntropyIndicator`

```rust
let password   = RwSignal::new(String::new());
let pw_valid   = RwSignal::new(false);

view! {
    <VerifiedPassword password=password is_valid=pw_valid min_entropy=50.0 />
    // Renders password + confirm fields with strength indicator and match status
}
```

---

## Typical Page Pattern

```rust
#[component]
pub fn MyListPage() -> impl IntoView {
    let dialog_ref = NodeRef::new();
    let model = ListComponentModel::new("items", use_auth_token());
    let form  = RwSignal::new(ItemRequest::default());

    load_list_component_model(model.clone(), use_navigate(), None, "/login");

    let on_submit = create_persist_event(model.clone(), form, move || close_dialog(dialog_ref));

    view! {
        <BaseComponent is_authenticated=use_auth_signal()>
            <div class="main">
                <ComponentTitleBar title="Items".to_string()
                    on_add=move |_| { form.set(ItemRequest::default()); open_dialog(dialog_ref); }
                />
                <dialog node_ref=dialog_ref class="dialog">
                    <DialogTitle title="Edit Item".to_string() on_close=move |_| close_dialog(dialog_ref)/>
                    <MyEditForm form=form saving=model.saving on_submit=on_submit/>
                </dialog>
                <ListComponentView
                    loading=model.loading
                    error=model.error
                    data=model.data
                    cell_view=move |item: &Item| {
                        let on_delete = create_delete_event(item.get_id(), model.clone());
                        view! { <DeleteRowButton on_delete=on_delete/> }
                    }
                />
            </div>
        </BaseComponent>
    }
}
```