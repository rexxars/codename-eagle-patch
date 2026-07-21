//! The `CutsceneDecoder` seam.
//!
//! This trait is the boundary the FFI layer (a later task) depends on: it opens
//! a sidecar clip, exposes the frame metadata the engine reads out of the fake
//! Smack context, hands over the current frame as RGB888, and steps forward one
//! frame at a time. The concrete AV1/WebM implementation lives in
//! [`rav1d_webm`]; the trait keeps the container/codec choice swappable.

pub(crate) mod rav1d_webm;

// `Rav1dWebmDecoder` (in `rav1d_webm`) and this trait are the seam the FFI
// layer (Task 10) will consume; until then they are exercised only by tests.

/// Video-stream metadata mirroring the fields the engine reads from `SmackCtx`
/// (`width` +0x04, `height` +0x08, `frames` +0x0c) plus the pacing `fps`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct FrameMeta {
    /// Frame width in pixels.
    pub(crate) width: u32,
    /// Frame height in pixels.
    pub(crate) height: u32,
    /// Total number of frames in the clip.
    pub(crate) frames: u32,
    /// Playback rate in frames per second.
    pub(crate) fps: f64,
}

/// Fully decoded PCM for a cutscene's audio track, handed to rodio in a later
/// task. Samples are interleaved by channel (`L R L R ...` for stereo).
pub(crate) struct AudioTrack {
    /// Interleaved f32 samples in `[-1.0, 1.0]`.
    pub(crate) samples: Vec<f32>,
    /// Sample rate in Hz.
    pub(crate) sample_rate: u32,
    /// Number of interleaved channels.
    pub(crate) channels: u16,
}

/// A decoder for a single cutscene clip. `open()` decodes the first frame so
/// `current_rgb()` is valid immediately; `advance()` steps to each subsequent
/// frame and reports `false` once the stream is exhausted.
pub(crate) trait CutsceneDecoder: Send {
    /// Open `path` and decode the first frame.
    fn open(path: &str) -> std::io::Result<Self>
    where
        Self: Sized;

    /// Frame dimensions, count, and rate.
    fn meta(&self) -> FrameMeta;

    /// The current frame as tightly packed RGB888 (`width * height * 3` bytes).
    fn current_rgb(&self) -> &[u8];

    /// Decode the next frame, making it current. Returns `false` at end of
    /// stream (the current frame is then left on the last decoded frame).
    fn advance(&mut self) -> bool;

    /// Take the decoded audio track, if any. Returns `Some` at most once.
    fn take_audio(&mut self) -> Option<AudioTrack>;
}
