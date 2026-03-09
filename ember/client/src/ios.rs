//! C FFI bindings for iOS. Built only with the "ios" feature.
//! Called from Swift via the bridging header.

use std::ffi::{CStr, CString};
use std::os::raw::c_char;

/// Ask the AI a question via the ember server.
/// Returns a pointer to a null-terminated string, or null on error.
/// Caller must free with ember_free_string.
#[no_mangle]
pub extern "C" fn ember_ask(server_addr: *const c_char, prompt: *const c_char) -> *mut c_char {
    if server_addr.is_null() || prompt.is_null() {
        return std::ptr::null_mut();
    }
    let addr_str = match unsafe { CStr::from_ptr(server_addr).to_str() } {
        Ok(s) => s.to_string(),
        Err(_) => return CString::new("Error: invalid UTF-8 address").unwrap().into_raw(),
    };
    let prompt_str = match unsafe { CStr::from_ptr(prompt).to_str() } {
        Ok(s) => s.to_string(),
        Err(_) => return CString::new("Error: invalid UTF-8 prompt").unwrap().into_raw(),
    };

    match crate::ask_ai(&addr_str, &prompt_str) {
        Ok(response) => CString::new(response).unwrap().into_raw(),
        Err(e) => CString::new(format!("Error: {}", e)).unwrap().into_raw(),
    }
}

/// Free a string returned by ember_ask.
#[no_mangle]
pub extern "C" fn ember_free_string(ptr: *mut c_char) {
    if !ptr.is_null() {
        unsafe {
            drop(CString::from_raw(ptr));
        }
    }
}
