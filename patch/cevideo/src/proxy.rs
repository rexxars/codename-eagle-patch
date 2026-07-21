//! Original-DLL proxy loader (Windows only).
//!
//! Any `SmackOpen` the shim does **not** handle itself (no `.webm` sidecar, a
//! decode error, or a non-`.smk` path) is forwarded to the stock RAD Smacker
//! DLL, shipped renamed as `smackw32_orig.dll`. This module resolves that DLL's
//! nine exports once via `LoadLibraryA` + `GetProcAddress` (by their exact
//! decorated stdcall names) and exposes them as `extern "system"` function
//! pointers.
//!
//! If `smackw32_orig.dll` is absent (or any export is missing), [`original`]
//! returns `None`; the FFI layer then degrades every forwarded call to a safe
//! default (null handle / `0`), which is exactly the engine's existing
//! skip-the-cutscene path.

use std::ffi::{c_void, CStr};
use std::mem::transmute;
use std::os::raw::c_char;
use std::sync::OnceLock;

use windows_sys::Win32::Foundation::HMODULE;
use windows_sys::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryA};

use crate::logging::log;
use crate::smack_struct::SmackCtx;

// The nine stdcall signatures, matching the stock DLL's export table exactly.
pub(crate) type SmackOpenFn = unsafe extern "system" fn(*const c_char, u32, u32) -> *mut SmackCtx;
pub(crate) type SmackToBufferFn =
    unsafe extern "system" fn(*mut SmackCtx, u32, u32, u32, u32, *mut c_void, u32);
pub(crate) type SmackDoFrameFn = unsafe extern "system" fn(*mut SmackCtx);
pub(crate) type SmackNextFrameFn = unsafe extern "system" fn(*mut SmackCtx);
pub(crate) type SmackWaitFn = unsafe extern "system" fn(*mut SmackCtx) -> u32;
pub(crate) type SmackCloseFn = unsafe extern "system" fn(*mut SmackCtx);
pub(crate) type SmackSoundUseDirectSoundFn = unsafe extern "system" fn(u32) -> u32;
pub(crate) type SmackDDSurfaceTypeFn = unsafe extern "system" fn(*mut c_void) -> u32;
pub(crate) type SmackIsSoftwareCursorFn = unsafe extern "system" fn(*mut c_void, u32) -> u32;

/// The untyped function pointer `GetProcAddress` returns (matching `FARPROC`'s
/// inner type); transmuted to the typed pointers above.
type RawProc = unsafe extern "system" fn() -> isize;

/// Resolved entry points into the stock `smackw32_orig.dll`.
pub(crate) struct Original {
    pub(crate) open: SmackOpenFn,
    pub(crate) to_buffer: SmackToBufferFn,
    pub(crate) do_frame: SmackDoFrameFn,
    pub(crate) next_frame: SmackNextFrameFn,
    pub(crate) wait: SmackWaitFn,
    pub(crate) close: SmackCloseFn,
    pub(crate) sound_use_direct_sound: SmackSoundUseDirectSoundFn,
    pub(crate) dd_surface_type: SmackDDSurfaceTypeFn,
    pub(crate) is_software_cursor: SmackIsSoftwareCursorFn,
}

// SAFETY: `Original` holds only bare `fn` pointers, which are `Send + Sync`.
// The pointee code in the loaded DLL is immutable and never freed (the handle
// leaks for the process lifetime), so sharing a `&'static Original` across
// threads is sound.
unsafe impl Send for Original {}
unsafe impl Sync for Original {}

static ORIGINAL: OnceLock<Option<Original>> = OnceLock::new();

/// The stock DLL's exports, resolved once. `None` if `smackw32_orig.dll` is not
/// present or is missing an expected export.
pub(crate) fn original() -> Option<&'static Original> {
    ORIGINAL.get_or_init(load).as_ref()
}

/// Look up one export by its exact decorated name.
///
/// SAFETY: `module` is a live handle from `LoadLibraryA`. `GetProcAddress`
/// returns `None` for a missing export. `PCSTR` is `*const u8`, so the C-string
/// pointer (`*const c_char`) is cast accordingly.
unsafe fn proc(module: HMODULE, name: &CStr) -> Option<RawProc> {
    GetProcAddress(module, name.as_ptr().cast())
}

fn load() -> Option<Original> {
    // SAFETY: a NUL-terminated ASCII path; `LoadLibraryA` returns null on
    // failure (DLL absent), which we check before use.
    let module = unsafe { LoadLibraryA(c"smackw32_orig.dll".as_ptr().cast()) };
    if module.is_null() {
        log("cevideo: smackw32_orig.dll not found; .smk forwarding disabled");
        return None;
    }

    // SAFETY: `module` is live. Each name is the exact decorated stdcall export
    // from the stock DLL's table, so the transmute rebinds a real function of
    // the matching ABI to its typed pointer. Any missing export short-circuits
    // the whole resolution to `None` (`?`), so a partial DLL disables
    // forwarding rather than binding a wrong pointer.
    let original = unsafe {
        Original {
            open: transmute::<RawProc, SmackOpenFn>(proc(module, c"_SmackOpen@12")?),
            to_buffer: transmute::<RawProc, SmackToBufferFn>(proc(module, c"_SmackToBuffer@28")?),
            do_frame: transmute::<RawProc, SmackDoFrameFn>(proc(module, c"_SmackDoFrame@4")?),
            next_frame: transmute::<RawProc, SmackNextFrameFn>(proc(module, c"_SmackNextFrame@4")?),
            wait: transmute::<RawProc, SmackWaitFn>(proc(module, c"_SmackWait@4")?),
            close: transmute::<RawProc, SmackCloseFn>(proc(module, c"_SmackClose@4")?),
            sound_use_direct_sound: transmute::<RawProc, SmackSoundUseDirectSoundFn>(proc(
                module,
                c"_SmackSoundUseDirectSound@4",
            )?),
            dd_surface_type: transmute::<RawProc, SmackDDSurfaceTypeFn>(proc(
                module,
                c"_SmackDDSurfaceType@4",
            )?),
            is_software_cursor: transmute::<RawProc, SmackIsSoftwareCursorFn>(proc(
                module,
                c"_SmackIsSoftwareCursor@8",
            )?),
        }
    };
    log("cevideo: smackw32_orig.dll loaded; .smk cutscenes will forward");
    Some(original)
}
