# Matrix

A universe simulation built from scratch in Rust. Real physics. One seed. One run. Whatever happens, happens once.

## What is this?

Matrix simulates a universe from the Big Bang through star formation, planetary systems, and the emergence of life — all driven by actual physical equations, not scripted scenarios.

A single star is 10⁵⁷ atoms. A single cell is 10¹⁴. Even a DNA molecule has 100,000× more atoms than this entire simulation. No computer on Earth can simulate a single bacterium atom-by-atom.

So the simulation works like a **map, not the territory**. Each scale has its own equations:

- **Early Universe** — 100K particles with N-body gravity, showing how matter clumps into large-scale structure
- **Stars** — generated from Madau & Dickinson star formation rates + Kroupa IMF mass distribution
- **Planets** — Keplerian orbital mechanics + Stefan-Boltzmann law for surface temperature
- **Life** — Drake equation + biochemical habitability constraints
- **Evolution** — constrained by planetary physics: temperature, atmosphere, chemistry

Everything beyond the particle level is computed mathematically. This runs on a laptop, not a NASA supercluster.

## Architecture

Rust workspace with 6 crates, built on **Bevy 0.15** + **wgpu**:

| Crate | Purpose |
|---|---|
| `matrix_core` | Types, config, constants (particles, regions, genome) |
| `matrix_physics` | N-body gravity, Friedmann cosmology, procedural generation |
| `matrix_gpu` | GPU compute infrastructure (Barnes-Hut, WGSL shaders) |
| `matrix_sim` | Universe state machine, lazy LOD region system |
| `matrix_render` | Camera, particle/cosmos rendering, HUD |
| `matrix_storage` | Persistence layer (WIP) |

### Key design decisions

- **Lazy Universe**: 512 regions (8³), each 100 Mpc. Only observed regions get full detail.
- **LOD system**: Statistical → Galactic → Stellar → Planetary → Biosphere, based on camera distance.
- **Deterministic**: ChaCha8 RNG seeded per region. Same seed = same universe, every time.
- **Abstract genome**: Life is defined by 10 trait axes (substrate, cognition, motility, etc.) — not hardcoded to be human-like.

## Building

Requires Rust nightly (edition 2024):

```bash
rustup override set nightly
cargo run --release
```

## Controls

| Key | Action |
|---|---|
| WASD / Mouse | Fly camera |
| Tab | Cycle particle filter |
| P | Pause / Resume |
| +/- | Speed up / slow down time |

## Status

Work in progress. Phases completed:
- Particle system & gravity (CPU)
- Universe state & phase tracking
- Cosmological equations (scale factor, temperature, SFR)
- 512-region lazy universe with LOD
- Procedural star/planet generation
- Abstract genome & life emergence
- Dual rendering (particles + cosmos)

Next: GPU gravity (Barnes-Hut), civilization agents, soul persistence across cycles.

## License

MIT
