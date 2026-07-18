//! Ogg Vorbis encoding via vorbis_rs (bundled aoTuV libvorbis).

use std::error::Error;
use std::fs::File;
use std::io::BufWriter;
use std::num::{NonZeroU32, NonZeroU8};
use std::path::Path;
use vorbis_rs::{VorbisBitrateManagementStrategy, VorbisEncoderBuilder};

const TARGET_QUALITY: f32 = 0.6; // ~ ffmpeg -q:a 6

/// Encode deinterleaved stereo f32 to `path` as Ogg Vorbis (VBR ~q6).
///
/// `tags` are written as Vorbis comments (the Ogg equivalent of ID3 tags), e.g.
/// `[("ARTIST", "..."), ("TITLE", "..."), ("TRACKNUMBER", "1")]`.
pub fn encode_ogg(
    channels: &[Vec<f32>; 2],
    rate: u32,
    path: &Path,
    tags: &[(&str, &str)],
) -> Result<(), Box<dyn Error>> {
    let sink = BufWriter::new(File::create(path)?);
    let mut builder = VorbisEncoderBuilder::new(
        NonZeroU32::new(rate).ok_or("zero sample rate")?,
        NonZeroU8::new(2).ok_or("zero channels")?,
        sink,
    )?;
    builder.bitrate_management_strategy(VorbisBitrateManagementStrategy::QualityVbr {
        target_quality: TARGET_QUALITY,
    });
    for &(tag, value) in tags {
        builder.comment_tag(tag, value)?;
    }
    let mut encoder = builder.build()?;
    encoder.encode_audio_block(channels)?;
    encoder.finish()?;
    Ok(())
}
