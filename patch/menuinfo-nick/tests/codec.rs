use menuinfo_nick::{
    decode, encode, patch_file, sanitize_nickname, set_nickname, DEFAULT_NAME, PLAIN_LEN,
};

const FIXTURE: &[u8] = include_bytes!("fixtures/menuinfo.dat");

// Find the PlayInfo block start in decoded plaintext (mirrors the lib's own
// scan; used only by tests to assert on the right bytes).
fn playinfo_start(plain: &[u8]) -> usize {
    for b in 0..3 {
        let start = b * 272;
        let tag = &plain[start..start + 8];
        if tag == b"PlayInfo" {
            return start;
        }
    }
    panic!("no PlayInfo block");
}

fn read_name(plain: &[u8], field_off: usize) -> String {
    let field = &plain[field_off..field_off + 20];
    let end = field.iter().position(|&b| b == 0).unwrap_or(field.len());
    String::from_utf8_lossy(&field[..end]).into_owned()
}

#[test]
fn decodes_fixture_to_three_blocks_with_expected_tags() {
    let plain = decode(FIXTURE).expect("decode");
    assert_eq!(plain.len(), PLAIN_LEN);
    assert_eq!(&plain[0..8], b"PlayInfo");
    assert_eq!(&plain[272..282], b"LevelsDone");
    assert_eq!(&plain[544..555], b"OptionsMenu");
}

#[test]
fn fixture_ships_with_cedemo_as_the_player_name() {
    let plain = decode(FIXTURE).expect("decode");
    let p = playinfo_start(&plain);
    assert_eq!(read_name(&plain, p + 0x42), "CEDemo");
}

#[test]
fn roundtrip_preserves_plaintext_byte_for_byte() {
    let plain = decode(FIXTURE).expect("decode");
    let file = encode(&plain).expect("encode");
    let plain2 = decode(&file).expect("decode again");
    assert_eq!(
        plain, plain2,
        "plaintext must survive a decode/encode round-trip"
    );
}

#[test]
fn set_nickname_writes_name_then_nul_then_zero_fill() {
    let mut plain = decode(FIXTURE).expect("decode");
    let p = playinfo_start(&plain);
    set_nickname(&mut plain, "Xyz").expect("set");
    let field = &plain[p + 0x42..p + 0x42 + 20];
    assert_eq!(&field[..3], b"Xyz");
    assert_eq!(field[3], 0);
    assert!(
        field[3..].iter().all(|&b| b == 0),
        "rest of field zero-filled"
    );
}

#[test]
fn set_nickname_touches_nothing_but_the_name_field() {
    let original = decode(FIXTURE).expect("decode");
    let mut plain = original.clone();
    let p = playinfo_start(&plain);
    set_nickname(&mut plain, "Xyz").expect("set");
    for i in 0..plain.len() {
        let in_name_field = i >= p + 0x42 && i < p + 0x42 + 20;
        if !in_name_field {
            assert_eq!(
                plain[i], original[i],
                "byte {i} changed outside the name field"
            );
        }
    }
}

#[test]
fn set_nickname_leaves_the_host_name_field_untouched() {
    // The 40-byte host field at block +0x1a holds "CEDEMO" in the fixture.
    let mut plain = decode(FIXTURE).expect("decode");
    let p = playinfo_start(&plain);
    set_nickname(&mut plain, "Newname").expect("set");
    assert_eq!(read_name(&plain, p + 0x1a), "CEDEMO");
}

#[test]
fn patch_file_end_to_end_sets_the_name() {
    let file = patch_file(FIXTURE, "Bob").expect("patch");
    let plain = decode(&file).expect("decode patched");
    let p = playinfo_start(&plain);
    assert_eq!(read_name(&plain, p + 0x42), "Bob");
}

#[test]
fn patch_file_uncompressed_size_header_stays_816() {
    let file = patch_file(FIXTURE, "Bob").expect("patch");
    let usize_hdr = u32::from_le_bytes(file[0..4].try_into().unwrap());
    let csize_hdr = u32::from_le_bytes(file[4..8].try_into().unwrap());
    assert_eq!(usize_hdr as usize, PLAIN_LEN);
    assert_eq!(file.len(), 8 + csize_hdr as usize);
}

#[test]
fn sanitize_empty_falls_back_to_default() {
    assert_eq!(sanitize_nickname(""), DEFAULT_NAME);
    assert_eq!(sanitize_nickname("   "), DEFAULT_NAME);
}

#[test]
fn sanitize_truncates_to_ten() {
    assert_eq!(sanitize_nickname("abcdefghijklmno"), "abcdefghij");
}

#[test]
fn sanitize_strips_non_ascii_and_quotes() {
    assert_eq!(sanitize_nickname("Bj\u{f8}rn"), "Bjrn");
    assert_eq!(sanitize_nickname("ab\"cd"), "abcd");
}

#[test]
fn sanitize_strips_chars_the_engine_would_x_out() {
    // ce.exe's session sanitizer replaces these with a literal 'X'; we drop
    // them instead so the profile name plays back as typed.
    assert_eq!(sanitize_nickname("a.b,c-d_e"), "abcde");
    assert_eq!(sanitize_nickname("w^x~y`z"), "wxyz");
    assert_eq!(sanitize_nickname("joe black"), "joeblack");
    assert_eq!(sanitize_nickname("foo\\bar"), "foobar");
}

#[test]
fn sanitize_short_results_fall_back_to_default() {
    // 1-2 char names get "(XXXXXX)" appended by the engine in a session.
    assert_eq!(sanitize_nickname("Ed"), DEFAULT_NAME);
    assert_eq!(sanitize_nickname("a-b"), DEFAULT_NAME); // only 2 chars survive
}

#[test]
fn decode_rejects_a_truncated_file() {
    assert!(decode(&FIXTURE[..FIXTURE.len() - 5]).is_err());
}

#[test]
fn decode_rejects_a_wrong_compressed_size_header() {
    let mut bad = FIXTURE.to_vec();
    bad[4] = bad[4].wrapping_add(1); // compressedSize no longer matches file len
    assert!(decode(&bad).is_err());
}
