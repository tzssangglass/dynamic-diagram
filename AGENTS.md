# dynamic-diagram — agent guide

Rust diagram engine: JSON spec → SVG → PNG/kitty/frames. 28 reference sims.
One static binary, rendering happens in-process or short-lived children.

## Commands

```sh
cargo build --release                                # → target/release/dynamic-diagram
./target/release/dynamic-diagram check               # full regression — keep green, zero warnings
./target/release/dynamic-diagram spec examples/network.json          # render → .png
./target/release/dynamic-diagram spec examples/req-path.json         # animated example
```

## Writing a spec

Read `skills/dynamic-diagram/SKILL.md` first (authoring steps, geometry
rules, icon tiers); full field reference in `docs/SPEC.md`. Verify a spec by
rendering it and looking at the PNG — collisions are coordinate bugs, not
renderer bugs.

## Map

- `src/timeline.rs` — the universal mechanism: timeline docs (`Doc`), keyframe
  resolution (`Tl`), `frame_at(t)` → Frame, `DocSim` (Sim adapter)
- `src/spec.rs` — v1 spec sugar (nodes/links/packets+window) → compiled to Doc
- `src/svg.rs` — scene → SVG; owns icon tier dispatch, geometry, fonts
- `src/frame.rs` — the static Frame model
- `src/sims_data.rs` + `sims/*.json` — the 28 animations as data, embedded at
  compile time (source of truth on disk; zero per-animation Rust)
- `src/assets/` — icon tables (`material-symbols.txt` 960-grid fill paths,
  `tabler-icons.txt` 24-grid stroke, `cloud-icons.txt` full-color fragments),
  embedded fonts
- `skills/dynamic-diagram/SKILL.md` — the agent skill (`dynamic-diagram skill`
  re-prints it; serves as the skill's own regression check)

## Gotchas

- Content is data: never add per-animation Rust — new sims are timeline docs
  in `sims/` (then a line in `src/sims_data.rs` via its generator comment)
- No expressions in docs, ever: keyframes only (numbers lerp, strings step);
  computed content is precomputed at authoring time
- usvg can't parse comma font lists — fonts are single names
  (`Inconsolata`, `Liberation Sans`), embedded in `src/assets/fonts/`.
- Material icon paths are 960×960 y-negative-up; scaling lives only in
  `emit_icon` (svg.rs). Don't re-scale per call site.
- Icon node + status badge stack ≈ 60 stage-units below `y`; specs keep such
  nodes at y ≤ 75 (caption divider collision otherwise).

## Related project

The pi extension that drives this engine lives separately in
`../pi-diagram` (tool `diagram`, `/diagram`, `/anim`). It only needs the
binary on PATH or `DYNAMIC_DIAGRAM_BIN`.
