//! Faithful port of fazamhd.com's TcpVsUdpSim: 5 segments/datagrams, #3 drops.
//! TCP notices (no ACK), resends, receiver flushes the buffer in order — complete
//! but late. UDP never notices — on time with a permanent gap.

use crate::frame::*;

const N: u64 = 5;
const INTERVAL: u64 = 650; // send spacing (original: c)
const FLY: u64 = 1600; // full flight (original: s)
const SENDER: f64 = 8.0;
const RECV: f64 = 92.0;
const RESEND_AT: u64 = 4300;
pub const DURATION: u64 = 8600;

const LOST_SEQ: u64 = 3;

const TCP_Y: f64 = 25.0;
const UDP_Y: f64 = 65.0;

// packet x at time t for a send at `sent`; lost packets die at the midpoint
fn pkt_x(t: u64, sent: u64, lost: bool) -> Option<f64> {
    if t < sent {
        return None;
    }
    let dt = t - sent;
    if !lost {
        if dt > FLY {
            return None; // arrived, removed
        }
        return Some(SENDER + (RECV - SENDER) * dt as f64 / FLY as f64);
    }
    // lost: half flight (800ms), fade 700ms, gone
    if dt > 1500 {
        return None;
    }
    let p = (dt as f64 / 800.0).min(1.0);
    Some(SENDER + (50.0 - SENDER) * p)
}

pub struct TcpVsUdp;

impl Sim for TcpVsUdp {
    fn duration(&self) -> u64 {
        DURATION
    }

    fn frame(&self, t: u64) -> Frame {
        let tt = t % DURATION;
        let mut packets = vec![];
        for i in 1..=N {
            let sent = (i - 1) * INTERVAL;
            if let Some(x) = pkt_x(tt, sent, i == LOST_SEQ) {
                packets.push(PacketSpec { label: i.to_string(), x1: x, y1: TCP_Y, x2: x, y2: TCP_Y, p: 1.0, landed: false });
                if let Some(xu) = pkt_x(tt, sent, i == LOST_SEQ) {
                    packets.push(PacketSpec { label: i.to_string(), x1: xu, y1: UDP_Y, x2: xu, y2: UDP_Y, p: 1.0, landed: false });
                }
            }
        }
        // TCP resend of 3
        if let Some(x) = pkt_x(tt, RESEND_AT, false) {
            packets.push(PacketSpec { label: "3".into(), x1: x, y1: TCP_Y, x2: x, y2: TCP_Y, p: 1.0, landed: false });
        }

        // slot states
        let resent_arrives = RESEND_AT + FLY; // 5900
        let tcp_slot = |i: u64| -> &'static str {
            let arr = (i - 1) * INTERVAL + FLY;
            if i <= 2 {
                if tt >= arr { "delivered" } else { "" }
            } else if i == LOST_SEQ {
                if tt >= resent_arrives { "delivered" } else { "" }
            } else if tt >= resent_arrives {
                "delivered"
            } else if tt >= arr {
                "buffered"
            } else {
                ""
            }
        };
        let udp_slot = |i: u64| -> &'static str {
            if i == LOST_SEQ {
                return if tt >= (N - 1) * INTERVAL + FLY + 50 { "missing" } else { "" };
            }
            if tt >= (i - 1) * INTERVAL + FLY { "delivered" } else { "" }
        };
        let slot_row = |f: &dyn Fn(u64) -> &'static str| -> String {
            (1..=N)
                .map(|i| match f(i) {
                    "delivered" => format!("{i}✓"),
                    "buffered" => format!("{i}◦"),
                    "missing" => "×".to_string(),
                    _ => i.to_string(),
                })
                .collect::<Vec<_>>()
                .join("  ")
        };

        // lane status lines (verbatim captions)
        let tcp_note = if tt >= 8476 {
            "Complete, but late, every segment arrived, in order, at the cost of that one round trip."
        } else if tt >= resent_arrives {
            "The resent segment 3 arrives, the receiver flushes the buffer and delivers 3, 4, 5 in order."
        } else if tt >= RESEND_AT {
            "No ACK for segment 3 after enough time, TCP assumes it was lost and resends it."
        } else if tt >= 4 * INTERVAL + FLY {
            "Segment 5 arrives but waits, it's ahead of the gap at 3, so it's held instead of delivered."
        } else if tt >= 3 * INTERVAL + FLY {
            "Segment 4 arrives but waits, it's ahead of the gap at 3, so it's held instead of delivered."
        } else {
            "Sending 5 segments; 3 is about to drop."
        };
        let udp_note = if tt >= (N - 1) * INTERVAL + FLY {
            "On time, but missing a piece, the stream moved on and left a permanent gap at 3."
        } else if tt >= 2 * INTERVAL + 800 {
            "Datagram 3 is lost. UDP has no mechanism to notice, let alone resend it."
        } else {
            "Sending 5 datagrams; 3 is about to drop."
        };

        Frame {
            header: Some("INTERACTIVE · TCP VS UDP".into()),
            nodes: vec![
                NodeSpec { label: "sender".into(), x: SENDER, y: TCP_Y, status: None, lifeline: false, icon: None },
                NodeSpec { label: "receiver".into(), x: RECV, y: TCP_Y, status: None, lifeline: false, icon: None },
                NodeSpec { label: "sender".into(), x: SENDER, y: UDP_Y, status: None, lifeline: false, icon: None },
                NodeSpec { label: "receiver".into(), x: RECV, y: UDP_Y, status: None, lifeline: false, icon: None },
            ],
            packets,
            trails: vec![
                TrailSpec { x1: SENDER, y1: TCP_Y, x2: RECV, y2: TCP_Y, arrow_end: false },
                TrailSpec { x1: SENDER, y1: UDP_Y, x2: RECV, y2: UDP_Y, arrow_end: false },
            ],
            texts: vec![
                TextSpec { text: "TCP · reliable, ordered".into(), x: 2.0, y: 12.0, dim: true, left: true },
                TextSpec { text: slot_row(&tcp_slot), x: 30.0, y: 38.0, dim: false, left: true },
                TextSpec { text: "UDP · best-effort, no resend".into(), x: 2.0, y: 52.0, dim: true, left: true },
                TextSpec { text: slot_row(&udp_slot), x: 30.0, y: 78.0, dim: false, left: true },
            ],
            polylines: vec![],
            paths: vec![],
            badge: Some(format!("UDP: {udp_note}")),
            note: format!("TCP: {tcp_note}"),
        }
    }
}
