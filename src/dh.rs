//! Faithful port of fazamhd.com's DiffieHellmanSim: g=5, p=23 public; a=6 / b=15
//! private; A=5⁶ mod 23=8 crosses right, B=19 crosses back; both derive 2; the
//! eavesdropper holds everything on the wire and still can't crack it.

use crate::frame::*;
use std::sync::OnceLock;

const FLY: u64 = 1400;
const PAUSE: u64 = 2600;
const BROWSER: f64 = 13.0;
const SERVER: f64 = 87.0;

const fn cap(n: u64) -> u64 {
    if n < 1600 { 1600 } else if n > 5200 { 5200 } else { n }
}

const CAP_PUBLICS: &str = "Everything starts in the open. g = 5 and p = 23 are public constants; the eavesdropper holds them too.";
const CAP_PICK: &str = "Each side picks a private number and keeps it, a = 6 in the browser, b = 15 in the server. These two numbers never touch the wire.";
const CAP_SHARE_A: &str = "The browser computes its share A = 5⁶ mod 23 = 8 and sends it. The eavesdropper records it.";
const CAP_SHARE_B: &str = "The server computes B = 5¹⁵ mod 23 = 19 and sends it back, recorded too. The wire has now carried everything it will ever carry: 5, 23, 8, and 19.";
const CAP_DERIVE: &str = "Each side raises the share it received to its own private number, 19⁶ mod 23 = 2 in the browser, 8¹⁵ mod 23 = 2 in the server. Both equal 5⁹⁰ mod 23, the same number, never sent.";
const CAP_CRACK: &str = "The eavesdropper holds 5, 23, 8, and 19, and needs a or b to go further: which power of 5 gives 8, mod 23? Trial finds it instantly at this size; with p hundreds of digits long, no known method finishes. The shares alone are worthless.";

#[derive(Clone, Copy, PartialEq)]
enum Stage {
    Publics,
    Pick,
    ShareA,
    ShareB,
    Derive,
    Crack,
}

struct Sched {
    t_pick: u64,
    t_share_a: u64,
    t_share_b: u64,
    t_derive: u64,
    t_crack: u64,
    pub duration: u64,
}

fn sched() -> &'static Sched {
    static S: OnceLock<Sched> = OnceLock::new();
    S.get_or_init(|| {
        let t_pick = cap(CAP_PUBLICS.len() as u64 * 28);
        let t_share_a = t_pick + cap(CAP_PICK.len() as u64 * 28);
        let t_share_b = t_share_a + cap(CAP_SHARE_A.len() as u64 * 28).max(2300);
        let t_derive = t_share_b + cap(CAP_SHARE_B.len() as u64 * 28).max(2300);
        let t_crack = t_derive + cap(CAP_DERIVE.len() as u64 * 28);
        let duration = t_crack + cap(CAP_CRACK.len() as u64 * 28) + PAUSE;
        Sched { t_pick, t_share_a, t_share_b, t_derive, t_crack, duration }
    })
}

pub struct Dh;

impl Sim for Dh {
    fn duration(&self) -> u64 {
        sched().duration
    }

    fn frame(&self, t: u64) -> Frame {
        let s = sched();
        let tt = t % s.duration;
        let (stage, note) = if tt >= s.t_crack {
            (Stage::Crack, CAP_CRACK)
        } else if tt >= s.t_derive {
            (Stage::Derive, CAP_DERIVE)
        } else if tt >= s.t_share_b {
            (Stage::ShareB, CAP_SHARE_B)
        } else if tt >= s.t_share_a {
            (Stage::ShareA, CAP_SHARE_A)
        } else if tt >= s.t_pick {
            (Stage::Pick, CAP_PICK)
        } else {
            (Stage::Publics, CAP_PUBLICS)
        };
        let ord = |x: Stage| x as u8;

        // flights: A=8 leaves at shareA+500 (1400ms), B=19 at shareB+500
        let mut packets = vec![];
        let fly = |start: u64, label: &str, right: bool| -> Option<PacketSpec> {
            if tt < start || tt > start + FLY {
                return None;
            }
            let p = (tt - start) as f64 / FLY as f64;
            Some(PacketSpec {
                label: label.into(),
                x1: if right { BROWSER } else { SERVER },
                y1: 50.0,
                x2: if right { SERVER } else { BROWSER },
                y2: 50.0,
                p,
                landed: false,
            })
        };
        if stage as u8 >= ord(Stage::ShareA) {
            if let Some(p) = fly(s.t_share_a + 500, "A = 8", true) {
                packets.push(p);
            }
        }
        if stage as u8 >= ord(Stage::ShareB) {
            if let Some(p) = fly(s.t_share_b + 500, "B = 19", false) {
                packets.push(p);
            }
        }

        let at_least = |st: Stage| ord(stage) >= ord(st);
        let mut texts = vec![
            TextSpec {
                text: match stage {
                    Stage::Publics => "public constants",
                    Stage::Pick => "private numbers picked",
                    Stage::ShareA => "share A crosses the wire",
                    Stage::ShareB => "share B crosses the wire",
                    Stage::Derive => "both sides derive the key",
                    Stage::Crack => "the eavesdropper tries",
                }
                .into(),
                x: 2.0,
                y: 6.0,
                dim: true,
                left: true,
            },
            TextSpec { text: "g = 5, p = 23 · public".into(), x: 50.0, y: 12.0, dim: false, left: false },
        ];
        // per-side fact columns
        let col = |x: f64, facts: Vec<&str>| -> Vec<TextSpec> {
            facts
                .iter()
                .enumerate()
                .map(|(i, f)| TextSpec { text: f.to_string(), x, y: 40.0 + i as f64 * 5.0, dim: false, left: false })
                .collect()
        };
        let mut bf = vec![];
        if at_least(Stage::Pick) {
            bf.push("a = 6 · private");
        }
        if at_least(Stage::ShareA) {
            bf.push("A = 5⁶ mod 23 = 8");
        }
        if at_least(Stage::Derive) {
            bf.push("Bᵃ = 19⁶ mod 23 = 2 · key");
        }
        let mut sf = vec![];
        if at_least(Stage::Pick) {
            sf.push("b = 15 · private");
        }
        if at_least(Stage::ShareB) {
            sf.push("B = 5¹⁵ mod 23 = 19");
        }
        if at_least(Stage::Derive) {
            sf.push("Aᵇ = 8¹⁵ mod 23 = 2 · key");
        }
        texts.extend(col(BROWSER, bf));
        texts.extend(col(SERVER, sf));
        // eavesdropper row
        texts.push(TextSpec { text: "eavesdropper · reads everything on the wire".into(), x: 50.0, y: 74.0, dim: true, left: false });
        let mut eve = vec!["g = 5, p = 23"];
        if at_least(Stage::ShareA) && tt > s.t_share_a + 500 + FLY {
            eve.push("A = 8");
        }
        if at_least(Stage::ShareB) && tt > s.t_share_b + 500 + FLY {
            eve.push("B = 19");
        }
        texts.push(TextSpec { text: eve.join("   ·   "), x: 50.0, y: 80.0, dim: false, left: false });
        if at_least(Stage::Crack) {
            texts.push(TextSpec { text: "✕ 5ˣ mod 23 = 8 → x = ? · infeasible at real sizes".into(), x: 50.0, y: 88.0, dim: false, left: false });
        }

        Frame {
            header: Some("INTERACTIVE · DIFFIE-HELLMAN ·· THE KEY EXCHANGE".into()),
            nodes: vec![
                NodeSpec { label: "browser".into(), x: BROWSER, y: 30.0, status: None, lifeline: true, icon: None },
                NodeSpec { label: "server".into(), x: SERVER, y: 30.0, status: None, lifeline: true, icon: None },
            ],
            packets,
            trails: vec![TrailSpec { x1: BROWSER, y1: 50.0, x2: SERVER, y2: 50.0, arrow_end: false }],
            texts,
            polylines: vec![],
            paths: vec![],
            badge: None,
            note: note.into(),
        }
    }
}
