//! Faithful port of fazamhd.com's BandwidthLatencySim: one byte of pulses
//! crossing a link, slowed down. Bandwidth sets how fast bits leave the sender,
//! distance sets how long the signal travels. Defaults fixed at 1 Mbps × 6,000 km
//! (original has selectors + drag-to-seek; we auto-play the loop).

use crate::frame::*;

const SVG_W: f64 = 760.0;
const SVG_H: f64 = 190.0;
const S_X: f64 = 70.0; // sender x
const WIRE_Y: f64 = 55.0;
const BITS: [u8; 8] = [1, 0, 1, 1, 0, 0, 1, 0];
const HOLD_MS: u64 = 2600; // seek span mapped to wire (original h)
pub const DURATION: u64 = 3400; // loop (original te)

// fixed config: 1 Mbps, 6,000 km transatlantic
const SIM_TX_MS: f64 = 24.0; // R
const SIM_PROP_MS: f64 = 45.0; // L
const BIT_MS: f64 = SIM_TX_MS / 8.0; // B
const V_PX: f64 = 100.0 + SIM_PROP_MS * 8.0; // wire length px (460)
const IE: f64 = SIM_TX_MS * 2.5;
const W_SPAN: f64 = V_PX + IE;

// smoothed bit value at wave-time e (original ne)
fn ne(e: f64) -> f64 {
    let t = BIT_MS;
    let n = BITS.len() as f64;
    let total = t * n;
    let i = t * 0.16;
    let a = i / 2.0;
    if e < -a || e > total + a {
        return 0.0;
    }
    let o = (e / t).round();
    let s = e - o * t;
    if o >= 0.0 && o <= n && s.abs() < a {
        let prev = if o >= 1.0 && (o as usize) <= BITS.len() { BITS[(o - 1.0) as usize] } else { 0 } as f64;
        let cur = if o >= 0.0 && (o as usize) < BITS.len() { BITS[o as usize] } else { 0 } as f64;
        let r = (s + a) / i;
        let c = 3.0 * r * r - 2.0 * r * r * r;
        return prev + (cur - prev) * c;
    }
    let c = (e / t).floor();
    if c >= 0.0 && (c as usize) < BITS.len() {
        BITS[c as usize] as f64
    } else {
        0.0
    }
}

pub struct Bandwidth;

impl Sim for Bandwidth {
    fn duration(&self) -> u64 {
        DURATION
    }

    fn frame(&self, t: u64) -> Frame {
        let tt = (t % DURATION) as f64;
        let u_px = S_X + V_PX;
        let g = S_X + (tt.min(HOLD_MS as f64) / HOLD_MS as f64) * W_SPAN;
        let h_px = V_PX / SIM_PROP_MS;
        let k = if g <= u_px { (g - S_X) / h_px } else { SIM_PROP_MS + (g - u_px) / 2.5 };

        let transmitting = k > 0.0 && k < SIM_TX_MS;
        let arriving = k > SIM_PROP_MS && k < SIM_PROP_MS + SIM_TX_MS;
        let done = k >= SIM_PROP_MS + SIM_TX_MS;

        // waveform: 200 samples along the wire, each delayed by its propagation
        let mut pts = Vec::new();
        for i in 0..=200 {
            let n = i as f64 / 200.0;
            let x = S_X + n * V_PX;
            let y = WIRE_Y - ne(k - n * SIM_PROP_MS) * 20.0;
            pts.push((x / SVG_W * 100.0, y / SVG_H * 100.0));
        }

        let note = if transmitting && !arriving {
            "Transmitter is keying the bit sequence onto the link."
        } else if arriving && transmitting {
            "Signal is arriving at receiver while sender is still transmitting."
        } else if arriving {
            "Transmission finished at sender; last bits are arriving at receiver."
        } else if done {
            "Message fully received and decoded."
        } else {
            "Idle."
        };
        // P = 30 ms prop (slowed), F = 8 ms tx (slowed): neither dominates at 1 Mbps × 6000 km
        let note2 = "Neither can be ignored, sending duration and travel time take comparable lengths.";

        Frame {
            header: Some("INTERACTIVE · BANDWIDTH AND LATENCY IN SIGNAL PROPAGATION".into()),
            nodes: vec![
                NodeSpec { label: "Tx".into(), x: S_X / SVG_W * 100.0, y: WIRE_Y / SVG_H * 100.0, status: Some("(sender)".into()), lifeline: false, icon: None },
                NodeSpec { label: "Rx".into(), x: u_px / SVG_W * 100.0, y: WIRE_Y / SVG_H * 100.0, status: Some("(receiver)".into()), lifeline: false, icon: None },
            ],
            packets: vec![],
            trails: vec![TrailSpec { x1: S_X / SVG_W * 100.0, y1: WIRE_Y / SVG_H * 100.0, x2: u_px / SVG_W * 100.0, y2: WIRE_Y / SVG_H * 100.0, arrow_end: false }],
            texts: vec![
                TextSpec { text: "1 Mbps · early broadband   ×   6,000 km · transatlantic cable".into(), x: 50.0, y: 6.0, dim: true, left: false },
                TextSpec { text: "bits: 10110010".into(), x: 50.0, y: 14.0, dim: true, left: false },
                TextSpec { text: format!("propagation {} ms (slowed) · transmission {} ms (slowed)", SIM_PROP_MS as u64, SIM_TX_MS as u64), x: 50.0, y: 85.0, dim: true, left: false },
                TextSpec { text: note2.into(), x: 50.0, y: 92.0, dim: true, left: false },
            ],
            polylines: vec![PolylineSpec { points: pts, dim: false }],
            paths: vec![],
            badge: None,
            note: note.into(),
        }
    }
}
