# dynamic-diagram

Portable diagram/simulation engine (Rust). Two ways in:

1. **Declarative spec** — a JSON document describes the picture, render anywhere.
2. **Sims** — 28 faithful ports of fazamhd.com's interactive network diagrams.

## The spec format (`dynamic-diagram spec <file.json> [svg|png|kitty]`)

```json
{
  "header": "MY NETWORK ·· OVERVIEW",
  "nodes": [
    { "id": "laptop", "label": "your laptop", "icon": "laptop", "x": 10, "y": 55 },
    { "id": "fw", "label": "firewall", "icon": "firewall", "x": 32, "y": 55 },
    { "id": "db", "label": "primary db", "icon": "database", "x": 88, "y": 30, "status": "10.0.0.5:5432" }
  ],
  "links": [
    { "from": "laptop", "to": "fw", "arrow": "end" },
    { "from": "fw", "to": "dns", "arrow": "both" }
  ],
  "packets": [ { "label": "GET /api", "from": "laptop", "to": "fw", "progress": 0.5 } ],
  "texts":   [ { "text": "dmz", "x": 43, "y": 68, "dim": true } ],
  "badge": "live",
  "note": "caption line at the bottom"
}
```

Coordinates are 0..100 scene space. `status` = badge under a node; `arrow` =
`"end"` | `"both"`. See `examples/network.json`.

### Icons — three tiers

1. **Material Symbols — the FULL library: 3,912 filled icons** (embedded path
   data, Apache 2.0, from npm @material-symbols/svg-400). Router, dns, cell_tower,
   heat_pump, cyclone, volcano, skull — every symbol Google ships.
   `dynamic-diagram spec --list-icons` prints every name; unknown names degrade
   to label-only gracefully.
   Aliases: `server→dns, phone/client→smartphone, tower→cell_tower,
   switch→settings_ethernet, firewall→security, database/db→storage,` etc.
2. **Flowchart/graphviz shapes** (semantic, not pictorial):
   `box, cylinder, ellipse/oval/circle, diamond, hexagon, stadium/pill,
   triangle, subroutine`.
3. Unknown name → no glyph, label text only.

## Architecture

```
timeline docs (sims/*.json, embedded at build) ─┐
spec JSON (v1 sugar) ───────────────────────────┼─→ Doc ─→ Frame(t) ─→ SVG ─┬─→ browser
                                                └─→ keyframes resolve      ├─→ PNG (resvg, 30fps)
                                                                          └─→ kitty protocol
```

- **One universal mechanism**: a scene is elements whose properties are
  time-lines (constants or keyframes; numbers lerp, strings step). `frame_at(t)`
  resolves a doc into a static Frame — no per-animation code anywhere.
- The 28 network sims (TCP handshake, DNS, QUIC, BGP, …) are pure data in
  `sims/`, embedded into the binary at compile time (self-contained, any cwd).
- Renderers: `src/svg.rs` (SVG), `src/kitty.rs` (terminal pixels).
  Rasterization is in-process (`resvg`), no subprocess.

## Run

```bash
mise use rust
cargo build --release
./target/release/dynamic-diagram spec examples/network.json        # → examples/network.png
./target/release/dynamic-diagram spec examples/network.json svg    # → stdout
./target/release/dynamic-diagram spec examples/network.json kitty  # → terminal
./target/release/dynamic-diagram spec examples/req-path.json       # animated example (has duration)
./target/release/dynamic-diagram            # browser sim player: localhost:8080
./target/release/dynamic-diagram kitty quic # terminal sim player (30fps)
./target/release/dynamic-diagram check      # correctness check, all sims
./target/release/dynamic-diagram skill      # print the agent authoring skill
```

## Sims (28) — data, not code

tcphs (default) · encap · arp · modem · vpn · ipbits · checksum · bgp · certchain ·
tcpvsudp · anycast · dialup · dh · routerhop · switchlearn · mtu · tls · tcpsim ·
igp · wdm · nat · dns · bandwidth · telegraph · netsim · msgjourney · linkclick · quic

Each is a timeline document in `sims/<name>.json` (embedded at build).
Faithful ports of the original site — timing, captions, geometry preserved;
simplifications noted in the docs

Icon data © Google, Apache 2.0 (src/assets/icons.rs).

## AI agent integration

The engine is agent-agnostic: it's a CLI with a JSON contract, so any agent
(pi, Claude Code, Codex, …) can render diagrams by writing a spec and running
the binary.

1. **Put the binary on PATH** — `ln -s $PWD/target/release/dynamic-diagram
   ~/.local/bin/` (or export `DYNAMIC_DIAGRAM_BIN`).
2. **Expose the skill to the agent:**
   - pi: `cp -r skills/dynamic-diagram ~/.pi/agent/skills/`
     (or via the [pi-diagram](../pi-diagram) extension, which also registers
     a `diagram` tool)
   - Claude Code: copy into the project's `.claude/skills/`, or work inside
     this repo — `CLAUDE.md` imports `AGENTS.md`
   - Codex and others: `AGENTS.md` at the repo root is picked up
     automatically; point other projects at `skills/dynamic-diagram/SKILL.md`
3. **Self-ingest anywhere:** `dynamic-diagram skill` prints the authoring
   skill — an agent that has never seen this repo can render diagrams after
   reading one command's output.

The full spec reference is `docs/SPEC.md`; authoring steps, geometry rules,
and icon tiers are in `skills/dynamic-diagram/SKILL.md`. The [pi-diagram]
(../pi-diagram) extension embeds the compact spec doc in its tool description,
so LLMs using that tool need no other files.
