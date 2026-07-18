//! cemusic.dll - file-based background music for Codename Eagle.
//!
//! ce-patch detours the engine's CD-music functions (FUN_00483020 play /
//! FUN_00483170 stop / FUN_00486b10 volume) into these three exports, so music
//! comes from `<gamedir>\music\trackNN.ogg` (patent-free Ogg Vorbis) instead of
//! CD Redbook audio. See ../README.md.
//!
//! Independent volume: playback runs on our own output stream (cpal/WASAPI) and
//! `Sink::set_volume` is a software gain on the music samples, so the in-game
//! music slider scales only music and never touches the game's DirectSound SFX.
//!
//! rodio's `OutputStream` is `!Send`, so a dedicated thread owns all the rodio
//! objects and the exports just post commands to it over a channel. The thread
//! starts lazily on the first call; `DllMain` does nothing.
//!
//! Missing files never make noise or crash: a missing/undecodable track simply
//! stops current playback and plays silence (logged to `logs\cemusic.log`).

use rodio::Source;
use std::fs::File;
use std::io::{BufReader, Write};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::OnceLock;

enum Cmd {
    Play(u32),
    Stop,
    Volume(f32),
}

static SENDER: OnceLock<Sender<Cmd>> = OnceLock::new();

/// Best-effort line log next to the game's other logs (cwd is `<gamedir>`, and
/// ce-patch's startup routine has already created `logs\`). Errors are ignored.
fn log(msg: &str) {
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(r"logs\cemusic.log") {
        let _ = writeln!(f, "{msg}");
    }
}

fn sender() -> &'static Sender<Cmd> {
    SENDER.get_or_init(|| {
        let (tx, rx) = mpsc::channel();
        // The audio thread outlives every call; it ends when the process exits
        // and the last Sender drops.
        std::thread::spawn(move || music_thread(rx));
        tx
    })
}

fn music_thread(rx: Receiver<Cmd>) {
    let (_stream, handle) = match rodio::OutputStream::try_default() {
        Ok(s) => s,
        Err(e) => {
            log(&format!("no audio output device: {e}"));
            return; // drain so senders never block, but produce no sound
        }
    };
    let mut sink: Option<rodio::Sink> = None;
    let mut volume = 1.0f32;
    // The track we're currently on (Some even if its file was missing). CE calls
    // the play function every frame expecting its internal "already playing" guard
    // to no-op; our detour bypasses that guard, so we dedupe here - otherwise every
    // call would restart the track and it never actually plays (silence + log spam).
    let mut current: Option<u32> = None;

    while let Ok(cmd) = rx.recv() {
        match cmd {
            Cmd::Volume(v) => {
                volume = v.clamp(0.0, 1.0);
                if let Some(s) = &sink {
                    s.set_volume(volume);
                }
            }
            Cmd::Stop => {
                current = None;
                if let Some(s) = sink.take() {
                    s.stop();
                }
            }
            Cmd::Play(track) => {
                if current == Some(track) {
                    continue; // already on this track - ignore the per-frame re-ask
                }
                current = Some(track);
                if let Some(s) = sink.take() {
                    s.stop();
                }
                let path = format!(r"music\track{track:02}.ogg");
                match open_track(&path) {
                    Ok(source) => match rodio::Sink::try_new(&handle) {
                        Ok(s) => {
                            s.set_volume(volume);
                            s.append(source.repeat_infinite()); // loop forever
                            s.play();
                            sink = Some(s);
                            log(&format!("playing {path}"));
                        }
                        Err(e) => log(&format!("sink error for {path}: {e}")),
                    },
                    // Missing or undecodable file -> stay silent (already stopped above).
                    Err(e) => log(&format!("skip {path}: {e}")),
                }
            }
        }
    }
}

fn open_track(path: &str) -> Result<rodio::Decoder<BufReader<File>>, String> {
    let file = File::open(path).map_err(|e| e.to_string())?;
    rodio::Decoder::new(BufReader::new(file)).map_err(|e| e.to_string())
}

/// Run an export body, swallowing any panic so it never unwinds across the FFI
/// boundary into ce.exe (which would be undefined behaviour).
fn guard(f: impl FnOnce() + std::panic::UnwindSafe) {
    let _ = std::panic::catch_unwind(f);
}

/// Play `<gamedir>\music\trackNN.ogg` on a loop (NN = `track`, zero-padded to 2),
/// replacing any current track. A missing file plays nothing.
#[no_mangle]
pub extern "C" fn cemusic_play(track: u32) {
    guard(move || {
        let _ = sender().send(Cmd::Play(track));
    });
}

/// Stop and release any current track.
#[no_mangle]
pub extern "C" fn cemusic_stop() {
    guard(|| {
        let _ = sender().send(Cmd::Stop);
    });
}

/// Set music volume (0.0..1.0 software gain); persists across track changes.
#[no_mangle]
pub extern "C" fn cemusic_volume(volume: f32) {
    guard(move || {
        let _ = sender().send(Cmd::Volume(volume));
    });
}
