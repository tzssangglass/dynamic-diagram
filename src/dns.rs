//! Faithful port of fazamhd.com's DnsResolverSim: cold lookup walks root → .org
//! → wikipedia ns with referrals back; answer cached at resolver and device;
//! second lookup served from device cache; third from resolver cache for another
//! device; TTL expires and the chain must be walked again (loops).

use crate::frame::*;
use std::sync::OnceLock;

const HOP_MS: u64 = 450; // per-hop flight (original C(): all hops clamp to 450)
const INTER_HOP: u64 = 120;
const POST: u64 = 650;
const PHASE_HOLD: u64 = 1500;
const TTL_TICK_MS: u64 = 500; // compressed (original overlaps 12x1500ms with the story)
const TTL_STEP: u32 = 300;
const END_PAUSE: u64 = 4200;

const NAME: &str = "en.wikipedia.org";
const ADDR: &str = "91.198.174.192";
const TTL_START: u32 = 3600;

#[derive(Clone, Copy)]
struct N {
    x: f64,
    y: f64,
    label: &'static str,
}
const NODES: [(&str, N); 10] = [
    ("device2", N { x: 15.0, y: 28.0, label: "another device" }),
    ("device", N { x: 15.0, y: 62.0, label: "your device" }),
    ("ra", N { x: 27.0, y: 43.0, label: "" }),
    ("resolver", N { x: 38.0, y: 22.0, label: "resolver" }),
    ("rb", N { x: 58.0, y: 26.0, label: "" }),
    ("rc", N { x: 66.0, y: 64.0, label: "" }),
    ("root", N { x: 78.0, y: 18.0, label: "root ns" }),
    ("org", N { x: 84.0, y: 40.0, label: ".org ns" }),
    ("wiki", N { x: 80.0, y: 80.0, label: "wikipedia ns" }),
    ("web", N { x: 48.0, y: 82.0, label: "web server" }),
];
fn node(id: &str) -> N {
    NODES.iter().find(|(k, _)| *k == id).unwrap().1
}

const LINKS: [(&str, &str); 10] = [
    ("device", "ra"), ("device2", "ra"), ("ra", "resolver"), ("ra", "rc"),
    ("resolver", "rb"), ("rb", "rc"), ("rb", "root"), ("rc", "org"),
    ("rc", "wiki"), ("rc", "web"),
];

enum Step {
    Phase(&'static str),
    Hit(&'static str),
    Flight {
        path: &'static [&'static str],
        label: &'static str,
        fills: Option<&'static str>,
        status: &'static str,
    },
}

const T_DEV: &[&str] = &["device", "ra", "resolver"];
const T_ROOT: &[&str] = &["resolver", "rb", "root"];
const T_ORG: &[&str] = &["resolver", "rb", "rc", "org"];
const T_WIKI: &[&str] = &["resolver", "rb", "rc", "wiki"];
const T_WEB: &[&str] = &["device", "ra", "rc", "web"];
const T_DEV2: &[&str] = &["device2", "ra", "resolver"];
const T_BACK_DEV2: &[&str] = &["resolver", "ra", "device2"];

const STEPS: &[Step] = &[
    Step::Phase("lookup 1, nothing cached"),
    Step::Flight { path: T_DEV, label: NAME, fills: None, status: "No cache holds the name yet, so your device hands the question to its recursive resolver." },
    Step::Flight { path: T_ROOT, label: NAME, fills: None, status: "The root doesn't know the name. It knows exactly one thing, who runs .org, a referral, one step down the chain." },
    Step::Flight { path: T_ROOT, label: ".org referral", fills: None, status: "" },
    Step::Flight { path: T_ORG, label: NAME, fills: None, status: ".org doesn't know it either, but it knows who runs wikipedia.org." },
    Step::Flight { path: T_ORG, label: "wikipedia.org referral", fills: None, status: "" },
    Step::Flight { path: T_WIKI, label: NAME, fills: None, status: "Wikipedia's own name server holds the actual record and answers, 91.198.174.192, cacheable for 3600 seconds." },
    Step::Flight { path: T_WIKI, label: "answer, ttl 3600", fills: Some("resolver"), status: "" },
    Step::Flight { path: T_DEV, label: ADDR, fills: Some("device"), status: "The resolver keeps a copy, and so does your device's operating system. Three round trips, not to be repeated while the TTL lasts." },
    Step::Flight { path: T_WEB, label: "TCP:443", fills: None, status: "DNS is done, name turned into number. The browser opens a TCP connection to the address it was given." },
    Step::Phase("lookup 2, answered on the device"),
    Step::Hit("The second lookup never leaves your machine, the OS answers from its own cache. No DNS packet on the wire at all."),
    Step::Flight { path: T_WEB, label: "TCP:443", fills: None, status: "" },
    Step::Phase("lookup 3, another device asks"),
    Step::Flight { path: T_DEV2, label: NAME, fills: None, status: "Another device behind the same resolver asks for the first time. Its own cache is empty but the resolver's isn't." },
    Step::Flight { path: T_BACK_DEV2, label: "cached answer", fills: None, status: "Root, .org, and Wikipedia's name server never hear about it. One walk of the chain, shared by everyone behind this resolver." },
];

const CAP_TTL0: &str = "TTL zero: device and resolver discard the record. The next lookup must walk the chain again, watch it repeat.";

struct Beat {
    t: u64,
    flight: Option<(&'static [&'static str], usize, u64, &'static str)>,
    resolver_cached: bool,
    device_cached: bool,
    ttl: Option<u32>, // Some = countdown phase
    phase: &'static str,
    note: String,
}

fn beats() -> &'static [Beat] {
    static B: OnceLock<Vec<Beat>> = OnceLock::new();
    B.get_or_init(|| {
        let mut v: Vec<Beat> = Vec::new();
        let mut t = 0u64;
        let mut note = "One lookup walks the chain of delegations; every lookup after it, from this device or any other behind the same resolver, is answered from a cache, until the TTL runs out.".to_string();
        let mut phase = "";
        let (mut rc, mut dc) = (false, false);
        let beat = |t: u64, flight, rc: bool, dc: bool, ttl: Option<u32>, phase: &'static str, note: &str, v: &mut Vec<Beat>| {
            v.push(Beat { t, flight, resolver_cached: rc, device_cached: dc, ttl, phase, note: note.to_string() });
        };
        for step in STEPS {
            match step {
                Step::Phase(p) => {
                    phase = p;
                    beat(t, None, rc, dc, None, phase, &note, &mut v);
                    t += PHASE_HOLD;
                }
                Step::Hit(status) => {
                    note = status.to_string();
                    beat(t, None, rc, dc, None, phase, &note, &mut v);
                    t += status.len() as u64 * 24;
                }
                Step::Flight { path, label, fills, status } => {
                    if !status.is_empty() {
                        note = status.to_string();
                    }
                    let mut depart = t;
                    for h in 0..path.len() - 1 {
                        beat(depart, Some((path, h, depart, label)), rc, dc, None, phase, &note, &mut v);
                        depart += 40 + HOP_MS + INTER_HOP;
                    }
                    if let Some(f) = fills {
                        if *f == "resolver" {
                            rc = true;
                        } else {
                            dc = true;
                        }
                    }
                    beat(depart, None, rc, dc, None, phase, &note, &mut v);
                    t = depart + POST;
                }
            }
        }
        // TTL countdown at the end (serialized; original overlaps it with the story)
        let mut ttl = TTL_START;
        while ttl > 0 {
            beat(t, None, rc, dc, Some(ttl), "ttl", &format!("TTL {ttl}: caches hold the record for {NAME}."), &mut v);
            t += TTL_TICK_MS;
            ttl = ttl.saturating_sub(TTL_STEP);
        }
        beat(t, None, rc, dc, Some(0), "ttl", CAP_TTL0, &mut v);
        t += TTL_TICK_MS + END_PAUSE;
        // stash duration in the last beat's t (unused field pattern avoided: return via global)
        v.push(Beat { t, flight: None, resolver_cached: false, device_cached: false, ttl: None, phase: "end", note: String::new() });
        v
    })
}

pub fn duration() -> u64 {
    beats().last().unwrap().t
}

pub struct Dns;

impl Sim for Dns {
    fn duration(&self) -> u64 {
        duration()
    }

    fn frame(&self, t: u64) -> Frame {
        let bs = beats();
        let tt = t % duration();
        // never resolve to the sentinel end beat
        let cur = bs
            .iter()
            .rev()
            .find(|b| tt >= b.t && b.phase != "end")
            .unwrap_or(&bs[0]);

        let mut packets = vec![];
        if let Some((path, hop, depart, label)) = cur.flight {
            let (from, to) = (node(path[hop]), node(path[hop + 1]));
            let p = ((tt.saturating_sub(depart + 40)) as f64 / HOP_MS as f64).min(1.0);
            if p < 1.0 {
                packets.push(PacketSpec {
                    label: label.to_string(),
                    x1: from.x,
                    y1: from.y,
                    x2: to.x,
                    y2: to.y,
                    p,
                    landed: false,
                });
            }
        }

        let cache_txt = |on: bool| -> Option<String> {
            on.then(|| format!("cache: {NAME} → {ADDR}"))
        };
        let nodes = NODES
            .iter()
            .map(|(id, n)| {
                let status = match *id {
                    "resolver" => cache_txt(cur.resolver_cached),
                    "device" => cache_txt(cur.device_cached),
                    _ => None,
                };
                NodeSpec { label: n.label.into(), x: n.x, y: n.y, status, lifeline: false, icon: None }
            })
            .collect();

        let phase_text = if cur.phase == "ttl" {
            format!("ttl countdown: {}", cur.ttl.unwrap_or(0))
        } else {
            cur.phase.to_string()
        };

        Frame {
            header: Some("INTERACTIVE · DNS RESOLVER".into()),
            nodes,
            packets,
            trails: LINKS
                .iter()
                .map(|(a, b)| {
                    let (na, nb) = (node(a), node(b));
                    TrailSpec { x1: na.x, y1: na.y, x2: nb.x, y2: nb.y, arrow_end: false }
                })
                .collect(),
            texts: vec![TextSpec { text: phase_text, x: 2.0, y: 4.0, dim: true, left: true }],
            polylines: vec![],
            paths: vec![],
            badge: None,
            note: cur.note.clone(),
        }
    }
}
