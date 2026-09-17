//! Faithful port of fazamhd.com's CertChainSim: the browser climbs the signing
//! chain — wiki cert → intermediate → root in the trust store — then the forged
//! case: unknown issuer, no key to verify with, chain not trusted.

use crate::frame::*;
use std::sync::OnceLock;

const STEP1: u64 = 2400; // initial delay before first reveal (original: a)

const fn cap(n: u64) -> u64 {
    if n < 1400 { 1400 } else if n > 4200 { 4200 } else { n }
}

const STORE: [&str; 3] = ["DigiCert Global Root CA", "GlobalSign Root R1", "ISRG Root X1"];

struct Cert {
    name: &'static str,
    sub: &'static str,
    check_ok: bool,
}

struct Scenario {
    intro: &'static str,
    chain: Vec<Cert>,
    root_trusted: bool,
    narration: Vec<&'static str>,
    verdict: &'static str,
}

fn scenarios() -> Vec<Scenario> {
    vec![
        Scenario {
            intro: "The honest case first, the browser connects to the real en.wikipedia.org.",
            chain: vec![
                Cert { name: "en.wikipedia.org", sub: "the certificate the server presents", check_ok: true },
                Cert { name: "DigiCert TLS Hybrid ECC SHA384 2020 CA1", sub: "intermediate authority, not itself trusted", check_ok: true },
                Cert { name: "DigiCert Global Root CA", sub: "ships inside the browser, already trusted", check_ok: true },
            ],
            root_trusted: true,
            narration: vec![
                "The server presents a certificate claiming to be en.wikipedia.org, naming an intermediate authority run by DigiCert as its issuer. The browser trusts none of this yet.",
                "The browser hashes the certificate and checks its signature with the intermediate's public key. It verifies, so the intermediate really signed it, but the intermediate isn't trusted either, so the browser climbs a link.",
                "The same check one level up: the intermediate's own signature verifies with DigiCert Global Root CA's public key, and that root ships inside the browser. The chain terminates at something already trusted.",
            ],
            verdict: "Chain verified: en.wikipedia.org ← intermediate ← DigiCert Global Root CA. Padlock shown.",
        },
        Scenario {
            intro: "Now the failure case, an attacker intercepts the connection and answers in the server's place.",
            chain: vec![
                Cert { name: "en.wikipedia.org", sub: "forged, presented by an attacker", check_ok: true },
                Cert { name: "unknown issuer", sub: "no certificate offered, not in any trust store", check_ok: false },
            ],
            root_trusted: false,
            narration: vec![
                "The attacker presents their own certificate, also claiming to be en.wikipedia.org. Same first step, nothing trusted yet.",
                "The browser attempts the same check, but the named issuer is in no trust store and no certificate for it was offered, so there is no public key to verify the signature with, and no further link to climb.",
            ],
            verdict: "Verification fails, browser blocks the connection: \"Your connection is not private.\"",
        },
    ]
}

enum Phase {
    Intro,
    Reveal(usize), // boxes 0..=k shown
    Verdict,
}

struct Sched {
    events: Vec<(u64, usize, Phase)>, // (time, scenario, phase)
    pub duration: u64,
}

fn sched() -> &'static Sched {
    static S: OnceLock<Sched> = OnceLock::new();
    S.get_or_init(|| {
        let sc = scenarios();
        let mut events = Vec::new();
        let mut t = 0u64;
        for (si, s) in sc.iter().enumerate() {
            events.push((t, si, Phase::Intro));
            t += STEP1.max(cap(s.intro.len() as u64 * 28));
            for k in 0..s.chain.len() {
                events.push((t, si, Phase::Reveal(k)));
                t += cap(s.narration[k].len() as u64 * 28);
            }
            events.push((t, si, Phase::Verdict));
            t += cap(s.verdict.len() as u64 * 28) + 600;
        }
        Sched { events, duration: t }
    })
}

pub struct CertChain;

impl Sim for CertChain {
    fn duration(&self) -> u64 {
        sched().duration
    }

    fn frame(&self, t: u64) -> Frame {
        let sc = scenarios();
        let sd = sched();
        let tt = t % sd.duration;
        let (_, si, phase) = sd
            .events
            .iter()
            .rev()
            .find(|(at, _, _)| tt >= *at)
            .map(|(at, si, p)| (*at, *si, p))
            .unwrap();
        let s = &sc[si];
        let (revealed, note, verdict_shown) = match phase {
            Phase::Intro => (0, s.intro, false),
            Phase::Reveal(k) => (*k + 1, s.narration[*k], false),
            Phase::Verdict => (s.chain.len(), s.verdict, true),
        };

        let mut texts = vec![];
        // chain boxes, left column
        for (i, cert) in s.chain.iter().enumerate() {
            if i >= revealed {
                break;
            }
            let y = 14.0 + i as f64 * 16.0;
            texts.push(TextSpec { text: cert.name.into(), x: 2.0, y, dim: false, left: true });
            texts.push(TextSpec { text: cert.sub.into(), x: 2.0, y: y + 5.0, dim: true, left: true });
            if i > 0 {
                let arrow = if verdict_shown || i + 1 < revealed {
                    if cert.check_ok { "signed by, ✓ signature valid" } else { "signed by, ✕ no key to verify with" }
                } else {
                    "signed by, verifying signature…"
                };
                texts.push(TextSpec { text: format!("↑ {arrow}"), x: 2.0, y: y - 6.0, dim: true, left: true });
            }
        }
        // trust store, right column
        texts.push(TextSpec { text: "browser trust store".into(), x: 68.0, y: 8.0, dim: true, left: true });
        let last_name = s.chain.last().unwrap().name;
        for (i, root) in STORE.iter().enumerate() {
            let matched = verdict_shown && s.root_trusted && *root == last_name;
            texts.push(TextSpec {
                text: format!("{}{}", if matched { "● " } else { "" }, root),
                x: 68.0,
                y: 14.0 + i as f64 * 6.0,
                dim: !matched,
                left: true,
            });
        }

        Frame {
            header: Some("INTERACTIVE · TLS CERTIFICATES ·· THE CHAIN OF TRUST".into()),
            nodes: vec![],
            packets: vec![],
            trails: vec![],
            texts,
            polylines: vec![],
            paths: vec![],
            badge: if verdict_shown {
                Some(if s.root_trusted { "✓ chain verified" } else { "✕ chain not trusted" }.into())
            } else {
                None
            },
            note: note.into(),
        }
    }
}
