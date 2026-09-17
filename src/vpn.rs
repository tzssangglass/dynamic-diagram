//! Faithful port of fazamhd.com's VpnTunnelSim: the whole packet (IP header and
//! all) is encrypted and wrapped in a second packet addressed to the VPN server;
//! the ISP sees only the outer header. Same zigzag-hop shape as encap.

use crate::frame::*;
use std::sync::OnceLock;

const HOP_MS: u64 = 1600;
const POS: [(f64, f64); 6] = [
    (10.0, 24.0), // your device
    (26.0, 60.0), // your isp
    (42.0, 28.0), // router
    (58.0, 70.0), // vpn server
    (73.0, 26.0), // router
    (89.0, 58.0), // the site
];

const INNER: &str = "IP site ▸ TCP:443 ▸ TLS ▸ ▚▞▚▞▚";
const WRAPPED: &str = "IP vpn ▸ [ IP site ▸ TCP:443 ▸ TLS ▸ ▚▞▚▞▚ ]";

enum Ev {
    Move(usize),
    Stack(&'static str),
    Note(&'static str),
}

fn timeline() -> &'static [(u64, Ev)] {
    static T: OnceLock<Vec<(u64, Ev)>> = OnceLock::new();
    T.get_or_init(|| {
        let mut v = vec![
            (0, Ev::Stack(INNER)),
            (0, Ev::Note("Your request as it would normally leave: the TLS-encrypted payload, TCP, and an IP header naming the site.")),
            (2200, Ev::Stack(WRAPPED)),
            (2200, Ev::Note("The VPN client encrypts that entire packet, IP header included and wraps it in a new packet addressed to the VPN server.")),
            (4800, Ev::Move(1)),
            (6400, Ev::Note("Your ISP forwards it like any other packet, but all it can read is the outer header, encrypted bytes, bound for the VPN server. The site's name appears nowhere.")),
            (9200, Ev::Move(2)),
            (10800, Ev::Move(3)),
            (12400, Ev::Stack(INNER)),
            (12400, Ev::Note("The VPN server strips the outer packet and decrypts. Your original packet emerges, and is forwarded with the server's own address written in as the source.")),
            (15200, Ev::Move(4)),
            (16800, Ev::Move(5)),
            (18400, Ev::Note("The site answers to the VPN server's address; yours never appears. The view your ISP had of you now belongs to the VPN operator instead.")),
        ];
        v.sort_by_key(|(t, _)| *t);
        v
    })
}

pub const DURATION: u64 = 21800; // 18400 + 3400 pause (original)

pub struct Vpn;

impl Sim for Vpn {
    fn duration(&self) -> u64 {
        DURATION
    }

    fn frame(&self, t: u64) -> Frame {
        let tt = t % DURATION;
        let mut stack = INNER;
        let mut note = "";
        let mut from = POS[0];
        let mut to = POS[0];
        let mut mstart = 0u64;
        for (at, ev) in timeline() {
            if *at > tt {
                break;
            }
            match ev {
                Ev::Move(i) => {
                    from = to;
                    to = POS[*i];
                    mstart = *at;
                }
                Ev::Stack(s) => stack = s,
                Ev::Note(s) => note = s,
            }
        }
        let p = ((tt.saturating_sub(mstart)) as f64 / HOP_MS as f64).min(1.0);
        Frame {
            header: Some("INTERACTIVE · VPN ·· A PACKET INSIDE A PACKET".into()),
            nodes: vec![
                NodeSpec { label: "your device".into(), x: POS[0].0, y: POS[0].1, status: None, lifeline: false, icon: None },
                NodeSpec { label: "your isp".into(), x: POS[1].0, y: POS[1].1, status: None, lifeline: false, icon: None },
                NodeSpec { label: String::new(), x: POS[2].0, y: POS[2].1, status: None, lifeline: false, icon: None },
                NodeSpec { label: "vpn server".into(), x: POS[3].0, y: POS[3].1, status: None, lifeline: false, icon: None },
                NodeSpec { label: String::new(), x: POS[4].0, y: POS[4].1, status: None, lifeline: false, icon: None },
                NodeSpec { label: "the site".into(), x: POS[5].0, y: POS[5].1, status: None, lifeline: false, icon: None },
            ],
            packets: vec![PacketSpec {
                label: stack.into(),
                x1: from.0,
                y1: from.1,
                x2: to.0,
                y2: to.1,
                p,
                landed: false,
            }],
            trails: POS
                .windows(2)
                .map(|w| TrailSpec { x1: w[0].0, y1: w[0].1, x2: w[1].0, y2: w[1].1, arrow_end: false })
                .collect(),
            texts: vec![],
            polylines: vec![],
            paths: vec![],
            badge: None,
            note: note.into(),
        }
    }
}
