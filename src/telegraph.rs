//! Faithful port of fazamhd.com's TelegraphSim: Morse "WHAT HATH GOD WROUGHT"
//! travels Washington → two relay stations → Baltimore. Regen mode: relays
//! recreate clean pulses. Amp mode: relays boost noise too, Baltimore misreads.
//! Mode alternates each scroll cycle (original is a toggle).

use crate::frame::*;

const W: f64 = 800.0; // svg width
const H: f64 = 270.0; // svg height
const BASE: f64 = 104.0; // wire y
const LEFT: f64 = 80.0;
const RIGHT: f64 = 720.0;
const STATIONS: [f64; 2] = [293.33333333333337, 506.6666666666667];
const PX_PER_UNIT: f64 = 55.0; // p
const PX_PER_MS: f64 = 0.18; // m — wave scroll speed
const PULSE_H: f64 = 30.0; // f
const STEP: f64 = 2.5; // sampling step px

const MESSAGE: &[u8] = b"WHAT HATH GOD WROUGHT";
const MORSE: &[(u8, &str)] = &[
    (b'W', ".--"), (b'H', "...."), (b'A', ".-"), (b'T', "-"), (b'G', "--."),
    (b'O', "---"), (b'D', "-.."), (b'R', ".-."), (b'U', "..-"),
];
// amp-mode misreads: char index -> wrong letter (original v)
const MISREADS: [(usize, char); 3] = [(5, 'S'), (10, 'M'), (17, 'I')];

fn morse(c: u8) -> &'static str {
    MORSE.iter().find(|(k, _)| *k == c).unwrap().1
}

// (start, end) unit-time spans of every pulse, and (char, idx, start, end) letters
fn timeline() -> (Vec<(f64, f64)>, Vec<(usize, f64, f64)>) {
    let mut pulses = Vec::new();
    let mut letters = Vec::new();
    let mut e = 0.0f64;
    for (idx, &c) in MESSAGE.iter().enumerate() {
        if c == b' ' {
            e += 4.0;
            continue;
        }
        let code = morse(c);
        let start = e;
        for (i, sym) in code.chars().enumerate() {
            let len = if sym == '.' { 1.0 } else { 3.0 };
            pulses.push((e, e + len));
            e += len;
            if i < code.len() - 1 {
                e += 1.0;
            }
        }
        letters.push((idx, start, e));
        e += 3.0;
    }
    (pulses, letters)
}

fn wave_len() -> f64 {
    let (pulses, letters) = timeline();
    // total units: recompute exactly like the original closure
    let mut e = 0.0;
    for &c in MESSAGE.iter() {
        if c == b' ' {
            e += 4.0;
            continue;
        }
        let code = morse(c);
        for (i, sym) in code.chars().enumerate() {
            e += if sym == '.' { 1.0 } else { 3.0 };
            if i < code.len() - 1 {
                e += 1.0;
            }
        }
        e += 3.0;
    }
    let _ = (pulses, letters);
    (e + 16.0) * PX_PER_UNIT
}

// pulse value at unit-time t (original w)
fn pulse_at(pulses: &[(f64, f64)], units: f64) -> f64 {
    for (s, e) in pulses {
        if units < *s {
            return 0.0;
        }
        if units < *e {
            return 1.0;
        }
    }
    0.0
}

// noise (original T)
fn noise(px: f64, g: f64) -> f64 {
    0.6 * (px * 0.31 + g * 0.011).sin() + 0.4 * (px * 0.83 - g * 0.017 + (px * 0.05).sin()).sin()
}

const CAP_REGEN: &str = "An electromechanical relay along the line didn't need to pass the wave itself; it only needed to detect whether a pulse was present, and then recreate a brand new, clean copy of that pulse to send down the next segment of wire.";
const CAP_AMP: &str = "Now the relays merely boost what arrives (like an analog amplifier). It cannot distinguish a pulse from noise, so it boosts the noise along with the signal, until Baltimore misreads the message.";

pub struct Telegraph;

impl Sim for Telegraph {
    fn duration(&self) -> u64 {
        // two full cycles: regen then amp
        ((wave_len() + (RIGHT - LEFT) / PX_PER_MS) * 2.0) as u64
    }

    fn frame(&self, t: u64) -> Frame {
        let (pulses, letters) = timeline();
        let x_len = wave_len();
        let s_win = (RIGHT - LEFT) / PX_PER_MS; // scroll window px
        let cycle = x_len + s_win;
        let total = cycle * 2.0;
        let tt = (t as f64) % total;
        let (cycle_pos, regen) = if tt < cycle { (tt, true) } else { (tt - cycle, false) };
        let i_pos = cycle_pos % cycle;

        // wave samples
        let seg = (RIGHT - LEFT) / 3.0;
        let mut pts = Vec::new();
        let mut px = LEFT;
        while px <= RIGHT {
            let units = (i_pos - (px - LEFT) / PX_PER_MS) / PX_PER_UNIT;
            let n = pulse_at(&pulses, units);
            let r = if px < STATIONS[0] { LEFT } else if px < STATIONS[1] { STATIONS[0] } else { STATIONS[1] };
            let dist = px - r;
            let atten = 1.0 - dist / seg * 0.55;
            let o = if regen { 0.5 + dist / seg * 5.5 } else { 0.5 + 6.0 * (px - LEFT) / seg };
            let y = BASE - n * atten * PULSE_H + o * noise(px, i_pos);
            pts.push((px / W * 100.0, y / H * 100.0));
            px += STEP;
        }

        // keying char / received chars
        let mut keying = None;
        for (idx, s, e) in &letters {
            if s * PX_PER_UNIT <= i_pos && i_pos < e * PX_PER_UNIT {
                keying = Some(*idx);
            }
        }
        let received: Vec<usize> = letters
            .iter()
            .filter(|(_, _, e)| e * PX_PER_UNIT + s_win <= i_pos)
            .map(|(idx, _, _)| *idx)
            .collect();

        let sent: String = MESSAGE
            .iter()
            .enumerate()
            .map(|(_i, &c)| c as char)
            .collect();
        let recv: String = MESSAGE
            .iter()
            .enumerate()
            .map(|(i, &c)| {
                if !received.contains(&i) {
                    return ' ';
                }
                if !regen {
                    if let Some((_, wrong)) = MISREADS.iter().find(|(idx, _)| *idx == i) {
                        return *wrong;
                    }
                }
                c as char
            })
            .collect();

        Frame {
            header: Some(format!(
                "INTERACTIVE · TELEGRAPH RELAY ·· {}, REGENERATED AT EACH RELAY",
                if regen { "DISCRETE SYMBOLS" } else { "ANALOG AMPLIFIED" }
            )),
            nodes: vec![
                NodeSpec {
                    label: "Washington".into(),
                    x: LEFT / W * 100.0,
                    y: BASE / H * 100.0,
                    status: keying.map(|i| format!("keying {}", MESSAGE[i] as char)),
                    lifeline: false,
                    icon: None,
                },
                NodeSpec { label: "relay station".into(), x: STATIONS[0] / W * 100.0, y: BASE / H * 100.0, status: Some(if regen { "regenerates" } else { "amplifies" }.into()), lifeline: false, icon: None },
                NodeSpec { label: "relay station".into(), x: STATIONS[1] / W * 100.0, y: BASE / H * 100.0, status: Some(if regen { "regenerates" } else { "amplifies" }.into()), lifeline: false, icon: None },
                NodeSpec { label: "Baltimore".into(), x: RIGHT / W * 100.0, y: BASE / H * 100.0, status: None, lifeline: false, icon: None },
            ],
            packets: vec![],
            trails: vec![TrailSpec { x1: LEFT / W * 100.0, y1: BASE / H * 100.0, x2: RIGHT / W * 100.0, y2: BASE / H * 100.0, arrow_end: false }],
            texts: vec![
                TextSpec { text: "pulses fade, noise creeps in →".into(), x: 186.66666666666669 / W * 100.0, y: (BASE - PULSE_H - 22.0) / H * 100.0, dim: true, left: false },
                TextSpec { text: format!("SENT FROM WASHINGTON: {sent}"), x: (LEFT - 7.0) / W * 100.0, y: 192.0 / H * 100.0, dim: false, left: true },
                TextSpec { text: format!("RECEIVED AT BALTIMORE: {recv}"), x: (LEFT - 7.0) / W * 100.0, y: 258.0 / H * 100.0, dim: false, left: true },
            ],
            polylines: vec![PolylineSpec { points: pts, dim: false }],
            paths: vec![],
            badge: None,
            note: if regen { CAP_REGEN } else { CAP_AMP }.into(),
        }
    }
}
