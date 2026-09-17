//! Faithful port of fazamhd.com's TcpSim: sliding window of 4 over 8 packets,
//! packet 3 lost mid-flight. Sender keeps going, receiver buffers 4-6 behind the
//! gap, three duplicate ACKs trigger the resend, everything delivers in order.
//! The original picks the lost packet randomly; we fix it at 3 (the original's
//! own reduced-motion preview choice), making the whole timeline deterministic.

use crate::frame::*;
use std::sync::OnceLock;

const N: u64 = 8;
const WINDOW: u64 = 4;
const FLY: u64 = 2200;
const ACK_FLY: u64 = 1800;
const SENDER: f64 = 14.0;
const RECV: f64 = 86.0;

const fn cap(n: u64) -> u64 {
    if n < 1100 { 1100 } else if n > 3400 { 3400 } else { n }
}

const NOTE_NEW: &str = "New message: the sender fills the window with packets 1–4 without waiting for each to be confirmed.";
const NOTE_LOST: &str = "Packet 3 is lost in the network, but the sender doesn't know yet, and keeps the window full.";
const NOTE_ACK1: &str = "ACK 1 confirms everything through 1, the window slides forward and the next packet goes out.";
const NOTE_ACK2: &str = "ACK 2 confirms everything through 2, the window slides forward and the next packet goes out.";
const NOTE_BUF4: &str = "Packet 4 arrived but can't be delivered yet, it's buffered behind the gap.";
const NOTE_BUF5: &str = "Packet 5 arrived but can't be delivered yet, it's buffered behind the gap.";
const NOTE_BUF6: &str = "Packet 6 arrived but can't be delivered yet, it's buffered behind the gap.";
const NOTE_DUP: &str = "3 duplicate ACKs for 2, the sender realises packet 3 was lost and resends it.";
const NOTE_GAP: &str = "The missing packet arrives, the receiver fills the gap and delivers 3–6 in order.";
const NOTE_ACK6: &str = "ACK 6 confirms everything through 6, the window slides forward and the next packet goes out.";
const NOTE_ACK7: &str = "ACK 7 confirms everything through 7, the window slides forward and the next packet goes out.";
const NOTE_DONE: &str = "Whole message delivered in order, despite a loss along the way, that's TCP on top of best-effort IP.";

// the deterministic event trace (lost = packet 3, resent after 3 dup ACKs)
struct Trace {
    sends: Vec<(u64, u64, bool)>, // (t, seq, lost)
    acks: Vec<(u64, u64)>,        // (t, n) ack departs
    o_at: Vec<(u64, u64)>,        // (t, window base)
    j_at: Vec<(u64, u64)>,        // (t, delivered pointer)
    notes: Vec<(u64, &'static str)>,
    pub duration: u64,
}

fn trace() -> &'static Trace {
    static T: OnceLock<Trace> = OnceLock::new();
    T.get_or_init(|| {
        let sends = vec![
            (0, 1, false),
            (700, 2, false),
            (1400, 3, true), // lost mid-flight
            (2100, 4, false),
            (4000, 5, false),
            (4700, 6, false),
            (8700, 3, false), // resend after 3 dup ACKs
            (12700, 7, false),
            (16700, 8, false),
        ];
        let acks = vec![
            (2200, 1),
            (2900, 2),
            (4300, 2),
            (6200, 2),
            (6900, 2),
            (10900, 6),
            (14900, 7),
            (18900, 8),
        ];
        let o_at = vec![(0, 1), (4000, 2), (4700, 3), (12700, 7), (16700, 8), (20700, 9)];
        let j_at = vec![(0, 0), (2200, 1), (2900, 2), (10900, 6), (14900, 7), (18900, 8)];
        let notes = vec![
            (0, NOTE_NEW),
            (2500, NOTE_LOST),
            (4000, NOTE_ACK1),
            (4300, NOTE_BUF4),
            (4700, NOTE_ACK2),
            (6200, NOTE_BUF5),
            (6900, NOTE_BUF6),
            (8700, NOTE_DUP),
            (10900, NOTE_GAP),
            (12700, NOTE_ACK6),
            (16700, NOTE_ACK7),
            (20700, NOTE_DONE),
        ];
        let duration = 20700 + cap(NOTE_DONE.len() as u64 * 28).max(1700);
        Trace { sends, acks, o_at, j_at, notes, duration }
    })
}

pub struct TcpSim;

impl Sim for TcpSim {
    fn duration(&self) -> u64 {
        trace().duration
    }

    fn frame(&self, t: u64) -> Frame {
        let tr = trace();
        let tt = t % tr.duration;

        let state_at = |table: &[(u64, u64)]| table.iter().rev().find(|(at, _)| tt >= *at).map(|(_, v)| *v).unwrap_or(0);
        let o = state_at(&tr.o_at);
        let j = state_at(&tr.j_at);
        let mut note = tr.notes[0].1;
        for (at, s) in &tr.notes {
            if tt >= *at {
                note = s;
            }
        }

        // in-flight packets (lost one flies half-way then fades)
        let mut packets = vec![];
        for (st, seq, lost) in &tr.sends {
            if tt < *st {
                continue;
            }
            let dt = tt - st;
            let (dur, visible) = if *lost { (FLY / 2, dt < FLY / 2 + 360) } else { (FLY, dt < FLY) };
            if visible {
                let p = (dt as f64 / dur as f64).min(1.0);
                packets.push(PacketSpec {
                    label: seq.to_string(),
                    x1: SENDER,
                    y1: 45.0,
                    x2: RECV,
                    y2: 45.0,
                    p,
                    landed: false,
                });
            }
        }
        // in-flight ACKs
        for (st, n) in &tr.acks {
            if tt >= *st && tt < st + ACK_FLY {
                let p = (tt - st) as f64 / ACK_FLY as f64;
                packets.push(PacketSpec {
                    label: format!("ack {n}"),
                    x1: RECV,
                    y1: 60.0,
                    x2: SENDER,
                    y2: 60.0,
                    p,
                    landed: false,
                });
            }
        }

        // sender window cells: acked / window / inflight / lost
        let inflight = |seq: u64| -> bool {
            tr.sends.iter().any(|(st, s, lost)| {
                *s == seq && tt >= *st && tt < st + if *lost { FLY / 2 + 360 } else { FLY }
            })
        };
        let lost_now = |seq: u64| -> bool {
            // lost at midpoint until the resend departs
            seq == 3 && tt >= 2500 && tt < 8700
        };
        let window_cells: Vec<String> = (1..=N)
            .map(|i| {
                if i < o {
                    format!("{i}✓")
                } else if i < o + WINDOW && i <= N {
                    if lost_now(i) {
                        format!("{i}✕")
                    } else if inflight(i) {
                        format!("{i}→")
                    } else {
                        format!("{i}□")
                    }
                } else {
                    format!("{i}·")
                }
            })
            .collect();
        // receiver slots: delivered / buffered
        let arrived = |seq: u64| -> bool {
            // arrival = send + FLY (or resend + FLY for 3)
            match seq {
                1 => tt >= 2200,
                2 => tt >= 2900,
                3 => tt >= 10900,
                4 => tt >= 4300,
                5 => tt >= 6200,
                6 => tt >= 6900,
                7 => tt >= 14900,
                _ => tt >= 18900,
            }
        };
        let slots: Vec<String> = (1..=N)
            .map(|i| {
                if i <= j {
                    format!("{i}✓")
                } else if arrived(i) {
                    format!("{i}◦")
                } else {
                    i.to_string()
                }
            })
            .collect();

        Frame {
            header: Some("INTERACTIVE · TCP ·· RELIABLE DELIVERY".into()),
            nodes: vec![
                NodeSpec { label: "sender".into(), x: SENDER, y: 45.0, status: None, lifeline: false, icon: None },
                NodeSpec { label: "receiver".into(), x: RECV, y: 45.0, status: None, lifeline: false, icon: None },
            ],
            packets,
            trails: vec![
                TrailSpec { x1: SENDER, y1: 45.0, x2: RECV, y2: 45.0, arrow_end: false },
                TrailSpec { x1: SENDER, y1: 60.0, x2: RECV, y2: 60.0, arrow_end: false },
            ],
            texts: vec![
                TextSpec { text: "sender window (4 in flight, sliding):".into(), x: 2.0, y: 8.0, dim: true, left: true },
                TextSpec { text: window_cells.join(" "), x: 2.0, y: 15.0, dim: false, left: true },
                TextSpec { text: "receiver slots (✓ delivered, ◦ buffered):".into(), x: 2.0, y: 78.0, dim: true, left: true },
                TextSpec { text: slots.join(" "), x: 2.0, y: 85.0, dim: false, left: true },
            ],
            polylines: vec![],
            paths: vec![],
            badge: None,
            note: note.into(),
        }
    }
}
