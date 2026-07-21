//! The nine `smackw32.dll` exports + owned/forwarded routing (Windows only).
//!
//! Each function is a plain `#[no_mangle]` stdcall symbol; `smackw32.def`
//! (applied by `build.rs` as a cdylib `/DEF` link-arg) re-exports it under the
//! engine's exact decorated name (`_Smack…@N`). `#[export_name]` can't do this
//! on i686-msvc — the linker re-decorates a stdcall `export_name` into
//! `__name@N@M` — so the `.def` alias is the reliable mechanism. The body wraps
//! its logic in [`guard`]
//! (`catch_unwind`) so a Rust panic can never unwind across the FFI boundary
//! into `ce.exe` (undefined behaviour), then routes on the incoming handle:
//!
//! * A handle in the [`sessions`] registry is one *we* minted in `SmackOpen` for
//!   a webm cutscene — handled here (decode / blit / pace / audio).
//! * Any other handle is forwarded to the stock DLL via [`original`]; with no
//!   stock DLL present, forwarded calls return a safe default (null / `0`),
//!   matching the engine's existing skip-on-fail behaviour.
//!
//! **Handle identity:** `SmackOpen` returns `&session.ctx`, which (ctx is the
//! first `#[repr(C)]` field of [`DecoderSession`]) equals the session's own
//! address. So the `Smack*` the engine passes back on every later call *is* the
//! registry key.
//!
//! **SmackWait polarity** (from `FUN_00447a60`/`FUN_00447840` in the 1.41
//! decompile): the boot loop presents & advances only when
//! `_SmackWait_4(smk) == 0`; a nonzero return means "keep waiting". So we return
//! `1` while the pacer says the current frame is still on-screen, `0` once its
//! wall-clock deadline has passed.

use std::collections::HashMap;
use std::ffi::{c_void, CStr};
use std::os::raw::c_char;
use std::panic::{self, AssertUnwindSafe};
use std::ptr;
use std::slice;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Mutex, MutexGuard, OnceLock};
use std::time::Instant;

use crate::audio;
use crate::bigstack::run_on_big_stack;
use crate::decoder::rav1d_webm::Rav1dWebmDecoder;
use crate::decoder::CutsceneDecoder;
use crate::logging::log;
use crate::pacing::Pacer;
use crate::pixels::{blit_frame, surface_len, DestSurface, RgbMasks, SrcFrame};
use crate::proxy::original;
use crate::session::DecoderSession;
use crate::sidecar::sidecar_path;
use crate::smack_struct::SmackCtx;

/// The sidecar container extension we look for next to each `.smk` path.
const SIDECAR_EXT: &str = "webm";

/// Run an export body under `catch_unwind`, returning `default` if it panics.
/// `AssertUnwindSafe` is sound here: on a panic we discard the in-flight result
/// and return the safe default without further touching any possibly-poisoned
/// shim state in this call.
fn guard<T>(default: T, body: impl FnOnce() -> T) -> T {
    match panic::catch_unwind(AssertUnwindSafe(body)) {
        Ok(value) => value,
        Err(_) => {
            log("cevideo: caught panic at FFI boundary (returned safe default)");
            default
        }
    }
}

/// The owned-handle registry: `ctx pointer as usize` → session. Poison is
/// recovered (a prior panic must not brick every later cutscene).
fn sessions() -> MutexGuard<'static, HashMap<usize, Box<DecoderSession>>> {
    static SESSIONS: OnceLock<Mutex<HashMap<usize, Box<DecoderSession>>>> = OnceLock::new();
    SESSIONS
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .unwrap_or_else(|poison| poison.into_inner())
}

/// The RGB channel masks last stashed by `SmackDDSurfaceType`, used by our
/// `SmackToBuffer` to pack RGB888 → 16bpp. Defaults to RGB565 (the engine's
/// 640×480×16bpp display in practice). Poison-recovered like `sessions`.
fn masks() -> MutexGuard<'static, RgbMasks> {
    static MASKS: OnceLock<Mutex<RgbMasks>> = OnceLock::new();
    MASKS
        .get_or_init(|| {
            Mutex::new(RgbMasks {
                r: 0xF800,
                g: 0x07E0,
                b: 0x001F,
            })
        })
        .lock()
        .unwrap_or_else(|poison| poison.into_inner())
}

/// Last `SmackSoundUseDirectSound` flag, noted for our path (informational).
static SOUND_FLAG: AtomicU32 = AtomicU32::new(0);

// ---------------------------------------------------------------------------
// SmackOpen
// ---------------------------------------------------------------------------

#[no_mangle]
pub extern "system" fn smack_open(name: *const c_char, bufsize: u32, flags: u32) -> *mut SmackCtx {
    guard(ptr::null_mut(), || open_impl(name, bufsize, flags))
}

fn open_impl(name: *const c_char, bufsize: u32, flags: u32) -> *mut SmackCtx {
    // Forward to the stock DLL, or null if there is none.
    let forward = || match original() {
        // SAFETY: forwarding the engine's own arguments unchanged to the stock
        // export of the matching ABI.
        Some(orig) => unsafe { (orig.open)(name, bufsize, flags) },
        None => ptr::null_mut(),
    };

    if name.is_null() {
        return forward();
    }
    // SAFETY: `name` is a non-null NUL-terminated C string from the engine.
    let Ok(path) = (unsafe { CStr::from_ptr(name) }).to_str() else {
        return forward();
    };

    // Only `.smk` paths map to a sidecar; anything else is the stock DLL's.
    let Some(sidecar) = sidecar_path(path, SIDECAR_EXT) else {
        return forward();
    };
    if !std::path::Path::new(&sidecar).exists() {
        return forward();
    }

    // rav1d decode runs on a large-stack worker thread: the game's 32-bit main
    // thread has only a ~1 MiB stack, which AV1 frame decode overflows
    // (EXCEPTION_STACK_OVERFLOW). `open` decodes frame 0, so it must hop too.
    log(&format!("cevideo: opening sidecar {sidecar}"));
    let mut decoder = match run_on_big_stack(|| Rav1dWebmDecoder::open(&sidecar)) {
        Ok(decoder) => decoder,
        Err(err) => {
            log(&format!(
                "cevideo: sidecar {sidecar} present but decode-open failed ({err}); forwarding"
            ));
            return forward();
        }
    };
    let meta = decoder.meta();

    // Zero the video clock and pacer *before* dispatching audio, so the video's
    // start point isn't skewed behind the moment audio playback is kicked off.
    let mut pacer = Pacer::new(meta.fps);
    let start = Instant::now();
    pacer.start(0); // now_ms is measured relative to `start`

    // Kick off the audio track (if any) so it plays alongside the frames.
    // Vorbis decode also runs on the big-stack worker, for the same reason.
    if let Some(track) = run_on_big_stack(|| decoder.take_audio()) {
        audio::play(track);
    }

    let session = Box::new(DecoderSession {
        ctx: SmackCtx::new(meta.width, meta.height, meta.frames),
        decoder: Box::new(decoder),
        pacer,
        start,
        frame_index: 0,
    });

    // The engine-facing handle is the ctx pointer, which (ctx at offset 0)
    // equals the session pointer and is the registry key.
    let handle = &session.ctx as *const SmackCtx as usize;
    sessions().insert(handle, session);
    log(&format!(
        "cevideo: playing {sidecar} ({}x{}, {} frames @ {:.3} fps)",
        meta.width, meta.height, meta.frames, meta.fps
    ));
    handle as *mut SmackCtx
}

// ---------------------------------------------------------------------------
// SmackToBuffer
// ---------------------------------------------------------------------------

#[no_mangle]
pub extern "system" fn smack_to_buffer(
    smk: *mut SmackCtx,
    left: u32,
    top: u32,
    dest_pitch: u32,
    dest_height: u32,
    buf: *mut c_void,
    flags: u32,
) {
    guard((), || {
        to_buffer_impl(smk, left, top, dest_pitch, dest_height, buf, flags)
    })
}

fn to_buffer_impl(
    smk: *mut SmackCtx,
    left: u32,
    top: u32,
    dest_pitch: u32,
    dest_height: u32,
    buf: *mut c_void,
    flags: u32,
) {
    let handle = smk as usize;
    {
        let sessions = sessions();
        if let Some(session) = sessions.get(&handle) {
            if buf.is_null() || dest_pitch == 0 || dest_height == 0 {
                return;
            }
            let pitch = dest_pitch as usize;
            let height = dest_height as usize;
            // Guard the length before building the slice: on 32-bit i686 a
            // garbage pitch*height could wrap `usize` or exceed `isize::MAX`,
            // and `from_raw_parts_mut` with such a `len` is instant UB. A `None`
            // here means "implausible surface" → treat as a no-op frame.
            let Some(len) = surface_len(pitch, height) else {
                log("cevideo: SmackToBuffer surface length overflow; skipping frame");
                return;
            };
            // SAFETY: the engine locks the whole DirectDraw back-buffer and
            // passes its base pointer with `pitch = visible_width * 2` and
            // `dest_height = surface height` (never a sub-rectangle lock), so the
            // buffer is exactly `pitch * height` bytes; `len` is that product,
            // checked above to be `<= isize::MAX`, and all writes below stay
            // within `full`.
            let full = unsafe { slice::from_raw_parts_mut(buf as *mut u8, len) };

            // Honour (left, top) as the top-left origin of the drawable region;
            // the engine passes (0, 0). The video is then letterboxed within
            // the remaining region by `blit_frame`. NOTE: `full_w = pitch / 2`
            // assumes zero row padding (pitch == visible_width * 2); like the
            // RGB565 assumption in `SmackDDSurfaceType`, this must be validated
            // against real DirectDraw surfaces in Phase 5.
            let full_w = pitch / 2; // 16bpp
            let (x0, y0) = (left as usize, top as usize);
            if x0 >= full_w || y0 >= height {
                return; // origin outside the surface
            }
            let byte_off = y0 * pitch + x0 * 2;
            if byte_off >= len {
                return;
            }
            let region_w = (full_w - x0) as u32;
            let region_h = (height - y0) as u32;

            let m = *masks();
            let meta = session.decoder.meta();
            let rgb = session.decoder.current_rgb();
            blit_frame(
                DestSurface {
                    buf: &mut full[byte_off..],
                    pitch,
                    w: region_w,
                    h: region_h,
                },
                SrcFrame {
                    rgb,
                    w: meta.width,
                    h: meta.height,
                },
                &m,
            );
            return;
        }
    }
    // Not ours: forward.
    if let Some(orig) = original() {
        // SAFETY: forwarding the engine's arguments unchanged to the stock export.
        unsafe { (orig.to_buffer)(smk, left, top, dest_pitch, dest_height, buf, flags) };
    }
}

// ---------------------------------------------------------------------------
// SmackDoFrame
// ---------------------------------------------------------------------------

#[no_mangle]
pub extern "system" fn smack_do_frame(smk: *mut SmackCtx) {
    guard((), || do_frame_impl(smk));
}

fn do_frame_impl(smk: *mut SmackCtx) {
    let handle = smk as usize;
    if sessions().contains_key(&handle) {
        // Our model: the frame was decoded in `open`/`SmackNextFrame` and blitted
        // by `SmackToBuffer`, so presenting the current frame is a no-op.
        return;
    }
    if let Some(orig) = original() {
        // SAFETY: forwarding to the stock export.
        unsafe { (orig.do_frame)(smk) };
    }
}

// ---------------------------------------------------------------------------
// SmackNextFrame
// ---------------------------------------------------------------------------

#[no_mangle]
pub extern "system" fn smack_next_frame(smk: *mut SmackCtx) {
    guard((), || next_frame_impl(smk));
}

fn next_frame_impl(smk: *mut SmackCtx) {
    let handle = smk as usize;
    {
        let mut sessions = sessions();
        if let Some(session) = sessions.get_mut(&handle) {
            let last = session.ctx.frames.saturating_sub(1);
            // Decode the next frame on the big-stack worker (see `open`).
            if run_on_big_stack(|| session.decoder.advance()) {
                // Normal advance; never run past the last frame (the engine ends
                // the reel when `current_frame == frames - 1`, checked before
                // this call).
                session.frame_index = (session.frame_index + 1).min(last);
            } else {
                // End of stream OR a hard mid-clip decode error: both jump to the
                // last frame so `current_frame == frames - 1` fires and the engine
                // terminates the reel with the last good frame on screen. Without
                // this, a decode failure would freeze `frame_index` below `last`
                // and the engine's loop would spin forever (full game hang).
                session.frame_index = last;
            }
            session.ctx.current_frame = session.frame_index;
            // Truecolor output: there is never a palette to signal.
            session.ctx.new_palette = 0;
            return;
        }
    }
    if let Some(orig) = original() {
        // SAFETY: forwarding to the stock export.
        unsafe { (orig.next_frame)(smk) };
    }
}

// ---------------------------------------------------------------------------
// SmackWait
// ---------------------------------------------------------------------------

#[no_mangle]
pub extern "system" fn smack_wait(smk: *mut SmackCtx) -> u32 {
    guard(0, || wait_impl(smk))
}

fn wait_impl(smk: *mut SmackCtx) -> u32 {
    let handle = smk as usize;
    {
        let sessions = sessions();
        if let Some(session) = sessions.get(&handle) {
            let now_ms = session.start.elapsed().as_millis() as u64;
            // Nonzero == "keep waiting" (engine advances only on a 0 return).
            return u32::from(session.pacer.should_wait(now_ms, session.frame_index));
        }
    }
    match original() {
        // SAFETY: forwarding to the stock export.
        Some(orig) => unsafe { (orig.wait)(smk) },
        None => 0,
    }
}

// ---------------------------------------------------------------------------
// SmackClose
// ---------------------------------------------------------------------------

#[no_mangle]
pub extern "system" fn smack_close(smk: *mut SmackCtx) {
    guard((), || close_impl(smk));
}

fn close_impl(smk: *mut SmackCtx) {
    let handle = smk as usize;
    // Removing drops the session (and its decoder), releasing all resources.
    let removed = sessions().remove(&handle);
    if removed.is_some() {
        audio::stop();
        log("cevideo: closed cutscene");
        return;
    }
    if let Some(orig) = original() {
        // SAFETY: forwarding to the stock export.
        unsafe { (orig.close)(smk) };
    }
}

// ---------------------------------------------------------------------------
// SmackSoundUseDirectSound
// ---------------------------------------------------------------------------

#[no_mangle]
pub extern "system" fn smack_sound_use_direct_sound(arg: u32) -> u32 {
    guard(0, || {
        // Note the flag for our (webm) path — our audio runs through rodio, not
        // DirectSound, so it is informational — and forward to the stock DLL,
        // which is harmless and keeps the `.smk` path's audio init intact.
        SOUND_FLAG.store(arg, Ordering::Relaxed);
        match original() {
            // SAFETY: forwarding to the stock export.
            Some(orig) => unsafe { (orig.sound_use_direct_sound)(arg) },
            None => 0,
        }
    })
}

// ---------------------------------------------------------------------------
// SmackDDSurfaceType
// ---------------------------------------------------------------------------

#[no_mangle]
pub extern "system" fn smack_dd_surface_type(surface: *mut c_void) -> u32 {
    guard(0, || dd_surface_type_impl(surface))
}

fn dd_surface_type_impl(surface: *mut c_void) -> u32 {
    // TODO(phase5): read the real DDPIXELFORMAT masks from the DirectDraw
    // surface via its COM vtable and stash those instead. The engine sets a
    // 640×480×16bpp display that is RGB565 in practice, and this path cannot be
    // runtime-tested on the dev host, so we assume RGB565 for now.
    *masks() = RgbMasks {
        r: 0xF800,
        g: 0x07E0,
        b: 0x001F,
    };
    match original() {
        // SAFETY: forwarding to the stock export, whose returned code the engine
        // caches and passes back to `SmackToBuffer` as `flags` (which our webm
        // path ignores in favour of the stashed masks).
        Some(orig) => unsafe { (orig.dd_surface_type)(surface) },
        None => 0,
    }
}

// ---------------------------------------------------------------------------
// SmackIsSoftwareCursor
// ---------------------------------------------------------------------------

#[no_mangle]
pub extern "system" fn smack_is_software_cursor(surface: *mut c_void, cursor: u32) -> u32 {
    guard(0, || match original() {
        // SAFETY: forwarding to the stock export.
        Some(orig) => unsafe { (orig.is_software_cursor)(surface, cursor) },
        None => 0,
    })
}
