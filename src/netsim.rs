//! Faithful port of fazamhd.com's NetSim (packet-switching mode): 2 clients,
//! 2 servers, a 4x3 router grid (seeded layout, original mulberry32(1337)); flows
//! spawn every 3.5s, packets route hop-by-hop with load-aware greedy choice and
//! 1.8% per-hop loss. Randomness is replayed once at init with the original PRNG,
//! making frame(t) deterministic. (Circuit mode is interactive-only in the
//! original; not ported.)

use crate::frame::*;
use std::collections::HashMap;
use std::sync::OnceLock;

const HOP_BASE: f64 = 1200.0;
const FLOW_EVERY: u64 = 3500;
const DROP_P: f64 = 0.018;
const SIM_MS: u64 = 90000; // virtual time replayed per loop

// original mulberry32
struct Rng(u32);
impl Rng {
    fn next(&mut self) -> f64 {
        self.0 = self.0.wrapping_add(1831565813);
        let mut t = self.0;
        t = t.wrapping_mul((t ^ (t >> 15)) | 1);
        t = t.wrapping_add(((t ^ (t >> 7)).wrapping_mul(t | 61)) ^ t);
        ((t ^ (t >> 14)) as f64) / 4294967296.0
    }
}

struct GNode {
    x: f64,
    y: f64,
    label: String,
    router: bool,
}

struct Graph {
    nodes: HashMap<String, GNode>,
    adj: HashMap<String, Vec<String>>,
    edges: Vec<(String, String)>,
}

fn build_graph() -> Graph {
    let mut rng = Rng(1337);
    let jitter = |rng: &mut Rng| rng.next() * 3.0 - 1.5;
    let mut nodes = HashMap::new();
    let mut adj: HashMap<String, Vec<String>> = HashMap::new();
    let add_edge = |adj: &mut HashMap<String, Vec<String>>, a: &str, b: &str| {
        adj.entry(a.into()).or_default().push(b.into());
        adj.entry(b.into()).or_default().push(a.into());
    };
    let cols = [24.0, 42.0, 60.0, 78.0];
    let rows = [24.0, 50.0, 76.0];
    let mut grid: Vec<Vec<String>> = Vec::new();
    let mut c = 1;
    for (ci, &cx) in cols.iter().enumerate() {
        grid.push(Vec::new());
        for (ri, &ry) in rows.iter().enumerate() {
            let id = format!("r{ci}-{ri}");
            nodes.insert(id.clone(), GNode { x: cx + jitter(&mut rng), y: ry + jitter(&mut rng), label: format!("R{c}"), router: true });
            c += 1;
            grid[ci].push(id);
        }
    }
    for (i, y) in [32.0, 68.0].iter().enumerate() {
        nodes.insert(format!("c{i}"), GNode { x: 5.0, y: *y, label: format!("{}", ['A', 'B'][i]), router: false });
    }
    for (i, y) in [32.0, 68.0].iter().enumerate() {
        nodes.insert(format!("s{i}"), GNode { x: 95.0, y: *y, label: format!("{}", ['1', '2'][i]), router: false });
    }
    for ci in 0..cols.len() {
        for ri in 0..rows.len() {
            let id = grid[ci][ri].clone();
            if ci + 1 < cols.len() {
                for dj in [ri as i64 - 1, ri as i64, ri as i64 + 1] {
                    if dj >= 0 && (dj as usize) < rows.len() {
                        add_edge(&mut adj, &id, &grid[ci + 1][dj as usize]);
                    }
                }
            }
            if ri + 1 < rows.len() {
                add_edge(&mut adj, &id, &grid[ci][ri + 1]);
            }
        }
    }
    // clients/servers attach to 2 nearest routers by y
    let nearest = |col: &Vec<String>, y: f64| -> Vec<String> {
        let mut v = col.clone();
        v.sort_by_key(|id| ((nodes[id].y - y).abs() * 100.0) as i64);
        v.truncate(2);
        v
    };
    for i in 0..2 {
        for r in nearest(&grid[0], if i == 0 { 32.0 } else { 68.0 }) {
            add_edge(&mut adj, &format!("c{i}"), &r);
        }
        for r in nearest(&grid[3], if i == 0 { 32.0 } else { 68.0 }) {
            add_edge(&mut adj, &format!("s{i}"), &r);
        }
    }
    // unique edges
    let mut seen = std::collections::HashSet::new();
    let mut edges = Vec::new();
    for (a, nbrs) in &adj {
        for b in nbrs {
            let key = if a < b { format!("{a}|{b}") } else { format!("{b}|{a}") };
            if seen.insert(key) {
                edges.push((a.clone(), b.clone()));
            }
        }
    }
    Graph { nodes, adj, edges }
}

// replay events
struct Hop {
    #[allow(dead_code)]
    pid: usize,
    start: u64,
    dur: u64,
    from: (f64, f64),
    to: (f64, f64),
}
struct Ev {
    t: u64,
    kind: EvKind,
}
enum EvKind {
    Delivered,
    Dropped,
    Caption(&'static str),
}

struct Replay {
    hops: Vec<Hop>,
    events: Vec<Ev>,
}

fn replay() -> &'static Replay {
    static R: OnceLock<Replay> = OnceLock::new();
    R.get_or_init(|| {
        let g = build_graph();
        let mut rng = Rng(9001);
        let mut hops = Vec::new();
        let mut events = Vec::new();
        let mut pid = 0usize;
        let dist = |g: &Graph, a: &str, b: &str| -> f64 {
            let (na, nb) = (&g.nodes[a], &g.nodes[b]);
            ((na.x - nb.x).powi(2) + (na.y - nb.y).powi(2)).sqrt()
        };
        let next_hop = |g: &Graph, rng: &mut Rng, load: &HashMap<String, i32>, cur: &str, dest: &str| -> Option<String> {
            let d0 = dist(g, cur, dest);
            let ok = |n: &str| g.nodes[n].router || n == dest;
            let mut cand: Vec<&String> = g.adj[cur].iter().filter(|n| ok(n) && dist(g, n, dest) < d0).collect();
            if cand.is_empty() {
                cand = g.adj[cur].iter().filter(|n| ok(n)).collect();
            }
            if cand.is_empty() {
                return None;
            }
            let lk = |a: &str, b: &str| if a < b { format!("{a}|{b}") } else { format!("{b}|{a}") };
            let mut best = cand[0].clone();
            let mut best_cost = f64::MAX;
            for n in &cand {
                let cost = dist(g, n, dest) + *load.get(&lk(cur, n)).unwrap_or(&0) as f64 * 1.6 + rng.next() * 1.4;
                if cost < best_cost {
                    best_cost = cost;
                    best = (*n).clone();
                }
            }
            Some(best)
        };
        // schedule: flows every 3.5s while <2 active
        struct FlowSpawn {
            t: u64,
            src: String,
            dest: String,
            count: u64,
        }
        let mut spawns: Vec<FlowSpawn> = Vec::new();
        let mut t = 0u64;
        let mut active_until: Vec<u64> = Vec::new();
        while t < SIM_MS {
            active_until.retain(|x| *x > t);
            if active_until.len() < 2 {
                let src = format!("c{}", (rng.next() * 2.0) as usize);
                let dest = format!("s{}", (rng.next() * 2.0) as usize);
                let count = 3 + (rng.next() * 2.0) as u64;
                let cap = format!("Client {} ➔ Server {}, sending {} packets. Each routes independently.", g.nodes[&src].label, g.nodes[&dest].label, count);
                spawns.push(FlowSpawn { t, src: src.clone(), dest: dest.clone(), count });
                active_until.push(t + count * 600 + 8000);
                events.push(Ev { t, kind: EvKind::Caption(Box::leak(cap.into_boxed_str())) });
            }
            t += FLOW_EVERY;
        }
        for f in spawns {
            for i in 0..f.count {
                let start = f.t + i * 600;
                pid += 1;
                // walk the packet hop by hop
                let mut cur = f.src.clone();
                let mut time = start + 20;
                let mut guard = 0;
                loop {
                    guard += 1;
                    if guard > 20 {
                        break;
                    }
                    if rng.next() < DROP_P {
                        events.push(Ev { t: time, kind: EvKind::Dropped });
                        if rng.next() < 0.55 {
                            events.push(Ev { t: time, kind: EvKind::Caption("A packet was dropped in transit, IP is best-effort, so nothing here notices or resends it.") });
                        }
                        break;
                    }
                    let Some(next) = next_hop(&g, &mut rng, &HashMap::new(), &cur, &f.dest) else { break };
                    let dur = (HOP_BASE + (rng.next() * 120.0 - 40.0)).max(300.0) as u64;
                    let (fa, ta) = (&g.nodes[&cur], &g.nodes[&next]);
                    hops.push(Hop { pid, start: time, dur, from: (fa.x, fa.y), to: (ta.x, ta.y) });
                    time += dur;
                    if next == f.dest {
                        events.push(Ev { t: time, kind: EvKind::Delivered });
                        break;
                    }
                    cur = next;
                }
            }
        }
        hops.sort_by_key(|h| h.start);
        events.sort_by_key(|e| e.t);
        Replay { hops, events }
    })
}

pub struct NetSim;

impl Sim for NetSim {
    fn duration(&self) -> u64 {
        SIM_MS
    }

    fn frame(&self, t: u64) -> Frame {
        let r = replay();
        let g = build_graph();
        let tt = t % SIM_MS;

        let mut packets = vec![];
        let mut delivered = 0u64;
        let mut dropped = 0u64;
        let mut caption = "Two clients, two servers, a dozen routers between them, every packet routed hop by hop.";
        for e in &r.events {
            if e.t > tt {
                break;
            }
            match &e.kind {
                EvKind::Delivered => delivered += 1,
                EvKind::Dropped => dropped += 1,
                EvKind::Caption(s) => caption = s,
            }
        }
        // in-flight packets
        for h in &r.hops {
            if tt >= h.start && tt < h.start + h.dur {
                let p = (tt - h.start) as f64 / h.dur as f64;
                packets.push(PacketSpec {
                    label: String::new(),
                    x1: h.from.0,
                    y1: h.from.1,
                    x2: h.to.0,
                    y2: h.to.1,
                    p,
                    landed: false,
                });
            }
        }

        let nodes = g
            .nodes
            .values()
            .map(|n| NodeSpec { label: n.label.clone(), x: n.x, y: n.y, status: None, lifeline: false, icon: None })
            .collect();
        let trails = g
            .edges
            .iter()
            .map(|(a, b)| {
                let (na, nb) = (&g.nodes[a], &g.nodes[b]);
                TrailSpec { x1: na.x, y1: na.y, x2: nb.x, y2: nb.y, arrow_end: false }
            })
            .collect();

        Frame {
            header: Some("INTERACTIVE · PACKET SWITCHING".into()),
            nodes,
            packets,
            trails,
            texts: vec![
                TextSpec { text: format!("delivered: {delivered}   dropped: {dropped}"), x: 2.0, y: 4.0, dim: true, left: true },
            ],
            polylines: vec![],
            paths: vec![],
            badge: None,
            note: caption.into(),
        }
    }
}
