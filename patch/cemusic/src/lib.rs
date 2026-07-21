//! cemusic.dll - file-based background music for Codename Eagle.
//!
//! ce-patch detours the engine's CD-music functions (FUN_00483020 play /
//! FUN_00483170 stop / FUN_00486b10 volume) into these three exports, so music
//! comes from `<gamedir>\music\` (patent-free Ogg Vorbis) instead of CD Redbook
//! audio. See ../README.md.
//!
//! Track resolution prefers a named file over the numbered `music\trackNN.ogg`.
//! Single-player campaign tracks (2-13) use a hardcoded title table (see
//! [`SP_TRACK_TITLES`]), since the CD's music order doesn't line up with
//! `levels.nfo`'s mission order for the whole campaign. Everything else
//! (multiplayer) looks up the level name in `levels.nfo` by `Val:` number
//! (the CD track number minus one), so track 129 (level `Val:128`, "No mans
//! land") first tries `music\No mans land.ogg`. See [`resolve_track`].
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
                let (path, opened) = resolve_track(track);
                match opened {
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
                    // Track 0 (menu music) has no level and is legitimately absent from
                    // most installs, so don't spam the log for it.
                    Err(e) if track != 0 => log(&format!("skip {path}: {e}")),
                    Err(_) => {}
                }
            }
        }
    }
}

fn open_track(path: &str) -> Result<rodio::Decoder<BufReader<File>>, String> {
    let file = File::open(path).map_err(|e| e.to_string())?;
    rodio::Decoder::new(BufReader::new(file)).map_err(|e| e.to_string())
}

/// Single-player campaign CD tracks 2-13, in CD order (index 0 = track 2).
/// The original disc's music track order doesn't consistently match the
/// mission order in `levels.nfo` from track 8 on (e.g. mission "Betrayal" -
/// `levels.nfo` level `Val:9` - actually plays the CD track titled "Wild
/// goose chase"), so these titles are hardcoded here to mirror `ripmusic`'s
/// own `TRACK_TITLES` rather than derived from `levels.nfo`.
const SP_TRACK_TITLES: [&str; 12] = [
    "The village fool",
    "Ghost rockets",
    "The assassin",
    "The dam",
    "A train to catch",
    "Demolition man",
    "A daring rescue",
    "Dooms day device",
    "Wild goose chase",
    "Internal conflict",
    "Into the eagle's nest",
    "Eagle's flight",
];

/// Resolve a CD track to a playable file, preferring a named file over the
/// numbered `music\trackNN.ogg` - letting players drop in per-track music
/// without renumbering anything.
///
/// Tracks 2-13 (the single-player campaign) use [`SP_TRACK_TITLES`] directly,
/// since their CD order doesn't line up with `levels.nfo`. Everything else
/// (multiplayer, which added its levels later with CD track = `levels.nfo`
/// `Val:` + 1, e.g. level `Val:128` / "No mans land" asks for track 129)
/// looks up the level name in `levels.nfo` instead. Falls back to
/// `trackNN.ogg` if there's no matching name, or no file, at that name.
fn resolve_track(track: u32) -> (String, Result<rodio::Decoder<BufReader<File>>, String>) {
    let named = match sp_track_title(track) {
        Some(title) => Some(title.to_string()),
        None => track.checked_sub(1).and_then(level_name),
    };
    if let Some(name) = named {
        let named_path = format!(r"music\{name}.ogg");
        if let Ok(source) = open_track(&named_path) {
            return (named_path, Ok(source));
        }
    }
    let path = format!(r"music\track{track:02}.ogg");
    let opened = open_track(&path);
    (path, opened)
}

/// The single-player campaign title for CD track `track` (2-13), if any.
fn sp_track_title(track: u32) -> Option<&'static str> {
    let idx = track.checked_sub(2)?;
    SP_TRACK_TITLES.get(idx as usize).copied()
}

/// Look up a level's display name in `levels.nfo` (`Name:<name> Val:<num>` per
/// line, one entry per level) by its `Val:` number.
fn level_name(val: u32) -> Option<String> {
    let content = std::fs::read_to_string("levels.nfo").ok()?;
    content.lines().find_map(|line| {
        let rest = line.trim().strip_prefix("Name:")?;
        let (name, num) = rest.split_once("Val:")?;
        (num.trim().parse::<u32>().ok()? == val).then(|| name.trim().to_string())
    })
}

/// Run an export body, swallowing any panic so it never unwinds across the FFI
/// boundary into ce.exe (which would be undefined behaviour).
fn guard(f: impl FnOnce() + std::panic::UnwindSafe) {
    let _ = std::panic::catch_unwind(f);
}

/// Play the resolved music for `track` on a loop, replacing any current
/// track. See [`resolve_track`] for how the file is chosen; falls back to
/// `<gamedir>\music\trackNN.ogg` (NN = `track`, zero-padded to 2). A missing
/// file plays nothing.
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
