//! dynamic-diagram — portable interactive-diagram engine.
//!
//!   dynamic-diagram [sim]         serve at http://localhost:8080 (SVG in browser)
//!   dynamic-diagram kitty [sim]   real-pixel animation in a kitty-protocol terminal
//!   dynamic-diagram check         runnable correctness check
//!   dynamic-diagram skill         print the agent authoring skill (SKILL.md)
//!
//! sims: tcphs (default) | encap

mod assets;
mod frame;
mod sims_data;
mod spec;
mod timeline;
mod kitty;
mod svg;

use frame::{Frame, Sim};
use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

fn get_sim(name: &str) -> Box<dyn Sim> {
    // every animation is a timeline document (embedded at compile time) —
    // the engine carries zero per-animation code
    let json = sims_data::get(name).unwrap_or_else(|| sims_data::get("tcphs").unwrap());
    let doc = timeline::parse_doc(json).unwrap_or_else(|e| { eprintln!("sim {name:?}: {e}"); std::process::exit(2) });
    Box::new(timeline::DocSim(doc))
}

fn main() {
    let mut args = std::env::args().skip(1);
    let mode = args.next().unwrap_or_else(|| "web".into());
    match mode.as_str() {
        "check" => check(),
        "spec" => spec_mode(
            &args.next().unwrap_or_else(|| { eprintln!("usage: dynamic-diagram spec <file.json> [svg|png|kitty|frames|kitty-anim]"); std::process::exit(2) }),
            args.next().unwrap_or_else(|| "png".into()),
            args.collect(),
        ),
        "kitty" => kitty_mode(get_sim(&args.next().unwrap_or_default())),
        "skill" => print!("{}", include_str!("../skills/dynamic-diagram/SKILL.md")),
        "web" => web_mode(get_sim(&args.next().unwrap_or_default())),
        // allow `dynamic-diagram encap` as shorthand for `web encap`
        sim => web_mode(get_sim(sim)),
    }
}

// spec mode: render a declarative JSON spec document.
//   spec <f.json> [png]        single frame -> <f>.png (or stdout path print)
//   spec <f.json> svg          single frame SVG to stdout
//   spec <f.json> kitty        single frame inline to terminal
//   spec <f.json> frames <dir> [n]  sample n frames of an animated spec
//   spec <f.json> kitty-anim   stream animated frames until Ctrl+C
fn spec_mode(path: &str, out_mode: String, extra: Vec<String>) {
    if path == "--list-icons" {
        for name in assets::icons::all() {
            println!("{name}");
        }
        return;
    }
    let json = std::fs::read_to_string(path).unwrap_or_else(|e| { eprintln!("read {path}: {e}"); std::process::exit(2) });
    // one door for any scene: docs with "els" are v2 timeline documents,
    // everything else is v1 sugar (both compile to the same Doc)
    let parsed = if json.contains("\"els\"") {
        timeline::parse_doc(&json).unwrap_or_else(|e| { eprintln!("{e}"); std::process::exit(2) })
    } else {
        spec::parse(&json).unwrap_or_else(|e| { eprintln!("{e}"); std::process::exit(2) })
    };
    let frame_png = |t: u64| -> Vec<u8> { raster(&svg::render_svg(&parsed.frame_at(t))).unwrap() };
    let (cols, rows) = term_size();
    let fit_rows = (cols.saturating_sub(2), rows.saturating_sub(2));

    match out_mode.as_str() {
        "svg" => print!("{}", svg::render_svg(&parsed.frame_at(0))),
        "kitty" => {
            let png = frame_png(0);
            let (c, r) = fit_rows;
            print!("{}", kitty::kitty_png(&png, c.saturating_sub(2), r.min((c as f64 * 0.5 * (svg::H / svg::W)).round().max(8.0) as u32)));
        }
        "frames" => {
            let Some(duration) = parsed.duration else { eprintln!("spec has no \"duration\" — nothing to animate"); std::process::exit(2) };
            let dir = extra.first().cloned().unwrap_or_else(|| path.trim_end_matches(".json").to_string() + ".frames");
            let count: u64 = extra.get(1).and_then(|s| s.parse().ok()).unwrap_or(12);
            std::fs::create_dir_all(&dir).unwrap();
            for i in 0..count {
                let t = i * duration / count;
                let out = format!("{dir}/{:03}.png", i + 1);
                std::fs::write(&out, frame_png(t)).unwrap();
            }
            println!("wrote {count} frames to {dir}/ (loop {duration}ms)");
        }
        "kitty-anim" => {
            let Some(duration) = parsed.duration else { eprintln!("spec has no \"duration\" — nothing to animate"); std::process::exit(2) };
            let count: u64 = 12;
            let frame_ms = (duration / count).max(60);
            // pre-render the loop once, then replay
            let frames: Vec<Vec<u8>> = (0..count).map(|i| frame_png(i * duration / count)).collect();
            let quit = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
            signal_hook::flag::register(signal_hook::consts::SIGINT, Arc::clone(&quit)).unwrap();
            let out = std::io::stdout();
            let mut out = out.lock();
            out.write_all(b"\x1b[?25l").unwrap();
            let t0 = std::time::Instant::now();
            while !quit.load(Ordering::Relaxed) {
                let idx = ((t0.elapsed().as_millis() as u64 / frame_ms) % count) as usize;
                let (c, r) = fit_rows;
                let rows = r.min((c as f64 * 0.5 * (svg::H / svg::W)).round().max(8.0) as u32);
                out.write_all(format!("\x1b[H{}", kitty::kitty_png(&frames[idx], c.saturating_sub(2), rows)).as_bytes()).unwrap();
                out.flush().unwrap();
                std::thread::sleep(std::time::Duration::from_millis(frame_ms));
            }
            out.write_all(format!("{}\x1b[?25h", kitty::KITTY_CLEAR).as_bytes()).unwrap();
        }
        _ => {
            let png = frame_png(0);
            let out = path.trim_end_matches(".json").to_string() + ".png";
            std::fs::write(&out, png).unwrap();
            println!("wrote {out} ({} bytes)", std::fs::metadata(&out).unwrap().len());
        }
    }
}

fn check() {
    // every sim is a timeline doc in sims/ — these asserts run against the
    // loaded documents, so the check doubles as the migration regression gate.
    fn find_frame(sim: &dyn Sim, mut pred: impl FnMut(&Frame) -> bool) -> Frame {
        let mut t = 0;
        loop {
            let f = sim.frame(t);
            if pred(&f) {
                return f;
            }
            t += 50;
            if t > sim.duration() {
                panic!("frame predicate never matched within a loop");
            }
        }
    }

    // tcphs
    let sim = get_sim("tcphs");
    const T_SYNACK: u64 = 3164;
    const T_EST: u64 = 9604;
    let f0 = sim.frame(0);
    assert_eq!(f0.packets[0].label, "SYN seq=5000", "SYN departs at t=0");
    assert!(f0.packets[0].p == 0.0);
    assert_eq!(f0.nodes[0].status.as_deref(), Some("seq 5000"), "client seq at t=0");
    assert_eq!(f0.nodes[1].status.as_deref(), Some("sequence pending"), "server pending at t=0");
    let mid_syn = sim.frame(900);
    assert!(mid_syn.packets[0].p == 0.5, "1800ms linear flight: p=0.5 at t=900");
    let landed_syn = sim.frame(1800);
    assert!(landed_syn.packets[0].landed && landed_syn.trails.len() == 1, "SYN lands at 1800 + trail");
    let mid_synack = sim.frame(T_SYNACK + 900);
    assert!(mid_synack.packets.iter().any(|p| p.label.starts_with("SYN-ACK") && !p.landed), "SYN-ACK flying");
    assert_eq!(mid_synack.nodes[1].status.as_deref(), Some("seq 9000"), "server seq revealed");
    assert!(mid_synack.packets.iter().find(|p| !p.landed).unwrap().x1 == 80.0, "reply server->client");
    let est = sim.frame(T_EST + 10);
    assert_eq!(est.badge.as_deref(), Some("established"), "badge at established");
    assert!(est.trails.len() == 3, "3 trails at established");
    assert!(est.note.contains("byte count starts"), "established caption");
    assert_eq!(sim.frame(sim.duration()).packets[0].label, "SYN seq=5000", "loops cleanly");
    let s = svg::render_svg(&mid_syn);
    assert!(s.contains("<svg") && s.contains("SYN seq=5000") && s.contains("sequence pending"), "svg: scene");
    assert!(svg::render_svg(&est).contains("ESTABLISHED"), "svg: badge");

    // utf-8 wrap regression: multi-byte captions must not panic the renderer
    let cjk = frame::Frame {
        nodes: vec![], packets: vec![], trails: vec![], texts: vec![], polylines: vec![], paths: vec![],
        badge: None, header: None,
        note: "中文长字幕换行回归测试，超过九十二个字符阈值，包含上标 ⁵⁶¹ 与混合文本 mixed superscripts，确保字节边界安全。".into(),
    };
    let _ = svg::render_svg(&cjk);
    let dh_sim = get_sim("dh");
    let _ = svg::render_svg(&dh_sim.frame(3000)); // superscript captions

    assert_eq!(kitty::base64_encode(b"fakepng"), "ZmFrZXBuZw==", "base64 known vector");
    let esc = kitty::kitty_png(b"fakepng", 60, 18);
    assert!(esc.starts_with("\x1b_Ga=T,f=100") && esc.ends_with("\x1b\\"), "kitty: escape structure");

    // encap
    let encap = get_sim("encap");
    let e0 = encap.frame(0);
    assert_eq!(e0.packets[0].label, "GET /wiki/…", "encap starts with raw request");
    assert!(e0.packets[0].x1 == 10.0 && e0.packets[0].p == 0.0, "encap packet at device");
    let hop = encap.frame(6000 + 750);
    assert!(hop.packets[0].p > 0.0 && hop.packets[0].p < 1.0, "encap flying to home router");
    assert!(hop.packets[0].label.starts_with("WIFI"), "wifi frame outbound first hop");
    let at_server = encap.frame(24000);
    assert!(at_server.packets[0].x2 == 90.0 && at_server.packets[0].p == 1.0, "reached server");
    assert_eq!(encap.frame(40500).packets[0].label, "200 OK, HTML", "response decrypted at device");
    assert_eq!(encap.frame(encap.duration() + 100).packets[0].label, "GET /wiki/…", "encap loops");
    let esvg = svg::render_svg(&encap.frame(6000 + 750));
    assert!(esvg.contains("your device") && esvg.contains("server") && esvg.contains("WIFI"), "encap svg scene");

    // arp — broadcast, learn, cache hit (episodes probed: schedule is data now)
    let arp_sim = get_sim("arp");
    assert_eq!(arp_sim.frame(100).packets.len(), 3, "broadcast to 3 other hosts");
    let fly = arp_sim.frame(250);
    assert!(fly.packets[0].x1 > 8.0 && fly.packets[0].x1 < 50.0, "broadcast mid leg1");
    let _ = find_frame(&*arp_sim, |f| f.badge.as_deref().is_some_and(|b| b.contains("7A:8B:9C")));
    let hit = find_frame(&*arp_sim, |f| f.note.contains("already has") && f.packets.is_empty());
    assert!(hit.packets.is_empty(), "cache-hit episode: no packets");
    let _ = find_frame(&*arp_sim, |f| f.badge.as_deref().is_some_and(|b| b.contains("AA:BB:CC")));
    assert!(arp_sim.frame(arp_sim.duration() + 100).packets.len() == 3, "arp loops");
    let asvg = svg::render_svg(&fly);
    assert!(asvg.contains("host 1") && asvg.contains("who has 203.0.113.30?"), "arp svg scene");

    // modem — wave split at playhead, bits revealed as they arrive
    let modem = get_sim("modem");
    let m0 = modem.frame(0);
    assert!(m0.polylines.len() == 2 && m0.polylines[0].points.len() <= 1, "modem: nothing drawn at t=0");
    let mmid = modem.frame(2600);
    let (bold, faint) = (&mmid.polylines[0], &mmid.polylines[1]);
    assert!(bold.points.len() > 50 && faint.points.len() > 50, "modem: wave split at playhead");
    assert!(modem.frame(0).texts.iter().any(|t| t.text == "?"), "modem: received hidden at start");
    let mdone = modem.frame(modem.duration() - 100);
    assert!(mdone.note.contains("0x41") && mdone.note.contains("'A'"), "modem: byte decoded at end");
    assert!(!mdone.texts.iter().any(|t| t.text == "?"), "modem: all received at end");
    let msvg = svg::render_svg(&mmid);
    assert!(msvg.contains("<polyline"), "modem svg: waveform");

    // vpn
    let vpn = get_sim("vpn");
    assert!(vpn.frame(0).packets[0].label.starts_with("IP site"), "vpn: plain packet at start");
    let tun = vpn.frame(3000);
    assert!(tun.packets[0].label.starts_with("IP vpn"), "vpn: wrapped in tunnel");
    let midway = vpn.frame(6400 + 1600);
    assert!(midway.packets[0].x2 == 42.0 || midway.packets[0].x2 == 26.0, "vpn: crossing isp segment");
    assert!(vpn.frame(13000).packets[0].label.starts_with("IP site"), "vpn: unwrapped after server");
    assert!(vpn.frame(vpn.duration() + 100).packets[0].label.starts_with("IP site"), "vpn loops");
    let vsvg = svg::render_svg(&tun);
    assert!(vsvg.contains("vpn server") && vsvg.contains("IP vpn"), "vpn svg scene");

    // ipbits
    let ipb = get_sim("ipbits");
    let s0 = ipb.frame(0);
    assert!(s0.texts.iter().any(|t| t.text.contains("checking route 1 of 4")), "ipbits: step 0");
    assert!(s0.note.contains("fixes the first 24 bits"), "ipbits: /24 caption");
    let s2 = ipb.frame(2 * 2600);
    assert!(s2.note.contains("bit 2 differs"), "ipbits: /8 ruled out at bit 2");
    let fin = ipb.frame(4 * 2600);
    assert!(fin.note.contains("3 routes match") && fin.note.contains("91.198.174.0/24 is the most specific"), "ipbits: longest prefix wins");
    assert!(fin.texts.iter().any(|t| t.text.contains("← used")), "ipbits: winner marked");
    assert!(ipb.frame(ipb.duration() + 10).note.contains("fixes the first 24"), "ipbits loops");

    // checksum — clean run accepted, corrupt run discarded (probed)
    let cks = get_sim("checksum");
    let c0 = cks.frame(0);
    assert!(c0.note.contains("Sender adds up the bytes") && c0.packets.is_empty(), "checksum: computing stage");
    assert!(c0.texts.iter().any(|t| t.text.contains("0x48")), "checksum: sender bytes shown");
    let _ = find_frame(&*cks, |f| f.note.contains("flips a bit") && f.texts.iter().any(|t| t.text == "bit flip"));
    let _ = find_frame(&*cks, |f| f.note.contains("mismatch") && f.texts.iter().any(|t| t.text == "✕ discarded"));
    let _ = find_frame(&*cks, |f| f.texts.iter().any(|t| t.text == "✓ accepted"));
    assert!(cks.frame(cks.duration() + 10).note.contains("Sender adds up"), "checksum loops");
    let csvg = svg::render_svg(&cks.frame(1500));
    assert!(csvg.contains("sender") && csvg.contains("receiver"), "checksum svg scene");

    // bgp
    let bgp = get_sim("bgp");
    let b0 = bgp.frame(0);
    assert!(b0.note.contains("originates") && b0.packets.is_empty(), "bgp: origin stage");
    let hop1_start = find_frame(&*bgp, |f| f.packets.len() == 2).badge.clone();
    let mid1 = find_frame(&*bgp, |f| f.packets.len() == 2 && f.packets[0].label == "[1]");
    assert!(mid1.packets[0].label == "[1]", "bgp: hop1 announcements");
    let _ = hop1_start;
    let after1 = find_frame(&*bgp, |f| f.nodes.get(1).and_then(|n| n.status.as_deref()).is_some_and(|s| s.contains("[1, 2]")));
    assert!(after1.nodes[1].status.as_deref().unwrap().contains("[1, 2]"), "bgp: AS2 learned");
    let last = bgp.frame(bgp.duration() - 100);
    assert!(last.note.contains("shortest AS-path wins") && last.nodes[4].status.as_deref() == Some("2 routes"), "bgp: decided");
    assert!(last.texts.iter().any(|t| t.text.contains("← used")), "bgp: winner marked");
    assert!(bgp.frame(bgp.duration() + 50).note.contains("originates"), "bgp loops");

    // certchain
    let cc = get_sim("certchain");
    let c0 = cc.frame(0);
    assert!(c0.note.contains("honest case") && c0.texts.iter().all(|t| t.dim), "certchain: intro, no boxes");
    let mut tv = 0u64;
    let mut saw_root = false;
    let mut saw_ok = false;
    let mut saw_fail = false;
    while tv < cc.duration() {
        let f = cc.frame(tv);
        if f.texts.iter().any(|t| t.text == "DigiCert Global Root CA" && !t.dim) {
            saw_root = true;
        }
        if f.badge.as_deref() == Some("✓ chain verified") {
            saw_ok = true;
        }
        if f.badge.as_deref() == Some("✕ chain not trusted") {
            saw_fail = true;
        }
        tv += 300;
    }
    assert!(saw_root && saw_ok && saw_fail, "certchain: both scenarios play out");
    assert!(cc.frame(cc.duration() + 50).note.contains("honest case"), "certchain loops");

    // tcpvsudp
    let tvu = get_sim("tcpvsudp");
    assert!(tvu.frame(0).note.contains("Sending 5 segments"), "tcpvsudp: start");
    let drop_t = tvu.frame(2 * 650 + 400);
    assert!(drop_t.packets.iter().any(|p| p.label == "3" && p.x1 < 50.0 && p.x1 > 8.0), "tcpvsudp: packet 3 mid-drop");
    let resent = tvu.frame(4500);
    assert!(resent.note.contains("resends it"), "tcpvsudp: resend caption");
    let done = tvu.frame(6000);
    assert!(done.note.contains("flushes the buffer"), "tcpvsudp: flush on resend arrival");
    assert!(done.texts.iter().any(|t| t.text.contains("3✓")), "tcpvsudp: slot 3 delivered after resend");
    assert!(done.badge.as_deref().unwrap().contains("permanent gap"), "tcpvsudp: udp gap");
    assert!(tvu.frame(tvu.duration() + 100).note.contains("Sending 5 segments"), "tcpvsudp loops");

    // anycast
    let ac = get_sim("anycast");
    let a0 = ac.frame(0);
    assert!(a0.paths.len() == 1 && a0.paths[0].len() > 40000, "anycast: world map embedded");
    assert!(a0.note.contains("Lisbon"), "anycast: first client Lisbon");
    let fwd = find_frame(&*ac, |f| f.note.contains("Delivered to edge, London"));
    assert!(fwd.packets.len() == 1 && fwd.polylines.iter().filter(|p| !p.dim).count() == 1, "anycast: one winner route");
    let mut ts = 0;
    let mut tokyo = false;
    while ts < ac.duration() {
        if ac.frame(ts).note.contains("Delivered to edge, Tokyo") {
            tokyo = true;
            break;
        }
        ts += 300;
    }
    assert!(tokyo, "anycast: Sydney -> Tokyo episode");
    assert!(ac.frame(ac.duration() + 50).note.contains("Lisbon"), "anycast loops");

    // dialup
    let du = get_sim("dialup");
    let d0 = du.frame(0);
    assert!(d0.polylines.len() == 2 && d0.note.contains("dial tone"), "dialup: phase 0");
    assert!(du.frame(7000).note.contains("low-speed FSK bursts"), "dialup: capability phase at 7s");
    assert!(du.frame(27000).note.contains("speaker goes silent"), "dialup: connected at 27s");
    let wlen = du.frame(7000).polylines[0].points.len();
    assert!((150..=175).contains(&wlen), "dialup: waveform samples (got {wlen})");
    assert!(du.frame(du.duration() + 50).note.contains("dial tone"), "dialup loops");

    // dh
    let dh = get_sim("dh");
    assert!(dh.frame(0).note.contains("g = 5 and p = 23"), "dh: publics");
    let mut saw_a = false;
    let mut saw_b = false;
    let mut saw_key = false;
    let mut saw_fail = false;
    let mut t = 0;
    while t < dh.duration() {
        let f = dh.frame(t);
        if f.packets.iter().any(|p| p.label == "A = 8") {
            saw_a = true;
        }
        if f.packets.iter().any(|p| p.label == "B = 19") {
            saw_b = true;
        }
        if f.texts.iter().any(|t| t.text.contains("mod 23 = 2 · key")) {
            saw_key = true;
        }
        if f.texts.iter().any(|t| t.text.contains("infeasible at real sizes")) {
            saw_fail = true;
        }
        t += 200;
    }
    assert!(saw_a && saw_b && saw_key && saw_fail, "dh: shares fly, key derived, crack fails");
    assert!(dh.frame(dh.duration() + 50).note.contains("g = 5 and p = 23"), "dh loops");

    // routerhop
    let rh = get_sim("routerhop");
    assert!(rh.frame(0).note.contains("ttl 58"), "routerhop: packet 1 arrives");
    let mut saw = [false; 4];
    let mut t = 0;
    while t < rh.duration() {
        let n = rh.frame(t).note;
        if n.contains("Forwarded out line 3") {
            saw[0] = true;
        }
        if n.contains("Forwarded out line 2") {
            saw[1] = true;
        }
        if n.contains("Forwarded out line 5") {
            saw[2] = true;
        }
        if n.contains("Forwarded out line 1") {
            saw[3] = true;
        }
        t += 150;
    }
    assert!(saw.iter().all(|&x| x), "routerhop: all 4 packets routed to their winning lines");
    assert!(rh.frame(rh.duration() + 50).note.contains("ttl 58"), "routerhop loops");

    // switchlearn
    let sw = get_sim("switchlearn");
    assert!(sw.frame(0).note.contains("Frame arrives on port 1"), "switchlearn: ep0 arriving");
    let mut saw_flood = false;
    let mut saw_direct = false;
    let mut t = 0;
    while t < sw.duration() {
        let f = sw.frame(t);
        if f.note.contains("Flooded out every port") {
            saw_flood = true;
        }
        if f.note.contains("Forwarded out port 4 only") || f.note.contains("Forwarded out port 2 only") {
            saw_direct = true;
        }
        t += 150;
    }
    assert!(saw_flood && saw_direct, "switchlearn: flood + direct forwards happen");
    assert!(sw.frame(sw.duration() + 50).note.contains("Frame arrives on port 1"), "switchlearn loops");

    // mtu
    let mtu = get_sim("mtu");
    assert!(mtu.frame(0).note.contains("3000 byte packet"), "mtu: fragment scenario starts");
    let mut saw_split = false;
    let mut saw_icmp = false;
    let mut saw_reassemble = false;
    let mut t = 0;
    while t < mtu.duration() {
        let f = mtu.frame(t);
        if f.note.contains("slices it into two fragments") {
            saw_split = true;
        }
        if f.packets.iter().any(|p| p.label == "ICMP") {
            saw_icmp = true;
        }
        if f.note.contains("reassembles both fragments") {
            saw_reassemble = true;
        }
        t += 150;
    }
    assert!(saw_split && saw_icmp && saw_reassemble, "mtu: split + ICMP + reassemble");
    assert!(mtu.frame(mtu.duration() + 50).note.contains("3000 byte packet"), "mtu loops");

    // tls
    let tls = get_sim("tls");
    assert!(tls.frame(0).note.contains("ClientHello"), "tls: hello stage");
    let mut saw_cipher = false;
    let mut saw_trail = false;
    let mut t = 0;
    while t < tls.duration() {
        let f = tls.frame(t);
        if f.packets.iter().any(|p| p.label.contains(':') && !p.label.contains("Hello")) {
            saw_cipher = true;
        }
        if f.trails.len() >= 5 {
            saw_trail = true;
        }
        t += 200;
    }
    assert!(saw_cipher && saw_trail, "tls: encrypted messages fly + trails accumulate");
    assert!(tls.frame(tls.duration() + 50).note.contains("ClientHello"), "tls loops");

    // tcpsim
    let ts = get_sim("tcpsim");
    assert!(ts.frame(0).note.contains("fills the window"), "tcpsim: new message");
    let mid = ts.frame(3000);
    assert!(mid.note.contains("Packet 3 is lost"), "tcpsim: loss noticed");
    let dup = ts.frame(8800);
    assert!(dup.note.contains("3 duplicate ACKs"), "tcpsim: dup-ack resend");
    assert!(ts.frame(7000).packets.iter().any(|p| p.label == "ack 2"), "tcpsim: acks flowing");
    let done = ts.frame(20800);
    assert!(done.note.contains("Whole message delivered in order"), "tcpsim: done");
    assert!(ts.frame(ts.duration() + 50).note.contains("fills the window"), "tcpsim loops");

    // igp
    let igp = get_sim("igp");
    assert!(igp.frame(0).note.contains("Steady state"), "igp: steady state");
    let mut saw_flood = false;
    let mut saw_loop = false;
    let mut saw_ospf_conv = false;
    let mut saw_rip_conv = false;
    let mut t = 0;
    while t < igp.duration() {
        let f = igp.frame(t);
        if f.packets.iter().any(|p| p.label == "flood") {
            saw_flood = true;
        }
        if f.packets.iter().any(|p| p.label.contains("ttl")) {
            saw_loop = true;
        }
        if f.note == "Converged, two floods, every router certain." {
            saw_ospf_conv = true;
        }
        if f.note == "The news reaches R1 the same way every number here traveled, one hop at a time." {
            saw_rip_conv = true;
        }
        t += 150;
    }
    assert!(saw_flood && saw_loop && saw_ospf_conv && saw_rip_conv, "igp: both stories play");
    assert!(igp.frame(igp.duration() + 50).note.contains("Steady state"), "igp loops");

    // wdm
    let wdm = get_sim("wdm");
    assert!(wdm.frame(0).note.contains("Each laser keys"), "wdm: start caption");
    let mut saw_combined = false;
    let mut saw_out = false;
    let mut t = 0;
    while t < wdm.duration() {
        let f = wdm.frame(t);
        if f.polylines.len() == 4 {
            saw_combined = true;
        }
        if f.polylines.len() >= 6 {
            saw_out = true;
        }
        t += 300;
    }
    assert!(saw_combined && saw_out, "wdm: combined waveform + separated outputs");
    assert!(wdm.frame(wdm.duration() - 100).note.contains("delivered intact"), "wdm: done caption");

    // nat — table entries probed (schedule is data now)
    let nat = get_sim("nat");
    assert!(nat.frame(0).note.contains("Both laptops send at once"), "nat: phase 1 caption");
    let _ = find_frame(&*nat, |f| f.nodes.get(2).and_then(|n| n.status.as_deref()).is_some_and(|s| s.contains("203.0.113.7:40001")));
    let two = find_frame(&*nat, |f| f.packets.len() == 2);
    assert!(two.packets.len() == 2, "nat: both laptops in flight");
    let _ = find_frame(&*nat, |f| f.note.contains("phone in home A"));
    let _ = find_frame(&*nat, |f| f.nodes.get(2).and_then(|n| n.status.as_deref()).is_some_and(|s| s.contains("50307")));
    assert!(nat.frame(nat.duration() + 50).note.contains("Both laptops"), "nat loops");

    // bandwidth
    let bw = get_sim("bandwidth");
    assert!(bw.frame(0).note.len() > 0, "bandwidth: renders");
    let mut saw_tx = false;
    let mut saw_done = false;
    let mut t = 0;
    while t < bw.duration() {
        let n = bw.frame(t).note;
        if n.contains("keying the bit") {
            saw_tx = true;
        }
        if n.contains("fully received") {
            saw_done = true;
        }
        t += 100;
    }
    assert!(saw_tx && saw_done, "bandwidth: tx + done phases");
    let bwlen = bw.frame(0).polylines[0].points.len();
    assert!((190..=212).contains(&bwlen), "bandwidth: waveform samples (got {bwlen})");

    // linkclick
    let lc = get_sim("linkclick");
    assert!(lc.frame(0).note.contains("the click"), "linkclick: stage 0");
    assert!(lc.frame(4000).note.contains("DNS"), "linkclick: dns stage");
    assert!(lc.frame(10000).packets.iter().any(|pk| pk.label == "SYN"), "linkclick: tcp syn flying");
    assert!(lc.frame(45000).note.contains("the page builds"), "linkclick: build stage");
    let dns_end = lc.frame(9500);
    assert!(dns_end.note.contains("5.9 ms") || dns_end.note.contains("6.0 ms"), "linkclick: dns maps to ~6 ms");
    assert!(lc.frame(lc.duration() + 50).note.contains("the click"), "linkclick loops");

    // msgjourney
    let mj = get_sim("msgjourney");
    assert!(mj.frame(0).note.contains("another continent"), "msgjourney: premise");
    let room = mj.frame(4500);
    assert!(room.note.contains("radio wave"), "msgjourney: room chapter");
    assert!(!room.polylines.is_empty(), "msgjourney: radio waveform drawn");
    let ocean = mj.frame(4200 + 3400 + 3800 + 4400 + 1000);
    assert!(ocean.note.contains("6,600 km"), "msgjourney: ocean chapter");
    assert!(ocean.paths.len() == 1, "msgjourney: map shown");
    assert!(mj.frame(mj.duration() - 100).note.contains("None of the companies"), "msgjourney: summary");
    assert!(mj.frame(mj.duration() + 50).note.contains("another continent"), "msgjourney loops");

    // netsim — deterministic graph (sorted edge iteration), storm probes
    let ns = get_sim("netsim");
    let n0 = ns.frame(0);
    assert!(n0.nodes.len() == 16 && n0.trails.len() > 20, "netsim: graph built (12 routers + 2 clients + 2 servers)");
    let mut saw_pkt = false;
    let mut saw_delivered = false;
    let mut t = 0;
    while t < 30000 {
        let f = ns.frame(t);
        if !f.packets.is_empty() {
            saw_pkt = true;
        }
        if f.texts[0].text.contains("delivered: ") && !f.texts[0].text.contains("delivered: 0   dropped: 0") {
            saw_delivered = true;
        }
        t += 800;
    }
    assert!(saw_pkt && saw_delivered, "netsim: packets flow and deliver");
    assert!(ns.frame(5000).packets.len() <= 12, "netsim: bounded packets in flight");

    // telegraph
    let tg = get_sim("telegraph");
    assert!(tg.frame(0).note.contains("electromechanical relay"), "telegraph: regen mode first");
    let amp_t = tg.duration() / 2 + 100;
    assert!(tg.frame(amp_t).note.contains("analog amplifier"), "telegraph: amp mode second half");
    assert!(tg.frame(0).polylines[0].points.len() > 100, "telegraph: waveform samples");
    assert!(tg.frame(tg.duration() + 50).note.contains("electromechanical relay"), "telegraph loops");

    // quic
    let quic = get_sim("quic");
    assert!(quic.frame(0).note.contains("TCP first"), "quic: ch1 tcp lane");
    assert!(quic.frame(0).badge.as_deref().unwrap().contains("single handshake"), "quic: ch1 quic lane");
    assert!(quic.frame(12000).note.contains("Two full round trips"), "quic: tcp 2-rtt");
    let hol = quic.frame(17600 + 7200);
    assert!(hol.note.contains("stalls only stream B") || hol.badge.as_deref().unwrap().contains("stalls only stream B"), "quic: ch2 hol contrast");
    assert!(quic.frame(29950 + 9900).badge.as_deref().unwrap().contains("never breaks"), "quic: ch3 migration");
    assert!(quic.frame(29950 + 7500).note.contains("download is dead"), "quic: ch3 tcp dies");
    assert!(quic.frame(quic.duration() + 50).note.contains("TCP first"), "quic loops");

    // dns
    let dns = get_sim("dns");
    assert!(dns.frame(0).note.contains("One lookup walks the chain"), "dns: intro note");
    let mut saw_root = false;
    let mut saw_wiki = false;
    let mut saw_hit = false;
    let mut saw_ttl0 = false;
    let mut t = 0;
    while t < dns.duration() {
        let f = dns.frame(t);
        if f.note.contains("who runs .org") {
            saw_root = true;
        }
        if f.note.contains("holds the actual record") {
            saw_wiki = true;
        }
        if f.note.contains("never leaves your machine") {
            saw_hit = true;
        }
        if f.note.contains("TTL zero") {
            saw_ttl0 = true;
        }
        t += 250;
    }
    assert!(saw_root && saw_wiki && saw_hit && saw_ttl0, "dns: full story + ttl expiry");
    assert!(dns.frame(dns.duration() + 50).note.contains("One lookup walks"), "dns loops");

    // animation pattern verbs — choreography compiled to the same keyframes
    let mk = |anim: &str, body: &str| -> timeline::Doc {
        let json = format!("{{\"duration\":10000,\"anim\":\"{anim}\",{body}}}");
        spec::parse(&json).unwrap_or_else(|e| panic!("pattern {anim}: {e}"))
    };
    let near = |a: f64, b: f64| (a - b).abs() < 0.02;

    // seq: one at a time — 3 relay slots over 10s
    let d = mk("seq", "\"nodes\":[{\"id\":\"a\",\"x\":10,\"y\":50},{\"id\":\"b\",\"x\":50,\"y\":50},{\"id\":\"c\",\"x\":90,\"y\":50}],\"packets\":[{\"label\":\"1\",\"from\":\"a\",\"to\":\"b\"},{\"label\":\"2\",\"from\":\"b\",\"to\":\"c\"},{\"label\":\"3\",\"from\":\"c\",\"to\":\"a\"}]");
    let f = d.frame_at(1600);
    assert!(near(f.packets[0].p, 0.506) && f.packets[1].p == 0.0 && f.packets[2].p == 0.0, "seq: only packet 1 flying at t=1600 ({})", f.packets[0].p);
    let f = d.frame_at(5000);
    assert!(f.packets[0].p == 1.0 && near(f.packets[1].p, 0.61) && f.packets[2].p == 0.0, "seq: relay at t=5000");
    let f = d.frame_at(9500);
    assert!(f.packets.iter().all(|p| p.p == 1.0), "seq: all landed by t=9500");

    // fanout: halves — first half out (ripple), second half back
    let d = mk("fanout", "\"nodes\":[{\"id\":\"a\",\"x\":10,\"y\":50},{\"id\":\"b\",\"x\":50,\"y\":50},{\"id\":\"c\",\"x\":90,\"y\":30},{\"id\":\"d\",\"x\":90,\"y\":70}],\"packets\":[{\"label\":\"req\",\"from\":\"a\",\"to\":\"b\"},{\"label\":\"fwd\",\"from\":\"b\",\"to\":\"c\"},{\"label\":\"resp\",\"from\":\"c\",\"to\":\"b\"},{\"label\":\"ok\",\"from\":\"b\",\"to\":\"a\"}]");
    let f = d.frame_at(800);
    assert!(near(f.packets[0].p, 0.075) && f.packets[2].p == 0.0, "fanout: outbound only at t=800");
    let f = d.frame_at(7000);
    assert!(f.packets[0].p == 1.0 && near(f.packets[2].p, 0.5) && f.packets[2].x1 == 90.0, "fanout: return leg at t=7000");

    // flood: one simultaneous block
    let d = mk("flood", "\"nodes\":[{\"id\":\"a\",\"x\":10,\"y\":50},{\"id\":\"b\",\"x\":90,\"y\":20},{\"id\":\"c\",\"x\":90,\"y\":50},{\"id\":\"d\",\"x\":90,\"y\":80}],\"packets\":[{\"label\":\"q\",\"from\":\"a\",\"to\":\"b\"},{\"label\":\"q\",\"from\":\"a\",\"to\":\"c\"},{\"label\":\"q\",\"from\":\"a\",\"to\":\"d\"}]");
    let f = d.frame_at(2000);
    assert!(f.packets.iter().all(|p| p.p > 0.0 && p.p < 1.0), "flood: everything in flight at t=2000");

    // flip: statuses reveal in node order — state spreads through the system
    let d = mk("flip", "\"nodes\":[{\"id\":\"a\",\"x\":15,\"y\":50,\"status\":\"s1\"},{\"id\":\"b\",\"x\":50,\"y\":50,\"status\":\"s2\"},{\"id\":\"c\",\"x\":85,\"y\":50,\"status\":\"s3\"}],\"packets\":[]");
    let f = d.frame_at(2000);
    assert!(f.nodes[0].status.as_deref() == Some("s1") && f.nodes[1].status.is_none() && f.nodes[2].status.is_none(), "flip: only node 1 revealed at t=2000");
    let f = d.frame_at(5000);
    assert!(f.nodes[1].status.as_deref() == Some("s2") && f.nodes[2].status.is_none(), "flip: node 2 at t=5000");
    let f = d.frame_at(9000);
    assert!(f.nodes.iter().all(|n| n.status.is_some()), "flip: all revealed by t=9000");

    // pattern errors
    assert!(spec::parse("{\"anim\":\"bogus\",\"duration\":1000}").unwrap_err().contains("unknown anim"), "anim: unknown verb rejected");
    assert!(spec::parse("{\"anim\":\"seq\",\"nodes\":[]}").unwrap_err().contains("requires"), "anim: duration required");

    // unified node layout: dense columns flip label/status to the side,
    // sparse scenes keep the classic below-stack — and no label/badge/glyph
    // rects may intersect in EITHER case
    {
        use svg::{plan_node_layout, NodeLayout};
        let dense: Vec<crate::frame::NodeSpec> = (0..3)
            .map(|i| crate::frame::NodeSpec {
                label: format!("svc{i}"),
                x: 88.0,
                y: [26.0, 50.0, 74.0][i],
                status: Some(format!("200 OK {i}")),
                lifeline: false,
                icon: Some("terminal".into()),
            })
            .collect();
        let plans = plan_node_layout(&dense);
        assert!(plans[0] != NodeLayout::Below && plans[1] != NodeLayout::Below, "dense column flips to side {:?}", plans);
        // geometry: label+badge rects must not hit any glyph rect
        let rects = |n: &crate::frame::NodeSpec, p: NodeLayout| -> Vec<(f64, f64, f64, f64)> {
            let (x, y) = (svg::sx(n.x), svg::sy(n.y));
            let glyph = (x - 17.0, y - 17.0, x + 17.0, y + 17.0);
            match p {
                NodeLayout::Below => {
                    let mut v = vec![glyph];
                    if !n.label.is_empty() {
                        v.push((x - 40.0, y + 18.0, x + 40.0, y + 32.0));
                    }
                    if let Some(s) = &n.status {
                        let bw = s.chars().count() as f64 * 6.0 + 16.4;
                        v.push((x - bw / 2.0, y + 40.0, x + bw / 2.0, y + 60.0));
                    }
                    v
                }
                _ => {
                    let mut v = vec![glyph];
                    let tx = if p == NodeLayout::SideLeft { x - 25.0 } else { x + 25.0 };
                    let (lx0, lx1) = if p == NodeLayout::SideLeft { (tx - 60.0, tx) } else { (tx, tx + 60.0) };
                    if !n.label.is_empty() {
                        v.push((lx0, y - 12.0, lx1, y + 2.0));
                    }
                    if let Some(s) = &n.status {
                        let bw = s.chars().count() as f64 * 6.0 + 16.4;
                        let bx = if p == NodeLayout::SideLeft { tx - bw } else { tx };
                        v.push((bx, y + 8.0, bx + bw, y + 28.0));
                    }
                    v
                }
            }
        };
        let mut all: Vec<(f64, f64, f64, f64)> = Vec::new();
        for (n, p) in dense.iter().zip(&plans) {
            all.extend(rects(n, *p));
        }
        let hit = |a: (f64, f64, f64, f64), b: (f64, f64, f64, f64)| a.0 < b.2 && b.0 < a.2 && a.1 < b.3 && b.1 < a.3;
        for i in 0..all.len() {
            for j in i + 1..all.len() {
                assert!(!hit(all[i], all[j]), "dense layout: rects {:?} and {:?} overlap", all[i], all[j]);
            }
        }
        // sparse scene keeps the classic look (regression guard for the seeds)
        let sparse = vec![
            crate::frame::NodeSpec { label: "a".into(), x: 10.0, y: 50.0, status: None, lifeline: false, icon: Some("dns".into()) },
            crate::frame::NodeSpec { label: "b".into(), x: 90.0, y: 50.0, status: Some("up".into()), lifeline: false, icon: Some("dns".into()) },
        ];
        assert!(plan_node_layout(&sparse) == vec![NodeLayout::Below, NodeLayout::Below], "sparse stays below");
        // note wrapping: long captions wrap to <=3 lines, ellipsized not clipped
        let long = "word ".repeat(60);
        assert!(svg::wrap(&long, 92).len() <= 3, "note wraps to <=3 lines");
    }

    println!("check ok");
}

// rasterize SVG -> PNG in-process via resvg (3x supersampling so the terminal's
// downscale stays crisp). No subprocess, no spawn tax (~70ms -> ~5ms).
// embedded site fonts: the raster output is identical on every machine
const FONT_INCONSOLATA: &[u8] = include_bytes!("assets/fonts/Inconsolata.ttf");
const FONT_LIBERATION: &[u8] = include_bytes!("assets/fonts/LiberationSans.ttf");

// parse the fontdb once for the whole process (was rebuilt every frame: ~2MB churn)
fn fontdb() -> std::sync::Arc<resvg::usvg::fontdb::Database> {
    static DB: std::sync::OnceLock<std::sync::Arc<resvg::usvg::fontdb::Database>> = std::sync::OnceLock::new();
    DB.get_or_init(|| {
        // ponytail: resvg renders Adobe/Google "Source Sans 3" TTFs blank/broken
        // (fontdb bug); Liberation Sans is the proven-good embedded sans.
        let mut db = resvg::usvg::fontdb::Database::new();
        db.load_font_data(FONT_INCONSOLATA.to_vec());
        db.load_font_data(FONT_LIBERATION.to_vec());
        std::sync::Arc::new(db)
    })
    .clone()
}

// one pixmap for the process lifetime (was 8.6MB alloc/free per frame);
// render + encode happen inside the lock, no lifetime gymnastics
fn shared_render(tree: &resvg::usvg::Tree, w: u32, h: u32, scale: f32) -> std::io::Result<Vec<u8>> {
    static PM: std::sync::Mutex<Option<resvg::tiny_skia::Pixmap>> = std::sync::Mutex::new(None);
    let mut guard = PM.lock().unwrap();
    let need = guard.as_ref().map(|p| p.width() != w || p.height() != h).unwrap_or(true);
    if need {
        *guard = resvg::tiny_skia::Pixmap::new(w, h);
    }
    let pixmap = guard.as_mut().unwrap();
    let transform = resvg::tiny_skia::Transform::from_scale(scale, scale);
    let mut pmref = pixmap.as_mut();
    resvg::render(tree, transform, &mut pmref);
    pixmap.encode_png().map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, format!("png: {e}")))
}

fn raster(svg_text: &str) -> std::io::Result<Vec<u8>> {
    let opts = resvg::usvg::Options { fontdb: fontdb(), ..Default::default() };
    let tree = resvg::usvg::Tree::from_str(svg_text, &opts)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, format!("svg parse: {e}")))?;
    let size = tree.size();
    let scale: f32 = std::env::var("DDA_SCALE").ok().and_then(|v| v.parse().ok()).unwrap_or(3.0);
    let w = (size.width() * scale).ceil() as u32;
    let h = (size.height() * scale).ceil() as u32;
    shared_render(&tree, w, h, scale)
}

// terminal (cols, rows) via stty on the controlling tty; fallback 80x24
fn term_size() -> (u32, u32) {
    if let Ok(out) = Command::new("stty").arg("size").stdin(Stdio::inherit()).stdout(Stdio::piped()).output() {
        if let Ok(s) = String::from_utf8(out.stdout) {
            let mut it = s.split_whitespace();
            if let (Some(Ok(r)), Some(Ok(c))) = (it.next().map(str::parse), it.next().map(str::parse)) {
                return (c, r);
            }
        }
    }
    (80, 24)
}

fn kitty_mode(sim: Box<dyn Sim>) {
    let (term_cols, term_rows) = term_size();
    let cols = term_cols.saturating_sub(2);
    // terminal cells are ~1:2 (w:h) — pick rows that preserve the image aspect ratio
    let rows = term_rows
        .saturating_sub(2)
        .min((cols as f64 * 0.5 * (svg::H / svg::W)).round().max(8.0) as u32);

    let quit = Arc::new(AtomicBool::new(false));
    signal_hook::flag::register(signal_hook::consts::SIGINT, Arc::clone(&quit)).unwrap();

    let out = std::io::stdout();
    let mut out = out.lock();
    out.write_all(b"\x1b[?25l").unwrap(); // hide cursor
    let t0 = Instant::now();
    while !quit.load(Ordering::Relaxed) {
        let t = t0.elapsed().as_millis() as u64;
        match raster(&svg::render_svg(&sim.frame(t))) {
            Ok(png) => {
                out.write_all(format!("\x1b[H{}", kitty::kitty_png(&png, cols, rows)).as_bytes()).unwrap();
                out.flush().unwrap();
            }
            Err(e) => {
                eprintln!("raster failed: {e}");
                break;
            }
        }
        // 30fps target; in-process resvg leaves plenty of headroom
        let spent = t0.elapsed().as_millis() as u64 - t;
        if spent < 33 {
            std::thread::sleep(Duration::from_millis(33 - spent));
        }
    }
    out.write_all(format!("{}\x1b[?25h", kitty::KITTY_CLEAR).as_bytes()).unwrap();
}

const INDEX_HTML: &str = r#"<!doctype html>
<meta charset="utf-8">
<title>TCP — the three-way handshake</title>
<body style="margin:0;background:#fafafa;display:grid;place-items:center;min-height:100vh">
<img id="sim" style="width:min(760px,96vw);border:1px solid #e5e7eb;border-radius:8px;box-shadow:0 1px 4px rgb(0 0 0/.06)">
<script>
const img = document.getElementById('sim');
const t0 = Date.now();
setInterval(() => { img.src = '/frame.svg?t=' + (Date.now() - t0); }, 100);
</script>
"#;

fn respond(mut stream: std::net::TcpStream, status: &str, content_type: &str, body: &[u8]) {
    let head = format!(
        "HTTP/1.1 {status}\r\ncontent-type: {content_type}\r\ncache-control: no-store\r\ncontent-length: {}\r\n\r\n",
        body.len()
    );
    let _ = stream.write_all(head.as_bytes());
    let _ = stream.write_all(body);
}

// zero-dep static+frame server (sequential; each response is instant)
fn web_mode(sim: Box<dyn Sim>) {
    let listener = std::net::TcpListener::bind("127.0.0.1:8080").unwrap();
    println!("open http://localhost:8080  (Ctrl+C to stop)");
    for stream in listener.incoming() {
        let mut stream = match stream {
            Ok(s) => s,
            Err(_) => continue,
        };
        let mut buf = [0u8; 4096];
        let n = match stream.read(&mut buf) {
            Ok(0) | Err(_) => continue,
            Ok(n) => n,
        };
        let req = String::from_utf8_lossy(&buf[..n]);
        let path = req.lines().next().unwrap_or("").split_whitespace().nth(1).unwrap_or("/");
        if path == "/" {
            respond(stream, "200 OK", "text/html", INDEX_HTML.as_bytes());
        } else if path.starts_with("/frame.svg") {
            let t = path
                .split("t=")
                .nth(1)
                .and_then(|s| s.parse::<u64>().ok())
                .unwrap_or(0);
            respond(stream, "200 OK", "image/svg+xml", svg::render_svg(&sim.frame(t)).as_bytes());
        } else {
            respond(stream, "404 Not Found", "text/plain", b"not found");
        }
    }
}
