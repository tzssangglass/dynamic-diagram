//! Faithful port of fazamhd.com's IgpCompareSim: OSPF vs RIP reacting to a link
//! drop. Shared prologue (steady state, a packet walked R1→R2→R3→N, R3's link
//! drops), then OSPF converges with two floods, or RIP loops, bounces a packet
//! to TTL 0, and counts to infinity. Both stories play in alternation.

use crate::frame::*;
use std::sync::OnceLock;

const HOP_MS: u64 = 760; // flight (30 + 730, original K())
const FLASH_MS: u64 = 1200;
const PAUSE: u64 = 2600;

const fn cap(n: u64) -> u64 {
    if n < 1600 { 1600 } else if n > 3600 { 3600 } else { n }
}

const R1: f64 = 8.0;
const R2: f64 = 38.0;
const R3: f64 = 68.0;
const NET: f64 = 94.0;
const WIRE_Y: f64 = 45.0;

const CAP_STEADY: &str = "Steady state, every router already knows its hop count to net N.";
const CAP_FWD1: &str = "R1 forwards to R2, the only next hop R1 knows about.";
const CAP_FWD2: &str = "R2 does the same with its own table. It has no memory of where the packet came from, and no idea where it's ultimately headed.";
const CAP_FWD3: &str = "Delivered. Three independent lookups, no router ever saw the whole path.";
const CAP_DROP: &str = "R3's link to net N drops. R3 knows immediately.";
const CAP_MAP: &str = "R2 already has a full map of the network, it just recomputes without that link.";
const CAP_CONV_OSPF: &str = "Converged, two floods, every router certain.";
const CAP_LOOP3: &str = "R2's routine update still says 2, stale, from before the break. R3 trusts it: 2 + 1 = 3. A loop has formed: R3 now routes toward R2, and R2 still routes toward R3.";
const CAP_LOOP_PKT: &str = "A real packet for net N arrives at R2 right now and gets caught in that loop.";
const CAP_TTL0: &str = "TTL hit zero, discarded. The loop won't clear until the hop count climbs high enough to be declared infinite.";
const CAP_4: &str = "R2 hears R3's new number and adds one: 4.";
const CAP_5: &str = "R3 adds one again: 5, and this keeps going, every round, up toward 16.";
const CAP_16: &str = "Eventually the count reaches 16, RIP's definition of infinity. Only then do R2 and R3 agree net N is truly gone.";
const CAP_CONV_RIP: &str = "The news reaches R1 the same way every number here traveled, one hop at a time.";


struct Shot {
    t: u64,
    note: &'static str,
    fly: Option<(f64, f64, &'static str)>, // in flight (departed at t)
    d1: u8,  // R1 hops (255 = unreachable)
    d2: u8,
    d3: u8,
    ttl_pkt: Option<(f64, u8)>, // loop packet x + ttl
    ospf: bool,
}

fn build() -> Vec<Shot> {
    let mut v: Vec<Shot> = Vec::new();
    let mut t = 0u64;
    let d = &mut t;
    let cap = |s: &str| cap(s.len() as u64 * 32);

    // ---- shared prologue ----
    v.push(Shot { t: 0, note: CAP_STEADY, fly: None, d1: 3, d2: 2, d3: 1, ttl_pkt: None, ospf: true });
    *d += cap(CAP_STEADY);
    // walk R1->R2->R3->N
    for (from, to, note) in [(R1, R2, CAP_FWD1), (R2, R3, CAP_FWD2), (R3, NET, CAP_FWD3)] {
        v.push(Shot { t: *d, note, fly: Some((from, to, "data")), d1: 3, d2: 2, d3: 1, ttl_pkt: None, ospf: true });
        *d += HOP_MS;
        v.push(Shot { t: *d, note, fly: None, d1: 3, d2: 2, d3: 1, ttl_pkt: None, ospf: true });
        *d += cap(note);
    }
    // link drop
    v.push(Shot { t: *d, note: CAP_DROP, fly: None, d1: 3, d2: 2, d3: 255, ttl_pkt: None, ospf: true });
    *d += cap(CAP_DROP) + FLASH_MS;

    // ---- OSPF story ----
    v.push(Shot { t: *d, note: CAP_MAP, fly: Some((R3, R2, "flood")), d1: 3, d2: 2, d3: 255, ttl_pkt: None, ospf: true });
    *d += HOP_MS;
    v.push(Shot { t: *d, note: CAP_MAP, fly: None, d1: 3, d2: 255, d3: 255, ttl_pkt: None, ospf: true });
    *d += cap(CAP_MAP);
    v.push(Shot { t: *d, note: CAP_CONV_OSPF, fly: Some((R2, R1, "flood")), d1: 3, d2: 255, d3: 255, ttl_pkt: None, ospf: true });
    *d += HOP_MS;
    v.push(Shot { t: *d, note: CAP_CONV_OSPF, fly: None, d1: 255, d2: 255, d3: 255, ttl_pkt: None, ospf: true });
    *d += cap(CAP_CONV_OSPF) + PAUSE;

    // ---- RIP story (rewind to just after the drop) ----
    let t_drop_end = {
        // recompute: position after prologue = t at CAP_DROP start + cap + FLASH
        let mut x = 0;
        x += cap(CAP_STEADY);
        for (_, _, note) in [(R1, R2, CAP_FWD1), (R2, R3, CAP_FWD2), (R3, NET, CAP_FWD3)] {
            x += HOP_MS + cap(note);
        }
        x += cap(CAP_DROP) + FLASH_MS;
        x
    };
    let mut rt = t_drop_end;
    v.push(Shot { t: rt, note: CAP_LOOP3, fly: Some((R2, R3, "vector")), d1: 3, d2: 2, d3: 255, ttl_pkt: None, ospf: false });
    rt += HOP_MS;
    v.push(Shot { t: rt, note: CAP_LOOP3, fly: None, d1: 3, d2: 2, d3: 3, ttl_pkt: None, ospf: false });
    rt += cap(CAP_LOOP3);
    v.push(Shot { t: rt, note: CAP_LOOP_PKT, fly: None, d1: 3, d2: 2, d3: 3, ttl_pkt: Some((R2, 4)), ospf: false });
    // bounce: R2->R3 (ttl 4->3), R3->R2 (3->2), R2->R3 (2->1), R3->R2 (1->0 dead)
    for (_from, to, ttl) in [(R2, R3, 3u8), (R3, R2, 2), (R2, R3, 1), (R3, R2, 0)] {
        rt += 500;
        v.push(Shot { t: rt, note: CAP_LOOP_PKT, fly: None, d1: 3, d2: 2, d3: 3, ttl_pkt: Some((to, ttl)), ospf: false });
    }
    rt += 800;
    v.push(Shot { t: rt, note: CAP_TTL0, fly: None, d1: 3, d2: 2, d3: 3, ttl_pkt: None, ospf: false });
    rt += cap(CAP_TTL0);
    for (from, to, note, d3v, d2v) in [
        (R3, R2, CAP_4, 3u8, 4u8),
        (R2, R3, CAP_5, 5, 4),
    ] {
        v.push(Shot { t: rt, note, fly: Some((from, to, "vector")), d1: 3, d2: d2v, d3: d3v, ttl_pkt: None, ospf: false });
        rt += HOP_MS;
        v.push(Shot { t: rt, note, fly: None, d1: 3, d2: d2v, d3: d3v, ttl_pkt: None, ospf: false });
        rt += cap(note);
    }
    // count reaches 16: R2,R3 unreachable
    v.push(Shot { t: rt, note: CAP_16, fly: Some((R3, R2, "vector")), d1: 3, d2: 5, d3: 5, ttl_pkt: None, ospf: false });
    rt += HOP_MS;
    v.push(Shot { t: rt, note: CAP_16, fly: None, d1: 3, d2: 255, d3: 255, ttl_pkt: None, ospf: false });
    rt += cap(CAP_16);
    v.push(Shot { t: rt, note: CAP_CONV_RIP, fly: Some((R2, R1, "vector")), d1: 3, d2: 255, d3: 255, ttl_pkt: None, ospf: false });
    rt += HOP_MS;
    v.push(Shot { t: rt, note: CAP_CONV_RIP, fly: None, d1: 255, d2: 255, d3: 255, ttl_pkt: None, ospf: false });
    rt += cap(CAP_CONV_RIP) + PAUSE;
    *d = rt;
    v.sort_by_key(|s| s.t);
    v
}

fn shots() -> &'static [Shot] {
    static S: OnceLock<Vec<Shot>> = OnceLock::new();
    S.get_or_init(build)
}

pub fn duration() -> u64 {
    shots().last().unwrap().t + cap(CAP_CONV_RIP.len() as u64 * 32) + PAUSE
}

pub struct Igp;

impl Sim for Igp {
    fn duration(&self) -> u64 {
        duration()
    }

    fn frame(&self, t: u64) -> Frame {
        let sh = shots();
        let tt = t % duration();
        let cur = sh.iter().rev().find(|s| tt >= s.t).unwrap();

        let hop_text = |d: u8| -> String {
            match d {
                255 => "unreachable".into(),
                1 => "1 hop".into(),
                n => format!("{n} hops"),
            }
        };
        let ospf_phase = cur.ospf;
        let header_proto = if ospf_phase { "OSPF · LINK-STATE" } else { "RIP · DISTANCE-VECTOR" };

        let mut packets = vec![];
        if let Some((from, to, kind)) = cur.fly {
            let p = ((tt - cur.t) as f64 / HOP_MS as f64).min(1.0);
            packets.push(PacketSpec {
                label: match kind {
                    "flood" => format!("{kind}"),
                    "vector" => format!("{kind}"),
                    _ => "data".into(),
                },
                x1: from,
                y1: WIRE_Y,
                x2: to,
                y2: WIRE_Y,
                p,
                landed: false,
            });
        }
        if let Some((x, ttl)) = cur.ttl_pkt {
            packets.push(PacketSpec { label: format!("pkt ttl {ttl}{}", if ttl == 0 { " · dead" } else { "" }), x1: x, y1: WIRE_Y, x2: x, y2: WIRE_Y, p: 1.0, landed: false });
        }

        Frame {
            header: Some(format!("INTERACTIVE · INTERIOR GATEWAY PROTOCOL ·· {header_proto}")),
            nodes: vec![
                NodeSpec { label: "R1".into(), x: R1, y: WIRE_Y, status: Some(hop_text(cur.d1)), lifeline: false, icon: None },
                NodeSpec { label: "R2".into(), x: R2, y: WIRE_Y, status: Some(hop_text(cur.d2)), lifeline: false, icon: None },
                NodeSpec { label: "R3".into(), x: R3, y: WIRE_Y, status: Some(hop_text(cur.d3)), lifeline: false, icon: None },
                NodeSpec { label: "net N".into(), x: NET, y: WIRE_Y, status: None, lifeline: false, icon: None },
            ],
            packets,
            trails: vec![
                TrailSpec { x1: R1, y1: WIRE_Y, x2: R2, y2: WIRE_Y, arrow_end: false },
                TrailSpec { x1: R2, y1: WIRE_Y, x2: R3, y2: WIRE_Y, arrow_end: false },
                TrailSpec { x1: R3, y1: WIRE_Y, x2: NET, y2: WIRE_Y, arrow_end: false },
            ],
            texts: vec![],
            polylines: vec![],
            paths: vec![],
            badge: None,
            note: cur.note.into(),
        }
    }
}
