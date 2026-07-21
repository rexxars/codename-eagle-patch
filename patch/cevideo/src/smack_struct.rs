//! ABI-stable stand-in for the real Smacker context struct.
//!
//! The 1.41 engine reads several fields straight out of the Smacker context by
//! fixed byte offset (recovered from the decompile), so our fake context must
//! reproduce that exact layout. Explicit `_padN` byte-array fillers place each
//! named field at its engine offset. `#[repr(C)]` keeps field order stable and,
//! because every named field already lands on its natural alignment (all `u32`
//! offsets are 4-aligned; `pal_related` at the even offset `0x8a` is 2-aligned),
//! no `#[repr(packed)]` is needed — the struct stays naturally aligned, so
//! `offset_of!` and field references are free of unaligned-access UB.

/// Fake Smacker context laid out to match what `ce.exe` reads by offset.
///
/// All fields are ABI slots the engine (or the later FFI layer) touches via raw
/// pointer, so they read as dead code to the host test build.
#[repr(C)]
pub(crate) struct SmackCtx {
    _pad00: [u8; 0x04],
    /// Frame width in pixels. Engine reads `*(u32*)(smk+0x04)`.
    pub(crate) width: u32,
    /// Frame height in pixels. Engine reads `*(u32*)(smk+0x08)`.
    pub(crate) height: u32,
    /// Total frame count. Engine reads `*(u32*)(smk+0x0c)`.
    pub(crate) frames: u32,
    _pad10: [u8; 0x68 - 0x10],
    /// Non-zero signals a new palette this frame. Engine checks `!= 0` at `+0x68`.
    pub(crate) new_palette: u32,
    _pad6c: [u8; 0x8a - 0x6c],
    /// Palette-related flag at the odd-ish offset `0x8a`; present so the layout
    /// doesn't alias even though we may never set it.
    pub(crate) pal_related: u16,
    _pad8c: [u8; 0x374 - 0x8c],
    /// Index of the frame currently decoded. Engine reads `*(u32*)(smk+0x374)`.
    pub(crate) current_frame: u32,
}

impl SmackCtx {
    /// A zeroed context with the given dimensions/frame count. Returned by value:
    /// the FFI layer embeds it as the first field of a boxed `DecoderSession`, so
    /// the engine still gets a stable pointer to the fixed layout without a
    /// separate allocation here.
    pub(crate) fn new(width: u32, height: u32, frames: u32) -> SmackCtx {
        SmackCtx {
            _pad00: [0; 0x04],
            width,
            height,
            frames,
            _pad10: [0; 0x68 - 0x10],
            new_palette: 0,
            _pad6c: [0; 0x8a - 0x6c],
            pal_related: 0,
            _pad8c: [0; 0x374 - 0x8c],
            current_frame: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::mem::offset_of;

    #[test]
    fn fields_land_at_engine_offsets() {
        assert_eq!(offset_of!(SmackCtx, width), 0x04);
        assert_eq!(offset_of!(SmackCtx, height), 0x08);
        assert_eq!(offset_of!(SmackCtx, frames), 0x0c);
        assert_eq!(offset_of!(SmackCtx, new_palette), 0x68);
        assert_eq!(offset_of!(SmackCtx, pal_related), 0x8a);
        assert_eq!(offset_of!(SmackCtx, current_frame), 0x374);
        assert!(core::mem::size_of::<SmackCtx>() >= 0x378);
    }

    #[test]
    fn new_sets_dims_and_zeroes_the_rest() {
        let ctx = SmackCtx::new(320, 200, 42);
        assert_eq!(ctx.width, 320);
        assert_eq!(ctx.height, 200);
        assert_eq!(ctx.frames, 42);
        assert_eq!(ctx.new_palette, 0);
        assert_eq!(ctx.pal_related, 0);
        assert_eq!(ctx.current_frame, 0);
    }
}
