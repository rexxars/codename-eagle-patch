//! AV1-in-WebM `CutsceneDecoder`: `matroska-demuxer` splits the container,
//! `rav1d` (a dav1d port) decodes the AV1 video track, and `symphonia` decodes
//! the Vorbis audio track. Video is decoded lazily one frame at a time (only
//! the compressed packets and the current RGB frame are held in memory); audio
//! is decoded in full on demand.
//!
//! rav1d only exposes dav1d's C ABI, so the video path is unavoidably `unsafe`
//! FFI. Every call is wrapped here with the pointer invariants documented at
//! the call site; nothing `unsafe` leaks past this module.

use std::ffi::c_int;
use std::fs::File;
use std::io;
use std::ptr::NonNull;
use std::slice;

use matroska_demuxer::{Frame, MatroskaFile, TrackType};
use rav1d::include::dav1d::data::Dav1dData;
use rav1d::include::dav1d::dav1d::{Dav1dContext, Dav1dSettings};
use rav1d::include::dav1d::headers::DAV1D_PIXEL_LAYOUT_I420;
use rav1d::include::dav1d::picture::Dav1dPicture;
use rav1d::src::lib::{
    dav1d_close, dav1d_data_create, dav1d_data_unref, dav1d_default_settings, dav1d_get_picture,
    dav1d_open, dav1d_picture_unref, dav1d_send_data,
};
use rav1d::Dav1dResult;
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::{Decoder, DecoderOptions, CODEC_TYPE_VORBIS};
use symphonia::core::formats::Packet;
use symphonia::default::codecs::VorbisDecoder;

use super::{AudioTrack, CutsceneDecoder, FrameMeta};
use crate::color::{yuv420_to_rgb888_into, I420Frame};

/// dav1d's C ABI returns `-EAGAIN` to mean "need more data / try again". The
/// value of `EAGAIN` is platform-specific, so read it from `libc`.
const EAGAIN: c_int = libc::EAGAIN;

/// Compressed Vorbis packets plus the format info needed to decode them once
/// [`CutsceneDecoder::take_audio`] is called.
struct PendingAudio {
    track: u64,
    codec_private: Vec<u8>,
    packets: Vec<Vec<u8>>,
    sample_rate: u32,
    channels: u16,
}

/// AV1/WebM implementation of [`CutsceneDecoder`].
pub(crate) struct Rav1dWebmDecoder {
    ctx: Dav1dContext,
    /// Compressed AV1 access units, one per output frame, in decode order.
    video_packets: Vec<Vec<u8>>,
    /// Index of the next packet to feed to dav1d.
    next_send: usize,
    /// A packet dav1d has not fully consumed yet (kept across `send_data`
    /// `EAGAIN`s until it is accepted).
    pending: Option<Dav1dData>,
    /// The current frame as RGB888.
    current: Vec<u8>,
    width: u32,
    height: u32,
    frames: u32,
    fps: f64,
    /// Undecoded audio, taken once by `take_audio`.
    audio: Option<PendingAudio>,
}

// SAFETY: two fields are not auto-`Send` because they hold raw pointers:
//   - `ctx`: a `RawArc` wrapping an `Arc<Rav1dContext>`, whose pointee is
//     `Send + Sync` and internally synchronised.
//   - `pending`: a `Dav1dData` holding `NonNull` pointers into a dav1d-owned
//     buffer that is itself an `Arc`-backed `[u8]` (`Send + Sync`).
// Both are exclusively owned by this decoder and are only ever *moved* between
// threads with it, never shared or aliased. Moving an owning handle to an
// Arc-backed, thread-safe pointee across threads is sound, so the whole
// decoder is `Send`. (All other fields are plain `Send` types.)
unsafe impl Send for Rav1dWebmDecoder {}

impl CutsceneDecoder for Rav1dWebmDecoder {
    fn open(path: &str) -> io::Result<Self> {
        let file = File::open(path)?;
        let mut mkv = MatroskaFile::open(file).map_err(to_io)?;

        let mut video_track: Option<u64> = None;
        let mut fps = 25.0;
        let mut audio: Option<PendingAudio> = None;

        for track in mkv.tracks() {
            match track.track_type() {
                TrackType::Video
                    if video_track.is_none() && track.codec_id().starts_with("V_AV1") =>
                {
                    video_track = Some(track.track_number().get());
                    fps = fps_from_default_duration(track.default_duration().map(|d| d.get()));
                }
                TrackType::Audio if audio.is_none() && track.codec_id() == "A_VORBIS" => {
                    if let (Some(cp), Some(a)) = (track.codec_private(), track.audio()) {
                        audio = Some(PendingAudio {
                            track: track.track_number().get(),
                            codec_private: cp.to_vec(),
                            packets: Vec::new(),
                            sample_rate: a.sampling_frequency() as u32,
                            channels: a.channels().get() as u16,
                        });
                    }
                }
                _ => {}
            }
        }

        let video_track = video_track
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "no AV1 video track"))?;
        let audio_track_num = audio.as_ref().map(|a| a.track);

        // One demux pass, routing blocks by track. Video-block count is the
        // exact output-frame count (one shown AV1 frame per block).
        let mut video_packets: Vec<Vec<u8>> = Vec::new();
        let mut frame = Frame::default();
        while mkv.next_frame(&mut frame).map_err(to_io)? {
            if frame.track == video_track {
                video_packets.push(std::mem::take(&mut frame.data));
            } else if Some(frame.track) == audio_track_num {
                if let Some(a) = audio.as_mut() {
                    a.packets.push(std::mem::take(&mut frame.data));
                }
            }
        }
        let frames = video_packets.len() as u32;

        let ctx = open_dav1d()?;

        let mut decoder = Rav1dWebmDecoder {
            ctx,
            video_packets,
            next_send: 0,
            pending: None,
            current: Vec::new(),
            width: 0,
            height: 0,
            frames,
            fps,
            audio,
        };

        // Decode the first frame so `current_rgb()` is valid immediately.
        match decoder.pull_frame() {
            Some((w, h)) => {
                decoder.width = w;
                decoder.height = h;
            }
            None => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "no decodable frames",
                ));
            }
        }

        Ok(decoder)
    }

    fn meta(&self) -> FrameMeta {
        FrameMeta {
            width: self.width,
            height: self.height,
            frames: self.frames,
            fps: self.fps,
        }
    }

    fn current_rgb(&self) -> &[u8] {
        &self.current
    }

    fn advance(&mut self) -> bool {
        match self.pull_frame() {
            Some((w, h)) => {
                self.width = w;
                self.height = h;
                true
            }
            None => false,
        }
    }

    fn take_audio(&mut self) -> Option<AudioTrack> {
        let pending = self.audio.take()?;
        decode_audio(pending)
    }
}

impl Rav1dWebmDecoder {
    /// Drive the dav1d send/get loop until one output frame is produced,
    /// writing it into `self.current` and returning its dimensions, or `None`
    /// at end of stream (or on a hard decode error).
    fn pull_frame(&mut self) -> Option<(u32, u32)> {
        loop {
            let mut pic = Dav1dPicture::default();
            // SAFETY: `ctx` is a live handle from `dav1d_open`; `pic` is a
            // valid, default-initialised `Dav1dPicture` to write into.
            let got = unsafe { dav1d_get_picture(Some(self.ctx), Some(NonNull::from(&mut pic))) };
            if got.0 == 0 {
                let dims = pic_to_rgb_into(&mut self.current, &pic);
                // SAFETY: `pic` was populated by `dav1d_get_picture`; unref
                // releases dav1d's reference to its buffers.
                unsafe { dav1d_picture_unref(Some(NonNull::from(&mut pic))) };
                return dims;
            }
            if got != Dav1dResult(-EAGAIN) {
                return None; // hard decode error
            }

            // dav1d wants more data. Take the next packet if none is pending.
            if self.pending.is_none() {
                let pkt = self.video_packets.get(self.next_send)?;
                self.pending = Some(make_data(pkt).ok()?);
                self.next_send += 1;
            }
            // `pending` was just ensured to be `Some`; bind it with `?` so
            // there is no apparent panic path (a `None` here just ends the
            // stream, matching the semantics above).
            let data = self.pending.as_mut()?;
            // SAFETY: `ctx` is live; `data` is a valid `Dav1dData` that
            // `dav1d_send_data` reads from and writes back to.
            let sent = unsafe { dav1d_send_data(Some(self.ctx), Some(NonNull::from(&mut *data))) };
            if sent.0 == 0 || data.sz == 0 {
                self.pending = None; // fully consumed
            } else if sent != Dav1dResult(-EAGAIN) {
                return None; // hard send error
            }
            // else EAGAIN: keep the packet, loop back to drain a picture.
        }
    }
}

impl Drop for Rav1dWebmDecoder {
    fn drop(&mut self) {
        if let Some(mut data) = self.pending.take() {
            // SAFETY: `data` is a valid `Dav1dData`; unref releases dav1d's ref.
            unsafe { dav1d_data_unref(Some(NonNull::from(&mut data))) };
        }
        let mut ctx = Some(self.ctx);
        // SAFETY: `self.ctx` came from `dav1d_open` and has not been closed;
        // `dav1d_close` reads and clears the `Option`, freeing the context.
        unsafe { dav1d_close(Some(NonNull::from(&mut ctx))) };
    }
}

/// Open a single-threaded, low-latency dav1d context.
fn open_dav1d() -> io::Result<Dav1dContext> {
    let mut settings = std::mem::MaybeUninit::<Dav1dSettings>::uninit();
    // SAFETY: `dav1d_default_settings` fully initialises the `Dav1dSettings`
    // it is handed; the pointer is a valid, writable stack slot.
    unsafe { dav1d_default_settings(NonNull::new(settings.as_mut_ptr()).unwrap()) };
    // SAFETY: initialised by the call above.
    let mut settings = unsafe { settings.assume_init() };
    // One frame in flight, no worker threads: `pull_frame` maps one send to one
    // get, and the decoder object stays trivially `Send`.
    settings.n_threads = 1;
    settings.max_frame_delay = 1;

    let mut ctx: Option<Dav1dContext> = None;
    // SAFETY: `ctx` and `settings` are valid stack slots to write/read.
    let res = unsafe {
        dav1d_open(
            Some(NonNull::from(&mut ctx)),
            Some(NonNull::from(&mut settings)),
        )
    };
    if res.0 != 0 {
        return Err(io::Error::other("dav1d_open failed"));
    }
    ctx.ok_or_else(|| io::Error::other("dav1d_open returned no context"))
}

/// Allocate a dav1d-owned buffer holding a copy of `pkt`, wrapped in a
/// `Dav1dData` ready for `dav1d_send_data`.
fn make_data(pkt: &[u8]) -> io::Result<Dav1dData> {
    let mut data = Dav1dData::default();
    // SAFETY: `data` is a valid, writable `Dav1dData`; `pkt.len()` is in range.
    let ptr = unsafe { dav1d_data_create(Some(NonNull::from(&mut data)), pkt.len()) };
    if ptr.is_null() {
        return Err(io::Error::other("dav1d_data_create failed"));
    }
    // SAFETY: `dav1d_data_create` returned a buffer of exactly `pkt.len()`
    // bytes; source and destination do not overlap.
    unsafe { std::ptr::copy_nonoverlapping(pkt.as_ptr(), ptr, pkt.len()) };
    Ok(data)
}

/// Convert a decoded dav1d picture into `dst` as tightly packed RGB888,
/// returning its size. Reuses `dst`'s allocation across frames. Only 8-bit
/// I420 is colour-converted (what our transcoded clips use); any other format
/// yields a neutral-grey frame so playback never reads OOB.
fn pic_to_rgb_into(dst: &mut Vec<u8>, pic: &Dav1dPicture) -> Option<(u32, u32)> {
    let w = pic.p.w;
    let h = pic.p.h;
    if w <= 0 || h <= 0 {
        return None;
    }
    let (w, h) = (w as usize, h as usize);

    if pic.p.bpc != 8 || pic.p.layout != DAV1D_PIXEL_LAYOUT_I420 {
        dst.clear();
        dst.resize(w * h * 3, 128);
        return Some((w as u32, h as u32));
    }

    // dav1d emits top-down (positive) strides for the progressive I420 content
    // we decode; the `as usize` casts below assume that. Documented as a debug
    // assert rather than a runtime branch since a negative stride never occurs.
    debug_assert!(
        pic.stride[0] > 0 && pic.stride[1] > 0,
        "unexpected non-positive dav1d stride"
    );
    let y_stride = pic.stride[0] as usize;
    let uv_stride = pic.stride[1] as usize;
    let y_ptr = pic.data[0]?.as_ptr() as *const u8;
    let u_ptr = pic.data[1]?.as_ptr() as *const u8;
    let v_ptr = pic.data[2]?.as_ptr() as *const u8;
    let chroma_h = h.div_ceil(2);

    // SAFETY: for the reported 8-bit I420 layout, dav1d guarantees the luma
    // plane is `y_stride * h` bytes and each chroma plane `uv_stride *
    // ceil(h/2)` bytes; we read exactly those extents.
    let (y, u, v) = unsafe {
        (
            slice::from_raw_parts(y_ptr, y_stride * h),
            slice::from_raw_parts(u_ptr, uv_stride * chroma_h),
            slice::from_raw_parts(v_ptr, uv_stride * chroma_h),
        )
    };
    yuv420_to_rgb888_into(
        dst,
        &I420Frame {
            y,
            u,
            v,
            width: w,
            height: h,
            y_stride,
            uv_stride,
        },
    );
    Some((w as u32, h as u32))
}

/// Frames per second from a Matroska track's `DefaultDuration` (ns per frame),
/// falling back to 25.0 when absent or degenerate.
fn fps_from_default_duration(default_duration_ns: Option<u64>) -> f64 {
    match default_duration_ns {
        Some(ns) if ns > 0 => 1e9 / ns as f64,
        _ => 25.0,
    }
}

/// Decode every Vorbis packet to interleaved f32 PCM.
fn decode_audio(pending: PendingAudio) -> Option<AudioTrack> {
    let extra = build_vorbis_extra_data(&pending.codec_private)?;

    let mut params = symphonia::core::codecs::CodecParameters::new();
    params
        .for_codec(CODEC_TYPE_VORBIS)
        .with_sample_rate(pending.sample_rate)
        .with_extra_data(extra.into_boxed_slice());

    let mut decoder = VorbisDecoder::try_new(&params, &DecoderOptions::default()).ok()?;

    let mut samples: Vec<f32> = Vec::new();
    let mut sample_rate = pending.sample_rate;
    let mut channels = pending.channels;
    let mut sbuf: Option<SampleBuffer<f32>> = None;

    for (i, pkt) in pending.packets.iter().enumerate() {
        let packet = Packet::new_from_slice(0, i as u64, 0, pkt);
        let Ok(audio) = decoder.decode(&packet) else {
            continue; // skip a corrupt packet rather than abort the track
        };
        let spec = *audio.spec();
        sample_rate = spec.rate;
        channels = spec.channels.count() as u16;
        // Invariant: the Vorbis decoder's output buffer capacity (its long
        // block size) and channel spec are fixed for the whole stream, so
        // sizing the `SampleBuffer` once from the first packet is always large
        // enough for every later `copy_interleaved_ref`.
        let buf = sbuf.get_or_insert_with(|| SampleBuffer::new(audio.capacity() as u64, spec));
        buf.copy_interleaved_ref(audio);
        samples.extend_from_slice(buf.samples());
    }

    Some(AudioTrack {
        samples,
        sample_rate,
        channels,
    })
}

/// Build the `extra_data` symphonia's Vorbis decoder expects (identification
/// header immediately followed by the setup header) from a Matroska
/// `CodecPrivate`, which packs the three Vorbis headers in Xiph lacing:
/// `[count-1][laced sizes...][ident][comment][setup]`. The comment header is
/// dropped.
fn build_vorbis_extra_data(codec_private: &[u8]) -> Option<Vec<u8>> {
    let n_minus_1 = *codec_private.first()?;
    if n_minus_1 != 2 {
        return None; // expect exactly three packed headers
    }
    let mut pos = 1;
    let mut sizes = [0usize; 2]; // ident, comment (setup is the remainder)
    for size in &mut sizes {
        loop {
            let b = *codec_private.get(pos)?;
            pos += 1;
            *size += b as usize;
            if b != 255 {
                break;
            }
        }
    }
    let ident_start = pos;
    let comment_start = ident_start.checked_add(sizes[0])?;
    let setup_start = comment_start.checked_add(sizes[1])?;
    let ident = codec_private.get(ident_start..comment_start)?;
    let setup = codec_private.get(setup_start..)?;

    let mut out = Vec::with_capacity(ident.len() + setup.len());
    out.extend_from_slice(ident);
    out.extend_from_slice(setup);
    Some(out)
}

/// Map any displayable error to an `io::Error`.
fn to_io<E: std::fmt::Display>(e: E) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> String {
        format!("{}/tests/fixtures/tiny.webm", env!("CARGO_MANIFEST_DIR"))
    }

    #[test]
    fn open_reports_expected_meta() {
        let dec = match Rav1dWebmDecoder::open(&fixture()) {
            Ok(d) => d,
            Err(e) => panic!("open failed: {e}"),
        };
        let meta = dec.meta();
        assert_eq!(meta.width, 64);
        assert_eq!(meta.height, 64);
        assert_eq!(meta.frames, 10);
        assert_eq!(meta.fps, 25.0);
        assert_eq!(dec.current_rgb().len(), 64 * 64 * 3);
    }

    #[test]
    fn advance_walks_every_frame_then_stops() {
        let mut dec = Rav1dWebmDecoder::open(&fixture()).expect("open");
        // Frame 0 is already current after open.
        let mut seen = 1;
        while dec.advance() {
            assert_eq!(dec.current_rgb().len(), 64 * 64 * 3);
            seen += 1;
        }
        assert_eq!(seen, dec.meta().frames);
        // Past the end stays false.
        assert!(!dec.advance());
    }

    #[test]
    fn take_audio_decodes_vorbis_once() {
        let mut dec = Rav1dWebmDecoder::open(&fixture()).expect("open");
        let audio = dec.take_audio().expect("audio track");
        assert_eq!(audio.sample_rate, 44100);
        assert_eq!(audio.channels, 1);
        assert!(!audio.samples.is_empty(), "decoded PCM should be non-empty");
        // Taken only once.
        assert!(dec.take_audio().is_none());
    }
}
