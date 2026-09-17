//! Faithful port of fazamhd.com's ModemSim: byte 'A' (0x41, bits 01000001) sent
//! as FSK tones — high-frequency "mark" = 1, low-frequency "space" = 0 — the wave
//! drawn progressively, bits revealed at the receiver as they arrive.

use crate::frame::*;

const BITS: [u8; 8] = [0, 1, 0, 0, 0, 0, 0, 1]; // 'A' = 0x41
const BIT_MS: u64 = 650;
const L: u64 = BITS.len() as u64 * BIT_MS; // 5200
pub const DURATION: u64 = L + 1100;

// wave geometry (original: svg 720x120, x 24..696, baseline 60, amplitude 42,
// mark=5 cycles/bit, space=2)
fn wave_points() -> Vec<(f64, f64, f64)> {
    let mut pts = Vec::new();
    let mut gx = 24.0;
    while gx <= 696.0 {
        let t = (gx - 24.0) / (696.0 - 24.0) * L as f64;
        let bit = ((t / BIT_MS as f64) as usize).min(BITS.len() - 1);
        let cycles = if BITS[bit] == 1 { 5.0 } else { 2.0 };
        let phase = (t - bit as f64 * BIT_MS as f64) / BIT_MS as f64;
        let y = 60.0 - (2.0 * std::f64::consts::PI * cycles * phase).sin() * 42.0;
        pts.push((gx / 720.0 * 100.0, y / 120.0 * 100.0, t));
        gx += 3.0;
    }
    pts
}

pub struct Modem;

impl Sim for Modem {
    fn duration(&self) -> u64 {
        DURATION
    }

    fn frame(&self, t: u64) -> Frame {
        let tt = t % DURATION;
        let tc = tt.min(L);
        let sending = tt < L;
        let cur_bit = ((tc / BIT_MS) as usize).min(BITS.len() - 1);

        // wave split at playhead: drawn part bold, future part faint
        let pts = wave_points();
        let drawn: Vec<(f64, f64)> = pts.iter().filter(|p| p.2 <= tc as f64).map(|p| (p.0, p.1)).collect();
        let future: Vec<(f64, f64)> = pts.iter().filter(|p| p.2 >= tc as f64).map(|p| (p.0, p.1)).collect();

        // playhead
        let px = 24.0 + tc as f64 / L as f64 * (696.0 - 24.0);
        let playhead = TrailSpec { x1: px / 720.0 * 100.0, y1: 8.0, x2: px / 720.0 * 100.0, y2: 95.0, arrow_end: false };

        let mut texts = vec![
            TextSpec { text: "sending 'A' as bits 01000001".into(), x: 50.0, y: 6.0, dim: true, left: false },
            TextSpec { text: "received".into(), x: 6.0, y: 88.0, dim: true, left: false },
        ];
        // sent bits row (current bold)
        for (i, b) in BITS.iter().enumerate() {
            let x = (i as f64 + 0.5) * 100.0 / 8.0;
            texts.push(TextSpec {
                text: b.to_string(),
                x,
                y: 12.0,
                dim: !(sending && i == cur_bit),
                left: false,
            });
        }
        // received bits row (revealed as each bit's duration elapses)
        for (i, b) in BITS.iter().enumerate() {
            let revealed = tc >= (i as u64 + 1) * BIT_MS;
            texts.push(TextSpec {
                text: if revealed { b.to_string() } else { "?".into() },
                x: (i as f64 + 0.5) * 100.0 / 8.0,
                y: 95.0,
                dim: !revealed,
                left: false,
            });
        }

        Frame {
            header: Some("INTERACTIVE · MODEM ·· MODULATION / DEMODULATION, ONE BYTE".into()),
            nodes: vec![],
            packets: vec![],
            trails: vec![playhead],
            texts,
            polylines: vec![
                PolylineSpec { points: drawn, dim: false },
                PolylineSpec { points: future, dim: true },
            ],
            paths: vec![],
            badge: None,
            note: if !sending {
                "All 8 bits have arrived. The receiving modem read the tones back as 01000001, byte 0x41, the ASCII character 'A'.".into()
            } else if BITS[cur_bit] == 1 {
                "Sending bit 1, the modem plays the higher \"mark\" tone for this bit's whole duration.".into()
            } else {
                "Sending bit 0, the modem plays the lower \"space\" tone for this bit's whole duration.".into()
            },
        }
    }
}
