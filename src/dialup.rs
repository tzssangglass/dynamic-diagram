//! Faithful port of fazamhd.com's DialupSim: a dial-up handshake in six phases
//! (dial tone → answered → FSK bursts → probing → equalizer training → connected).
//! Original syncs to a real audio recording; we drive the same deterministic
//! waveform math with our own clock (no audio). Phase boundaries verbatim.

use crate::frame::*;

const TOTAL_S: f64 = 28.595374; // recording length
const WINDOW_S: f64 = 2.2; // waveform window (original: S)
pub const DURATION: u64 = (TOTAL_S * 1000.0) as u64 + 1000;

// phase boundaries in seconds (original frac * 28.595374)
const PHASE_T: [f64; 6] = [0.0, 4.9, 6.8, 13.0, 15.0, 26.7];
const PHASES: [(&str, &str); 6] = [
    ("dial tone & dialing", "The modem picks up the line, confirms it has a dial tone, and dials the ISP's number as DTMF (Dual-Tone Multi-Frequency) touch-tones, the same tones a telephone keypad makes."),
    ("call answered", "The far end's modem picks up and holds a steady tone, telling your modem a modem, not a person, answered."),
    ("capability handshake", "The two modems exchange low-speed FSK bursts, digital, just slow enough to survive an unoptimized line, listing which modulation standards, compression, and error-correction each one supports."),
    ("modulation negotiation", "Having agreed on a shared standard, both sides probe this specific phone line, a noisy, band-limited, decades-old copper pair, to measure how many bits per second it can actually carry today."),
    ("equalizer training", "A burst of full-spectrum noise trains each modem's equalizer and echo canceller to the exact distortions of this one line, the last calibration before real data can flow."),
    ("connected", "The speaker goes silent. Every hertz of bandwidth the line can carry is now spent moving your data, not letting you listen to it."),
];

fn phase_at(t: f64) -> usize {
    let mut p = 0;
    for (i, &b) in PHASE_T.iter().enumerate() {
        if t >= b {
            p = i;
        }
    }
    p
}

// original waveform synth, verbatim
fn u(e: f64) -> f64 {
    (e.sin() * 0.0
        + (e * 97.0).sin()
        + (e * 53.3 + 1.7).sin() * 0.8
        + (e * 181.0 + 0.4).sin() * 0.9
        + (e * 139.7 + 2.9).sin() * 0.6
        + (e * 211.3 + 4.2).sin() * 0.7
        + (e * 71.9 + 0.9).sin() * 0.5)
        / 4.5
}

fn burst(e: f64, t: f64, n: f64, r: f64) -> f64 {
    let i = 1.0 / t;
    let a = (e * t).floor();
    let o = (a / 2.0).ceil();
    let s = if a as i64 % 2 == 0 { n } else { r };
    let c = i * (o * n + (a - o) * r) + (e - a * i) * s;
    c.sin()
}

// amplitude at time e (seconds), dir down=ISP->you / up=you->ISP
fn amp(e: f64, down: bool) -> f64 {
    if e < 0.0 {
        return 0.0;
    }
    match phase_at(e) {
        0 => {
            if down {
                if e < 1.4 { ((e * 35.0).sin() + (e * 44.0).sin()) * 0.38 } else { 0.0 }
            } else if e < 1.4 || (e * 2.4).floor() as i64 % 2 != 0 {
                0.0
            } else {
                ((e * 33.0).sin() + (e * 52.0).sin()) * 0.42
            }
        }
        1 => if down { (e * 26.0).sin() * 0.7 } else { 0.0 },
        2 => if down { burst(e, 7.0, 44.0, 60.0) * 0.85 } else { burst(e, 7.0, 20.0, 30.0) * 0.8 },
        3 => {
            if down {
                (e * 17.0).sin() * 0.35 + (e * 26.0).sin() * 0.3 + (e * 37.0).sin() * 0.25
            } else {
                (e * 15.0).sin() * 0.35 + (e * 23.0).sin() * 0.3 + (e * 41.0).sin() * 0.25
            }
        }
        4 => if down { u(e) * 0.95 } else { u(e + 7.3) * 0.9 },
        _ => if down { u(e) * 0.3 } else { u(e + 7.3) * 0.28 },
    }
}

pub struct Dialup;

impl Sim for Dialup {
    fn duration(&self) -> u64 {
        DURATION
    }

    fn frame(&self, t: u64) -> Frame {
        let now = (t % DURATION) as f64 / 1000.0;
        let now = now.min(TOTAL_S);
        let phase = phase_at(now);

        // scrolling waveform window [now-2.2s, now], 160 samples each direction
        let mut down = Vec::new();
        let mut up = Vec::new();
        for i in 0..=160 {
            let f = i as f64 / 160.0;
            let te = now - WINDOW_S + f * WINDOW_S;
            let x = 10.0 + f * 80.0;
            // original: down y = 54-16-a*11 in a 128-tall viewBox
            down.push((x, (54.0 - 16.0 - amp(te, true) * 11.0) / 128.0 * 100.0));
            up.push((x, (70.0 - amp(te, false) * 11.0) / 128.0 * 100.0));
        }

        // phase strip: 6 labels, current bold; playhead marker
        let mut texts = vec![];
        for (i, (label, _)) in PHASES.iter().enumerate() {
            texts.push(TextSpec {
                text: label.to_string(),
                x: 10.0 + (i as f64 + 0.5) * 80.0 / 6.0,
                y: 84.0,
                dim: i != phase,
                left: false,
            });
        }
        let frac = now / TOTAL_S;
        let playhead_x = 10.0 + frac * 80.0;

        Frame {
            header: Some("INTERACTIVE · DIAL-UP HANDSHAKE".into()),
            nodes: vec![
                NodeSpec { label: "your modem".into(), x: 10.0, y: 45.0, status: None, lifeline: false, icon: None },
                NodeSpec { label: "ISP modem".into(), x: 90.0, y: 45.0, status: None, lifeline: false, icon: None },
            ],
            packets: vec![],
            trails: vec![
                TrailSpec { x1: 10.0, y1: 32.0, x2: 90.0, y2: 32.0, arrow_end: false },
                TrailSpec { x1: playhead_x, y1: 78.0, x2: playhead_x, y2: 90.0, arrow_end: false },
            ],
            texts,
            polylines: vec![
                PolylineSpec { points: down, dim: false },
                PolylineSpec { points: up, dim: true },
            ],
            paths: vec![],
            badge: None,
            note: PHASES[phase].1.into(),
        }
    }
}
