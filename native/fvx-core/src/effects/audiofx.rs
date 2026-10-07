use std::collections::HashMap;
use std::f64::consts::{PI, TAU};

use super::{EffectSpec, FxCtx, angle, check, no_pad, popup, slider};
use crate::model::Value;
use crate::render::image::Img;

const CAT: &str = "Audio";
pub const PREFIX: &str = "fx.audio.";
pub const DISTORTION: [&str; 6] = ["Soft Clip", "Hard Clip", "Saturation 1", "Saturation 2", "Tube", "Fuzz"];

fn pass(_: &FxCtx, _: &mut Img) {}

fn spec(id: &'static str, name: &'static str, params: Vec<super::ParamSpec>) -> EffectSpec {
    EffectSpec { id, name, category: CAT, params, render: pass, pad: no_pad }
}

fn pc(id: &'static str, name: &'static str, v: f64) -> super::ParamSpec {
    slider(id, name, v, 0.0, 100.0, 2)
}

pub fn specs() -> Vec<EffectSpec> {
    let mut eq = vec![];
    let bands: [(&str, &str, &str, &str, &str, &str, &str, &str, f64, bool); 3] = [
        ("band1Enable", "Band 1 Enabled", "band1Frequency", "Band 1 Frequency", "band1Bandwidth", "Band 1 Bandwidth", "band1BoostCut", "Band 1 Boost/Cut", 200.0, true),
        ("band2Enable", "Band 2 Enabled", "band2Frequency", "Band 2 Frequency", "band2Bandwidth", "Band 2 Bandwidth", "band2BoostCut", "Band 2 Boost/Cut", 1000.0, false),
        ("band3Enable", "Band 3 Enabled", "band3Frequency", "Band 3 Frequency", "band3Bandwidth", "Band 3 Bandwidth", "band3BoostCut", "Band 3 Boost/Cut", 5000.0, false),
    ];
    for b in bands {
        eq.push(check(b.0, b.1, b.9));
        eq.push(slider(b.2, b.3, b.8, 20.0, 20000.0, 1));
        eq.push(slider(b.4, b.5, 30.0, 1.0, 100.0, 1));
        eq.push(slider(b.6, b.7, 0.0, -24.0, 24.0, 2));
    }
    vec![
        spec("fx.audio.backwards", "Backwards", vec![check("swapChannels", "Swap Channels", false)]),
        spec("fx.audio.bassTreble", "Bass & Treble", vec![slider("bass", "Bass", 0.0, -100.0, 100.0, 2), slider("treble", "Treble", 0.0, -100.0, 100.0, 2)]),
        spec(
            "fx.audio.compressor",
            "Compressor",
            vec![
                slider("threshold", "Threshold (dB)", -16.0, -60.0, 0.0, 2),
                slider("ratio", "Ratio (x:1)", 3.0, 1.0, 30.0, 2),
                slider("knee", "Knee (dB)", 15.0, 0.0, 30.0, 2),
                slider("attack", "Attack (ms)", 6.0, 0.0, 400.0, 2),
                slider("release", "Release (ms)", 440.0, 1.0, 4000.0, 2),
                check("autoRelease", "Auto Release", true),
                slider("makeupGain", "Makeup Gain (dB)", 0.0, -30.0, 30.0, 2),
                slider("outputLimit", "Output Limit (dB)", 0.0, -30.0, 0.0, 2),
            ],
        ),
        spec("fx.audio.delay", "Delay", vec![slider("delayTime", "Delay Time (milliseconds)", 500.0, 1.0, 10000.0, 2), pc("delayAmount", "Delay Amount", 50.0), pc("feedback", "Feedback", 50.0), pc("dryOut", "Dry Out", 75.0), pc("wetOut", "Wet Out", 75.0)]),
        spec(
            "fx.audio.distortion",
            "Distortion",
            vec![
                popup("distortionType", "Type", 0, &DISTORTION),
                pc("drive", "Drive", 25.0),
                slider("gain", "Gain", 25.0, 0.0, 300.0, 2),
                pc("mix", "Mix", 100.0),
                slider("volume", "Volume", 11.0, 0.0, 100.0, 2),
                slider("bitDepth", "Resolution (bit)", 24.0, 1.0, 24.0, 0),
                slider("downsample", "Downsample (x)", 1.0, 1.0, 64.0, 0),
            ],
        ),
        spec(
            "fx.audio.flangeChorus",
            "Flange & Chorus",
            vec![
                slider("voiceSeparationTime", "Voice Separation Time (ms)", 3.0, 0.0, 100.0, 2),
                slider("voices", "Voices", 1.0, 1.0, 8.0, 0),
                slider("modulationRate", "Modulation Rate", 0.4, 0.0, 10.0, 2),
                pc("modulationDepth", "Modulation Depth", 50.0),
                angle("voicePhaseChange", "Voice Phase Change", 0.0),
                check("invertPhase", "Invert Phase", false),
                check("stereoVoices", "Stereo Voices", false),
                pc("dryOut", "Dry Out", 50.0),
                pc("wetOut", "Wet Out", 50.0),
            ],
        ),
        spec("fx.audio.gate", "Gate", vec![slider("threshold", "Threshold (dB)", -60.0, -60.0, 0.0, 2), slider("attack", "Attack (ms)", 10.0, 0.0, 1000.0, 2), slider("hold", "Hold (ms)", 40.0, 0.0, 2500.0, 2), slider("release", "Release (ms)", 220.0, 5.0, 4000.0, 2)]),
        spec("fx.audio.highLowPass", "High-Low Pass", vec![popup("filterOptions", "Filter Options", 0, &["High Pass", "Low Pass"]), slider("cutoffFrequency", "Cutoff Frequency", 2000.0, 20.0, 20000.0, 1), pc("dryOut", "Dry Out", 0.0), pc("wetOut", "Wet Out", 100.0)]),
        spec("fx.audio.modulator", "Modulator", vec![popup("modulationType", "Modulation Type", 0, &["Sine", "Triangle"]), slider("modulationRate", "Modulation Rate", 10.0, 0.0, 100.0, 2), pc("modulationDepth", "Modulation Depth", 5.0), pc("amplitudeModulation", "Amplitude Modulation", 0.0)]),
        spec("fx.audio.parametricEq", "Parametric EQ", eq),
        spec("fx.audio.reverb", "Reverb", vec![slider("reverbTime", "Reverb Time (milliseconds)", 100.0, 1.0, 5000.0, 2), pc("diffusion", "Diffusion", 75.0), pc("decay", "Decay", 25.0), pc("brightness", "Brightness", 10.0), pc("dryOut", "Dry Out", 90.0), pc("wetOut", "Wet Out", 10.0)]),
        spec("fx.audio.stereoMixer", "Stereo Mixer", vec![slider("leftLevel", "Left Level", 100.0, 0.0, 400.0, 2), slider("rightLevel", "Right Level", 100.0, 0.0, 400.0, 2), slider("leftPan", "Left Pan", -100.0, -100.0, 100.0, 2), slider("rightPan", "Right Pan", 100.0, -100.0, 100.0, 2), check("invertPhase", "Invert Phase", false)]),
        spec(
            "fx.audio.tone",
            "Tone",
            vec![
                popup("waveformOptions", "Waveform Options", 0, &["Sine", "Triangle", "Saw", "Square", "White Noise"]),
                slider("frequency1", "Frequency 1", 440.0, 0.0, 22000.0, 2),
                slider("frequency2", "Frequency 2", 493.88, 0.0, 22000.0, 2),
                slider("frequency3", "Frequency 3", 587.33, 0.0, 22000.0, 2),
                slider("frequency4", "Frequency 4", 659.26, 0.0, 22000.0, 2),
                slider("frequency5", "Frequency 5", 783.99, 0.0, 22000.0, 2),
                pc("level", "Level", 20.0),
            ],
        ),
    ]
}

pub fn is_audio(id: &str) -> bool {
    id.starts_with(PREFIX)
}

pub fn makes_sound(id: &str) -> bool {
    id == "fx.audio.tone"
}

pub type Params = HashMap<String, Value>;

fn f(p: &Params, id: &str) -> f64 {
    p.get(id).map(|v| v.num()).unwrap_or(0.0)
}

fn b(p: &Params, id: &str) -> bool {
    p.get(id).map(|v| v.boolean()).unwrap_or(false)
}

fn e(p: &Params, id: &str) -> u32 {
    p.get(id).map(|v| v.index()).unwrap_or(0)
}

fn db(v: f64) -> f64 {
    10f64.powf(v / 20.0)
}

pub fn preroll(id: &str, p: &Params) -> f64 {
    match id {
        "fx.audio.delay" => {
            let fb = (f(p, "feedback") / 100.0).clamp(0.0, 0.99);
            let d = f(p, "delayTime").max(1.0) / 1000.0;
            let repeats = if fb > 1e-3 { (0.001f64.ln() / fb.ln()).ceil() } else { 1.0 };
            (d * (repeats + 1.0)).min(8.0)
        }
        "fx.audio.reverb" => {
            let fb = 0.7 + 0.28 * f(p, "decay") / 100.0;
            let scale = (f(p, "reverbTime") / 100.0).clamp(0.05, 20.0);
            (0.001f64.ln() / fb.ln() * 1640.0 * scale / 44100.0 + 0.1).min(8.0)
        }
        "fx.audio.compressor" => ((f(p, "attack") + f(p, "release") * 4.0) / 1000.0).min(8.0),
        "fx.audio.gate" => ((f(p, "attack") + f(p, "hold") + f(p, "release") * 4.0) / 1000.0).min(8.0),
        "fx.audio.flangeChorus" => (f(p, "voiceSeparationTime") * 9.0 / 1000.0 + 0.05).min(2.0),
        "fx.audio.modulator" => 0.05,
        "fx.audio.bassTreble" | "fx.audio.highLowPass" | "fx.audio.parametricEq" => 0.05,
        _ => 0.0,
    }
}

#[derive(Clone, Copy, Default)]
struct Biquad {
    c: [f64; 5],
    z: [[f64; 4]; 2],
}

impl Biquad {
    fn set(&mut self, b0: f64, b1: f64, b2: f64, a0: f64, a1: f64, a2: f64) {
        self.c = [b0 / a0, b1 / a0, b2 / a0, a1 / a0, a2 / a0];
    }

    fn pass(&mut self, kind: u32, freq: f64, q: f64, rate: f64) {
        let w = TAU * freq.clamp(10.0, rate * 0.49) / rate;
        let (s, cw) = w.sin_cos();
        let al = s / (2.0 * q.max(1e-3));
        if kind == 0 {
            self.set((1.0 + cw) / 2.0, -(1.0 + cw), (1.0 + cw) / 2.0, 1.0 + al, -2.0 * cw, 1.0 - al);
        } else {
            self.set((1.0 - cw) / 2.0, 1.0 - cw, (1.0 - cw) / 2.0, 1.0 + al, -2.0 * cw, 1.0 - al);
        }
    }

    fn peak(&mut self, freq: f64, octaves: f64, gain_db: f64, rate: f64) {
        let a = 10f64.powf(gain_db / 40.0);
        let w = TAU * freq.clamp(10.0, rate * 0.49) / rate;
        let (s, cw) = w.sin_cos();
        let al = s * ((2f64.ln() / 2.0) * octaves.max(0.01) * w / s.max(1e-9)).sinh();
        self.set(1.0 + al * a, -2.0 * cw, 1.0 - al * a, 1.0 + al / a, -2.0 * cw, 1.0 - al / a);
    }

    fn shelf(&mut self, high: bool, freq: f64, gain_db: f64, rate: f64) {
        let a = 10f64.powf(gain_db / 40.0);
        let w = TAU * freq.clamp(10.0, rate * 0.49) / rate;
        let (s, cw) = w.sin_cos();
        let al = s / 2.0 * 2f64.sqrt();
        let k = 2.0 * a.sqrt() * al;
        if high {
            self.set(a * ((a + 1.0) + (a - 1.0) * cw + k), -2.0 * a * ((a - 1.0) + (a + 1.0) * cw), a * ((a + 1.0) + (a - 1.0) * cw - k), (a + 1.0) - (a - 1.0) * cw + k, 2.0 * ((a - 1.0) - (a + 1.0) * cw), (a + 1.0) - (a - 1.0) * cw - k);
        } else {
            self.set(a * ((a + 1.0) - (a - 1.0) * cw + k), 2.0 * a * ((a - 1.0) - (a + 1.0) * cw), a * ((a + 1.0) - (a - 1.0) * cw - k), (a + 1.0) + (a - 1.0) * cw + k, -2.0 * ((a - 1.0) + (a + 1.0) * cw), (a + 1.0) + (a - 1.0) * cw - k);
        }
    }

    fn run(&mut self, ch: usize, x: f64) -> f64 {
        let z = &mut self.z[ch];
        let y = self.c[0] * x + self.c[1] * z[0] + self.c[2] * z[1] - self.c[3] * z[2] - self.c[4] * z[3];
        *z = [x, z[0], y, z[2]];
        y
    }
}

struct Line {
    buf: Vec<f64>,
    at: usize,
}

impl Line {
    fn new(n: usize) -> Line {
        Line { buf: vec![0.0; n.max(2)], at: 0 }
    }

    fn push(&mut self, v: f64) {
        self.at = (self.at + 1) % self.buf.len();
        self.buf[self.at] = v;
    }

    fn tap(&self, d: f64) -> f64 {
        let n = self.buf.len();
        let d = d.clamp(0.0, (n - 2) as f64);
        let i = d.floor() as usize;
        let fr = d - i as f64;
        let a = self.buf[(self.at + n - i) % n];
        let b = self.buf[(self.at + n - i - 1) % n];
        a + (b - a) * fr
    }
}

struct Comb {
    line: Line,
    len: usize,
    lp: f64,
}

struct Allpass {
    line: Line,
    len: usize,
}

enum State {
    None,
    Filters(Vec<Biquad>),
    Delay([Line; 2]),
    Lines([Line; 2]),
    Reverb(Vec<[Comb; 2]>, Vec<[Allpass; 2]>, Biquad),
    Dyn { env: f64, slow: f64, gain: f64, hold: f64 },
    Hold([f64; 2], u32),
}

pub struct Proc {
    id: &'static str,
    rate: f64,
    st: State,
}

const COMBS: [usize; 8] = [1116, 1188, 1277, 1356, 1422, 1491, 1557, 1617];
const ALLPASSES: [usize; 4] = [556, 441, 341, 225];

impl Proc {
    pub fn new(id: &str, rate: u32) -> Option<Proc> {
        let spec = super::all().iter().find(|s| s.id == id && is_audio(s.id))?;
        let r = rate as f64;
        let st = match spec.id {
            "fx.audio.bassTreble" => State::Filters(vec![Biquad::default(); 2]),
            "fx.audio.highLowPass" => State::Filters(vec![Biquad::default(); 2]),
            "fx.audio.parametricEq" => State::Filters(vec![Biquad::default(); 3]),
            "fx.audio.delay" => State::Delay([Line::new((r * 10.1) as usize), Line::new((r * 10.1) as usize)]),
            "fx.audio.flangeChorus" | "fx.audio.modulator" => State::Lines([Line::new((r * 0.9) as usize), Line::new((r * 0.9) as usize)]),
            "fx.audio.reverb" => State::Reverb(vec![], vec![], Biquad::default()),
            "fx.audio.compressor" | "fx.audio.gate" => State::Dyn { env: 0.0, slow: 0.0, gain: 1.0, hold: 0.0 },
            "fx.audio.distortion" => State::Hold([0.0; 2], 0),
            _ => State::None,
        };
        Some(Proc { id: spec.id, rate: r, st })
    }

    pub fn block(&mut self, p: &Params, buf: &mut [f32], t0: f64) {
        let rate = self.rate;
        let n = buf.len() / 2;
        match (self.id, &mut self.st) {
            ("fx.audio.backwards", _) => {
                if b(p, "swapChannels") {
                    buf.chunks_exact_mut(2).for_each(|s| s.swap(0, 1));
                }
            }
            ("fx.audio.bassTreble", State::Filters(fl)) => {
                fl[0].shelf(false, 200.0, f(p, "bass") * 0.12, rate);
                fl[1].shelf(true, 4000.0, f(p, "treble") * 0.12, rate);
                for (i, s) in buf.iter_mut().enumerate() {
                    let ch = i % 2;
                    let v = fl[0].run(ch, *s as f64);
                    *s = fl[1].run(ch, v) as f32;
                }
            }
            ("fx.audio.highLowPass", State::Filters(fl)) => {
                fl[0].pass(e(p, "filterOptions"), f(p, "cutoffFrequency"), std::f64::consts::FRAC_1_SQRT_2, rate);
                let (dry, wet) = (f(p, "dryOut") / 100.0, f(p, "wetOut") / 100.0);
                for (i, s) in buf.iter_mut().enumerate() {
                    let x = *s as f64;
                    *s = (x * dry + fl[0].run(i % 2, x) * wet) as f32;
                }
            }
            ("fx.audio.parametricEq", State::Filters(fl)) => {
                let on: Vec<bool> = (1..=3).map(|k| b(p, &format!("band{k}Enable")) && f(p, &format!("band{k}BoostCut")).abs() > 1e-6).collect();
                for k in 0..3 {
                    if on[k] {
                        let n = k + 1;
                        fl[k].peak(f(p, &format!("band{n}Frequency")), f(p, &format!("band{n}Bandwidth")) / 30.0, f(p, &format!("band{n}BoostCut")), rate);
                    }
                }
                for (i, s) in buf.iter_mut().enumerate() {
                    let mut v = *s as f64;
                    for k in 0..3 {
                        if on[k] {
                            v = fl[k].run(i % 2, v);
                        }
                    }
                    *s = v as f32;
                }
            }
            ("fx.audio.delay", State::Delay(lines)) => {
                let d = (f(p, "delayTime") / 1000.0 * rate).max(1.0);
                let (amt, fb, dry, wet) = (f(p, "delayAmount") / 100.0, (f(p, "feedback") / 100.0).min(0.99), f(p, "dryOut") / 100.0, f(p, "wetOut") / 100.0);
                for s in buf.chunks_exact_mut(2) {
                    for ch in 0..2 {
                        let x = s[ch] as f64;
                        let y = lines[ch].tap(d - 1.0);
                        lines[ch].push(x * amt + y * fb);
                        s[ch] = (x * dry + y * wet) as f32;
                    }
                }
            }
            ("fx.audio.flangeChorus", State::Lines(lines)) => {
                let sep = f(p, "voiceSeparationTime") / 1000.0 * rate;
                let voices = f(p, "voices").round().clamp(1.0, 8.0) as usize;
                let lfo = f(p, "modulationRate");
                let depth = f(p, "modulationDepth") / 100.0;
                let vph = f(p, "voicePhaseChange").to_radians();
                let sign = if b(p, "invertPhase") { -1.0 } else { 1.0 };
                let stereo = b(p, "stereoVoices");
                let (dry, wet) = (f(p, "dryOut") / 100.0, f(p, "wetOut") / 100.0);
                for (k, s) in buf.chunks_exact_mut(2).enumerate() {
                    let t = t0 + k as f64 / rate;
                    let x = [s[0] as f64, s[1] as f64];
                    lines[0].push(x[0]);
                    lines[1].push(x[1]);
                    let mut acc = [0.0f64; 2];
                    for v in 0..voices {
                        let ph = TAU * lfo * t + vph * v as f64;
                        let d = sep * (v as f64 + 1.0) * (1.0 + depth * 0.5 * (1.0 + ph.sin()));
                        for (ch, a) in acc.iter_mut().enumerate() {
                            if stereo && voices > 1 && v % 2 != ch {
                                continue;
                            }
                            *a += lines[ch].tap(d);
                        }
                    }
                    let norm = if stereo && voices > 1 { 2.0 / voices as f64 } else { 1.0 / voices as f64 };
                    for ch in 0..2 {
                        s[ch] = (x[ch] * dry + acc[ch] * norm * wet * sign) as f32;
                    }
                }
            }
            ("fx.audio.modulator", State::Lines(lines)) => {
                let lfo = f(p, "modulationRate");
                let depth = f(p, "modulationDepth") / 100.0 * 0.01 * rate;
                let am = f(p, "amplitudeModulation") / 100.0;
                let tri = e(p, "modulationType") == 1;
                for (k, s) in buf.chunks_exact_mut(2).enumerate() {
                    let t = t0 + k as f64 / rate;
                    let ph = (lfo * t).rem_euclid(1.0);
                    let m = if tri { 1.0 - 4.0 * (ph - 0.5).abs() } else { (ph * TAU).sin() };
                    let d = depth * (1.0 + m);
                    let g = 1.0 - am * 0.5 * (1.0 + m);
                    for ch in 0..2 {
                        lines[ch].push(s[ch] as f64);
                        s[ch] = (if depth > 0.0 { lines[ch].tap(d) } else { s[ch] as f64 } * g) as f32;
                    }
                }
            }
            ("fx.audio.reverb", State::Reverb(combs, aps, damp)) => {
                let scale = (f(p, "reverbTime") / 100.0).clamp(0.05, 20.0) * rate / 44100.0;
                if combs.is_empty() {
                    let max_len = |l: usize| (l as f64 * 20.0 * rate / 44100.0) as usize + 32;
                    *combs = COMBS.iter().map(|&l| [0, 23].map(|o| Comb { line: Line::new(max_len(l + o)), len: l + o, lp: 0.0 })).collect();
                    *aps = ALLPASSES.iter().map(|&l| [0, 23].map(|o| Allpass { line: Line::new(max_len(l + o)), len: l + o })).collect();
                }
                let decay = f(p, "decay") / 100.0;
                let fb = 0.7 + 0.28 * decay;
                let bright = f(p, "brightness") / 100.0;
                let dmp = 0.05 + 0.6 * (1.0 - bright);
                let diff = 0.3 + 0.4 * f(p, "diffusion") / 100.0;
                let (dry, wet) = (f(p, "dryOut") / 100.0, f(p, "wetOut") / 100.0);
                damp.pass(1, 2000.0 + 16000.0 * bright, std::f64::consts::FRAC_1_SQRT_2, rate);
                for s in buf.chunks_exact_mut(2) {
                    let x = (s[0] as f64 + s[1] as f64) * 0.5 * 0.2;
                    for ch in 0..2 {
                        let mut y = 0.0;
                        for c in combs.iter_mut() {
                            let cb = &mut c[ch];
                            let o = cb.line.tap((cb.len as f64 * scale).max(1.0) - 1.0);
                            cb.lp = o * (1.0 - dmp) + cb.lp * dmp;
                            cb.line.push(x + cb.lp * fb);
                            y += o;
                        }
                        for a in aps.iter_mut() {
                            let ap = &mut a[ch];
                            let bo = ap.line.tap((ap.len as f64 * scale.sqrt()).max(1.0) - 1.0);
                            ap.line.push(y + bo * diff);
                            y = bo - y * diff;
                        }
                        let y = damp.run(ch, y);
                        s[ch] = (s[ch] as f64 * dry + y * wet) as f32;
                    }
                }
            }
            ("fx.audio.compressor", State::Dyn { env, slow, gain, .. }) => {
                let th = f(p, "threshold");
                let ratio = f(p, "ratio").max(1.0);
                let knee = f(p, "knee").max(0.0);
                let att = (-1.0 / (f(p, "attack").max(0.01) / 1000.0 * rate)).exp();
                let rel_ms = f(p, "release").max(1.0);
                let auto = b(p, "autoRelease");
                let makeup = db(f(p, "makeupGain"));
                let limit = db(f(p, "outputLimit"));
                let slow_c = (-1.0 / (0.4 * rate)).exp();
                for s in buf.chunks_exact_mut(2) {
                    let x = (s[0].abs().max(s[1].abs())) as f64;
                    *slow = x + (*slow - x) * slow_c;
                    let rel_eff = if auto { rel_ms * (0.2 + 0.8 * (*slow / (*env).max(1e-9)).min(1.0)) } else { rel_ms };
                    let rel = (-1.0 / (rel_eff / 1000.0 * rate)).exp();
                    let c = if x > *env { att } else { rel };
                    *env = x + (*env - x) * c;
                    let lv = 20.0 * env.max(1e-9).log10();
                    let over = lv - th;
                    let red = if 2.0 * over < -knee {
                        0.0
                    } else if knee > 0.0 && 2.0 * over.abs() <= knee {
                        (1.0 / ratio - 1.0) * (over + knee / 2.0).powi(2) / (2.0 * knee)
                    } else {
                        (1.0 / ratio - 1.0) * over
                    };
                    *gain = db(red) * makeup;
                    for v in s.iter_mut() {
                        *v = ((*v as f64) * *gain).clamp(-limit, limit) as f32;
                    }
                }
            }
            ("fx.audio.gate", State::Dyn { env, gain, hold, .. }) => {
                let th = db(f(p, "threshold"));
                let att = 1.0 / (f(p, "attack").max(0.01) / 1000.0 * rate);
                let rel = 1.0 / (f(p, "release").max(1.0) / 1000.0 * rate);
                let hold_n = f(p, "hold") / 1000.0 * rate;
                let det = (-1.0 / (0.002 * rate)).exp();
                for s in buf.chunks_exact_mut(2) {
                    let x = (s[0].abs().max(s[1].abs())) as f64;
                    *env = if x > *env { x } else { x + (*env - x) * det };
                    if *env >= th {
                        *hold = hold_n;
                        *gain = (*gain + att).min(1.0);
                    } else if *hold > 0.0 {
                        *hold -= 1.0;
                    } else {
                        *gain = (*gain - rel).max(0.0);
                    }
                    for v in s.iter_mut() {
                        *v = (*v as f64 * *gain) as f32;
                    }
                }
            }
            ("fx.audio.distortion", State::Hold(held, count)) => {
                let ty = e(p, "distortionType");
                let pre = f(p, "gain") / 25.0 * (1.0 + f(p, "drive") / 100.0 * 30.0);
                let mix = f(p, "mix") / 100.0;
                let vol = f(p, "volume") / 11.0;
                let bits = f(p, "bitDepth").round().clamp(1.0, 24.0);
                let q = 2f64.powf(bits - 1.0);
                let ds = f(p, "downsample").round().clamp(1.0, 64.0) as u32;
                for s in buf.chunks_exact_mut(2) {
                    if *count == 0 {
                        *held = [s[0] as f64, s[1] as f64];
                    }
                    *count = (*count + 1) % ds;
                    for ch in 0..2 {
                        let x = held[ch] * pre;
                        let y = match ty {
                            1 => x.clamp(-1.0, 1.0),
                            2 => x / (1.0 + x.abs()),
                            3 => {
                                let c = x.clamp(-1.5, 1.5);
                                c - c * c * c / 6.75
                            }
                            4 => {
                                if x >= 0.0 { 1.0 - (-x).exp() } else { -0.8 * (1.0 - (x * 1.25).exp()) }
                            }
                            5 => x.signum() * (1.0 - (-(x.abs() * 3.0)).exp()),
                            _ => x.tanh(),
                        };
                        let y = if bits < 24.0 { (y * q).round() / q } else { y };
                        s[ch] = ((s[ch] as f64 * (1.0 - mix) + y * mix) * vol) as f32;
                    }
                }
            }
            ("fx.audio.stereoMixer", _) => {
                let (ll, rl) = (f(p, "leftLevel") / 100.0, f(p, "rightLevel") / 100.0);
                let pan = |v: f64| {
                    let a = (v.clamp(-100.0, 100.0) / 100.0 + 1.0) * PI / 4.0;
                    (a.cos() * 2f64.sqrt(), a.sin() * 2f64.sqrt())
                };
                let (lp, rp) = (pan(f(p, "leftPan")), pan(f(p, "rightPan")));
                let lp = (lp.0.min(1.0), lp.1.min(1.0));
                let rp = (rp.0.min(1.0), rp.1.min(1.0));
                let sign = if b(p, "invertPhase") { -1.0 } else { 1.0 };
                for s in buf.chunks_exact_mut(2) {
                    let (l, r) = (s[0] as f64 * ll, s[1] as f64 * rl);
                    s[0] = ((l * lp.0 + r * rp.0) * sign) as f32;
                    s[1] = ((l * lp.1 + r * rp.1) * sign) as f32;
                }
            }
            ("fx.audio.tone", _) => {
                let wave = e(p, "waveformOptions");
                let fr: Vec<f64> = (1..=5).map(|k| f(p, &format!("frequency{k}"))).filter(|v| *v > 0.0).collect();
                let lv = f(p, "level") / 100.0;
                if fr.is_empty() && wave != 4 {
                    return;
                }
                let norm = lv / (fr.len().max(1) as f64).sqrt();
                for k in 0..n {
                    let t = t0 + k as f64 / rate;
                    let v = if wave == 4 {
                        let i = (t * rate).round() as i64;
                        (super::util::rand01((i & 0x7fff_ffff) as i32, (i >> 31) as i32, 5, 17) as f64 * 2.0 - 1.0) * lv
                    } else {
                        fr.iter()
                            .map(|fq| {
                                let ph = (fq * t).rem_euclid(1.0);
                                match wave {
                                    1 => 1.0 - 4.0 * (ph - 0.5).abs(),
                                    2 => ph * 2.0 - 1.0,
                                    3 => if ph < 0.5 { 1.0 } else { -1.0 },
                                    _ => (ph * TAU).sin(),
                                }
                            })
                            .sum::<f64>()
                            * norm
                    };
                    buf[k * 2] += v as f32;
                    buf[k * 2 + 1] += v as f32;
                }
            }
            _ => {}
        }
    }
}
