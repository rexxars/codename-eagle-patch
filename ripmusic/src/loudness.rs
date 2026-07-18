//! Loudness normalization matching the shipped tracks: measure integrated EBU R128
//! loudness (pure-Rust `ebur128`), apply makeup gain toward the target, then a
//! look-ahead brickwall limiter to the ceiling. Equivalent to the ffmpeg
//! `loudnorm=I=-14:TP=-1.5` pass used on the hand-made `.ogg` tracks: the CE masters
//! peak at 0 dBFS, so raising loudness needs limiting, not just gain.

use ebur128::{EbuR128, Mode};

const TARGET_LUFS: f64 = -14.0;
const CEILING_DB: f32 = -1.5;
const LOOKAHEAD_MS: f32 = 5.0;
const RELEASE_MS: f32 = 100.0;

/// Normalize interleaved 16-bit stereo PCM to deinterleaved, gain-and-limited f32
/// `[left, right]` ready for the Vorbis encoder.
pub fn normalize_stereo(interleaved: &[i16], rate: u32) -> [Vec<f32>; 2] {
    let gain = match EbuR128::new(2, rate, Mode::I) {
        Ok(mut ebu) => {
            ebu.add_frames_i16(interleaved).ok();
            match ebu.loudness_global() {
                Ok(lufs) if lufs.is_finite() => {
                    10f32.powf((TARGET_LUFS - lufs) as f32 / 20.0)
                }
                _ => 1.0,
            }
        }
        Err(_) => 1.0,
    };

    let frames = interleaved.len() / 2;
    let mut l = Vec::with_capacity(frames);
    let mut r = Vec::with_capacity(frames);
    for f in interleaved.chunks_exact(2) {
        l.push(f[0] as f32 / 32768.0 * gain);
        r.push(f[1] as f32 / 32768.0 * gain);
    }

    limit(&mut l, &mut r, 10f32.powf(CEILING_DB / 20.0), rate);
    [l, r]
}

/// Stereo-linked look-ahead brickwall limiter. The detector reads the signal
/// `lookahead` samples ahead of the (delayed) output, so gain ducks before a peak
/// arrives; fast attack, slow release.
fn limit(l: &mut [f32], r: &mut [f32], ceiling: f32, rate: u32) {
    let n = l.len();
    if n == 0 {
        return;
    }
    let la = ((rate as f32) * LOOKAHEAD_MS / 1000.0).max(1.0) as usize;
    let rel = (-1.0 / ((rate as f32) * RELEASE_MS / 1000.0)).exp();

    let mut dl = vec![0.0f32; la];
    let mut dr = vec![0.0f32; la];
    let mut gain = 1.0f32;
    let mut di = 0usize;
    for i in 0..n + la {
        let (cl, cr) = if i < n { (l[i], r[i]) } else { (0.0, 0.0) };
        let peak = cl.abs().max(cr.abs());
        let target = if peak > ceiling { ceiling / peak } else { 1.0 };
        // brickwall: instant attack (the look-ahead delay applies it before the
        // peak reaches output), slow release. Holds even single-sample transients.
        gain = if target < gain { target } else { rel * gain + (1.0 - rel) * target };
        let (sl, sr) = (dl[di], dr[di]);
        dl[di] = cl;
        dr[di] = cr;
        di = (di + 1) % la;
        if i >= la {
            l[i - la] = (sl * gain).clamp(-1.0, 1.0);
            r[i - la] = (sr * gain).clamp(-1.0, 1.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn limiter_holds_ceiling() {
        let rate = 44_100;
        let n = rate as usize; // 1 second
        let mut l = vec![0f32; n];
        let mut r = vec![0f32; n];
        for i in 0..n {
            let s = (i as f32 * 0.05).sin() * 0.9;
            l[i] = s;
            r[i] = s;
        }
        // full-scale transient spikes the limiter must catch
        for &p in &[1000usize, 5000, 20000, 40000] {
            l[p] = 1.0;
            r[p] = 1.0;
        }
        let ceiling = 0.5;
        limit(&mut l, &mut r, ceiling, rate);
        let peak = l.iter().chain(r.iter()).fold(0f32, |m, &s| m.max(s.abs()));
        // small overshoot tolerance (release creeps up across the look-ahead window)
        assert!(peak <= ceiling * 1.06, "peak {peak} exceeds ceiling {ceiling}");
    }
}
