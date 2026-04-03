# Best Practices — Rust / Leptos / webapp-lib

Distilled from real project corrections. Apply these rules to every new project
that uses `webapp-lib`.

---

## 1. Always audit the library before writing any code

**Mistake:** Wrote manual `spawn_local` / `send_delete` / `send_request` blocks inline.  
**Correction:** `delete_resource`, `update_resource`, `create_persist_event`,
`load_resource_list` already existed in `webapp-lib`.

**Rule:** Before writing any async HTTP logic, grep `webapp-lib/src/reactive/mod.rs`
for an existing helper. Only write manual `spawn_local` if nothing fits.

---

## 2. If the library is missing a variant, add it — don't work around it

**Mistake:** `update_resource` required the same type for request and response (`T`).
Instead of adding an overload, the code worked around it with a manual `spawn_local`.  
**Correction:** Added `update_resource_as<B, R>` to the library, then `update_resource`
was reimplemented in terms of it.

**Rule:** If a library function *almost* fits but has a type or shape constraint that
doesn't match, add the overload to the library. Do not inline the logic at the call site.

---

## 3. Lazy callbacks belong in factory functions, not inline closures

**Mistake:** `on_delete` was written as an inline `move || { spawn_local(...) }` at
the call site.  
**Correction:** Added `create_delete_callback` to the library, returning `impl Fn()`.
The call site becomes `create_delete_callback(path, auth, on_success, on_fail)`.

**Rule:** When a side-effectful operation needs to be passed as a callback prop, look
for (or create) a factory function in `webapp-lib` that returns `impl Fn(...)`. This
mirrors `create_persist_event` and `create_delete_event` which already follow this
pattern.

---

## 4. Extract large closures to named functions before the view!

**Mistake:** `on_submit` was a large inline closure inside the component body.  
**Correction:** Extracted to `create_save_permission_event(...)` returning
`impl Fn(SubmitEvent)`.

**Rule:** Any closure longer than ~5 lines that is passed to a component or used as
an event handler should be extracted to a named function returning `impl Fn(...)`.
Keep `view!` blocks declarative and free of logic.

---

## 5. Further split large functions into focused helpers

**Mistake:** An orchestrating function contained a large `if/else` with two full
`spawn_local` blocks inline.  
**Correction:** Split into two focused helpers (`grant_permission`,
`update_permission`). The orchestrating function became a thin decision layer.

**Rule:** If a function contains a top-level `if/else` where each branch has
significant logic, extract each branch to its own named function. The outer function
should only validate, decide, and delegate.

---

## 6. Define callbacks above view!, not inline in component props

**Mistake:** Complex `on_success` and `on_fail` closures were written inline inside
a library call's argument list.  
**Correction:** Extracted to `let on_saved = ...` and `let on_fail = ...` bindings
immediately before the call.

**Rule:** If a callback passed to a function is more than one line, bind it to a
named `let` before the call. This keeps argument lists readable as a list of *what*
is being passed, not *how* it works.

---

## 7. Use the richest available type as the single form model

**Mistake:** Used a minimal request type (only the fields the API PUT accepts) plus
separate `is_new`, `edit_id`, `add_user_id` signals.  
**Correction:** Switched to the full API response type (`StorePermission`) as the
form model signal. `id.is_empty()` replaces `is_new`. All path segments are read
from the single signal.

**Rule:** Use the richest domain type (typically the API response type) as the form
model signal. Derive secondary state (`is_new`, path segments) from it rather than
maintaining parallel signals. A new record is represented by `Default::default()`
(empty `id`).

---

## 8. Move resource fetching to where it is consumed

**Mistake:** Data was fetched in a parent component and passed down as a raw signal
or `Vec` prop.  
**Correction:** An opaque model struct (`AutocompleteInputModel`) was constructed
where `auth` and the path were available, and passed as a single prop. The child
component owns its own fetch lifecycle.

**Rule:** Fetch data at the lowest component that needs it. Pass opaque models rather
than raw signals or vectors. The parent should not manage data it doesn't display.

---

## 9. Components should not receive signals they can capture or derive

**Mistake:** `auth`, `is_new`, `selected_user_id`, `store_id` were all passed as
props when they were either `Copy` signals already in scope, or derivable from
another prop.  
**Correction:** Removed them one by one. `auth` was encapsulated in a model.
`is_new` was derived from `form.get().id.is_empty()`. `store_id` was read from
a field already present in the form model.

**Rule:** Before adding a prop, ask:
- Is it `Copy` and already in scope of the call site?
- Can it be derived from an existing prop?
- Can it be encapsulated in a model struct?

If any answer is yes, do not add the prop.

---

## 10. `#[component]` cannot capture from a parent component's scope — use callbacks instead

**Mistake:** Tried to implicitly capture a parent signal inside a child `#[component]`
by converting the component to a plain function.  
**Correction:** Kept it as a `#[component]` but reduced it to pure presentation.
All logic (auth, path, signals) moved to the call site as pre-built callbacks.

**Rule:** A `#[component]` is an isolated reactive scope. It cannot implicitly capture
signals from a parent component. The solution is not to convert it to a plain
function — it is to push logic *up* to the parent as callbacks, making the component
purely presentational.

---

## 11. Do not duplicate module declarations

**Mistake:** Added `mod foo; pub use foo::...` to a `mod.rs` when it was already
declared there.  
**Correction:** Removed the duplicate.

**Rule:** Before adding a `mod` or `pub use` declaration, grep the target file first.
Module declarations in Rust are not idempotent — duplicates are a compile error.

---

## 12. Leptos view! macro: no type annotations in closure parameters

**Mistake:** Wrote `on_select=move |x: Option<MyType>| { ... }` inside `view!`.  
**Correction:** Removed the type annotation: `on_select=move |x| { ... }`. Rust
infers the type from the component's prop signature.

**Rule:** Never write explicit type annotations on closure parameters inside `view!`
macros. The macro parses `<` as an HTML tag. Let the compiler infer types from the
prop's `impl Fn(...)` bound.

---

## 13. All builder / path types should derive Clone

**Observation:** `ResourcePath` did not derive `Clone`, which blocked its use inside
closures and factory functions.  
**Correction:** Added `#[derive(Clone)]` to `ResourcePath`.

**Rule:** All builder and path types in `webapp-lib` (and consuming projects) should
derive `Clone` so they can be freely captured and passed in closures without
workarounds.

