//! Best-effort line logging to the shim's log file.
//!
//! Every subsystem (decoder open/close, audio, the FFI exports, the proxy
//! loader) logs to the **same** file, `logs\cevideo.log`, next to the game's
//! other logs. The game's cwd is `<gamedir>` and ce-patch's startup routine has
//! already created `logs\`, so this is a plain relative append. All errors are
//! swallowed — logging must never affect playback or unwind across the FFI
//! boundary.

use std::io::Write;

/// Path shared by every logging site in the crate. Settled here so it is
/// defined in exactly one place.
pub(crate) const LOG_PATH: &str = r"logs\cevideo.log";

/// Append one line to `logs\cevideo.log`. Silent on any I/O error.
pub(crate) fn log(msg: &str) {
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(LOG_PATH)
    {
        let _ = writeln!(f, "{msg}");
    }
}
