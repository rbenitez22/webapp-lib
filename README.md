# webapp-lib

A reusable Leptos 0.8 CSR/WASM component library for Rust web applications. Provides HTTP utilities, browser storage helpers, reactive list/resource patterns, CSS theming, authentication primitives, and a set of generic UI components.

## Documentation

- [Best Practices](docs/best-practices.md) — rules learned from real projects:
  when to use library helpers, how to structure components, signal hygiene, and
  Leptos-specific pitfalls.

---

## Feature Flags

Features are additive. Each feature enables the ones it depends on.

| Feature      | Enables                  | Description                                                      |
|--------------|--------------------------|------------------------------------------------------------------|
| `http`       | —                        | HTTP client, `ResourcePath`, `ApiError`                          |
| `storage`    | —                        | `localStorage` / `sessionStorage` helpers                        |
| `reactive`   | `http`, `storage`        | `HasId`, `HasName`, `ListComponentModel`, data-loading functions |
| `theme`      | —                        | CSS injection at runtime                                         |
| `components` | `reactive`, `theme`      | All generic Leptos UI components                                 |
| `auth`       | `components`, `leptos-use` | Authentication models, JWT helpers, auth UI components         |

```toml
# Cargo.toml

# From GitHub — always pin to a tag (highly encouraged).
# Without a tag, Cargo resolves to the latest commit on the default branch,
# which can silently break builds after upstream changes.
webapp-lib = { git = "ssh://git@github.com/rbenitez22/webapp-lib.git", tag = "v0.3.1", features = ["components"] }

# With authentication support:
webapp-lib = { git = "ssh://git@github.com/rbenitez22/webapp-lib.git", tag = "v0.3.1", features = ["auth"] }

# Local path (for active co-development on the library itself):
# webapp-lib = { path = "…/webapp-lib", features = ["components"] }
```

> **Version alignment** — if your project depends on another crate that also
> depends on `webapp-lib` (e.g. a shared domain crate), **all** crates in the
> workspace must reference the **same** `tag` (or `rev`). Cargo treats each
> unique git URL + tag/rev combination as a distinct package; mismatched
> versions will cause duplicate-type errors at compile time that are hard to
> diagnose. The simplest solution is to define the dependency once in the
> workspace root `Cargo.toml` and inherit it everywhere:
>
> ```toml
> # Workspace Cargo.toml
> [workspace.dependencies]
> webapp-lib = { git = "ssh://git@github.com/rbenitez22/webapp-lib.git", tag = "v0.3.1", features = ["auth"] }
>
> # Per-crate Cargo.toml
> [dependencies]
> webapp-lib = { workspace = true }
> # Add extra features only needed by this crate:
> # webapp-lib = { workspace = true, features = ["components"] }
> ```

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

With the `auth` feature, also optionally configure auth paths and start the token refresh timer:

```rust
fn main() {
    webapp_lib::http::set_base_url("https://api.example.com");
    webapp_lib::theme::init();
    // Override only what differs from defaults:
    webapp_lib::auth::init(webapp_lib::auth::AuthPaths {
        after_login: "/dashboard",
        ..Default::default()
    });
    webapp_lib::auth::start_token_refresh_timer(); // optional: refreshes JWT every 50 min
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

| Method         | Description                      |
|----------------|----------------------------------|
| `new(base)`    | Start a path from a base segment |
| `.id(id)`      | Append an id segment             |
| `.child(name)` | Append a sub-resource segment    |
| `.build()`     | Return `String`                  |
| `Display`      | Same as `.build()`               |

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

### `HasId` / `HasName` traits

These replace the former `DataRow` trait. Functions only require the bound they
actually use — `HasId` for operations that look up or route by id, `HasName` for
display/sorting.

Implement manually or derive using
[ferrox-webapp-macros](https://github.com/rbenitez22/ferrox-webapp-macros):

```toml
# Cargo.toml — add alongside webapp-lib
ferrox-webapp-macros = { git = "https://github.com/rbenitez22/ferrox-webapp-macros" }
```

```rust
use webapp_lib::reactive::{HasId, HasName};
use ferrox_webapp_macros::{HasId, HasName};

// Fields named `id` and `name` — defaults, no attribute needed
#[derive(Clone, HasId, HasName)]
pub struct ShoppingList { pub id: String, pub name: String }

// Non-default field names — override with helper attributes
#[derive(Clone, HasId, HasName)]
#[has_name(field = "display_name")]
pub struct UserAccount { pub id: String, pub display_name: String, pub email: String }

#[derive(Clone, HasId, HasName)]
#[has_id(field = "email")]
#[has_name(field = "display_name")]
pub struct UserAccountRequest { pub email: String, pub display_name: String }
```

Manual implementation (no macro dependency):

```rust
impl HasId for ShoppingList {
    fn get_id(&self) -> String { self.id.clone() }
}
impl HasName for ShoppingList {
    fn get_name(&self) -> String { self.name.clone() }
}
```

| Bound required by                  | Traits needed     |
|------------------------------------|-------------------|
| `load_list_component_model`        | `HasName`         |
| `create_persist_event` form type T | `HasId`           |
| `create_persist_event` list type D | `HasId + HasName` |
| `create_delete_event`              | `HasId`           |
| `update_record`                    | `HasId`           |
| `render_options`                   | `HasName`         |

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

Fetches the list on mount, redirects to the login page on 401:

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

let data:    RwSignal<Vec<Permission>>  = RwSignal::new(vec![]);
let error:   RwSignal<Option<String>>  = RwSignal::new(None);
let loading: RwSignal<bool>            = RwSignal::new(false);

// Pass Some(loading) to have the signal managed automatically,
// or None if you manage loading state yourself.
load_resource_list(
    ResourcePath::new("lists").id(list_id).child("permissions"),
    auth_token,
    data,
    error,
    Some(loading),
);
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
    form_model_value,         // D: HasId + Serialize + DeserializeOwned
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

| Variable          | Role                       |
|-------------------|----------------------------|
| `--text`          | Body text colour           |
| `--background`    | Page background            |
| `--primary`       | Primary action colour      |
| `--secondary`     | Secondary / muted colour   |
| `--accent`        | Highlight / focus colour   |
| `--white`         | White (toolbar text, etc.) |
| `--error-color`   | Error messages             |
| `--warning-color` | Warning messages           |

---

## `auth` — Authentication (feature: `auth`)

Requires the `auth` feature. Depends on `components` and `leptos-use`.

### `AuthPaths`

All API paths and redirect targets are configurable via `AuthPaths`. Call
`auth::init` once before mounting; if omitted, defaults below are used.

```rust
pub struct AuthPaths {
    pub login:                &'static str,  // default: "login"
    pub refresh:              &'static str,  // default: "refresh"
    pub accounts:             &'static str,  // default: "accounts"
    pub update_name:          &'static str,  // default: "accounts/update_name"
    pub change_password:      &'static str,  // default: "accounts/change_passwd"
    pub invitations:          &'static str,  // default: "accounts/invitations"
    pub login_page:           &'static str,  // default: "/login"
    pub after_login:          &'static str,  // default: "/lists"
    pub after_register:       &'static str,  // default: "/"
    pub min_password_entropy: f64,           // default: 80.0
}
```

```rust
webapp_lib::auth::init(webapp_lib::auth::AuthPaths {
    after_login: "/dashboard",
    min_password_entropy: 60.0,
    ..Default::default()
});
```

### Storage constants

```rust
pub const STORAGE_AUTH_KEY:  &str = "auth";         // key for Auth in localStorage
pub const STORAGE_USER_KEY:  &str = "user_account"; // key for UserAccount in localStorage
```

### Auth models

```rust
// Stored in localStorage under STORAGE_AUTH_KEY
pub struct Auth {
    pub token: Option<String>,
}
impl Auth {
    pub fn from(token: String) -> Self { … }
    pub fn is_authenticated(&self) -> bool { … }
}

// Stored in localStorage under STORAGE_USER_KEY
pub struct UserAccount {
    pub id:           String,
    pub display_name: String,
    pub email:        String,
    pub auth_type:    Option<String>,
    pub admin:        bool,
}

// Form model for account creation; id field is `email`
pub struct UserAccountRequest {
    pub display_name: String,
    pub email:        String,
    pub password:     String,
}
impl UserAccountRequest {
    pub fn from_account(account: &UserAccount) -> Self { … }
}

pub struct LoginRequest  { pub email: String, pub password: String }
pub struct LoginResponse { pub token: String, pub user_account: UserAccount }
pub struct RefreshResponse { pub token: String }
pub struct UpdateNameRequest     { pub display_name: String }
pub struct ChangePasswordRequest { pub current_password: String, pub new_password: String }

pub struct InvitationRequest {
    pub id:           String,
    pub email:        String,
    pub display_name: String,
    pub is_admin:     bool,
}
impl InvitationRequest {
    pub fn new() -> Self { … }
}
```

### Reactive helpers

```rust
use webapp_lib::auth::{use_auth_token, use_user_account, use_auth_signal};

// Must be called inside a reactive context (component or effect)
let token:   Signal<Option<String>> = use_auth_token();    // None = logged out
let account: Signal<UserAccount>    = use_user_account();
let is_auth: Signal<bool>           = use_auth_signal();

// Non-reactive direct read from localStorage
let token: Option<String> = webapp_lib::auth::read_auth_token_from_local_storage();
```

### Token refresh timer

Starts a background `setInterval` that silently refreshes the JWT every 50 minutes.
The new token is written to localStorage without dispatching a storage event so no
re-renders are triggered.

```rust
// Call once in main() after set_base_url
webapp_lib::auth::start_token_refresh_timer();

// Or call the async function directly if you need the token:
let new_token: Result<String, ApiError> = refresh_auth_token(&current_token).await;
```

### Login helpers

```rust
// Low-level async login — returns the full response or an ApiError
let resp: Result<LoginResponse, ApiError> = submit_login(&email, &password).await;

// Higher-level — returns a form submit handler wired to navigate + storage
let on_submit = submit_login_request(
    email_signal,
    password_signal,
    use_navigate(),
    set_auth,        // WriteSignal<Auth>
    set_login_msg,   // WriteSignal<String>
    set_loading,     // WriteSignal<bool>
);
```

### Auth components (feature: `auth`)

#### `Login`

A self-contained login page. Navigates to `AuthPaths::after_login` on success.

```rust
view! { <Login /> }
```

#### `Logout`

Clears auth/user from localStorage and navigates to `AuthPaths::login_page`.

```rust
view! { <Logout /> }
// Optional callback before clearing:
view! { <Logout on_logout=Callback::new(|_| do_cleanup()) /> }
```

Props: `on_logout: Option<Callback<()>>`.

#### `NewAccount`

Registration page. Navigates to `AuthPaths::after_register` on success.

```rust
view! { <NewAccount /> }
```

#### `AccountEditor`

Reusable account-creation form (used inside `NewAccount`, but embeddable elsewhere).

```rust
view! {
    <AccountEditor
        form_model=form_model   // RwSignal<UserAccountRequest>
        on_saved=move || { /* called after successful POST */ }
    />
}
```

#### `Account`

Account management page. Displays display name (editable), email, admin flag, and a
Change Password dialog. Wraps itself in `<BaseComponent>` — redirects to login if
unauthenticated.

```rust
view! { <Account /> }
```

#### `Invitations`

Admin invitation list page. The `+` add button is hidden for non-admin users.
Wraps itself in `<BaseComponent>`.

```rust
view! { <Invitations /> }
```

#### `InvitationView`

A single invitation row with an optional delete button.

```rust
view! {
    <InvitationView
        model=invitation.clone()
        list_model=model.clone()
        is_admin=is_admin
        on_click=move || { form.set(invitation.clone()); open_dialog(dialog_ref); }
    />
}
```

#### `InvitationEditForm`

Create/edit form for an `InvitationRequest`. The email field is disabled when
`is_new` is `false`.

```rust
view! {
    <InvitationEditForm
        is_new=is_new              // RwSignal<bool>
        form_model=edit_form_model // RwSignal<InvitationRequest>
        saving=model.saving
        on_submit=on_submit
    />
}
```

---

## `components` — UI Components

### `MessageModel` / `MessageType`

```rust
use webapp_lib::components::{MessageModel, MessageType};

// Constructors:
MessageModel::empty()                          // blank (no message rendered)
MessageModel::info("Saved successfully".to_string())
MessageModel::warn("Check your input".to_string())
MessageModel::error("Something went wrong".to_string())

// MessageType variants: Info, Warning, Error
// MessageType::get_style_class() → "info" | "warning" | "error"
// MessageType::get_icon_class()  → icon CSS class string
```

### Dialog helpers

```rust
open_dialog(dialog_ref);   // calls showModal()
close_dialog(dialog_ref);  // calls close()
submit_form(mouse_event);  // finds nearest <form> and calls requestSubmit()
```

### `render_options`

Creates a `<datalist>`-style `<option>` list from a signal. Shows "Loading…" while
`loading` is `true`.

```rust
use webapp_lib::components::render_options;

// D must implement HasName
let opts = render_options(options_signal, loading_signal);
view! { <datalist id="my-list">{opts}</datalist> }
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

### `Spinner`

Full-overlay loading spinner.

```rust
view! { <Spinner /> }
// Custom message:
view! { <Spinner message="Saving…".to_string() /> }
```

Props: `message: String` (default `"Loading…"`).

### `SideMenu` + `MenuLink`

```rust
view! {
    <SideMenu
        title="My App".to_string()
        user_name=user_name_signal
        is_authenticated=use_auth_signal()   // optional: hides badge when logged out
    >
        <MenuLink href="/home">"Home"</MenuLink>
        <MenuLink href="/settings">"Settings"</MenuLink>
        <MenuLink href="/logout">"Logout"</MenuLink>
    </SideMenu>
}
```

| Prop               | Type                     | Required | Description                                              |
|--------------------|--------------------------|----------|----------------------------------------------------------|
| `title`            | `MaybeSignal<String>`    | No       | App title shown in the toolbar                           |
| `user_name`        | `Option<Signal<String>>` | No       | Display name badge in toolbar and drawer footer          |
| `is_authenticated` | `Option<Signal<bool>>`   | No       | When provided, hides the user badge while logged out     |
| `children`         | `Children`               | Yes      | `<MenuLink>` elements                                    |

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
        // show_add=false  (hide the + button; accepts Signal<bool>)
    >
        // optional children rendered between the + button and the title
    </ComponentTitleBar>
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

`title` accepts anything `Into<Signal<String>>`, so both `String` and `RwSignal<String>` work.

### `DataGrid`

```rust
view! {
    <DataGrid
        data=move || items.get()
        cell=move |item: &MyType| view! { <div>{item.name.clone()}</div> }
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
        // style="display: inline;"  (optional)
        // class="my-class"          (optional)
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

Renders a `✖` button. Shows an inline spinner and becomes non-interactive while
the delete request is in flight.

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

### `ConfirmDialog`

Generic two-button confirm dialog. Unlike `DeleteConfirmDialog`, it does not
render its own `<dialog>` — pass a `NodeRef` you control.

```rust
let dialog_ref: NodeRef<Dialog> = NodeRef::new();
let visible = RwSignal::new(false);

view! {
    <ConfirmDialog
        dialog_ref=dialog_ref
        is_visible=visible
        on_confirm=move || { /* confirmed */ }
        on_cancel=move || {}
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
        // required=true        (default true — adds a disabled placeholder option)
        // force_refresh=true   — bypass localStorage cache
        // style="width: 200px" — optional inline style for the <select>
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

Standard CRUD row plus form pair for reference tables:

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

#### `VerifiedPassword`

Renders password + confirm-password fields with a show/hide toggle, an entropy
strength indicator, and a match status display. Sets `is_valid` to `true` only
when entropy ≥ `min_entropy` **and** both fields match.

```rust
let password = RwSignal::new(String::new());
let pw_valid = RwSignal::new(false);

view! {
    <VerifiedPassword password=password is_valid=pw_valid min_entropy=50.0 />
}
```

| Prop          | Type             | Description                                         |
|---------------|------------------|-----------------------------------------------------|
| `password`    | `RwSignal<String>` | Controlled signal for the password value          |
| `is_valid`    | `RwSignal<bool>` | Set to `true` when entropy + match both pass        |
| `min_entropy` | `f64`            | Minimum entropy in bits required for "Strong"       |

#### `EntropyIndicator`

Standalone entropy strength display. Useful when you only need the indicator
without the matched-confirm field.

```rust
let is_strong = RwSignal::new(false);
view! {
    <EntropyIndicator
        password=move || password_signal.get()
        min_entropy=60.0
        is_valid=is_strong
    />
}
```

Strength labels and bit thresholds:

| Label       | Entropy (bits) |
|-------------|----------------|
| Very Weak   | < 25           |
| Weak        | 25 – 39        |
| Fair        | 40 – (min - 1) |
| Strong      | ≥ min_entropy  |

#### `calc_password_entropy`

Low-level helper used internally by `EntropyIndicator` and `VerifiedPassword`.
Returns entropy in bits based on password length × log₂(charset size).

```rust
use webapp_lib::components::calc_password_entropy;

let bits: f64 = calc_password_entropy("MyP@ssw0rd!");
```

### `AutocompleteInput` / `AutocompleteInputModel`

A text input with a live-filter dropdown backed by a remote API list. Supports
keyboard navigation (↑ ↓ Enter Escape) and a manual refresh button.

#### `AutocompleteInputModel<T>`

Owns the reactive loading state and fetches options from the API. Must be
created in a reactive context (e.g., inside a component).

```rust
use webapp_lib::components::AutocompleteInputModel;
use webapp_lib::http::ResourcePath;

// T must be: DeserializeOwned + Clone + Send + Sync + 'static
let model = AutocompleteInputModel::<MyItem>::new(
    ResourcePath::new("items"),
    auth_token_signal, // Signal<Option<String>>
);

// Signals exposed:
model.options  // RwSignal<Vec<T>>
model.error    // RwSignal<Option<String>>
model.loading  // RwSignal<bool>

// Force a re-fetch:
model.refresh();
```

#### `AutocompleteInput<T>`

```rust
use webapp_lib::components::AutocompleteInput;

let input_text = RwSignal::new(String::new());
let selected   = RwSignal::new(Option::<MyItem>::None);

view! {
    <AutocompleteInput
        model=model
        item_key=|item: &MyItem| item.id.clone()   // unique key per item
        label=|item: &MyItem| item.name.clone()    // display text + filter text
        input_value=input_text.into()
        on_input=move |text| input_text.set(text)
        on_select=move |opt| selected.set(opt)     // None on blur with no match
    />
}
```

| Prop          | Type                               | Description                                                   |
|---------------|------------------------------------|---------------------------------------------------------------|
| `model`       | `AutocompleteInputModel<T>`        | Owns fetched options, loading/error state                     |
| `item_key`    | `Fn(&T) -> String`                 | Unique key per item (used for keyboard highlighting)          |
| `label`       | `Fn(&T) -> String`                 | Display text and substring filter text                        |
| `input_value` | `Signal<String>`                   | Controlled text-input value                                   |
| `on_input`    | `Fn(String)`                       | Called on every keystroke with the new text                   |
| `on_select`   | `Fn(Option<T>)`                    | `Some(item)` on selection; `None` on blur with no exact match |

