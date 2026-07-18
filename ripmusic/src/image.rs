//! Rip audio tracks from a disc image file (CUE/BIN or CloneCD IMG) instead of a
//! physical drive. The image is raw 2352-byte sectors; the `.cue` sheet gives each
//! track's type and start (INDEX 01, MSF). Audio tracks are interleaved 16-bit LE
//! stereo PCM - the same bytes a physical raw read returns.
//!
//! Note: a `.iso` is the data track only (no audio) - callers should point at the
//! `.cue`/`.img`, not the `.iso`.

use crate::cdrom::Track;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

const SECTOR: u64 = 2352; // raw CD sector (audio and, in a CloneCD/BIN image, data too)

pub struct CueImage {
    pub img: PathBuf,
    pub tracks: Vec<Track>,
}

/// Parse a `.cue` sheet next to its image into a track list.
pub fn parse_cue(cue: &Path) -> io::Result<CueImage> {
    let text = std::fs::read_to_string(cue)?;
    let dir = cue.parent().unwrap_or_else(|| Path::new("."));
    let mut img: Option<PathBuf> = None;
    let mut raw: Vec<(u8, bool, u32)> = Vec::new(); // (track number, is_audio, start LBA)
    let mut cur: Option<(u8, bool)> = None;

    for line in text.lines() {
        let line = line.trim();
        let mut w = line.split_whitespace();
        match w.next() {
            Some("FILE") => {
                if let (Some(a), Some(b)) = (line.find('"'), line.rfind('"')) {
                    if b > a {
                        img = Some(dir.join(&line[a + 1..b]));
                    }
                }
            }
            Some("TRACK") => {
                let num = w.next().and_then(|s| s.parse().ok()).unwrap_or(0u8);
                let is_audio = w.next().is_some_and(|t| t.eq_ignore_ascii_case("AUDIO"));
                cur = Some((num, is_audio));
            }
            Some("INDEX") => {
                let idx = w.next().unwrap_or("");
                if idx == "1" || idx == "01" {
                    if let (Some((num, is_audio)), Some(lba)) =
                        (cur, w.next().and_then(msf_to_lba))
                    {
                        raw.push((num, is_audio, lba));
                    }
                }
            }
            _ => {}
        }
    }

    let img = img.ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "no FILE entry in cue sheet"))?;
    let total = (std::fs::metadata(&img)?.len() / SECTOR) as u32;
    raw.sort_by_key(|&(_, _, lba)| lba);

    let tracks = raw
        .iter()
        .enumerate()
        .map(|(i, &(number, is_audio, start_lba))| Track {
            number,
            start_lba,
            // a track runs until the next track's start, or the image end
            end_lba: raw.get(i + 1).map_or(total, |&(_, _, l)| l),
            is_audio,
        })
        .collect();
    Ok(CueImage { img, tracks })
}

/// Read a track's audio from the image as interleaved 16-bit LE stereo PCM.
pub fn read_audio(img: &Path, start_lba: u32, end_lba: u32) -> io::Result<Vec<i16>> {
    let mut f = std::fs::File::open(img)?;
    f.seek(SeekFrom::Start(start_lba as u64 * SECTOR))?;
    let mut buf = vec![0u8; (end_lba - start_lba) as usize * SECTOR as usize];
    f.read_exact(&mut buf)?;
    Ok(buf.chunks_exact(2).map(|c| i16::from_le_bytes([c[0], c[1]])).collect())
}

/// File-relative MSF (`MM:SS:FF`) to LBA. No 150-sector lead-in offset: in an image
/// file track 1 starts at byte 0 = `00:00:00`.
fn msf_to_lba(s: &str) -> Option<u32> {
    let mut p = s.split(':');
    let m: u32 = p.next()?.parse().ok()?;
    let sec: u32 = p.next()?.parse().ok()?;
    let f: u32 = p.next()?.parse().ok()?;
    if p.next().is_some() {
        return None;
    }
    Some((m * 60 + sec) * 75 + f)
}
