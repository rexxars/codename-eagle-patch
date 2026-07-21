//! Call-recording stand-in for the stock `smackw32_orig.dll`.
//!
//! Built as `smackw32_orig.dll` and dropped next to the shim so cevideo's proxy
//! loader ([`crate::proxy`] in the main crate) resolves it. Each of the nine
//! exports appends its name to the file named by the `CEVIDEO_STUB_LOG`
//! environment variable (default `smackw32_orig_calls.log` in the cwd), so the
//! ABI integration test can assert which forwarded calls reached the original.
//! `SmackOpen` returns a stable, non-null fake handle distinct from any handle
//! the shim mints, so the shim keeps forwarding every later call for it here.
//!
//! Like the shim, exact decorated stdcall names are produced via a `.def` file
//! (see `smackw32_orig.def` + `build.rs`), because `#[export_name]` re-decorates
//! stdcall symbols on i686-msvc.

use std::ffi::c_void;
use std::io::Write;
use std::os::raw::c_char;

/// A fixed, readable, non-null block used as the fake Smacker handle returned by
/// the stub's `SmackOpen`. Its address is stable for the process lifetime.
static FAKE_CTX: [u8; 0x400] = [0u8; 0x400];

/// Record one call by name, best-effort (any I/O error is ignored).
fn record(call: &str) {
    let path =
        std::env::var("CEVIDEO_STUB_LOG").unwrap_or_else(|_| "smackw32_orig_calls.log".to_string());
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        let _ = writeln!(f, "{call}");
    }
}

#[no_mangle]
pub extern "system" fn stub_open(_name: *const c_char, _bufsize: u32, _flags: u32) -> *mut c_void {
    record("SmackOpen");
    FAKE_CTX.as_ptr() as *mut c_void
}

#[no_mangle]
pub extern "system" fn stub_to_buffer(
    _smk: *mut c_void,
    _left: u32,
    _top: u32,
    _pitch: u32,
    _height: u32,
    _buf: *mut c_void,
    _flags: u32,
) {
    record("SmackToBuffer");
}

#[no_mangle]
pub extern "system" fn stub_do_frame(_smk: *mut c_void) {
    record("SmackDoFrame");
}

#[no_mangle]
pub extern "system" fn stub_next_frame(_smk: *mut c_void) {
    record("SmackNextFrame");
}

#[no_mangle]
pub extern "system" fn stub_wait(_smk: *mut c_void) -> u32 {
    record("SmackWait");
    0 // never keep the engine waiting
}

#[no_mangle]
pub extern "system" fn stub_close(_smk: *mut c_void) {
    record("SmackClose");
}

#[no_mangle]
pub extern "system" fn stub_sound_use_direct_sound(_arg: u32) -> u32 {
    record("SmackSoundUseDirectSound");
    0
}

#[no_mangle]
pub extern "system" fn stub_dd_surface_type(_surface: *mut c_void) -> u32 {
    record("SmackDDSurfaceType");
    0
}

#[no_mangle]
pub extern "system" fn stub_is_software_cursor(_surface: *mut c_void, _cursor: u32) -> u32 {
    record("SmackIsSoftwareCursor");
    0
}
