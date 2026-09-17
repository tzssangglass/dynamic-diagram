//! The portable format. A sim is a pure function of time: frame(t_ms) -> Frame.
//! Deterministic, no DOM, no deps. Renderers own the clock; sims own the content.
// ponytail: interactivity (input/actions) deferred — add when porting sims with buttons.

#[derive(Clone, Debug)]
pub struct NodeSpec {
    pub label: String,
    pub x: f64, // 0..100 scene coords
    pub y: f64, // 0..100
    pub status: Option<String>, // badge under the node (e.g. "seq 5000")
    pub lifeline: bool,         // draw a vertical dashed line below the node
    pub icon: Option<String>,   // glyph kind: router|server|client|phone|tower|cloud|dns|switch|firewall
}

#[derive(Clone, Debug)]
pub struct PacketSpec {
    pub label: String,
    pub x1: f64,
    pub y1: f64, // from (0..100)
    pub x2: f64,
    pub y2: f64, // to
    pub p: f64,     // progress 0..1
    pub landed: bool, // arrived: sits faded at destination
}

#[derive(Clone, Debug)]
pub struct TrailSpec {
    pub x1: f64,
    pub y1: f64,
    pub x2: f64,
    pub y2: f64,
    pub arrow_end: bool, // draw an arrowhead at (x2,y2)
} // line segment: landed-packet trail or static link

#[derive(Clone, Debug)]
pub struct TextSpec {
    pub text: String,
    pub x: f64,
    pub y: f64,
    pub dim: bool,  // dim = secondary color (annotations, axis labels)
    pub left: bool, // left-aligned at x (default: centered)
}

#[derive(Clone, Debug)]
pub struct PolylineSpec {
    pub points: Vec<(f64, f64)>,
    pub dim: bool,
} // waveforms, graphs

#[derive(Clone, Debug)]
pub struct Frame {
    pub nodes: Vec<NodeSpec>,
    pub packets: Vec<PacketSpec>,
    pub trails: Vec<TrailSpec>,
    pub texts: Vec<TextSpec>,     // free-floating annotations
    pub polylines: Vec<PolylineSpec>,
    pub paths: Vec<String>, // raw SVG path data in 0..100 scene coords (maps, curves)
    pub badge: Option<String>,  // centered bottom badge (e.g. "established")
    pub header: Option<String>, // top bar text, "··" splits left/right
    pub note: String,           // commentary line for the current stage
}

pub trait Sim {
    fn duration(&self) -> u64; // ms per loop
    fn frame(&self, t: u64) -> Frame;
}
