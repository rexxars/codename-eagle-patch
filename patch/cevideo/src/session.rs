//! The owned-handle routing session.
//!
//! `SmackOpen` mints one `DecoderSession` per webm cutscene it handles. The
//! session bundles everything the later per-frame exports need: the fake Smacker
//! context the engine reads by offset, the concrete decoder, the frame pacer,
//! and the playback clock.
//!
//! **Layout invariant (honoured by the FFI layer):** `ctx` is the *first* field
//! of the `#[repr(C)]` struct, so `&session as *const _` equals
//! `&session.ctx as *const SmackCtx` (offset 0). That single pointer is both the
//! `Smack*` handed to the engine *and* the key the routing registry looks it up
//! by — one identity, no side table mapping session⇄ctx. The `ctx_is_first_field`
//! test below asserts the offset and runs on the host (this module is not
//! Windows-gated).

use std::time::Instant;

use crate::decoder::CutsceneDecoder;
use crate::pacing::Pacer;
use crate::smack_struct::SmackCtx;

/// One playing (or ready-to-play) webm cutscene owned by the shim.
#[repr(C)]
pub(crate) struct DecoderSession {
    /// MUST stay first (offset 0): the engine-facing `Smack*` handle IS
    /// `&self.ctx`, which equals `&self`, and doubles as the routing key. See
    /// the module docs and `ctx_is_first_field`.
    pub(crate) ctx: SmackCtx,
    /// The video/audio decoder for this clip. `Box<dyn>` keeps the codec
    /// swappable behind the `CutsceneDecoder` seam.
    pub(crate) decoder: Box<dyn CutsceneDecoder>,
    /// Frame-pacing math (started at `t = 0` relative to `start`).
    pub(crate) pacer: Pacer,
    /// Wall-clock origin for pacing; `now_ms` is `start.elapsed()`.
    pub(crate) start: Instant,
    /// Index of the frame currently decoded/presented (mirrors
    /// `ctx.current_frame`, kept as a plain `u32` for the pacing math).
    pub(crate) frame_index: u32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::mem::offset_of;

    #[test]
    fn ctx_is_first_field() {
        // The whole routing scheme relies on the ctx pointer and the session
        // pointer being the same address.
        assert_eq!(offset_of!(DecoderSession, ctx), 0);
    }
}
