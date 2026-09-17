//! Faithful port of fazamhd.com's LinkClickSim: what happens when you click a
//! link — the click, DNS (0→6 ms), TCP handshake (6→42), TLS (42→66), the request
//! (66→88), the response (88→186), and the page building (186→242). Stage
//! durations and simulated-ms mapping ported verbatim; visuals simplified to the
//! packet/timing-bar core (original also draws a browser window and finger art).

use crate::frame::*;

const SVG_W: f64 = 760.0;
const SVG_H: f64 = 240.0;
const BROWSER_X: f64 = 130.0;
const RESOLVER_X: f64 = 300.0;
const SERVER_X: f64 = 682.0;
const WIRE_Y: f64 = 130.0;

struct Stage {
    key: &'static str,
    label: &'static str,
    dur: u64,
    // progress (0..1) -> simulated ms
    ms: fn(f64) -> f64,
}
fn lerp(a: f64, b: f64, p: f64) -> f64 {
    a + (b - a) * p
}

const STAGES: [Stage; 7] = [
    Stage { key: "click", label: "the click", dur: 3600, ms: |_| 0.0 },
    Stage { key: "dns", label: "step 1, DNS", dur: 6000, ms: |p| lerp(0.0, 6.0, p) },
    Stage { key: "tcp", label: "step 2, TCP handshake", dur: 7800, ms: |p| lerp(6.0, 42.0, p) },
    Stage { key: "tls", label: "step 3, TLS handshake", dur: 7000, ms: |p| lerp(42.0, 66.0, p) },
    Stage { key: "get", label: "step 4, the request", dur: 5400, ms: |p| if p < 0.55 { lerp(66.0, 78.0, p / 0.55) } else { lerp(78.0, 88.0, (p - 0.55) / 0.45) } },
    Stage { key: "resp", label: "step 5, the response", dur: 12600, ms: |p| lerp(88.0, 186.0, p) },
    Stage { key: "build", label: "the page builds", dur: 11000, ms: |p| lerp(186.0, 242.0, p.min(0.78) / 0.78) },
];

// timing-bar segment boundaries in simulated ms (0..242)
const MS_BOUNDS: [f64; 8] = [0.0, 6.0, 42.0, 66.0, 78.0, 88.0, 186.0, 242.0];

fn fmt_ms(ms: f64) -> String {
    if ms == 0.0 {
        "0 ms".into()
    } else if ms < 10.0 {
        format!("{ms:.1} ms")
    } else {
        format!("{} ms", ms.round())
    }
}

pub struct LinkClick;

impl Sim for LinkClick {
    fn duration(&self) -> u64 {
        STAGES.iter().map(|s| s.dur).sum()
    }

    fn frame(&self, t: u64) -> Frame {
        let total: u64 = STAGES.iter().map(|s| s.dur).sum();
        let tt = t % total;
        let mut acc = 0;
        let mut si = 0;
        let mut p = 1.0;
        for (i, s) in STAGES.iter().enumerate() {
            if tt < acc + s.dur {
                si = i;
                p = (tt - acc) as f64 / s.dur as f64;
                break;
            }
            acc += s.dur;
        }
        let stage = &STAGES[si];
        let ms_now = (stage.ms)(p);

        // per-stage packet story (position along the wire)
        let mut packets = vec![];
        let fly = |label: &str, from: f64| PacketSpec {
            label: label.into(),
            x1: from / SVG_W * 100.0,
            y1: WIRE_Y / SVG_H * 100.0,
            x2: from / SVG_W * 100.0,
            y2: WIRE_Y / SVG_H * 100.0,
            p: 1.0,
            landed: false,
        };
        match stage.key {
            "dns" => {
                // query out (0..0.5), answer back (0.5..1)
                if p < 0.5 {
                    let x = lerp(BROWSER_X, RESOLVER_X, p / 0.5);
                    packets.push(fly("en.wikipedia.org?", x));
                } else {
                    let x = lerp(RESOLVER_X, BROWSER_X, (p - 0.5) / 0.5);
                    packets.push(fly("91.198.174.192", x));
                }
            }
            "tcp" => {
                // SYN / SYN-ACK / ACK thirds
                if p < 0.33 {
                    packets.push(fly("SYN", lerp(BROWSER_X, SERVER_X, p / 0.33)));
                } else if p < 0.66 {
                    packets.push(fly("SYN-ACK", lerp(SERVER_X, BROWSER_X, (p - 0.33) / 0.33)));
                } else {
                    packets.push(fly("ACK", lerp(BROWSER_X, SERVER_X, (p - 0.66) / 0.34)));
                }
            }
            "tls" => {
                if p < 0.5 {
                    packets.push(fly("ClientHello + key share", lerp(BROWSER_X, SERVER_X, p / 0.5)));
                } else {
                    packets.push(fly("ServerHello + cert + Finished", lerp(SERVER_X, BROWSER_X, (p - 0.5) / 0.5)));
                }
            }
            "get" => {
                packets.push(fly("GET /wiki/… (encrypted)", lerp(BROWSER_X, SERVER_X, p)));
            }
            "resp" => {
                packets.push(fly("200 OK + HTML chunks", lerp(SERVER_X, BROWSER_X, p)));
            }
            "build" => {}
            _ => {}
        }

        // timing bar: segments between MS_BOUNDS scaled to 10..90% width
        let bar_y = 210.0;
        let (bx0, bx1) = (10.0_f64, 90.0_f64);
        let ms_to_x = |ms: f64| bx0 + (ms - 0.0) / 242.0 * (bx1 - bx0);
        let mut trails = vec![TrailSpec { x1: bx0, y1: bar_y / SVG_H * 100.0, x2: bx1, y2: bar_y / SVG_H * 100.0, arrow_end: false }];
        let mut texts = vec![
            TextSpec { text: stage.label.into(), x: 50.0, y: 8.0, dim: false, left: false },
            TextSpec { text: format!("simulated time: {}", fmt_ms(ms_now)), x: 50.0, y: 16.0, dim: true, left: false },
        ];
        for &b in MS_BOUNDS.iter() {
            let x = ms_to_x(b);
            trails.push(TrailSpec { x1: x, y1: (bar_y - 6.0) / SVG_H * 100.0, x2: x, y2: (bar_y + 6.0) / SVG_H * 100.0, arrow_end: false });
            texts.push(TextSpec { text: format!("{}", b.round() as u64), x, y: (bar_y + 18.0) / SVG_H * 100.0, dim: true, left: false });
        }
        // playhead on the bar
        let ph = ms_to_x(ms_now);
        trails.push(TrailSpec { x1: ph, y1: (bar_y - 10.0) / SVG_H * 100.0, x2: ph, y2: (bar_y + 10.0) / SVG_H * 100.0, arrow_end: false });

        // page build progress (last stage): growing bar in the browser area
        if stage.key == "build" {
            texts.push(TextSpec {
                text: format!("█{}", "░".repeat((p * 20.0) as usize)),
                x: BROWSER_X / SVG_W * 100.0,
                y: (WIRE_Y - 40.0) / SVG_H * 100.0,
                dim: false,
                left: true,
            });
        }

        Frame {
            header: Some("INTERACTIVE · WHAT ACTUALLY HAPPENS WHEN YOU CLICK A LINK".into()),
            nodes: vec![
                NodeSpec { label: "your browser".into(), x: BROWSER_X / SVG_W * 100.0, y: WIRE_Y / SVG_H * 100.0, status: None, lifeline: false, icon: None },
                NodeSpec { label: "resolver".into(), x: RESOLVER_X / SVG_W * 100.0, y: (WIRE_Y - 50.0) / SVG_H * 100.0, status: None, lifeline: false, icon: None },
                NodeSpec { label: "server".into(), x: SERVER_X / SVG_W * 100.0, y: WIRE_Y / SVG_H * 100.0, status: None, lifeline: false, icon: None },
            ],
            packets,
            trails,
            texts,
            polylines: vec![],
            paths: vec![],
            badge: None,
            note: format!("{} — {}", stage.label, fmt_ms(ms_now)),
        }
    }
}
