//! The declarative spec format: a JSON document describing a diagram —
//! nodes with icon kinds, links with arrows, packets, texts — parsed into a
//! Frame and rendered by the same pipeline as the sims.
//!
//! Static spec (no "duration"): packets sit at their "progress".
//! Animated spec ("duration": 6000): packets fly from→to inside their time
//! window each loop; sample with frame_at(t).
//!
//! ```json
//! {
//!   "header": "MY NETWORK ·· OVERVIEW",
//!   "duration": 6000,
//!   "nodes": [
//!     { "id": "client", "label": "laptop", "icon": "phone", "x": 10, "y": 50 },
//!     { "id": "srv", "label": "app", "icon": "server", "x": 85, "y": 50, "status": "10.0.0.5" }
//!   ],
//!   "links": [ { "from": "client", "to": "srv", "arrow": "end" } ],
//!   "packets": [ { "label": "GET /api", "from": "client", "to": "srv",
//!                  "window": [0.1, 0.7] } ],
//!   "texts": [ { "text": "dmz", "x": 60, "y": 20, "dim": true } ],
//!   "badge": "live",
//!   "note": "caption"
//! }
//! ```

use crate::frame::*;
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Deserialize)]
pub struct Spec {
    pub duration: Option<u64>,
    pub header: Option<String>,
    #[serde(default)]
    pub nodes: Vec<NodeD>,
    #[serde(default)]
    pub links: Vec<LinkD>,
    #[serde(default)]
    pub packets: Vec<PacketD>,
    #[serde(default)]
    pub texts: Vec<TextD>,
    pub badge: Option<String>,
    pub note: Option<String>,
}

#[derive(Deserialize)]
pub struct NodeD {
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
pub struct LinkD {
    pub from: String,
    pub to: String,
    #[serde(default)]
    pub arrow: String, // "" | "end" | "both"
}

#[derive(Deserialize)]
pub struct PacketD {
    pub label: String,
    pub from: String,
    pub to: String,
    #[serde(default)]
    pub progress: Option<f64>, // static mode; absent = mid-flight
    #[serde(default)]
    pub faded: bool,
    #[serde(default)]
    pub window: Option<(f64, f64)>, // animated mode, loop fractions
}

#[derive(Deserialize)]
pub struct TextD {
    pub text: String,
    pub x: f64,
    pub y: f64,
    #[serde(default)]
    pub dim: bool,
    #[serde(default)]
    pub left: bool,
}

/// A parsed spec: layout resolved once, sampled per frame.
pub struct ParsedSpec {
    duration: Option<u64>,
    header: Option<String>,
    nodes: Vec<NodeSpec>,
    links: Vec<(f64, f64, f64, f64, bool)>, // x1,y1,x2,y2,arrow_end
    packets: Vec<PacketPlan>,
    texts: Vec<TextSpec>,
    badge: Option<String>,
    note: String,
}

struct PacketPlan {
    label: String,
    from: (f64, f64),
    to: (f64, f64),
    static_p: f64,
    faded: bool,
    window: (f64, f64),
}

pub fn parse(json: &str) -> Result<ParsedSpec, String> {
    let spec: Spec = serde_json::from_str(json).map_err(|e| format!("spec parse: {e}"))?;
    let mut pos: HashMap<String, (f64, f64)> = HashMap::new();
    for (i, n) in spec.nodes.iter().enumerate() {
        pos.insert(n.id.clone().unwrap_or_else(|| i.to_string()), (n.x, n.y));
    }
    let get = |k: &str| -> Result<(f64, f64), String> { pos.get(k).copied().ok_or_else(|| format!("unknown node {k:?}")) };

    let links = spec
        .links
        .iter()
        .map(|l| -> Result<Vec<(f64, f64, f64, f64, bool)>, String> {
            let (a, b) = (get(&l.from)?, get(&l.to)?);
            let end = l.arrow == "end" || l.arrow == "both";
            let mut v = vec![(a.0, a.1, b.0, b.1, end)];
            if l.arrow == "both" {
                v.push((b.0, b.1, a.0, a.1, true));
            }
            Ok(v)
        })
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .flatten()
        .collect();

    let packets = spec
        .packets
        .iter()
        .enumerate()
        .map(|(i, p)| -> Result<PacketPlan, String> {
            let (a, b) = (get(&p.from)?, get(&p.to)?);
            let window = p.window.unwrap_or((
                (0.06 + i as f64 * 0.06).min(0.5),
                (0.55 + i as f64 * 0.06).min(0.92),
            ));
            Ok(PacketPlan { label: p.label.clone(), from: a, to: b, static_p: p.progress.unwrap_or(0.5).clamp(0.0, 1.0), faded: p.faded, window })
        })
        .collect::<Result<_, _>>()?;

    Ok(ParsedSpec {
        duration: spec.duration,
        header: spec.header,
        nodes: spec
            .nodes
            .into_iter()
            .map(|n| NodeSpec { label: n.label.unwrap_or_default(), x: n.x, y: n.y, status: n.status, lifeline: n.lifeline, icon: n.icon })
            .collect(),
        links,
        packets,
        texts: spec.texts.into_iter().map(|t| TextSpec { text: t.text, x: t.x, y: t.y, dim: t.dim, left: t.left }).collect(),
        badge: spec.badge,
        note: spec.note.unwrap_or_default(),
    })
}

impl ParsedSpec {
    pub fn duration(&self) -> Option<u64> {
        self.duration
    }

    /// sample the animation at time t (ms); t=0 for the static frame
    pub fn frame_at(&self, t: u64) -> Frame {
        let frac = self
            .duration
            .map(|d| (t % d) as f64 / d as f64)
            .unwrap_or(0.0);
        let packets = self
            .packets
            .iter()
            .map(|p| {
                let prog = if self.duration.is_some() {
                    let (w0, w1) = p.window;
                    ((frac - w0) / (w1 - w0).max(1e-6)).clamp(0.0, 1.0)
                } else {
                    p.static_p
                };
                PacketSpec { label: p.label.clone(), x1: p.from.0, y1: p.from.1, x2: p.to.0, y2: p.to.1, p: prog, landed: p.faded }
            })
            .collect();
        Frame {
            header: self.header.clone(),
            nodes: self.nodes.clone(),
            packets,
            trails: self.links.iter().map(|(x1, y1, x2, y2, a)| TrailSpec { x1: *x1, y1: *y1, x2: *x2, y2: *y2, arrow_end: *a }).collect(),
            texts: self.texts.clone(),
            polylines: vec![],
            paths: vec![],
            badge: self.badge.clone(),
            note: self.note.clone(),
        }
    }
}
