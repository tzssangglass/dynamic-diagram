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
  "note":    "Caption line at the bottom (wrapped at ~92 chars).",
  "nodes": [
    { "id": "client", "label": "client", "icon": "phone",
      "x": 8, "y": 50, "status": "seq 5000", "lifeline": false }
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
| `badge` | string? | bottom-center label (uppercased, letterspaced) |
| `note` | string? | bottom caption, wrapped to ≤ 2 lines |
| `nodes[]` | | `id?` (else index), `label?`, `icon?`, `x`, `y` (0–100), `status?`, `lifeline?` (default false; dashed vertical line under the node) |
| `links[]` | | `from`, `to` (node ids), `arrow?` = `""` \| `"end"` \| `"both"` |
| `packets[]` | | `label`, `from`, `to`, `progress?` (0–1, static mode, default 0.5), `window?` = `[start, end]` loop fractions (animated), `faded?` (landed style) |
| `texts[]` | | `text`, `x`, `y`, `dim?`, `left?` (default: centered) |

Static mode without `progress` parks a packet mid-flight (`0.5`). Animated
mode without `window` assigns staggered defaults (`0.06+0.06i → 0.55+0.06i`).
Packets at fraction ≤ 0.02 or ≥ 0.98 on a real travel path are not drawn
(resting at an endpoint would stack the label onto the node).

## Output modes

```
dynamic-diagram spec <f.json> [png]      # single frame -> <f>.png (3× scale)
dynamic-diagram spec <f.json> svg        # SVG to stdout
dynamic-diagram spec <f.json> kitty      # single frame inline (kitty protocol)
dynamic-diagram spec <f.json> frames <dir> [n]   # n loop frames (default 12)
dynamic-diagram spec <f.json> kitty-anim # stream frames until Ctrl+C
dynamic-diagram spec --list-icons        # all 11,831 icon names
dynamic-diagram skill                    # print skills/dynamic-diagram/SKILL.md
```

Exit codes: 0 ok, 2 usage/parse error. Env: `DDA_SCALE` raster scale
(default 3; memory ≈ 25MB @3, 16MB @2, 10MB @1 in the child process).

## Canvas geometry (SVG units)

Canvas 760×330: header band 36px, stage 216px, caption band below.
`sy(y) = 36 + y/100 × 216`. Icon nodes stack glyph (34px) + label (+30) +
status badge (+40, height 20) ≈ 60 stage-units below `y` — keep icon nodes
with a status badge at **y ≤ 75** so the badge clears the caption divider at
y = 260. All text renders in Inconsolata (embedded; usvg requires single font
names).

## Icon resolution order

cloud (namespaced or aliased) → Material Symbols (bare name) → Tabler
(`ti/` prefix forces the tier; bare names fall through if Material lacks them)
→ flowchart shapes → dashed "?" placeholder. Material paths are 960×960
y-negative-up grids; Tabler are 24×24 stroke paths; scaling lives in
`src/svg.rs::emit_icon`.
