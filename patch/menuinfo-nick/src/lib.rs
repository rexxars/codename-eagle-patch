//! Codec for Codename Eagle's `menuinfo.dat` and an in-place edit of the
//! multiplayer player name it stores.
//!
//! The file is three layers deep: a zlib stream wrapped by a per-byte additive
//! cipher (KEY1), whose plaintext is itself wrapped by a second additive cipher
//! (KEY2). The plaintext is three fixed 272-byte blocks (`PlayInfo`,
//! `LevelsDone`, `OptionsMenu`), each a 16-byte NUL-padded tag followed by a
//! 256-byte struct. The player name lives in the `PlayInfo` block.

// KEY1 is the outer cipher (over the compressed bytes); KEY2 the inner one (over
// the plaintext). Both are applied per byte, mod 256, cyclically indexed by
// `i % KEY.len()`. The modulus is ALWAYS the real byte length (128 / 70) — the
// original spec's "126 / 69" are wrong and make inflation fail.
pub const KEY1: &[u8] = b"You really shouldn't be messing about with this file, you should be playing the game. You will find nothing in here you know ;-)";
pub const KEY2: &[u8] = b"Didn't you read the first message? I promise there is nothing in here.";

/// Total plaintext size: three 272-byte blocks.
pub const PLAIN_LEN: usize = 3 * 272;

/// Longest name that fully round-trips into multiplayer (the host truncates the
/// broadcast name to this).
pub const MAX_NAME: usize = 10;

/// Default name when none is supplied — the value the demo ships with.
pub const DEFAULT_NAME: &str = "CEDemo";

const BLOCK_LEN: usize = 272;
const TAG_LEN: usize = 16;
const PLAYINFO_TAG: &[u8] = b"PlayInfo";
// Player-name field inside a block: 20 bytes at block offset 0x42 (struct body
// +0x32). Do NOT touch the 40-byte host-name field at block offset 0x1a.
const NAME_OFF_IN_BLOCK: usize = 0x42;
const NAME_FIELD_LEN: usize = 20;

/// Decode a `menuinfo.dat` file into its plaintext (three 272-byte blocks).
pub fn decode(file: &[u8]) -> Result<Vec<u8>, String> {
    if file.len() < 8 {
        return Err("file too small to hold the 8-byte header".into());
    }
    let uncompressed = u32::from_le_bytes(file[0..4].try_into().unwrap()) as usize;
    let compressed = u32::from_le_bytes(file[4..8].try_into().unwrap()) as usize;
    if file.len() != 8 + compressed {
        return Err(format!(
            "compressedSize header ({compressed}) doesn't match file body ({})",
            file.len().saturating_sub(8)
        ));
    }

    // Strip KEY1 (outer) off the body to recover the zlib stream.
    let mut zstream = file[8..].to_vec();
    for (i, b) in zstream.iter_mut().enumerate() {
        *b = b.wrapping_sub(KEY1[i % KEY1.len()]);
    }

    let inflated = miniz_oxide::inflate::decompress_to_vec_zlib(&zstream)
        .map_err(|e| format!("inflate failed: {e:?}"))?;
    if inflated.len() != uncompressed {
        return Err(format!(
            "inflated length ({}) != uncompressedSize header ({uncompressed})",
            inflated.len()
        ));
    }
    if inflated.len() != PLAIN_LEN {
        return Err(format!(
            "plaintext is {} bytes, expected {PLAIN_LEN}",
            inflated.len()
        ));
    }

    // Strip KEY2 (inner) off the inflated bytes to recover the plaintext.
    let mut plain = inflated;
    for (i, b) in plain.iter_mut().enumerate() {
        *b = b.wrapping_sub(KEY2[i % KEY2.len()]);
    }
    Ok(plain)
}

/// Re-encode plaintext back into a `menuinfo.dat` file.
pub fn encode(plain: &[u8]) -> Result<Vec<u8>, String> {
    if plain.len() != PLAIN_LEN {
        return Err(format!(
            "plaintext must be {PLAIN_LEN} bytes, got {}",
            plain.len()
        ));
    }

    // Add KEY2 (inner), then deflate.
    let mut mid = plain.to_vec();
    for (i, b) in mid.iter_mut().enumerate() {
        *b = b.wrapping_add(KEY2[i % KEY2.len()]);
    }
    let mut body = miniz_oxide::deflate::compress_to_vec_zlib(&mid, 6);

    // Add KEY1 (outer) over the compressed bytes.
    for (i, b) in body.iter_mut().enumerate() {
        *b = b.wrapping_add(KEY1[i % KEY1.len()]);
    }

    let mut file = Vec::with_capacity(8 + body.len());
    file.extend_from_slice(&(mid.len() as u32).to_le_bytes());
    file.extend_from_slice(&(body.len() as u32).to_le_bytes());
    file.extend_from_slice(&body);
    Ok(file)
}

/// Overwrite the `PlayInfo` player-name field in decoded plaintext, in place.
pub fn set_nickname(plain: &mut [u8], name: &str) -> Result<(), String> {
    let block = find_playinfo(plain)?;
    let field = block + NAME_OFF_IN_BLOCK;

    // Sanity-check the field really is a C-string (or empty) before overwriting,
    // so a wrong offset / unexpected layout aborts rather than corrupts.
    let current = &plain[field..field + NAME_FIELD_LEN];
    let is_cstring = current
        .iter()
        .take_while(|&&b| b != 0)
        .all(|&b| (0x20..=0x7e).contains(&b));
    if !is_cstring {
        return Err("PlayInfo name field is not an ASCII string — refusing to patch".into());
    }

    let bytes = name.as_bytes();
    if bytes.len() >= NAME_FIELD_LEN {
        return Err(format!("name too long for the {NAME_FIELD_LEN}-byte field"));
    }
    // Write name, one NUL terminator, then zero-fill the rest of the field.
    for b in &mut plain[field..field + NAME_FIELD_LEN] {
        *b = 0;
    }
    plain[field..field + bytes.len()].copy_from_slice(bytes);
    Ok(())
}

/// Normalize raw user input into an acceptable name: printable ASCII only,
/// no double-quotes, trimmed to `MAX_NAME`, falling back to [`DEFAULT_NAME`].
pub fn sanitize_nickname(raw: &str) -> String {
    let cleaned: String = raw
        .chars()
        .filter(|&c| c.is_ascii_graphic() && c != '"' || c == ' ')
        .take(MAX_NAME)
        .collect();
    let cleaned = cleaned.trim();
    if cleaned.is_empty() {
        DEFAULT_NAME.to_string()
    } else {
        cleaned.to_string()
    }
}

/// Decode a file, set the (already-sanitized) name, and re-encode.
pub fn patch_file(file: &[u8], name: &str) -> Result<Vec<u8>, String> {
    let mut plain = decode(file)?;
    set_nickname(&mut plain, name)?;
    encode(&plain)
}

/// Find the start of the `PlayInfo` block (0, 272, or 544) by its tag.
fn find_playinfo(plain: &[u8]) -> Result<usize, String> {
    for b in 0..3 {
        let start = b * BLOCK_LEN;
        let tag = &plain[start..start + TAG_LEN];
        let name_end = tag.iter().position(|&c| c == 0).unwrap_or(TAG_LEN);
        if &tag[..name_end] == PLAYINFO_TAG {
            return Ok(start);
        }
    }
    Err("no PlayInfo block found in plaintext".into())
}
