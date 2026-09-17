//! Faithful port of fazamhd.com's NatCollisionSim: two homes behind NAT hand out
//! the identical private address; both routers rewrite to their own public
//! address (tables shown), then home A's phone shares one public address,
//! distinguished by port.

use crate::frame::*;

const HOP_MS: u64 = 450; // per-hop flight (original se(): all hops clamp to 450)
const SWAP_DWELL: u64 = 1600; // pause at the NAT router while rewriting
const DWELL: u64 = 150;
const START_DELAY: u64 = 1400; // before first packet leaves

// timeline (ms), reconstructed from the original's chained timeouts
pub const T_PKT_A: u64 = START_DELAY; // laptopA departs
pub const T_SWAP_A: u64 = T_PKT_A + 40 + HOP_MS; // 1890: routerA rewrites
pub const T_PKT_B: u64 = 1900 + 1400; // laptopB departs (3300 in original: U(1900..) after c=1400)
const T_SWAP_B: u64 = T_PKT_B + 40 + HOP_MS;
const T_B_WEB: u64 = T_SWAP_B + SWAP_DWELL + 40 + HOP_MS + DWELL + 40 + HOP_MS;
pub const T_PHASE2: u64 = T_B_WEB + 2600 + 1000; // phone sends
pub const T_SWAP_A2: u64 = T_PHASE2 + START_DELAY + 40 + HOP_MS; // phone swapped at routerA
const T_PHONE_WEB: u64 = T_SWAP_A2 + SWAP_DWELL + 40 + HOP_MS + DWELL + 40 + HOP_MS;
pub const DURATION: u64 = T_PHONE_WEB + 2600 + 1800;

// positions (scene %)
const LAPTOP_A: (f64, f64) = (10.0, 14.0);
const PHONE_A: (f64, f64) = (10.0, 38.0);
const ROUTER_A: (f64, f64) = (32.0, 26.0);
const LAPTOP_B: (f64, f64) = (10.0, 76.0);
const ROUTER_B: (f64, f64) = (32.0, 76.0);
const RX: (f64, f64) = (58.0, 52.0);
const WEB: (f64, f64) = (86.0, 52.0);

// piecewise hop position along a path; hops with dwell after landing
fn along(path: &[(f64, f64)], start: u64, t: u64, swap_at: Option<usize>) -> Option<((f64, f64), bool)> {
    // returns (position, public_label)
    let mut cursor = start;
    let mut public = false;
    for (i, w) in path.windows(2).enumerate() {
        let (from, to) = (w[0], w[1]);
        let depart = cursor + 40;
        let arrive = depart + HOP_MS;
        if t < depart {
            return Some((from, public));
        }
        if t < arrive {
            let p = (t - depart) as f64 / HOP_MS as f64;
            return Some(((from.0 + (to.0 - from.0) * p, from.1 + (to.1 - from.1) * p), public));
        }
        public = public || swap_at == Some(i + 1);
        cursor = arrive + if swap_at == Some(i + 1) { SWAP_DWELL } else { DWELL };
    }
    Some((*path.last().unwrap(), public))
}

pub struct Nat;

impl Sim for Nat {
    fn duration(&self) -> u64 {
        DURATION
    }

    fn frame(&self, t: u64) -> Frame {
        let tt = t % DURATION;
        let phase2 = tt >= T_PHASE2;

        // captions at natural event times (original queues them; sequence preserved)
        let note = if tt >= T_PHONE_WEB {
            "The server sees two ordinary, distinct public addresses. The shared private address never left either home, so it never collides."
        } else if tt >= T_SWAP_A2 {
            "Same public address, 203.0.113.7, the whole home shares it. The router hands this conversation a different port, and the table is what tells each reply apart."
        } else if tt >= T_PHASE2 {
            "Now the phone in home A sends, from its own private address, 192.168.1.23."
        } else if tt >= T_B_WEB {
            "The server sees two ordinary, distinct public addresses. The shared private address never left either home, so it never collides."
        } else if tt >= T_SWAP_A {
            "Each router rewrites the source to its own public address, 203.0.113.7 for A, 198.51.100.42 for B, and notes the conversation in its table so the reply can find its way back."
        } else {
            "Both laptops send at once: identical private address, even the same ephemeral port, in two unrelated homes. Nothing about the source says which home it belongs to."
        };

        // packets in flight
        let mut packets = vec![];
        let path_a = [LAPTOP_A, ROUTER_A, RX, WEB];
        let path_b = [LAPTOP_B, ROUTER_B, RX, WEB];
        let path_p = [PHONE_A, ROUTER_A, RX, WEB];
        if !phase2 {
            if let Some((pos, public)) = along(&path_a, T_PKT_A, tt, Some(1)) {
                if tt < T_B_WEB + 2600 {
                    packets.push(PacketSpec {
                        label: if public { "public source" } else { "private source" }.into(),
                        x1: pos.0, y1: pos.1, x2: pos.0, y2: pos.1, p: 1.0, landed: false,
                    });
                }
            }
            if let Some((pos, public)) = along(&path_b, T_PKT_B, tt, Some(1)) {
                if tt < T_B_WEB + 2600 {
                    packets.push(PacketSpec {
                        label: if public { "public source" } else { "private source" }.into(),
                        x1: pos.0, y1: pos.1, x2: pos.0, y2: pos.1, p: 1.0, landed: false,
                    });
                }
            }
        } else if tt < T_PHONE_WEB + 2600 {
            if let Some((pos, public)) = along(&path_p, T_PHASE2 + START_DELAY, tt, Some(1)) {
                packets.push(PacketSpec {
                    label: if public { "public source" } else { "private source" }.into(),
                    x1: pos.0, y1: pos.1, x2: pos.0, y2: pos.1, p: 1.0, landed: false,
                });
            }
        }

        // NAT tables
        let table_a = if tt >= T_SWAP_A2 {
            "192.168.1.5:54211→203.0.113.7:40001 · 192.168.1.23:50307→203.0.113.7:40002"
        } else if tt >= T_SWAP_A {
            "192.168.1.5:54211→203.0.113.7:40001"
        } else {
            "nat table empty"
        };
        let table_b = if tt >= T_SWAP_B && tt < T_PHASE2 + 500 {
            "192.168.1.5:54211→198.51.100.42:40001"
        } else if tt >= T_PHASE2 + 500 {
            "192.168.1.5:54211→198.51.100.42:40001"
        } else {
            "nat table empty"
        };

        Frame {
            header: Some("INTERACTIVE · NAT".into()),
            nodes: vec![
                NodeSpec { label: "laptop".into(), x: LAPTOP_A.0, y: LAPTOP_A.1, status: Some("192.168.1.5".into()), lifeline: false, icon: None },
                NodeSpec { label: "phone".into(), x: PHONE_A.0, y: PHONE_A.1, status: Some("192.168.1.23".into()), lifeline: false, icon: None },
                NodeSpec { label: "NAT router".into(), x: ROUTER_A.0, y: ROUTER_A.1, status: Some(table_a.into()), lifeline: false, icon: None },
                NodeSpec { label: "laptop".into(), x: LAPTOP_B.0, y: LAPTOP_B.1, status: Some("192.168.1.5".into()), lifeline: false, icon: None },
                NodeSpec { label: "NAT router".into(), x: ROUTER_B.0, y: ROUTER_B.1, status: Some(table_b.into()), lifeline: false, icon: None },
                NodeSpec { label: String::new(), x: RX.0, y: RX.1, status: None, lifeline: false, icon: None },
                NodeSpec { label: "web server".into(), x: WEB.0, y: WEB.1, status: None, lifeline: false, icon: None },
            ],
            packets,
            trails: vec![
                TrailSpec { x1: LAPTOP_A.0, y1: LAPTOP_A.1, x2: ROUTER_A.0, y2: ROUTER_A.1, arrow_end: false },
                TrailSpec { x1: PHONE_A.0, y1: PHONE_A.1, x2: ROUTER_A.0, y2: ROUTER_A.1, arrow_end: false },
                TrailSpec { x1: LAPTOP_B.0, y1: LAPTOP_B.1, x2: ROUTER_B.0, y2: ROUTER_B.1, arrow_end: false },
                TrailSpec { x1: ROUTER_A.0, y1: ROUTER_A.1, x2: RX.0, y2: RX.1, arrow_end: false },
                TrailSpec { x1: ROUTER_B.0, y1: ROUTER_B.1, x2: RX.0, y2: RX.1, arrow_end: false },
                TrailSpec { x1: RX.0, y1: RX.1, x2: WEB.0, y2: WEB.1, arrow_end: false },
            ],
            texts: vec![
                TextSpec { text: "private lan a".into(), x: 8.0, y: 2.0, dim: true, left: false },
                TextSpec { text: "private lan b".into(), x: 8.0, y: 64.0, dim: true, left: false },
                TextSpec { text: "public internet".into(), x: 62.0, y: 2.0, dim: true, left: false },
                TextSpec {
                    text: if phase2 { "one home, one public address, shared" } else { "two homes, the same private address" }.into(),
                    x: 70.0, y: 90.0, dim: true, left: true,
                },
            ],
            polylines: vec![],
            paths: vec![],
            badge: None,
            note: note.into(),
        }
    }
}
