    /// CSS theming — embeds library CSS at compile time and injects it into `<head>` at runtime.
///
/// # Usage
///
/// Call once at app startup (before mounting):
///
/// ```rust,ignore
/// // Use built-in defaults:
/// webapp_lib::theme::init();
///
/// // Or supply custom variable values:
/// webapp_lib::theme::init_with(&ThemeVars {
///     primary: "#1a73e8",
///     accent:  "#0d47a1",
///     ..ThemeVars::default()
/// });
/// ```
///
/// Consumers can also override individual CSS variables in their own stylesheet
/// (loaded after `init()`) — any `--primary`, `--accent`, etc. redefinition wins.

const BASE_CSS: &str = include_str!("../../style/base.css");
const ICONS_CSS: &str = include_str!("../../style/icons.css");
const LOADING_CSS: &str = include_str!("../../style/loading.css");

/// Default values for every CSS custom property exposed by the library.
pub struct ThemeVars {
    pub text: &'static str,
    pub background: &'static str,
    pub primary: &'static str,
    pub secondary: &'static str,
    pub accent: &'static str,
    pub white: &'static str,
    pub error_color: &'static str,
    pub warning_color: &'static str,
}

impl Default for ThemeVars {
    fn default() -> Self {
        ThemeVars {
            text: "#050315",
            background: "#fbfbfe",
            primary: "#2f27ce",
            secondary: "#dedcff",
            accent: "#433bff",
            white: "#ffffff",
            error_color: "#e33030",
            warning_color: "#e3a330",
        }
    }
}

impl ThemeVars {
    /// Renders a `:root { … }` CSS block from the current field values.
    pub fn to_css(&self) -> String {
        format!(
            ":root {{\
                --text: {text}; \
                --background: {bg}; \
                --primary: {primary}; \
                --secondary: {secondary}; \
                --accent: {accent}; \
                --white: {white}; \
                --error-color: {err}; \
                --warning-color: {warn}; \
            }}\
            .main {{\
                --background: {bg}; \
                --primary: {primary}; \
                --secondary: {secondary}; \
                --accent: {accent}; \
            }}",
            text      = self.text,
            bg        = self.background,
            primary   = self.primary,
            secondary = self.secondary,
            accent    = self.accent,
            white     = self.white,
            err       = self.error_color,
            warn      = self.warning_color,
        )
    }
}

/// Injects a `<style id="{id}">` element into `<head>`.  No-ops if the id already exists.
fn inject_style(id: &str, css: &str) {
    let Some(window) = web_sys::window() else { return };
    let Some(document) = window.document() else { return };
    if document.get_element_by_id(id).is_some() { return; }
    let Ok(style) = document.create_element("style") else { return };
    style.set_id(id);
    style.set_inner_html(css);
    let Some(head) = document.head() else { return };
    let _ = head.append_child(&style);
}

/// Inject all library CSS using the default [`ThemeVars`].
pub fn init() {
    init_with(&ThemeVars::default());
}

/// Inject all library CSS with custom theme variable values.
///
/// Injection order:
/// 1. `webapp-lib-vars` — CSS custom properties (`:root` block)
/// 2. `webapp-lib-base` — structural styles for all library components
/// 3. `webapp-lib-icons` — Unicode icon classes
/// 4. `webapp-lib-loading` — spinner, saving animation, initial load screen
pub fn init_with(vars: &ThemeVars) {
    inject_style("webapp-lib-vars", &vars.to_css());
    inject_style("webapp-lib-base", BASE_CSS);
    inject_style("webapp-lib-icons", ICONS_CSS);
    inject_style("webapp-lib-loading", LOADING_CSS);
}