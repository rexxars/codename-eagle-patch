use menuinfo_nick::{decode, DEFAULT_NAME};
use std::process::Command;

const FIXTURE: &[u8] = include_bytes!("fixtures/menuinfo.dat");

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_menuinfo-nick")
}

fn name_at(plain: &[u8], off: usize) -> String {
    let field = &plain[off..off + 20];
    let end = field.iter().position(|&b| b == 0).unwrap_or(field.len());
    String::from_utf8_lossy(&field[..end]).into_owned()
}

// Write the fixture to a unique temp path, patch it via the CLI, return the
// decoded PlayInfo name (block +0x42). tmp_dir is per-test to avoid collisions.
fn patch_via_cli(tmp: &std::path::Path, arg: &str) -> (std::process::ExitStatus, String) {
    std::fs::write(tmp, FIXTURE).unwrap();
    let status = Command::new(bin()).arg(tmp).arg(arg).status().unwrap();
    let plain = decode(&std::fs::read(tmp).unwrap()).expect("decode patched");
    (status, name_at(&plain, 0x42))
}

#[test]
fn cli_sets_the_name() {
    let tmp = std::env::temp_dir().join("mni_cli_sets.dat");
    let (status, name) = patch_via_cli(&tmp, "Falcon");
    assert!(status.success());
    assert_eq!(name, "Falcon");
    let _ = std::fs::remove_file(&tmp);
}

#[test]
fn cli_truncates_to_ten() {
    let tmp = std::env::temp_dir().join("mni_cli_trunc.dat");
    let (status, name) = patch_via_cli(&tmp, "abcdefghijklmnop");
    assert!(status.success());
    assert_eq!(name, "abcdefghij");
    let _ = std::fs::remove_file(&tmp);
}

#[test]
fn cli_empty_name_falls_back_to_default() {
    let tmp = std::env::temp_dir().join("mni_cli_empty.dat");
    let (status, name) = patch_via_cli(&tmp, "");
    assert!(status.success());
    assert_eq!(name, DEFAULT_NAME);
    let _ = std::fs::remove_file(&tmp);
}

#[test]
fn cli_errors_without_enough_args() {
    let status = Command::new(bin()).status().unwrap();
    assert!(!status.success());
}

#[test]
fn cli_errors_on_missing_file() {
    let status = Command::new(bin())
        .arg("/no/such/menuinfo.dat")
        .arg("Bob")
        .status()
        .unwrap();
    assert!(!status.success());
}
