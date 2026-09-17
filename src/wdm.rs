//! Faithful port of fazamhd.com's WdmSim: three lasers key independent bit
//! streams onto different wavelengths; inside the fiber the waves superpose into
//! one combined waveform; a splitter filters each wavelength back out intact.
//! Waveform math ported verbatim (envelope + sine carriers). Stream count fixed
//! at 3 (original is interactive 1/2/3).

use crate::frame::*;

const S_X: f64 = 95.0; // laser x
const C_X: f64 = 250.0; // combine
const L_X: f64 = 510.0; // split
const U_X: f64 = 665.0; // receiver
const H: f64 = 0.16; // px per ms
const G: f64 = 260.0; // bit duration ms
const WAVE_MS: f64 = (U_X - S_X) / H; // 3562.5 wavefront transit
const V_END: f64 = 4.0 * G; // 5 bits - 1
const Y_END: f64 = V_END + G + WAVE_MS;
pub const DURATION: u64 = (Y_END + 900.0) as u64;

const LANE_Y: [f64; 3] = [50.0, 105.0, 160.0]; // laser/receiver lanes (svg y)
const FIBER_Y: f64 = 105.0;
const LAMBDA: [f64; 3] = [13.0, 9.0, 6.0]; // carrier wavelengths (svg px)
const AMP: f64 = 3.2;

// original bit patterns per stream
const BITS: [[u8; 5]; 3] = [[1, 0, 1, 1, 0], [0, 1, 1, 0, 1], [1, 1, 0, 1, 0]];

// pulse envelope for stream at position pos, timeline L (original E)
fn envelope(stream: usize, pos: f64, l: f64) -> f64 {
    let i = l - (pos - S_X) / H;
    let a = (i / G).floor();
    if i < 0.0 || a < 0.0 || a as usize >= 5 || BITS[stream][a as usize] != 1 {
        return 0.0;
    }
    let o = i / G - a;
    AMP * (1.0_f64).min(o.min(1.0 - o) / 0.15)
}

// sampled waveform of the given streams along [p0,p1] at base y (original D)
fn wave(streams: &[usize], p0: f64, p1: f64, l: f64, ybase: Box<dyn Fn(f64) -> f64>) -> Vec<(f64, f64)> {
    let mut pts = Vec::new();
    let mut p = p0;
    while p <= p1 {
        let mut sum = 0.0;
        for &i in streams {
            let e = envelope(i, p, l);
            if e > 0.0 {
                sum += e * (2.0 * std::f64::consts::PI * (p - l * H) / LAMBDA[i]).sin();
            }
        }
        pts.push((p / 760.0 * 100.0, (ybase(p) + sum) / 230.0 * 100.0));
        p += 0.7;
    }
    pts
}

pub struct Wdm;

impl Sim for Wdm {
    fn duration(&self) -> u64 {
        DURATION
    }

    fn frame(&self, t: u64) -> Frame {
        let l = (t % DURATION) as f64;
        let l = l.min(Y_END);
        let done = l >= Y_END;
        let v_front = S_X + l * H; // input wavefront
        let h_front = S_X + (l - V_END - G) * H; // output wavefront
        let in_fiber = v_front > C_X && h_front < L_X && !done;
        let out_fiber = v_front >= L_X && h_front < U_X && !done;

        let map_in = |i: usize| Box::new(move |p: f64| LANE_Y[i] + (FIBER_Y - LANE_Y[i]) * (p - S_X) / (C_X - S_X)) as Box<dyn Fn(f64) -> f64>;
        let map_out = |i: usize| Box::new(move |p: f64| FIBER_Y + (LANE_Y[i] - FIBER_Y) * (p - L_X) / (U_X - L_X)) as Box<dyn Fn(f64) -> f64>;
        let flat = || Box::new(|_p: f64| FIBER_Y) as Box<dyn Fn(f64) -> f64>;

        let mut polylines = Vec::new();
        // per-stream input carriers (laser -> combiner)
        for i in 0..3 {
            polylines.push(PolylineSpec { points: wave(&[i], S_X, C_X, l, map_in(i)), dim: true });
        }
        // combined waveform in the fiber (bold)
        if in_fiber || out_fiber {
            polylines.push(PolylineSpec { points: wave(&[0, 1, 2], C_X, L_X, l, flat()), dim: false });
        }
        // separated outputs after the splitter
        if out_fiber || done {
            for i in 0..3 {
                polylines.push(PolylineSpec { points: wave(&[i], L_X, U_X, l, map_out(i)), dim: false });
            }
        }

        let note = if done {
            "All 3 streams delivered intact over the one strand."
        } else if out_fiber && in_fiber {
            "The splitter is a wavelength filter: out of the combined light, each output port passes only its own carrier, and each comes out exactly as its laser keyed it."
        } else if in_fiber {
            "All 3 carriers overlap in the same core, the glass carries their sum (bold line). The waves add without mixing, each passes through unchanged, so every carrier is still in there."
        } else if out_fiber {
            "The last of the combined light has been filtered apart by wavelength and is arriving at the receivers."
        } else {
            "Each laser keys its own bit stream onto its own wavelength of light."
        };

        let sx = |x: f64| x / 760.0 * 100.0;
        let sy = |y: f64| y / 230.0 * 100.0;
        let mut texts = vec![
            TextSpec { text: "one fiber strand, the waves add".into(), x: sx(380.0), y: sy(82.0), dim: true, left: false },
            TextSpec { text: "combine".into(), x: sx(256.0), y: sy(140.0), dim: true, left: false },
            TextSpec { text: "split".into(), x: sx(L_X - 6.0), y: sy(140.0), dim: true, left: false },
            TextSpec { text: "3 wavelengths, the same strand now carries 3× the bits per second.".into(), x: sx(380.0), y: sy(200.0), dim: true, left: false },
        ];
        for (i, name) in ["λ1", "λ2", "λ3"].iter().enumerate() {
            texts.push(TextSpec { text: name.to_string(), x: sx(S_X), y: sy(LANE_Y[i] - 22.0), dim: false, left: false });
        }

        Frame {
            header: Some("INTERACTIVE · WAVELENGTH-DIVISION MULTIPLEXING".into()),
            nodes: (0..3)
                .map(|i| NodeSpec { label: String::new(), x: sx(S_X), y: sy(LANE_Y[i]), status: None, lifeline: false, icon: None })
                .chain((0..3).map(|i| NodeSpec { label: String::new(), x: sx(U_X), y: sy(LANE_Y[i]), status: None, lifeline: false, icon: None }))
                .collect(),
            packets: vec![],
            trails: (0..3)
                .map(|i| TrailSpec { x1: sx(S_X), y1: sy(LANE_Y[i]), x2: sx(C_X), y2: sy(FIBER_Y), arrow_end: false })
                .chain((0..3).map(|i| TrailSpec { x1: sx(L_X), y1: sy(FIBER_Y), x2: sx(U_X), y2: sy(LANE_Y[i]), arrow_end: false }))
                .chain([TrailSpec { x1: sx(C_X + 12.0), y1: sy(FIBER_Y - 11.0), x2: sx(L_X - 12.0), y2: sy(FIBER_Y - 11.0), arrow_end: false }, TrailSpec { x1: sx(C_X + 12.0), y1: sy(FIBER_Y + 11.0), x2: sx(L_X - 12.0), y2: sy(FIBER_Y + 11.0), arrow_end: false }])
                .collect(),
            texts,
            polylines,
            paths: vec![],
            badge: None,
            note: note.into(),
        }
    }
}
