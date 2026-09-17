//! Faithful port of fazamhd.com's TcpHandshakeSim — timing, captions, and
//! geometry taken from the original component (TcpHandshakeSim.*.js) and CSS.
//! Differences: runs immediately (no scroll-into-view trigger), no pause/reset
//! buttons (needs input), no reduced-motion shortcut.

use crate::frame::*;

const FLY: u64 = 1800; // ms per segment, linear (original: transition 1800ms linear)
const LOOP_PAUSE: u64 = 9000; // established beat before restarting (original: 9e3)
const CLIENT: f64 = 20.0;
const SERVER: f64 = 80.0; // node x positions (original: 20% / 80%)
const LANES: [f64; 3] = [38.0, 56.0, 74.0]; // message lanes y (original: 5/7/9rem of 12rem stage)

// original: caption display time = max(2100, min(5200, chars*28)) ms
const fn cap_dur(s: &str) -> u64 {
    let n = s.len() as u64 * 28;
    if n < 2100 {
        2100
    } else if n > 5200 {
        5200
    } else {
        n
    }
}

// verbatim captions
const CAP_SYN: &str = "Client picks a random starting sequence number and sends SYN, seq=5000: \"I'll start counting my bytes from 5000.\"";
const CAP_SYNACK: &str = "Server picks its own starting number and replies SYN-ACK, seq=9000, ack=5001: \"I'll start from 9000; I've received your byte 5000.\"";
const CAP_ACK: &str = "Client replies ACK, ack=9001: \"Received your byte 9000.\" Three segments, no payload in any of them.";
const CAP_EST: &str = "Both sides now agree on where the byte count starts. This is what lets TCP say precisely which bytes are missing when one goes astray.";

// stage boundaries (ms)
pub const T_SYN: u64 = 0;
pub const T_SYNACK: u64 = cap_dur(CAP_SYN);
pub const T_ACK: u64 = T_SYNACK + cap_dur(CAP_SYNACK);
pub const T_EST: u64 = T_ACK + cap_dur(CAP_ACK);
pub const DURATION: u64 = T_EST + LOOP_PAUSE;

struct Msg {
    label: &'static str,
    right: bool,
    lane: usize,
    start: u64,
}

const MSGS: [Msg; 3] = [
    Msg { label: "SYN seq=5000", right: true, lane: 0, start: T_SYN },
    Msg { label: "SYN-ACK seq=9000 ack=5001", right: false, lane: 1, start: T_SYNACK },
    Msg { label: "ACK ack=9001", right: true, lane: 2, start: T_ACK },
];

pub struct TcpHandshake;

impl Sim for TcpHandshake {
    fn duration(&self) -> u64 {
        DURATION
    }

    fn frame(&self, t: u64) -> Frame {
        let tt = t % DURATION;
        let mut packets = Vec::new();
        let mut trails = Vec::new();
        for m in MSGS.iter() {
            if tt < m.start {
                break;
            }
            let (from, to) = if m.right { (CLIENT, SERVER) } else { (SERVER, CLIENT) };
            let y = LANES[m.lane];
            let p = ((tt - m.start) as f64 / FLY as f64).min(1.0);
            let landed = p >= 1.0;
            packets.push(PacketSpec {
                label: m.label.into(),
                x1: from,
                y1: y,
                x2: to,
                y2: y,
                p,
                landed,
            });
            if landed {
                // trail across the lane
                trails.push(TrailSpec { x1: CLIENT, y1: y, x2: SERVER, y2: y, arrow_end: false });
            }
        }
        let established = tt >= T_EST;
        Frame {
            header: Some("INTERACTIVE · TCP ·· THE THREE-WAY HANDSHAKE".into()),
            nodes: vec![
                NodeSpec {
                    label: "CLIENT".into(),
                    x: CLIENT,
                    y: 8.0,
                    lifeline: true,
                    icon: None,
                    status: Some("seq 5000".into()), // original sets client seq at stage start
                },
                NodeSpec {
                    label: "SERVER".into(),
                    x: SERVER,
                    y: 8.0,
                    lifeline: true,
                    icon: None,
                    status: Some(
                        if tt >= T_SYNACK { "seq 9000" } else { "sequence pending" }.into(),
                    ),
                },
            ],
            packets,
            trails,
            texts: vec![],
            polylines: vec![],
            paths: vec![],
            badge: if established { Some("established".into()) } else { None },
            note: if tt >= T_EST {
                CAP_EST
            } else if tt >= T_ACK {
                CAP_ACK
            } else if tt >= T_SYNACK {
                CAP_SYNACK
            } else {
                CAP_SYN
            }
            .into(),
        }
    }
}
