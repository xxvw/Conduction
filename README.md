# Conduction

> Conduct your mix, don't perform it.

**Conduction** is a programmable, extensible DJ engine for live performance. It treats human UI input, AI suggestions, plug-ins, and Lua scripts as equally valid *instructions* to a single Conductor layer — so the DJ can choreograph a set instead of frantically reaching for every knob.

- License: [MIT](./LICENSE)
- Status: in-development (pre-1.0) — the first public build is tagged **`in-dev 1.0`**
- Platform: macOS (primary), Linux, Windows
- Built with: Rust + Tauri v2 + React 18 + TypeScript
- 日本語版: [README_ja.md](./README_ja.md)

---

## Why Conduction?

Most DJ software treats automation as an afterthought — you record an Ableton clip, or you bind a MIDI controller, or you fight a piano-roll. Conduction inverts this: **every parameter change is a first-class instruction**, whether it comes from a fader you touched, a Cue you tagged, a setlist transition, or a Lua function. They all flow through the same Conductor, so handing a section to automation and taking it back is just a context switch — not a workflow.

### Core ideas

1. **One Conductor, many sources.** Faders, AI, scripts, and Cue matching all write to the same parameter bus. Override / Resume / Commit are universal verbs.
2. **Automation templates are programs.** Visual timeline, node graph, or Lua script — they compile to the same `Template` AST.
3. **Cues are typed.** `Drop` / `Intro` / `Breakdown` / `Outro` have semantics, so "the next track that mixes here" is a query, not a guess.
4. **Setlists are documents.** Export to `.cset`, share with a collaborator, re-open on another machine.

---

## Features

### Mixer
- Two decks with effective BPM tracking, master tempo (keylock), and ±6 / ±10 / ±16 % tempo range
- 3-band EQ + filter + FX (reverb / delay / etc.) per deck
- Crossfader, master, channel volumes, PFL (CUE bus) routing
- Beat-snapped loops with bar-wise extend / shrink
- 8 Hot Cues per track plus typed Cues (Drop / Intro / Breakdown / Outro …)

### Library
- Folder-scan import, BPM and Camelot-key detection, energy estimation, waveform cache
- SQLite-backed (auto-migrated, currently schema v5)
- `MixSuggestion` panel proposes the next track based on BPM / Key / Energy compatibility against typed Cues of the active deck

### Automation Templates
Three editors, one underlying `Template` model:

- **Visual timeline** — drag keyframes per target with `linear` / `smooth` / `ease-in` / `ease-out` curves
- **Node editor** — `react-flow` graph of `Source → Target` pairs
- **Script (Lua)** — write a real program. Lua is sandboxed and used as a *code generator*; it never runs on the audio thread.

Lua API (excerpt):

```lua
set_duration(16)                     -- bars
set_direction("a_to_b")              -- or "b_to_a"
keyframe("crossfader", 0,  -1.0, "linear")
keyframe("crossfader", 64, 1.0,  "smooth")

each_bar(16, function(bar, beat)
  local t = bar / 16
  keyframe("deck_eq_low.A", beat,
           lerp(1.0, 0.0, smoothstep(0, 1, t)), "linear")
end)
```

The Monaco-based editor ships with inline error markers, completion (Conduction API + prelude helpers + Lua keywords + `math.*`), syntax highlighting, and a slide-in docs panel.

Five built-in presets (`linear_16bar`, `crossfade_outro_intro`, `eq_swap`, `filter_sweep`, `build_drop`) come with their Lua source so they can be forked.

### Setlists
- Drag-reorder, per-transition duration, template assignment
- `.cset` export / import (carries the setlist plus lightweight track metadata)
- One-click "Load to Deck" from a setlist row

### YouTube import (optional)
- `cargo build --features yt-download` enables yt-dlp integration

---

## Quick start

> Full instructions: [`install.md`](./install.md). Full feature manual: [`usage.md`](./usage.md).

```bash
# 1. Prerequisites
#    - rustup (Rust stable, pinned by rust-toolchain.toml)
#    - Node.js 20+ and npm
#    - macOS: xcode-select --install   (needed by mlua vendored Lua)
#    - Linux: webkit2gtk / gtk3 / appindicator dev packages

# 2. Clone and install
git clone https://github.com/xxvw/conduction.git
cd conduction
cd ui && npm install && cd ..

# 3. Run in dev mode
npm run --prefix ui app:dev

# 4. Build a release bundle (.app / .dmg / .AppImage / .msi)
npm run --prefix ui app:build
```

---

## Repository layout

```
conduction/
├── crates/
│   ├── conduction-core/        Data model (Track, Cue, Template, Deck)
│   ├── conduction-audio/       Audio engine (cpal)
│   ├── conduction-analysis/    BPM / key / energy detection
│   ├── conduction-conductor/   Template execution + Cue matching
│   ├── conduction-library/     SQLite persistence
│   ├── conduction-claude/      Claude CLI bridge (feature: claude-analysis)
│   ├── conduction-download/    yt-dlp bridge (feature: yt-download)
│   ├── conduction-export/      .cset serialization
│   ├── conduction-script/      Lua → Template compiler
│   └── conduction-app/         Tauri app (IPC commands)
└── ui/
    ├── design-system/          CSS variables, preview
    └── src/                    React + TS (screens / components / hooks)
```

---

## Documentation

- [`install.md`](./install.md) — install and build instructions detailed enough for an LLM agent to follow
- [`usage.md`](./usage.md) — every screen, every keybinding, every Lua API
- [`conduction-requirements.md`](./conduction-requirements.md) — full design spec (Japanese)
- [`ui/design-system/DESIGN_SYSTEM.md`](./ui/design-system/DESIGN_SYSTEM.md) — design tokens and components

---

## Releases

The first prebuilt artifact is published as **`in-dev 1.0`**. Find the macOS `.dmg` / `.app.tar.gz` on the [Releases](https://github.com/xxvw/conduction/releases) page.

The version string `0.1.0` in `Cargo.toml` will move to `0.2.0` once Phase 2 (live FX automation + scripting playback controls) ships. A `1.0.0` semver bump is reserved for the first non-in-dev release.

---

## Contributing

Issues and pull requests are welcome. Before opening a PR:

1. `cargo fmt --all` and `cargo clippy --workspace --all-targets -- -D warnings`
2. `cargo test --workspace`
3. `cd ui && npx tsc --noEmit`

For UI-touching changes, please attach before / after screenshots from the dev build.

---

## License

MIT — see [`LICENSE`](./LICENSE).
