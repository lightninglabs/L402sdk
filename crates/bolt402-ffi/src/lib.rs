//! C-compatible FFI layer for the bolt402 L402 client SDK.
//!
//! This crate exposes the Rust core through a C ABI, enabling bindings for
//! Go (via `CGo`), Swift, Kotlin/JNI, and any language with C FFI support.
//!
//! # Design
//!
//! - **Opaque pointers**: Rust objects are heap-allocated and passed as raw
//!   pointers. Callers must free them with the corresponding `_free` function.
//! - **Error handling**: Functions return null pointers or sentinel values on
//!   error. Call [`bolt402_last_error_message`] to retrieve the error string.
//! - **String ownership**: Strings returned by this API are heap-allocated
//!   C strings. Callers must free them with [`bolt402_string_free`].
//! - **Thread safety**: A shared tokio runtime handles async operations. All
//!   functions are safe to call from any thread.

use std::cell::RefCell;
use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::sync::OnceLock;

use bolt402_core::budget::Budget;
use bolt402_core::cache::InMemoryTokenStore;
use bolt402_core::{L402Client, L402ClientConfig};
use bolt402_mock::{EndpointConfig, MockL402Server};

// ---------------------------------------------------------------------------
// Runtime
// ---------------------------------------------------------------------------

/// Shared tokio runtime for all FFI calls.
fn runtime() -> &'static tokio::runtime::Runtime {
    static RT: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
    RT.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("failed to create tokio runtime")
    })
}

// ---------------------------------------------------------------------------
// Thread-local error
// ---------------------------------------------------------------------------

thread_local! {
    static LAST_ERROR: RefCell<Option<String>> = const { RefCell::new(None) };
}

fn set_error(msg: String) {
    LAST_ERROR.with(|e| *e.borrow_mut() = Some(msg));
}

fn clear_error() {
    LAST_ERROR.with(|e| *e.borrow_mut() = None);
}

// ---------------------------------------------------------------------------
// Opaque types
// ---------------------------------------------------------------------------

/// Opaque handle to a mock L402 server.
pub struct Bolt402MockServer {
    server: MockL402Server,
    url: CString,
}

/// Opaque handle to an L402 client.
pub struct Bolt402Client {
    inner: L402Client,
}

/// Opaque handle to an L402 response.
pub struct Bolt402Response {
    status: u16,
    paid: bool,
    body: CString,
    receipt_amount_sats: u64,
    receipt_fee_sats: u64,
    receipt_payment_hash: CString,
    receipt_preimage: CString,
    has_receipt: bool,
}

/// Endpoint configuration for the mock server (path + price in sats).
#[repr(C)]
pub struct Bolt402Endpoint {
    /// Null-terminated endpoint path (e.g. "/api/data").
    pub path: *const c_char,
    /// Price in satoshis.
    pub price_sats: u64,
}

// ---------------------------------------------------------------------------
// Error API
// ---------------------------------------------------------------------------

/// Get the last error message, or null if no error occurred.
///
/// The returned string is valid until the next FFI call on the same thread.
/// Do **not** free it with `bolt402_string_free`.
#[unsafe(no_mangle)]
pub extern "C" fn bolt402_last_error_message() -> *const c_char {
    LAST_ERROR.with(|e| {
        let borrow = e.borrow();
        match borrow.as_ref() {
            Some(msg) => {
                // Leak a CString that lives until next error set
                // This is intentional — caller reads but does not free
                let cs = CString::new(msg.as_str()).unwrap_or_default();
                cs.into_raw().cast_const()
            }
            None => std::ptr::null(),
        }
    })
}

/// Free a string allocated by bolt402 FFI functions.
///
/// # Safety
///
/// `s` must be a pointer returned by a bolt402 FFI function, or null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn bolt402_string_free(s: *mut c_char) {
    if !s.is_null() {
        unsafe {
            drop(CString::from_raw(s));
        }
    }
}

// ---------------------------------------------------------------------------
// Mock Server
// ---------------------------------------------------------------------------

/// Create a new mock L402 server with the given endpoints.
///
/// Returns null on error (check `bolt402_last_error_message`).
///
/// # Safety
///
/// `endpoints` must point to `count` valid `Bolt402Endpoint` structs.
/// Each `path` field must be a valid null-terminated C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn bolt402_mock_server_new(
    endpoints: *const Bolt402Endpoint,
    count: usize,
) -> *mut Bolt402MockServer {
    clear_error();

    if endpoints.is_null() && count > 0 {
        set_error("endpoints pointer is null".to_string());
        return std::ptr::null_mut();
    }

    let rt = runtime();
    let eps = unsafe { std::slice::from_raw_parts(endpoints, count) };

    let result = rt.block_on(async {
        let mut builder = MockL402Server::builder();
        for ep in eps {
            let path = unsafe { CStr::from_ptr(ep.path) }
                .to_str()
                .map_err(|e| format!("invalid UTF-8 in endpoint path: {e}"))?;
            builder = builder.endpoint(path, EndpointConfig::new(ep.price_sats));
        }
        builder
            .build()
            .await
            .map_err(|e| format!("failed to start mock server: {e}"))
    });

    match result {
        Ok(server) => {
            let url = CString::new(server.url()).unwrap_or_default();
            Box::into_raw(Box::new(Bolt402MockServer { server, url }))
        }
        Err(e) => {
            set_error(e);
            std::ptr::null_mut()
        }
    }
}

/// Get the URL of the mock server.
///
/// The returned string is owned by the server and valid until the server is freed.
/// Do **not** free it with `bolt402_string_free`.
///
/// # Safety
///
/// `server` must be a valid pointer from `bolt402_mock_server_new`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn bolt402_mock_server_url(
    server: *const Bolt402MockServer,
) -> *const c_char {
    if server.is_null() {
        return std::ptr::null();
    }
    unsafe { (*server).url.as_ptr() }
}

/// Free a mock server.
///
/// # Safety
///
/// `server` must be a pointer from `bolt402_mock_server_new`, or null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn bolt402_mock_server_free(server: *mut Bolt402MockServer) {
    if !server.is_null() {
        unsafe {
            drop(Box::from_raw(server));
        }
    }
}

// ---------------------------------------------------------------------------
// Client
// ---------------------------------------------------------------------------

/// Create a client connected to a mock server.
///
/// The mock server must have been created with `bolt402_mock_server_new`.
/// Returns null on error.
///
/// # Safety
///
/// `server` must be a valid pointer from `bolt402_mock_server_new`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn bolt402_client_new_mock(
    server: *const Bolt402MockServer,
    max_fee_sats: u64,
) -> *mut Bolt402Client {
    clear_error();

    if server.is_null() {
        set_error("server pointer is null".to_string());
        return std::ptr::null_mut();
    }

    let server_ref = unsafe { &*server };
    let backend = server_ref.server.mock_backend();
    let token_store = InMemoryTokenStore::default();

    let config = L402ClientConfig {
        max_fee_sats,
        max_retries: 1,
        user_agent: format!("bolt402-ffi/{}", env!("CARGO_PKG_VERSION")),
    };

    match L402Client::builder()
        .ln_backend(backend)
        .token_store(token_store)
        .budget(Budget::unlimited())
        .config(config)
        .build()
    {
        Ok(client) => Box::into_raw(Box::new(Bolt402Client { inner: client })),
        Err(e) => {
            set_error(format!("failed to build client: {e}"));
            std::ptr::null_mut()
        }
    }
}

/// Free a client.
///
/// # Safety
///
/// `client` must be a pointer from `bolt402_client_new_mock`, or null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn bolt402_client_free(client: *mut Bolt402Client) {
    if !client.is_null() {
        unsafe {
            drop(Box::from_raw(client));
        }
    }
}

// ---------------------------------------------------------------------------
// Requests
// ---------------------------------------------------------------------------

/// Send a GET request through the L402 client.
///
/// Returns null on error (check `bolt402_last_error_message`).
///
/// # Safety
///
/// `client` must be a valid client pointer. `url` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn bolt402_client_get(
    client: *mut Bolt402Client,
    url: *const c_char,
) -> *mut Bolt402Response {
    clear_error();

    if client.is_null() || url.is_null() {
        set_error("null pointer argument".to_string());
        return std::ptr::null_mut();
    }

    let client_ref = unsafe { &*client };
    let url_str = match unsafe { CStr::from_ptr(url) }.to_str() {
        Ok(s) => s,
        Err(e) => {
            set_error(format!("invalid UTF-8 in URL: {e}"));
            return std::ptr::null_mut();
        }
    };

    let rt = runtime();
    let result = rt.block_on(async { client_ref.inner.get(url_str).await });
    convert_response(rt, result)
}

/// Send a POST request through the L402 client.
///
/// `body` may be null for requests with no body.
/// Returns null on error (check `bolt402_last_error_message`).
///
/// # Safety
///
/// `client` must be a valid client pointer. `url` must be a valid C string.
/// `body` must be a valid C string or null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn bolt402_client_post(
    client: *mut Bolt402Client,
    url: *const c_char,
    body: *const c_char,
) -> *mut Bolt402Response {
    clear_error();

    if client.is_null() || url.is_null() {
        set_error("null pointer argument".to_string());
        return std::ptr::null_mut();
    }

    let client_ref = unsafe { &*client };
    let url_str = match unsafe { CStr::from_ptr(url) }.to_str() {
        Ok(s) => s,
        Err(e) => {
            set_error(format!("invalid UTF-8 in URL: {e}"));
            return std::ptr::null_mut();
        }
    };

    let body_str = if body.is_null() {
        None
    } else {
        match unsafe { CStr::from_ptr(body) }.to_str() {
            Ok(s) => Some(s),
            Err(e) => {
                set_error(format!("invalid UTF-8 in body: {e}"));
                return std::ptr::null_mut();
            }
        }
    };

    let rt = runtime();
    let result = rt.block_on(async { client_ref.inner.post(url_str, body_str).await });
    convert_response(rt, result)
}

fn convert_response(
    rt: &tokio::runtime::Runtime,
    result: Result<bolt402_core::L402Response, bolt402_proto::ClientError>,
) -> *mut Bolt402Response {
    match result {
        Ok(resp) => {
            let status = resp.status().as_u16();
            let paid = resp.paid();

            let (has_receipt, amount, fee, hash, preimage) = if let Some(r) = resp.receipt() {
                (
                    true,
                    r.amount_sats,
                    r.fee_sats,
                    CString::new(r.payment_hash.as_str()).unwrap_or_default(),
                    CString::new(r.preimage.as_str()).unwrap_or_default(),
                )
            } else {
                (false, 0, 0, CString::default(), CString::default())
            };

            let body_text = rt.block_on(async { resp.text().await }).unwrap_or_default();
            let body = CString::new(body_text).unwrap_or_default();

            Box::into_raw(Box::new(Bolt402Response {
                status,
                paid,
                body,
                has_receipt,
                receipt_amount_sats: amount,
                receipt_fee_sats: fee,
                receipt_payment_hash: hash,
                receipt_preimage: preimage,
            }))
        }
        Err(e) => {
            set_error(format!("{e}"));
            std::ptr::null_mut()
        }
    }
}

// ---------------------------------------------------------------------------
// Response accessors
// ---------------------------------------------------------------------------

/// Get the HTTP status code from a response.
///
/// # Safety
///
/// `response` must be a valid response pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn bolt402_response_status(response: *const Bolt402Response) -> u16 {
    if response.is_null() {
        return 0;
    }
    unsafe { (*response).status }
}

/// Check whether a Lightning payment was made for this response.
///
/// # Safety
///
/// `response` must be a valid response pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn bolt402_response_paid(response: *const Bolt402Response) -> bool {
    if response.is_null() {
        return false;
    }
    unsafe { (*response).paid }
}

/// Get the response body as a C string.
///
/// The returned string is owned by the response. Do not free it separately.
///
/// # Safety
///
/// `response` must be a valid response pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn bolt402_response_body(response: *const Bolt402Response) -> *const c_char {
    if response.is_null() {
        return std::ptr::null();
    }
    unsafe { (*response).body.as_ptr() }
}

/// Check whether the response contains a payment receipt.
///
/// # Safety
///
/// `response` must be a valid response pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn bolt402_response_has_receipt(response: *const Bolt402Response) -> bool {
    if response.is_null() {
        return false;
    }
    unsafe { (*response).has_receipt }
}

/// Get the amount paid in satoshis (0 if no receipt).
///
/// # Safety
///
/// `response` must be a valid response pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn bolt402_response_receipt_amount_sats(
    response: *const Bolt402Response,
) -> u64 {
    if response.is_null() {
        return 0;
    }
    unsafe { (*response).receipt_amount_sats }
}

/// Get the routing fee in satoshis (0 if no receipt).
///
/// # Safety
///
/// `response` must be a valid response pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn bolt402_response_receipt_fee_sats(
    response: *const Bolt402Response,
) -> u64 {
    if response.is_null() {
        return 0;
    }
    unsafe { (*response).receipt_fee_sats }
}

/// Get the payment hash from the receipt.
///
/// Returns null if no receipt. The string is owned by the response.
///
/// # Safety
///
/// `response` must be a valid response pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn bolt402_response_receipt_payment_hash(
    response: *const Bolt402Response,
) -> *const c_char {
    if response.is_null() {
        return std::ptr::null();
    }
    let resp = unsafe { &*response };
    if !resp.has_receipt {
        return std::ptr::null();
    }
    resp.receipt_payment_hash.as_ptr()
}

/// Get the preimage (proof of payment) from the receipt.
///
/// Returns null if no receipt. The string is owned by the response.
///
/// # Safety
///
/// `response` must be a valid response pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn bolt402_response_receipt_preimage(
    response: *const Bolt402Response,
) -> *const c_char {
    if response.is_null() {
        return std::ptr::null();
    }
    let resp = unsafe { &*response };
    if !resp.has_receipt {
        return std::ptr::null();
    }
    resp.receipt_preimage.as_ptr()
}

/// Free a response.
///
/// # Safety
///
/// `response` must be a pointer from `bolt402_client_get` or `bolt402_client_post`, or null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn bolt402_response_free(response: *mut Bolt402Response) {
    if !response.is_null() {
        unsafe {
            drop(Box::from_raw(response));
        }
    }
}

// ---------------------------------------------------------------------------
// Client state
// ---------------------------------------------------------------------------

/// Get the total amount spent by the client in satoshis.
///
/// # Safety
///
/// `client` must be a valid client pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn bolt402_client_total_spent(client: *const Bolt402Client) -> u64 {
    if client.is_null() {
        return 0;
    }
    let client_ref = unsafe { &*client };
    runtime().block_on(async { client_ref.inner.total_spent().await })
}

/// Get all receipts as a JSON array string.
///
/// The caller must free the returned string with `bolt402_string_free`.
/// Returns null on error.
///
/// # Safety
///
/// `client` must be a valid client pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn bolt402_client_receipts_json(client: *const Bolt402Client) -> *mut c_char {
    clear_error();

    if client.is_null() {
        set_error("null client pointer".to_string());
        return std::ptr::null_mut();
    }

    let client_ref = unsafe { &*client };
    let receipts = runtime().block_on(async { client_ref.inner.receipts().await });

    match serde_json::to_string(&receipts) {
        Ok(json) => match CString::new(json) {
            Ok(cs) => cs.into_raw(),
            Err(e) => {
                set_error(format!("JSON contains null byte: {e}"));
                std::ptr::null_mut()
            }
        },
        Err(e) => {
            set_error(format!("JSON serialization failed: {e}"));
            std::ptr::null_mut()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mock_server_lifecycle() {
        let path = CString::new("/api/test").unwrap();
        let endpoints = [Bolt402Endpoint {
            path: path.as_ptr(),
            price_sats: 10,
        }];

        let server = unsafe { bolt402_mock_server_new(endpoints.as_ptr(), endpoints.len()) };
        assert!(!server.is_null());

        let url = unsafe { bolt402_mock_server_url(server) };
        assert!(!url.is_null());

        let url_str = unsafe { CStr::from_ptr(url) }.to_str().unwrap();
        assert!(url_str.starts_with("http"));

        unsafe { bolt402_mock_server_free(server) };
    }

    #[test]
    fn client_lifecycle() {
        let path = CString::new("/api/test").unwrap();
        let endpoints = [Bolt402Endpoint {
            path: path.as_ptr(),
            price_sats: 10,
        }];

        let server = unsafe { bolt402_mock_server_new(endpoints.as_ptr(), endpoints.len()) };
        assert!(!server.is_null());

        let client = unsafe { bolt402_client_new_mock(server, 100) };
        assert!(!client.is_null());

        let spent = unsafe { bolt402_client_total_spent(client) };
        assert_eq!(spent, 0);

        unsafe {
            bolt402_client_free(client);
            bolt402_mock_server_free(server);
        }
    }

    #[test]
    fn get_request_with_payment() {
        let path = CString::new("/api/data").unwrap();
        let endpoints = [Bolt402Endpoint {
            path: path.as_ptr(),
            price_sats: 10,
        }];

        let server = unsafe { bolt402_mock_server_new(endpoints.as_ptr(), endpoints.len()) };
        assert!(!server.is_null());

        let client = unsafe { bolt402_client_new_mock(server, 100) };
        assert!(!client.is_null());

        // Build the full URL
        let base_url = unsafe { CStr::from_ptr(bolt402_mock_server_url(server)) }
            .to_str()
            .unwrap();
        let full_url = CString::new(format!("{base_url}/api/data")).unwrap();

        let response = unsafe { bolt402_client_get(client, full_url.as_ptr()) };
        assert!(!response.is_null(), "response should not be null");

        let status = unsafe { bolt402_response_status(response) };
        assert_eq!(status, 200);

        let paid = unsafe { bolt402_response_paid(response) };
        assert!(paid);

        let has_receipt = unsafe { bolt402_response_has_receipt(response) };
        assert!(has_receipt);

        let amount = unsafe { bolt402_response_receipt_amount_sats(response) };
        assert!(amount > 0);

        let spent = unsafe { bolt402_client_total_spent(client) };
        assert!(spent > 0);

        let receipts_json = unsafe { bolt402_client_receipts_json(client) };
        assert!(!receipts_json.is_null());
        let json_str = unsafe { CStr::from_ptr(receipts_json) }.to_str().unwrap();
        assert!(json_str.contains("amount_sats"));
        unsafe { bolt402_string_free(receipts_json) };

        unsafe {
            bolt402_response_free(response);
            bolt402_client_free(client);
            bolt402_mock_server_free(server);
        }
    }

    #[test]
    fn null_safety() {
        // All functions should handle null gracefully
        assert_eq!(unsafe { bolt402_response_status(std::ptr::null()) }, 0);
        assert!(!unsafe { bolt402_response_paid(std::ptr::null()) });
        assert!(unsafe { bolt402_response_body(std::ptr::null()) }.is_null());
        assert!(!unsafe { bolt402_response_has_receipt(std::ptr::null()) });
        assert_eq!(unsafe { bolt402_client_total_spent(std::ptr::null()) }, 0);

        // Free null should be safe
        unsafe {
            bolt402_client_free(std::ptr::null_mut());
            bolt402_mock_server_free(std::ptr::null_mut());
            bolt402_response_free(std::ptr::null_mut());
            bolt402_string_free(std::ptr::null_mut());
        }
    }
}
