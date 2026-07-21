//! Add/replace TGA textures inside Codename Eagle texture archives
//! (`24bits/textures.dat`, `24bits/texsec.dat`).
//!
//! The archives store each texture as a standard uncompressed true-color TGA
//! minus its first 8 constant bytes ([`TGA_PREFIX`]). This crate validates
//! standalone TGAs for insertion ([`validate_tga`]), reads archives leniently
//! ([`parse_archive`]), writes them canonically ([`build_archive`]), and
//! adds/replaces entries ([`upsert`]); the CLI comes on top of it.

/// The 8 constant bytes the archives strip off the front of every stored TGA:
/// id_length 0, color_map_type 0, image_type 2 (uncompressed true-color), and
/// the first 5 bytes of the (all-zero) color map spec.
pub const TGA_PREFIX: [u8; 8] = [0, 0, 2, 0, 0, 0, 0, 0];

/// Standard TGA header length; pixel data follows immediately (no image ID,
/// no color map).
pub const TGA_HEADER_LEN: usize = 18;

/// Dimensions and pixel depth of a validated TGA.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TgaInfo {
    pub width: u16,
    pub height: u16,
    pub depth: u8,
}

/// Validate a standalone TGA for archive insertion, strictly (square
/// power-of-two). Equivalent to [`validate_tga_opts`] with `allow_any = false`;
/// this is the right check for the in-game archives `textures.dat`/`texsec.dat`.
pub fn validate_tga(bytes: &[u8]) -> Result<TgaInfo, String> {
    validate_tga_opts(bytes, false)
}

/// Validate a standalone TGA for archive insertion and return its info.
///
/// The archives only hold uncompressed true-color TGAs, so anything else is
/// refused rather than risk corrupting the archive:
/// - header must match [`TGA_PREFIX`] (id_length 0, color_map_type 0,
///   image_type 2 — no RLE, no color map, no image ID);
/// - depth 24 or 32; descriptor 0x00/0x08 (alpha bits) with optional 0x20
///   (top-to-bottom origin);
/// - dimensions: with `allow_any = false` the 3D renderer needs square
///   power-of-two textures (`width == height`, a power of two in `1..=1024`);
///   with `allow_any = true` — for the menu archives (e.g. `menupics.dat`),
///   whose bitmaps are arbitrary sizes like 80×144 or 640×480 — that is relaxed
///   to each side in `1..=4096`;
/// - length exactly `18 + w*h*(depth/8)` — truncated pixel data or trailing
///   junk (e.g. a TGA v2 footer) is rejected. This exact-length rule always
///   applies, so `allow_any` never lets a malformed blob into the archive.
pub fn validate_tga_opts(bytes: &[u8], allow_any: bool) -> Result<TgaInfo, String> {
    if bytes.len() < TGA_HEADER_LEN {
        return Err(format!(
            "file is {} bytes, shorter than the {TGA_HEADER_LEN}-byte TGA header",
            bytes.len()
        ));
    }
    if bytes[0] != 0 {
        return Err(format!(
            "id_length is {}, expected 0 (no image ID)",
            bytes[0]
        ));
    }
    if bytes[1] != 0 {
        return Err(format!(
            "color_map_type is {}, expected 0 (no color map)",
            bytes[1]
        ));
    }
    if bytes[2] != 2 {
        return Err(format!(
            "image_type is {}, expected 2 (uncompressed true-color; RLE is not supported)",
            bytes[2]
        ));
    }
    if bytes[..TGA_PREFIX.len()] != TGA_PREFIX {
        return Err(format!(
            "header bytes 3..8 are {:?}, expected all zero (start of the color map spec)",
            &bytes[3..TGA_PREFIX.len()]
        ));
    }

    let depth = bytes[16];
    if depth != 24 && depth != 32 {
        return Err(format!("pixel depth is {depth}, expected 24 or 32"));
    }
    let descriptor = bytes[17];
    if !matches!(descriptor, 0x00 | 0x08 | 0x20 | 0x28) {
        return Err(format!(
            "descriptor is 0x{descriptor:02x}, expected 0x00, 0x08, 0x20 or 0x28"
        ));
    }

    let width = u16le(bytes, 12);
    let height = u16le(bytes, 14);
    if allow_any {
        if width == 0 || height == 0 || width > 4096 || height > 4096 {
            return Err(format!(
                "image is {width}x{height}, expected each side in 1..=4096"
            ));
        }
    } else {
        if width != height {
            return Err(format!("image is {width}x{height}, expected square"));
        }
        if width == 0 || !width.is_power_of_two() || width > 1024 {
            return Err(format!(
                "width is {width}, expected a power of two in 1..=1024"
            ));
        }
    }

    let expected = TGA_HEADER_LEN + width as usize * height as usize * (depth / 8) as usize;
    if bytes.len() != expected {
        return Err(format!(
            "file is {} bytes, expected exactly {expected} for {width}x{height}x{depth} \
             (truncated pixel data or trailing bytes)",
            bytes.len()
        ));
    }

    Ok(TgaInfo {
        width,
        height,
        depth,
    })
}

fn u16le(b: &[u8], o: usize) -> u16 {
    u16::from_le_bytes([b[o], b[o + 1]])
}

// ---------------------------------------------------------------------------
// Archive format
// ---------------------------------------------------------------------------
//
// All integers little-endian:
//
//   offset 0     : u32 entry_count (used slots)
//   offset 4     : TOC = 2048 FIXED slots x 17 bytes (regardless of entry_count)
//   offset 34820 : blobs, concatenated in TOC order   (34820 = 4 + 2048*17)
//
// Each 17-byte TOC record (at `4 + i*17`) is a 13-byte name field
// (NUL-terminated Latin-1, max 12 name bytes; bytes after the NUL are
// don't-care, canonical fill 0xCC) followed by a u32 ABSOLUTE blob offset
// (not a length). Blob length is implied: `offset[i+1] - offset[i]`; the last
// blob runs to EOF. Unused TOC slots are all-0xCC; the whole file is
// 0xCC-prefilled before writing. A blob is a standard TGA minus its first 8
// constant bytes ([`TGA_PREFIX`]).

/// Length of one TOC record: 13-byte name field + u32 absolute blob offset.
pub const RECORD_LEN: usize = 17;

/// Length of the name field inside a TOC record: at most 12 name bytes plus
/// the NUL terminator.
pub const NAME_FIELD_LEN: usize = 13;

/// The TOC always has this many slots, used or not.
pub const TOC_SLOTS: usize = 2048;

/// Where blobs start in a canonical archive: right after the fixed-size TOC.
pub const BLOB_START: usize = 4 + TOC_SLOTS * RECORD_LEN; // 34820

/// Canonical fill byte for unused TOC slots, name-field padding, and any
/// other slack.
pub const FILL: u8 = 0xCC;

/// One texture in an archive: the raw name bytes before the NUL (Latin-1,
/// 1..=12 bytes) and the stored blob (a TGA minus its 8-byte [`TGA_PREFIX`]).
#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    pub name: Vec<u8>,
    pub blob: Vec<u8>,
}

/// Parse a texture archive leniently: follow `entry_count` and the TOC
/// offsets, bounds-checked, without assuming canonical layout — shipped
/// original archives have garbage padding and may keep blobs elsewhere than
/// [`BLOB_START`].
///
/// Errors on: file shorter than 4 bytes, count > [`TOC_SLOTS`], TOC truncated
/// (file too short for `count` used slots), empty or unterminated name, blob
/// offset before the end of the used TOC or past EOF, non-monotonic offsets.
pub fn parse_archive(bytes: &[u8]) -> Result<Vec<Entry>, String> {
    if bytes.len() < 4 {
        return Err(format!(
            "file is {} bytes, shorter than the 4-byte entry count",
            bytes.len()
        ));
    }
    let count = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as usize;
    if count > TOC_SLOTS {
        return Err(format!(
            "entry count is {count}, the TOC has only {TOC_SLOTS} slots"
        ));
    }
    let toc_end = 4 + count * RECORD_LEN;
    if bytes.len() < toc_end {
        return Err(format!(
            "file is {} bytes, too short for the {toc_end}-byte header + {count}-entry TOC",
            bytes.len()
        ));
    }

    let mut offsets = Vec::with_capacity(count);
    let mut names = Vec::with_capacity(count);
    for i in 0..count {
        let rec = &bytes[4 + i * RECORD_LEN..4 + (i + 1) * RECORD_LEN];
        let name_field = &rec[..NAME_FIELD_LEN];
        let Some(nul) = name_field.iter().position(|&b| b == 0) else {
            return Err(format!(
                "entry {i}: name field has no NUL terminator within {NAME_FIELD_LEN} bytes"
            ));
        };
        if nul == 0 {
            return Err(format!("entry {i}: empty name"));
        }
        let offset = u32::from_le_bytes([
            rec[NAME_FIELD_LEN],
            rec[NAME_FIELD_LEN + 1],
            rec[NAME_FIELD_LEN + 2],
            rec[NAME_FIELD_LEN + 3],
        ]) as usize;
        if offset < toc_end || offset > bytes.len() {
            return Err(format!(
                "entry {i}: blob offset {offset} outside the valid range {toc_end}..={}",
                bytes.len()
            ));
        }
        if let Some(&prev) = offsets.last() {
            if offset < prev {
                return Err(format!(
                    "entry {i}: blob offset {offset} is before the previous entry's \
                     offset {prev} (offsets must be monotonic)"
                ));
            }
        }
        names.push(name_field[..nul].to_vec());
        offsets.push(offset);
    }

    Ok(names
        .into_iter()
        .enumerate()
        .map(|(i, name)| {
            let end = if i + 1 < count {
                offsets[i + 1]
            } else {
                bytes.len()
            };
            Entry {
                name,
                blob: bytes[offsets[i]..end].to_vec(),
            }
        })
        .collect())
}

/// Write a texture archive in the CANONICAL layout, mirroring cnetool's JS
/// `buildTextureArchive`: whole file [`FILL`]-prefilled, u32 entry count,
/// fixed [`TOC_SLOTS`]-slot TOC (per used slot: name bytes + NUL, rest of the
/// 13-byte field left [`FILL`], u32 absolute blob offset), blobs contiguous
/// from [`BLOB_START`].
///
/// Errors on: more than [`TOC_SLOTS`] entries, a name empty or longer than 12
/// bytes, a blob shorter than 10 bytes (a prefix-stripped TGA is at least its
/// remaining `18 - 8` header bytes), or a total archive size that would not
/// fit the u32 blob-offset fields (> [`u32::MAX`] bytes).
pub fn build_archive(entries: &[Entry]) -> Result<Vec<u8>, String> {
    if entries.len() > TOC_SLOTS {
        return Err(format!(
            "{} entries, the TOC has only {TOC_SLOTS} slots",
            entries.len()
        ));
    }
    const MIN_BLOB_LEN: usize = TGA_HEADER_LEN - TGA_PREFIX.len(); // 10
    for (i, e) in entries.iter().enumerate() {
        let name = String::from_utf8_lossy(&e.name);
        if e.name.is_empty() || e.name.len() > NAME_FIELD_LEN - 1 {
            return Err(format!(
                "entry {i} (\"{name}\"): name is {} bytes, expected 1..={} \
                 (13-byte field incl. NUL)",
                e.name.len(),
                NAME_FIELD_LEN - 1
            ));
        }
        if e.blob.len() < MIN_BLOB_LEN {
            return Err(format!(
                "entry {i} (\"{name}\"): blob is {} bytes, shorter than the \
                 {MIN_BLOB_LEN}-byte prefix-stripped TGA header",
                e.blob.len()
            ));
        }
    }

    // The TOC stores blob offsets as u32, so refuse any archive whose total
    // size would not fit — otherwise `blob_off as u32` below would silently
    // truncate offsets past 4 GiB. Untested by design: exercising it would
    // need > 4 GiB of actual blob `Vec`s (validate_tga caps real textures at
    // 1024*1024*4 bytes, and `Entry` holds owned bytes, so there is no cheap
    // way to fabricate the size).
    let total = entries
        .iter()
        .try_fold(BLOB_START, |acc, e| acc.checked_add(e.blob.len()))
        .filter(|&total| u32::try_from(total).is_ok())
        .ok_or_else(|| {
            format!(
                "archive would be larger than the {}-byte u32 blob-offset limit",
                u32::MAX
            )
        })?;
    let mut out = vec![FILL; total];
    out[..4].copy_from_slice(&(entries.len() as u32).to_le_bytes());
    let mut blob_off = BLOB_START;
    for (i, e) in entries.iter().enumerate() {
        let rec = 4 + i * RECORD_LEN;
        out[rec..rec + e.name.len()].copy_from_slice(&e.name);
        out[rec + e.name.len()] = 0;
        out[rec + NAME_FIELD_LEN..rec + RECORD_LEN]
            .copy_from_slice(&(blob_off as u32).to_le_bytes());
        out[blob_off..blob_off + e.blob.len()].copy_from_slice(&e.blob);
        blob_off += e.blob.len();
    }
    Ok(out)
}

/// What [`upsert`] did with the texture.
#[derive(Debug, PartialEq, Eq)]
pub enum UpsertOutcome {
    Replaced,
    Added,
}

/// Insert `tga` (a standalone TGA, validated via [`validate_tga`]) into
/// `entries` under `name` (the staged file's basename). The name is matched
/// case-insensitively (ASCII) against existing entries; on a match the STORED
/// name is preserved — stock archives use e.g. `TARGET.tga` while we stage
/// `Target.tga`. The blob is stored verbatim as `tga` minus the 8-byte
/// [`TGA_PREFIX`]. When nothing matches, the entry is appended.
///
/// Names are matched and stored as raw bytes: stored names are Latin-1, and
/// every real texture name is plain ASCII. Non-ASCII names are outside the
/// supported envelope — `eq_ignore_ascii_case` only folds ASCII letters, so a
/// non-ASCII `name` would be matched byte-exactly and stored as its UTF-8
/// bytes (which a Latin-1 reader would misrender). The CLI therefore rejects
/// non-ASCII basenames up front.
///
/// Errors on: an invalid TGA, and — when appending — an empty or > 12-byte
/// name or an already-full ([`TOC_SLOTS`] entries) archive. Replacing in a
/// full archive is fine. On any error `entries` is left unchanged.
pub fn upsert(entries: &mut Vec<Entry>, name: &str, tga: &[u8]) -> Result<UpsertOutcome, String> {
    upsert_opts(entries, name, tga, false)
}

/// Like [`upsert`], but `allow_any` relaxes the dimension check for the menu
/// archives (see [`validate_tga_opts`]); every other validation still applies.
pub fn upsert_opts(
    entries: &mut Vec<Entry>,
    name: &str,
    tga: &[u8],
    allow_any: bool,
) -> Result<UpsertOutcome, String> {
    validate_tga_opts(tga, allow_any)?;
    let blob = tga[TGA_PREFIX.len()..].to_vec();

    if let Some(existing) = entries
        .iter_mut()
        .find(|e| e.name.eq_ignore_ascii_case(name.as_bytes()))
    {
        existing.blob = blob;
        return Ok(UpsertOutcome::Replaced);
    }

    if name.is_empty() || name.len() > NAME_FIELD_LEN - 1 {
        return Err(format!(
            "name \"{name}\" is {} bytes, expected 1..={} (13-byte field incl. NUL)",
            name.len(),
            NAME_FIELD_LEN - 1
        ));
    }
    if entries.len() >= TOC_SLOTS {
        return Err(format!(
            "cannot add \"{name}\": the archive already has {TOC_SLOTS} entries (TOC is full)"
        ));
    }
    entries.push(Entry {
        name: name.as_bytes().to_vec(),
        blob,
    });
    Ok(UpsertOutcome::Added)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a valid uncompressed true-color TGA: 18-byte header (byte 2 = 2,
    /// u16LE width @12 / height @14, depth @16, descriptor @17 — 8 alpha bits
    /// for 32-bit, 0 otherwise) followed by w*h*(depth/8) zeroed pixel bytes.
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

    #[test]
    fn accepts_32x32_24bit() {
        let info = validate_tga(&make_tga(32, 32, 24)).unwrap();
        assert_eq!(info.width, 32);
        assert_eq!(info.height, 32);
        assert_eq!(info.depth, 24);
    }

    #[test]
    fn accepts_8x8_32bit() {
        let info = validate_tga(&make_tga(8, 8, 32)).unwrap();
        assert_eq!(info.width, 8);
        assert_eq!(info.height, 8);
        assert_eq!(info.depth, 32);
    }

    #[test]
    fn rejects_short_buffer() {
        assert!(validate_tga(&[0u8; 17]).is_err());
    }

    #[test]
    fn rejects_rle_image_type() {
        let mut tga = make_tga(32, 32, 24);
        tga[2] = 10; // RLE true-color
        assert!(validate_tga(&tga).is_err());
    }

    #[test]
    fn rejects_nonzero_id_length() {
        let mut tga = make_tga(32, 32, 24);
        tga[0] = 4;
        assert!(validate_tga(&tga).is_err());
    }

    #[test]
    fn rejects_nonzero_color_map_type() {
        let mut tga = make_tga(32, 32, 24);
        tga[1] = 1;
        assert!(validate_tga(&tga).is_err());
    }

    #[test]
    fn rejects_16bit_depth() {
        assert!(validate_tga(&make_tga(32, 32, 16)).is_err());
    }

    #[test]
    fn rejects_non_square() {
        // Pixel count matches 32x16, so only the squareness rule can reject it.
        let mut tga = make_tga(32, 32, 24);
        tga[14..16].copy_from_slice(&16u16.to_le_bytes());
        tga.truncate(TGA_HEADER_LEN + 32 * 16 * 3);
        assert!(validate_tga(&tga).is_err());
    }

    #[test]
    fn rejects_non_power_of_two() {
        assert!(validate_tga(&make_tga(24, 24, 24)).is_err());
    }

    #[test]
    fn rejects_oversize() {
        assert!(validate_tga(&make_tga(2048, 2048, 24)).is_err());
    }

    #[test]
    fn allow_any_accepts_non_square_non_pow2() {
        // 80x144 is the menufont: refused strictly, accepted with allow_any.
        let tga = make_tga(80, 144, 24);
        assert!(validate_tga_opts(&tga, false).is_err());
        let info = validate_tga_opts(&tga, true).unwrap();
        assert_eq!((info.width, info.height, info.depth), (80, 144, 24));
    }

    #[test]
    fn allow_any_still_enforces_exact_length_and_bounds() {
        // The length rule that guards the archive still applies with allow_any.
        let mut trailing = make_tga(80, 144, 24);
        trailing.push(0);
        assert!(validate_tga_opts(&trailing, true).is_err());
        let mut truncated = make_tga(80, 144, 24);
        truncated.pop();
        assert!(validate_tga_opts(&truncated, true).is_err());
        // Zero and > 4096 sides are still refused.
        assert!(validate_tga_opts(&make_tga(0, 16, 24), true).is_err());
        assert!(validate_tga_opts(&make_tga(4097, 1, 24), true).is_err());
    }

    #[test]
    fn rejects_truncated_pixel_data() {
        let mut tga = make_tga(32, 32, 24);
        tga.pop();
        assert!(validate_tga(&tga).is_err());
    }

    #[test]
    fn rejects_trailing_bytes() {
        let mut tga = make_tga(32, 32, 24);
        tga.push(0);
        assert!(validate_tga(&tga).is_err());
    }

    #[test]
    fn rejects_unknown_descriptor() {
        let mut tga = make_tga(32, 32, 24);
        tga[17] = 0x01;
        assert!(validate_tga(&tga).is_err());
    }

    #[test]
    fn rejects_nonzero_color_map_spec() {
        // Bytes 3..8 (color map spec) must be zero, like the rest of TGA_PREFIX.
        let mut tga = make_tga(32, 32, 24);
        tga[4] = 1;
        assert!(validate_tga(&tga).is_err());
    }

    // -- archive fixtures ---------------------------------------------------

    /// Hand-assemble a CANONICAL archive straight from the format spec, as the
    /// independent golden reference for `build_archive`: 0xCC prefill, u32LE
    /// entry count, fixed 2048-slot TOC (per used slot: name bytes + NUL, rest
    /// of the 13-byte field left 0xCC, u32LE absolute blob offset at +13),
    /// blobs concatenated from byte 34820.
    fn build_raw(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let total = BLOB_START + entries.iter().map(|(_, b)| b.len()).sum::<usize>();
        let mut out = vec![FILL; total];
        out[..4].copy_from_slice(&(entries.len() as u32).to_le_bytes());
        let mut blob_off = BLOB_START;
        for (i, (name, blob)) in entries.iter().enumerate() {
            let rec = 4 + i * RECORD_LEN;
            out[rec..rec + name.len()].copy_from_slice(name.as_bytes());
            out[rec + name.len()] = 0;
            out[rec + NAME_FIELD_LEN..rec + RECORD_LEN]
                .copy_from_slice(&(blob_off as u32).to_le_bytes());
            out[blob_off..blob_off + blob.len()].copy_from_slice(blob);
            blob_off += blob.len();
        }
        out
    }

    /// A NON-canonical but valid archive: blobs packed tightly right after the
    /// used TOC slots (not at 34820), zero fill instead of 0xCC, no unused
    /// slots. The lenient reader must accept this.
    fn build_tight(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let toc_end = 4 + entries.len() * RECORD_LEN;
        let total = toc_end + entries.iter().map(|(_, b)| b.len()).sum::<usize>();
        let mut out = vec![0u8; total];
        out[..4].copy_from_slice(&(entries.len() as u32).to_le_bytes());
        let mut blob_off = toc_end;
        for (i, (name, blob)) in entries.iter().enumerate() {
            let rec = 4 + i * RECORD_LEN;
            out[rec..rec + name.len()].copy_from_slice(name.as_bytes());
            out[rec + NAME_FIELD_LEN..rec + RECORD_LEN]
                .copy_from_slice(&(blob_off as u32).to_le_bytes());
            out[blob_off..blob_off + blob.len()].copy_from_slice(blob);
            blob_off += blob.len();
        }
        out
    }

    // -- parse_archive ------------------------------------------------------

    #[test]
    fn parses_canonical_two_entry_archive() {
        let blob_a: Vec<u8> = (0u8..40).collect();
        let blob_b: Vec<u8> = (100u8..115).collect();
        let raw = build_raw(&[("ALPHA.tga", &blob_a), ("beta.tga", &blob_b)]);
        let entries = parse_archive(&raw).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].name, b"ALPHA.tga");
        assert_eq!(entries[0].blob, blob_a);
        assert_eq!(entries[1].name, b"beta.tga");
        // Last blob's implied length runs to EOF.
        assert_eq!(entries[1].blob, blob_b);
    }

    #[test]
    fn parses_empty_archive() {
        let raw = build_raw(&[]);
        assert_eq!(parse_archive(&raw).unwrap(), Vec::<Entry>::new());
    }

    #[test]
    fn rejects_count_above_toc_slots() {
        let mut raw = build_raw(&[]);
        raw[..4].copy_from_slice(&2049u32.to_le_bytes());
        assert!(parse_archive(&raw).is_err());
    }

    #[test]
    fn rejects_offset_past_eof() {
        let blob: Vec<u8> = (0u8..20).collect();
        let mut raw = build_raw(&[("A.tga", &blob)]);
        let bad = (raw.len() + 1) as u32;
        raw[4 + NAME_FIELD_LEN..4 + RECORD_LEN].copy_from_slice(&bad.to_le_bytes());
        assert!(parse_archive(&raw).is_err());
    }

    #[test]
    fn rejects_non_monotonic_offsets() {
        let blob_a: Vec<u8> = (0u8..40).collect();
        let blob_b: Vec<u8> = (100u8..115).collect();
        let mut raw = build_raw(&[("A.tga", &blob_a), ("B.tga", &blob_b)]);
        // Both offsets stay in bounds, but the second points before the first.
        let first = BLOB_START as u32;
        let second = (BLOB_START + blob_a.len()) as u32;
        raw[4 + NAME_FIELD_LEN..4 + RECORD_LEN].copy_from_slice(&second.to_le_bytes());
        let rec2 = 4 + RECORD_LEN;
        raw[rec2 + NAME_FIELD_LEN..rec2 + RECORD_LEN].copy_from_slice(&first.to_le_bytes());
        assert!(parse_archive(&raw).is_err());
    }

    #[test]
    fn rejects_truncated_toc() {
        let blob: Vec<u8> = (0u8..20).collect();
        let mut raw = build_raw(&[("A.tga", &blob), ("B.tga", &blob)]);
        // Two used slots need 4 + 2*17 = 38 bytes; cut into the second record.
        raw.truncate(30);
        assert!(parse_archive(&raw).is_err());
    }

    #[test]
    fn rejects_file_shorter_than_count_field() {
        assert!(parse_archive(&[0u8; 3]).is_err());
    }

    #[test]
    fn rejects_empty_entry_name() {
        let blob: Vec<u8> = (0u8..20).collect();
        let mut raw = build_raw(&[("A.tga", &blob)]);
        raw[4] = 0; // NUL as the first name byte
        assert!(parse_archive(&raw).is_err());
    }

    #[test]
    fn rejects_name_without_nul_terminator() {
        let blob: Vec<u8> = (0u8..20).collect();
        let mut raw = build_raw(&[("A.tga", &blob)]);
        // Overwrite the whole 13-byte name field with letters: no NUL left.
        raw[4..4 + NAME_FIELD_LEN].fill(b'A');
        assert!(parse_archive(&raw).is_err());
    }

    #[test]
    fn rejects_offset_inside_used_toc() {
        let blob: Vec<u8> = (0u8..20).collect();
        let mut raw = build_raw(&[("A.tga", &blob)]);
        // One used slot ends at 4 + 17 = 21; an offset of 8 points into it.
        raw[4 + NAME_FIELD_LEN..4 + RECORD_LEN].copy_from_slice(&8u32.to_le_bytes());
        assert!(parse_archive(&raw).is_err());
    }

    #[test]
    fn parses_tight_non_canonical_layout() {
        let blob_a: Vec<u8> = (0u8..40).collect();
        let blob_b: Vec<u8> = (100u8..115).collect();
        let raw = build_tight(&[("ALPHA.tga", &blob_a), ("beta.tga", &blob_b)]);
        assert!(raw.len() < BLOB_START, "fixture must not be canonical");
        let entries = parse_archive(&raw).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].name, b"ALPHA.tga");
        assert_eq!(entries[0].blob, blob_a);
        assert_eq!(entries[1].name, b"beta.tga");
        assert_eq!(entries[1].blob, blob_b);
    }

    // -- build_archive ------------------------------------------------------

    fn entry(name: &str, blob: &[u8]) -> Entry {
        Entry {
            name: name.as_bytes().to_vec(),
            blob: blob.to_vec(),
        }
    }

    #[test]
    fn build_matches_hand_assembled_golden() {
        let blob_a: Vec<u8> = (0u8..40).collect();
        let blob_b: Vec<u8> = (100u8..115).collect();
        let built =
            build_archive(&[entry("ALPHA.tga", &blob_a), entry("beta.tga", &blob_b)]).unwrap();
        let golden = build_raw(&[("ALPHA.tga", &blob_a), ("beta.tga", &blob_b)]);
        assert_eq!(built, golden);
    }

    #[test]
    fn build_then_parse_round_trips() {
        let entries = vec![
            entry("ALPHA.tga", &(0u8..40).collect::<Vec<_>>()),
            entry("beta.tga", &(100u8..115).collect::<Vec<_>>()),
        ];
        let built = build_archive(&entries).unwrap();
        assert_eq!(parse_archive(&built).unwrap(), entries);
    }

    #[test]
    fn parse_then_build_of_canonical_archive_is_byte_identical() {
        let blob_a: Vec<u8> = (0u8..40).collect();
        let blob_b: Vec<u8> = (100u8..115).collect();
        let raw = build_raw(&[("ALPHA.tga", &blob_a), ("beta.tga", &blob_b)]);
        let rebuilt = build_archive(&parse_archive(&raw).unwrap()).unwrap();
        assert_eq!(rebuilt, raw);
    }

    #[test]
    fn parse_then_build_recanonicalizes_tight_layout() {
        let blob_a: Vec<u8> = (0u8..40).collect();
        let blob_b: Vec<u8> = (100u8..115).collect();
        let tight = build_tight(&[("ALPHA.tga", &blob_a), ("beta.tga", &blob_b)]);
        let entries = parse_archive(&tight).unwrap();
        let rebuilt = build_archive(&entries).unwrap();
        assert_ne!(
            rebuilt, tight,
            "canonical output must differ from tight input"
        );
        assert_eq!(parse_archive(&rebuilt).unwrap(), entries);
    }

    #[test]
    fn build_rejects_too_many_entries() {
        let blob: Vec<u8> = vec![0u8; 10];
        let entries: Vec<Entry> = (0..TOC_SLOTS + 1)
            .map(|i| entry(&format!("t{i}"), &blob))
            .collect();
        assert!(build_archive(&entries).is_err());
    }

    #[test]
    fn build_rejects_empty_name() {
        assert!(build_archive(&[entry("", &[0u8; 10])]).is_err());
    }

    #[test]
    fn build_rejects_13_byte_name() {
        // 13 name bytes leave no room for the NUL in the 13-byte field.
        assert!(build_archive(&[entry("ABCDEFGH.tga1", &[0u8; 10])]).is_err());
    }

    #[test]
    fn build_rejects_9_byte_blob() {
        assert!(build_archive(&[entry("A.tga", &[0u8; 9])]).is_err());
    }

    // -- upsert ---------------------------------------------------------------

    /// Three-entry fixture with distinct blob sizes (prefix-stripped TGAs so
    /// the result can also go through build_archive).
    fn sample_entries() -> Vec<Entry> {
        vec![
            entry("first.tga", &make_tga(8, 8, 24)[TGA_PREFIX.len()..]),
            entry("TARGET.tga", &make_tga(16, 16, 24)[TGA_PREFIX.len()..]),
            entry("last.tga", &make_tga(4, 4, 32)[TGA_PREFIX.len()..]),
        ]
    }

    #[test]
    fn upsert_replaces_by_exact_name() {
        let mut entries = sample_entries();
        let tga = make_tga(32, 32, 32);
        let outcome = upsert(&mut entries, "TARGET.tga", &tga).unwrap();
        assert_eq!(outcome, UpsertOutcome::Replaced);
        assert_eq!(entries.len(), 3);
        // Order and stored name preserved, blob swapped in.
        assert_eq!(entries[0], sample_entries()[0]);
        assert_eq!(entries[1].name, b"TARGET.tga");
        assert_eq!(entries[1].blob, tga[TGA_PREFIX.len()..]);
        assert_eq!(entries[2], sample_entries()[2]);
    }

    #[test]
    fn upsert_matches_name_case_insensitively() {
        let mut entries = sample_entries();
        let tga = make_tga(32, 32, 24);
        let outcome = upsert(&mut entries, "target.tga", &tga).unwrap();
        assert_eq!(outcome, UpsertOutcome::Replaced);
        assert_eq!(entries.len(), 3);
        // The STORED name wins, not the staged one.
        assert_eq!(entries[1].name, b"TARGET.tga");
        assert_eq!(entries[1].blob, tga[TGA_PREFIX.len()..]);
    }

    #[test]
    fn upsert_with_different_size_blob_shifts_offsets_correctly() {
        let mut entries = sample_entries();
        let tga = make_tga(64, 64, 24); // much larger than the 16x16 it replaces
        upsert(&mut entries, "TARGET.tga", &tga).unwrap();
        let reparsed = parse_archive(&build_archive(&entries).unwrap()).unwrap();
        assert_eq!(reparsed, entries);
        assert_eq!(reparsed[1].blob, tga[TGA_PREFIX.len()..]);
        assert_eq!(reparsed[2].blob, sample_entries()[2].blob);
    }

    #[test]
    fn upsert_appends_when_absent() {
        let mut entries = sample_entries();
        let tga = make_tga(8, 8, 32);
        let outcome = upsert(&mut entries, "new.tga", &tga).unwrap();
        assert_eq!(outcome, UpsertOutcome::Added);
        assert_eq!(entries.len(), 4);
        assert_eq!(entries[3].name, b"new.tga");
        assert_eq!(entries[3].blob, tga[TGA_PREFIX.len()..]);
    }

    #[test]
    fn upsert_rejects_invalid_tga_and_leaves_entries_unchanged() {
        let mut entries = sample_entries();
        let mut tga = make_tga(32, 32, 24);
        tga[2] = 10; // RLE — validate_tga refuses it
        assert!(upsert(&mut entries, "TARGET.tga", &tga).is_err());
        assert_eq!(entries, sample_entries());
    }

    #[test]
    fn upsert_rejects_13_byte_name_on_append() {
        let mut entries = sample_entries();
        let tga = make_tga(8, 8, 24);
        assert!(upsert(&mut entries, "ABCDEFGH.tga1", &tga).is_err());
        assert_eq!(entries, sample_entries());
    }

    #[test]
    fn upsert_rejects_append_when_toc_is_full() {
        let blob: Vec<u8> = vec![0u8; 10];
        let mut entries: Vec<Entry> = (0..TOC_SLOTS)
            .map(|i| entry(&format!("e{i}.t"), &blob))
            .collect();
        let tga = make_tga(8, 8, 24);
        assert!(upsert(&mut entries, "new.tga", &tga).is_err());
        assert_eq!(entries.len(), TOC_SLOTS);
        assert!(entries.iter().all(|e| e.blob == blob));
    }

    #[test]
    fn upsert_replaces_when_toc_is_full() {
        let blob: Vec<u8> = vec![0u8; 10];
        let mut entries: Vec<Entry> = (0..TOC_SLOTS)
            .map(|i| entry(&format!("e{i}.t"), &blob))
            .collect();
        let tga = make_tga(8, 8, 24);
        let outcome = upsert(&mut entries, "e42.t", &tga).unwrap();
        assert_eq!(outcome, UpsertOutcome::Replaced);
        assert_eq!(entries.len(), TOC_SLOTS);
        assert_eq!(entries[42].blob, tga[TGA_PREFIX.len()..]);
    }
}
