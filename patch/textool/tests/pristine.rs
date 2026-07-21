//! Integration tests against REAL pristine 1.43 archives: prove that a `set`
//! run by the installer replaces exactly the requested entries and leaves
//! every other entry of the shipped data byte-identical. `#[ignore]`d because
//! they need a local pristine install (and the textures.dat one copies a
//! ~151 MB file):
//!
//!   cd patch/textool
//!   CE_PRISTINE_143=/path/to/pristine/1.43 \
//!   cargo test -- --ignored --test-threads=1

use std::path::{Path, PathBuf};
use std::process::Command;

use textool::{parse_archive, validate_tga, Entry, TGA_HEADER_LEN, TGA_PREFIX};

fn pristine_dir() -> PathBuf {
    let v = std::env::var("CE_PRISTINE_143").unwrap_or_else(|_| {
        panic!(
            "CE_PRISTINE_143 must be set for the pristine-install tests. Run them as:\n  \
             CE_PRISTINE_143=/path/to/pristine/1.43 cargo test -- --ignored --test-threads=1"
        )
    });
    let dir = PathBuf::from(v);
    assert!(
        dir.is_dir(),
        "CE_PRISTINE_143={} is not a directory",
        dir.display()
    );
    dir
}

fn read_pristine_archive(name: &str) -> (PathBuf, Vec<Entry>) {
    let path = pristine_dir().join("24bits").join(name);
    let raw =
        std::fs::read(&path).unwrap_or_else(|e| panic!("reading pristine {}: {e}", path.display()));
    let entries =
        parse_archive(&raw).unwrap_or_else(|e| panic!("parsing pristine {}: {e}", path.display()));
    (path, entries)
}

/// Fresh per-test temp dir; the tag keeps parallel tests from colliding.
fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("textool_pristine_{tag}_{}", std::process::id()));
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

/// Run the built `textool` binary: `set <archive> <tgas...>`, asserting success.
fn run_set(archive: &Path, tgas: &[&Path]) {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_textool"));
    cmd.arg("set").arg(archive);
    for tga in tgas {
        cmd.arg(tga);
    }
    let out = cmd.output().unwrap();
    assert!(
        out.status.success(),
        "textool set failed ({}):\nstdout: {}\nstderr: {}",
        out.status,
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
#[ignore] // needs CE_PRISTINE_143, see module comment
fn pristine_texsec_replace_preserves_others() {
    let (pristine_path, original) = read_pristine_archive("texsec.dat");
    assert_eq!(original.len(), 27, "pristine texsec.dat entry count");
    let idx = original
        .iter()
        .position(|e| e.name == b"INTERFC1.tga")
        .expect("pristine texsec.dat must contain INTERFC1.tga");

    let dir = temp_dir("texsec");
    let archive = dir.join("texsec.dat");
    std::fs::copy(&pristine_path, &archive).unwrap();
    let tga = make_tga(256, 256, 32);
    let staged = dir.join("INTERFC1.tga");
    std::fs::write(&staged, &tga).unwrap();

    run_set(&archive, &[&staged]);

    let patched = parse_archive(&std::fs::read(&archive).unwrap()).unwrap();
    assert_eq!(patched.len(), 27, "entry count must be preserved");
    for (i, (before, after)) in original.iter().zip(&patched).enumerate() {
        let name = String::from_utf8_lossy(&before.name).into_owned();
        assert_eq!(before.name, after.name, "entry {i} ({name}): name/order");
        if i == idx {
            assert!(
                after.blob == tga[TGA_PREFIX.len()..],
                "INTERFC1.tga blob must be the staged TGA minus its 8-byte prefix"
            );
        } else {
            assert!(
                before.blob == after.blob,
                "entry {i} ({name}): blob must be byte-identical to the pristine one"
            );
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
#[ignore] // needs CE_PRISTINE_143, see module comment
fn pristine_textures_replace_offsets_ok() {
    let (pristine_path, original) = read_pristine_archive("textures.dat");
    assert_eq!(original.len(), 1557, "pristine textures.dat entry count");

    let dir = temp_dir("textures");
    let archive = dir.join("textures.dat");
    std::fs::copy(&pristine_path, &archive).unwrap();

    let snipe_tga = make_tga(256, 256, 32);
    let snipe_path = dir.join("SNIPEMOD.tga");
    std::fs::write(&snipe_path, &snipe_tga).unwrap();
    // Deliberately mixed-case: the archive stores TARGET.tga, so this proves
    // case-insensitive replace against the real data.
    let target_tga = make_tga(32, 32, 32);
    let target_path = dir.join("Target.tga");
    std::fs::write(&target_path, &target_tga).unwrap();

    // ONE invocation with both files.
    run_set(&archive, &[&snipe_path, &target_path]);

    // parse_archive succeeding proves the rewritten offsets are monotonic and
    // in bounds.
    let patched = parse_archive(&std::fs::read(&archive).unwrap()).unwrap();
    assert_eq!(patched.len(), 1557, "entry count must be preserved");

    let snipe_idx = patched
        .iter()
        .position(|e| e.name.eq_ignore_ascii_case(b"SNIPEMOD.tga"))
        .expect("patched textures.dat must contain SNIPEMOD.tga");
    assert!(
        patched[snipe_idx].blob == snipe_tga[TGA_PREFIX.len()..],
        "SNIPEMOD.tga blob must be the staged TGA minus its 8-byte prefix"
    );
    let target_idx = patched
        .iter()
        .position(|e| e.name.eq_ignore_ascii_case(b"Target.tga"))
        .expect("patched textures.dat must contain TARGET.tga");
    assert_eq!(
        patched[target_idx].name, b"TARGET.tga",
        "stored name must keep the archive's original (uppercase) form"
    );
    assert!(
        patched[target_idx].blob == target_tga[TGA_PREFIX.len()..],
        "TARGET.tga blob must be the staged TGA minus its 8-byte prefix"
    );

    // Every untouched entry must survive byte-identically (a superset of the
    // required spot checks).
    let mut untouched = 0;
    for (i, (before, after)) in original.iter().zip(&patched).enumerate() {
        let name = String::from_utf8_lossy(&before.name).into_owned();
        assert_eq!(before.name, after.name, "entry {i} ({name}): name/order");
        if i != snipe_idx && i != target_idx {
            assert!(
                before.blob == after.blob,
                "entry {i} ({name}): blob must be byte-identical to the pristine one"
            );
            untouched += 1;
        }
    }
    assert_eq!(untouched, 1555);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
#[ignore] // needs CE_PRISTINE_143, see module comment
fn committed_deltas_reproduce_shipped_texsec() {
    // Provenance for the committed texture artifacts: the repo ships the
    // PRISTINE texsec.dat plus delta TGAs that textool patches in at install
    // time - never a pre-patched archive.
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let overrides = repo_root.join("game/full-overrides/24bits");

    // (a) The committed full-payload texsec.dat must be byte-identical to the
    // pristine 1.43 one - guards against someone re-committing a patched copy.
    let (pristine_path, original) = read_pristine_archive("texsec.dat");
    let pristine_raw = std::fs::read(&pristine_path).unwrap();
    let committed_path = repo_root.join("game/full/24bits/texsec.dat");
    let committed_raw = std::fs::read(&committed_path)
        .unwrap_or_else(|e| panic!("reading committed {}: {e}", committed_path.display()));
    assert!(
        committed_raw == pristine_raw,
        "{} must be byte-identical to the pristine 1.43 texsec.dat - the repo ships \
         the pristine archive and patches it at install time, never a pre-patched copy",
        committed_path.display()
    );

    // (b) Running the shipped pipeline (textool set + the committed delta TGA)
    // against the pristine archive must replace exactly INTERFC1.tga.
    let interfc1_path = overrides.join("INTERFC1.tga");
    let interfc1 = std::fs::read(&interfc1_path)
        .unwrap_or_else(|e| panic!("reading committed {}: {e}", interfc1_path.display()));
    let idx = original
        .iter()
        .position(|e| e.name == b"INTERFC1.tga")
        .expect("pristine texsec.dat must contain INTERFC1.tga");

    let dir = temp_dir("committed_deltas");
    let archive = dir.join("texsec.dat");
    std::fs::copy(&pristine_path, &archive).unwrap();

    run_set(&archive, &[&interfc1_path]);

    let patched = parse_archive(&std::fs::read(&archive).unwrap()).unwrap();
    assert_eq!(patched.len(), 27, "entry count must be preserved");
    for (i, (before, after)) in original.iter().zip(&patched).enumerate() {
        let name = String::from_utf8_lossy(&before.name).into_owned();
        assert_eq!(before.name, after.name, "entry {i} ({name}): name/order");
        if i == idx {
            assert!(
                after.blob == interfc1[TGA_PREFIX.len()..],
                "INTERFC1.tga blob must be the committed override minus its 8-byte prefix"
            );
        } else {
            assert!(
                before.blob == after.blob,
                "entry {i} ({name}): blob must be byte-identical to the pristine one"
            );
        }
    }
    let _ = std::fs::remove_dir_all(&dir);

    // The other two committed delta TGAs (patched into textures.dat at install
    // time) must still validate with their authored dimensions - a cheap guard
    // against corrupted or mis-regenerated files.
    for (file, w, h, depth) in [
        ("snipemod32.tga", 256, 256, 32),
        ("target32.tga", 32, 32, 32),
    ] {
        let path = overrides.join(file);
        let bytes =
            std::fs::read(&path).unwrap_or_else(|e| panic!("reading {}: {e}", path.display()));
        let info = validate_tga(&bytes).unwrap_or_else(|e| panic!("{} must validate: {e}", file));
        assert_eq!(
            (info.width, info.height, info.depth),
            (w, h, depth),
            "{file} dimensions"
        );
    }
}
