//! Faithful port of fazamhd.com's QuicSim: three chapters, TCP lane vs QUIC lane.
//! 1) connection setup: TCP needs 2 RTT before first HTTP byte, QUIC 1.
//! 2) one lost packet: TCP head-of-line blocks every stream, QUIC only stream B.
//! 3) switching networks: the address change kills TCP, QUIC migrates by
//! connection ID and the download never breaks.

use crate::frame::*;

const FLY: u64 = 2400;
const SEND_GAP: u64 = 950;
const CLIENT: f64 = 9.0;
const SERVER: f64 = 91.0;
const TCP_Y: f64 = 28.0;
const QUIC_Y: f64 = 62.0;

// chapter boundaries (original: V 0..17600, q 0..12350, Y 0..13000)
const CH1: u64 = 0;
const CH2: u64 = 17600;
const CH3: u64 = 17600 + 12350;
pub const DURATION: u64 = CH3 + 13000;

const CAP_T_SETUP: &str = "TCP first: nothing moves until its handshake finishes, then TLS runs its own on top.";
const CAP_Q_SETUP: &str = "QUIC folds transport setup and encryption into a single handshake.";
const CAP_T_1RTT: &str = "One round trip spent, and TLS has not even started yet.";
const CAP_T_2RTT: &str = "Two full round trips of pure latency before the first HTTP byte moved.";
const CAP_Q_1RTT: &str = "One round trip, or zero on a repeat visit: the request rides the first packet.";
const CAP_T_MULTI: &str = "Three requests multiplexed over one TCP connection, a single ordered byte stream. B1 is about to drop.";
const CAP_Q_MULTI: &str = "The same three requests, each on its own independently acknowledged QUIC stream. B1 is about to drop.";
const CAP_T_C1: &str = "C1 arrives fine but waits, it belongs to a different request, yet it sits behind the gap in the one shared stream.";
const CAP_T_B1: &str = "B1 finally arrives, only now can C1, A2, B2, C2 be handed over. One loss stalled every request.";
const CAP_Q_C2: &str = "The loss stalls only stream B, the other requests deliver on schedule.";
const CAP_Q_B1: &str = "B1 arrives and stream B catches up. A and C never noticed the loss.";
const CAP_T_DL: &str = "A download in progress. TCP keys the connection to the IP and port tuple.";
const CAP_Q_DL: &str = "The same download. QUIC names the conversation with a connection ID, not the address.";
const CAP_T_LEAVE: &str = "The phone leaves Wi-Fi. New IP, so a new tuple, to TCP, this is a different connection.";
const CAP_Q_LEAVE: &str = "The phone leaves Wi-Fi and gets a new IP. The connection ID has not changed.";
const CAP_T_DEAD: &str = "The old tuple no longer exists, so the download is dead, TCP must redo both handshakes from act 1.";
const CAP_Q_MIGR: &str = "Same connection ID, new address, the download never breaks.";

struct Flight {
    t: u64,
    label: &'static str,
    rtl: bool,
    lost: bool,
    lane_tcp: bool, // true = TCP lane, false = QUIC
}

fn flights() -> Vec<Flight> {
    let mut v = Vec::new();
    let f = |t: u64, label: &'static str, rtl: bool, lost: bool, tcp: bool, v: &mut Vec<Flight>| {
        v.push(Flight { t, label, rtl, lost, lane_tcp: tcp });
    };
    // chapter 1
    for (t, label, rtl, tcp) in [
        (0u64, "SYN", false, true),
        (2400, "SYN-ACK", true, true),
        (4800, "ACK + TLS hello", false, true),
        (7200, "TLS finished", true, true),
        (9600, "HTTP request", false, true),
        (0, "QUIC hello", false, false),
        (2400, "QUIC reply", true, false),
        (4800, "HTTP request", false, false),
    ] {
        f(CH1 + t, label, rtl, false, tcp, &mut v);
    }
    // chapter 2: A1..C2 both lanes, B1 lost; resend at 6750
    const NAMES: [&str; 6] = ["A1", "B1", "C1", "A2", "B2", "C2"];
    for (i, name) in NAMES.iter().enumerate() {
        f(CH2 + i as u64 * SEND_GAP, Box::leak(name.to_string().into_boxed_str()), false, *name == "B1", true, &mut v);
        f(CH2 + i as u64 * SEND_GAP, Box::leak(name.to_string().into_boxed_str()), false, *name == "B1", false, &mut v);
    }
    f(CH2 + 6750, "B1", false, false, true, &mut v);
    f(CH2 + 6750, "B1", false, false, false, &mut v);
    // chapter 3: server->client 1,2,3 both; TCP 4 lost; QUIC 4,5,6
    for i in 0..3u64 {
        f(CH3 + i * SEND_GAP, Box::leak((i + 1).to_string().into_boxed_str()), true, false, true, &mut v);
        f(CH3 + i * SEND_GAP, Box::leak((i + 1).to_string().into_boxed_str()), true, false, false, &mut v);
    }
    f(CH3 + 5500, "4", true, true, true, &mut v);
    for i in 0..3u64 {
        f(CH3 + 5500 + i * SEND_GAP, Box::leak((i + 4).to_string().into_boxed_str()), true, false, false, &mut v);
    }
    v.sort_by_key(|x| x.t);
    v
}

fn pkt_label_pos(t: u64, fl: &Flight) -> Option<f64> {
    let dt = t.saturating_sub(fl.t);
    let (dur, visible) = if fl.lost { (FLY / 2, dt < FLY / 2 + 700) } else { (FLY, dt < FLY) };
    if !visible {
        return None;
    }
    let p = (dt as f64 / dur as f64).min(1.0);
    Some(if fl.rtl { 100.0 - p * 100.0 } else { p * 100.0 })
}

pub struct Quic;

impl Sim for Quic {
    fn duration(&self) -> u64 {
        DURATION
    }

    fn frame(&self, t: u64) -> Frame {
        let tt = t % DURATION;
        let fs = flights();
        let chapter = if tt >= CH3 { 3 } else if tt >= CH2 { 2 } else { 1 };
        let ct = tt - if chapter == 3 { CH3 } else if chapter == 2 { CH2 } else { CH1 };

        let mut packets = vec![];
        for fl in &fs {
            if let Some(x) = pkt_label_pos(tt, fl) {
                let y = if fl.lane_tcp { TCP_Y } else { QUIC_Y };
                packets.push(PacketSpec { label: fl.label.into(), x1: x, y1: y, x2: x, y2: y, p: 1.0, landed: false });
            }
        }

        let (tcp_note, quic_note, extra) = match chapter {
            1 => {
                let tcp = if ct >= 12000 {
                    CAP_T_2RTT
                } else if ct >= 4800 {
                    CAP_T_1RTT
                } else {
                    CAP_T_SETUP
                };
                let quic = if ct >= 7200 { CAP_Q_1RTT } else { CAP_Q_SETUP };
                let tcp_rtt = if ct >= 9600 { 2 } else if ct >= 4800 { 1 } else { 0 };
                let quic_rtt = if ct >= 4800 { 1 } else { 0 };
                (
                    tcp,
                    quic,
                    vec![
                        TextSpec { text: "TCP".into(), x: 2.0, y: 12.0, dim: true, left: true },
                        TextSpec { text: format!("round trips before first HTTP byte → {tcp_rtt}"), x: 40.0, y: 38.0, dim: true, left: true },
                        TextSpec { text: "QUIC".into(), x: 2.0, y: 46.0, dim: true, left: true },
                        TextSpec { text: format!("round trips before first HTTP byte → {quic_rtt}"), x: 40.0, y: 72.0, dim: true, left: true },
                    ],
                )
            }
            2 => {
                let tcp = if ct >= 9150 {
                    CAP_T_B1
                } else if ct >= 4300 {
                    CAP_T_C1
                } else {
                    CAP_T_MULTI
                };
                let quic = if ct >= 9150 {
                    CAP_Q_B1
                } else if ct >= 7150 {
                    CAP_Q_C2
                } else {
                    CAP_Q_MULTI
                };
                // slot rows: A1 A2 | B1 B2 | C1 C2
                let arrive = |i: u64| ct >= i * SEND_GAP + FLY;
                let b1_back = ct >= 9150;
                let tcp_slot = |i: u64| -> &'static str {
                    if i == 1 {
                        return if b1_back { "✓" } else if ct >= 2850 { "×" } else { "…" };
                    }
                    if !arrive(i) { return "…" }
                    // everything behind the gap waits until B1 returns
                    if b1_back || i == 0 { "✓" } else { "◦" }
                };
                let quic_slot = |i: u64| -> &'static str {
                    if i == 1 {
                        return if b1_back { "✓" } else if ct >= 2850 { "×" } else { "…" };
                    }
                    if !arrive(i) { return "…" }
                    if i == 4 {
                        // B2 waits for its stream
                        if b1_back { "✓" } else { "◦" }
                    } else {
                        "✓"
                    }
                };
                let names = ["A1", "B1", "C1", "A2", "B2", "C2"];
                let row = |f: &dyn Fn(u64) -> &'static str| -> String {
                    names.iter().enumerate().map(|(i, n)| format!("{n}{}", f(i as u64))).collect::<Vec<_>>().join(" ")
                };
                (
                    tcp,
                    quic,
                    vec![
                        TextSpec { text: "TCP · one ordered stream".into(), x: 2.0, y: 12.0, dim: true, left: true },
                        TextSpec { text: row(&tcp_slot), x: 2.0, y: 38.0, dim: false, left: true },
                        TextSpec { text: "QUIC · independent streams".into(), x: 2.0, y: 46.0, dim: true, left: true },
                        TextSpec { text: row(&quic_slot), x: 2.0, y: 72.0, dim: false, left: true },
                    ],
                )
            }
            _ => {
                let tcp = if ct >= 7100 {
                    CAP_T_DEAD
                } else if ct >= 4600 {
                    CAP_T_LEAVE
                } else {
                    CAP_T_DL
                };
                let quic = if ct >= 9800 {
                    CAP_Q_MIGR
                } else if ct >= 4600 {
                    CAP_Q_LEAVE
                } else {
                    CAP_Q_DL
                };
                let dl = |i: u64, tcp: bool| -> &'static str {
                    let arrived = ct >= i * SEND_GAP + if tcp { 0 } else { 5500 } + if tcp { 0 } else { 0 };
                    let _ = arrived;
                    // TCP: 1-3 delivered by 1900+2400=4300..; 4-6 missing after 5500
                    if tcp {
                        if i < 3 {
                            if ct >= i * SEND_GAP + FLY { "✓" } else { "…" }
                        } else {
                            "×"
                        }
                    } else if i < 3 {
                        if ct >= i * SEND_GAP + FLY { "✓" } else { "…" }
                    } else if ct >= 5500 + (i - 3) * SEND_GAP + FLY {
                        "✓"
                    } else {
                        "…"
                    }
                };
                let row = |tcp: bool| -> String {
                    (0..6).map(|i| format!("{}{}", i + 1, dl(i, tcp))).collect::<Vec<_>>().join(" ")
                };
                let addr = if ct >= 4600 { "cellular, 198.51.100.4" } else { "Wi-Fi, 192.0.2.7" };
                (
                    tcp,
                    quic,
                    vec![
                        TextSpec { text: format!("TCP · client address → {addr}"), x: 2.0, y: 12.0, dim: true, left: true },
                        TextSpec { text: row(true), x: 2.0, y: 38.0, dim: false, left: true },
                        TextSpec { text: format!("QUIC · client address → {addr}"), x: 2.0, y: 46.0, dim: true, left: true },
                        TextSpec { text: row(false), x: 2.0, y: 72.0, dim: false, left: true },
                    ],
                )
            }
        };

        Frame {
            header: Some("INTERACTIVE · QUIC".into()),
            nodes: vec![
                NodeSpec { label: "client".into(), x: CLIENT, y: TCP_Y, status: None, lifeline: false, icon: None },
                NodeSpec { label: "server".into(), x: SERVER, y: TCP_Y, status: None, lifeline: false, icon: None },
                NodeSpec { label: "client".into(), x: CLIENT, y: QUIC_Y, status: None, lifeline: false, icon: None },
                NodeSpec { label: "server".into(), x: SERVER, y: QUIC_Y, status: None, lifeline: false, icon: None },
            ],
            packets,
            trails: vec![
                TrailSpec { x1: CLIENT, y1: TCP_Y, x2: SERVER, y2: TCP_Y, arrow_end: false },
                TrailSpec { x1: CLIENT, y1: QUIC_Y, x2: SERVER, y2: QUIC_Y, arrow_end: false },
            ],
            texts: extra,
            polylines: vec![],
            paths: vec![],
            badge: Some(format!("QUIC: {quic_note}")),
            note: format!("TCP: {tcp_note}"),
        }
    }
}
