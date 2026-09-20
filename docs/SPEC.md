# dynamic-diagram spec reference

A spec is a JSON document describing one diagram. Static specs (no `duration`)
render a single frame with packets parked at their `progress`; animated specs
(`"duration": ms`) loop forever, each packet flying `from → to` inside its
`window`.

```json
{
  "header":  "REQUEST PATH ·· CLIENT → LB → SERVICE",
  "duration": 8000,
  "badge":   "8s loop",
  "note":    "Caption at the bottom, wrapped by measured text width.",
  "nodes": [
    { "id": "client", "label": "client", "icon": "phone",
      "x": 8, "y": 50, "status": "seq 5000", "lifeline": false },
    { "id": "lb", "label": "load balancer", "icon": "router", "x": 80, "y": 50 }
  ],
  "links":   [ { "from": "client", "to": "lb", "arrow": "end" } ],
  "packets": [ { "label": "GET /api", "from": "client", "to": "lb",
                 "progress": 0.4, "window": [0.02, 0.26], "faded": false } ],
  "texts":   [ { "text": "dmz", "x": 62, "y": 50, "dim": true, "left": false } ]
}
```

## Fields

| field | type | meaning |
|-------|------|---------|
| `header` | string? | top bar; `··` splits left/right sides |
| `duration` | u64? | ms per loop; presence switches to animated mode |
| `badge` | string? | bottom-center label, uppercased |
| `note` | string? | measured, wrapped bottom caption; height follows content |
| `canvas` | object? | `width` (760), `min_height` (330), `scale` (1); applies to v1 and v2 |
| `layout` | string/object? | v1 structural placement: `flow`, `grid`, `columns`, or `{ "mode": "grid", "columns": 3 }` |
| `nodes[]` | | `id?` (else index), `label?`, `icon?`, `x`, `y` (0–100, omitted with structural layout), `status?`, `lifeline?` (default false; dashed vertical line under the node) |
| `links[]` | | `from`, `to` (node ids), `arrow?` = `""` \| `"end"` \| `"both"` |
| `packets[]` | | `label`, `from`, `to`, `progress?` (0–1, static mode, default 0.5), `window?` = `[start, end]` loop fractions (animated), `faded?` (landed style) |
| `texts[]` | | `text`, `x`, `y`, `dim?`, `left?` (default: centered) |

Static mode without `progress` parks a packet mid-flight (`0.5`). Animated
mode without `window` assigns staggered defaults (`0.06+0.06i → 0.55+0.06i`).
Moving packets are hidden at their endpoints. A label that would overlap a
node moves to a nearby free slot, with a leader marking its actual position;
labels are omitted only when no slot fits. ID-based links and packets share
routes clipped to measured node bounds. Raw-coordinate v2 elements retain
their world-coordinate paths.

## Universal timeline format (v2)

Below the v1 sugar sits one universal mechanism: **elements whose properties
are timelines**. Any property accepts either a constant or `[[t, v], ...]`
keyframes — numbers/points interpolate linearly between keys, strings/bools
hold until the next key. No expressions, ever.

```json
{
  "duration": 9000,
  "els": [
    { "type": "node", "id": "gw", "label": "router", "icon": "router",
      "x": 50, "y": 40,
      "status": [[0, "down"], [3000, "up"]],
      "show": [[0, false], [1500, true]] },
    { "type": "line", "x1": 10, "y1": 40, "x2": 50, "y2": 40, "arrow_end": true },
    { "type": "packet", "label": "GET", "x1": 10, "y1": 40, "x2": 50, "y2": 40,
      "p": [[0, 0], [1500, 0], [4000, 1]] },
    { "type": "text", "text": [[0, "connecting…"], [3000, "online"]], "x": 50, "y": 60 },
    { "type": "polyline", "points": [0, 50, 25, 20, 50, 50], "dim": false,
      "slice": [0, 3] },
    { "type": "path", "d": "M…" }
  ],
  "badge": [[0, "…"], [9000, "…"]],
  "note": "caption"
}
```

- Element kinds: `node` (icon/label/status/lifeline), `line` (coords or
  `from`/`to` node ids — id endpoints track moving nodes), `packet` (optional
  `from`/`to` references, label +
  flight geometry + `p` progress timeline + `landed`), `text`, `polyline`
  (flat points array, optional moving `slice` [from,to) for reveals), `path`.
- `show` (step timeline of bool) on any element; `""` in a string timeline
  means absent (status/badge/note).
- v1 `window: [w0, w1]` is literally two keyframes on `p`; the compiler emits
  `[[0,0],[w0·D,0],[w1·D,1],[D,1]]`.
- The 28 built-in sims are v2 documents (`sims/*.json`, embedded at build).

## Animation patterns (verbs)

Picking a choreography is as cheap as picking an icon: one `anim` verb
compiles to the same keyframes a hand-written window list would produce.
Requires `duration`. Verbs override per-packet `window`s; custom stories use
explicit windows or keyframes.

| verb | choreography | fits |
|------|--------------|------|
| `seq` | strict relay — one packet at a time, each in its own slot | pipelines, encapsulation chains, handshakes |
| `fanout` | halves — first half of the packet list flies out (slight ripple), second half flies back | fan-out/fan-in, request/response, load balancing |
| `flood` | one simultaneous block | broadcasts, pub/sub |
| `flip` | node statuses reveal in node order (state spreads); packets keep default stagger | state machines, convergence stories |

```json
{ "duration": 8000, "anim": "fanout", "nodes": [...], "packets": [
    { "label": "GET", "from": "client", "to": "lb" },
    { "label": "fwd", "from": "lb", "to": "a" },
    { "label": "fwd", "from": "lb", "to": "b" },
    { "label": "resp", "from": "a", "to": "lb" },
    { "label": "resp", "from": "b", "to": "lb" },
    { "label": "200", "from": "lb", "to": "client" } ] }
```
(see `examples/fanout.json`; unknown verbs are a parse error, `anim` without
`duration` is a parse error)

## Output modes

```
dynamic-diagram spec <f.json> [png]      # single frame -> <f>.png (density 3)
dynamic-diagram spec <f.json> svg        # SVG to stdout
dynamic-diagram spec <f.json> info       # JSON width, height, duration; no PNG
dynamic-diagram spec <f.json> kitty      # single frame inline (kitty protocol)
dynamic-diagram spec <f.json> frames <dir> [n]   # default ceil(duration × fps / 1000)
dynamic-diagram spec <f.json> kitty-anim # stream frames until Ctrl+C
dynamic-diagram spec --list-icons        # all 11,831 icon names
dynamic-diagram skill                    # print skills/dynamic-diagram/SKILL.md
```

Options: `--width N`, `--height N` (minimum height), `--scale N`, `--density N`,
`--fps N` (default 30, range 1..120), `--at MS` (single-frame sample time).
Sizes/density/fps must be positive and finite. `DDA_SCALE` remains a compatible
default for raster density; `--density` overrides it. Exit codes: 0 success,
2 usage/parse/render error.

Frame export writes `00001.png`, …, and `frames.json` with `files`, `count`,
`duration`, `fps`, `width`, `height`. Read the manifest to identify the current
export; unrelated/older files in that directory are preserved. An explicit
count must be 1..1800; derived counts are capped at 1800 and still cover the
whole duration. PNGs are limited to 32 Mi pixels each; actual process memory
also includes parsing, encoding and a bounded 16 MiB PNG cache.

## Layout and size

New v1 diagrams can delegate placement to Taffy:

```json
{
  "layout": { "mode": "grid", "columns": 2 },
  "canvas": { "min_height": 660, "scale": 1 },
  "nodes": [
    { "id": "web", "icon": "phone", "label": "client", "status": "ready" },
    { "id": "api", "icon": "dns", "label": "API", "status": "healthy" }
  ],
  "links": [{ "from": "web", "to": "api", "arrow": "end" }]
}
```

`flow` wraps nodes in document order; `grid` fills rows; `columns` fills
columns. Explicit `x`/`y` placement remains available when `layout` is absent.
Node boxes include icons, wrapped labels and badges. Fonts and spacing come
from shared theme metrics. Header, annotations, badge and caption reserve
their own measured space; the canvas grows to fit. Rendering prepares the
whole timeline, reserving each node's text states so status changes do not
move surrounding content.

`canvas.width` is the logical width (100..100000), `min_height` is a lower
bound (up to 100000), and `scale` multiplies both displayed dimensions (up to
64). For more space in a width-constrained terminal, request a taller canvas
with `min_height`/`--height`. For 1.5× or 2× proportional presentation, use
`scale`/`--scale`. `--density` only changes PNG sampling; hosts may fit the
result to their viewport, independently of its pixel resolution.

Text measurement and rasterization share embedded Inconsolata and Liberation
Sans. They do not provide general CJK/emoji coverage. SVG consumers need the
same fonts for matching metrics; PNG embeds the resulting pixels. Arbitrary
v2 paths and moving raw-coordinate elements are author-controlled: inspect
representative frames for crossings and clipping.

## Icon resolution order

cloud (namespaced or aliased) → Material Symbols (bare name) → Tabler
(`ti/` prefix forces the tier; bare names fall through if Material lacks them)
→ flowchart shapes → dashed "?" placeholder. Material paths are 960×960
y-negative-up grids; Tabler are 24×24 stroke paths; scaling lives in
`src/svg.rs::emit_icon`.
