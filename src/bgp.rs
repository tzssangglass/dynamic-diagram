//! Faithful port of fazamhd.com's BgpPropagationSim: AS1 originates a route to
//! 91.198.174.0/24, announcements propagate hop by hop (each AS prepends itself),
//! AS5 ends with two paths and picks the shortest.

use crate::frame::*;
use std::sync::OnceLock;

const FLY: u64 = 900; // announcement flight (original: a)
const PAUSE: u64 = 2400;

// original: caption time = max(1100, min(3400, chars*28)) ms
const fn cap(n: u64) -> u64 {
    if n < 1100 { 1100 } else if n > 3400 { 3400 } else { n }
}

const PREFIX: &str = "91.198.174.0/24";

struct As {
    label: &'static str,
    x: f64,
    y: f64,
}
const ASES: [As; 5] = [
    As { label: "AS1, origin", x: 11.0, y: 50.0 },
    As { label: "AS2", x: 38.0, y: 16.0 },
    As { label: "AS3", x: 38.0, y: 84.0 },
    As { label: "AS4", x: 68.0, y: 16.0 },
    As { label: "AS5, table", x: 93.0, y: 50.0 },
];
const LINKS: [(usize, usize); 5] = [(0, 1), (0, 2), (1, 3), (2, 4), (3, 4)];

const CAP0: &str = "AS1 originates 91.198.174.0/24 and announces it to its two neighbors, AS2 and AS3.";
const CAP1: &str = "AS2 and AS3 each learn the route directly from AS1, then add themselves before re-announcing: AS-path [1, 2] and AS-path [1, 3].";
const CAP2: &str = "AS4 adds itself, AS-path [1, 2, 4], and forwards on. AS5 has already heard a shorter route from AS3: [1, 3], two ASes.";
const CAP3: &str = "AS5 now holds two routes to 91.198.174.0/24: via AS3 (AS-path [1, 3], length 2) and via AS4 ([1, 2, 4], length 3). With policy stripped away, shortest AS-path wins. AS5's table: 91.198.174.0/24 ➔ next hop AS3.";

struct Stage {
    start: u64,
    // announcements in flight during this stage: (from, to, path label)
    flights: Vec<(usize, usize, &'static str)>,
    note: &'static str,
}

fn stages() -> &'static [Stage] {
    static S: OnceLock<Vec<Stage>> = OnceLock::new();
    S.get_or_init(|| {
        let mut t = 0u64;
        let mut v = Vec::new();
        let mut push = |flights: Vec<(usize, usize, &'static str)>, note: &'static str, extra: u64| {
            v.push(Stage { start: t, flights, note });
            t += cap(note.len() as u64 * 28) + extra;
        };
        push(vec![], CAP0, 0);
        push(vec![(0, 1, "[1]"), (0, 2, "[1]")], CAP1, 0);
        push(vec![(1, 3, "[1, 2]"), (2, 4, "[1, 3]")], CAP2, 0);
        push(vec![(3, 4, "[1, 2, 4]")], CAP3, PAUSE);
        v
    })
}

pub fn duration() -> u64 {
    stages().last().unwrap().start + cap(CAP3.len() as u64 * 28) + PAUSE
}

pub struct Bgp;

impl Sim for Bgp {
    fn duration(&self) -> u64 {
        duration()
    }

    fn frame(&self, t: u64) -> Frame {
        let st = stages();
        let tt = t % duration();
        let (si, stage) = st.iter().enumerate().rev().find(|(_, s)| tt >= s.start).unwrap();
        let dt = tt - stage.start;

        // announcement packets: fly during first 30..930ms of the stage
        let mut packets = vec![];
        if dt >= 30 && dt < 930 {
            let p = (dt - 30) as f64 / FLY as f64;
            for (from, to, label) in &stage.flights {
                packets.push(PacketSpec {
                    label: label.to_string(),
                    x1: ASES[*from].x,
                    y1: ASES[*from].y,
                    x2: ASES[*to].x,
                    y2: ASES[*to].y,
                    p,
                    landed: false,
                });
            }
        }

        // AS table states accumulate as announcements land (end of each stage's flight)
        let landed_through = |stage_idx: usize| tt >= st[stage_idx].start + 930;
        let as1 = "[1]";
        let as2 = if landed_through(1) { "[1, 2]" } else { "-" };
        let as3 = if landed_through(1) { "[1, 3]" } else { "-" };
        let as4 = if landed_through(2) { "[1, 2, 4]" } else { "-" };
        let as5_routes = if landed_through(3) { "2 routes" } else if landed_through(2) { "1 route" } else { "-" };
        let paths = [as1, as2, as3, as4, "-"];

        let nodes = ASES
            .iter()
            .enumerate()
            .map(|(i, a)| NodeSpec {
                label: a.label.into(),
                x: a.x,
                y: a.y,
                status: Some(if i == 4 { as5_routes.to_string() } else { format!("announces {}", paths[i]) }),
                lifeline: false,
                icon: None,
            })
            .collect();

        let mut texts = vec![TextSpec {
            text: match si {
                0 => "AS1 originating route",
                3 if landed_through(3) => "AS5 comparing AS-path length",
                _ => "announcement propagating",
            }
            .into(),
            x: 2.0,
            y: 5.0,
            dim: true,
            left: true,
        }];
        // decision panel once AS5 holds both routes
        if landed_through(3) {
            texts.push(TextSpec { text: format!("AS5's routes for {PREFIX}"), x: 62.0, y: 56.0, dim: true, left: true });
            texts.push(TextSpec { text: "via AS3, [1, 3], 2 ASes ← used".into(), x: 62.0, y: 63.0, dim: false, left: true });
            texts.push(TextSpec { text: "via AS4, [1, 2, 4], 3 ASes, longer".into(), x: 62.0, y: 70.0, dim: true, left: true });
        }

        Frame {
            header: Some("INTERACTIVE · BORDER GATEWAY PROTOCOL".into()),
            nodes,
            packets,
            trails: LINKS
                .iter()
                .map(|(a, b)| TrailSpec { x1: ASES[*a].x, y1: ASES[*a].y, x2: ASES[*b].x, y2: ASES[*b].y, arrow_end: false })
                .collect(),
            texts,
            polylines: vec![],
            paths: vec![],
            badge: None,
            note: stage.note.into(),
        }
    }
}
