//! Faithful port of fazamhd.com's AnycastCdnSim: three edge servers announce the
//! same IP; clients in Lisbon/Sydney/Buenos Aires each get routed to the nearest
//! announcer by BGP path length. World map path is the original asset.

use crate::frame::*;
use std::sync::OnceLock;

const IP: &str = "203.0.113.5";
const FLY: u64 = 750; // packet flight to winning edge (original: o)
const PAUSE: u64 = 1500;

const fn cap(n: u64) -> u64 {
    if n < 1100 { 1100 } else if n > 3800 { 3800 } else { n }
}

// original: x=(lon+180)/360*100, y=(84-lat)/140*100
const fn geo(lon: f64, lat: f64) -> (f64, f64) {
    ((lon + 180.0) / 360.0 * 100.0, (84.0 - lat) / 140.0 * 100.0)
}

struct Edge {
    id: &'static str,
    city: &'static str,
    pos: (f64, f64),
    label: (f64, f64),
}
struct Client {
    city: &'static str,
    pos: (f64, f64),
    label: (f64, f64),
    hops: [u64; 3], // to SP, London, Tokyo
}

const EDGES: [Edge; 3] = [
    Edge { id: "edge", city: "São Paulo", pos: geo(-46.6333, -23.5505), label: (20.0, 69.0) },
    Edge { id: "edge", city: "London", pos: geo(-0.1276, 51.5072), label: (60.2, 14.8) },
    Edge { id: "edge", city: "Tokyo", pos: geo(139.6917, 35.6895), label: (78.6, 24.5) },
];
const CLIENTS: [Client; 3] = [
    Client { city: "Lisbon", pos: geo(-9.1393, 38.7223), label: (41.7, 19.8), hops: [6, 2, 9] },
    Client { city: "Sydney", pos: geo(151.2093, -33.8688), label: (80.5, 70.5), hops: [8, 9, 3] },
    Client { city: "Buenos Aires", pos: geo(-58.3816, -34.6037), label: (50.0, 91.5), hops: [2, 7, 10] },
];

const WORLD: &str = include_str!("assets/world.path");

// quadratic bezier client->edge, control above midpoint (original formula)
fn route_points(c: (f64, f64), e: (f64, f64)) -> Vec<(f64, f64)> {
    let mx = (c.0 + e.0) / 2.0;
    let my = (c.1 + e.1) / 2.0 - (7.0_f64).min((c.0 - e.0).abs() / 5.0 + 2.0);
    (0..=24)
        .map(|i| {
            let t = i as f64 / 24.0;
            let (a, b, cc) = (1.0 - t, t, 1.0 - t);
            (
                a * a * c.0 + 2.0 * a * b * mx + b * b * e.0,
                a * cc * c.1 + 2.0 * a * b * my + b * b * e.1,
            )
        })
        .collect()
}

struct Episode {
    start: u64,
    t_routing: u64,
    t_forward: u64,
    note_arrive: String,
    note_route: String,
    note_deliver: String,
}

fn episodes() -> &'static [Episode] {
    static E: OnceLock<Vec<Episode>> = OnceLock::new();
    E.get_or_init(|| {
        let mut v = Vec::new();
        let mut t = 0u64;
        for (i, c) in CLIENTS.iter().enumerate() {
            let winner = c.hops.iter().enumerate().min_by_key(|(_, h)| *h).unwrap().0;
            let note_arrive = format!("client, {} sends a request to {IP}, the same address everyone uses.", c.city);
            let mut by_hops: Vec<usize> = (0..3).collect();
            by_hops.sort_by_key(|&e| c.hops[e]);
            let note_route = format!(
                "BGP compares AS path length to each announcer of {IP}: {}, shortest path wins, no different from any other route.",
                by_hops.iter().map(|&e| format!("{} ({})", EDGES[e].city, c.hops[e])).collect::<Vec<_>>().join(", ")
            );
            let note_deliver = if i == 0 {
                format!("Delivered to edge, {}, the topologically closest machine announcing that address. Route length alone chooses it, with no DNS trick involved.", EDGES[winner].city)
            } else {
                format!("Delivered to edge, {}, the topologically closest machine announcing that address, a different server than last time, same IP, no DNS trick involved.", EDGES[winner].city)
            };
            let t_routing = t + cap(note_arrive.len() as u64 * 28).max(700);
            let t_forward = t_routing + cap(note_route.len() as u64 * 28);
            let end = t_forward + cap(note_deliver.len() as u64 * 28).max(900) + PAUSE;
            v.push(Episode { start: t, t_routing, t_forward, note_arrive, note_route, note_deliver });
            t = end;
        }
        v
    })
}

pub fn duration() -> u64 {
    let eps = episodes();
    let last = eps.last().unwrap();
    last.t_forward + cap(last.note_deliver.len() as u64 * 28).max(900) + PAUSE
}

pub struct Anycast;

impl Sim for Anycast {
    fn duration(&self) -> u64 {
        duration()
    }

    fn frame(&self, t: u64) -> Frame {
        let eps = episodes();
        let tt = t % duration();
        let ep = eps.iter().rev().find(|e| tt >= e.start).unwrap();
        let ci = (tt, ep.start);
        let _ = ci;
        let client_idx = eps.iter().position(|e| std::ptr::eq(e, ep)).unwrap_or(0);
        let c = &CLIENTS[client_idx];
        let winner = c.hops.iter().enumerate().min_by_key(|(_, h)| *h).unwrap().0;

        let routing = tt >= ep.t_routing && tt < ep.t_forward;
        let forwarding = tt >= ep.t_forward;
        let note = if forwarding { &ep.note_deliver } else if routing { &ep.note_route } else { &ep.note_arrive };

        // routes: all 3 dim; winner bold during forwarding
        let polylines = (0..3)
            .map(|e| PolylineSpec { points: route_points(c.pos, EDGES[e].pos), dim: !(forwarding && e == winner) })
            .collect();

        // packet flies along the winning curve during forwarding (750ms)
        let mut packets = vec![];
        if forwarding {
            let pts = route_points(c.pos, EDGES[winner].pos);
            let p = ((tt - ep.t_forward) as f64 / FLY as f64).min(1.0);
            let idx = (p * (pts.len() - 1) as f64).round() as usize;
            let (x, y) = pts[idx.min(pts.len() - 1)];
            packets.push(PacketSpec { label: IP.into(), x1: x, y1: y, x2: x, y2: y, p: 1.0, landed: false });
        }

        // nodes: client pin + 3 edge pins
        let mut nodes = vec![NodeSpec { label: format!("client, {}", c.city), x: c.pos.0, y: c.pos.1, status: None, lifeline: false, icon: None }];
        for e in EDGES.iter() {
            nodes.push(NodeSpec { label: String::new(), x: e.pos.0, y: e.pos.1, status: None, lifeline: false, icon: None });
        }

        let mut texts = vec![
            TextSpec { text: format!("client, {}", c.city), x: c.label.0, y: c.label.1, dim: false, left: false },
        ];
        for (i, e) in EDGES.iter().enumerate() {
            texts.push(TextSpec { text: format!("{}, {}", e.id, e.city), x: e.label.0, y: e.label.1, dim: !(forwarding && i == winner), left: false });
            texts.push(TextSpec { text: IP.into(), x: e.label.0, y: e.label.1 + 4.0, dim: true, left: false });
        }
        // hop counts at route midpoints during routing
        if routing {
            for (i, e) in EDGES.iter().enumerate() {
                let pts = route_points(c.pos, e.pos);
                let (x, y) = pts[pts.len() / 2];
                texts.push(TextSpec { text: c.hops[i].to_string(), x, y: y - 2.0, dim: false, left: false });
            }
        }

        Frame {
            header: Some("INTERACTIVE · ANYCAST ·· ONE ADDRESS, MANY MACHINES".into()),
            nodes,
            packets,
            trails: vec![],
            texts,
            polylines,
            paths: vec![WORLD.to_string()],
            badge: None,
            note: note.clone(),
        }
    }
}
