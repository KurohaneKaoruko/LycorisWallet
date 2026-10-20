//! WebAssembly bridge for the demo build.
//!
//! The static demo (`ui/?mock=1`) used to invent addresses and QR codes. This
//! crate exposes the real core — derivation, fingerprints, address validation
//! and the rounded QR renderer — so the demo displays exactly what the desktop
//! app would. It is a raw C ABI instead of `wasm-bindgen` so the artifact is a
//! single `.wasm` file with no generated shim.
//!
//! Calling convention:
//!
//! 1. JS copies the UTF-8 request into `lycoris_alloc(len)`.
//! 2. `lycoris_call(op, ptr, len)` returns a pointer to
//!    `[u32 little-endian length][UTF-8 JSON]`.
//! 3. JS reads the bytes, then calls `lycoris_result_free(ptr)`.
//!
//! Errors are JSON objects of the shape `{"error": {"code", "message"}}`; the
//! JavaScript wrapper turns those back into exceptions.

use lycoris_core::address;
use lycoris_core::chains::ChainId;
use lycoris_core::derive::{derive_private_key, preview_derivation, WalletSecret};
use lycoris_core::error::Error;
use lycoris_core::qr;
use serde_json::{json, Value};
use std::alloc::{alloc, dealloc, Layout};

const OP_PREVIEW: u32 = 1;
const OP_QR_SVG: u32 = 2;
const OP_VALIDATE_ADDRESS: u32 = 3;
const OP_PRIVATE_KEY: u32 = 4;

fn response(value: Value) -> String {
    serde_json::to_string(&value).unwrap_or_else(|_| "{\"error\":{\"code\":\"JSON\"}}".into())
}

fn failure(error: Error) -> String {
    response(json!({
        "error": { "code": error.code(), "message": error.to_string() }
    }))
}

fn parse<T: serde::de::DeserializeOwned>(input: &str) -> Result<T, Error> {
    serde_json::from_str(input).map_err(|error| Error::Message(format!("invalid request: {error}")))
}

fn dispatch(op: u32, input: &str) -> String {
    match op {
        OP_PREVIEW => {
            match parse::<WalletSecret>(input).and_then(|secret| preview_derivation(&secret)) {
                Ok(preview) => response(json!(preview)),
                Err(error) => failure(error),
            }
        }
        OP_QR_SVG => {
            #[derive(serde::Deserialize)]
            struct Request {
                text: String,
                #[serde(default)]
                dark: Option<String>,
                #[serde(default)]
                light: Option<String>,
            }
            match parse::<Request>(input).and_then(|request| {
                qr::svg(
                    &request.text,
                    request.dark.as_deref().unwrap_or("#1b1a19"),
                    request.light.as_deref().unwrap_or("#f7f1e8"),
                )
            }) {
                Ok(svg) => response(json!(svg)),
                Err(error) => failure(error),
            }
        }
        OP_VALIDATE_ADDRESS => {
            #[derive(serde::Deserialize)]
            struct Request {
                chain: ChainId,
                address: String,
            }
            match parse::<Request>(input) {
                Ok(request) => response(json!(address::validate_address(
                    request.chain,
                    &request.address
                )
                .is_ok())),
                Err(error) => failure(error),
            }
        }
        OP_PRIVATE_KEY => {
            #[derive(serde::Deserialize)]
            struct Request {
                secret: WalletSecret,
                chain: ChainId,
            }
            match parse::<Request>(input)
                .and_then(|request| derive_private_key(&request.secret, request.chain))
                .and_then(|material| material.export())
            {
                Ok(exported) => response(json!(exported)),
                Err(error) => failure(error),
            }
        }
        _ => response(json!({
            "error": { "code": "BAD_OP", "message": format!("unknown operation {op}") }
        })),
    }
}

#[no_mangle]
pub extern "C" fn lycoris_alloc(len: usize) -> *mut u8 {
    if len == 0 {
        return std::ptr::null_mut();
    }
    let layout = Layout::from_size_align(len, 1).expect("valid layout");
    unsafe { alloc(layout) }
}

/// # Safety
/// `ptr` must have been returned by [`lycoris_alloc`] with the same `len`.
#[no_mangle]
pub unsafe extern "C" fn lycoris_free(ptr: *mut u8, len: usize) {
    if ptr.is_null() || len == 0 {
        return;
    }
    let layout = Layout::from_size_align(len, 1).expect("valid layout");
    dealloc(ptr, layout)
}

/// # Safety
/// `ptr` must point at `len` bytes previously written by JavaScript.
#[no_mangle]
pub unsafe extern "C" fn lycoris_call(op: u32, ptr: *const u8, len: usize) -> *mut u8 {
    let input = if ptr.is_null() || len == 0 {
        String::new()
    } else {
        String::from_utf8_lossy(std::slice::from_raw_parts(ptr, len)).into_owned()
    };
    let output = dispatch(op, &input);
    let bytes = output.as_bytes();
    let total = 4 + bytes.len();
    let layout = Layout::from_size_align(total, 1).expect("valid layout");
    let buffer = alloc(layout);
    if buffer.is_null() {
        return std::ptr::null_mut();
    }
    std::ptr::copy_nonoverlapping((bytes.len() as u32).to_le_bytes().as_ptr(), buffer, 4);
    std::ptr::copy_nonoverlapping(bytes.as_ptr(), buffer.add(4), bytes.len());
    buffer
}

/// # Safety
/// `ptr` must be a pointer returned by [`lycoris_call`].
#[no_mangle]
pub unsafe extern "C" fn lycoris_result_free(ptr: *mut u8) {
    if ptr.is_null() {
        return;
    }
    let mut header = [0u8; 4];
    std::ptr::copy_nonoverlapping(ptr, header.as_mut_ptr(), 4);
    let total = 4 + u32::from_le_bytes(header) as usize;
    let layout = Layout::from_size_align(total, 1).expect("valid layout");
    dealloc(ptr, layout);
}
