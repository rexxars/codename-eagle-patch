//! ripmusic - rip the Codename Eagle CD's audio tracks to `<out>\music\` (Ogg
//! Vorbis, loudness-normalized) for use with the `cemusic.dll` in-game music
//! patch.
//!
//! Usage: ripmusic [OUT_DIR] [--drive X | --image FILE]
//!   OUT_DIR     where to create the `music\` folder (default: current dir)
//!   --drive X   rip from physical CD drive X instead of auto-detecting
//!   --image F   rip from a disc image (.cue, or .img/.bin beside a .cue) instead
//!               of a physical drive. (A .iso has no audio - use the .cue/.img.)
//!
//! `cemusic.dll` resolves single-player campaign CD tracks (2-13) to a named
//! file using its own hardcoded title table (`SP_TRACK_TITLES` in
//! `patch/cemusic/src/lib.rs`, kept in sync with `TRACK_TITLES` below - the
//! CD's music order doesn't consistently follow the mission order in
//! `levels.nfo`), falling back to the numbered `music\trackNN.ogg`. Tracks
//! here are written straight to `music\<title>.ogg` for that same reason
//! (e.g. track02 -> "The Village Fool" -> `music\The Village Fool.ogg`), or
//! `music\trackNN.ogg` for a track past the title table.

use ripmusic::encode::encode_ogg;
use ripmusic::{cdrom, image, loudness};
use std::error::Error;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

const RATE: u32 = 44_100;

/// Soundtrack metadata, written as Vorbis comments on each ripped track.
const ARTIST: &str = "Örjan Strandberg";
const ALBUM: &str = "Codename Eagle";

/// Track titles in CD order. The data track is CD track 1, so audio starts at CD
/// track 2: index 0 here = CD track 2 = album track 1 (`TRACKNUMBER` = CD number - 1).
const TRACK_TITLES: [&str; 13] = [
    "The Village Fool",
    "Ghost Rockets",
    "The Assassin",
    "The Dam",
    "A Train To Catch",
    "Demolition Man",
    "A Daring Rescue",
    "Dooms Day Device",
    "Wild Goose Chase",
    "Internal Conflict",
    "Into The Eagle's Nest",
    "Eagle's Flight",
    "Outro",
];

fn main() -> ExitCode {
    let code = match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            // `Reported` errors have already printed their own message.
            let msg = e.to_string();
            if !msg.is_empty() {
                eprintln!("\nerror: {msg}");
            }
            ExitCode::FAILURE
        }
    };
    wait_for_keypress(); // so double-click users can read the output
    code
}

/// Where to read tracks from: a physical CD drive or a disc image file.
enum Source {
    Cd(cdrom::Cd),
    Image(PathBuf),
}

impl Source {
    fn read(&self, t: &cdrom::Track) -> std::io::Result<Vec<i16>> {
        match self {
            Source::Cd(cd) => cdrom::read_audio(cd, t.start_lba, t.end_lba),
            Source::Image(img) => image::read_audio(img, t.start_lba, t.end_lba),
        }
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let mut out_dir = PathBuf::from(".");
    let mut forced_drive: Option<char> = None;
    let mut image_path: Option<PathBuf> = None;
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--drive" => {
                forced_drive =
                    args.next().and_then(|s| s.chars().next()).map(|c| c.to_ascii_uppercase());
            }
            "--image" => {
                image_path = Some(PathBuf::from(args.next().ok_or("--image needs a file path")?));
            }
            "-h" | "--help" => {
                print_help();
                return Ok(());
            }
            _ => out_dir = PathBuf::from(a),
        }
    }

    // The third element is the CD drive letter when reading from a physical disc,
    // so an empty-audio error can name it; `None` for disc images.
    let (source, tracks, cd_drive) = match image_path {
        Some(p) => {
            let ci = open_image(&p)?;
            println!("Reading Codename Eagle image {}", ci.img.display());
            (Source::Image(ci.img), ci.tracks, None)
        }
        None => {
            let (drive, detected) = match forced_drive {
                Some(d) => (d, false),
                None => {
                    let drives = cdrom::cdrom_drives();
                    if drives.is_empty() {
                        return Err(
                            "no cd/dvd drive found, specify --image <path> to use an image".into(),
                        );
                    }
                    println!("Looking for Codename Eagle CD...");
                    match find_ce_cd(&drives) {
                        Some(d) => (d, true),
                        None => {
                            println!(
                                "No Codename Eagle CD found (looked for codename.ico / cutscn on \
                                 the CD/DVD drives). Insert the CD, pass --drive X, or rip an \
                                 image with --image.\n"
                            );
                            print_help();
                            return Err(Box::new(Reported));
                        }
                    }
                }
            };
            if detected {
                println!("Codename Eagle CD data found in {drive}:");
            } else {
                println!("Reading Codename Eagle CD in drive {drive}:");
            }
            let cd = cdrom::open(drive).map_err(|e| format!("cannot open drive {drive}: {e}"))?;
            let tracks =
                cdrom::read_toc(&cd).map_err(|e| format!("cannot read CD table of contents: {e}"))?;
            (Source::Cd(cd), tracks, Some(drive))
        }
    };

    let audio: Vec<_> = tracks.into_iter().filter(|t| t.is_audio).collect();
    if audio.is_empty() {
        match cd_drive {
            Some(d) => {
                eprintln!(
                    "Error: No audio tracks found on {d}: (note: iso files do not have audio tracks)\n"
                );
                print_help();
                return Err(Box::new(Reported));
            }
            None => return Err("no audio tracks found (an .iso / data-only disc has none)".into()),
        }
    }

    let music_dir = out_dir.join("music");
    std::fs::create_dir_all(&music_dir)?;
    println!("Ripping {} audio tracks to {}", audio.len(), music_dir.display());

    for t in &audio {
        // CD track 1 is data, so album track number = CD number - 1, and the title
        // table is indexed from CD track 2 (index 0). Untitled if beyond the table.
        let track_no = t.number.saturating_sub(1).to_string();
        let title = (t.number as usize).checked_sub(2).and_then(|i| TRACK_TITLES.get(i)).copied();
        let path = match title {
            Some(title) => music_dir.join(format!("{title}.ogg")),
            None => music_dir.join(format!("track{:02}.ogg", t.number)),
        };
        print!("  track {:02} ... ", t.number);
        let pcm = source.read(t).map_err(|e| format!("read track {} failed: {e}", t.number))?;
        let channels = loudness::normalize_stereo(&pcm, RATE);

        let mut tags = vec![("ARTIST", ARTIST), ("ALBUM", ALBUM), ("TRACKNUMBER", track_no.as_str())];
        if let Some(title) = title {
            tags.push(("TITLE", title));
        }
        encode_ogg(&channels, RATE, &path, &tags)?;

        let secs = channels[0].len() as f32 / RATE as f32;
        let name = path.file_name().unwrap().to_string_lossy();
        match title {
            Some(title) => println!("{secs:.0}s -> {name}  ({title})"),
            None => println!("{secs:.0}s -> {name}"),
        }
    }

    println!("\nDone. Put the `music` folder in your Codename Eagle directory.");
    Ok(())
}

/// Resolve an `--image` argument to a parsed cue/image, with friendly errors.
fn open_image(path: &Path) -> Result<image::CueImage, Box<dyn Error>> {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
    match ext.as_str() {
        "cue" => Ok(image::parse_cue(path)?),
        "img" | "bin" => {
            let cue = path.with_extension("cue");
            if cue.exists() {
                Ok(image::parse_cue(&cue)?)
            } else {
                Err(format!("no .cue sheet beside {}; point --image at the .cue", path.display()).into())
            }
        }
        "iso" => Err(format!(
            "{} is an ISO (data track only - no CD audio). Point --image at the .cue or .img \
             from the same rip instead.",
            path.display()
        )
        .into()),
        _ => Err("unsupported image type (expected .cue, .img or .bin)".into()),
    }
}

/// The first of `drives` whose root holds a CE marker.
fn find_ce_cd(drives: &[char]) -> Option<char> {
    drives.iter().copied().find(|&d| {
        Path::new(&format!(r"{d}:\codename.ico")).exists()
            || Path::new(&format!(r"{d}:\cutscn")).exists()
    })
}

/// The version / author banner, shown at the top of the help text.
fn print_banner() {
    println!("{} v{}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
    println!("(c) Espen Hovlandsdal - https://espen.codes/");
    println!("Codename Eagle Nation - https://codenameeagle.net/");
}

/// Print the `--help` / usage text (also shown when no CE CD is found).
fn print_help() {
    print_banner();
    println!();
    println!(
        "usage: ripmusic [OUT_DIR] [--drive X | --image FILE]\n\
         \n\
         Rips the Codename Eagle CD's audio tracks to <OUT_DIR>\\music\\ (Ogg Vorbis,\n\
         loudness-normalized) for the cemusic.dll in-game music patch. Named after\n\
         each track's title (e.g. track02.ogg -> \"The Village Fool.ogg\"), else\n\
         trackNN.ogg for tracks with no title.\n\
         \n\
         Options:\n\
         \x20 OUT_DIR       where to create the `music` folder (default: current dir)\n\
         \x20 --drive X     rip from CD/DVD drive X instead of auto-detecting\n\
         \x20 --image FILE  rip from a disc image (.cue, or .img/.bin beside a .cue)\n\
         \x20               instead of a physical drive. A .iso has no audio - use the\n\
         \x20               .cue/.img.\n\
         \x20 -h, --help    show this help"
    );
}

/// An error whose message has already been printed, so `main` won't print it again.
#[derive(Debug)]
struct Reported;
impl std::fmt::Display for Reported {
    fn fmt(&self, _f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Ok(())
    }
}
impl Error for Reported {}

/// Pause if we own the console (i.e. were double-clicked), so the window doesn't
/// vanish before the user reads the result. No-op when launched from a shell.
fn wait_for_keypress() {
    #[cfg(windows)]
    {
        use std::io::{Read, Write};
        use windows_sys::Win32::System::Console::GetConsoleProcessList;
        let mut buf = [0u32; 2];
        let count = unsafe { GetConsoleProcessList(buf.as_mut_ptr(), buf.len() as u32) };
        if count <= 1 {
            print!("\nPress Enter to exit...");
            let _ = std::io::stdout().flush();
            let _ = std::io::stdin().read(&mut [0u8; 1]);
        }
    }
}
