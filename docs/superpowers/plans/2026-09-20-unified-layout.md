# Unified Layout and Adaptive Playback Implementation Plan

> **For agentic workers:** Use superpowers:subagent-driven-development or superpowers:executing-plans task-by-task. Steps use checkbox syntax for tracking.

**Goal:** Replace local visual offsets with measured layout, support expanded content space and independent 2× display, and reduce rendering/playback overhead.

**Architecture:** Prepare stable geometry from the complete timeline document, then sample time into that geometry. Taffy handles composite boxes; measured node bounds, annotations, ports and paths share a scene model. Raster density and host presentation remain independent.

**Tech Stack:** Rust, existing resvg/usvg, Taffy; TypeScript pi extension.

**Spec:** `outputs/layout-performance-analysis.md`, `outputs/layout-engine-research.md`; user approved implementation after requesting 2× size.

## Global Constraints

- One Rust binary; no per-animation Rust; timeline values remain keyframes without expressions.
- Existing v1 and v2 documents and all 28 seed simulations remain supported.
- Width fits the host; content determines height. User-configurable `canvas.min_height` requests additional room and `canvas.scale` scales the whole presentation independently; both support a 2× result without per-scene renderer constants.
- Preserve fixed-coordinate semantics for timelines; automatic structural layout is opt-in for coordinate-free v1 nodes.
- Layout decisions use measured typography and theme tokens. Icon path coordinates remain intrinsic resource data.
- Keep SVG/PNG/kitty consistent and deterministic for random time access.
- User requested implementation in these working checkouts. Work on feature branches, preserving existing analysis files; no push/publish.

## Task 1: Measured and prepared rendering

Files: `src/layout.rs`, `src/typography.rs`, `src/svg.rs`, `src/frame.rs`, `src/spec.rs`, `src/timeline.rs`, `Cargo.toml`, relevant tests.

Interface: `svg::Renderer::new(&timeline::Doc) -> Result<Renderer,String>`, `renderer.render(t: u64) -> String`, `renderer.size() -> (f64,f64)` for displayed SVG dimensions. Keep `svg::render_svg(&Frame) -> String` for compatibility. Register new modules in main through the integrating worker.

- [x] Write behavior tests using the retained CI/mechanism/scatter-gather fixtures: final measured labels must not overlap each other or leave the viewport; footer must follow content; long unspaced captions must wrap.
- [x] Confirm existing renderer fails those contracts. Reuse actual usvg text bounds rather than reproducing renderer offsets in tests.
- [x] Implement cached font measurement, theme metrics, Taffy composite boxes and complete layout results. Reserve full timeline text envelopes and stable node placements. Add validated `canvas` configuration and coordinate-free flow/grid layout in v1.
- [x] Connect lines/packets to shared routes and actual node bounds, preserving raw timeline/world coordinates. Renderer consumes layout without hidden per-node sizing heuristics.
- [x] Verify frame-order independence, content-height expansion, 2× presentation, old seed semantics and real raster output.

## Task 2: Renderer integration and raster performance

Files: `src/main.rs`, `src/raster.rs`, `src/kitty.rs`, CLI integration tests.

Consumes Task 1 Renderer interface. Produces CLI `spec FILE svg|png|frames|info` where `info` emits JSON width/height/duration for host sizing. Existing explicit frame count remains supported; optional trailing `--scale`, `--height`, `--density` override canvas/output settings.

- [x] Add CLI tests for malformed scale/density, proportional SVG/PNG sizes and frame output count before implementing.
- [x] Prepare one Renderer per document/player; share cached font database, own/reuse raster buffer without global render lock; cache identical SVG frames within a bounded budget.
- [x] Fit kitty rows/cols to actual SVG size and actual terminal cell ratio when available. Derive browser aspect from current scene rather than fixed W/H.
- [x] Select frame counts by duration/fps when not explicitly supplied; bound generated work and frame cache. Preserve explicit frame-count CLI semantics.
- [x] Measure equivalent workloads against prior baseline, and run release build/check/tests.

## Task 3: pi playback

Files: sibling `pi-diagram/extensions/index.ts`, its tests and README. No changes to Rust in this task.

- [x] Add executable tests for responsive async child generation, fps-based frame counts, cancellation/errors, adaptive width and bounded frame cache.
- [x] Replace blocking generation with async spawn; avoid redundant poster rendering where a generated frame suffices; cache base64 bytes with a byte budget, retain changing frame identity and host lifecycle semantics.
- [x] Use viewport width instead of an 84-column cap. Playback samples elapsed time, not tick counts. Limit lifecycle timers and generated frames without leaving misleading documentation.
- [x] Run extension tests and real engine integration.

## Task 4: Documentation, visuals and final verification

Files: `docs/SPEC.md`, `skills/dynamic-diagram/SKILL.md`, `AGENTS.md`, `README.md`, examples, implementation evidence under `outputs/`.

- [x] Replace authoring rules such as `y <= 75` with layout intents/options/diagnostics; describe display scale versus raster density and compatibility mode.
- [x] Render scatter-gather, CI, mechanism and seed frames; visually inspect generated PNGs, including content expansion and 2× scale.
- [x] Run meaningful regression tests and 28-sim check; request independent code review and resolve findings.
- [x] Record benchmark configuration, before/after results and practical limits; leave changes ready to review.

## Progress

- Implementation and verification: complete. Evidence, limits and reproduction commands are in `outputs/unified-layout/RESULTS.md`. The engine and linked pi extension remain uncommitted for review.
- Planning: complete. Interface dependencies reviewed: Task 2 consumes Task 1 Renderer; Task 3 consumes existing frames CLI and optional info; no worker shares edited source files.
- Decision: preserve current working checkouts on feature branches so the binary PATH and installed pi extension pick up the work. No worktree or publication needed.
