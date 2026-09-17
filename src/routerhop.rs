//! Faithful port of fazamhd.com's RouterHopSim: a packet arrives, header read,
//! TTL decremented, longest-prefix-match against the routing table, forwarded out
//! the winning line. Four destinations cycle.

use crate::frame::*;
use std::sync::OnceLock;

const TTL: u8 = 58;
const FLY_IN: u64 = 550;
const FLY_OUT: u64 = 750;
const PAUSE: u64 = 1400;
const ROUTER_X: f64 = 44.0;
const PREV_X: f64 = 6.0;
const OUT_X: f64 = 92.0;

const fn cap(n: u64) -> u64 {
    if n < 1000 { 1000 } else if n > 4200 { 4200 } else { n }
}

struct Route {
    cidr: &'static str,
    octets: [u8; 4],
    prefix: usize,
    line: &'static str,
    y: f64,
}
const ROUTES: [Route; 4] = [
    Route { cidr: "0.0.0.0/0", octets: [0, 0, 0, 0], prefix: 0, line: "line 1", y: 14.0 },
    Route { cidr: "91.198.0.0/16", octets: [91, 198, 0, 0], prefix: 16, line: "line 2", y: 38.0 },
    Route { cidr: "91.198.174.0/24", octets: [91, 198, 174, 0], prefix: 24, line: "line 3", y: 62.0 },
    Route { cidr: "10.0.0.0/8", octets: [10, 0, 0, 0], prefix: 8, line: "line 5", y: 86.0 },
];

const PACKETS: [[u8; 4]; 4] = [[91, 198, 174, 192], [91, 198, 5, 10], [10, 1, 2, 3], [8, 8, 8, 8]];

fn matches(octets: &[u8; 4], r: &Route) -> bool {
    let ob: Vec<u8> = octets.iter().flat_map(|o| (0..8).rev().map(move |i| (o >> i) & 1)).collect();
    let rb: Vec<u8> = r.octets.iter().flat_map(|o| (0..8).rev().map(move |i| (o >> i) & 1)).collect();
    (0..r.prefix).all(|i| ob[i] == rb[i])
}

fn best_route(octets: &[u8; 4]) -> &'static Route {
    ROUTES
        .iter()
        .filter(|r| matches(octets, r))
        .max_by_key(|r| r.prefix)
        .unwrap()
        // ROUTES is 'static
}

struct Episode {
    start: u64,
    t_read: u64,
    t_ttl: u64,
    t_lookup: u64,
    t_forward: u64,
    caps: [String; 5],
}

fn ip_text(o: &[u8; 4]) -> String {
    o.iter().map(|b| b.to_string()).collect::<Vec<_>>().join(".")
}

fn episodes() -> &'static [Episode] {
    static E: OnceLock<Vec<Episode>> = OnceLock::new();
    E.get_or_init(|| {
        let mut v = Vec::new();
        let mut t = 0u64;
        for pkt in PACKETS.iter() {
            let ip = ip_text(pkt);
            let best = best_route(pkt);
            let matching: Vec<&Route> = ROUTES.iter().filter(|r| matches(pkt, r)).collect();
            let caps = [
                format!("Packet arrives on the \"in\" line: destination {ip}, ttl {TTL}."),
                format!("Router reads the header: destination {ip}, ttl {TTL}."),
                format!("TTL decremented to {}, one hop closer to being discarded if this packet is ever looping.", TTL - 1),
                if matching.len() > 1 {
                    format!(
                        "Checking the table: {} all match. Longest prefix wins, {} is the most specific, so {}.",
                        matching.iter().map(|r| r.cidr).collect::<Vec<_>>().join(", "),
                        best.cidr,
                        best.line
                    )
                } else {
                    format!("Checking the table: only 0.0.0.0/0 matches, the catch-all default. Forwarding out line 1.")
                },
                format!("Forwarded out {}, toward {}, carrying ttl {}.", best.line, best.cidr, TTL - 1),
            ];
            let t_read = t + cap(caps[0].len() as u64 * 28).max(700);
            let t_ttl = t_read + cap(caps[1].len() as u64 * 28);
            let t_lookup = t_ttl + cap(caps[2].len() as u64 * 28);
            let t_forward = t_lookup + cap(caps[3].len() as u64 * 28);
            let end = t_forward + cap(caps[4].len() as u64 * 28).max(FLY_OUT) + PAUSE;
            v.push(Episode { start: t, t_read, t_ttl, t_lookup, t_forward, caps });
            t = end;
        }
        v
    })
}

pub fn duration() -> u64 {
    let eps = episodes();
    let last = eps.last().unwrap();
    last.t_forward + cap(last.caps[4].len() as u64 * 28).max(FLY_OUT) + PAUSE
}

pub struct RouterHop;

impl Sim for RouterHop {
    fn duration(&self) -> u64 {
        duration()
    }

    fn frame(&self, t: u64) -> Frame {
        let eps = episodes();
        let tt = t % duration();
        let ei = eps.iter().rposition(|e| tt >= e.start).unwrap();
        let ep = &eps[ei];
        let pkt = &PACKETS[ei];
        let ip = ip_text(pkt);
        let best = best_route(pkt);
        let dt = tt - ep.start;

        let (stage, note) = if dt >= ep.t_forward - ep.start {
            ("forwarding", &ep.caps[4])
        } else if dt >= ep.t_lookup - ep.start {
            ("lookup", &ep.caps[3])
        } else if dt >= ep.t_ttl - ep.start {
            ("ttl", &ep.caps[2])
        } else if dt >= ep.t_read - ep.start {
            ("reading", &ep.caps[1])
        } else {
            ("arriving", &ep.caps[0])
        };
        let ttl_shown = if matches!(stage, "ttl" | "lookup" | "forwarding") { TTL - 1 } else { TTL };

        // packet: in-flight to router, then out to the winning line
        let mut packets = vec![];
        if stage == "arriving" {
            let p = if dt < 30 { 0.0 } else { ((dt - 30) as f64 / FLY_IN as f64).min(1.0) };
            packets.push(PacketSpec { label: format!("{ip} · ttl {ttl_shown}"), x1: PREV_X, y1: 50.0, x2: ROUTER_X, y2: 50.0, p, landed: p >= 1.0 });
        } else if stage == "forwarding" {
            let p = if dt < (ep.t_forward - ep.start) + 30 { 0.0 } else { ((dt - (ep.t_forward - ep.start) - 30) as f64 / FLY_OUT as f64).min(1.0) };
            packets.push(PacketSpec { label: format!("{ip} · ttl {ttl_shown}"), x1: ROUTER_X, y1: 50.0, x2: OUT_X, y2: best.y, p, landed: false });
        }

        // table texts: match/winner states during lookup/forwarding
        let mut texts = vec![TextSpec { text: "routing table · matched here".into(), x: 2.0, y: 68.0, dim: true, left: true }];
        for r in ROUTES.iter() {
            let is_match = matches(pkt, r);
            let is_winner = r.cidr == best.cidr;
            let strong = (stage == "lookup" && is_match) || (stage == "forwarding" && is_winner);
            texts.push(TextSpec {
                text: format!("{}  {}  {}{}", r.cidr, r.line, if is_match { "match" } else { "" }, if is_winner && stage != "arriving" && stage != "reading" && stage != "ttl" { " ←" } else { "" }),
                x: 2.0,
                y: 74.0 + ROUTES.iter().position(|q| q.cidr == r.cidr).unwrap() as f64 * 5.0,
                dim: !strong,
                left: true,
            });
        }

        Frame {
            header: Some("INTERACTIVE · ROUTER".into()),
            nodes: vec![
                NodeSpec { label: "previous hop".into(), x: PREV_X, y: 50.0, status: None, lifeline: false, icon: None },
                NodeSpec {
                    label: "router".into(),
                    x: ROUTER_X,
                    y: 50.0,
                    status: Some(
                        match stage {
                            "reading" => "reading header",
                            "ttl" => "ttl −1",
                            "lookup" => "table lookup",
                            _ => "waiting",
                        }
                        .into(),
                    ),
                    lifeline: false,
                    icon: None,
                },
            ],
            packets,
            trails: std::iter::once(TrailSpec { x1: PREV_X, y1: 50.0, x2: ROUTER_X, y2: 50.0, arrow_end: false })
                .chain(ROUTES.iter().map(|r| TrailSpec { x1: ROUTER_X, y1: 50.0, x2: OUT_X, y2: r.y, arrow_end: false }))
                .collect(),
            texts,
            polylines: vec![],
            paths: vec![],
            badge: None,
            note: note.clone(),
        }
    }
}
