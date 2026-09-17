//! Faithful port of fazamhd.com's IpBitsSim: longest-prefix-match route lookup.
//! Destination 91.198.174.192 is compared bit-by-bit against 4 routes, one route
//! per step; then all verdicts shown and the /24 wins.

use crate::frame::*;

const STEP_MS: u64 = 2600;
const N_STEPS: u64 = 5; // 4 routes + final summary (original cycles s.length+1 states)
pub const DURATION: u64 = STEP_MS * N_STEPS;

const DEST: [u8; 4] = [91, 198, 174, 192];
const DEST_TEXT: &str = "91.198.174.192";

struct Route {
    cidr: &'static str,
    octets: [u8; 4],
    prefix: usize,
}
const ROUTES: [Route; 4] = [
    Route { cidr: "91.198.174.0/24", octets: [91, 198, 174, 0], prefix: 24 },
    Route { cidr: "91.198.0.0/16", octets: [91, 198, 0, 0], prefix: 16 },
    Route { cidr: "10.0.0.0/8", octets: [10, 0, 0, 0], prefix: 8 },
    Route { cidr: "0.0.0.0/0", octets: [0, 0, 0, 0], prefix: 0 },
];

fn bits_of(octets: &[u8; 4]) -> Vec<u8> {
    octets.iter().flat_map(|o| (0..8).rev().map(move |i| (o >> i) & 1)).collect()
}

// first prefix-bit mismatch vs destination, or None (match)
fn first_mismatch(r: &Route) -> Option<usize> {
    let rb = bits_of(&r.octets);
    let db = bits_of(&DEST);
    (0..r.prefix).find(|&i| rb[i] != db[i])
}

fn bit_string(r: &Route, mark_mismatch: bool) -> String {
    let rb = bits_of(&r.octets);
    let mm = if mark_mismatch { first_mismatch(r) } else { None };
    let mut s = String::new();
    for i in 0..r.prefix {
        if i > 0 && i % 8 == 0 {
            s.push(' ');
        }
        s.push(if Some(i) == mm { '×' } else { char::from(b'0' + rb[i]) });
    }
    if r.prefix == 0 {
        s.push_str("0 bits fixed");
    }
    s
}

pub struct IpBits;

impl Sim for IpBits {
    fn duration(&self) -> u64 {
        DURATION
    }

    fn frame(&self, t: u64) -> Frame {
        let step = ((t % DURATION) / STEP_MS) as usize; // 0..4
        let done = step >= ROUTES.len();
        let winner = "91.198.174.0/24";

        let mut texts = vec![
            TextSpec { text: format!("destination address {DEST_TEXT}, 32 bits"), x: 2.0, y: 8.0, dim: true, left: true },
            TextSpec { text: {
                let b = bits_of(&DEST);
                b.chunks(8).map(|c| c.iter().map(|b| char::from(b'0' + b)).collect::<String>()).collect::<Vec<_>>().join(" ")
            }, x: 2.0, y: 14.0, dim: false, left: true },
            TextSpec {
                text: if done { "every route, checked against the address".into() } else { format!("checking route {} of {}", step + 1, ROUTES.len()) },
                x: 2.0,
                y: 22.0,
                dim: true,
                left: true,
            },
        ];

        for (i, r) in ROUTES.iter().enumerate() {
            let y = 30.0 + i as f64 * 13.0;
            let resolved = i < step || done;
            let checking = i == step && !done;
            let mm = first_mismatch(r);
            let is_winner = done && r.cidr == winner;
            let bits = if r.prefix == 0 {
                "0 bits fixed".to_string()
            } else {
                bit_string(r, resolved && mm.is_some())
            };
            let verdict = if is_winner {
                " ← used".to_string()
            } else if checking {
                ", checking…".into()
            } else if resolved {
                if r.prefix == 0 {
                    ", always matches".into()
                } else if let Some(m) = mm {
                    format!(", bit {} differs", m + 1)
                } else {
                    ", match".into()
                }
            } else {
                String::new()
            };
            texts.push(TextSpec {
                text: format!("{}  {}{}", r.cidr, bits, verdict),
                x: 2.0,
                y,
                dim: !(checking || is_winner),
                left: true,
            });
        }

        Frame {
            header: Some("INTERACTIVE · IP ROUTE LOOKUP".into()),
            nodes: vec![],
            packets: vec![],
            trails: vec![],
            texts,
            polylines: vec![],
            paths: vec![],
            badge: None,
            note: if done {
                let matches: Vec<&str> = ROUTES.iter().filter(|r| first_mismatch(r).is_none()).map(|r| r.cidr).collect();
                format!("{} routes match: {}. Longest prefix wins, {winner} is the most specific, so that's the line the packet takes.", matches.len(), matches.join(", "))
            } else {
                let r = &ROUTES[step];
                if r.prefix == 0 {
                    format!("{} fixes no bits at all, the catch-all default. It always matches, but it's the least specific route there is.", r.cidr)
                } else if first_mismatch(r).is_none() {
                    format!("{} fixes the first {} bits, and every one of them equals the address's first {} bits, a match.", r.cidr, r.prefix, r.prefix)
                } else {
                    let m = first_mismatch(r).unwrap();
                    format!("{} fixes the first {} bits, but bit {} differs from the address's, ruled out right there, no need to check the rest.", r.cidr, r.prefix, m + 1)
                }
            },
        }
    }
}
