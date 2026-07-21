//! `textool` — add/replace TGA textures inside Codename Eagle texture archives
//! (`24bits/textures.dat`, `24bits/texsec.dat`). The patch installer runs this
//! at install time via `nsExec::ExecToLog`, so the exit code is the only
//! failure signal it checks; stdout/stderr just end up in the install log.
//!
//! ```text
//! textool set <archive.dat> <texture.tga>...   upsert TGAs (entry name = basename)
//! textool list <archive.dat>                   NAME  WxHxDEPTH  <blob bytes>
//! ```
//!
//! Exit codes: 0 ok, 1 runtime error (the archive is never touched), 2 usage
//! error.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use textool::{build_archive, parse_archive, upsert, validate_tga, UpsertOutcome};

const USAGE: &str =
    "usage: textool set <archive.dat> <texture.tga>...\n       textool list <archive.dat>";

fn main() -> ExitCode {
    // args_os + into_string: a non-Unicode argument is a graceful usage error
    // (exit 2) instead of the panic std::env::args() would raise — an abort
    // under the release profile's panic="abort".
    let Ok(args) = std::env::args_os()
        .skip(1)
        .map(|a| a.into_string())
        .collect::<Result<Vec<String>, _>>()
    else {
        eprintln!("textool: arguments must be valid Unicode\n{USAGE}");
        return ExitCode::from(2);
    };
    let result = match args.split_first() {
        Some((cmd, rest)) => match (cmd.as_str(), rest) {
            ("set", [archive, tgas @ ..]) if !tgas.is_empty() => cmd_set(Path::new(archive), tgas),
            ("list", [archive]) => cmd_list(Path::new(archive)),
            _ => {
                eprintln!("{USAGE}");
                return ExitCode::from(2);
            }
        },
        None => {
            eprintln!("{USAGE}");
            return ExitCode::from(2);
        }
    };
    match result {
        Ok(lines) => {
            for line in lines {
                println!("{line}");
            }
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("textool: {e}");
            ExitCode::FAILURE
        }
    }
}

/// `set`: validate and upsert every staged TGA in memory first, then rewrite
/// the archive with ONE atomic write — on any error nothing is written.
/// Returns the report lines (`replaced NAME (WxHxDEPTH)` / `added ...`) in
/// argument order, printed only after the write succeeded.
fn cmd_set(archive_path: &Path, tga_paths: &[String]) -> Result<Vec<String>, String> {
    let raw = std::fs::read(archive_path)
        .map_err(|e| format!("reading archive {}: {e}", archive_path.display()))?;
    let mut entries =
        parse_archive(&raw).map_err(|e| format!("archive {}: {e}", archive_path.display()))?;

    let mut lines = Vec::with_capacity(tga_paths.len());
    for tga_path in tga_paths {
        let tga_path = Path::new(tga_path);
        let name = ascii_basename(tga_path)?;
        let tga =
            std::fs::read(tga_path).map_err(|e| format!("reading {}: {e}", tga_path.display()))?;
        let info = validate_tga(&tga).map_err(|e| format!("{}: {e}", tga_path.display()))?;
        let outcome = upsert(&mut entries, &name, &tga)
            .map_err(|e| format!("{}: {e}", tga_path.display()))?;
        let verb = match outcome {
            UpsertOutcome::Replaced => "replaced",
            UpsertOutcome::Added => "added",
        };
        lines.push(format!(
            "{verb} {name} ({}x{}x{})",
            info.width, info.height, info.depth
        ));
    }

    let rebuilt = build_archive(&entries)
        .map_err(|e| format!("rebuilding {}: {e}", archive_path.display()))?;
    write_atomic(archive_path, &rebuilt)?;
    Ok(lines)
}

/// `list`: one line per entry, `NAME  WxHxDEPTH  <blob bytes>`, with `raw` in
/// place of the dimensions when the blob doesn't carry a plausible
/// prefix-stripped TGA header.
fn cmd_list(archive_path: &Path) -> Result<Vec<String>, String> {
    let raw = std::fs::read(archive_path)
        .map_err(|e| format!("reading archive {}: {e}", archive_path.display()))?;
    let entries =
        parse_archive(&raw).map_err(|e| format!("archive {}: {e}", archive_path.display()))?;
    Ok(entries
        .iter()
        .map(|e| {
            let name = String::from_utf8_lossy(&e.name);
            let dims = match blob_dims(&e.blob) {
                Some((w, h, d)) => format!("{w}x{h}x{d}"),
                None => "raw".to_string(),
            };
            format!("{name}  {dims}  {}", e.blob.len())
        })
        .collect())
}

/// Read WxHxDEPTH out of a stored blob's 10-byte prefix-stripped TGA header
/// (u16LE width @4, u16LE height @6, u8 depth @8 — TGA header bytes 12/14/16
/// shifted down by the stripped 8-byte prefix). Only trusted when the blob
/// length is exactly `10 + w*h*(depth/8)`; anything else is `None` (raw).
fn blob_dims(blob: &[u8]) -> Option<(u16, u16, u8)> {
    if blob.len() < 10 {
        return None;
    }
    let w = u16::from_le_bytes([blob[4], blob[5]]);
    let h = u16::from_le_bytes([blob[6], blob[7]]);
    let depth = blob[8];
    if depth == 0 || !depth.is_multiple_of(8) {
        return None;
    }
    let expected = 10 + w as usize * h as usize * (depth / 8) as usize;
    (blob.len() == expected).then_some((w, h, depth))
}

/// The staged file's basename, required to be plain ASCII: archive names are
/// stored as Latin-1 bytes and matched byte-wise (see [`textool::upsert`]),
/// and every real texture name is ASCII — anything else is outside the
/// supported envelope.
fn ascii_basename(path: &Path) -> Result<String, String> {
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| format!("{}: cannot determine the file's basename", path.display()))?;
    if !name.is_ascii() {
        return Err(format!(
            "{}: non-ASCII basename \"{name}\" is not supported (archive texture names are ASCII)",
            path.display()
        ));
    }
    Ok(name.to_string())
}

/// Rewrite `path` atomically and durably: write the new bytes to a sibling
/// `.tmp` file in the same directory, fsync it, then rename over the
/// original — a crash or power loss can never leave a half-written archive
/// (without the fsync the OS could commit the rename before the data reaches
/// disk). The temp file is removed on failure.
fn write_atomic(path: &Path, data: &[u8]) -> Result<(), String> {
    let tmp = tmp_sibling(path);
    write_synced(&tmp, data).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        format!("writing {}: {e}", tmp.display())
    })?;
    std::fs::rename(&tmp, path).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        format!("renaming {} -> {}: {e}", tmp.display(), path.display())
    })
}

/// `fs::write` plus `sync_all`: create, write everything, and flush file data
/// and metadata to disk before the file is closed.
fn write_synced(path: &Path, data: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    let mut file = std::fs::File::create(path)?;
    file.write_all(data)?;
    file.sync_all()
}

fn tmp_sibling(path: &Path) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(".tmp");
    path.with_file_name(name)
}
