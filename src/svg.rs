//! SVG renderer: Frame -> standalone SVG string, styled after fazamhd.com's CSS
//! (mono bordered message boxes, dashed seq badges, dashed lifelines, ink/dim palette).
//! Any GUI that can show SVG works (browser DOM, <img>, desktop viewer, or rasterize).

use crate::frame::Frame;
use crate::{
    layout::{Plan, Rect},
    timeline::{Doc, El},
    typography::{escape as esc, Typography},
};

// fazamhd.com's real palette (light theme, PageLayout.css :root)
const INK: &str = "#1b1f2a";
const DIM: &str = "#4a5163";
const LINE: &str = "#c2c9da";
const FAINT: &str = "#5b6274";
const PAGE: &str = "#fafbfd";
const FONT_MONO: &str = "Inconsolata"; // single name: usvg can't parse comma lists

// icon glyph at (x,y). Four tiers:
//   1. software-engineering structural glyphs (stack, queue, pool, heap…)
//   2. Material Symbols full library (3,912) + aliases
//   3. cloud provider official icons (aws/*, azure/*, gcp/*, cf/*) — full color
//   4. flowchart shapes (box, cylinder, diamond…)
fn icon_glyph(out: &mut String, icon: &str, x: f64, y: f64, size: f64) {
    // aliases into material
    let mkey = match icon {
        "server" => "dns",                                       // stacked-servers look
        "client" | "phone" | "smartphone" => "ti/device-mobile", // material "smartphone" is absent from the table
        "tower" => "cell_tower",
        "switch" => "settings_ethernet",
        "firewall" => "security",
        "database" | "db" => "storage",
        "globe" | "internet" => "public",
        "key" => "vpn_key",
        "gear" => "settings",
        "power" => "bolt",
        "doc" => "description",
        "bank" => "account_balance",
        "robot" => "precision_manufacturing",
        "laptop" => "laptop_mac", // "laptop" has no symbol; mac variant stands in
        "sd_storage" => "storage",
        "https" => "lock",
        "restore" => "autorenew",
        "source" => "code",
        "gpp_good" => "verified_user",
        other => other,
    };
    if let Some(d) = crate::assets::icons::get(mkey) {
        emit_icon(out, d, x, y, size);
        return;
    }
    // brand fallback: "docker" → ti/brand-docker (376 tabler brand glyphs)
    if let Some(d) = crate::assets::icons::get(&format!("brand-{icon}")) {
        emit_icon(out, d, x, y, size);
        return;
    }
    // Flowchart paths use an intrinsic 48-unit resource viewport, like the
    // external icon tiers. Their presentation size comes only from the theme.
    out.push_str(&format!(
        "<g transform=\"translate({x} {y}) scale({})\">",
        size / 48.
    ));
    let (x, y) = (0., 0.);
    let frag = match icon {
        // flowchart / graphviz shapes (semantic, not pictorial)
        "box" => format!("<g transform=\"translate({x} {y})\"><rect x=\"-17\" y=\"-10\" width=\"34\" height=\"20\" rx=\"2\" fill=\"{PAGE}\" stroke=\"{INK}\" stroke-width=\"1.4\"/></g>\n"),
        "cylinder" => format!(
            "<g transform=\"translate({x} {y})\"><path d=\"M -13 -12 L -13 8 A 13 6 0 0 0 13 8 L 13 -12 A 13 6 0 0 0 -13 -12 Z\" fill=\"{PAGE}\" stroke=\"{INK}\" stroke-width=\"1.5\"/><ellipse cx=\"0\" cy=\"-12\" rx=\"13\" ry=\"6\" fill=\"{PAGE}\" stroke=\"{INK}\" stroke-width=\"1.5\"/></g>\n"
        ),
        "ellipse" | "oval" | "circle" => format!(
            "<ellipse cx=\"{x}\" cy=\"{y}\" rx=\"18\" ry=\"12\" fill=\"{PAGE}\" stroke=\"{INK}\" stroke-width=\"1.5\"/>\n"
        ),
        "diamond" => format!(
            "<path d=\"M {x} {} L {} {y} L {x} {} L {} {y} Z\" fill=\"{PAGE}\" stroke=\"{INK}\" stroke-width=\"1.5\"/>\n",
            y - 16.0, x + 20.0, y + 16.0, x - 20.0
        ),
        "hexagon" => format!(
            "<path d=\"M {} {y} L {} {} L {} {} L {} {y} L {} {} L {} {} Z\" fill=\"{PAGE}\" stroke=\"{INK}\" stroke-width=\"1.5\"/>\n",
            x - 22.0, x - 11.0, y - 12.0, x + 11.0, y - 12.0, x + 22.0, x + 11.0, y + 12.0, x - 11.0, y + 12.0
        ),
        "stadium" | "pill" => format!(
            "<rect x=\"{}\" y=\"{}\" width=\"40\" height=\"20\" rx=\"10\" fill=\"{PAGE}\" stroke=\"{INK}\" stroke-width=\"1.5\"/>\n",
            x - 20.0, y - 10.0
        ),
        "triangle" => format!(
            "<path d=\"M {x} {} L {} {} L {} {} Z\" fill=\"{PAGE}\" stroke=\"{INK}\" stroke-width=\"1.5\"/>\n",
            y - 14.0, x + 14.0, y + 10.0, x - 14.0, y + 10.0
        ),
        "subroutine" => format!(
            "<g transform=\"translate({x} {y})\"><rect x=\"-18\" y=\"-11\" width=\"36\" height=\"22\" fill=\"{PAGE}\" stroke=\"{INK}\" stroke-width=\"1.4\"/><line x1=\"-12\" y1=\"-11\" x2=\"-12\" y2=\"11\" stroke=\"{INK}\" stroke-width=\"1.2\"/><line x1=\"12\" y1=\"-11\" x2=\"12\" y2=\"11\" stroke=\"{INK}\" stroke-width=\"1.2\"/></g>\n"
        ),
        // unknown name: dashed placeholder + "?" so a typo is visible, not silent
        _ => format!(
            "<g transform=\"translate({x} {y})\"><rect x=\"-17\" y=\"-11\" width=\"34\" height=\"22\" rx=\"3\" fill=\"{PAGE}\" stroke=\"{LINE}\" stroke-dasharray=\"3 3\"/><text x=\"0\" y=\"4\" text-anchor=\"middle\" fill=\"{DIM}\" font-size=\"12\">{}</text></g>\n",
            "?"
        ),
    };
    out.push_str(&frag);
    out.push_str("</g>");
}

/// draw a resolved icon glyph centered at (x,y)
fn emit_icon(out: &mut String, d: crate::assets::icons::Icon, x: f64, y: f64, size: f64) {
    match d {
        crate::assets::icons::Icon::Path(d) => {
            // material fill paths: 960x960 grid, y negative-up (viewBox 0 -960 960 960);
            // center (480,-480) in the allocated icon box
            out.push_str(&format!(
                "<g transform=\"translate({x} {y}) scale({}) translate(-480 480)\"><path d=\"{d}\" fill=\"{INK}\"/></g>\n", size/960.
            ));
        }
        crate::assets::icons::Icon::Stroke(d) => {
            // tabler stroke paths: 24x24 viewbox, fill=none + ink stroke
            out.push_str(&format!(
                "<g transform=\"translate({x} {y}) scale({}) translate(-12 -12)\"><path d=\"{d}\" fill=\"none\" stroke=\"{INK}\" stroke-width=\"1.7\" stroke-linecap=\"round\" stroke-linejoin=\"round\"/></g>\n", size/24.
            ));
        }
        crate::assets::icons::Icon::Svg(vb, inner) => {
            // full-color provider icon: nested SVG in the allocated icon box
            out.push_str(&format!(
                "<svg x=\"{}\" y=\"{}\" width=\"{size}\" height=\"{size}\" viewBox=\"{vb}\">{inner}</svg>\n",
                x - size/2.,
                y - size/2.
            ));
        }
    }
}

/// A document owns its immutable layout and its bounded-by-document text cache.
pub struct Renderer {
    doc: Doc,
    plan: Plan,
    ty: Typography,
}
impl Renderer {
    pub fn new(doc: &Doc) -> Result<Self, String> {
        let ty = Typography::default();
        let plan = Plan::new(doc, &ty)?;
        Ok(Self {
            doc: doc.clone(),
            plan,
            ty,
        })
    }
    pub fn size(&self) -> (f64, f64) {
        (
            self.plan.width * self.plan.canvas.display_scale(),
            self.plan.height * self.plan.canvas.display_scale(),
        )
    }
    pub fn render(&self, t: u64) -> String {
        let time = match self.doc.duration {
            Some(d) if d > 0 => t % d,
            _ => t,
        };
        let (mut ns, mut ts, mut ls, mut ps) = (vec![], vec![], vec![], vec![]);
        let (mut ni, mut ti) = (0, 0);
        for el in &self.doc.els {
            match el {
                El::Node(n) => {
                    if !n.show.as_ref().is_some_and(|s| !s.step(time)) {
                        ns.push(ni);
                    }
                    ni += 1;
                }
                El::Text(t) => {
                    if !t.show.as_ref().is_some_and(|s| !s.step(time)) {
                        ts.push(ti);
                    }
                    ti += 1;
                }
                El::Line(l) => {
                    if !l.show.as_ref().is_some_and(|s| !s.step(time)) {
                        ls.push((l.from.clone(), l.to.clone()));
                    }
                }
                El::Packet(p) => {
                    if !p.show.as_ref().is_some_and(|s| !s.step(time)) {
                        ps.push((p.from.clone(), p.to.clone()));
                    }
                }
                _ => {}
            }
        }
        let ids = self
            .doc
            .els
            .iter()
            .filter_map(|el| {
                if let El::Node(n) = el {
                    Some(n.id.clone())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        render(
            &self.doc.frame_at(t),
            &self.plan,
            &self.ty,
            &ns,
            &ts,
            &ls,
            &ps,
            &ids,
        )
    }
}
pub fn render_svg(f: &Frame) -> String {
    let ty = Typography::default();
    let plan = Plan::for_frame(f, &ty);
    render(
        f,
        &plan,
        &ty,
        &(0..f.nodes.len()).collect::<Vec<_>>(),
        &(0..f.texts.len()).collect::<Vec<_>>(),
        &[],
        &[],
        &[],
    )
}
fn text(
    out: &mut String,
    ty: &Typography,
    s: &str,
    size: f64,
    box_: Rect,
    color: &str,
    center: bool,
    gap: f64,
) {
    let lines = ty.wrap(s, size, box_.w.max(1.));
    let line_h = ty.measure("Mg", size).height;
    for (i, line) in lines.iter().enumerate() {
        if line.is_empty() {
            continue;
        }
        let m = ty.measure(line, size);
        let x = box_.x + if center { (box_.w - m.width) / 2. } else { 0. } - m.left;
        let y = box_.y + i as f64 * (line_h + gap) - m.top;
        out.push_str(&format!(
            "<text x=\"{x}\" y=\"{y}\" fill=\"{color}\" font-size=\"{size}\">{}</text>\n",
            esc(line)
        ));
    }
}
fn pill(out: &mut String, ty: &Typography, s: &str, box_: Rect, p: &Plan, pending: bool) {
    let th = p.theme;
    let op = if pending { 0.5 } else { 1. };
    let dash = if pending {
        format!("{} {}", th.dash, th.dash)
    } else {
        "none".into()
    };
    let stroke = if pending { LINE } else { INK };
    out.push_str(&format!("<g opacity=\"{op}\"><rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"{}\" fill=\"{PAGE}\" stroke=\"{stroke}\" stroke-dasharray=\"{dash}\"/>\n",box_.x,box_.y,box_.w,box_.h,th.badge_radius));
    text(
        out,
        ty,
        s,
        th.font,
        Rect {
            x: box_.x + th.badge_padding,
            y: box_.y + th.badge_padding / 2.,
            w: box_.w - 2. * th.badge_padding,
            h: box_.h - th.badge_padding,
        },
        INK,
        true,
        th.line_gap,
    );
    out.push_str("</g>\n");
}
type References = (Option<String>, Option<String>);
#[allow(clippy::too_many_arguments)]
fn render(
    f: &Frame,
    p: &Plan,
    ty: &Typography,
    ns: &[usize],
    ts: &[usize],
    ls: &[References],
    ps: &[References],
    ids: &[Option<String>],
) -> String {
    let th = p.theme;
    let (w, h) = (p.width, p.height);
    let mut out = String::with_capacity(16384);
    out.push_str(&format!("<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{}\" height=\"{}\" viewBox=\"0 0 {w} {h}\" font-family=\"{FONT_MONO}\">\n",w*p.canvas.display_scale(),h*p.canvas.display_scale()));
    out.push_str(&format!("<rect width=\"{w}\" height=\"{h}\" fill=\"{PAGE}\"/><g stroke-width=\"{}\"><rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"{}\" fill=\"none\" stroke=\"{LINE}\"/>\n",th.border,th.border/2.,th.border/2.,w-th.border,h-th.border,th.corner_radius));
    if let Some(header) = &f.header {
        let parts = header.split("··").map(str::trim).collect::<Vec<_>>();
        for (i, part) in parts.iter().take(2).enumerate() {
            let width = (w - 2. * th.padding) / 2. - th.gap;
            text(
                &mut out,
                ty,
                part,
                th.font,
                Rect {
                    x: th.padding + if i == 0 { 0. } else { w / 2. },
                    y: th.padding,
                    w: width,
                    h: p.header.h,
                },
                DIM,
                false,
                th.line_gap,
            );
        }
        out.push_str(&format!(
            "<line x1=\"0\" y1=\"{}\" x2=\"{w}\" y2=\"{}\" stroke=\"{LINE}\"/>\n",
            p.header.bottom(),
            p.header.bottom()
        ));
    }
    let node_boxes = f
        .nodes
        .iter()
        .zip(ns)
        .map(|(n, i)| {
            let (x, y) = p.node_point(*i, n.x, n.y);
            p.nodes[*i].bounds.moved(x, y)
        })
        .collect::<Vec<_>>();
    let node_ref = |id: &Option<String>| -> Option<Rect> {
        let id = id.as_ref()?;
        ns.iter()
            .position(|i| ids.get(*i).and_then(Option::as_ref) == Some(id))
            .map(|i| node_boxes[i])
    };
    let route = |x1: f64, y1: f64, x2: f64, y2: f64, refs: Option<&References>| {
        let mut a = p.point(x1, y1);
        let mut b = p.point(x2, y2);
        if let Some((from, to)) = refs {
            let (ab, bb) = (node_ref(from), node_ref(to));
            if let Some(r) = ab {
                a = r.center();
            }
            if let Some(r) = bb {
                b = r.center();
            }
            let (ac, bc) = (a, b);
            if let Some(r) = ab {
                a = crate::layout::boundary(r, bc);
            }
            if let Some(r) = bb {
                b = crate::layout::boundary(r, ac);
            }
        }
        (a, b)
    };
    for (n, b) in f.nodes.iter().zip(&node_boxes) {
        if n.lifeline {
            let (x, _) = b.center();
            let bottom = p.point(n.x, 100.).1.max(b.bottom() + th.gap);
            out.push_str(&format!("<line x1=\"{x}\" y1=\"{}\" x2=\"{x}\" y2=\"{bottom}\" stroke=\"{LINE}\" stroke-dasharray=\"{} {}\"/>\n",b.bottom()+th.gap,th.dash,th.dash));
        }
    }
    if !f.paths.is_empty() {
        out.push_str(&format!(
            "<g transform=\"translate({} {}) scale({} {})\">",
            p.stage_left,
            p.stage_top,
            p.stage_width / 100.,
            p.stage_span / 100.
        ));
        for d in &f.paths {
            out.push_str(&format!("<path d=\"{}\" fill=\"{LINE}\"/>\n", esc(d)));
        }
        out.push_str("</g>");
    }
    for pl in &f.polylines {
        let pts = pl
            .points
            .iter()
            .map(|(x, y)| {
                let (x, y) = p.point(*x, *y);
                format!("{x},{y}")
            })
            .collect::<Vec<_>>()
            .join(" ");
        out.push_str(&format!(
            "<polyline points=\"{pts}\" fill=\"none\" stroke=\"{}\" stroke-width=\"{}\"/>\n",
            if pl.dim { LINE } else { INK },
            th.route_stroke
        ));
    }
    for (i, l) in f.trails.iter().enumerate() {
        let ((x1, y1), (x2, y2)) = route(l.x1, l.y1, l.x2, l.y2, ls.get(i));
        out.push_str(&format!("<line id=\"route-{i}\" x1=\"{x1}\" y1=\"{y1}\" x2=\"{x2}\" y2=\"{y2}\" stroke=\"{LINE}\" opacity=\"0.6\"/>\n"));
        if l.arrow_end {
            let (dx, dy) = (x2 - x1, y2 - y1);
            let len = dx.hypot(dy).max(1.);
            let (ux, uy) = (dx / len, dy / len);
            out.push_str(&format!(
                "<path d=\"M {x2} {y2} L {} {} L {} {} Z\" fill=\"{INK}\"/>\n",
                x2 - ux * th.gap - uy * th.gap / 2.,
                y2 - uy * th.gap + ux * th.gap / 2.,
                x2 - ux * th.gap + uy * th.gap / 2.,
                y2 - uy * th.gap - ux * th.gap / 2.
            ));
        }
    }
    let mut label_obstacles = node_boxes.clone();
    label_obstacles.extend(f.texts.iter().zip(ts).map(|(t, index)| {
        let a = &p.annotations[*index];
        let (x, y) = p.point(t.x, t.y);
        Rect {
            x: x + a.dx,
            y: y + a.dy,
            ..a.bounds
        }
    }));
    for (i, pk) in f.packets.iter().enumerate() {
        let (a, b) = route(pk.x1, pk.y1, pk.x2, pk.y2, ps.get(i));
        let x = a.0 + (b.0 - a.0) * pk.p;
        let y = a.1 + (b.1 - a.1) * pk.p;
        let lines = ty.wrap(
            &pk.label,
            th.font,
            w - 2. * th.padding - 2. * th.badge_padding,
        );
        let label_width = lines
            .iter()
            .map(|s| ty.measure(s, th.font).width)
            .fold(0., f64::max);
        let line_height = ty.measure("Mg", th.font).height;
        let label_height = lines.len() as f64 * (line_height + th.line_gap) - th.line_gap;
        let wanted = Rect {
            x: x - label_width / 2. - th.badge_padding,
            y: y - (label_height + th.badge_padding) / 2.,
            w: label_width + 2. * th.badge_padding,
            h: label_height + th.badge_padding,
        };
        let travel = (b.0 - a.0).hypot(b.1 - a.1) > f64::EPSILON;
        if travel && !pk.landed && (pk.p <= 0. || pk.p >= 1.) {
            continue;
        }
        let viewport = Rect {
            x: th.padding,
            y: p.header.bottom() + th.gap,
            w: w - 2. * th.padding,
            h: p.badge.y - p.header.bottom() - 2. * th.gap,
        };
        let Some(box_) = crate::layout::place_near(wanted, &label_obstacles, viewport, th.gap / 2.)
        else {
            continue;
        };
        label_obstacles.push(box_);
        if pk.landed {
            out.push_str("<g opacity=\"0.55\">");
        }
        out.push_str(&format!("<g id=\"packet-{i}\">"));
        if (box_.x - wanted.x).abs() + (box_.y - wanted.y).abs() > f64::EPSILON {
            let (lx, ly) = crate::layout::boundary(box_, (x, y));
            out.push_str(&format!("<line x1=\"{x}\" y1=\"{y}\" x2=\"{lx}\" y2=\"{ly}\" stroke=\"{LINE}\"/><circle cx=\"{x}\" cy=\"{y}\" r=\"{}\" fill=\"{INK}\"/>",th.route_stroke));
        }
        pill(&mut out, ty, &pk.label, box_, p, false);
        out.push_str("</g>");
        if pk.landed {
            out.push_str("</g>");
        }
    }
    for ((n, index), bounds) in f.nodes.iter().zip(ns).zip(&node_boxes) {
        let nb = &p.nodes[*index];
        let (x, y) = p.node_point(*index, n.x, n.y);
        out.push_str(&format!("<g id=\"node-{index}\" data-node=\"{index}\">"));
        if let Some(icon) = &n.icon {
            let (ix, iy) = nb.icon.unwrap().moved(x, y).center();
            icon_glyph(&mut out, icon, ix, iy, th.icon);
        } else if n.label.is_empty() {
            out.push_str(&format!("<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"{PAGE}\" stroke=\"{FAINT}\"/>\n",x-th.gap/2.,y-th.gap/2.,th.gap,th.gap));
        }
        text(
            &mut out,
            ty,
            &n.label,
            th.label_font,
            nb.label.moved(x, y),
            INK,
            true,
            th.line_gap,
        );
        if let Some(status) = &n.status {
            if nb.status.h > 0. {
                pill(
                    &mut out,
                    ty,
                    status,
                    nb.status.moved(x, y),
                    p,
                    status.contains("pending"),
                );
            }
        }
        let _ = bounds;
        out.push_str("</g>\n");
    }
    for (t, index) in f.texts.iter().zip(ts) {
        let a = &p.annotations[*index];
        let (x, y) = p.point(t.x, t.y);
        text(
            &mut out,
            ty,
            &t.text,
            th.font,
            Rect {
                x: x + a.dx,
                y: y + a.dy,
                ..a.bounds
            },
            if t.dim { DIM } else { INK },
            false,
            th.line_gap,
        );
    }
    if let Some(badge) = &f.badge {
        text(
            &mut out,
            ty,
            &badge.to_uppercase(),
            th.font,
            Rect {
                x: th.padding,
                y: p.badge.y + th.gap,
                w: w - 2. * th.padding,
                h: p.badge.h,
            },
            INK,
            true,
            th.line_gap,
        );
    }
    out.push_str(&format!(
        "<line data-footer=\"true\" x1=\"0\" y1=\"{}\" x2=\"{w}\" y2=\"{}\" stroke=\"{LINE}\"/>\n",
        p.caption.y, p.caption.y
    ));
    text(
        &mut out,
        ty,
        &f.note,
        th.font,
        Rect {
            x: th.padding,
            y: p.caption.y + th.padding,
            w: w - 2. * th.padding,
            h: p.caption.h,
        },
        DIM,
        false,
        th.line_gap,
    );
    out.push_str("</g></svg>\n");
    out
}
#[cfg(test)]
#[path = "render_tests.rs"]
mod render_tests;
