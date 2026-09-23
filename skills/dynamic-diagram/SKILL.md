---
name: dynamic-diagram
description: Render architecture, data-flow, and runtime diagrams as polished images from a JSON spec, via the dynamic-diagram CLI (11,831 icons, PNG/SVG/kitty, animation). Use when a visual would clarify system structure or message flow, when writing or fixing a dynamic-diagram spec, and when choosing icon names or animation windows.
---

# dynamic-diagram

A spec (JSON) describes the picture; the `dynamic-diagram` CLI renders it.
Write spec → render → inspect the PNG → adjust layout intent → re-render.
Node boxes and page bands use measured text and shared theme metrics.

## Steps

1. **Draft the scene.** Prefer `"layout": "flow"`, `"grid"`, or
   `{"mode":"columns","columns":3}` with coordinate-free nodes
   (`{id,label,icon,status?}`). Use explicit `x,y` in 0..100 scene space only
   when relative placement matters; omit `layout` in that case. Connect node
   ids using links and packets. Done when each endpoint names an existing id
   and the chosen layout expresses the flow.
2. **Pick icons** (optional). Search before guessing:
   `dynamic-diagram spec --list-icons | rg <substr>`. An unknown name renders
   a dashed "?" placeholder — a typo shows, never silently. Done when each
   icon name returns a search hit (or you drop the icon field for label-only).
3. **Write the spec.** Minimal shape:
   ```json
   {
     "header": "TITLE ·· RIGHT SIDE",
     "layout": "flow",
     "nodes":  [ { "id": "web", "label": "browser", "icon": "phone" },
                   { "id": "lb", "label": "gateway", "icon": "router" } ],
     "links":  [ { "from": "web", "to": "lb", "arrow": "end" } ],
     "packets": [ { "label": "GET /api", "from": "web", "to": "lb", "window": [0.02, 0.26] } ],
     "badge": "live", "note": "one caption line"
   }
   ```
   For animation add `"duration": 6000` (ms per loop). Prefer an `anim` verb
   over hand-tuned windows — picking a choreography is as cheap as picking an
   icon: `seq` (one-at-a-time relay), `fanout` (out-half then back-half of the
   packet list), `flood` (all at once), `flip` (statuses reveal in node order).
   Only hand-write `"window": [start, end]` loop fractions when no verb fits.
   Done when `dynamic-diagram spec f.json` exits 0.
4. **Render and re-read.** `dynamic-diagram spec f.json` writes `f.png`;
   output modes: `svg` (stdout), `kitty` (inline to terminal), `frames <dir>
   [n]`, `kitty-anim`. `info` reports display width/height/duration without a
   PNG. Inspect animated scenes at representative times using `--at MS`.
   Done when text fits, routes read clearly and status changes remain stable.

## Size and layout

- Canvas height follows measured content. `"canvas":{"min_height":660}`
  requests extra vertical room; labels/statuses determine their own boxes.
- `"canvas":{"scale":2}` or `--scale 2` doubles the whole presentation.
  `--density 2` controls PNG resolution independently. Width-constrained
  hosts fit the image to their viewport, so use height for a taller inline view.
- Fixed coordinates express relative world placement. For dense diagrams,
  choose structural layout or increase width; inspect raw v2 paths and moving
  objects for crossings. ID-based links/packets share measured endpoint bounds.
- Fonts are embedded Inconsolata and Liberation Sans; general CJK and emoji
  are outside their glyph coverage. Use supported labels for portable PNGs.
- Export counts follow duration/fps (default 30fps, capped at 1800 frames), or
  pass an explicit count. Read `frames.json` for the files in that export.

## Icon tiers

1. Material Symbols (3,912, filled) — default tier, no prefix: `router`, `dns`,
   `cloud`, `storage`, `security`, `cell_tower`, `factory`, `shuffle`, …
2. Cloud officials (full color): `aws/…`, `azure/…`, `gcp/…`, `cf/…` plus
   short aliases `ec2 s3 lambda dynamodb sqs sns iam vpc cloudfront route53
   azure_vm bigquery cloud_run gce cloudflare workers r2`.
3. Tabler (5,130, stroke style): prefix `ti/` — `ti/cpu`, `ti/container`,
   `ti/topology-star`, `ti/hierarchy`, `ti/brand-docker` (or bare `docker`).
4. Flowchart shapes (semantic): `box cylinder ellipse diamond hexagon stadium
   triangle subroutine`.

Common aliases: `server→dns`, `phone|client|smartphone→ti/device-mobile`, `firewall→security`,
`database|db→storage`, `globe→public`, `switch→settings_ethernet`.

This file is the full authoring reference; re-read it anywhere with `dynamic-diagram skill`.

Beyond the sugar, the universal escape hatch is **keyframe timelines**: any
property (x, y, status, text, badge, visibility) accepts `[[t, v], ...]` —
numbers interpolate linearly, strings step. Use it for state changes over time
(status flips, appearing nodes, multi-phase stories) without touching any
special field.
