//! Prepared geometry: typography feeds Taffy boxes; world coordinates remain a
//! single affine transform. No frame depends on the previously rendered frame.
use crate::{
    frame::Frame,
    timeline::{Doc, El, Tl},
    typography::Typography,
};
use serde::Deserialize;
use taffy::prelude::*;
#[derive(Deserialize, Clone, Copy, Debug)]
#[serde(default, deny_unknown_fields)]
pub struct Canvas {
    pub width: f64,
    pub min_height: f64,
    pub scale: f64,
}
impl Default for Canvas {
    fn default() -> Self {
        Self {
            width: 760.,
            min_height: 330.,
            scale: 1.,
        }
    }
}
impl Canvas {
    pub fn validate(&self) -> Result<(), String> {
        for (name, v) in [
            ("width", self.width),
            ("min_height", self.min_height),
            ("scale", self.scale),
        ] {
            if !v.is_finite() || v <= 0. {
                return Err(format!("canvas.{name} must be finite and positive"));
            }
        }
        if !(100. ..=100000.).contains(&self.width) || self.min_height > 100000. || self.scale > 64.
        {
            return Err(
                "canvas width must be 100..100000, min_height <= 100000, scale <= 64".into(),
            );
        }
        Ok(())
    }
}
#[derive(Clone, Copy)]
pub struct Theme {
    pub font: f64,
    pub label_font: f64,
    pub icon: f64,
    pub gap: f64,
    pub padding: f64,
    pub badge_padding: f64,
    pub line_gap: f64,
    pub stage_span: f64,
    pub node_text_width: f64,
    pub border: f64,
    pub route_stroke: f64,
    pub corner_radius: f64,
    pub badge_radius: f64,
    pub dash: f64,
}
impl Default for Theme {
    fn default() -> Self {
        Self {
            font: 12.,
            label_font: 13.,
            icon: 36.,
            gap: 8.,
            padding: 14.,
            badge_padding: 8.,
            line_gap: 4.,
            stage_span: 216.,
            node_text_width: 240.,
            border: 1.,
            route_stroke: 1.4,
            corner_radius: 8.,
            badge_radius: 3.,
            dash: 4.,
        }
    }
}
#[derive(Clone, Copy, Default, Debug)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}
impl Rect {
    pub fn right(self) -> f64 {
        self.x + self.w
    }
    pub fn bottom(self) -> f64 {
        self.y + self.h
    }
    pub fn moved(self, x: f64, y: f64) -> Self {
        Self {
            x: self.x + x,
            y: self.y + y,
            ..self
        }
    }
    pub fn overlaps(self, b: Self, gap: f64) -> bool {
        self.x < b.right() + gap
            && b.x < self.right() + gap
            && self.y < b.bottom() + gap
            && b.y < self.bottom() + gap
    }
    pub fn center(self) -> (f64, f64) {
        (self.x + self.w / 2., self.y + self.h / 2.)
    }
}
#[derive(Clone, Debug)]
pub struct NodeBox {
    pub bounds: Rect,
    pub icon: Option<Rect>,
    pub label: Rect,
    pub status: Rect,
}
#[derive(Clone, Debug)]
pub struct Annotation {
    pub bounds: Rect,
    pub dx: f64,
    pub dy: f64,
}
pub struct Plan {
    pub canvas: Canvas,
    pub theme: Theme,
    pub width: f64,
    pub height: f64,
    pub stage_left: f64,
    pub stage_top: f64,
    pub stage_width: f64,
    pub stage_span: f64,
    pub header: Rect,
    pub badge: Rect,
    pub caption: Rect,
    pub nodes: Vec<NodeBox>,
    pub annotations: Vec<Annotation>,
    structural_points: Option<Vec<(f64, f64)>>,
}
fn values<T>(v: &Tl<T>) -> Vec<&T> {
    match v {
        Tl::Const(v) => vec![v],
        Tl::Keyed(k) => k.iter().map(|(_, v)| v).collect(),
    }
}
fn max_text(ty: &Typography, strings: &[String], size: f64, width: f64, gap: f64) -> (f64, f64) {
    let mut w: f64 = 0.;
    let mut h: f64 = 0.;
    for s in strings {
        let lines = ty.wrap(s, size, width);
        w = w.max(
            lines
                .iter()
                .map(|l| ty.measure(l, size).width)
                .fold(0., f64::max),
        );
        let lh = ty.measure("Mg", size).height;
        h = h.max(lines.len() as f64 * (lh + gap) - gap);
    }
    (w, h.max(0.))
}
fn leaf(tree: &mut TaffyTree, w: f64, h: f64) -> NodeId {
    tree.new_leaf(Style {
        size: Size {
            width: Dimension::length(w as f32),
            height: Dimension::length(h as f32),
        },
        flex_shrink: 0.,
        ..Default::default()
    })
    .expect("leaf")
}
fn rect(tree: &TaffyTree, id: NodeId) -> Rect {
    let l = tree.layout(id).expect("computed layout");
    Rect {
        x: l.location.x as f64,
        y: l.location.y as f64,
        w: l.size.width as f64,
        h: l.size.height as f64,
    }
}
fn column(tree: &mut TaffyTree, children: &[NodeId], gap: f64) -> NodeId {
    tree.new_with_children(
        Style {
            display: Display::Flex,
            flex_direction: FlexDirection::Column,
            align_items: Some(AlignItems::CENTER),
            gap: Size {
                width: LengthPercentage::length(gap as f32),
                height: LengthPercentage::length(gap as f32),
            },
            flex_shrink: 0.,
            ..Default::default()
        },
        children,
    )
    .expect("column")
}
fn compute(tree: &mut TaffyTree, id: NodeId) {
    tree.compute_layout(id, Size::MAX_CONTENT)
        .expect("box layout");
}
fn node_box(
    ty: &Typography,
    theme: Theme,
    icon: bool,
    labels: &[String],
    statuses: &[String],
    width: f64,
) -> NodeBox {
    let mut tree = TaffyTree::new();
    tree.disable_rounding();
    let mut children = vec![];
    let icon_id = icon.then(|| {
        let id = leaf(&mut tree, theme.icon, theme.icon);
        children.push(id);
        id
    });
    let (lw, lh) = max_text(ty, labels, theme.label_font, width, theme.line_gap);
    let label = leaf(&mut tree, lw, lh);
    if lh > 0. {
        children.push(label);
    }
    let (sw, sh) = max_text(
        ty,
        statuses,
        theme.font,
        width - theme.badge_padding * 2.,
        theme.line_gap,
    );
    let status = leaf(
        &mut tree,
        if sh > 0. {
            sw + theme.badge_padding * 2.
        } else {
            0.
        },
        if sh > 0. {
            sh + theme.badge_padding
        } else {
            0.
        },
    );
    if sh > 0. {
        children.push(status);
    }
    let root = column(&mut tree, &children, theme.gap);
    compute(&mut tree, root);
    let mut bounds = rect(&tree, root);
    let anchor_y = if icon { theme.icon / 2. } else { lh / 2. };
    let dx = -bounds.w / 2.;
    bounds.x = dx;
    bounds.y = -anchor_y;
    NodeBox {
        bounds,
        icon: icon_id.map(|id| rect(&tree, id).moved(dx, -anchor_y)),
        label: if lh > 0. {
            rect(&tree, label).moved(dx, -anchor_y)
        } else {
            Rect::default()
        },
        status: if sh > 0. {
            rect(&tree, status).moved(dx, -anchor_y)
        } else {
            Rect::default()
        },
    }
}
impl Plan {
    pub fn point(&self, x: f64, y: f64) -> (f64, f64) {
        (
            self.stage_left + x * self.stage_width / 100.,
            self.stage_top + y * self.stage_span / 100.,
        )
    }
    pub fn node_point(&self, index: usize, x: f64, y: f64) -> (f64, f64) {
        self.structural_points
            .as_ref()
            .map(|points| {
                (
                    self.theme.padding + points[index].0,
                    self.stage_top + points[index].1,
                )
            })
            .unwrap_or_else(|| self.point(x, y))
    }
    pub fn new(doc: &Doc, ty: &Typography) -> Result<Self, String> {
        doc.validate()?;
        let th = Theme::default();
        let width = doc.canvas.width;
        let available = width - 2. * th.padding;
        let mut nodes = vec![];
        let mut anchors = vec![];
        let mut text_inputs = vec![];
        let mut node_inputs = vec![];
        let structural_width = doc
            .structural
            .as_ref()
            .map(|config| {
                config.node_width(
                    available,
                    doc.els.iter().filter(|e| matches!(e, El::Node(_))).count(),
                    th,
                )
            })
            .transpose()?;
        for el in &doc.els {
            match el {
                El::Node(n) => {
                    let labels = n
                        .label
                        .as_ref()
                        .map(|v| values(v).into_iter().cloned().collect::<Vec<_>>())
                        .unwrap_or_default();
                    let statuses = n
                        .status
                        .as_ref()
                        .map(|v| {
                            values(v)
                                .into_iter()
                                .filter(|s| !s.is_empty())
                                .cloned()
                                .collect::<Vec<_>>()
                        })
                        .unwrap_or_default();
                    nodes.push(node_box(
                        ty,
                        th,
                        n.icon.is_some(),
                        &labels,
                        &statuses,
                        structural_width.unwrap_or(th.node_text_width.min(available)),
                    ));
                    node_inputs.push((n, labels, statuses));
                    let xs = values(&n.x);
                    let ys = values(&n.y);
                    if xs
                        .iter()
                        .chain(ys.iter())
                        .any(|v| !v.is_finite() || !(0. ..=100.).contains(*v))
                    {
                        return Err("node coordinates must be finite and in 0..100".into());
                    }
                    anchors.push((
                        xs.iter().map(|v| **v).fold(f64::INFINITY, f64::min),
                        xs.iter().map(|v| **v).fold(f64::NEG_INFINITY, f64::max),
                        ys.iter().map(|v| **v).fold(f64::INFINITY, f64::min),
                        ys.iter().map(|v| **v).fold(f64::NEG_INFINITY, f64::max),
                    ));
                }
                El::Text(t) => {
                    let strings = values(&t.text).into_iter().cloned().collect::<Vec<_>>();
                    text_inputs.push((t, strings));
                }
                _ => {}
            }
        }
        let text_margin = text_inputs
            .iter()
            .map(|(t, strings)| {
                let (w, _) = max_text(
                    ty,
                    strings,
                    th.font,
                    if t.left {
                        available / 2. - th.gap
                    } else {
                        available - th.gap
                    },
                    th.line_gap,
                );
                if t.left {
                    w
                } else {
                    w / 2.
                }
            })
            .fold(0., f64::max);
        let margin = nodes
            .iter()
            .map(|n| n.bounds.w / 2.)
            .fold(text_margin.max(th.padding), f64::max)
            + th.padding;
        let stage_width = (width - 2. * margin).max(th.gap);
        if doc.structural.is_none() {
            for i in 0..nodes.len() {
                let mut width = th.node_text_width.min(available);
                for j in 0..nodes.len() {
                    if i == j {
                        continue;
                    }
                    let (a, b) = (anchors[i], anchors[j]);
                    // Same-row anchors cannot gain clearance by growing page height.
                    if a.2 <= b.3 && b.2 <= a.3 {
                        let distance = (a.0 - b.1).max(b.0 - a.1).max(0.);
                        width = width.min(distance * stage_width / 100. - th.gap);
                    }
                }
                let (n, labels, statuses) = &node_inputs[i];
                nodes[i] = node_box(ty, th, n.icon.is_some(), labels, statuses, width.max(1.));
                if width < nodes[i].bounds.w - 0.01 {
                    return Err(format!("node {i} cannot fit fixed anchors at this canvas width; increase width, separate anchors, or use structural layout"));
                }
            }
        }
        let mut structure = doc
            .structural
            .as_ref()
            .map(|config| structural_layout(config, available, &nodes, th, 0.))
            .transpose()?;
        let top = if structure.is_some() {
            th.gap
        } else {
            nodes.iter().map(|n| -n.bounds.y).fold(0., f64::max) + th.gap
        };
        let below = if structure.is_some() {
            th.gap
        } else {
            nodes.iter().map(|n| n.bounds.bottom()).fold(0., f64::max) + th.gap
        };
        let mut span = structure.as_ref().map(|s| s.1).unwrap_or(th.stage_span);
        // Fixed world anchors retain order and relative position. Expand their common
        // coordinate system until vertically separated composite boxes have room.
        for i in 0..if structure.is_none() { nodes.len() } else { 0 } {
            for j in i + 1..nodes.len() {
                let (a, b) = (anchors[i], anchors[j]);
                let xdist = if a.1 < b.0 {
                    b.0 - a.1
                } else if b.1 < a.0 {
                    a.0 - b.1
                } else {
                    0.
                };
                if xdist * stage_width / 100.
                    < (nodes[i].bounds.w + nodes[j].bounds.w) / 2. + th.gap
                {
                    let (first, second, dy) = if a.3 < b.2 {
                        (i, j, b.2 - a.3)
                    } else {
                        (j, i, a.2 - b.3)
                    };
                    if dy > 0. {
                        span = span.max(
                            (nodes[first].bounds.bottom() - nodes[second].bounds.y + th.gap) * 100.
                                / dy,
                        );
                    }
                }
            }
        }
        let headers = doc
            .header
            .as_ref()
            .map(|v| values(v).into_iter().cloned().collect::<Vec<_>>())
            .unwrap_or_default();
        let header_h = if headers.is_empty() {
            th.padding
        } else {
            headers
                .iter()
                .map(|s| {
                    s.split("··")
                        .map(|part| {
                            max_text(
                                ty,
                                &[part.trim().to_owned()],
                                th.font,
                                available / 2. - th.gap,
                                th.line_gap,
                            )
                            .1
                        })
                        .fold(0., f64::max)
                })
                .fold(0., f64::max)
                + 2. * th.padding
        };
        let notes = doc
            .note
            .as_ref()
            .map(|v| values(v).into_iter().cloned().collect::<Vec<_>>())
            .unwrap_or_default();
        let caption_h = max_text(ty, &notes, th.font, available, th.line_gap).1 + 2. * th.padding;
        let badges = doc
            .badge
            .as_ref()
            .map(|v| {
                values(v)
                    .into_iter()
                    .map(|s| s.to_uppercase())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let badge_h = if badges.is_empty() {
            0.
        } else {
            max_text(ty, &badges, th.font, available, th.line_gap).1 + 2. * th.gap
        };
        span = span.max(doc.canvas.min_height - header_h - top - below - badge_h - caption_h);
        if let Some(config) = &doc.structural {
            let arranged = structural_layout(config, available, &nodes, th, span)?;
            span = arranged.1;
            structure = Some(arranged);
        }
        let stage_top = header_h + top;
        let mut occupied = vec![];
        for (i, (n, a)) in nodes.iter().zip(&anchors).enumerate() {
            if let Some((points, _)) = &structure {
                occupied.push(
                    n.bounds
                        .moved(th.padding + points[i].0, stage_top + points[i].1),
                );
                continue;
            }
            occupied.push(Rect {
                x: margin + a.0 * stage_width / 100. + n.bounds.x,
                y: stage_top + a.2 * span / 100. + n.bounds.y,
                w: n.bounds.w + (a.1 - a.0) * stage_width / 100.,
                h: n.bounds.h + (a.3 - a.2) * span / 100.,
            });
        }
        let mut content_bottom = stage_top + span + below;
        for r in &occupied {
            content_bottom = content_bottom.max(r.bottom() + th.gap);
        }
        let mut annotations = vec![];
        for (t, strings) in text_inputs {
            let text_width = if t.left {
                available / 2. - th.gap
            } else {
                available - th.gap
            };
            let (w, h) = max_text(ty, &strings, th.font, text_width, th.line_gap);
            let (min_x, max_x) = range(&t.x);
            let (min_y, max_y) = range(&t.y);
            let x = margin + min_x * stage_width / 100.;
            let y = stage_top + min_y * span / 100.;
            let mut b = Rect {
                x: if t.left { x } else { x - w / 2. },
                y: y - ty.measure("Mg", th.font).height,
                w: w + (max_x - min_x) * stage_width / 100.,
                h: h + (max_y - min_y) * span / 100.,
            };
            b.x =
                b.x.clamp(th.padding, (width - th.padding - w).max(th.padding));
            b.y = b.y.max(header_h + th.gap);
            // Annotations have no attachment semantics in v1/v2. Preserve their x
            // intent and choose the nearest free vertical slot below their anchor.
            loop {
                let hit = occupied
                    .iter()
                    .filter(|r| b.overlaps(**r, th.gap))
                    .map(|r| r.bottom() + th.gap)
                    .fold(b.y, f64::max);
                if hit <= b.y {
                    break;
                }
                b.y = hit;
            }
            occupied.push(b);
            content_bottom = content_bottom.max(b.bottom() + th.gap);
            annotations.push(Annotation {
                bounds: Rect { w, h, ..b },
                dx: b.x - x,
                dy: b.y - y,
            });
        }
        let mut tree = TaffyTree::new();
        tree.disable_rounding();
        let header = leaf(&mut tree, width, header_h);
        let stage = leaf(&mut tree, width, content_bottom - header_h);
        let badge = leaf(&mut tree, width, badge_h);
        let caption = leaf(&mut tree, width, caption_h);
        let page = column(&mut tree, &[header, stage, badge, caption], 0.);
        compute(&mut tree, page);
        let height = rect(&tree, page).h.ceil();
        if !height.is_finite() || height > 100000. {
            return Err("layout exceeds 100000 logical units; separate overlapping fixed anchors or use structural layout".into());
        }
        Ok(Self {
            canvas: doc.canvas,
            theme: th,
            width,
            height,
            stage_left: margin,
            stage_top,
            stage_width,
            stage_span: span,
            header: rect(&tree, header),
            badge: rect(&tree, badge),
            caption: rect(&tree, caption),
            nodes,
            annotations,
            structural_points: structure.map(|s| s.0),
        })
    }
    pub fn for_frame(f: &Frame, ty: &Typography) -> Self {
        let els = f
            .nodes
            .iter()
            .map(|n| {
                El::Node(crate::timeline::NodeEl {
                    id: None,
                    label: Some(Tl::Const(n.label.clone())),
                    icon: n.icon.clone(),
                    x: Tl::Const(n.x),
                    y: Tl::Const(n.y),
                    status: n.status.clone().map(Tl::Const),
                    lifeline: n.lifeline,
                    show: None,
                })
            })
            .chain(f.texts.iter().map(|t| {
                El::Text(crate::timeline::TextEl {
                    text: Tl::Const(t.text.clone()),
                    x: Tl::Const(t.x),
                    y: Tl::Const(t.y),
                    dim: Tl::Const(t.dim),
                    left: t.left,
                    show: None,
                })
            }))
            .collect();
        Self::new(
            &Doc {
                structural: None,
                canvas: Canvas::default(),
                duration: None,
                header: f.header.clone().map(Tl::Const),
                badge: f.badge.clone().map(Tl::Const),
                note: Some(Tl::Const(f.note.clone())),
                els,
            },
            ty,
        )
        .expect("finite frame layout")
    }
}
/// Clip a shared straight route against its endpoint boxes.
pub fn boundary(rect: Rect, towards: (f64, f64)) -> (f64, f64) {
    let (x, y) = rect.center();
    let (dx, dy) = (towards.0 - x, towards.1 - y);
    if dx.abs() + dy.abs() < f64::EPSILON {
        return (x, y);
    }
    let t = (if dx == 0. {
        f64::INFINITY
    } else {
        rect.w / 2. / dx.abs()
    })
    .min(if dy == 0. {
        f64::INFINITY
    } else {
        rect.h / 2. / dy.abs()
    });
    (x + dx * t, y + dy * t)
}

/// Place a measured floating label in the nearest free slot. Candidate slots
/// come from obstacle edges, so no scene-specific offsets or search step exist.
pub fn place_near(wanted: Rect, obstacles: &[Rect], viewport: Rect, gap: f64) -> Option<Rect> {
    if wanted.w > viewport.w || wanted.h > viewport.h {
        return None;
    }
    let clamp = |r: Rect| Rect {
        x: r.x.clamp(viewport.x, viewport.right() - r.w),
        y: r.y.clamp(viewport.y, viewport.bottom() - r.h),
        ..r
    };
    let mut candidates = vec![clamp(wanted)];
    for obstacle in obstacles {
        candidates.extend([
            clamp(Rect {
                x: obstacle.x - gap - wanted.w,
                ..wanted
            }),
            clamp(Rect {
                x: obstacle.right() + gap,
                ..wanted
            }),
            clamp(Rect {
                y: obstacle.y - gap - wanted.h,
                ..wanted
            }),
            clamp(Rect {
                y: obstacle.bottom() + gap,
                ..wanted
            }),
        ]);
    }
    candidates
        .into_iter()
        .filter(|r| !obstacles.iter().any(|b| r.overlaps(*b, gap)))
        .min_by(|a, b| {
            ((a.x - wanted.x).powi(2) + (a.y - wanted.y).powi(2))
                .total_cmp(&((b.x - wanted.x).powi(2) + (b.y - wanted.y).powi(2)))
        })
}
#[derive(Deserialize, Clone, Copy, Debug)]
#[serde(rename_all = "snake_case")]
pub enum LayoutMode {
    Flow,
    Grid,
    Columns,
}
#[derive(Deserialize, Clone, Debug)]
#[serde(untagged)]
pub enum Structural {
    Name(LayoutMode),
    Options(StructuralOptions),
}
#[derive(Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct StructuralOptions {
    mode: LayoutMode,
    columns: Option<usize>,
}
impl Structural {
    fn configuration(&self, len: usize) -> Result<(LayoutMode, usize), String> {
        let (mode, columns) = match self {
            Self::Name(m) => (*m, None),
            Self::Options(o) => (o.mode, o.columns),
        };
        if columns == Some(0) || columns.is_some_and(|n| n > len.max(1)) {
            return Err("layout.columns must be between 1 and the node count".into());
        }
        Ok((
            mode,
            columns.unwrap_or_else(|| (len.max(1) as f64).sqrt().ceil() as usize),
        ))
    }
    fn node_width(&self, available: f64, len: usize, th: Theme) -> Result<f64, String> {
        let (mode, columns) = self.configuration(len)?;
        let width = match mode {
            LayoutMode::Flow => available.min(th.node_text_width),
            LayoutMode::Grid | LayoutMode::Columns => {
                (available - (columns - 1) as f64 * th.gap * 2.) / columns as f64
            }
        };
        if width < th.icon {
            return Err("canvas width is too narrow for layout.columns; use fewer columns or increase width".into());
        }
        Ok(width)
    }
}
fn range(t: &Tl<f64>) -> (f64, f64) {
    values(t)
        .iter()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(min, max), v| {
            (min.min(**v), max.max(**v))
        })
}
/// The returned anchors are in physical content coordinates, without a second
/// normalization. The same measured leaves determine wrapping and placement.
fn structural_layout(
    config: &Structural,
    available: f64,
    shapes: &[NodeBox],
    th: Theme,
    min_height: f64,
) -> Result<(Vec<(f64, f64)>, f64), String> {
    let (mode, count) = config.configuration(shapes.len())?;
    if shapes.is_empty() {
        return Ok((vec![], 0.));
    }
    let mut tree = TaffyTree::new();
    tree.disable_rounding();
    let gap = th.gap * 2.;
    let nodes = shapes
        .iter()
        .map(|s| leaf(&mut tree, s.bounds.w, s.bounds.h))
        .collect::<Vec<_>>();
    let mut style = Style {
        min_size: Size {
            width: LengthPercentageAuto::auto(),
            height: LengthPercentageAuto::length(min_height as f32),
        },
        align_content: Some(AlignContent::SPACE_EVENLY),
        size: Size {
            width: Dimension::length(available as f32),
            height: Dimension::auto(),
        },
        align_items: Some(AlignItems::START),
        gap: Size {
            width: LengthPercentage::length(gap as f32),
            height: LengthPercentage::length(gap as f32),
        },
        ..Default::default()
    };
    match mode {
        LayoutMode::Flow => {
            style.display = Display::Flex;
            style.flex_direction = FlexDirection::Row;
            style.flex_wrap = FlexWrap::Wrap;
        }
        LayoutMode::Grid | LayoutMode::Columns => {
            style.display = Display::Grid;
            style.grid_template_columns = vec![fr(1.); count];
            style.justify_items = Some(AlignItems::CENTER);
        }
    }
    if matches!(mode, LayoutMode::Columns) {
        let rows = shapes.len().div_ceil(count);
        for (i, node) in nodes.iter().enumerate() {
            let mut item = tree.style(*node).map_err(|e| e.to_string())?.clone();
            item.grid_row = line((i % rows + 1) as i16);
            item.grid_column = line((i / rows + 1) as i16);
            tree.set_style(*node, item).map_err(|e| e.to_string())?;
        }
    }
    let root = tree
        .new_with_children(style, &nodes)
        .map_err(|e| e.to_string())?;
    compute(&mut tree, root);
    let positions = nodes
        .iter()
        .zip(shapes)
        .map(|(id, s)| {
            let b = rect(&tree, *id);
            (b.x + b.w / 2., b.y - s.bounds.y)
        })
        .collect::<Vec<_>>();
    for (point, shape) in positions.iter().zip(shapes) {
        let b = shape.bounds.moved(point.0, point.1);
        if b.x < -0.01 || b.right() > available + 0.01 {
            return Err("node cannot fit structural column width".into());
        }
    }
    Ok((positions, rect(&tree, root).h))
}
/// v1 compiles authored relationships into ordinary world coordinates. The
/// structural intent is also retained for preparation after canvas overrides.
pub fn structural_positions(
    config: &Structural,
    canvas: Canvas,
    items: &[(bool, String, Option<String>)],
) -> Result<Vec<(f64, f64)>, String> {
    if items.iter().any(|(_, label, status)| {
        !crate::typography::valid_text(label)
            || status
                .as_ref()
                .is_some_and(|s| !crate::typography::valid_text(s))
    }) {
        return Err("node text contains characters unsupported by SVG".into());
    }
    let ty = Typography::default();
    let th = Theme::default();
    let available = canvas.width - 2. * th.padding;
    let width = config.node_width(available, items.len(), th)?;
    let shapes = items
        .iter()
        .map(|(icon, label, status)| {
            node_box(
                &ty,
                th,
                *icon,
                std::slice::from_ref(label),
                &status.clone().into_iter().collect::<Vec<_>>(),
                width,
            )
        })
        .collect::<Vec<_>>();
    let (points, height) = structural_layout(config, available, &shapes, th, 0.)?;
    Ok(points
        .into_iter()
        .map(|(x, y)| (x / available * 100., y / height.max(1.) * 100.))
        .collect())
}
