# Matrix

Universe simulation in Rust. From Big Bang to life emergence, with cyclic rebirth and soul persistence.

## Stack

Rust (nightly, edition 2024), Bevy 0.15, wgpu, WGSL

## Crates

| Crate | Description |
|---|---|
| `matrix_core` | Types, config, constants, genome, regions |
| `matrix_physics` | N-body gravity, Friedmann cosmology, procedural generation |
| `matrix_gpu` | GPU compute (Barnes-Hut, WGSL shaders) |
| `matrix_sim` | Universe state, lazy LOD region system, app state machine |
| `matrix_render` | Camera, rendering, HUD, menu, surface exploration |
| `matrix_storage` | Save/load snapshots (binary format) |

## Build

```bash
rustup override set nightly
cargo run --release
```

---

## How the Universe Works

### App Flow

Menu → Loading → Running

- **Menu**: New Universe (generate from seed) or Load Save (latest snapshot)
- **Loading**: Async generation on background thread, loading screen displayed
- **Running**: Simulation ticks, exploration enabled

### Units

| Quantity | Unit |
|---|---|
| Distance | 1 = 1 megaparsec (Mpc) |
| Mass | 1 = 10¹⁰ solar masses |
| Time | 1 = ~1 billion years (Gyr) |

### Big Bang & Particles

100,000 particles spawned in a sphere. Each particle has position, velocity, mass, kind, temperature, and alive flag. Initial velocity = `big_bang_velocity * normalized_direction` (default 5.0).

Particle types:
- **Quarks**: UpQuark, DownQuark (mass 0.003)
- **Leptons**: Electron (0.0005), Neutrino (massless)
- **Bosons**: Photon, Gluon (massless)
- **Composite**: Proton, Neutron (mass 1.0)
- **Atoms**: Hydrogen (1.0), Helium (4.0), Carbon (12.0), Nitrogen (14.0), Oxygen (16.0), Iron (56.0)
- **Dark Matter** (mass 10.0)

Default mix: 27% dark matter, rest baryonic (hydrogen-dominated).

### Universe Phases

| Phase | Age (Gyr) | What happens |
|---|---|---|
| BigBang | 0 | Particles explode outward |
| Inflation | >10⁻⁹ | Rapid expansion, Hubble=1000 |
| NuclearEra | >10⁻⁵ | Nucleosynthesis, Hubble=50 |
| AtomicEra | >0.0004 | Atoms form, Hubble=20 |
| CosmicDawn | >0.4 | First stars ignite, Hubble=10 |
| StellarEra | >1.0 | Galaxies form, Hubble=5 |
| BiologicalEra | >10.0 | Life emerges, Hubble=3 |
| CivilizationEra | >13.0 | Intelligence possible, Hubble=2 |
| HeatDeath | entropy >90% max | Universe cooling, Hubble=1 |
| Collapse | entropy >100% max | Contraction, Hubble=-10, then rebirth |

### Scale Factor (Friedmann Equation)

```
if age < 13.8 Gyr:
    a(t) = (t / 13.8)^(2/3)        # Matter-dominated
else:
    a(t) = exp((t - 13.8) * 0.07)  # Dark energy dominated
```

Expansion applied to particles each tick: `position += position * hubble * dt * 0.001`

### Cosmic Temperature

```
T(t) = 2.725K / a(t)
If a < 10⁻¹⁰: T = 10¹² K (quark-gluon plasma)
```

---

## Gravity

### Hybrid System (CPU)

Two components computed together:

1. **Near-field** — direct summation of K=32 nearest neighbors
2. **Far-field** — 16³ = 4096 spatial grid, gravity from cell centers-of-mass

Formula per pair:
```
r² = dx² + dy² + dz² + softening²    (softening = 0.01)
acceleration = G * mass_other / (r² * √r²) * direction
```

Integration: Verlet (velocity += acc * dt, position += vel * dt)

Velocity damping: `vel *= 1.0 - dt * 0.002`
Cooling: `temperature *= 1.0 - dt * 0.01`

### Throttling

Gravity doesn't run every frame. Interval depends on time scale:
- Normal (1×): every 3 frames
- Fast (100×): every 5 frames
- Faster (10K×): every 30 frames
- Fastest (1M+×): every 120 frames

### GPU Compute (Prepared, Not Active)

WGSL shader with O(n²) direct summation, workgroup size 256. Barnes-Hut opening angle θ=0.5. Not yet wired into render loop — CPU hybrid gravity is active.

---

## Entropy & Thermodynamics

Calculated every 30 frames.

**Entropy** = velocity dispersion across all particles:
```
var_vx = E[vx²] - E[vx]²
var_vy = E[vy²] - E[vy]²
var_vz = E[vz²] - E[vz]²
dispersion = var_vx + var_vy + var_vz
entropy = ln(dispersion) * particle_count
```

**Temperature** = average kinetic energy:
```
T = Σ(0.5 * mass * |velocity|²) / particle_count
```

Max entropy threshold: 1,000,000. At 90% — HeatDeath phase. At 100% — Collapse and rebirth.

---

## Regions (Lazy Universe)

512 regions in 8×8×8 grid, each 100 Mpc. Only the observed region gets full detail.

### LOD Levels

| Level | When | CPU cost |
|---|---|---|
| Statistical | Far from camera | Zero — just numbers |
| Galactic | Within 200 Mpc | ~100 mass points |
| Stellar | Within 50 Mpc | Individual star systems |
| Planetary | Camera inside | Surface detail, geology |
| Biosphere | Life detected | Evolution running |

LOD updates every 5 frames. Region stats recalculated when universe age changes by >2 Gyr.

### Region Properties

Each region tracks: density (0.3×–3× cosmic average, log-normal), temperature, chemical composition [H, He, metals], dark matter fraction, star count, planet count, life presence, seed.

### Chemical Evolution

```
Hydrogen: 75% - metals*0.6
Helium:   25% - metals*0.4
Metals:   0 → 2% over 13 Gyr (stellar nucleosynthesis)
```

---

## Stars

### Formation Rate (Madau & Dickinson 2014)

No stars before 0.4 Gyr. Peak at 3.3 Gyr. Then exponential decline:
```
if age < 3.3: SFR = 0.15 * (age/3.3)^2.5
else:         SFR = 0.15 * exp(-0.12 * (age - 3.3))
```
Units: solar masses / year / Mpc³

### Initial Mass Function (Kroupa IMF)

```
mass = 0.08 + (1 - uniform_random)^(-1/1.3) * 0.3
```
Range: 0.08 to ~50 solar masses. Most stars are red dwarfs.

### Luminosity & Temperature

```
luminosity = mass^3.5                                    # Main sequence
surface_temp = 5778K * (luminosity / mass²)^0.25
```

### Spectral Classes

| Class | Temperature | Color |
|---|---|---|
| O | >30,000K | Blue [0.6, 0.7, 1.0] |
| B | 10,000–30,000K | Blue-white [0.7, 0.8, 1.0] |
| A | 7,500–10,000K | White [0.9, 0.9, 1.0] |
| F | 6,000–7,500K | Yellow-white [1.0, 1.0, 0.9] |
| G | 5,200–6,000K | Yellow [1.0, 1.0, 0.7] |
| K | 3,700–5,200K | Orange [1.0, 0.8, 0.5] |
| M | 2,400–3,700K | Red [1.0, 0.5, 0.3] |

Max rendered stars per region: 1000 (generated), 80 (rendered).

---

## Planets

### Orbital Mechanics

Spacing (Titius-Bode):
```
orbital_radius = 0.2 * 1.5^orbit_index + random(-0.1..0.1) AU
```

Kepler's third law: `period = radius^1.5` years

### Temperature (Stefan-Boltzmann)

```
T_surface = 278K * luminosity^0.25 / √(radius_AU)
```

### Mass & Radius

Mass: log-uniform 0.1 to ~3000 Earth masses
```
if mass < 2:    radius = mass^0.27        # Rocky
if mass < 100:  radius = mass^0.06 * 2    # Sub-Neptune
if mass > 100:  radius = mass^-0.04 * 11  # Gas giant
```

### Planet Types

| Type | Condition |
|---|---|
| GasGiant | mass > 100 |
| IceGiant | mass > 15 |
| Lava | temp > 500K |
| Frozen | temp < 200K |
| Ocean | mass > 0.5, 30% chance |
| Rocky | default |

### Atmosphere

| Atmosphere | Condition |
|---|---|
| None | mass < 0.3 or temp > 2000K |
| Hydrogen | mass > 100 (gas giant) |
| NitrogenOxygen | has water, 30% chance |
| ThinCO2 | has water |
| ThickCO2 | temp > 400K |
| Methane | default |

---

## Life Emergence

### Probability (Drake-inspired)

```
base = 0.1 (10% for habitable)
× temp_factor = exp(-(temp - 288K)² / 800)     # Earth-like = best
× planet_type: Rocky=1.0, Ocean=0.5, Frozen=0.01, other=0.001
× time_factor = (1 - exp(-life_age * 0.3))     # Needs time
```
Clamped to 10⁻⁷ – 15%.

Habitability requires: 200K < temp < 400K, water, atmosphere.

### Biosphere Complexity (0–10)

Probabilistic gates modeled on Earth's timeline:

| Stage | Age (Gyr after life) | Chance | Complexity |
|---|---|---|---|
| Prokaryotes | 0+ | 100% | 0–1 |
| Diversification | 0.5+ | 100% | 1–2 |
| Eukaryotes | 2.0+ | 20% | 2–3 |
| Multicellular | 3.0+ | 10% | 3–5 |
| Complex life | 3.5+ | 5% | 5–7 |
| Intelligence | 4.5+ | 1% | 7–10 |

Caps: Ocean planets max 6 (no fire/tools), Frozen max 2 (subsurface only).

---

## Genome (10 Trait Axes)

Every creature has a genome — NOT human-specific. Completely abstract.

### 1. Substrate (biochemical basis)
- 0: carbon-water (Earth)
- 1: carbon-ammonia (cold worlds)
- 2: carbon-methane (Titan-like)
- 3: silicon (hot rocky)
- 4: sulfur-iron (volcanic)
- 5: hydrocarbon-lipid (oil worlds)

### 2. Structure (body plan)
- 0: single-cell → 1: colonial → 2: biofilm → 3: radial (jellyfish)
- 4: bilateral (worm→fish→mammal) → 5: modular (coral) → 6: fractal-branching → 7: asymmetric

### 3. Senses (bitmap)
- 1: photoreception (light)
- 2: mechanoreception (touch/hearing)
- 4: chemoreception (smell/taste)
- 8: thermoreception
- 16: electroreception
- 32: magnetoreception
- 64: proprioception

### 4. Size (log₁₀ meters)
- -6: virus → -5: bacterium → -3: mm insect → 0: 1m → 1: 10m whale → 2: 100m fungal network

### 5. Energy Source
- 0: photosynthesis → 1: chemosynthesis → 2: geothermal → 3: radiotrophic
- 4: fermentation → 5: osmotic → 6: thermosynthesis → 7: heterotrophy (predator)

### 6. Cognition (0.0–1.0)
- 0.0: reactive → 0.2: learning → 0.4: problem-solving → 0.6: tool-use → 0.8: language → 0.9+: superintelligence

### 7. Collective (0.0–1.0)
- 0.0: solitary → 0.4: herds → 0.6: eusocial → 0.8: cooperative culture → 1.0: superorganism

### 8. Propagation
- 0: binary fission → 1: budding → 2: spore → 3: fragmentation → 4: sexual → 5: parthenogenesis

### 9. Motility
- 0: sessile → 1: drift → 2: flagella → 3: crawling → 4: swimming → 5: walking → 6: burrowing → 7: flight

### 10. Interface (outer boundary)
- 0: cell membrane → 1: cell wall → 2: exoskeleton → 3: endoskeleton → 4: shell → 5: mucous → 6: fur/feathers/scales

Genome constrained by environment: substrate by planet type, structure follows complexity, cognition requires bilateral body, flight requires atmosphere.

Mutation rate per trait per generation: configurable per creature.

---

## Cyclic Universe & Souls

When entropy hits maximum — heat death. Universe collapses (Hubble goes negative) and restarts. Cycle counter increments.

**Soul** = creature's accumulated experience vector:
- Duration of life
- Genome stability (which axes mutated least = "strong" genes)
- Environmental conditions survived

Souls persist between cycles. New creatures in the next cycle inherit soul data: initial genome based on previous cycle's experience, not random. Strong genes have higher chance of preservation. Weak genes mutate freely.

Result: each cycle, creatures start from a better base. Not smarter — more adapted. The universe learns.

---

## Surface Exploration

When you land on a planet (select + B), the surface system generates:

### Terrain
- 200×200 unit patch, 64×64 grid resolution
- Multi-octave noise (5 octaves) with domain warping
- Amplitude by planet type: Rocky=20, Ocean=6, Frozen=12, Lava=25, GasGiant=2, IceGiant=4
- Vertex-colored biomes by height (shore → grass → forest → rock → snow for Rocky)

### Water & Sky
- Water plane at Y=-0.5, alpha 0.6 (only if planet has water)
- Sky dome 500 unit radius with scattered stars
- Star count by atmosphere density: None=400, NitrogenOxygen=150, ThickCO2=60
- Directional sunlight colored by parent star's spectral class

### Creatures
- Up to 80, spawned from planet's dominant genome
- Mesh by structure axis (sphere for cells, cuboid for bilateral, tall for modular)
- Color by substrate (green=carbon-water, blue=ammonia, gray=silicon, orange=sulfur)
- Scale from size axis: 10^(size_log), clamped 0.2–5.0
- Speed from motility axis: sessile=0, walking=4, flight=6
- AI: wander to random targets every 3–10 sec, freeze when camera within 3m

### Surface Zoom Levels

| Level | Eye Height | What spawns |
|---|---|---|
| Landscape | 5–10m | Terrain overview, creatures |
| Ground | 1–5m | Detail objects (rocks, plants) |
| Close-Up | 0.3–1m | Smaller details |
| Microscopic | 0.05–0.3m | Microbe particles (30 max) |

---

## Rendering

### Particles
- Max 3,000 rendered (stride sampling from 100K)
- Distance culling at 2000 units
- One mesh per particle kind (batched)
- Each particle = 1 triangle
- Triangle size scales with camera distance: 0.04 (close) to 3.0 (far)
- Updates every 3rd frame
- Active only at Planetary and Surface zoom levels

### Stars & Planets
- 80 stars max rendered, sorted by distance
- Shared materials per spectral class (7 total)
- Only 2 nearest stars get point lights
- Planets rendered for nearest 15 stars
- Life planets glow green, tech planets glow yellow
- Pulse animation on life/tech planets

### Regions
- 512 cubes at Cosmic/Galactic zoom
- Size by density: (density×5) clamped 2–20
- Colors: life=green, high density=orange, mid=blue, low=gray

### Performance
- Gravity throttled by time scale (3–120 frame intervals)
- HUD updates every 10 frames
- LOD updates every 5 frames
- Entropy calculated every 30 frames
- Dead particles compacted every 100 frames
- All materials shared/batched per type

---

## Snapshots

Binary format (bincode). Saves everything: particles, regions, stars, life planets, age, phase, entropy, config, time scale, camera state.

Location: `saves/snapshot_{timestamp}.bin`

---

## Controls

### Space Mode

| Key | Action |
|---|---|
| WASD | Fly |
| Mouse RMB + Drag | Look |
| E/Q | Up / Down |
| Scroll | Speed (1–10,000) |
| Shift | 5× speed |
| LMB | Select planet / region |
| B | Enter region / Land on planet |
| Esc | Exit to Cosmic |
| -/= | Zoom out / in |
| O | Origin |
| F | Densest cluster |
| N | Nearest particle |
| T | Track particle |
| Tab | Cycle particle types |
| G/H | Next / Prev region |
| L | Find life |
| Space | Pause / Resume |
| 1–5 | Time: 1×, 100×, 10K×, 1M×, 1B× |
| F5 | Save snapshot |
| F9 | Load snapshot |

### Surface Mode

| Key | Action |
|---|---|
| WASD | Walk |
| Mouse | Look (always active) |
| Shift | 3× speed |
| Scroll | Eye height (0.05–10m) |
| B / Esc | Return to space |
| Space | Pause / Resume |
| 1–5 | Time scale |

---

## Config Defaults

```rust
particle_count: 100,000
seed: 42
big_bang_velocity: 5.0
gravity_scale: 1.0
dark_matter_fraction: 0.27
```

## Constants

```
G = 1.0                    Gravitational constant
C = 3000 Mpc/Gyr           Speed of light
SOFTENING = 0.01            Gravity softening
MAX_ENTROPY = 1,000,000     Heat death threshold
DT = 0.001 Gyr             Time step
BH_THETA = 0.5             Barnes-Hut opening angle
NEAR_FIELD_K = 32           Nearest neighbors for direct gravity
WORKGROUP_SIZE = 256        GPU shader workgroups
```

## License

MIT
