# Endpoint Design Options

_Recorded: 2026-04-03_

Context: the current `Endpoint` enum has sub-resource methods (`permissions`, `items`, etc.)
defined on `impl Endpoint`, making them callable on any variant (e.g. `Endpoint::Test.permissions(...)`
compiles but should not). The goal is compile-time enforcement that sub-resource methods are
only reachable from the correct endpoint.

---

## Option C — Marker Trait + Blanket Impl (no macro)

Each endpoint is a unit struct. A marker trait carries the base URL as a `const`.
A blanket `impl` gives every marker struct `ApiEndpoint` for free.
Sub-resource methods are added only on the structs that need them.

```rust
// In webapp-lib (one-time addition to http/mod.rs)
pub trait EndpointBase {
    const BASE: &'static str;
}

impl<T: EndpointBase> ApiEndpoint for T {
    fn base(&self) -> &str { T::BASE }
}

// In the app's api.rs
pub struct Stores;
pub struct Users;
pub struct Test;

impl EndpointBase for Stores { const BASE: &'static str = "api/stores"; }
impl EndpointBase for Users  { const BASE: &'static str = "api/users";  }
impl EndpointBase for Test   { const BASE: &'static str = "api/test";   }

// Sub-resource methods only where they belong
impl Stores {
    pub fn permissions(&self, id: impl ToString) -> ResourcePath {
        self.path().id(id).child("permissions")
    }
    pub fn items(&self, id: impl ToString) -> ResourcePath {
        self.path().id(id).child("items")
    }
    // ...
}

// Usage
Stores.permissions(&id)   // ✅
Test.permissions(&id)     // ❌ compile error: no method `permissions` on `Test`
```

### Trade-offs

| | |
|---|---|
| ✅ | No macros — plain idiomatic Rust |
| ✅ | One `const BASE` per endpoint — blanket impl eliminates the second boilerplate impl |
| ✅ | `EndpointBase` trait lives in webapp-lib, available to all projects |
| ⚠️ | Still one `impl EndpointBase` + one `impl <Type>` per endpoint as the app grows |
| ⚠️ | No central list of all endpoints — they are scattered unit structs |

**Best for:** current app size, or any app where the endpoint list is small and stable.
Easy to migrate to Option B later — the struct shape is identical.

---

## Option B — `define_endpoints!` Declarative Macro (most scalable)

A single `macro_rules!` macro in webapp-lib generates a struct, an `ApiEndpoint` impl,
and all sub-resource methods from one terse declaration per endpoint.

### Call site (`api.rs`)

```rust
use webapp_lib::define_endpoints;

define_endpoints! {
    Stores = "api/stores" {
        fn items(id)           => .id(id).child("items"),
        fn sub_stores(id)      => .id(id).child("stores"),
        fn permissions(id)     => .id(id).child("permissions"),
        fn available_users(id) => .id(id).child("permissions").child("available"),
        fn leave(id)           => .id(id).child("leave"),
        fn info(id)            => .id(id).child("info"),
    },
    Users  = "api/users"  {
        fn profile(id)         => .id(id).child("profile"),
    },
    Orders = "api/orders" {},
    Test   = "api/test"   {},
}
```

### What it expands to

```rust
pub struct Stores;

impl webapp_lib::http::ApiEndpoint for Stores {
    fn base(&self) -> &str { "api/stores" }
}

impl Stores {
    pub fn items(&self, id: impl ToString) -> webapp_lib::http::ResourcePath {
        webapp_lib::http::ApiEndpoint::path(self).id(id).child("items")
    }
    pub fn permissions(&self, id: impl ToString) -> webapp_lib::http::ResourcePath {
        webapp_lib::http::ApiEndpoint::path(self).id(id).child("permissions")
    }
    // ... one method per fn line
}

pub struct Test;

impl webapp_lib::http::ApiEndpoint for Test {
    fn base(&self) -> &str { "api/test" }
}
// (no sub-resource methods generated — empty block)
```

### The macro definition (goes in `webapp-lib/src/http/mod.rs` or `macros.rs`)

```rust
#[macro_export]
macro_rules! define_endpoints {
    (
        $( $name:ident = $base:literal {
            $( fn $method:ident($id:ident) => $( .$call:ident($arg:expr) )+ , )*
        } ),*
        $(,)?
    ) => {
        $(
            pub struct $name;

            impl $crate::http::ApiEndpoint for $name {
                fn base(&self) -> &str { $base }
            }

            impl $name {
                $(
                    pub fn $method(&self, $id: impl ::std::string::ToString)
                        -> $crate::http::ResourcePath
                    {
                        $crate::http::ApiEndpoint::path(self) $( .$call($arg) )+
                    }
                )*
            }
        )*
    };
}
```

### Key design decisions

| Decision | Reason |
|---|---|
| `macro_rules!` not proc-macro | Pattern is purely repetitive; no `syn`/`quote` needed. Simpler, compiles faster. |
| Path calls use `.$call($arg)` chaining | Mirrors `ResourcePath` builder directly — no mental translation between syntax and output. |
| `$crate::http::...` qualified paths | Works wherever macro is imported without extra `use` statements. |
| No enum | Each variant is its own struct. `Test.permissions(...)` is a hard compile error. |
| `ApiEndpoint` trait still usable generically | Each struct implements the same trait, so `load_resource`, `ListComponentModel::new` etc. accept any of them. |

### Trade-offs

| | |
|---|---|
| ✅ | Most scalable — adding an endpoint is one line |
| ✅ | All definitions in one place, easy to scan |
| ✅ | Full compile-time enforcement |
| ✅ | Macro lives in webapp-lib — available to all projects |
| ⚠️ | One-time cost: implement and test the macro in webapp-lib |
| ⚠️ | Slightly less transparent (macro expansion not immediately visible) |

**Best for:** apps with many endpoints, or when the endpoint list is expected to grow.

### Extension: no-argument sub-resources

If a future sub-resource needs no `id` (e.g. just `.child("login")`), add a second
macro arm:

```rust
fn $method() => $( .$call($arg) )+
```

---

## Migration path

Option C → Option B is mechanical:
1. Implement the macro in webapp-lib.
2. Replace each `impl EndpointBase for X` + `impl X { ... }` block with one line in `define_endpoints!`.
3. Struct names and method signatures are identical — all call sites unchanged.

