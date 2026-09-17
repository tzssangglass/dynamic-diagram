//! Faithful port of fazamhd.com's TlsHandshakeSim: ClientHello (share A) →
//! ServerHello (cert + share B) → both derive the session key → encrypted
//! Finished both ways → encrypted application data. This is HTTPS.

use crate::frame::*;
use std::sync::OnceLock;

const FLY: u64 = 1200; // plaintext flight (original: a)
const ENC_STEP: u64 = 750; // encrypt/decrypt beat (original: o)
const LOOP_PAUSE: u64 = 2600;
const BROWSER: f64 = 20.0;
const SERVER: f64 = 80.0;

const fn cap(n: u64) -> u64 {
    if n < 1600 { 1600 } else if n > 5200 { 5200 } else { n }
}

const CAP_HELLO: &str = "Browser picks a private number a = 6, keeps it, and sends ClientHello: the ciphers it supports plus share A = 5⁶ mod 23 = 8. Getting a back from 8 is the discrete logarithm problem, infeasible at real sizes.";
const CAP_SERVERHELLO: &str = "Server picks its own private number b = 15 and replies with its chosen cipher, share B = 5¹⁵ mod 23 = 19, and its certificate; the share is signed with the certificate's key, so a machine in the middle cannot swap in a share of its own.";
const CAP_KEYEXCHANGE: &str = "Each side raises the share it received to its own private number: 19⁶ mod 23 = 2 in the browser, 8¹⁵ mod 23 = 2 on the server. Same value, the session key, and the wire carried only 8 and 19.";
const CAP_FINISHED: &str = "Each side sends Finished, the first message protected by the new key. Watch the key work: encrypted at the sender, ciphertext on the wire, decrypted with the same key on arrival.";
const CAP_APPDATA: &str = "Every packet from here on gets the same treatment, encrypted with the session key before leaving, decrypted with it on arrival. This is HTTPS.";

struct Msg {
    text: &'static str,
    cipher: Option<&'static str>,
    right: bool,
    row: usize,
    start: u64, // flight start (after encrypt beat for encrypted msgs)
}

struct Sched {
    t_serverhello: u64,
    t_keyexchange: u64,
    t_key_shown: u64,
    t_finished: u64,
    t_appdata: u64,
    pub duration: u64,
    msgs: Vec<Msg>,
}

fn sched() -> &'static Sched {
    static S: OnceLock<Sched> = OnceLock::new();
    S.get_or_init(|| {
        let v = |c: &str, min: u64| cap(c.len() as u64 * 28).max(min);
        let t_serverhello = v(CAP_HELLO, 1500);
        let t_keyexchange = t_serverhello + v(CAP_SERVERHELLO, 1500);
        let t_key_shown = t_keyexchange + 1100;
        let t_finished = t_keyexchange + v(CAP_KEYEXCHANGE, 2400);
        // encrypted message: encrypt 750 -> fly 1200 -> decrypt 750 -> landed
        let t_finished2 = t_finished + 3000;
        let t_appdata = t_finished + v(CAP_FINISHED, 6050);
        let t_appdata2 = t_appdata + 3000;
        let duration = t_appdata + v(CAP_APPDATA, 6050) + LOOP_PAUSE;        let msgs = vec![
            Msg { text: "ClientHello + share A = 8", cipher: None, right: true, row: 0, start: 0 },
            Msg { text: "ServerHello + cert + share B = 19", cipher: None, right: false, row: 1, start: t_serverhello },
            Msg { text: "Finished", cipher: Some("f0:8a:c1:9e"), right: true, row: 2, start: t_finished + ENC_STEP },
            Msg { text: "Finished", cipher: Some("2d:77:0b:e4"), right: false, row: 3, start: t_finished2 + ENC_STEP },
            Msg { text: "GET /page", cipher: Some("27:b3:9c:4e:d1"), right: true, row: 4, start: t_appdata + ENC_STEP },
            Msg { text: "200 OK + HTML", cipher: Some("6c:e2:41:9a:b7"), right: false, row: 5, start: t_appdata2 + ENC_STEP },
        ];
        Sched { t_serverhello, t_keyexchange, t_key_shown, t_finished, t_appdata, duration, msgs }
    })
}

const LANE_Y: [f64; 6] = [45.0, 54.0, 63.0, 72.0, 81.0, 90.0];

pub struct TlsHandshake;

impl Sim for TlsHandshake {
    fn duration(&self) -> u64 {
        sched().duration
    }

    fn frame(&self, t: u64) -> Frame {
        let s = sched();
        let tt = t % s.duration;
        let (phase, note) = if tt >= s.t_appdata {
            ("encrypted application data", CAP_APPDATA)
        } else if tt >= s.t_finished {
            ("Finished (encrypted)", CAP_FINISHED)
        } else if tt >= s.t_keyexchange {
            ("both sides derive the session key", CAP_KEYEXCHANGE)
        } else if tt >= s.t_serverhello {
            ("ServerHello + certificate + key share", CAP_SERVERHELLO)
        } else {
            ("ClientHello + key share", CAP_HELLO)
        };
        let key_shown = tt >= s.t_key_shown;
        let sv_shown = tt >= s.t_serverhello;

        let mut packets = vec![];
        let mut trails = vec![];
        for m in &s.msgs {
            let lane_y = LANE_Y[m.row];
            let (from, to) = if m.right { (BROWSER, SERVER) } else { (SERVER, BROWSER) };
            let fly_end = m.start + FLY;
            let full_end = fly_end + if m.cipher.is_some() { ENC_STEP } else { 0 };
            if tt >= m.start && tt < fly_end {
                let p = (tt - m.start) as f64 / FLY as f64;
                let label = if let Some(c) = m.cipher { c } else { m.text };
                packets.push(PacketSpec { label: label.into(), x1: from, y1: lane_y, x2: to, y2: lane_y, p, landed: false });
            } else if tt >= full_end {
                // landed: faded cipher/text parked at destination + trail
                let label = m.cipher.unwrap_or(m.text);
                packets.push(PacketSpec { label: label.into(), x1: from, y1: lane_y, x2: to, y2: lane_y, p: 1.0, landed: true });
                trails.push(TrailSpec { x1: BROWSER, y1: lane_y, x2: SERVER, y2: lane_y, arrow_end: false });
            }
            // decrypting beat (750ms after landing): show cipher at destination —
            // rendered as flying packet that has arrived; covered by landed state.
        }

        let mut texts = vec![
            TextSpec { text: phase.into(), x: 2.0, y: 6.0, dim: true, left: true },
            TextSpec { text: "g = 5, p = 23 · public".into(), x: 50.0, y: 14.0, dim: false, left: false },
            TextSpec { text: "a = 6 · private, never sent".into(), x: BROWSER, y: 24.0, dim: false, left: false },
            TextSpec { text: "b = 15 · private, never sent".into(), x: SERVER, y: 24.0, dim: !sv_shown, left: false },
        ];
        if key_shown {
            texts.push(TextSpec { text: "Bᵃ = 19⁶ mod 23 = 2 · key".into(), x: BROWSER, y: 30.0, dim: false, left: false });
            texts.push(TextSpec { text: "Aᵇ = 8¹⁵ mod 23 = 2 · key".into(), x: SERVER, y: 30.0, dim: false, left: false });
        }

        Frame {
            header: Some("INTERACTIVE · TLS ·· THE HANDSHAKE".into()),
            nodes: vec![
                NodeSpec { label: "browser".into(), x: BROWSER, y: 18.0, status: None, lifeline: true, icon: None },
                NodeSpec { label: "server".into(), x: SERVER, y: 18.0, status: None, lifeline: true, icon: None },
            ],
            packets,
            trails,
            texts,
            polylines: vec![],
            paths: vec![],
            badge: None,
            note: note.into(),
        }
    }
}
