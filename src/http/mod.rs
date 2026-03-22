use serde::de::DeserializeOwned;
use serde::Serialize;
use wasm_bindgen::JsCast;
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::JsFuture;
use web_sys::{Request, RequestInit, RequestMode, Response};

// ---------------------------------------------------------------------------
// Base URL — call set_base_url() once at app startup
// ---------------------------------------------------------------------------

static BASE_URL: std::sync::OnceLock<String> = std::sync::OnceLock::new();

pub fn set_base_url(url: &str) {
    let _ = BASE_URL.set(url.to_string());
}

pub(crate) fn base_url() -> &'static str {
    BASE_URL.get().map(|s| s.as_str()).unwrap_or("")
}

// ---------------------------------------------------------------------------
// HttpMethod
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum HttpMethod {
    GET,
    POST,
    PUT,
    DELETE,
}

impl HttpMethod {
    pub fn name(&self) -> String {
        format!("{:?}", self)
    }
}

// ---------------------------------------------------------------------------
// ApiError
// ---------------------------------------------------------------------------

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct ApiError {
    pub message: String,
    pub status: u16,
}

impl ApiError {
    pub fn new(message: String, status: u16) -> Self {
        ApiError { message, status }
    }

    pub fn is_unauthorized(&self) -> bool {
        self.status == 401
    }
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}] {}", self.status, self.message)
    }
}

// ---------------------------------------------------------------------------
// ResourcePath — fluent path builder
// ---------------------------------------------------------------------------

pub struct ResourcePath(Vec<String>);

impl ResourcePath {
    pub fn new(base: impl Into<String>) -> Self {
        ResourcePath(vec![base.into()])
    }

    /// Append an id segment:  /users → /users/123
    pub fn id(mut self, id: impl ToString) -> Self {
        self.0.push(id.to_string());
        self
    }

    /// Append a sub-resource name:  /users/123 → /users/123/permissions
    pub fn child(mut self, resource: impl Into<String>) -> Self {
        self.0.push(resource.into());
        self
    }

    pub fn build(&self) -> String {
        self.0.join("/")
    }
}

impl std::fmt::Display for ResourcePath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0.join("/"))
    }
}

// ---------------------------------------------------------------------------
// ApiEndpoint trait — app implements this for its endpoint enum
// ---------------------------------------------------------------------------

pub trait ApiEndpoint {
    fn base(&self) -> &str;

    fn path(&self) -> ResourcePath {
        ResourcePath::new(self.base())
    }
}

// ---------------------------------------------------------------------------
// ApiRequest
// ---------------------------------------------------------------------------

pub struct ApiRequest<'a, T: Serialize> {
    pub method: &'a HttpMethod,
    pub auth_token: Option<&'a str>,
    pub path: &'a str,
    pub payload: &'a T,
}

impl<'a, T: Serialize> ApiRequest<'a, T> {
    pub fn new(
        method: &'a HttpMethod,
        auth_token: Option<&'a str>,
        path: &'a str,
        payload: &'a T,
    ) -> Self {
        ApiRequest { method, auth_token, path, payload }
    }
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

async fn handle_response_error(resp: &Response) -> ApiError {
    let status = resp.status();
    let msg = try_extract_error_message(resp)
        .await
        .unwrap_or_else(|| get_http_error(status));
    ApiError::new(msg, status)
}

async fn try_extract_error_message(resp: &Response) -> Option<String> {
    let text = JsFuture::from(resp.text().ok()?)
        .await
        .ok()?
        .as_string()?;
    if text.is_empty() {
        return None;
    }
    let msg = serde_json::from_str::<ApiError>(&text)
        .map(|e| e.message)
        .unwrap_or(text);
    Some(msg)
}

fn js_to_api_error(e: JsValue) -> ApiError {
    parse_api_error(e)
}

// ---------------------------------------------------------------------------
// HTTP functions
// ---------------------------------------------------------------------------

pub async fn send_get<R: DeserializeOwned>(token: &str, path: &str) -> Result<R, ApiError> {
    let opts = RequestInit::new();
    opts.set_method(HttpMethod::GET.name().as_str());
    opts.set_mode(RequestMode::Cors);

    let url = format!("{}/{}", base_url(), path);
    let request = Request::new_with_str_and_init(&url, &opts).map_err(js_to_api_error)?;
    request.headers().set("Content-Type", "application/json").map_err(js_to_api_error)?;
    request.headers().set("Authorization", &format!("Bearer {}", token)).map_err(js_to_api_error)?;

    let window = web_sys::window().ok_or_else(|| ApiError::new("No window".to_string(), 0))?;
    let resp_value = JsFuture::from(window.fetch_with_request(&request))
        .await
        .map_err(js_to_api_error)?;
    let resp: Response = resp_value.dyn_into().map_err(|_| ApiError::new("Invalid response".to_string(), 0))?;

    if !resp.ok() {
        return Err(handle_response_error(&resp).await);
    }

    read_response_body(resp).await
}

pub async fn send_request<T: Serialize, R: DeserializeOwned>(
    req: ApiRequest<'_, T>,
) -> Result<R, ApiError> {
    let body = serde_json::to_string(req.payload)
        .map_err(|e| ApiError::new(format!("Serialization error: {}", e), 0))?;

    let opts = RequestInit::new();
    opts.set_method(req.method.name().as_str());
    opts.set_mode(RequestMode::Cors);
    opts.set_body(&JsValue::from_str(&body));

    let url = format!("{}/{}", base_url(), req.path);
    let request = Request::new_with_str_and_init(&url, &opts).map_err(js_to_api_error)?;
    request.headers().set("Content-Type", "application/json").map_err(js_to_api_error)?;
    if let Some(token) = req.auth_token {
        request.headers().set("Authorization", &format!("Bearer {}", token)).map_err(js_to_api_error)?;
    }

    let window = web_sys::window().ok_or_else(|| ApiError::new("No window".to_string(), 0))?;
    let resp_value = JsFuture::from(window.fetch_with_request(&request))
        .await
        .map_err(js_to_api_error)?;
    let resp: Response = resp_value.dyn_into().map_err(|_| ApiError::new("Invalid response".to_string(), 0))?;

    if !resp.ok() {
        return Err(handle_response_error(&resp).await);
    }

    read_response_body(resp).await
}

pub async fn send_delete(token: &str, path: &str) -> Result<(), ApiError> {
    let opts = RequestInit::new();
    opts.set_method(HttpMethod::DELETE.name().as_str());
    opts.set_mode(RequestMode::Cors);

    let url = format!("{}/{}", base_url(), path);
    let request = Request::new_with_str_and_init(&url, &opts).map_err(js_to_api_error)?;
    request.headers().set("Content-Type", "application/json").map_err(js_to_api_error)?;
    request.headers().set("Authorization", &format!("Bearer {}", token)).map_err(js_to_api_error)?;

    let window = web_sys::window().ok_or_else(|| ApiError::new("No window".to_string(), 0))?;
    let resp_value = JsFuture::from(window.fetch_with_request(&request))
        .await
        .map_err(js_to_api_error)?;
    let resp: Response = resp_value.dyn_into().map_err(|_| ApiError::new("Invalid response".to_string(), 0))?;

    if !resp.ok() {
        return Err(handle_response_error(&resp).await);
    }

    Ok(())
}

async fn read_response_body<R: DeserializeOwned>(resp: Response) -> Result<R, ApiError> {
    let text_val = JsFuture::from(resp.text().map_err(js_to_api_error)?)
        .await
        .map_err(js_to_api_error)?;
    let text = text_val
        .as_string()
        .ok_or_else(|| ApiError::new("Response body is not a string".to_string(), 0))?;
    serde_json::from_str::<R>(&text)
        .map_err(|e| ApiError::new(format!("Deserialization error: {}", e), 0))
}

// ---------------------------------------------------------------------------
// Error helpers
// ---------------------------------------------------------------------------

pub fn parse_api_error(e: JsValue) -> ApiError {
    if let Some(err_str) = e.as_string() {
        if let Ok(api_error) = serde_json::from_str::<ApiError>(&err_str) {
            return api_error;
        }
        return ApiError::new(err_str, 500);
    }

    let name = js_sys::Reflect::get(&e, &JsValue::from_str("name"))
        .ok()
        .and_then(|v| v.as_string());
    let message = js_sys::Reflect::get(&e, &JsValue::from_str("message"))
        .ok()
        .and_then(|v| v.as_string());

    let msg = match (name, message) {
        (_, Some(m)) => m,
        (Some(n), None) => n,
        (None, None) => format!("{:?}", e),
    };

    ApiError::new(msg, 500)
}

pub fn get_http_error(code: u16) -> String {
    match code {
        400 => "Bad Request".to_string(),
        401 => "Unauthorized".to_string(),
        403 => "Forbidden".to_string(),
        404 => "Not Found".to_string(),
        405 => "Method Not Allowed".to_string(),
        408 => "Request Timeout".to_string(),
        409 => "Conflict".to_string(),
        422 => "Unprocessable Entity".to_string(),
        429 => "Too Many Requests".to_string(),
        500 => "Internal Server Error".to_string(),
        502 => "Bad Gateway".to_string(),
        503 => "Service Unavailable".to_string(),
        504 => "Gateway Timeout".to_string(),
        _ => format!("HTTP Error {}", code),
    }
}