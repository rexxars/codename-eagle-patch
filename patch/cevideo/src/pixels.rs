//! RGB888 -> 16bpp packing and letterboxed blit into a DirectDraw-style surface.

/// Channel bit masks read from a DirectDraw surface's pixel format (e.g. RGB565
/// or RGB555). Each mask marks the contiguous bits a channel occupies in the
/// 16-bit pixel.
#[derive(Clone, Copy)]
pub(crate) struct RgbMasks {
    pub(crate) r: u16,
    pub(crate) g: u16,
    pub(crate) b: u16,
}

/// Pack an 8-bit-per-channel colour into a 16-bit pixel using `m`, deriving each
/// channel's shift (mask trailing zeros) and bit-width (mask popcount).
pub(crate) fn pack_pixel(r: u8, g: u8, b: u8, m: &RgbMasks) -> u16 {
    pack_channel(r, m.r) | pack_channel(g, m.g) | pack_channel(b, m.b)
}

/// Scale one 8-bit channel value into the bits its `mask` occupies.
fn pack_channel(value: u8, mask: u16) -> u16 {
    if mask == 0 {
        return 0;
    }
    let shift = mask.trailing_zeros();
    let width = mask.count_ones();
    // 16bpp channels are <= 8 bits wide; drop the low bits that don't fit.
    debug_assert!(width <= 8);
    let scaled = u16::from(value) >> (8u32.saturating_sub(width));
    (scaled << shift) & mask
}

/// Byte length of a `pitch`-by-`height` surface, or `None` if the product
/// overflows `usize` or exceeds `isize::MAX`.
///
/// This guards the one arithmetic that feeds `slice::from_raw_parts_mut` in the
/// FFI layer: on 32-bit i686, `usize` is 32-bit, so a garbage `pitch`/`height`
/// from the engine could otherwise wrap or produce a `len` above `isize::MAX`,
/// which is instant UB when handed to `from_raw_parts_mut` (its documented
/// precondition is `len <= isize::MAX`). A `checked_mul` + `isize::MAX` filter
/// turns that into a clean `None` the caller treats as a no-op frame.
pub(crate) fn surface_len(pitch: usize, height: usize) -> Option<usize> {
    pitch
        .checked_mul(height)
        .filter(|&n| n <= isize::MAX as usize)
}

/// The 16bpp destination surface: a byte buffer whose rows are `pitch` bytes
/// apart, `w` x `h` pixels of 2 bytes each.
pub(crate) struct DestSurface<'a> {
    pub(crate) buf: &'a mut [u8],
    pub(crate) pitch: usize,
    pub(crate) w: u32,
    pub(crate) h: u32,
}

/// The RGB888 source frame: `w` x `h` pixels, 3 bytes each, row-major.
pub(crate) struct SrcFrame<'a> {
    pub(crate) rgb: &'a [u8],
    pub(crate) w: u32,
    pub(crate) h: u32,
}

/// Centre (letterbox) an RGB888 `src` frame into the 16bpp `dst` surface.
/// Borders are left untouched. A `src` larger than `dst` in either axis is
/// clipped to the destination (offsets saturate to 0, out-of-bounds rows/cols
/// are skipped) — never a panic or out-of-bounds write. Grouping the params into
/// two structs also removes the transpose-at-call-site hazard of many bare `u32`s.
pub(crate) fn blit_frame(dst: DestSurface, src: SrcFrame, masks: &RgbMasks) {
    let x_off = (dst.w.saturating_sub(src.w) / 2) as usize;
    let y_off = (dst.h.saturating_sub(src.h) / 2) as usize;
    let src_w = src.w as usize;
    let src_h = src.h as usize;
    let dst_w = dst.w as usize;
    let dst_h = dst.h as usize;

    for row in 0..src_h {
        let dy = y_off + row;
        if dy >= dst_h {
            break; // clip oversized source vertically
        }
        for col in 0..src_w {
            let dx = x_off + col;
            if dx >= dst_w {
                break; // clip oversized source horizontally
            }
            let s = (row * src_w + col) * 3;
            let pixel = pack_pixel(src.rgb[s], src.rgb[s + 1], src.rgb[s + 2], masks);
            let d = dy * dst.pitch + dx * 2;
            let [lo, hi] = pixel.to_le_bytes();
            dst.buf[d] = lo;
            dst.buf[d + 1] = hi;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn surface_len_normal_and_overflow() {
        // Normal 640x480x16bpp surface: pitch = 640*2, height = 480.
        assert_eq!(surface_len(1280, 480), Some(1280 * 480));
        assert_eq!(surface_len(0, 480), Some(0));
        // Overflow wraps to None rather than a wrapped/huge len.
        assert_eq!(surface_len(usize::MAX, 2), None);
        assert_eq!(surface_len(usize::MAX, usize::MAX), None);
        // A product that fits usize but exceeds isize::MAX is rejected too.
        let over_isize = (isize::MAX as usize) / 2 + 1;
        assert_eq!(surface_len(over_isize, 2), None);
    }

    #[test]
    fn pack_pixel_matches_565_and_555() {
        let m565 = RgbMasks {
            r: 0xF800,
            g: 0x07E0,
            b: 0x001F,
        };
        assert_eq!(pack_pixel(0xFF, 0, 0, &m565), 0xF800);
        assert_eq!(pack_pixel(0, 0xFF, 0, &m565), 0x07E0);
        assert_eq!(pack_pixel(0, 0, 0xFF, &m565), 0x001F);
        assert_eq!(pack_pixel(0xFF, 0xFF, 0xFF, &m565), 0xFFFF);

        let m555 = RgbMasks {
            r: 0x7C00,
            g: 0x03E0,
            b: 0x001F,
        };
        assert_eq!(pack_pixel(0xFF, 0xFF, 0xFF, &m555), 0x7FFF);
    }

    #[test]
    fn pack_pixel_truncates_mid_value() {
        // A mid red 0x80 into 565's 5-bit red: 0x80 >> 3 = 0x10, << 11 = 0x8000.
        // Guards against an off-by-one in the derived shift/width.
        let m565 = RgbMasks {
            r: 0xF800,
            g: 0x07E0,
            b: 0x001F,
        };
        assert_eq!(pack_pixel(0x80, 0, 0, &m565), 0x8000);
    }

    #[test]
    fn blit_centres_2x2_into_4x4_and_keeps_borders_zero() {
        let m565 = RgbMasks {
            r: 0xF800,
            g: 0x07E0,
            b: 0x001F,
        };
        let pitch = 8; // 4 px * 2 bytes
        let mut dst_buf = vec![0u8; pitch * 4];
        // 2x2 all-white source (RGB888) -> 0xFFFF -> LE [0xFF, 0xFF].
        let src_rgb = vec![0xFFu8; 2 * 2 * 3];

        blit_frame(
            DestSurface {
                buf: &mut dst_buf,
                pitch,
                w: 4,
                h: 4,
            },
            SrcFrame {
                rgb: &src_rgb,
                w: 2,
                h: 2,
            },
            &m565,
        );

        // Interior 2x2 block lands at dst (1,1)..(2,2).
        for row in 0..2 {
            for col in 0..2 {
                let off = (1 + row) * pitch + (1 + col) * 2;
                assert_eq!(&dst_buf[off..off + 2], &[0xFF, 0xFF], "pixel r{row} c{col}");
            }
        }
        // A few border pixels stay zero.
        assert_eq!(&dst_buf[0..2], &[0, 0]); // (0,0)
        assert_eq!(&dst_buf[6..8], &[0, 0]); // (0,3)
        let corner = 3 * pitch + 3 * 2;
        assert_eq!(&dst_buf[corner..corner + 2], &[0, 0]); // (3,3)
    }

    #[test]
    fn blit_clips_oversized_source_without_oob() {
        // 3x3 source into a 2x2 dest: offsets saturate to 0, only the top-left
        // 2x2 of the source is written, no panic, no out-of-bounds.
        let m565 = RgbMasks {
            r: 0xF800,
            g: 0x07E0,
            b: 0x001F,
        };
        let pitch = 4; // 2 px * 2 bytes
        let mut dst_buf = vec![0u8; pitch * 2];
        let src_rgb = vec![0xFFu8; 3 * 3 * 3];

        blit_frame(
            DestSurface {
                buf: &mut dst_buf,
                pitch,
                w: 2,
                h: 2,
            },
            SrcFrame {
                rgb: &src_rgb,
                w: 3,
                h: 3,
            },
            &m565,
        );

        // Every dest pixel got written (no clipping gaps within bounds).
        assert_eq!(dst_buf, vec![0xFFu8; pitch * 2]);
    }
}
