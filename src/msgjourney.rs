//! Faithful port of fazamhd.com's MessageJourneySim (the article hero): one
//! message's trip in 7 chapters — across the room (radio), under the street
//! (copper), into the data center, across the ocean (map + undersea fibre), the
//! far side (tower + radio), and a summary. Medium-specific waveforms, km/ms
//! counters, and the world map for the ocean leg.

use crate::frame::*;

const SVG_W: f64 = 960.0;
const SVG_H: f64 = 450.0;
const WAVE_X: f64 = 40.0;
const WAVE_Y: f64 = 38.0;
const WAVE_W: f64 = 880.0;
const WAVE_H: f64 = 140.0;
const WAVE_BASE: f64 = 106.0; // waveform baseline (svg y, inside box)
const WIRE_LEFT: f64 = 76.0;
const WIRE_RIGHT: f64 = 808.0;
const NODE_Y: f64 = 328.0;
const BITS: [u8; 11] = [1, 0, 1, 1, 0, 0, 1, 0, 1, 1, 0];

#[allow(dead_code)]
struct Leg {
    km: f64,
    ms: f64,
    medium: &'static str,
    owner: &'static str,
}
const LEGS: [Leg; 9] = [
    Leg { km: 0.008, ms: 1.0, medium: "radio", owner: "yours" },
    Leg { km: 0.5, ms: 0.4, medium: "copper", owner: "your internet provider" },
    Leg { km: 15.0, ms: 1.5, medium: "fiber", owner: "your internet provider" },
    Leg { km: 60.0, ms: 1.0, medium: "fiber", owner: "your internet provider" },
    Leg { km: 330.0, ms: 2.5, medium: "fiber", owner: "national backbone" },
    Leg { km: 6600.0, ms: 34.0, medium: "fiber", owner: "cable consortium" },
    Leg { km: 500.0, ms: 3.5, medium: "fiber", owner: "their internet provider" },
    Leg { km: 30.0, ms: 2.0, medium: "fiber", owner: "their internet provider" },
    Leg { km: 1.2, ms: 8.0, medium: "radio", owner: "their mobile carrier" },
];

struct Chapter {
    label: &'static str,
    dur: u64,
    segs: &'static [usize],
    xs: &'static [f64],
    note: &'static str,
}
const CHAPTERS: [Chapter; 7] = [
    Chapter { label: "you send a message", dur: 4200, segs: &[], xs: &[], note: "It goes to another continent, and the reply lands before you have put the phone down." },
    Chapter { label: "across the room", dur: 3400, segs: &[0], xs: &[190.0, 770.0], note: "The bytes become a radio wave, and cross the room to your router." },
    Chapter { label: "under your street", dur: 3800, segs: &[1, 2], xs: &[130.0, 480.0, 830.0], note: "Voltage steps on copper, until the cabinet turns them into light." },
    Chapter { label: "into the app's data center", dur: 4400, segs: &[3, 4], xs: &[130.0, 480.0, 830.0], note: "The message stops at the app's data center, a few hundred kilometres from your phone." },
    Chapter { label: "across the ocean", dur: 5200, segs: &[5], xs: &[90.0, 870.0], note: "6,600 km of glass on the sea floor, laid by a consortium. The app's company only leases fibre pairs inside it." },
    Chapter { label: "the far side", dur: 4600, segs: &[6, 7, 8], xs: &[110.0, 390.0, 660.0, 890.0], note: "Inland, out to a tower, and back into radio for the last kilometre." },
    Chapter { label: "the route it took", dur: 5600, segs: &[], xs: &[], note: "None of the companies along the way coordinated for this message, and none of them knows the whole path it took." },
];

const NODE_LABELS: [&str; 10] = [
    "your phone", "your router", "street cabinet", "your provider", "city exchange",
    "the app's dc", "the app's dc", "their provider", "cell tower", "their phone",
];

const WORLD: &str = include_str!("assets/world.path");

// map viewport (original _): lon -100..30, lat 54..34.14
fn map_xy(lon: f64, lat: f64) -> (f64, f64) {
    let g = (60.0, 44.0, 840.0, 275.0);
    let bx = (-100.0 + 180.0) / 360.0 * 100.0;
    let by = (84.0 - 54.0) / 140.0 * 100.0;
    let sx = g.2 / (((30.0 + 180.0) / 360.0 * 100.0) - bx);
    let sy = g.3 / (((84.0 - 34.14) / 140.0 * 100.0) - by);
    let vx = (lon + 180.0) / 360.0 * 100.0;
    let vy = (84.0 - lat) / 140.0 * 100.0;
    (g.0 + (vx - bx) * sx, g.1 + (vy - by) * sy)
}

fn fmt_km(km: f64) -> String {
    if km < 1.0 {
        format!("{} m", (km * 1000.0).round())
    } else if km < 10.0 {
        format!("{km:.1} km")
    } else {
        format!("{} km", km.round())
    }
}
fn fmt_ms(ms: f64) -> String {
    if ms < 10.0 {
        format!("{ms:.1} ms")
    } else {
        format!("{} ms", ms.round())
    }
}

// waveform points for a medium inside the wave box (svg px, unclipped)
fn wave_points(medium: &str, off: f64) -> Vec<(f64, f64)> {
    let k = (WIRE_RIGHT - 0.0) / BITS.len() as f64;
    let bit_at = |px: f64| -> u8 {
        let i = ((px / k).floor() as i64).rem_euclid(BITS.len() as i64) as usize;
        BITS[i]
    };
    let mut pts = Vec::new();
    match medium {
        "copper" => {
            let lo = WAVE_BASE - 30.0;
            let mut y = if bit_at(-off) == 1 { lo } else { 134.0 };
            pts.push((0.0, y));
            let mut px = 0.0;
            while px <= WIRE_RIGHT {
                let bit = bit_at(px - off);
                let ny = if bit == 1 { lo } else { 134.0 };
                pts.push((px, y));
                pts.push((px, ny));
                y = ny;
                px += 1.0;
            }
        }
        "radio" => {
            let mut px = 0.0;
            while px <= WIRE_RIGHT {
                let t = px - off;
                let a = if bit_at(t) == 1 { 34.0 } else { 7.0 };
                pts.push((px, WAVE_BASE + a * (t * 4.0 * std::f64::consts::PI / k).sin()));
                px += 1.5;
            }
        }
        // fiber: level line; pulses drawn as separate short segments below
        _ => pts.push((0.0, WAVE_BASE)),
    }
    pts
}

pub struct MessageJourney;

impl Sim for MessageJourney {
    fn duration(&self) -> u64 {
        CHAPTERS.iter().map(|c| c.dur).sum()
    }

    fn frame(&self, t: u64) -> Frame {
        let total: u64 = CHAPTERS.iter().map(|c| c.dur).sum();
        let tt = t % total;
        // find chapter + local progress
        let mut acc = 0;
        let mut ci = 0;
        let mut p = 0.0;
        for (i, c) in CHAPTERS.iter().enumerate() {
            if tt < acc + c.dur {
                ci = i;
                p = (tt - acc) as f64 / c.dur as f64;
                break;
            }
            acc += c.dur;
        }
        let ch = &CHAPTERS[ci];

        // travel fraction eased (original V smoothing at edges: travel:[.06,.9] etc.)
        let (t0, t1) = if ch.segs.is_empty() { (0.0, 0.0) } else { (0.06, 0.9) };
        let travel = if ch.segs.is_empty() { 0.0 } else { (t0 + (t1 - t0) * p).clamp(0.0, 1.0) };

        // current leg + fraction for counters + medium
        let (leg, u, medium) = if ch.segs.is_empty() {
            (0usize, 0.0f64, "")
        } else {
            // position along xs
            let x0 = ch.xs[0];
            let x1 = *ch.xs.last().unwrap();
            let x = x0 + travel * (x1 - x0);
            let mut seg_idx = 0;
            let mut uu = 0.0;
            for (k, seg) in ch.segs.iter().enumerate() {
                let a = ch.xs[k];
                let b = ch.xs[k + 1];
                if x <= b || k == ch.segs.len() - 1 {
                    seg_idx = *seg;
                    uu = ((x - a) / (b - a)).clamp(0.0, 1.0);
                    break;
                }
            }
            (seg_idx, uu, LEGS[seg_idx].medium)
        };
        // cumulative km/ms
        let (mut km, mut ms) = (0.0, 0.0);
        if !ch.segs.is_empty() {
            for (i, l) in LEGS.iter().enumerate() {
                if i < leg {
                    km += l.km;
                    ms += l.ms;
                }
            }
            km += LEGS[leg].km * u;
            ms += LEGS[leg].ms * u;
        }

        let mut polylines = Vec::new();
        let mut texts = vec![
            TextSpec { text: ch.label.into(), x: 50.0, y: 44.0 / SVG_H * 100.0, dim: true, left: false },
        ];
        let mut packets = vec![];
        let mut paths = vec![];

        if !ch.segs.is_empty() {
            // waveform (clipped conceptually to the box; we just draw in box-local coords)
            let off = -travel * 600.0;
            let local = wave_points(medium, off);
            let pts: Vec<(f64, f64)> = local
                .iter()
                .map(|(x, y)| ((WAVE_X + 4.0 + x / (WIRE_RIGHT - WIRE_LEFT) * (WAVE_W - 8.0)) / SVG_W * 100.0, (WAVE_Y + 4.0 + (y / 180.0) * (WAVE_H - 8.0)) / SVG_H * 100.0))
                .collect();
            if medium != "fiber" {
                polylines.push(PolylineSpec { points: pts, dim: false });
            } else {
                // fiber: pulses as small blocks along the box center
                let k = (WAVE_W - 8.0) / BITS.len() as f64;
                let mut fpts = Vec::new();
                for i in 0..BITS.len() {
                    if BITS[i] == 1 {
                        let cx = WAVE_X + 4.0 + (i as f64 + 0.5) * k;
                        fpts.push((cx / SVG_W * 100.0, (WAVE_Y + WAVE_H / 2.0) / SVG_H * 100.0));
                    }
                }
                polylines.push(PolylineSpec { points: fpts, dim: false });
            }
            texts.push(TextSpec {
                text: match medium {
                    "radio" => "radio waves",
                    "copper" => "voltage on copper",
                    _ => "light in glass",
                }
                .into(),
                x: (WAVE_X + 22.0) / SVG_W * 100.0,
                y: (WAVE_Y + 28.0) / SVG_H * 100.0,
                dim: false,
                left: true,
            });
            texts.push(TextSpec {
                text: format!("{}, {}", fmt_km(km), fmt_ms(ms)),
                x: (WAVE_X + WAVE_W - 22.0) / SVG_W * 100.0,
                y: (WAVE_Y + 28.0) / SVG_H * 100.0,
                dim: true,
                left: false,
            });
            // packet dot along the node line
            let x = ch.xs[0] + travel * (ch.xs.last().unwrap() - ch.xs[0]);
            packets.push(PacketSpec { label: String::new(), x1: x / SVG_W * 100.0, y1: NODE_Y / SVG_H * 100.0, x2: x / SVG_W * 100.0, y2: NODE_Y / SVG_H * 100.0, p: 1.0, landed: false });
        }

        // node row: lit up to current leg
        let lit = if ch.segs.is_empty() { 0 } else { leg + 1 };
        let nodes = NODE_LABELS
            .iter()
            .enumerate()
            .map(|(i, l)| NodeSpec {
                label: l.to_string(),
                x: (80.0 + i as f64 * 88.8) / SVG_W * 100.0,
                y: NODE_Y / SVG_H * 100.0,
                status: None,
                lifeline: false,
                icon: None,
            })
            .collect();
        texts.push(TextSpec {
            text: (0..NODE_LABELS.len()).map(|i| if i < lit { "●" } else { "○" }).collect::<String>(),
            x: 50.0,
            y: (NODE_Y + 22.0) / SVG_H * 100.0,
            dim: true,
            left: false,
        });

        // ocean chapter: world map + route NY -> London -> Frankfurt
        if ci == 4 {
            paths.push(WORLD.to_string());
            let (ny, lon_d, fra) = (map_xy(-74.0, 40.7), map_xy(-0.1, 51.5), map_xy(8.7, 50.1));
            let route = vec![
                ((ny.0) / SVG_W * 100.0, (ny.1) / SVG_H * 100.0),
                ((ny.0 + (lon_d.0 - ny.0) * 0.5) / SVG_W * 100.0, (ny.1 + (lon_d.1 - ny.1) * 0.5 - 30.0) / SVG_H * 100.0),
                ((lon_d.0) / SVG_W * 100.0, (lon_d.1) / SVG_H * 100.0),
                ((fra.0) / SVG_W * 100.0, (fra.1) / SVG_H * 100.0),
            ];
            polylines.push(PolylineSpec { points: route, dim: false });
            texts.push(TextSpec { text: "new york".into(), x: ny.0 / SVG_W * 100.0, y: (ny.1 + 14.0) / SVG_H * 100.0, dim: true, left: false });
            texts.push(TextSpec { text: "london".into(), x: lon_d.0 / SVG_W * 100.0, y: (lon_d.1 + 14.0) / SVG_H * 100.0, dim: true, left: false });
            texts.push(TextSpec { text: "frankfurt".into(), x: fra.0 / SVG_W * 100.0, y: (fra.1 + 14.0) / SVG_H * 100.0, dim: true, left: false });
        }

        // progress dots for chapters
        texts.push(TextSpec {
            text: (0..CHAPTERS.len()).map(|i| if i == ci { "◉" } else { "·" }).collect::<String>(),
            x: 50.0,
            y: 96.0,
            dim: true,
            left: false,
        });

        Frame {
            header: Some("INTERACTIVE · A MESSAGE, END TO END".into()),
            nodes,
            packets,
            trails: vec![TrailSpec { x1: 76.0 / SVG_W * 100.0, y1: NODE_Y / SVG_H * 100.0, x2: 884.0 / SVG_W * 100.0, y2: NODE_Y / SVG_H * 100.0, arrow_end: false }],
            texts,
            polylines,
            paths,
            badge: None,
            note: ch.note.into(),
        }
    }
}
