---
name: dynamic-diagram
description: Render architecture, data-flow, and runtime diagrams as polished images from a JSON spec, via the dynamic-diagram CLI (11,831 icons, PNG/SVG/kitty, animation). Use when a visual would clarify system structure or message flow, when writing or fixing a dynamic-diagram spec, and when choosing icon names or animation windows.
---

# dynamic-diagram

A spec (JSON) describes the picture; the `dynamic-diagram` CLI renders it.
Any agent can drive it: write spec → render → read the PNG → fix coordinates →
re-render. One uniform font (Inconsolata), site-grade styling, no design skill
needed.

## Steps

1. **Draft the scene.** The stage is 0..100 on both axes; `y` grows downward.
   Place nodes (`{id,label,icon,x,y,status?}`), connect with links
   (`{from,to,arrow}`), and add packets that fly between nodes. Left-to-right
   flow reads best; fan-outs stack vertically around a midline. Done when every
   node has coordinates and every from/to names a real node id.
2. **Pick icons** (optional). Search before guessing:
   `dynamic-diagram spec --list-icons | grep <substr>`. An unknown name renders
   a dashed "?" placeholder — a typo shows, never silently. Done when each
   icon name returns a search hit (or you drop the icon field for label-only).
3. **Write the spec.** Minimal shape:
   ```json
   {
     "header": "TITLE ·· RIGHT SIDE",
     "nodes":  [ { "id": "web", "label": "browser", "icon": "phone", "x": 10, "y": 50 } ],
     "links":  [ { "from": "web", "to": "lb", "arrow": "end" } ],
     "packets": [ { "label": "GET /api", "from": "web", "to": "lb", "window": [0.02, 0.26] } ],
     "badge": "live", "note": "one caption line"
   }
   ```
   For animation add `"duration": 6000` (ms per loop) and give each packet a
   `"window": [start, end]` — loop fractions 0..1 when that packet flies.
   Space windows so related packets overlap slightly (fan-out) but the scene
   never has more than ~3 packets in flight. Done when `dynamic-diagram spec
   f.json` exits 0.
4. **Render and re-read.** `dynamic-diagram spec f.json` writes `f.png`;
   output modes: `svg` (stdout), `kitty` (inline to terminal), `frames <dir>
   [n]`, `kitty-anim`. Look at the PNG yourself — move nodes that collide,
   re-render. Done when labels never overlap links or the caption divider and
   the flow reads at a glance.

## Geometry rules

- An icon node stacks glyph (34px) + label + optional status badge ≈ 60 units
  below its `y`. Keep icon-with-status nodes at **y ≤ 75**; lower collides with
  the caption divider.
- Header band occupies the top 36px, caption band the bottom ~80px; only the
  stage between them holds the scene (`sy(y) = 36 + y% × 216`).
- Node labels sit ~30 units under the icon; leave that space empty on both
  sides of a node column.

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

Common aliases: `server→dns`, `phone|client→smartphone`, `firewall→security`,
`database|db→storage`, `globe→public`, `switch→settings_ethernet`.

Full field reference: `docs/SPEC.md` in the engine repo, or re-read this file
anywhere with `dynamic-diagram skill`.
