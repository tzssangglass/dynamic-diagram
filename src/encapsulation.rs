//! Faithful port of fazamhd.com's EncapsulationSim: a request builds up one
//! header per layer, crosses 5 hops (frame swapped at each), unwraps at the
//! server, then the response retraces the path. Timeline is a fixed schedule
//! reconstructed from the original component's chained timeouts.

use crate::frame::*;
use std::sync::OnceLock;

const HOP_MS: u64 = 1500; // ms per hop, linear (original: transition 1500ms)
// zigzag hop positions (original: l=[{x:10,y:24},...])
const POS: [(f64, f64); 6] = [
    (10.0, 24.0), // your device
    (25.0, 62.0), // home router
    (43.0, 28.0), // isp router
    (60.0, 66.0), // backbone
    (76.0, 26.0), // edge router
    (90.0, 62.0), // server
];

const CIPHER: &str = "▚▞▚▞▚";

enum Ev {
    Move(usize),
    Stack(&'static str),
    Note(&'static str),
}

// (time_ms, event) — sorted; built once
fn timeline() -> &'static [(u64, Ev)] {
    static T: OnceLock<Vec<(u64, Ev)>> = OnceLock::new();
    T.get_or_init(|| {
        let get = "GET /wiki/…";
        let ok = "200 OK, HTML";
        let tls_cipher = format!("TLS ▸ {CIPHER}");
        let tcp_out = format!("TCP:443 ▸ {tls_cipher}");
        let ip_out = format!("IP ▸ {tcp_out}");
        let wifi = format!("WIFI ▸ {ip_out}");
        let frame_out = format!("FRAME ▸ {ip_out}");
        // leak to get 'static — built once, lives for the process
        let s = |s: String| Box::leak(s.into_boxed_str()) as &'static str;
        let mut v: Vec<(u64, Ev)> = vec![
            (0, Ev::Stack(get)),
            (0, Ev::Note("The browser writes its request, a few hundred bytes of plain text.")),
            (1500, Ev::Stack(s(tls_cipher.clone()))),
            (1500, Ev::Note("TLS encrypts it with the session key. From here on, no machine along the path can read it.")),
            (3000, Ev::Stack(s(tcp_out.clone()))),
            (3000, Ev::Note("TCP adds its header: the destination port (which program on the server) and a sequence number (where these bytes sit in the stream).")),
            (4500, Ev::Stack(s(ip_out.clone()))),
            (4500, Ev::Note("IP adds the source and destination addresses, the only part routers along the way will read.")),
            (6000, Ev::Stack(s(wifi.clone()))),
            (6000, Ev::Note("The Wi-Fi frame addresses the packet one hop only, to your home router, nothing further.")),
            (6000, Ev::Move(1)),
            (7500, Ev::Stack(s(frame_out.clone()))),
            (7500, Ev::Note("Your home router strips the frame, rewrites your private source address to your public one (NAT), and wraps the same packet in a new frame for the ISP link.")),
            (8900, Ev::Move(2)),
            (10400, Ev::Note("Your ISP's router repeats the move: strip the frame, read the IP destination, wrap a new frame for the next link. Nothing inside is touched.")),
            (11800, Ev::Move(3)),
            (13300, Ev::Note("The packet crosses onto a backbone network owned by someone else entirely. Same swap, no coordination needed, nothing read past the IP header.")),
            (14700, Ev::Move(4)),
            (16200, Ev::Note("A last boundary into the server's network, a last new frame. The IP header and everything inside it are unchanged since your device.")),
            (17600, Ev::Move(5)),
            (19100, Ev::Stack(s(ip_out.clone()))),
            (19100, Ev::Note("The server unwraps in reverse. The frame has served its final hop.")),
            (20600, Ev::Stack(s(tcp_out.clone()))),
            (20600, Ev::Note("The IP header carried it across the world and is done.")),
            (22100, Ev::Stack(s(tls_cipher.clone()))),
            (22100, Ev::Note("TCP slotted the bytes into order and hands over a clean stream.")),
            (23600, Ev::Stack(get)),
            (23600, Ev::Note("And only the server, holding the session key, turns the ciphertext back into the request.")),
            (25900, Ev::Stack(s(frame_out.clone()))),
            (25900, Ev::Note("The response \"200 OK\" and the page, is encrypted with the same session key and wrapped in the same layers for the trip back.")),
            (27400, Ev::Note("It retraces the path (as dozens of packets, in reality), each hop again replacing only the outermost frame.")),
            (27400, Ev::Move(4)),
            (29900, Ev::Move(3)),
            (32400, Ev::Move(2)),
            (34900, Ev::Move(1)),
            (36400, Ev::Stack(s(wifi.clone()))),
            (37400, Ev::Move(0)),
            (38900, Ev::Stack(s(tls_cipher.clone()))),
            (38900, Ev::Note("Your device unwraps in the same order, frame, IP, and TCP have each done their job.")),
            (40400, Ev::Stack(ok)),
            (40400, Ev::Note("And your browser, holding the session key, decrypts the page. Only the two endpoints ever saw plain bytes.")),
        ];
        v.sort_by_key(|(t, _)| *t);
        v
    })
}

pub const DURATION: u64 = 43000; // last event 40400 + 2600 pause (original)

pub struct Encapsulation;

impl Sim for Encapsulation {
    fn duration(&self) -> u64 {
        DURATION
    }

    fn frame(&self, t: u64) -> Frame {
        let tt = t % DURATION;
        let mut stack = "GET /wiki/…";
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
            header: Some("INTERACTIVE · ENCAPSULATION ·· ONE HEADER PER LAYER".into()),
            nodes: vec![
                NodeSpec { label: "your device".into(), x: POS[0].0, y: POS[0].1, status: None, lifeline: false, icon: None },
                NodeSpec { label: "home router".into(), x: POS[1].0, y: POS[1].1, status: None, lifeline: false, icon: None },
                NodeSpec { label: String::new(), x: POS[2].0, y: POS[2].1, status: None, lifeline: false, icon: None },
                NodeSpec { label: String::new(), x: POS[3].0, y: POS[3].1, status: None, lifeline: false, icon: None },
                NodeSpec { label: String::new(), x: POS[4].0, y: POS[4].1, status: None, lifeline: false, icon: None },
                NodeSpec { label: "server".into(), x: POS[5].0, y: POS[5].1, status: None, lifeline: false, icon: None },
            ],
            packets: vec![PacketSpec {
                label: stack.into(),
                x1: from.0,
                y1: from.1,
                x2: to.0,
                y2: to.1,
                p,
                landed: false, // encap packets keep full opacity when parked at a hop
            }],
            trails: POS
                .windows(2)
                .map(|w| TrailSpec { x1: w[0].0, y1: w[0].1, x2: w[1].0, y2: w[1].1, arrow_end: false })
                .collect(),
            badge: None,
            texts: vec![],
            polylines: vec![],
            paths: vec![],
            note: note.into(),
        }
    }
}
