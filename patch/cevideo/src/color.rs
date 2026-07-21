//! YUV (I420) -> RGB888 colour conversion.
//!
//! rav1d hands the shim planar YUV; the engine's surface blit wants packed
//! RGB888. The conversion lives here as a pure function so it is host-testable
//! independently of the decoder.
//!
//! Coefficients are **BT.601 limited-range** ("studio swing"): standard-
//! definition content — which is what Codename Eagle's cutscenes are, and what
//! our AV1/WebM fixtures encode (`range = tv`) — is authored in BT.601, and
//! Matroska/AV1 signal limited range unless stated otherwise. Luma spans
//! [16,235] and chroma [16,240] centred on 128:
//!
//! ```text
//! R = 1.164*(Y-16)                    + 1.596*(Cr-128)
//! G = 1.164*(Y-16) - 0.391*(Cb-128)   - 0.813*(Cr-128)
//! B = 1.164*(Y-16) + 2.018*(Cb-128)
//! ```

/// A decoded 8-bit I420 (4:2:0) frame: the three planar buffers plus their
/// dimensions and row strides. `y_stride`/`uv_stride` are bytes per row and may
/// exceed the visible width because decoders pad rows; the chroma planes are
/// half-resolution on both axes, so the chroma sample at `(col/2, row/2)`
/// serves each 2x2 luma block. Grouping the params also mirrors `pixels.rs`'s
/// `SrcFrame`/`DestSurface` and dodges the transpose-at-call-site hazard.
pub(crate) struct I420Frame<'a> {
    pub(crate) y: &'a [u8],
    pub(crate) u: &'a [u8],
    pub(crate) v: &'a [u8],
    pub(crate) width: usize,
    pub(crate) height: usize,
    pub(crate) y_stride: usize,
    pub(crate) uv_stride: usize,
}

/// Convert an I420 frame to a freshly allocated, tightly packed RGB888 buffer
/// (`width*height*3`). Uses BT.601 limited-range coefficients (see module docs).
pub(crate) fn yuv420_to_rgb888(frame: &I420Frame) -> Vec<u8> {
    let mut rgb = Vec::new();
    yuv420_to_rgb888_into(&mut rgb, frame);
    rgb
}

/// In-place variant of [`yuv420_to_rgb888`]: resize `dst` to `width*height*3`
/// and fill it with the converted frame, reusing its allocation across calls.
/// This is the hot-path form the decoder uses so `advance()` does not heap-
/// allocate a fresh buffer per frame (a 640x480 clip would otherwise churn a
/// ~900 KB `Vec` every frame).
pub(crate) fn yuv420_to_rgb888_into(dst: &mut Vec<u8>, frame: &I420Frame) {
    let (width, height) = (frame.width, frame.height);
    dst.resize(width * height * 3, 0);
    for row in 0..height {
        let y_row = row * frame.y_stride;
        let c_row = (row / 2) * frame.uv_stride;
        for col in 0..width {
            let luma = f32::from(frame.y[y_row + col]) - 16.0;
            let cb = f32::from(frame.u[c_row + col / 2]) - 128.0;
            let cr = f32::from(frame.v[c_row + col / 2]) - 128.0;
            let scaled = 1.164 * luma;
            let r = scaled + 1.596 * cr;
            let g = scaled - 0.391 * cb - 0.813 * cr;
            let b = scaled + 2.018 * cb;
            let o = (row * width + col) * 3;
            dst[o] = clamp_u8(r);
            dst[o + 1] = clamp_u8(g);
            dst[o + 2] = clamp_u8(b);
        }
    }
}

/// Round to nearest and clamp into the `u8` range.
fn clamp_u8(v: f32) -> u8 {
    v.round().clamp(0.0, 255.0) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn black_and_white_luma_extremes() {
        // 2x2, padded strides to prove stride handling. Limited-range black
        // (Y=16) -> RGB 0; limited-range white (Y=235) -> RGB 255. Neutral
        // chroma (128) throughout.
        let u = vec![128u8, 0 /*pad*/];
        let v = vec![128u8, 0 /*pad*/];
        let yb = vec![16u8, 16, 0 /*pad*/, 0, 16, 16, 0, 0];
        let rgb = yuv420_to_rgb888(&I420Frame {
            y: &yb,
            u: &u,
            v: &v,
            width: 2,
            height: 2,
            y_stride: 4,
            uv_stride: 2,
        });
        assert_eq!(rgb, vec![0u8; 2 * 2 * 3]);

        let yw = vec![235u8, 235, 0, 0, 235, 235, 0, 0];
        let rgbw = yuv420_to_rgb888(&I420Frame {
            y: &yw,
            u: &u,
            v: &v,
            width: 2,
            height: 2,
            y_stride: 4,
            uv_stride: 2,
        });
        assert_eq!(rgbw, vec![255u8; 2 * 2 * 3]);
    }

    #[test]
    fn mid_gray_luma() {
        // Y=126 (~midpoint of 16..235), neutral chroma -> ~128 grey.
        let rgb = yuv420_to_rgb888(&I420Frame {
            y: &[126u8],
            u: &[128u8],
            v: &[128u8],
            width: 1,
            height: 1,
            y_stride: 1,
            uv_stride: 1,
        });
        // 1.164*(126-16) = 128.04 -> 128.
        assert_eq!(rgb, vec![128, 128, 128]);
    }

    #[test]
    fn primary_red_triple() {
        // BT.601 limited-range encoding of pure red is ~ (Y=81, Cb=90, Cr=240).
        // It must round-trip back to a saturated red with no green/blue.
        let rgb = yuv420_to_rgb888(&I420Frame {
            y: &[81u8],
            u: &[90u8],
            v: &[240u8],
            width: 1,
            height: 1,
            y_stride: 1,
            uv_stride: 1,
        });
        assert!(rgb[0] >= 250, "red channel {} should be near 255", rgb[0]);
        assert_eq!(rgb[1], 0, "green must be 0");
        assert_eq!(rgb[2], 0, "blue must be 0");
    }
}
