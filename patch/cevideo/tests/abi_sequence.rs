//! ABI integration test: drive the engine's Smacker call sequence against the
//! built shim DLL, black-box, via `GetProcAddress` on the nine decorated
//! exports.
//!
//! **Windows-only and `#[ignore]`d.** These tests do not run on the macOS dev
//! host (the file is `#![cfg(windows)]`, so it compiles to nothing there) and
//! are not run by default even on Windows — they are the Phase-5 / Windows-CI
//! smoke tests. They compile-check under the `i686-pc-windows-msvc` cross target
//! (`cargo xwin test --no-run`).
//!
//! ## Windows-CI setup (Phase 5)
//!
//! 1. Build the shim: `cargo build --release` → `smackw32.dll`.
//! 2. Build the stub: `cargo build --release` in `tests/stub` → `smackw32_orig.dll`.
//! 3. Put both DLLs in one directory and run the test with that directory as the
//!    cwd (so the shim's `LoadLibraryA("smackw32_orig.dll")` resolves the stub),
//!    pointing `CEVIDEO_SHIM_DLL` at the shim:
//!    `CEVIDEO_SHIM_DLL=…\smackw32.dll cargo test --release -- --ignored`.
//!
//! The `webm` test uses the Task 7 fixture (`tests/fixtures/tiny.webm`,
//! 64×64×10 frames) copied next to a `.smk` path so the shim takes its own
//! decode path; the `smk` test uses a `.smk` path with no sidecar so the shim
//! forwards to the recording stub.

#![cfg(windows)]

use std::ffi::{c_void, CString};
use std::mem::transmute;
use std::os::raw::c_char;
use std::path::PathBuf;

use windows_sys::Win32::Foundation::HMODULE;
use windows_sys::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryA};

type RawProc = unsafe extern "system" fn() -> isize;
type OpenFn = unsafe extern "system" fn(*const c_char, u32, u32) -> *mut c_void;
type ToBufferFn = unsafe extern "system" fn(*mut c_void, u32, u32, u32, u32, *mut c_void, u32);
type VoidFn = unsafe extern "system" fn(*mut c_void);
type WaitFn = unsafe extern "system" fn(*mut c_void) -> u32;
type DdFn = unsafe extern "system" fn(*mut c_void) -> u32;
type SoundFn = unsafe extern "system" fn(u32) -> u32;

/// The nine shim exports, resolved from the built `smackw32.dll`.
struct Shim {
    _module: HMODULE, // held so the DLL stays mapped for the test
    open: OpenFn,
    to_buffer: ToBufferFn,
    do_frame: VoidFn,
    next_frame: VoidFn,
    wait: WaitFn,
    close: VoidFn,
    sound: SoundFn,
    dd: DdFn,
}

/// Resolve one export by its decorated name; panics if missing.
unsafe fn proc(module: HMODULE, name: &std::ffi::CStr) -> RawProc {
    GetProcAddress(module, name.as_ptr().cast())
        .unwrap_or_else(|| panic!("missing export {name:?} in shim DLL"))
}

impl Shim {
    fn load() -> Shim {
        let path = std::env::var("CEVIDEO_SHIM_DLL").expect(
            "set CEVIDEO_SHIM_DLL to the built smackw32.dll path (see module docs) — \
             this test is Windows/Phase-5 only",
        );
        let path_c = CString::new(path).unwrap();
        // SAFETY: `path_c` is a valid NUL-terminated path; a null return means
        // the DLL failed to load, which we surface as a panic.
        let module = unsafe { LoadLibraryA(path_c.as_ptr().cast()) };
        assert!(!module.is_null(), "failed to LoadLibrary the shim DLL");
        // SAFETY: each name is a real decorated export of the shim, transmuted
        // to its matching stdcall ABI.
        unsafe {
            Shim {
                _module: module,
                open: transmute::<RawProc, OpenFn>(proc(module, c"_SmackOpen@12")),
                to_buffer: transmute::<RawProc, ToBufferFn>(proc(module, c"_SmackToBuffer@28")),
                do_frame: transmute::<RawProc, VoidFn>(proc(module, c"_SmackDoFrame@4")),
                next_frame: transmute::<RawProc, VoidFn>(proc(module, c"_SmackNextFrame@4")),
                wait: transmute::<RawProc, WaitFn>(proc(module, c"_SmackWait@4")),
                close: transmute::<RawProc, VoidFn>(proc(module, c"_SmackClose@4")),
                sound: transmute::<RawProc, SoundFn>(proc(module, c"_SmackSoundUseDirectSound@4")),
                dd: transmute::<RawProc, DdFn>(proc(module, c"_SmackDDSurfaceType@4")),
            }
        }
    }
}

/// Read a `u32` field the engine reads out of the Smacker context by offset.
fn ctx_u32(handle: *mut c_void, offset: usize) -> u32 {
    // SAFETY: `handle` points at our (or the stub's) >= 0x378-byte context; the
    // engine-read offsets (0x04/0x08/0x0c/0x374) are within it.
    unsafe { std::ptr::read_unaligned((handle as *const u8).add(offset) as *const u32) }
}

fn fixture() -> PathBuf {
    PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/tiny.webm"
    ))
}

fn scratch_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("cevideo_{tag}_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
#[ignore = "Windows/Phase-5 ABI test: needs the built shim (CEVIDEO_SHIM_DLL) — see module docs"]
fn webm_handle_decodes_frames_into_buffer() {
    let shim = Shim::load();

    // A `.smk` path whose `.webm` sidecar exists → the shim decodes it itself.
    let dir = scratch_dir("webm");
    std::fs::copy(fixture(), dir.join("clip.webm")).unwrap();
    let smk = CString::new(dir.join("clip.smk").to_str().unwrap()).unwrap();

    // SAFETY: driving the shim's exports exactly as the engine's boot loop does.
    unsafe {
        (shim.sound)(1);
        let handle = (shim.open)(smk.as_ptr(), 0x000f_e000, 0xffff_ffff);
        assert!(!handle.is_null(), "SmackOpen should mint a webm handle");

        let width = ctx_u32(handle, 0x04);
        let height = ctx_u32(handle, 0x08);
        let frames = ctx_u32(handle, 0x0c);
        assert_eq!((width, height, frames), (64, 64, 10), "fixture metadata");

        (shim.dd)(std::ptr::null_mut()); // stash pixel masks (RGB565)

        let pitch = width * 2; // 16bpp
        let mut buf = vec![0u8; (pitch * height) as usize];

        // Engine loop: pace with Wait (nonzero = keep waiting), ToBuffer, DoFrame,
        // end when current_frame == frames - 1, else NextFrame.
        loop {
            while (shim.wait)(handle) != 0 {}
            (shim.to_buffer)(handle, 0, 0, pitch, height, buf.as_mut_ptr().cast(), 0);
            (shim.do_frame)(handle);
            if ctx_u32(handle, 0x374) == frames - 1 {
                break;
            }
            (shim.next_frame)(handle);
        }

        assert_eq!(
            ctx_u32(handle, 0x374),
            frames - 1,
            "playback should reach the last frame"
        );
        assert!(
            buf.iter().any(|&b| b != 0),
            "SmackToBuffer should have written non-zero pixels"
        );

        (shim.close)(handle);
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
#[ignore = "Windows/Phase-5 ABI test: needs the built shim + stub smackw32_orig.dll — see module docs"]
fn smk_handle_forwards_to_original_stub() {
    let log = std::env::temp_dir().join(format!("cevideo_stub_{}.log", std::process::id()));
    let _ = std::fs::remove_file(&log);
    // The stub records each forwarded call to this file.
    std::env::set_var("CEVIDEO_STUB_LOG", &log);

    let shim = Shim::load();

    // A `.smk` path with NO sidecar → the shim forwards to smackw32_orig.dll.
    let dir = scratch_dir("smk");
    let smk = CString::new(dir.join("missing.smk").to_str().unwrap()).unwrap();

    // SAFETY: driving the shim's forwarded path against the recording stub.
    unsafe {
        let handle = (shim.open)(smk.as_ptr(), 0x000f_e000, 0xffff_ffff);
        assert!(
            !handle.is_null(),
            "forwarded SmackOpen returns the stub handle"
        );
        (shim.dd)(std::ptr::null_mut());
        let mut buf = [0u8; 16];
        (shim.to_buffer)(handle, 0, 0, 4, 2, buf.as_mut_ptr().cast(), 0);
        (shim.do_frame)(handle);
        (shim.wait)(handle);
        (shim.next_frame)(handle);
        (shim.close)(handle);
    }

    let recorded = std::fs::read_to_string(&log).unwrap_or_default();
    for call in [
        "SmackOpen",
        "SmackDDSurfaceType",
        "SmackToBuffer",
        "SmackDoFrame",
        "SmackWait",
        "SmackNextFrame",
        "SmackClose",
    ] {
        assert!(
            recorded.contains(call),
            "stub should have recorded a forwarded {call}; recorded:\n{recorded}"
        );
    }
    let _ = std::fs::remove_dir_all(&dir);
}
