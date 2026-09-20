//! The universal timeline document: a dynamic scene is a list of elements whose
//! properties are functions of time. Every property is either a constant or a
//! keyframe list — numbers/points interpolate linearly, strings/bools hold
//! until the next key. `frame_at(t)` resolves everything into a static Frame.
//! No expressions, no scripts: content is pure data (authoring-time
//! computation belongs in the author, not the renderer).

use crate::frame::*;
use serde::Deserialize;
use std::collections::HashMap;

/// A timeline value: `T` constant or `[[t, v], ...]` keyframes.
#[derive(Deserialize, Debug, Clone)]
#[serde(untagged)]
pub enum Tl<T> {
    Const(T),
    Keyed(Vec<(u64, T)>),
}

impl<T: Clone> Tl<T> {
    /// step resolution: value of the last key at or before t (strings, bools)
    pub fn step(&self, t: u64) -> T {
        match self {
            Tl::Const(v) => v.clone(),
            Tl::Keyed(k) => k
                .iter()
                .rev()
                .find(|(kt, _)| *kt <= t)
                .map(|(_, v)| v.clone())
                .unwrap_or_else(|| k[0].1.clone()),
        }
    }
}

impl Tl<f64> {
    /// linear resolution between keys (numbers)
    pub fn at(&self, t: u64) -> f64 {
        match self {
            Tl::Const(v) => *v,
            Tl::Keyed(k) => {
                if t <= k[0].0 {
                    return k[0].1;
                }
                for w in k.windows(2) {
                    if t <= w[1].0 {
                        let (t0, v0) = w[0];
                        let (t1, v1) = w[1];
                        let f = if t1 == t0 {
                            0.0
                        } else {
                            (t - t0) as f64 / (t1 - t0) as f64
                        };
                        return v0 + (v1 - v0) * f;
                    }
                }
                k[k.len() - 1].1
            }
        }
    }
}

impl Tl<Vec<f64>> {
    /// element-wise linear resolution (keyed arrays keep equal length)
    pub fn at_arr(&self, t: u64) -> Vec<f64> {
        match self {
            Tl::Const(v) => v.clone(),
            Tl::Keyed(k) => {
                if k.len() == 1 || t <= k[0].0 {
                    return k[0].1.clone();
                }
                let (t0, a) = (&k[0].0, &k[0].1);
                let mut span = (t0, a, &k[1].0, &k[1].1);
                for w in k.windows(2) {
                    span = (&w[0].0, &w[0].1, &w[1].0, &w[1].1);
                    if t <= w[1].0 {
                        break;
                    }
                }
                let f = if *span.2 == *span.0 {
                    0.0
                } else {
                    (t - *span.0) as f64 / (*span.2 - *span.0) as f64
                };
                span.1
                    .iter()
                    .zip(span.3)
                    .map(|(x, y)| x + (y - x) * f)
                    .collect()
            }
        }
    }
}

impl Default for Tl<bool> {
    fn default() -> Self {
        Tl::Const(false)
    }
}

#[derive(Deserialize, Debug, Clone)]
pub struct NodeEl {
    pub id: Option<String>,
    #[serde(default)]
    pub label: Option<Tl<String>>,
    pub icon: Option<String>,
    pub x: Tl<f64>,
    pub y: Tl<f64>,
    #[serde(default)]
    pub status: Option<Tl<String>>,
    #[serde(default)]
    pub lifeline: bool,
    pub show: Option<Tl<bool>>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct LineEl {
    pub from: Option<String>,
    pub to: Option<String>,
    pub x1: Option<Tl<f64>>,
    pub y1: Option<Tl<f64>>,
    pub x2: Option<Tl<f64>>,
    pub y2: Option<Tl<f64>>,
    #[serde(default)]
    pub arrow_end: bool,
    pub show: Option<Tl<bool>>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct PacketEl {
    pub from: Option<String>,
    pub to: Option<String>,
    pub label: String,
    pub x1: Tl<f64>,
    pub y1: Tl<f64>,
    pub x2: Tl<f64>,
    pub y2: Tl<f64>,
    pub p: Tl<f64>,
    #[serde(default)]
    pub landed: Tl<bool>,
    pub show: Option<Tl<bool>>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct TextEl {
    pub text: Tl<String>,
    pub x: Tl<f64>,
    pub y: Tl<f64>,
    #[serde(default)]
    pub dim: Tl<bool>,
    #[serde(default)]
    pub left: bool,
    pub show: Option<Tl<bool>>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct PolyEl {
    /// flat [x0,y0,x1,y1,...]; keyed arrays keep equal length
    pub points: Tl<Vec<f64>>,
    /// visible slice [from_idx, to_idx) — each side may be a timeline
    #[serde(default)]
    pub slice: Option<(Tl<f64>, Tl<f64>)>,
    #[serde(default)]
    pub dim: bool,
    pub show: Option<Tl<bool>>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct PathEl {
    pub d: String,
    pub show: Option<Tl<bool>>,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum El {
    Node(NodeEl),
    Line(LineEl),
    Packet(PacketEl),
    Text(TextEl),
    Polyline(PolyEl),
    Path(PathEl),
}

#[derive(Deserialize, Debug, Clone)]
pub struct Doc {
    /// v1 authoring intent retained so canvas overrides can reflow measured boxes.
    #[serde(skip)]
    pub structural: Option<crate::layout::Structural>,
    #[serde(default)]
    pub canvas: crate::layout::Canvas,
    /// ms per loop; None = static single frame
    pub duration: Option<u64>,
    pub header: Option<Tl<String>>,
    pub badge: Option<Tl<String>>,
    pub note: Option<Tl<String>>,
    #[serde(default)]
    pub els: Vec<El>,
}

impl Doc {
    pub fn frame_at(&self, t: u64) -> Frame {
        let t = match self.duration {
            Some(d) if d > 0 => t % d,
            _ => t,
        };
        let mut nodes = Vec::new();
        let mut pos: HashMap<String, (f64, f64)> = HashMap::new();
        for el in &self.els {
            if let El::Node(n) = el {
                if n.show.as_ref().is_some_and(|s| !s.step(t)) {
                    continue;
                }
                let (x, y) = (n.x.at(t), n.y.at(t));
                if let Some(id) = &n.id {
                    pos.insert(id.clone(), (x, y));
                }
                nodes.push(NodeSpec {
                    label: n.label.as_ref().map(|l| l.step(t)).unwrap_or_default(),
                    x,
                    y,
                    status: n
                        .status
                        .as_ref()
                        .map(|s| s.step(t))
                        .filter(|s| !s.is_empty()),
                    lifeline: n.lifeline,
                    icon: n.icon.clone(),
                });
            }
        }
        // line endpoint: node id wins, else raw coordinate
        let pt = |id: &Option<String>, raw: &Option<Tl<f64>>, idx: usize, fb: f64| -> f64 {
            if let Some(nid) = id {
                if let Some(p) = pos.get(nid) {
                    return if idx == 0 { p.0 } else { p.1 };
                }
            }
            raw.as_ref().map(|v| v.at(t)).unwrap_or(fb)
        };
        let mut trails = Vec::new();
        let mut packets = Vec::new();
        let mut texts = Vec::new();
        let mut polylines = Vec::new();
        let mut paths = Vec::new();
        for el in &self.els {
            match el {
                El::Node(_) => {}
                El::Line(l) => {
                    if l.show.as_ref().is_some_and(|s| !s.step(t)) {
                        continue;
                    }
                    trails.push(TrailSpec {
                        x1: pt(&l.from, &l.x1, 0, 0.0),
                        y1: pt(&l.from, &l.y1, 1, 0.0),
                        x2: pt(&l.to, &l.x2, 0, 100.0),
                        y2: pt(&l.to, &l.y2, 1, 0.0),
                        arrow_end: l.arrow_end,
                    });
                }
                El::Packet(p) => {
                    if p.show.as_ref().is_some_and(|s| !s.step(t)) {
                        continue;
                    }
                    packets.push(PacketSpec {
                        label: p.label.clone(),
                        x1: p.x1.at(t),
                        y1: p.y1.at(t),
                        x2: p.x2.at(t),
                        y2: p.y2.at(t),
                        p: p.p.at(t).clamp(0.0, 1.0),
                        landed: p.landed.step(t),
                    });
                }
                El::Text(x) => {
                    if x.show.as_ref().is_some_and(|s| !s.step(t)) {
                        continue;
                    }
                    texts.push(TextSpec {
                        text: x.text.step(t),
                        x: x.x.at(t),
                        y: x.y.at(t),
                        dim: x.dim.step(t),
                        left: x.left,
                    });
                }
                El::Polyline(pl) => {
                    if pl.show.as_ref().is_some_and(|s| !s.step(t)) {
                        continue;
                    }
                    let full = pl.points.at_arr(t);
                    let n = full.len() / 2;
                    let (a, b) = match &pl.slice {
                        Some((f, to)) => (
                            f.at(t).round().clamp(0.0, n as f64) as usize,
                            to.at(t).round().clamp(0.0, n as f64) as usize,
                        ),
                        None => (0, n),
                    };
                    let (a, b) = (a.min(b), b.max(a));
                    let pts = full[a * 2..b * 2].chunks(2).map(|c| (c[0], c[1])).collect();
                    polylines.push(PolylineSpec {
                        points: pts,
                        dim: pl.dim,
                    });
                }
                El::Path(p) => {
                    if p.show.as_ref().is_some_and(|s| !s.step(t)) {
                        continue;
                    }
                    paths.push(p.d.clone());
                }
            }
        }
        Frame {
            header: self.header.as_ref().map(|h| h.step(t)),
            nodes,
            packets,
            trails,
            texts,
            polylines,
            paths,
            badge: self
                .badge
                .as_ref()
                .map(|b| b.step(t))
                .filter(|s| !s.is_empty()),
            note: self
                .note
                .as_ref()
                .map(|n| n.step(t))
                .filter(|s| !s.is_empty())
                .unwrap_or_default(),
        }
    }
}

/// A timeline document behind the `Sim` interface: `duration` from the doc,
/// `frame(t)` = resolve at t. web/kitty players consume this unchanged.
pub struct DocSim(pub Doc);

impl Sim for DocSim {
    fn duration(&self) -> u64 {
        self.0.duration.unwrap_or(0)
    }
    fn frame(&self, t: u64) -> Frame {
        self.0.frame_at(t)
    }
}

pub fn parse_doc(json: &str) -> Result<Doc, String> {
    let doc: Doc = serde_json::from_str(json).map_err(|e| format!("doc parse: {e}"))?;
    doc.validate()?;
    Ok(doc)
}

impl<T> Tl<T> {
    fn validate(&self, name: &str, check: impl Fn(&T) -> bool) -> Result<(), String> {
        match self {
            Self::Const(value) if check(value) => Ok(()),
            Self::Keyed(keys)
                if !keys.is_empty()
                    && keys.windows(2).all(|pair| pair[0].0 <= pair[1].0)
                    && keys.iter().all(|(_, value)| check(value)) =>
            {
                Ok(())
            }
            _ => Err(format!(
                "{name}: expected a valid constant or nonempty, ordered keyframes"
            )),
        }
    }
}

impl Doc {
    pub fn validate(&self) -> Result<(), String> {
        self.canvas.validate()?;
        let coordinate =
            |v: &Tl<f64>| v.validate("coordinate", |x| x.is_finite() && (0. ..=100.).contains(x));
        let finite = |v: &Tl<f64>| v.validate("number", |x| x.is_finite());
        let visible = |v: &Option<Tl<bool>>| match v {
            Some(v) => v.validate("visibility", |_| true),
            None => Ok(()),
        };
        let string = |v: &Tl<String>| v.validate("text", |s| crate::typography::valid_text(s));
        for v in [&self.header, &self.badge, &self.note]
            .into_iter()
            .flatten()
        {
            string(v)?;
        }
        let mut ids = std::collections::HashSet::new();
        for el in &self.els {
            if let El::Node(n) = el {
                if let Some(id) = &n.id {
                    if !ids.insert(id) {
                        return Err(format!("duplicate node id {id:?}"));
                    }
                }
            }
        }
        let reference = |id: &Option<String>| {
            if id.as_ref().is_some_and(|id| !ids.contains(id)) {
                Err(format!("unknown node {id:?}"))
            } else {
                Ok(())
            }
        };
        for el in &self.els {
            match el {
                El::Node(n) => {
                    coordinate(&n.x)?;
                    coordinate(&n.y)?;
                    visible(&n.show)?;
                    for s in [&n.label, &n.status].into_iter().flatten() {
                        string(s)?;
                    }
                }
                El::Line(l) => {
                    for v in [&l.x1, &l.y1, &l.x2, &l.y2].into_iter().flatten() {
                        coordinate(v)?;
                    }
                    reference(&l.from)?;
                    reference(&l.to)?;
                    visible(&l.show)?;
                }
                El::Packet(p) => {
                    for v in [&p.x1, &p.y1, &p.x2, &p.y2] {
                        coordinate(v)?;
                    }
                    finite(&p.p)?;
                    visible(&p.show)?;
                    p.landed.validate("landed", |_| true)?;
                    reference(&p.from)?;
                    reference(&p.to)?;
                    string(&Tl::Const(p.label.clone()))?;
                }
                El::Text(t) => {
                    coordinate(&t.x)?;
                    coordinate(&t.y)?;
                    string(&t.text)?;
                    visible(&t.show)?;
                    t.dim.validate("dim", |_| true)?;
                }
                El::Polyline(p) => {
                    let mut length = None;
                    p.points.validate("points", |points| {
                        points.len() % 2 == 0 && points.iter().all(|x| x.is_finite())
                    })?;
                    if let Tl::Keyed(keys) = &p.points {
                        for (_, points) in keys {
                            if length.is_some_and(|len| len != points.len()) {
                                return Err("keyed point arrays must have equal lengths".into());
                            }
                            length = Some(points.len());
                        }
                    }
                    if let Some((a, b)) = &p.slice {
                        finite(a)?;
                        finite(b)?;
                    }
                    visible(&p.show)?;
                }
                El::Path(p) => {
                    visible(&p.show)?;
                    string(&Tl::Const(p.d.clone()))?;
                }
            }
        }
        Ok(())
    }
}
