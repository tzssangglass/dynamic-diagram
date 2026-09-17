//! Faithful port of fazamhd.com's ChecksumSim: sender sums "Hello" bytes mod 256,
//! ships data+checksum; run 1 clean (accepted), run 2 a bit flips in transit
//! (byte 2: 0x6c^0x40=0x2c) and the receiver discards the frame.

use crate::frame::*;
use std::sync::OnceLock;

const DATA: [u8; 5] = [72, 101, 108, 108, 111]; // "Hello"
const FLIP_IDX: usize = 2;
const SENDER_X: f64 = 10.0;
const RECV_X: f64 = 90.0;
const FLY_MS: u64 = 700;   // checksum packet flight (original: a)
const PAUSE: u64 = 1500;   // after compare (original: o)

// original: caption time = max(1100, min(4200, chars*28)) ms
const fn cap(n: u64) -> u64 {
    if n < 1100 { 1100 } else if n > 4200 { 4200 } else { n }
}

fn hex(b: u8) -> String {
    format!("0x{b:02x}")
}
fn sum(bytes: &[u8]) -> u32 {
    bytes.iter().map(|&b| b as u32).sum()
}
fn checksum(bytes: &[u8]) -> u32 {
    sum(bytes) % 256
}
fn calc_line(bytes: &[u8]) -> String {
    format!(
        "{} = {:#04x} mod 256 = {:#04x}",
        bytes.iter().map(|&b| hex(b)).collect::<Vec<_>>().join(" + "),
        sum(bytes),
        checksum(bytes)
    )
}

pub struct Run {
    pub corrupt: bool,
    pub t_send: u64,
    pub t_arrived: u64,
    pub t_recompute: u64,
    pub t_compare: u64,
    pub t_next: u64,
    cap_compute: String,
    cap_send: String,
    cap_arrived: String,
    cap_recompute: String,
    cap_compare: String,
}

pub fn runs() -> &'static [Run] {
    static R: OnceLock<Vec<Run>> = OnceLock::new();
    R.get_or_init(|| {
        let good = checksum(&DATA);
        let mut v = Vec::new();
        for corrupt in [false, true] {
            let rcvd: Vec<u8> = DATA
                .iter()
                .enumerate()
                .map(|(i, &b)| if corrupt && i == FLIP_IDX { b ^ 64 } else { b })
                .collect();
            let rsum = checksum(&rcvd);
            let data_hex = DATA.iter().map(|&b| hex(b)).collect::<Vec<_>>().join(" ");
            let cap_compute = format!(
                "Sender adds up the bytes: {}. That last value, {}, ships alongside the data as the checksum.",
                calc_line(&DATA),
                hex(good as u8)
            );
            let cap_send = if corrupt {
                format!("Sender ships the data {data_hex} with checksum {}. Noise on the link flips a bit in transit.", hex(good as u8))
            } else {
                format!("Sender ships the data {data_hex} with checksum {}, a clean run down the wire.", hex(good as u8))
            };
            let cap_arrived = if corrupt {
                format!("Received: byte {FLIP_IDX} arrived as {} instead of {}. The receiver has no way to see that yet.", hex(rcvd[FLIP_IDX]), hex(DATA[FLIP_IDX]))
            } else {
                format!("Received intact: {data_hex}, checksum {}.", hex(good as u8))
            };
            let cap_recompute = format!("Receiver runs the identical sum over the bytes it got: {}.", calc_line(&rcvd));
            let cap_compare = if rsum == good {
                format!("Compares {} to the checksum that arrived, {}, match. Frame accepted.", hex(rsum as u8), hex(good as u8))
            } else {
                format!("Compares {} to the checksum that arrived, {}, mismatch. Frame silently discarded, corruption caught with no idea which bit or why.", hex(rsum as u8), hex(good as u8))
            };
            let t_send = cap(cap_compute.len() as u64 * 28);
            let t_arrived = t_send + cap(cap_send.len() as u64 * 28).max(850);
            let t_recompute = t_arrived + cap(cap_arrived.len() as u64 * 28);
            let t_compare = t_recompute + cap(cap_recompute.len() as u64 * 28);
            let t_next = t_compare + cap(cap_compare.len() as u64 * 28) + PAUSE;
            v.push(Run { corrupt, t_send, t_arrived, t_recompute, t_compare, t_next, cap_compute, cap_send, cap_arrived, cap_recompute, cap_compare });
        }
        v
    })
}

pub fn duration() -> u64 {
    runs().iter().map(|r| r.t_next).sum()
}

pub struct Checksum;

impl Sim for Checksum {
    fn duration(&self) -> u64 {
        duration()
    }

    fn frame(&self, t: u64) -> Frame {
        let rs = runs();
        let tt = t % duration();
        // find current run
        let mut base = 0u64;
        let run = rs
            .iter()
            .find(|r| {
                if tt < base + r.t_next {
                    true
                } else {
                    base += r.t_next;
                    false
                }
            })
            .unwrap();
        let dt = tt - base;
        let rcvd: Vec<u8> = DATA
            .iter()
            .enumerate()
            .map(|(i, &b)| if run.corrupt && i == FLIP_IDX { b ^ 64 } else { b })
            .collect();
        let good = checksum(&DATA);

        let (note, stage) = if dt < run.t_send {
            (run.cap_compute.as_str(), "computing")
        } else if dt < run.t_arrived {
            (run.cap_send.as_str(), "sending")
        } else if dt < run.t_recompute {
            (run.cap_arrived.as_str(), "arrived")
        } else if dt < run.t_compare {
            (run.cap_recompute.as_str(), "recompute")
        } else {
            (run.cap_compare.as_str(), "compare")
        };

        // checksum packet: flies sender->receiver during sending, parked after
        let mut packets = vec![];
        if stage != "computing" {
            let p = if dt < run.t_send + 30 {
            0.0
        } else {
            ((dt - run.t_send - 30) as f64 / FLY_MS as f64).min(1.0)
        };
            packets.push(PacketSpec {
                label: format!("checksum {}", hex(good as u8)),
                x1: SENDER_X,
                y1: 18.0,
                x2: RECV_X,
                y2: 18.0,
                p,
                landed: p >= 1.0,
            });
        }

        let mut texts = vec![
            TextSpec { text: calc_line(&DATA), x: SENDER_X, y: 62.0, dim: true, left: false },
        ];
        if run.corrupt && (stage == "sending" || stage == "arrived") {
            texts.push(TextSpec { text: "bit flip".into(), x: 50.0, y: 42.0, dim: false, left: false });
        }
        // receiver side
        let recv_bytes = if stage == "computing" || stage == "sending" {
            "awaiting bytes".to_string()
        } else {
            rcvd.iter().map(|&b| hex(b)).collect::<Vec<_>>().join(" ")
        };
        texts.push(TextSpec { text: recv_bytes, x: RECV_X, y: 62.0, dim: true, left: false });
        if stage == "recompute" || stage == "compare" {
            texts.push(TextSpec { text: calc_line(&rcvd), x: RECV_X, y: 70.0, dim: true, left: false });
        }
        if stage == "compare" {
            let ok = checksum(&rcvd) == good;
            texts.push(TextSpec {
                text: if ok { "✓ accepted".into() } else { "✕ discarded".into() },
                x: RECV_X,
                y: 80.0,
                dim: false,
                left: false,
            });
        }

        Frame {
            header: Some("INTERACTIVE · CHECKSUM ·· DETECTING CORRUPTION".into()),
            nodes: vec![
                NodeSpec {
                    label: "sender".into(),
                    x: SENDER_X,
                    y: 50.0,
                    status: Some(DATA.iter().map(|&b| hex(b)).collect::<Vec<_>>().join(" ")),
                    lifeline: false,
                    icon: None,
                },
                NodeSpec { label: "receiver".into(), x: RECV_X, y: 50.0, status: None, lifeline: false, icon: None },
            ],
            packets,
            trails: vec![TrailSpec { x1: SENDER_X, y1: 50.0, x2: RECV_X, y2: 50.0, arrow_end: false }],
            texts,
            polylines: vec![],
            paths: vec![],
            badge: None,
            note: note.into(),
        }
    }
}
