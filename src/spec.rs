//! The v1 spec sugar: the compact AI/human-facing format (nodes with ids,
//! links by id, packets with from/to + window). Compiled into the universal
//! timeline Doc — `window` is literally two keyframes on travel progress.

use crate::frame::*;
use crate::timeline::*;
use serde::Deserialize;
use std::collections::HashMap;

/// ```json
/// {
///   "header": "MY NETWORK ·· OVERVIEW",
///   "duration": 6000,
///   "nodes": [ { "id": "client", "label": "laptop", "icon": "phone", "x": 10, "y": 50 } ],
///   "links": [ { "from": "client", "to": "srv", "arrow": "end" } ],
///   "packets": [ { "label": "GET /api", "from": "client", "to": "srv", "window": [0.1, 0.7] } ],
///   "texts": [ { "text": "dmz", "x": 60, "y": 20, "dim": true } ],
///   "badge": "live", "note": "caption"
/// }
/// ```
#[derive(Deserialize)]
struct SpecV1 {
    duration: Option<u64>,
    /// animation pattern verb: choreography compiler for the packet/status
    /// timelines ("seq" | "fanout" | "flood" | "flip"). Requires duration.
    anim: Option<String>,
    header: Option<String>,
    #[serde(default)]
    nodes: Vec<NodeD>,
    #[serde(default)]
    links: Vec<LinkD>,
    #[serde(default)]
    packets: Vec<PacketD>,
    #[serde(default)]
    texts: Vec<TextD>,
    badge: Option<String>,
    note: Option<String>,
}

#[derive(Deserialize)]
struct NodeD {
    pub id: Option<String>,
    pub label: Option<String>,
    pub icon: Option<String>,
    pub x: f64,
    pub y: f64,
    pub status: Option<String>,
    #[serde(default)]
    pub lifeline: bool,
}

#[derive(Deserialize)]
struct LinkD {
    from: String,
    to: String,
    #[serde(default)]
    arrow: String,
}

#[derive(Deserialize)]
struct PacketD {
    label: String,
    from: String,
    to: String,
    #[serde(default)]
    progress: Option<f64>,
    #[serde(default)]
    faded: bool,
    #[serde(default)]
    window: Option<(f64, f64)>,
}

#[derive(Deserialize)]
struct TextD {
    text: String,
    x: f64,
    y: f64,
    #[serde(default)]
    dim: bool,
    #[serde(default)]
    left: bool,
}

/// pattern verbs — one-word choreography, compiled to the same keyframes a
/// hand-written window list would produce (picking an animation is now as
/// cheap as picking an icon; custom stories fall through to explicit
/// windows/keyframes)
#[derive(Clone, Copy, PartialEq)]
enum Anim {
    /// strict relay: packets fly one at a time, each in its own slot
    Seq,
    /// out-and-back halves: first half of the list flies out (slight ripple),
    /// second half flies back — fan-out/fan-in, request/response
    Fanout,
    /// single simultaneous block — broadcast look
    Flood,
    /// status reveal relay: each node's status appears in sequence (state
    /// spreads through the system); packets keep the default stagger
    Flip,
}

fn parse_anim(s: &str) -> Result<Option<Anim>, String> {
    Ok(match s {
        "" => None,
        "seq" => Some(Anim::Seq),
        "fanout" => Some(Anim::Fanout),
        "flood" => Some(Anim::Flood),
        "flip" => Some(Anim::Flip),
        other => return Err(format!("unknown anim {other:?} (seq | fanout | flood | flip)")),
    })
}

/// choreography: packet index -> [w0, w1] loop fractions
fn pattern_window(anim: Anim, i: usize, n: usize) -> (f64, f64) {
    let i = i as f64;
    let n = n as f64;
    match anim {
        Anim::Seq => {
            let slot = 0.94 / n;
            let w0 = 0.03 + i * slot;
            (w0, w0 + slot * 0.82)
        }
        Anim::Fanout => {
            let out = (n / 2.0).ceil();
            if i < out {
                (0.05 + (i * 0.05).min(0.25), 0.45)
            } else {
                let j = i - out;
                (0.5 + (j * 0.05).min(0.25), 0.9)
            }
        }
        Anim::Flood => (0.08 + (i * 0.02).min(0.06), 0.58),
        Anim::Flip => ((0.06 + i * 0.06).min(0.5), (0.55 + i * 0.06).min(0.92)), // default stagger
    }
}

pub fn parse(json: &str) -> Result<Doc, String> {
    let spec: SpecV1 = serde_json::from_str(json).map_err(|e| format!("spec parse: {e}"))?;
    let anim = match spec.anim.as_deref() {
        None => None,
        Some(a) => {
            let v = parse_anim(a)?;
            if v.is_some() && spec.duration.is_none() {
                return Err(format!("anim {a:?} requires \"duration\" (ms)"));
            }
            v
        }
    };
    let mut pos: HashMap<String, (f64, f64)> = HashMap::new();
    let mut els = Vec::new();
    let flip_targets: Vec<Option<String>> = spec
        .nodes
        .iter()
        .map(|n| n.status.clone())
        .collect();
    let n_flip = flip_targets.iter().flatten().count().max(1);
    for (i, n) in spec.nodes.iter().enumerate() {
        let id = n.id.clone().unwrap_or_else(|| i.to_string());
        pos.insert(id.clone(), (n.x, n.y));
        // flip: statuses reveal left-to-right in node order — the story of
        // state spreading through the system ("" timeline key = absent)
        let status = if anim == Some(Anim::Flip) {
            flip_targets[i].clone().map(|s| {
                let k = flip_targets[..i].iter().flatten().count();
                let t = ((0.12 + 0.76 * k as f64 / n_flip as f64) * spec.duration.unwrap() as f64) as u64;
                Tl::Keyed(vec![(0, String::new()), (t, s)])
            })
        } else {
            n.status.clone().map(Tl::Const)
        };
        els.push(El::Node(NodeEl {
            id: Some(id),
            label: n.label.clone().map(Tl::Const),
            icon: n.icon.clone(),
            x: Tl::Const(n.x),
            y: Tl::Const(n.y),
            status,
            lifeline: n.lifeline,
            show: None,
        }));
    }
    let get = |k: &str| -> Result<(f64, f64), String> { pos.get(k).copied().ok_or_else(|| format!("unknown node {k:?}")) };
    for l in &spec.links {
        get(&l.from)?;
        get(&l.to)?;
        els.push(El::Line(LineEl {
            from: Some(l.from.clone()),
            to: Some(l.to.clone()),
            x1: None,
            y1: None,
            x2: None,
            y2: None,
            arrow_end: l.arrow == "end" || l.arrow == "both",
            show: None,
        }));
        if l.arrow == "both" {
            els.push(El::Line(LineEl {
                from: Some(l.to.clone()),
                to: Some(l.from.clone()),
                x1: None,
                y1: None,
                x2: None,
                y2: None,
                arrow_end: true,
                show: None,
            }));
        }
    }
    for (i, p) in spec.packets.iter().enumerate() {
        let (a, b) = (get(&p.from)?, get(&p.to)?);
        let npk = spec.packets.len();
        let p_tl = match (anim, spec.duration, p.window) {
            (Some(a), Some(d), _) => {
                let (w0, w1) = pattern_window(a, i, npk);
                Tl::Keyed(vec![
                    (0, 0.0),
                    ((w0 * d as f64) as u64, 0.0),
                    ((w1 * d as f64) as u64, 1.0),
                    (d, 1.0),
                ])
            }
            (None, Some(d), Some((w0, w1))) => {
                let (w0, w1) = (w0.clamp(0.0, 1.0), w1.clamp(0.0, 1.0));
                Tl::Keyed(vec![
                    (0, 0.0),
                    ((w0 * d as f64) as u64, 0.0),
                    ((w1 * d as f64) as u64, 1.0),
                    (d, 1.0),
                ])
            }
            (None, Some(d), None) => {
                // staggered default windows, matching the original semantics
                let w0 = (0.06 + i as f64 * 0.06).min(0.5);
                let w1 = (0.55 + i as f64 * 0.06).min(0.92);
                Tl::Keyed(vec![
                    (0, 0.0),
                    ((w0 * d as f64) as u64, 0.0),
                    ((w1 * d as f64) as u64, 1.0),
                    (d, 1.0),
                ])
            }
            (Some(_), None, _) => unreachable!("anim requires duration (validated above)"),
            (None, None, _) => Tl::Const(p.progress.unwrap_or(0.5).clamp(0.0, 1.0)),
        };
        els.push(El::Packet(PacketEl {
            label: p.label.clone(),
            x1: Tl::Const(a.0),
            y1: Tl::Const(a.1),
            x2: Tl::Const(b.0),
            y2: Tl::Const(b.1),
            p: p_tl,
            landed: Tl::Const(p.faded),
            show: None,
        }));
    }
    for t in &spec.texts {
        els.push(El::Text(TextEl {
            text: Tl::Const(t.text.clone()),
            x: Tl::Const(t.x),
            y: Tl::Const(t.y),
            dim: Tl::Const(t.dim),
            left: t.left,
            show: None,
        }));
    }
    Ok(Doc {
        duration: spec.duration,
        header: spec.header.map(Tl::Const),
        badge: spec.badge.map(Tl::Const),
        note: spec.note.map(Tl::Const),
        els,
    })
}

#[allow(dead_code)]
fn _frame_compat(_: &Frame) {}
