//! All sound is synthesised here; tspace ships no audio files.
//!
//! One-shot effects are rendered once at start-up into sample buffers. One
//! endless source plays your own engine: a low hum while thrusting that
//! roars when the afterburner is on. Panzer's layout, tspace's sounds.
//!
//! Without an audio device, or with `--mute`, every call does nothing.

use rodio::buffer::SamplesBuffer;
use rodio::{MixerDeviceSink, Source};
use std::f32::consts::TAU;
use std::num::NonZero;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::time::{Duration, Instant};

const SR: u32 = 44_100;
const SRF: f32 = SR as f32;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Sfx {
    Gun,
    GunFar,
    Bomb,
    Blast,
    Hit,
    Death,
    MineSet,
    Pickup,
    Spawn,
    Bounce,
    Select,
    Win,
    Low,
}

const ALL_SFX: [Sfx; 13] = [Sfx::Gun, Sfx::GunFar, Sfx::Bomb, Sfx::Blast, Sfx::Hit, Sfx::Death, Sfx::MineSet, Sfx::Pickup, Sfx::Spawn, Sfx::Bounce, Sfx::Select, Sfx::Win, Sfx::Low];

pub struct Audio {
    // the sink must outlive everything playing on its mixer
    sink: Option<MixerDeviceSink>,
    bank: Vec<SamplesBuffer>,
    engine: Arc<AtomicU8>,
    silent: Arc<AtomicBool>,
    pub muted: bool,
    last: [Option<Instant>; ALL_SFX.len()],
}

impl Audio {
    pub fn new(muted: bool) -> Audio {
        let sink = if muted {
            None
        } else {
            match rodio::DeviceSinkBuilder::open_default_sink() {
                Ok(mut s) => {
                    s.log_on_drop(false);
                    Some(s)
                }
                Err(e) => {
                    log::warn!("no audio: {e}");
                    None
                }
            }
        };
        let engine = Arc::new(AtomicU8::new(0));
        let silent = Arc::new(AtomicBool::new(false));
        let bank = if sink.is_some() { ALL_SFX.iter().map(|&s| buffer(render_sfx(s))).collect() } else { Vec::new() };
        if let Some(s) = &sink {
            s.mixer().add(Engine::new(engine.clone(), silent.clone()));
        }
        Audio { sink, bank, engine, silent, muted, last: [None; ALL_SFX.len()] }
    }

    pub fn available(&self) -> bool {
        self.sink.is_some()
    }

    pub fn play(&mut self, sfx: Sfx, volume: f32) {
        if self.muted {
            return;
        }
        let Some(sink) = &self.sink else { return };
        let i = ALL_SFX.iter().position(|&s| s == sfx).unwrap_or(0);
        let now = Instant::now();
        if self.last[i].is_some_and(|t| now.duration_since(t) < Duration::from_millis(40)) {
            return;
        }
        self.last[i] = Some(now);
        sink.mixer().add(self.bank[i].clone().amplify(volume));
    }

    /// Your engine: 0 off, 1 thrust, 2 afterburner.
    pub fn set_engine(&mut self, level: u8) {
        self.engine.store(level, Ordering::Relaxed);
    }

    pub fn toggle_mute(&mut self) {
        self.muted = !self.muted;
        self.silent.store(self.muted, Ordering::Relaxed);
    }
}

fn buffer(samples: Vec<f32>) -> SamplesBuffer {
    SamplesBuffer::new(NonZero::new(1).unwrap(), NonZero::new(SR).unwrap(), samples)
}

struct Noise(u32);

impl Noise {
    fn next(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        (self.0 as f32 / u32::MAX as f32) * 2.0 - 1.0
    }
}

fn midi(n: f32) -> f32 {
    440.0 * 2f32.powf((n - 69.0) / 12.0)
}

fn tri(ph: f32) -> f32 {
    let p = ph - ph.floor();
    4.0 * (p - 0.5).abs() - 1.0
}

fn square(ph: f32) -> f32 {
    if ph - ph.floor() < 0.5 { 1.0 } else { -1.0 }
}

/// `dur` seconds of samples from `f(t, noise)`.
fn synth(dur: f32, mut f: impl FnMut(f32, &mut Noise) -> f32) -> Vec<f32> {
    let mut n = Noise(0x1234_5678);
    (0..(dur * SRF) as usize).map(|i| f(i as f32 / SRF, &mut n)).collect()
}

/// A little tune: (midi note, seconds) with a soft attack.
fn tune(notes: &[(f32, f32)], gain: f32, wave: fn(f32) -> f32) -> Vec<f32> {
    let mut out = Vec::new();
    let mut ph = 0.0f32;
    for &(n, d) in notes {
        let len = (d * SRF) as usize;
        for i in 0..len {
            let t = i as f32 / SRF;
            if n <= 0.0 {
                out.push(0.0); // a rest
                continue;
            }
            ph += midi(n) / SRF;
            let env = (t / 0.01).min(1.0) * ((d - t) / 0.03).clamp(0.0, 1.0);
            out.push(wave(ph) * env * gain);
        }
    }
    out
}

fn render_sfx(s: Sfx) -> Vec<f32> {
    match s {
        // a short downward laser chirp
        Sfx::Gun | Sfx::GunFar => {
            let far = s == Sfx::GunFar;
            let mut ph = 0.0f32;
            synth(0.09, |t, _| {
                ph += (1900.0 * (-t * 22.0).exp() + 300.0) / SRF;
                square(ph) * (-t * 38.0).exp() * if far { 0.08 } else { 0.18 }
            })
        }
        Sfx::Bomb => {
            let (mut ph, mut lp) = (0.0f32, 0.0f32);
            synth(0.3, |t, n| {
                ph += (420.0 * (-t * 6.0).exp() + 90.0) / SRF;
                lp += (n.next() - lp) * 0.15;
                (tri(ph) * 0.5 + lp * 0.5) * (-t * 9.0).exp() * 0.6
            })
        }
        Sfx::Blast => {
            let (mut ph, mut lp, mut lp2) = (0.0f32, 0.0f32, 0.0f32);
            synth(1.2, |t, n| {
                ph += (34.0 + 70.0 * (-t * 7.0).exp()) / SRF;
                lp += (n.next() - lp) * (0.03 + 0.35 * (-t * 6.0).exp());
                lp2 += (lp - lp2) * 0.5;
                (lp2 * 1.7 * (-t * 3.0).exp() + (ph * TAU).sin() * 0.6 * (-t * 5.0).exp()).clamp(-1.0, 1.0) * 0.85
            })
        }
        Sfx::Hit => synth(0.07, |t, n| (n.next() * 0.5 + (TAU * 1200.0 * t).sin() * 0.5) * (-t * 60.0).exp() * 0.35),
        Sfx::Death => {
            let (mut ph, mut lp) = (0.0f32, 0.0f32);
            synth(1.4, |t, n| {
                ph += (700.0 * (-t * 2.5).exp() + 40.0) / SRF;
                lp += (n.next() - lp) * 0.08;
                ((ph * TAU).sin() * 0.4 * (-t * 2.0).exp() + lp * 1.2 * (-t * 2.2).exp()).clamp(-1.0, 1.0) * 0.8
            })
        }
        Sfx::MineSet => synth(0.16, |t, _| {
            let f = if t < 0.07 { 660.0 } else { 440.0 };
            tri(f * t) * (-(t % 0.08) * 30.0).exp() * 0.3
        }),
        Sfx::Pickup => tune(&[(76.0, 0.05), (83.0, 0.05), (88.0, 0.12)], 0.2, tri),
        Sfx::Spawn => {
            let mut ph = 0.0f32;
            synth(0.35, |t, _| {
                ph += (200.0 + 900.0 * t / 0.35) / SRF;
                tri(ph) * (t / 0.05).min(1.0) * (1.0 - t / 0.35) * 0.18
            })
        }
        Sfx::Bounce => synth(0.08, |t, n| ((TAU * 180.0 * t).sin() * 0.6 + n.next() * 0.4) * (-t * 50.0).exp() * 0.3),
        Sfx::Select => tune(&[(81.0, 0.04)], 0.18, tri),
        // two short low beeps: energy is running out
        Sfx::Low => tune(&[(64.0, 0.07), (0.0, 0.05), (64.0, 0.07)], 0.16, square),
        Sfx::Win => tune(&[(72.0, 0.12), (76.0, 0.12), (79.0, 0.12), (84.0, 0.5)], 0.22, square),
    }
}

/// The endless engine voice: filtered noise plus a low tone, following
/// the level the game sets.
struct Engine {
    level: Arc<AtomicU8>,
    silent: Arc<AtomicBool>,
    noise: Noise,
    lp: f32,
    ph: f32,
    gain: f32,
}

impl Engine {
    fn new(level: Arc<AtomicU8>, silent: Arc<AtomicBool>) -> Engine {
        Engine { level, silent, noise: Noise(0x0bad_cafe), lp: 0.0, ph: 0.0, gain: 0.0 }
    }
}

impl Iterator for Engine {
    type Item = f32;
    fn next(&mut self) -> Option<f32> {
        let level = if self.silent.load(Ordering::Relaxed) { 0 } else { self.level.load(Ordering::Relaxed) };
        let target = [0.0, 0.10, 0.22][level.min(2) as usize];
        self.gain += (target - self.gain) * 0.0008;
        let cutoff = if level == 2 { 0.12 } else { 0.05 };
        self.lp += (self.noise.next() - self.lp) * cutoff;
        self.ph += if level == 2 { 72.0 } else { 55.0 } / SRF;
        Some((self.lp * 1.4 + (self.ph * TAU).sin() * 0.3) * self.gain)
    }
}

impl Source for Engine {
    fn current_span_len(&self) -> Option<usize> {
        None
    }
    fn channels(&self) -> rodio::ChannelCount {
        NonZero::new(1).unwrap()
    }
    fn sample_rate(&self) -> rodio::SampleRate {
        NonZero::new(SR).unwrap()
    }
    fn total_duration(&self) -> Option<Duration> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sounds_render_and_stay_in_range() {
        for s in ALL_SFX {
            let v = render_sfx(s);
            assert!(!v.is_empty(), "{s:?}");
            assert!(v.iter().all(|x| x.is_finite() && x.abs() <= 1.0), "{s:?} out of range");
        }
        let mut e = Engine::new(Arc::new(AtomicU8::new(2)), Arc::new(AtomicBool::new(false)));
        assert!((0..SR).filter_map(|_| e.next()).all(|x| x.abs() <= 1.0));
    }
}
