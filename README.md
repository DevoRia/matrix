# Matrix

Universe simulation in Rust. Multi-scale physics from Big Bang to life emergence.

## Stack

Rust (nightly, edition 2024), Bevy 0.15, wgpu, WGSL

## Crates

| Crate | Description |
|---|---|
| `matrix_core` | Types, config, constants |
| `matrix_physics` | N-body gravity, Friedmann cosmology, procedural generation |
| `matrix_gpu` | GPU compute (Barnes-Hut, WGSL shaders) |
| `matrix_sim` | Universe state, lazy LOD region system |
| `matrix_render` | Camera, rendering, HUD |
| `matrix_storage` | Persistence (WIP) |

## Physics

| Scale | Model |
|---|---|
| Particles | 100K N-body with softened gravity |
| Stars | Madau & Dickinson SFR + Kroupa IMF |
| Planets | Kepler orbits + Stefan-Boltzmann |
| Life | Drake equation + habitability constraints |
| Evolution | Planetary physics constraints (temperature, atmosphere, chemistry) |

## Design

- 512 regions (8³), 100 Mpc each, LOD by camera distance
- Deterministic: ChaCha8 RNG, seeded per region
- Abstract genome: 10 trait axes, not human-specific

## Build

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
| +/- | Time scale |

## License

MIT
