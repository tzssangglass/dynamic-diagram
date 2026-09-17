//! Faithful port of fazamhd.com's SwitchLearnSim: a switch learns which MAC is on
//! which port by watching source addresses; unknown destination -> flood, known ->
//! direct forward. Table resets each cycle: flood, direct, flood, direct.

use crate::frame::*;
use std::sync::OnceLock;

const FLY_IN: u64 = 550;
const FLY_OUT: u64 = 750;
const PAUSE: u64 = 1300;
const SW: f64 = 50.0;

const fn cap(n: u64) -> u64 {
    if n < 950 { 950 } else if n > 4200 { 4200 } else { n }
}

struct Port {
    id: &'static str,
    mac: &'static str,
    x: f64,
    y: f64,
}
const PORTS: [Port; 4] = [
    Port { id: "port 1", mac: "A1:11:1A", x: 6.0, y: 22.0 },
    Port { id: "port 2", mac: "B2:22:2B", x: 6.0, y: 78.0 },
    Port { id: "port 3", mac: "C3:33:3C", x: 94.0, y: 22.0 },
    Port { id: "port 4", mac: "D4:44:4D", x: 94.0, y: 78.0 },
];
const EPISODES: [(usize, usize); 4] = [(0, 3), (3, 0), (1, 2), (2, 1)];

// packet start x: 15 for left-side ports, 85 for right (original: v())
fn edge_x(p: &Port) -> f64 {
    if p.x < SW { 15.0 } else { 85.0 }
}

struct Episode {
    start: u64,
    t_read: u64,
    t_learn: u64,
    t_lookup: u64,
    t_forward: u64,
    known: bool,
    caps: [String; 5],
}

fn episodes() -> &'static [Episode] {
    static E: OnceLock<Vec<Episode>> = OnceLock::new();
    E.get_or_init(|| {
        let mut v = Vec::new();
        let mut t = 0u64;
        let mut table: Vec<(&str, &str)> = Vec::new();
        for &(si, di) in EPISODES.iter() {
            let (s, d) = (&PORTS[si], &PORTS[di]);
            let known_to = table.iter().find(|(mac, _)| *mac == d.mac).map(|(_, p)| *p);
            let known = known_to.is_some();
            let caps = [
                format!("Frame arrives on {}: source {}, destination {}.", s.id, s.mac, d.mac),
                format!("Switch reads the frame: source {} on {}, destination {}.", s.mac, s.id, d.mac),
                format!("Learns: {} is on {}, adds it to the table, even though this frame isn't addressed to the switch itself.", s.mac, s.id),
                if known {
                    format!("Checks the table for {}, found it on {}, learned earlier. Forwards directly, {} only.", d.mac, known_to.unwrap(), known_to.unwrap())
                } else {
                    format!("Checks the table for {}, not there yet. Floods the frame out every port except {}, so it's guaranteed to arrive.", d.mac, s.id)
                },
                if known {
                    format!("Forwarded out {} only, the other ports never see this frame.", known_to.unwrap())
                } else {
                    format!("Flooded out every port but {}, every other device sees it, but only {} will recognize itself as the destination.", s.id, d.mac)
                },
            ];
            let t_read = t + cap(caps[0].len() as u64 * 28).max(700);
            let t_learn = t_read + cap(caps[1].len() as u64 * 28);
            let t_lookup = t_learn + cap(caps[2].len() as u64 * 28);
            let t_forward = t_lookup + cap(caps[3].len() as u64 * 28);
            let end = t_forward + cap(caps[4].len() as u64 * 28).max(FLY_OUT) + PAUSE;
            v.push(Episode { start: t, t_read, t_learn, t_lookup, t_forward, known, caps });
            table.push((s.mac, s.id));
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

pub struct SwitchLearn;

impl Sim for SwitchLearn {
    fn duration(&self) -> u64 {
        duration()
    }

    fn frame(&self, t: u64) -> Frame {
        let eps = episodes();
        let tt = t % duration();
        let ei = eps.iter().rposition(|e| tt >= e.start).unwrap();
        let ep = &eps[ei];
        let (si, di) = EPISODES[ei];
        let (s, d) = (&PORTS[si], &PORTS[di]);
        let dt = tt - ep.start;

        let (stage, note) = if dt >= ep.t_forward - ep.start {
            ("forwarding", &ep.caps[4])
        } else if dt >= ep.t_lookup - ep.start {
            ("lookup", &ep.caps[3])
        } else if dt >= ep.t_learn - ep.start {
            ("learning", &ep.caps[2])
        } else if dt >= ep.t_read - ep.start {
            ("reading", &ep.caps[1])
        } else {
            ("arriving", &ep.caps[0])
        };

        let dst_label = format!("dst {}", d.mac);
        let mut packets = vec![];
        if stage == "arriving" {
            let p = if dt < 30 { 0.0 } else { ((dt - 30) as f64 / FLY_IN as f64).min(1.0) };
            packets.push(PacketSpec { label: dst_label.clone(), x1: edge_x(s), y1: s.y, x2: SW, y2: s.y, p, landed: p >= 1.0 });
        } else if stage == "forwarding" {
            let targets: Vec<&Port> = if ep.known {
                vec![d]
            } else {
                PORTS.iter().filter(|p| p.id != s.id).collect()
            };
            for tp in targets {
                let fdt = dt - (ep.t_forward - ep.start);
                let p = if fdt < 30 { 0.0 } else { ((fdt - 30) as f64 / FLY_OUT as f64).min(1.0) };
                packets.push(PacketSpec { label: dst_label.clone(), x1: SW, y1: tp.y, x2: edge_x(tp), y2: tp.y, p, landed: false });
            }
        }

        // forwarding table state: entries learned by earlier episodes (+ this one past learning)
        let mut learned: Vec<(&str, &str)> = Vec::new();
        for (j, _e) in eps.iter().enumerate() {
            if j < ei || (j == ei && matches!(stage, "learning" | "lookup" | "forwarding")) {
                let (s2, _) = EPISODES[j];
                learned.push((PORTS[s2].mac, PORTS[s2].id));
            }
        }
        let mut texts = vec![TextSpec { text: "forwarding table · built by watching, not configured".into(), x: 2.0, y: 88.0, dim: true, left: true }];
        for (i, p) in PORTS.iter().enumerate() {
            let entry = learned.iter().find(|(mac, _)| *mac == p.mac);
            texts.push(TextSpec {
                text: format!("{}  {}", p.mac, entry.map(|(_, id)| *id).unwrap_or("not learned")),
                x: 2.0 + i as f64 * 25.0,
                y: 94.0,
                dim: entry.is_none(),
                left: true,
            });
        }

        Frame {
            header: Some("INTERACTIVE · NETWORK SWITCH".into()),
            nodes: std::iter::once(NodeSpec {
                label: "switch".into(),
                x: SW,
                y: 50.0,
                status: Some(
                    match stage {
                        "reading" => "reading frame",
                        "learning" => "learning source",
                        "lookup" => if ep.known { "known, direct" } else { "unknown, flood" },
                        _ => "waiting",
                    }
                    .into(),
                ),
                lifeline: false,
                icon: None,
            })
            .chain(PORTS.iter().map(|p| NodeSpec { label: p.mac.into(), x: p.x, y: p.y, status: Some(p.id.into()), lifeline: false, icon: None }))
            .collect(),
            packets,
            trails: PORTS.iter().map(|p| TrailSpec { x1: p.x, y1: p.y, x2: SW, y2: p.y, arrow_end: false }).collect(),
            texts,
            polylines: vec![],
            paths: vec![],
            badge: None,
            note: note.clone(),
        }
    }
}
