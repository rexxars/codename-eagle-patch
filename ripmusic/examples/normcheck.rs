//! Validate the loudness pipeline against a real WAV (host build):
//!   cargo run --example normcheck -- /path/to/track02.wav [/tmp/out.ogg]
//! Prints input vs output integrated LUFS and output peak; optionally encodes.

use ripmusic::{encode::encode_ogg, loudness};
use ebur128::{EbuR128, Mode};

fn lufs(interleaved: &[f32], rate: u32) -> f64 {
    let mut e = EbuR128::new(2, rate, Mode::I).unwrap();
    e.add_frames_f32(interleaved).unwrap();
    e.loudness_global().unwrap()
}

fn main() {
    let mut a = std::env::args().skip(1);
    let wav = a.next().expect("usage: normcheck <in.wav> [out.ogg]");
    let out = a.next();

    let mut reader = hound::WavReader::open(&wav).unwrap();
    let rate = reader.spec().sample_rate;
    let pcm: Vec<i16> = reader.samples::<i16>().map(Result::unwrap).collect();

    let input: Vec<f32> = pcm.iter().map(|&s| s as f32 / 32768.0).collect();
    println!("input  LUFS {:>6.1}", lufs(&input, rate));

    let channels = loudness::normalize_stereo(&pcm, rate);
    let mut inter = Vec::with_capacity(channels[0].len() * 2);
    for i in 0..channels[0].len() {
        inter.push(channels[0][i]);
        inter.push(channels[1][i]);
    }
    let peak = inter.iter().fold(0f32, |m, &s| m.max(s.abs()));
    println!("output LUFS {:>6.1}  peak {:>5.2} dBFS", lufs(&inter, rate), 20.0 * peak.log10());

    if let Some(out) = out {
        encode_ogg(&channels, rate, std::path::Path::new(&out), &[]).unwrap();
        println!("wrote {out}");
    }
}
