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
rules, icon tiers); full field reference in `docs/SPEC.md` (local, git-ignored). Prefer structural
layout for new v1 specs. Verify the PNG and representative animation frames;
diagnose collisions against the prepared layout and the spec's intent.

## Map

- `src/timeline.rs` — the universal mechanism: timeline docs (`Doc`), keyframe
  resolution (`Tl`), `frame_at(t)` → Frame, `DocSim` (Sim adapter)
- `src/spec.rs` — v1 spec sugar (nodes/links/packets+window) → compiled to Doc
- `src/layout.rs` — canvas/theme tokens, measured Taffy boxes, prepared geometry
- `src/typography.rs` — shared embedded fonts, measured text and wrapping
- `src/svg.rs` — prepared document + time → SVG, icon tier dispatch
- `src/raster.rs` — reusable pixel buffer and bounded PNG cache
- `src/terminal.rs` — host cell geometry and aspect-preserving fitting
- `src/check.rs` + `tests/` — semantic corpus and CLI regressions
- `src/frame.rs` — the static Frame model
- `src/sims_data.rs` + `sims/*.json` — the 28 animations as data, loaded at
  runtime from the repo-local `sims/` corpus (source of truth on disk; zero
  per-animation Rust; excluded from the published crate)
- `src/assets/` — icon tables (`material-symbols.txt` 960-grid fill paths,
  `tabler-icons.txt` 24-grid stroke, `cloud-icons.txt` full-color fragments),
  embedded fonts
- `skills/dynamic-diagram/SKILL.md` — the agent skill (`dynamic-diagram skill`
  re-prints it; serves as the skill's own regression check)

## Gotchas

- Content is data: never add per-animation Rust — new sims are timeline docs
  dropped into `sims/` (no registration; `check` and sim players find them by name)
- No expressions in docs, ever: keyframes only (numbers lerp, strings step);
  computed content is precomputed at authoring time
- usvg can't parse comma font lists — fonts are single names
  (`Inconsolata`, `Liberation Sans`), embedded in `src/assets/fonts/`.
- Material icon paths are 960×960 y-negative-up; scaling lives only in
  `emit_icon` (svg.rs). Don't re-scale per call site.
- Layout metrics live in `Theme`; change them there, then measure node boxes.
  `canvas.min_height` adds room; `canvas.scale` scales the whole presentation;
  raster `--density` affects pixel sampling only. See `docs/SPEC.md` (local, git-ignored).

## Related project

`../pi-diagram` is the pi extension driving this engine (tool `diagram`, `/diagram`;
npm: `@tzssangglass/pi-diagram`). Engine resolution: `DYNAMIC_DIAGRAM_BIN`
→ vendored `vendor/bin` (postinstall minisign-verified download from this
repo's Releases) → PATH (`cargo binstall dynamic-diagram`).
