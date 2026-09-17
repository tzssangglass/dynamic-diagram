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

- `src/spec.rs` — spec JSON → parsed plan; field semantics live here
- `src/svg.rs` — scene → SVG; owns icon tier dispatch, geometry, fonts
- `src/frame.rs` — the shared Frame model (`frame_at(t)`)
- `src/assets/` — icon tables (`material-symbols.txt` 960-grid fill paths,
  `tabler-icons.txt` 24-grid stroke, `cloud-icons.txt` full-color fragments),
  embedded fonts
- one file per sim in `src/` (`tcp_handshake.rs` = `tcphs`, …)
- `skills/dynamic-diagram/SKILL.md` — the agent skill (`dynamic-diagram skill`
  re-prints it; serves as the skill's own regression check)

## Gotchas

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
