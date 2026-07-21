//! Cutscene audio playback via rodio.
//!
//! Unlike cemusic's looping background music, a cutscene's audio track plays
//! exactly **once** and is dropped when the clip ends or the player skips it.
//! [`play`] hands a fully decoded [`AudioTrack`] to rodio; [`stop`] silences it
//! immediately.
//!
//! rodio's `OutputStream` is `!Send`, so - as in cemusic - a dedicated thread
//! owns every rodio object and the public functions just post commands to it
//! over a channel. The thread starts lazily on the first call. If there is no
//! output device the thread logs once and exits; later `send`s then fail and are
//! ignored, so the game thread never blocks or crashes. Every entry point is
//! wrapped in `catch_unwind` so a panic can never unwind across the FFI boundary
//! (Task 10) into ce.exe.
//!
//! Volume is a pure [`apply_gain`] pass over the sample buffer before it reaches
//! rodio (the testable gain seam); it defaults to unity.

use crate::decoder::AudioTrack;
use crate::logging::log;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::OnceLock;

/// Commands posted from the game thread to the audio thread.
enum Cmd {
    /// Play this decoded track once, replacing anything currently playing.
    Play(AudioTrack),
    /// Stop and release the current track.
    Stop,
    /// Set the software gain (>= 0.0) applied to subsequently played tracks.
    Volume(f32),
}

static SENDER: OnceLock<Sender<Cmd>> = OnceLock::new();

/// Apply a linear software gain to interleaved `f32` PCM in place, clamping the
/// result back into rodio's valid `[-1.0, 1.0]` range so a boost above unity
/// can't produce out-of-range samples. Unity gain is a no-op fast path.
fn apply_gain(samples: &mut [f32], gain: f32) {
    if gain == 1.0 {
        return;
    }
    for s in samples {
        *s = (*s * gain).clamp(-1.0, 1.0);
    }
}

/// Mutable state owned by the audio thread. Held out of the thread loop so the
/// per-command handler is unit-testable with `handle: None` (the "no output
/// device" path) without touching real hardware.
struct AudioState {
    /// `Some` once an output device opened; `None` means audio is unavailable
    /// and every play/stop is drained silently.
    handle: Option<rodio::OutputStreamHandle>,
    /// The sink for the track currently playing, if any.
    sink: Option<rodio::Sink>,
    /// Software gain applied to tracks as they start (default 1.0).
    gain: f32,
}

impl AudioState {
    fn new(handle: Option<rodio::OutputStreamHandle>) -> AudioState {
        AudioState {
            handle,
            sink: None,
            gain: 1.0,
        }
    }

    /// Process one command. Never panics; failures to build a sink are logged
    /// and leave playback silent.
    fn handle_cmd(&mut self, cmd: Cmd) {
        match cmd {
            Cmd::Volume(v) => self.gain = v.max(0.0),
            Cmd::Stop => {
                if let Some(s) = self.sink.take() {
                    s.stop();
                }
            }
            Cmd::Play(mut track) => {
                // Replace whatever is playing, matching "one cutscene at a time".
                if let Some(s) = self.sink.take() {
                    s.stop();
                }
                let Some(handle) = &self.handle else {
                    return; // no output device: drain silently
                };
                apply_gain(&mut track.samples, self.gain);
                let source = rodio::buffer::SamplesBuffer::new(
                    track.channels,
                    track.sample_rate,
                    track.samples,
                );
                match rodio::Sink::try_new(handle) {
                    Ok(s) => {
                        s.append(source); // plays once - no repeat_infinite
                        s.play();
                        self.sink = Some(s);
                    }
                    Err(e) => log(&format!("cutscene audio sink error: {e}")),
                }
            }
        }
    }
}

fn sender() -> &'static Sender<Cmd> {
    SENDER.get_or_init(|| {
        let (tx, rx) = mpsc::channel();
        // The audio thread outlives every call; it ends when the process exits
        // and the last Sender drops.
        std::thread::spawn(move || audio_thread(rx));
        tx
    })
}

fn audio_thread(rx: Receiver<Cmd>) {
    // `_stream` must stay alive for the thread's lifetime or output stops.
    let (_stream, handle) = match rodio::OutputStream::try_default() {
        Ok(s) => s,
        Err(e) => {
            log(&format!("no audio output device: {e}"));
            return; // channel closes; later sends fail and are ignored
        }
    };
    let mut state = AudioState::new(Some(handle));
    while let Ok(cmd) = rx.recv() {
        state.handle_cmd(cmd);
    }
}

/// Run an entry-point body, swallowing any panic so it never unwinds across the
/// FFI boundary (Task 10) into ce.exe (which would be undefined behaviour).
fn guard(f: impl FnOnce() + std::panic::UnwindSafe) {
    let _ = std::panic::catch_unwind(f);
}

/// Play a decoded cutscene audio track once, replacing any current track.
/// Non-blocking and safe to call from the game thread.
pub(crate) fn play(track: AudioTrack) {
    guard(move || {
        let _ = sender().send(Cmd::Play(track));
    });
}

/// Stop and release any currently playing cutscene audio. Non-blocking.
pub(crate) fn stop() {
    guard(|| {
        let _ = sender().send(Cmd::Stop);
    });
}

/// Set the software gain (>= 0.0; default 1.0) applied to tracks as they start.
/// Non-blocking.
pub(crate) fn set_volume(volume: f32) {
    guard(move || {
        let _ = sender().send(Cmd::Volume(volume));
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn track(samples: Vec<f32>) -> AudioTrack {
        AudioTrack {
            samples,
            sample_rate: 44_100,
            channels: 2,
        }
    }

    #[test]
    fn unity_gain_is_identity() {
        let mut s = [-1.0, -0.5, 0.0, 0.25, 1.0];
        let before = s;
        apply_gain(&mut s, 1.0);
        assert_eq!(s, before);
    }

    #[test]
    fn half_gain_halves() {
        let mut s = [-1.0, -0.5, 0.0, 0.4, 1.0];
        apply_gain(&mut s, 0.5);
        assert_eq!(s, [-0.5, -0.25, 0.0, 0.2, 0.5]);
    }

    #[test]
    fn zero_gain_silences() {
        let mut s = [-1.0, -0.3, 0.0, 0.7, 1.0];
        apply_gain(&mut s, 0.0);
        assert_eq!(s, [0.0; 5]);
    }

    #[test]
    fn boost_clamps_into_range() {
        let mut s = [0.6, -0.8, 0.1];
        apply_gain(&mut s, 2.0);
        assert_eq!(s, [1.0, -1.0, 0.2]);
    }

    #[test]
    fn no_device_drains_play_and_stop_silently() {
        // handle: None mirrors the "no output device" thread state. Commands must
        // be handled without panicking and without creating a sink.
        let mut state = AudioState::new(None);
        state.handle_cmd(Cmd::Play(track(vec![0.1, -0.1, 0.2, -0.2])));
        assert!(state.sink.is_none());
        state.handle_cmd(Cmd::Stop);
        assert!(state.sink.is_none());
    }

    #[test]
    fn volume_command_updates_and_clamps_gain() {
        let mut state = AudioState::new(None);
        state.handle_cmd(Cmd::Volume(0.5));
        assert_eq!(state.gain, 0.5);
        // Negative volume is clamped to 0.0 (never inverts phase).
        state.handle_cmd(Cmd::Volume(-2.0));
        assert_eq!(state.gain, 0.0);
    }
}
