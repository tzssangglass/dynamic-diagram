//! Faithful port of fazamhd.com's MtuFragmentationSim: a 3000B packet meets a
//! 1500B MTU. Scenario 1: the router splits it into two fragments. Scenario 2:
//! DF is set, packet dropped, ICMP "Fragmentation Needed" flies back. Alternate.

use crate::frame::*;
use std::sync::OnceLock;

const FLY: u64 = 700;
const PAUSE: u64 = 1600;
const SENDER: f64 = 5.0;
const ROUTER: f64 = 48.0;
const DEST: f64 = 95.0;
const MTU: u64 = 1500;
const SIZE: u64 = 3000;

const fn cap(n: u64) -> u64 {
    if n < 1050 { 1050 } else if n > 4200 { 4200 } else { n }
}

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    Fragment,
    Pmtud,
}

// stage boundaries within one scenario, from captions
struct Sched {
    t_at_router: u64,
    t_acting: u64,
    t_deliver: u64,
    t_f2: u64,        // fragment scenario: second fragment departs
    t_reassemble: u64,
    end: u64,
    mode: Mode,
    caps: [String; 7],
}

fn scheds() -> &'static [Sched] {
    static S: OnceLock<Vec<Sched>> = OnceLock::new();
    S.get_or_init(|| {
        let mut v = Vec::new();
        for &mode in [Mode::Fragment, Mode::Pmtud].iter() {
            let caps = [
                if mode == Mode::Fragment {
                    format!("Sender ships a {SIZE} byte packet toward the router. The next link's MTU is only {MTU} bytes.")
                } else {
                    format!("Sender ships a {SIZE} byte packet marked \"don't fragment\" (DF). The next link's MTU is only {MTU} bytes.")
                },
                format!("Router sees {SIZE} bytes won't fit through a {MTU} byte link."),
                if mode == Mode::Fragment {
                    format!("Router slices it into two fragments, {MTU}B and {}B, each a valid packet, and forwards both on.", SIZE - MTU)
                } else {
                    format!("DF forbids splitting it. Router drops the packet and sends back ICMP \"Fragmentation Needed, next-hop MTU {MTU}.\"")
                },
                format!("First fragment, {MTU}B, departs."),
                format!("Second fragment, {}B, departs right behind the first.", SIZE - MTU),
                format!("Destination reassembles both fragments back into the original {SIZE} byte packet using the fragment offsets in their headers."),
                format!("Sender receives the ICMP notice and remembers: for this destination, keep future packets at or under {MTU} bytes."),
            ];
            let c = |i: usize| cap(caps[i].len() as u64 * 28);
            let t_at_router = c(0).max(850);
            let t_acting = t_at_router + c(1);
            let t_deliver = t_acting + c(2).max(850);
            let (t_f2, t_reassemble, end) = if mode == Mode::Fragment {
                let d = t_deliver + c(3).max(850);
                let o = d + c(4).max(850);
                (d, o, o + c(5))
            } else {
                (t_deliver, t_deliver, t_deliver + c(6))
            };
            v.push(Sched { t_at_router, t_acting, t_deliver, t_f2, t_reassemble, end: end + PAUSE, mode, caps });
        }
        v
    })
}

pub fn duration() -> u64 {
    scheds().iter().map(|s| s.end).sum()
}

pub struct Mtu;

impl Sim for Mtu {
    fn duration(&self) -> u64 {
        duration()
    }

    fn frame(&self, t: u64) -> Frame {
        let ss = scheds();
        let tt = t % duration();
        // current scenario
        let mut base = 0u64;
        let mut si = 0;
        for (i, s) in ss.iter().enumerate() {
            if tt < base + s.end {
                si = i;
                break;
            }
            base += s.end;
        }
        let s = &ss[si];
        let dt = tt - base;
        let frag = s.mode == Mode::Fragment;

        let mut packets = vec![];
        let (stage, note) = if dt < s.t_at_router {
            // sending: packet flies sender->router
            let p = if dt < 30 { 0.0 } else { ((dt - 30) as f64 / FLY as f64).min(1.0) };
            packets.push(PacketSpec {
                label: format!("{SIZE}B{}", if frag { "" } else { ", DF" }),
                x1: SENDER,
                y1: 50.0,
                x2: ROUTER,
                y2: 50.0,
                p,
                landed: p >= 1.0,
            });
            ("sending", s.caps[0].clone())
        } else if dt < s.t_acting {
            ("atRouter", s.caps[1].clone())
        } else if dt < s.t_deliver {
            // pmtud: ICMP flies router->sender during acting
            if !frag {
                let e = dt - s.t_acting;
                let p = if e < 30 { 0.0 } else { ((e - 30) as f64 / FLY as f64).min(1.0) };
                packets.push(PacketSpec { label: "ICMP".into(), x1: ROUTER, y1: 50.0, x2: SENDER, y2: 50.0, p, landed: false });
            }
            ("acting", s.caps[2].clone())
        } else {
            // delivering: fragments fly (fragment mode), or final note (pmtud)
            if frag {
                let e = dt - s.t_deliver;
                let p1 = if e < 30 { 0.0 } else { ((e - 30) as f64 / FLY as f64).min(1.0) };
                packets.push(PacketSpec { label: format!("{MTU}B"), x1: ROUTER, y1: 44.0, x2: DEST, y2: 44.0, p: p1, landed: false });
                if dt >= s.t_f2 {
                    let e2 = dt - s.t_f2;
                    let p2 = if e2 < 30 { 0.0 } else { ((e2 - 30) as f64 / FLY as f64).min(1.0) };
                    packets.push(PacketSpec { label: format!("{}B", SIZE - MTU), x1: ROUTER, y1: 56.0, x2: DEST, y2: 56.0, p: p2, landed: false });
                }
                let note = if dt >= s.t_reassemble { s.caps[5].clone() } else if dt >= s.t_f2 { s.caps[4].clone() } else { s.caps[3].clone() };
                ("delivering", note)
            } else {
                ("delivering", s.caps[6].clone())
            }
        };

        let outcome = if matches!(stage, "acting" | "delivering") {
            if frag {
                format!("split into {MTU}B + {}B", SIZE - MTU)
            } else {
                format!("dropped, ICMP \"Fragmentation Needed, MTU {MTU}\"")
            }
        } else {
            "waiting".to_string()
        };

        Frame {
            header: Some(format!("INTERACTIVE · MTU ·· {}, MTU {MTU}B", if frag { "FRAGMENTATION" } else { "PATH MTU DISCOVERY" })),
            nodes: vec![
                NodeSpec { label: "sender".into(), x: SENDER, y: 50.0, status: None, lifeline: false, icon: None },
                NodeSpec {
                    label: "router".into(),
                    x: ROUTER,
                    y: 50.0,
                    status: Some(
                        match stage {
                            "atRouter" => "checking size",
                            "acting" => if frag { "fragmenting" } else { "dropping, DF set" },
                            "delivering" => if frag { "forwarded" } else { "notified sender" },
                            _ => "awaiting packet",
                        }
                        .into(),
                    ),
                    lifeline: false,
                    icon: None,
                },
                NodeSpec { label: "destination".into(), x: DEST, y: 50.0, status: None, lifeline: false, icon: None },
            ],
            packets,
            trails: vec![
                TrailSpec { x1: SENDER, y1: 50.0, x2: ROUTER, y2: 50.0, arrow_end: false },
                TrailSpec { x1: ROUTER, y1: 50.0, x2: DEST, y2: 50.0, arrow_end: false },
            ],
            texts: vec![
                TextSpec { text: format!("packet size: {SIZE}B{} · next-hop MTU: {MTU}B", if frag { "" } else { ", DF" }), x: 2.0, y: 72.0, dim: true, left: true },
                TextSpec { text: format!("outcome: {outcome}"), x: 2.0, y: 80.0, dim: !matches!(stage, "acting" | "delivering"), left: true },
            ],
            polylines: vec![],
            paths: vec![],
            badge: None,
            note,
        }
    }
}
