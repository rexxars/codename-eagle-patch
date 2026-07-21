//! Integration tests for the `textool` CLI: run the built binary against
//! fixture archives in per-test temp dirs.
//!
//! Contract under test:
//!   textool set <archive.dat> <texture.tga>...  -> upsert, ONE atomic write
//!   textool list <archive.dat>                  -> NAME  WxHxDEPTH  <blob bytes>
//! Exit codes: 0 ok, 1 runtime error (nothing written), 2 usage error.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use textool::{build_archive, Entry, TGA_HEADER_LEN, TGA_PREFIX};

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_textool")
}

/// Fresh per-test temp dir; the tag keeps parallel tests from colliding.
fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("textool_cli_{tag}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Same construction as the lib unit tests: valid uncompressed true-color TGA
/// (18-byte header, image_type 2, zeroed pixels).
fn make_tga(w: u16, h: u16, depth: u8) -> Vec<u8> {
    let mut tga = vec![0u8; TGA_HEADER_LEN];
    tga[2] = 2;
    tga[12..14].copy_from_slice(&w.to_le_bytes());
    tga[14..16].copy_from_slice(&h.to_le_bytes());
    tga[16] = depth;
    tga[17] = if depth == 32 { 8 } else { 0 };
    tga.extend(std::iter::repeat_n(
        0u8,
        w as usize * h as usize * (depth / 8) as usize,
    ));
    tga
}

fn entry(name: &str, tga: &[u8]) -> Entry {
    Entry {
        name: name.as_bytes().to_vec(),
        blob: tga[TGA_PREFIX.len()..].to_vec(),
    }
}

/// Two-entry fixture archive: TARGET.tga (16x16x24, 778-byte blob) then
/// other.tga (8x8x32, 266-byte blob).
fn write_fixture(dir: &Path) -> PathBuf {
    let raw = build_archive(&[
        entry("TARGET.tga", &make_tga(16, 16, 24)),
        entry("other.tga", &make_tga(8, 8, 32)),
    ])
    .unwrap();
    let path = dir.join("texsec.dat");
    std::fs::write(&path, raw).unwrap();
    path
}

fn run(args: &[&str]) -> Output {
    Command::new(bin()).args(args).output().unwrap()
}

fn stdout(out: &Output) -> String {
    String::from_utf8(out.stdout.clone()).unwrap()
}

fn stderr(out: &Output) -> String {
    String::from_utf8(out.stderr.clone()).unwrap()
}

fn list_stdout(archive: &Path) -> String {
    let out = run(&["list", archive.to_str().unwrap()]);
    assert!(out.status.success(), "list failed: {}", stderr(&out));
    stdout(&out)
}

fn assert_no_tmp_leftover(dir: &Path) {
    let leftovers: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|n| n.ends_with(".tmp"))
        .collect();
    assert!(
        leftovers.is_empty(),
        "temp files left behind: {leftovers:?}"
    );
}

// -- set ---------------------------------------------------------------------

#[test]
fn set_replaces_existing_entry() {
    let dir = temp_dir("replace");
    let archive = write_fixture(&dir);
    let tga_path = dir.join("TARGET.tga");
    std::fs::write(&tga_path, make_tga(32, 32, 32)).unwrap();

    let out = run(&["set", archive.to_str().unwrap(), tga_path.to_str().unwrap()]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    assert_eq!(stdout(&out), "replaced TARGET.tga (32x32x32)\n");

    let expected = format!(
        "TARGET.tga  32x32x32  {}\nother.tga  8x8x32  266\n",
        10 + 32 * 32 * 4
    );
    assert_eq!(list_stdout(&archive), expected);
    assert_no_tmp_leftover(&dir);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn set_non_square_needs_allow_any_flag() {
    let dir = temp_dir("allowany");
    let archive = write_fixture(&dir);
    // 80x144 like the menufont: refused by default, accepted with --allow-any.
    let tga_path = dir.join("menufont.tga");
    std::fs::write(&tga_path, make_tga(80, 144, 24)).unwrap();

    let refused = run(&["set", archive.to_str().unwrap(), tga_path.to_str().unwrap()]);
    assert_eq!(refused.status.code(), Some(1), "stderr: {}", stderr(&refused));
    assert!(stderr(&refused).contains("expected square"));
    // Nothing written: the fixture still has exactly its two entries.
    assert_eq!(list_stdout(&archive).lines().count(), 2);

    let ok = run(&[
        "set",
        "--allow-any",
        archive.to_str().unwrap(),
        tga_path.to_str().unwrap(),
    ]);
    assert!(ok.status.success(), "stderr: {}", stderr(&ok));
    assert_eq!(stdout(&ok), "added menufont.tga (80x144x24)\n");
    let lines: Vec<String> = list_stdout(&archive).lines().map(String::from).collect();
    assert_eq!(lines.len(), 3);
    assert_eq!(lines[2], format!("menufont.tga  80x144x24  {}", 10 + 80 * 144 * 3));
    assert_no_tmp_leftover(&dir);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn set_rejects_unknown_flag() {
    let dir = temp_dir("badflag");
    let archive = write_fixture(&dir);
    let tga_path = dir.join("x.tga");
    std::fs::write(&tga_path, make_tga(8, 8, 24)).unwrap();
    let out = run(&[
        "set",
        "--nope",
        archive.to_str().unwrap(),
        tga_path.to_str().unwrap(),
    ]);
    assert_eq!(out.status.code(), Some(2), "stderr: {}", stderr(&out));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn set_adds_new_entry_last() {
    let dir = temp_dir("add");
    let archive = write_fixture(&dir);
    let tga_path = dir.join("NEW.tga");
    std::fs::write(&tga_path, make_tga(8, 8, 24)).unwrap();

    let out = run(&["set", archive.to_str().unwrap(), tga_path.to_str().unwrap()]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    assert_eq!(stdout(&out), "added NEW.tga (8x8x24)\n");

    let list = list_stdout(&archive);
    let lines: Vec<&str> = list.lines().collect();
    assert_eq!(lines.len(), 3, "added entry must appear: {list}");
    assert_eq!(lines[2], format!("NEW.tga  8x8x24  {}", 10 + 8 * 8 * 3));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn set_handles_multiple_files_in_argument_order() {
    let dir = temp_dir("multi");
    let archive = write_fixture(&dir);
    let replace = dir.join("TARGET.tga");
    std::fs::write(&replace, make_tga(32, 32, 24)).unwrap();
    let add = dir.join("extra.tga");
    std::fs::write(&add, make_tga(4, 4, 32)).unwrap();

    let out = run(&[
        "set",
        archive.to_str().unwrap(),
        replace.to_str().unwrap(),
        add.to_str().unwrap(),
    ]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    assert_eq!(
        stdout(&out),
        "replaced TARGET.tga (32x32x24)\nadded extra.tga (4x4x32)\n"
    );

    let expected = format!(
        "TARGET.tga  32x32x24  {}\nother.tga  8x8x32  266\nextra.tga  4x4x32  {}\n",
        10 + 32 * 32 * 3,
        10 + 4 * 4 * 4
    );
    assert_eq!(list_stdout(&archive), expected);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn set_replaces_case_insensitively_keeping_stored_name() {
    let dir = temp_dir("case");
    let archive = write_fixture(&dir);
    // The archive stores TARGET.tga; we stage Target.tga.
    let tga_path = dir.join("Target.tga");
    std::fs::write(&tga_path, make_tga(16, 16, 32)).unwrap();

    let out = run(&["set", archive.to_str().unwrap(), tga_path.to_str().unwrap()]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    assert_eq!(stdout(&out), "replaced Target.tga (16x16x32)\n");

    let list = list_stdout(&archive);
    assert!(
        list.starts_with("TARGET.tga  16x16x32  "),
        "stored name must stay TARGET.tga: {list}"
    );
    assert!(
        !list.contains("Target.tga"),
        "staged casing must not leak into the archive: {list}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn set_errors_when_archive_missing() {
    let dir = temp_dir("missing");
    let tga_path = dir.join("TARGET.tga");
    std::fs::write(&tga_path, make_tga(8, 8, 24)).unwrap();
    let archive = dir.join("nope.dat");

    let out = run(&["set", archive.to_str().unwrap(), tga_path.to_str().unwrap()]);
    assert_eq!(out.status.code(), Some(1));
    assert!(
        stderr(&out).contains("nope.dat"),
        "stderr must name the archive: {}",
        stderr(&out)
    );
    assert!(!archive.exists(), "must not create the missing archive");
    assert_no_tmp_leftover(&dir);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn set_with_one_bad_tga_leaves_archive_untouched() {
    let dir = temp_dir("badtga");
    let archive = write_fixture(&dir);
    let before = std::fs::read(&archive).unwrap();

    let good = dir.join("TARGET.tga");
    std::fs::write(&good, make_tga(32, 32, 24)).unwrap();
    let bad = dir.join("broken.tga");
    let mut tga = make_tga(8, 8, 24);
    tga.pop(); // truncated pixel data -> validate_tga refuses it
    std::fs::write(&bad, tga).unwrap();

    let out = run(&[
        "set",
        archive.to_str().unwrap(),
        good.to_str().unwrap(),
        bad.to_str().unwrap(),
    ]);
    assert_eq!(out.status.code(), Some(1));
    assert!(
        stderr(&out).contains("broken.tga"),
        "stderr must name the bad file: {}",
        stderr(&out)
    );
    let after = std::fs::read(&archive).unwrap();
    assert!(
        after == before,
        "archive must be byte-identical after a failed set"
    );
    assert_no_tmp_leftover(&dir);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn set_rejects_non_ascii_basename() {
    let dir = temp_dir("nonascii");
    let archive = write_fixture(&dir);
    let before = std::fs::read(&archive).unwrap();
    let tga_path = dir.join("sm\u{f6}rg\u{e5}s.tga");
    std::fs::write(&tga_path, make_tga(8, 8, 24)).unwrap();

    let out = run(&["set", archive.to_str().unwrap(), tga_path.to_str().unwrap()]);
    assert_eq!(out.status.code(), Some(1));
    assert!(
        stderr(&out).contains("non-ASCII"),
        "stderr must explain the rejection: {}",
        stderr(&out)
    );
    let after = std::fs::read(&archive).unwrap();
    assert!(
        after == before,
        "archive must be byte-identical after a failed set"
    );
    assert_no_tmp_leftover(&dir);
    let _ = std::fs::remove_dir_all(&dir);
}

// -- list ----------------------------------------------------------------------

#[test]
fn list_prints_expected_lines_for_known_fixture() {
    let dir = temp_dir("list");
    let archive = write_fixture(&dir);
    let out = run(&["list", archive.to_str().unwrap()]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    assert_eq!(
        stdout(&out),
        "TARGET.tga  16x16x24  778\nother.tga  8x8x32  266\n"
    );
    assert!(stderr(&out).is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn list_prints_raw_for_undecodable_blob() {
    let dir = temp_dir("listraw");
    // 12 bytes of 0xAB: the implied 10-byte header doesn't match the length.
    let raw = build_archive(&[Entry {
        name: b"junk.tga".to_vec(),
        blob: vec![0xAB; 12],
    }])
    .unwrap();
    let archive = dir.join("texsec.dat");
    std::fs::write(&archive, raw).unwrap();

    assert_eq!(list_stdout(&archive), "junk.tga  raw  12\n");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn list_errors_when_archive_missing() {
    let dir = temp_dir("listmissing");
    let archive = dir.join("nope.dat");
    let out = run(&["list", archive.to_str().unwrap()]);
    assert_eq!(out.status.code(), Some(1));
    assert!(stderr(&out).contains("nope.dat"));
    let _ = std::fs::remove_dir_all(&dir);
}

// -- usage ---------------------------------------------------------------------

#[test]
fn no_args_exits_2_with_usage() {
    let out = run(&[]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("usage"), "stderr: {}", stderr(&out));
    assert!(stdout(&out).is_empty());
}

#[test]
fn unknown_subcommand_exits_2() {
    let out = run(&["frobnicate", "texsec.dat"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("usage"), "stderr: {}", stderr(&out));
}

#[test]
fn set_without_texture_args_exits_2() {
    let out = run(&["set", "texsec.dat"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("usage"), "stderr: {}", stderr(&out));
}

#[cfg(unix)]
#[test]
fn non_unicode_argument_exits_2() {
    use std::os::unix::ffi::OsStrExt;
    let out = Command::new(bin())
        .arg("set")
        .arg(std::ffi::OsStr::from_bytes(b"tex\xffsec.dat"))
        .arg("x.tga")
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2));
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("usage"),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn list_with_extra_args_exits_2() {
    let out = run(&["list", "texsec.dat", "extra.dat"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("usage"), "stderr: {}", stderr(&out));
}
