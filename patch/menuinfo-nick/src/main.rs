//! `menuinfo-nick <path-to-menuinfo.dat> <nickname>`
//!
//! Sets the Codename Eagle multiplayer player name inside `menuinfo.dat`. The
//! demo installer runs this once, post-copy, with the name the player typed.
//! The nickname is normalized here to what the game actually renders in a
//! session (the engine X-es out spaces, `_ - . , ^ ~ `` ` `` and all
//! non-ASCII; <=10 chars survive online), so an empty or junk argument still
//! yields a valid profile.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use menuinfo_nick::{patch_file, sanitize_nickname};

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let (path, raw_name) = match (args.next(), args.next()) {
        (Some(p), Some(n)) => (p, n),
        _ => {
            eprintln!("usage: menuinfo-nick <path-to-menuinfo.dat> <nickname>");
            return ExitCode::FAILURE;
        }
    };

    match run(Path::new(&path), &raw_name) {
        Ok(name) => {
            println!("set multiplayer name to \"{name}\"");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("menuinfo-nick: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run(path: &Path, raw_name: &str) -> Result<String, String> {
    let name = sanitize_nickname(raw_name);
    let file = std::fs::read(path).map_err(|e| format!("reading {}: {e}", path.display()))?;
    let patched = patch_file(&file, &name)?;
    write_atomic(path, &patched)?;
    Ok(name)
}

/// Write `data` to `path` via a sibling temp file + rename, so a crash mid-write
/// can't leave a half-written (and thus unreadable) profile.
fn write_atomic(path: &Path, data: &[u8]) -> Result<(), String> {
    let tmp = temp_sibling(path);
    std::fs::write(&tmp, data).map_err(|e| format!("writing {}: {e}", tmp.display()))?;
    std::fs::rename(&tmp, path).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        format!("renaming {} -> {}: {e}", tmp.display(), path.display())
    })
}

fn temp_sibling(path: &Path) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(".new");
    path.with_file_name(name)
}
