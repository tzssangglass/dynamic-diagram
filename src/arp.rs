//! Faithful port of fazamhd.com's ArpSim: host needs a MAC for an IP, broadcasts
//! "who has?", everyone else stays silent, target unicasts the reply, sender caches
//! it. Episodes: host1→host3, again (cache hit), host2→gateway, again (hit).
//! Cache clears when the cycle restarts, so the loop is fully deterministic.

use crate::frame::*;
use std::sync::OnceLock;

const O: u64 = 350; // ms per flight leg (original: a/2, a=700)
const PAUSE: u64 = 1300; // pause after each episode (original: o)

// original: caption time = max(950, min(4200, chars*28)) ms
const fn cap(n: u64) -> u64 {
    if n < 950 { 950 } else if n > 4200 { 4200 } else { n }
}

struct Host {
    id: &'static str,
    ip: &'static str,
    mac: &'static str,
    x: f64,
    y: f64,
}

const HOSTS: [Host; 4] = [
    Host { id: "host 1", ip: "203.0.113.10", mac: "1A:2B:3C", x: 8.0, y: 22.0 },
    Host { id: "host 2", ip: "203.0.113.20", mac: "4D:5E:6F", x: 8.0, y: 78.0 },
    Host { id: "host 3", ip: "203.0.113.30", mac: "7A:8B:9C", x: 92.0, y: 22.0 },
    Host { id: "gateway", ip: "203.0.113.1", mac: "AA:BB:CC", x: 92.0, y: 78.0 },
];
const CENTER: f64 = 50.0;

const EPISODES: [(usize, usize); 4] = [(0, 2), (0, 2), (1, 3), (1, 3)];

pub struct Ep {
    pub start: u64,
    pub miss: bool,
    pub k: u64, // -> waiting stage
    pub a: u64, // -> reply stage
    pub j: u64, // -> caching stage
}

pub struct Schedule {
    pub eps: Vec<Ep>,
    pub duration: u64,
}

pub fn schedule() -> &'static Schedule {
    static S: OnceLock<Schedule> = OnceLock::new();
    S.get_or_init(|| {
        let mut eps = Vec::new();
        let mut t = 0u64;
        let mut learned: Vec<(usize, usize)> = Vec::new();
        for &(src, dst) in EPISODES.iter() {
            let (s, d) = (&HOSTS[src], &HOSTS[dst]);
            let miss = !learned.contains(&(src, dst));
            if miss {
                let b = format!("{} needs a MAC for {}. It broadcasts: \"Who has {}? Reply to {}.\"", s.id, d.ip, d.ip, s.ip);
                let others: Vec<&str> = HOSTS.iter().filter(|h| h.id != s.id && h.id != d.id).map(|h| h.id).collect();
                let st = format!(
                    "Every host on the segment hears the broadcast. {} check their own IP, it isn't {}, and stay silent. Only {} recognizes its own address.",
                    others.join(" and "), d.ip, d.id
                );
                let w = format!("{} replies directly to {}, unicast, not broadcast: \"{} is at {}.\"", d.id, s.id, d.ip, d.mac);
                let e = format!("{} caches {} → {} in its ARP table. Every frame to {} for the next few minutes skips this whole exchange.", s.id, d.ip, d.mac, d.ip);
                let k = cap(b.len() as u64 * 28).max(880);
                let a = k + cap(st.len() as u64 * 28);
                let j = a + cap(w.len() as u64 * 28).max(850);
                let end = j + cap(e.len() as u64 * 28) + PAUSE;
                eps.push(Ep { start: t, miss: true, k, a, j });
                learned.push((src, dst));
                t += end;
            } else {
                let text = format!("{} already has {} → {} in its ARP cache. No broadcast needed, it addresses the frame directly.", s.id, d.ip, d.mac);
                let end = cap(text.len() as u64 * 28).max(1900) + PAUSE;
                eps.push(Ep { start: t, miss: false, k: 0, a: 0, j: 0 });
                t += end;
            }
        }
        Schedule { eps, duration: t }
    })
}

// two-leg packet path: (fx,fy) -> (CENTER,fy) -> (CENTER,ty) -> (tx,ty)
// spawn at +30, leg1 +60..+410, snap, leg2 +440..+790; sits parked otherwise.
fn two_leg(dt: u64, fx: f64, fy: f64, tx: f64, ty: f64) -> Option<(f64, f64)> {
    if dt < 30 {
        return None;
    }
    if dt < 60 {
        return Some((fx, fy));
    }
    if dt < 410 {
        return Some((fx + (CENTER - fx) * (dt - 60) as f64 / O as f64, fy));
    }
    if dt < 440 {
        return Some((CENTER, ty));
    }
    if dt < 790 {
        return Some((CENTER + (tx - CENTER) * (dt - 440) as f64 / O as f64, ty));
    }
    Some((tx, ty))
}

pub struct Arp;

impl Sim for Arp {
    fn duration(&self) -> u64 {
        schedule().duration
    }

    fn frame(&self, t: u64) -> Frame {
        let sch = schedule();
        let tt = t % sch.duration;
        let (ei, ep) = sch.eps.iter().enumerate().rev().find(|(_, e)| tt >= e.start).unwrap();
        let (src, dst) = EPISODES[ei];
        let (s, d) = (&HOSTS[src], &HOSTS[dst]);
        let dt = tt - ep.start;

        let mut packets = Vec::new();
        let note;
        let mut learned_now: Option<String> = None; // "ip → mac" once cached

        if !ep.miss {
            note = format!("{} already has {} → {} in its ARP cache. No broadcast needed, it addresses the frame directly.", s.id, d.ip, d.mac);
            learned_now = Some(format!("{} → {}", d.ip, d.mac));
        } else {
            let bcast_label = format!("who has {}?", d.ip);
            let reply_label = format!("{} at {}", d.ip, d.mac);
            if dt < ep.k {
                note = format!("{} needs a MAC for {}. It broadcasts: \"Who has {}? Reply to {}.\"", s.id, d.ip, d.ip, s.ip);
            } else if dt < ep.a {
                let others: Vec<&str> = HOSTS.iter().filter(|h| h.id != s.id && h.id != d.id).map(|h| h.id).collect();
                note = format!("Every host on the segment hears the broadcast. {} check their own IP, it isn't {}, and stay silent. Only {} recognizes its own address.", others.join(" and "), d.ip, d.id);
            } else if dt < ep.j {
                note = format!("{} replies directly to {}, unicast, not broadcast: \"{} is at {}.\"", d.id, s.id, d.ip, d.mac);
            } else {
                note = format!("{} caches {} → {} in its ARP table. Every frame to {} for the next few minutes skips this whole exchange.", s.id, d.ip, d.mac, d.ip);
                learned_now = Some(format!("{} → {}", d.ip, d.mac));
            }
            // broadcast packets: source -> every other host, parked at target until reply stage
            if dt >= 30 && dt < ep.a {
                for h in HOSTS.iter().filter(|h| h.id != s.id) {
                    if let Some((x, y)) = two_leg(dt, s.x, s.y, h.x, h.y) {
                        packets.push(PacketSpec { label: bcast_label.clone(), x1: x, y1: y, x2: x, y2: y, p: 1.0, landed: false });
                    }
                }
            }
            // reply packet: target -> source, parked at source until episode end
            if dt >= ep.a {
                let rdt = dt - ep.a;
                if let Some((x, y)) = two_leg(rdt, d.x, d.y, s.x, s.y) {
                    packets.push(PacketSpec { label: reply_label, x1: x, y1: y, x2: x, y2: y, p: 1.0, landed: false });
                }
            }
        }

        let nodes = HOSTS
            .iter()
            .map(|h| NodeSpec {
                label: h.id.into(),
                x: h.x,
                y: h.y,
                status: Some(format!("{} · {}", h.ip, h.mac)),
                lifeline: false,
                icon: None,
                // source active all episode; target active once we're waiting
                // (encode as part of the status line? no — NodeSpec has no active
                // field on purpose; highlight is carried by the note)
            })
            .collect();
        // static links: each host to the segment center (horizontal, original as-line)
        let trails = HOSTS
            .iter()
            .map(|h| TrailSpec { x1: h.x, y1: h.y, x2: CENTER, y2: h.y, arrow_end: false })
            .collect();

        Frame {
            header: Some("INTERACTIVE · ARP ·· RESOLVING AN IP TO A MAC".into()),
            nodes,
            packets,
            trails,
            badge: Some(match learned_now {
                Some(e) => format!("{}'s ARP cache: {}", s.id, e),
                None => format!("{}'s ARP cache: (empty)", s.id),
            }),
            texts: vec![],
            polylines: vec![],
            paths: vec![],
            note,
        }
    }
}
