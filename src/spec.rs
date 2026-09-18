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

pub fn parse(json: &str) -> Result<Doc, String> {
    let spec: SpecV1 = serde_json::from_str(json).map_err(|e| format!("spec parse: {e}"))?;
    let mut pos: HashMap<String, (f64, f64)> = HashMap::new();
    let mut els = Vec::new();
    for (i, n) in spec.nodes.iter().enumerate() {
        let id = n.id.clone().unwrap_or_else(|| i.to_string());
        pos.insert(id.clone(), (n.x, n.y));
        els.push(El::Node(NodeEl {
            id: Some(id),
            label: n.label.clone().map(Tl::Const),
            icon: n.icon.clone(),
            x: Tl::Const(n.x),
            y: Tl::Const(n.y),
            status: n.status.clone().map(Tl::Const),
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
        let p_tl = match (spec.duration, p.window) {
            (Some(d), Some((w0, w1))) => {
                let (w0, w1) = (w0.clamp(0.0, 1.0), w1.clamp(0.0, 1.0));
                Tl::Keyed(vec![
                    (0, 0.0),
                    ((w0 * d as f64) as u64, 0.0),
                    ((w1 * d as f64) as u64, 1.0),
                    (d, 1.0),
                ])
            }
            (Some(_), None) => {
                // staggered default windows, matching the original semantics
                let w0 = (0.06 + i as f64 * 0.06).min(0.5);
                let w1 = (0.55 + i as f64 * 0.06).min(0.92);
                let d = spec.duration.unwrap();
                Tl::Keyed(vec![
                    (0, 0.0),
                    ((w0 * d as f64) as u64, 0.0),
                    ((w1 * d as f64) as u64, 1.0),
                    (d, 1.0),
                ])
            }
            (None, _) => Tl::Const(p.progress.unwrap_or(0.5).clamp(0.0, 1.0)),
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
