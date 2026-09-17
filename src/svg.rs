//! SVG renderer: Frame -> standalone SVG string, styled after fazamhd.com's CSS
//! (mono bordered message boxes, dashed seq badges, dashed lifelines, ink/dim palette).
//! Any GUI that can show SVG works (browser DOM, <img>, desktop viewer, or rasterize).

use crate::frame::Frame;

pub const W: f64 = 760.0;
pub const H: f64 = 330.0;
const HEADER_H: f64 = 36.0;
const STAGE_H: f64 = 216.0; // stage: y in [HEADER_H, HEADER_H+STAGE_H]
// fazamhd.com's real palette (light theme, PageLayout.css :root)
const INK: &str = "#1b1f2a";
const DIM: &str = "#4a5163";
const LINE: &str = "#c2c9da";
const FAINT: &str = "#5b6274";
const PAGE: &str = "#fafbfd";
const HOVER: &str = "#1b1f2a0f";
const FONT_MONO: &str = "Inconsolata"; // single name: usvg can't parse comma lists

fn sx(x: f64) -> f64 {
    x / 100.0 * W
}
fn sy(y: f64) -> f64 {
    HEADER_H + y / 100.0 * STAGE_H
}

// icon glyph at (x,y). Four tiers:
//   1. software-engineering structural glyphs (stack, queue, pool, heap…)
//   2. Material Symbols full library (3,912) + aliases
//   3. cloud provider official icons (aws/*, azure/*, gcp/*, cf/*) — full color
//   4. flowchart shapes (box, cylinder, diamond…)
fn icon_glyph(out: &mut String, icon: &str, x: f64, y: f64) {
    // aliases into material
    let mkey = match icon {
        "server" => "dns",          // stacked-servers look
        "client" | "phone" => "ti/device-mobile", // material "smartphone" is absent from the table
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
        "laptop" => "laptop_mac",   // "laptop" has no symbol; mac variant stands in
        "sd_storage" => "storage",
        "https" => "lock",
        "restore" => "autorenew",
        "source" => "code",
        "gpp_good" => "verified_user",
        other => other,
    };
    if let Some(d) = crate::assets::icons::get(mkey) {
        emit_icon(out, d, x, y);
        return;
    }
    // brand fallback: "docker" → ti/brand-docker (376 tabler brand glyphs)
    if let Some(d) = crate::assets::icons::get(&format!("brand-{icon}")) {
        emit_icon(out, d, x, y);
        return;
    }
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
            esc(icon)
        ),
    };
    out.push_str(&frag);
}

/// draw a resolved icon glyph centered at (x,y)
fn emit_icon(out: &mut String, d: crate::assets::icons::Icon, x: f64, y: f64) {
    match d {
        crate::assets::icons::Icon::Path(d) => {
            // material fill paths: 960x960 grid, y negative-up (viewBox 0 -960 960 960);
            // center (480,-480) and scale to ~34px like the other tiers
            out.push_str(&format!(
                "<g transform=\"translate({x} {y}) scale(0.0354) translate(-480 480)\"><path d=\"{d}\" fill=\"{INK}\"/></g>\n"
            ));
        }
        crate::assets::icons::Icon::Stroke(d) => {
            // tabler stroke paths: 24x24 viewbox, fill=none + ink stroke
            out.push_str(&format!(
                "<g transform=\"translate({x} {y}) scale(1.4) translate(-12 -12)\"><path d=\"{d}\" fill=\"none\" stroke=\"{INK}\" stroke-width=\"1.7\" stroke-linecap=\"round\" stroke-linejoin=\"round\"/></g>\n"
            ));
        }
        crate::assets::icons::Icon::Svg(vb, inner) => {
            // full-color provider icon: nested <svg> scaled into a 34x34 box
            out.push_str(&format!(
                "<svg x=\"{}\" y=\"{}\" width=\"34\" height=\"34\" viewBox=\"{vb}\" overflow=\"visible\">{inner}</svg>\n",
                x - 17.0,
                y - 17.0
            ));
        }
    }
}

/// status pill centered on x, box top edge at `top`
fn status_badge(out: &mut String, x: f64, top: f64, status: &str) {
    let pending = status.contains("pending");
    let bw = status.chars().count() as f64 * 6.0 + 16.4;
    let (fill, stroke, dash, op, txt) = if pending {
        (PAGE, LINE, "3 3", 0.5, DIM)
    } else {
        (HOVER, INK, "none", 1.0, INK)
    };
    out.push_str(&format!("<g opacity=\"{op}\">\n"));
    out.push_str(&format!(
        "<rect x=\"{}\" y=\"{top}\" width=\"{bw}\" height=\"20\" rx=\"3\" fill=\"{fill}\" stroke=\"{stroke}\" stroke-dasharray=\"{dash}\"/>\n",
        x - bw / 2.0
    ));
    out.push_str(&format!(
        "<text x=\"{x}\" y=\"{}\" text-anchor=\"middle\" fill=\"{txt}\" font-size=\"12\" font-family=\"{FONT_MONO}\">{}</text>\n",
        top + 14.0,
        esc(status)
    ));
    out.push_str("</g>\n");
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

// wrap caption into <=2 lines (SVG text doesn't wrap); char-safe (CJK/superscripts)
fn wrap(s: &str, max_chars: usize) -> Vec<&str> {
    if s.chars().count() <= max_chars {
        return vec![s];
    }
    // byte offset of the max_chars-th char (never mid-char)
    let limit = s.char_indices().nth(max_chars).map(|(i, _)| i).unwrap_or(s.len());
    match s[..limit].rfind(' ') {
        Some(i) => vec![&s[..i], &s[i + 1..]],
        None => vec![s],
    }
}

pub fn render_svg(f: &Frame) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {} {}\" font-family=\"{FONT_MONO}\">\n",
        W as u32, H as u32
    ));
    out.push_str(&format!("<rect width=\"{}\" height=\"{}\" fill=\"{PAGE}\"/>\n", W as u32, H as u32));
    out.push_str(&format!("<rect x=\"0.5\" y=\"0.5\" width=\"{}\" height=\"{}\" rx=\"8\" fill=\"none\" stroke=\"{LINE}\"/>\n", W - 1.0, H - 1.0));

    // header bar
    if let Some(header) = &f.header {
        let mut parts = header.split("··").map(|s| s.trim());
        let left = parts.next().unwrap_or("");
        let right = parts.next().unwrap_or("");
        out.push_str(&format!(
            "<text x=\"14\" y=\"23\" fill=\"{DIM}\" font-size=\"12\" letter-spacing=\"2.16\">{}</text>\n",
            esc(left)
        ));
        out.push_str(&format!(
            "<text x=\"{}\" y=\"23\" text-anchor=\"end\" fill=\"{DIM}\" font-size=\"12\" letter-spacing=\"2.16\">{}</text>\n",
            W - 14.0,
            esc(right)
        ));
        out.push_str(&format!("<line x1=\"0\" y1=\"{HEADER_H}\" x2=\"{W}\" y2=\"{HEADER_H}\" stroke=\"{LINE}\"/>\n"));
    }

    // lifelines (dashed vertical under nodes)
    for n in &f.nodes {
        if !n.lifeline {
            continue;
        }
        out.push_str(&format!(
            "<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" stroke=\"{LINE}\" stroke-dasharray=\"4 4\"/>\n",
            sx(n.x),
            sy(n.y) + 44.0,
            sx(n.x),
            sy(100.0) - 4.0
        ));
    }

    // raw SVG paths (scene coords 0..100, scaled into the stage; fill = land color)
    if !f.paths.is_empty() {
        out.push_str(&format!(
            "<g transform=\"translate(0,{}) scale({} {})\">\n",
            HEADER_H,
            W / 100.0,
            STAGE_H / 100.0
        ));
        for d in &f.paths {
            out.push_str(&format!("<path d=\"{}\" fill=\"{LINE}\" stroke=\"none\"/>\n", d));
        }
        out.push_str("</g>\n");
    }

    // polylines (waveforms, graphs)
    for pl in &f.polylines {
        let pts: Vec<String> = pl.points.iter().map(|(x, y)| format!("{},{}", sx(*x), sy(*y))).collect();
        out.push_str(&format!(
            "<polyline points=\"{}\" fill=\"none\" stroke=\"{}\" stroke-width=\"1.4\"/>\n",
            pts.join(" "),
            if pl.dim { LINE } else { INK }
        ));
    }

    // free-floating annotations
    for t in &f.texts {
        out.push_str(&format!(
            "<text x=\"{}\" y=\"{}\" text-anchor=\"{}\" fill=\"{}\" font-size=\"12\">{}</text>\n",
            sx(t.x),
            sy(t.y),
            if t.left { "start" } else { "middle" },
            if t.dim { DIM } else { INK },
            esc(&t.text)
        ));
    }

    // trails (landed packet paths) / static links; optional arrowhead at the end
    for t in &f.trails {
        out.push_str(&format!(
            "<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" stroke=\"{LINE}\" opacity=\"0.6\"/>\n",
            sx(t.x1),
            sy(t.y1),
            sx(t.x2),
            sy(t.y2)
        ));
        if t.arrow_end {
            let (x1, y1, x2, y2) = (sx(t.x1), sy(t.y1), sx(t.x2), sy(t.y2));
            let (dx, dy) = (x2 - x1, y2 - y1);
            let len = (dx * dx + dy * dy).sqrt().max(1.0);
            let (ux, uy) = (dx / len, dy / len);
            let (px, py) = (-uy, ux); // perpendicular
            out.push_str(&format!(
                "<path d=\"M {x2} {y2} L {} {} L {} {} Z\" fill=\"{INK}\"/>\n",
                x2 - ux * 9.0 + px * 4.0,
                y2 - uy * 9.0 + py * 4.0,
                x2 - ux * 9.0 - px * 4.0,
                y2 - uy * 9.0 - py * 4.0
            ));
        }
    }

    // packets: bordered mono box, faded when landed; lerps both axes (zigzag paths)
    for p in &f.packets {
        // a travelling packet parked on an endpoint is invisible (not launched yet /
        // already arrived) — drawing it there stacks its label onto the node.
        // zero-length "pill resting at a host" packets (arp & friends) are exempt.
        let travel = (p.x2 - p.x1).abs() + (p.y2 - p.y1).abs() > 1.0;
        if travel && !p.landed && (p.p <= 0.02 || p.p >= 0.98) {
            continue;
        }
        let x = sx(p.x1 + (p.x2 - p.x1) * p.p);
        let y = sy(p.y1 + (p.y2 - p.y1) * p.p);
        let bw = p.label.chars().count() as f64 * 6.0 + 20.0; // Inconsolata 12px: 0.5em advance
        let op = if p.landed { 0.55 } else { 1.0 };
        out.push_str(&format!("<g opacity=\"{op}\">\n"));
        out.push_str(&format!(
            "<rect x=\"{}\" y=\"{}\" width=\"{bw}\" height=\"22\" rx=\"3\" fill=\"{PAGE}\" stroke=\"{FAINT}\"/>\n",
            x - bw / 2.0,
            y - 11.0
        ));
        out.push_str(&format!(
            "<text x=\"{x}\" y=\"{}\" text-anchor=\"middle\" fill=\"{INK}\" font-size=\"12\" font-family=\"{FONT_MONO}\">{}</text>\n",
            y + 4.0,
            esc(&p.label)
        ));
        out.push_str("</g>\n");
    }

    // nodes + seq badges (empty label = bare router square marker; icon = glyph + label below)
    for n in &f.nodes {
        let x = sx(n.x);
        let y = sy(n.y);
        if let Some(icon) = &n.icon {
            icon_glyph(&mut out, icon, x, y);
            if !n.label.is_empty() {
                out.push_str(&format!(
                    "<text x=\"{x}\" y=\"{}\" text-anchor=\"middle\" fill=\"{INK}\" font-size=\"13\" letter-spacing=\"1.04\">{}</text>\n",
                    y + 30.0,
                    esc(&n.label)
                ));
            }
            // status sits BELOW the label (label occupies ~y+20..y+30)
            if let Some(status) = &n.status {
                status_badge(&mut out, x, y + 40.0, status);
            }
            continue; // icon nodes draw their label below the glyph — never fall through
        }
        if n.label.is_empty() {
            out.push_str(&format!(
                "<rect x=\"{}\" y=\"{}\" width=\"10\" height=\"10\" rx=\"2\" fill=\"{PAGE}\" stroke=\"{FAINT}\"/>\n",
                x - 5.0,
                y - 5.0
            ));
            continue;
        }
        out.push_str(&format!(
            "<text x=\"{x}\" y=\"{y}\" text-anchor=\"middle\" fill=\"{INK}\" font-size=\"13\" letter-spacing=\"1.04\">{}</text>\n",
            esc(&n.label)
        ));
        if let Some(status) = &n.status {
            status_badge(&mut out, x, y + 11.0, status);
        }
    }

    // established badge
    if let Some(badge) = &f.badge {
        out.push_str(&format!(
            "<text x=\"{}\" y=\"{}\" text-anchor=\"middle\" fill=\"{INK}\" font-size=\"12\" letter-spacing=\"3\">{}</text>\n",
            W / 2.0,
            sy(100.0) + 2.0,
            esc(&badge.to_uppercase())
        ));
    }

    // caption bar (wrapped, dim)
    let lines = wrap(&f.note, 92);
    let cap_top = HEADER_H + STAGE_H + 24.0;
    out.push_str(&format!(
        "<line x1=\"0\" y1=\"{}\" x2=\"{W}\" y2=\"{}\" stroke=\"{LINE}\"/>\n",
        HEADER_H + STAGE_H + 8.0,
        HEADER_H + STAGE_H + 8.0
    ));
    for (i, s) in lines.iter().enumerate() {
        out.push_str(&format!(
            "<text x=\"14\" y=\"{}\" fill=\"{DIM}\" font-size=\"12\">{}</text>\n",
            cap_top + i as f64 * 16.0,
            esc(s)
        ));
    }

    out.push_str("</svg>\n");
    out
}
