//! Monte Carlo simulation of 100 universes.
//! Focus: catalogue the most interesting life forms that emerge.

use matrix_core::{Biosphere, SimConfig};
use matrix_physics::procgen;
use std::collections::HashMap;

/// A discovered creature with full context
#[derive(Clone)]
struct Creature {
    universe_id: u32,
    universe_seed: u64,
    age_gyr: f64,
    star_class: String,
    planet_type: String,
    planet_temp: f64,
    planet_has_water: bool,
    orbital_radius_au: f64,
    bio: Biosphere,
}

impl Creature {
    fn uniqueness_score(&self) -> f64 {
        let g = &self.bio.dominant_genome;
        // Prefer: high complexity, non-carbon substrates, large size, cognition, tech
        let exotic_substrate = if g.substrate >= 2 { 15.0 } else { 0.0 };
        let size_score = (g.size_log + 3.0).max(0.0) * 3.0; // bigger is more interesting
        let sense_richness = g.sense_count() as f64 * 3.0;
        let mind_score = g.cognition * 40.0;
        let collective_score = g.collective * 10.0;
        let complexity = self.bio.complexity * 5.0;
        let tech = if self.bio.has_technology { 50.0 } else { 0.0 };
        let multicellular = if g.structure >= 3 { 10.0 } else { 0.0 };

        exotic_substrate + size_score + sense_richness + mind_score
            + collective_score + complexity + tech + multicellular
    }

    /// Generate a vivid portrait — grounded in real biochemistry
    fn portrait(&self) -> String {
        let g = &self.bio.dominant_genome;
        let mut lines = Vec::new();

        // Biochemistry
        let body = match g.substrate {
            0 => "Carbon-water biochemistry — proteins, lipids, nucleic acids dissolved in liquid water. The same recipe as Earth life, yet the details differ in every way.",
            1 => "Carbon-ammonia biochemistry — in a world too cold for liquid water, ammonia serves as the solvent. Its proteins fold differently, its metabolism runs slow and cold.",
            2 => "Carbon-methane biochemistry — on a frigid world where methane flows as rivers, this organism uses hydrocarbon chemistry that would be impossible at Earth temperatures.",
            3 => "Silicon-based — on a scorching world where carbon compounds decompose, silicon-oxygen bonds form the backbone of its molecules. It is rock that lives.",
            4 => "Sulfur-iron biochemistry — born in volcanic vents, its metabolism is built on iron-sulfur clusters and sulfide chemistry, thriving in conditions that would dissolve Earth life.",
            5 => "Hydrocarbon-lipid biochemistry — membranes and energy storage based on complex hydrocarbons, in an oily world where water is scarce.",
            _ => "Carbon-water biochemistry — the most common recipe for life in the universe.",
        };
        lines.push(body.to_string());

        // Body plan
        let shape = match g.structure {
            0 => "Single-celled — one microscopic unit contains all the machinery of life. Simple, ancient, and astonishingly successful.",
            1 => "Colonial — clusters of identical cells that cooperate loosely. Not quite multicellular, but more than the sum of their parts.",
            2 => "A biofilm — a living mat spread across surfaces, cells communicating through chemical signals. A city without buildings.",
            3 => "Radially symmetric — like a jellyfish or sea urchin. No front or back, it faces the world equally from all directions.",
            4 => "Bilaterally symmetric — a head and a tail, a left and a right. This body plan concentrates senses at the front, enabling directed movement and hunting.",
            5 => "Modular — like a coral or plant, it grows by repeating units. Each module is semi-independent, and the organism can lose parts and regrow them.",
            6 => "Branching — tree-like or fractal, spreading outward to maximize surface area. Roots below, canopy above, competing for light and nutrients.",
            _ => "Asymmetric — no pattern, no symmetry. An organism shaped purely by its environment, unique as a snowflake.",
        };
        lines.push(shape.to_string());

        // Size
        let size_m = 10.0f64.powf(g.size_log);
        let size_str = if g.size_log < -4.0 {
            "Submicroscopic — smaller than most cells on Earth. Billions could fit on a pinhead.".to_string()
        } else if g.size_log < -2.0 {
            format!("Microscopic (~{:.0} micrometers). Invisible to the naked eye.", size_m * 1e6)
        } else if g.size_log < 0.0 {
            format!("{:.1} cm — small enough to hold in your hand.", size_m * 100.0)
        } else if g.size_log < 1.0 {
            format!("{:.1} meters tall — comparable to a dog or a person.", size_m)
        } else {
            format!("{:.0} meters — enormous, like a whale or a dinosaur.", size_m)
        };
        lines.push(size_str);

        // Energy source
        let energy = match g.energy_source {
            0 => "Photosynthetic — it captures starlight and converts it to chemical energy. The foundation of its world's food chain.",
            1 => "Chemosynthetic — it extracts energy from chemical reactions, thriving in darkness near volcanic vents or mineral-rich springs.",
            2 => "Geothermal — it taps the planet's internal heat, living where the crust is thin and warmth seeps upward.",
            3 => "Radiotrophic — it feeds on ionizing radiation, using melanin-like pigments to harvest gamma rays. A creature of nuclear decay.",
            4 => "Fermenter — it breaks down organic compounds anaerobically, producing waste gases. Ancient, simple, effective.",
            5 => "Osmotrophic — it absorbs dissolved nutrients directly through its surface, no mouth or gut needed.",
            6 => "Thermosynthetic — it harvests energy from temperature gradients, living at the boundary between hot and cold.",
            _ => "Heterotrophic — it eats other organisms. A consumer, part of the food web that recycles matter and energy.",
        };
        lines.push(energy.to_string());

        // Cognition
        let mind = if g.cognition > 0.8 {
            "Sapient — fully self-aware, capable of abstract thought, language, and tool use. It asks 'why?' and builds things to find answers."
        } else if g.cognition > 0.6 {
            "Tool-using intelligence — it solves novel problems, uses objects as tools, and may have rudimentary culture. Like crows or chimpanzees on Earth."
        } else if g.cognition > 0.4 {
            "Problem-solver — it learns from experience, remembers solutions, and adapts its behavior. Like an octopus, surprising in its cleverness."
        } else if g.cognition > 0.2 {
            "Learning-capable — it modifies behavior based on experience. Simple conditioning, but enough to adapt to changing environments."
        } else if g.cognition > 0.1 {
            "Basic taxis — it moves toward nutrients and away from danger, but doesn't learn. Pure chemical reflexes."
        } else {
            "Reactive — it responds to stimuli but cannot learn. Perfectly adapted through evolution alone."
        };
        lines.push(mind.to_string());

        // Social structure
        let social = if g.collective > 0.8 {
            "Eusocial superorganism — like ants or bees taken to the extreme. Individuals are expendable; the colony is the true organism."
        } else if g.collective > 0.6 {
            "Eusocial — specialized castes (workers, soldiers, queens), with individuals sacrificing reproduction for the colony."
        } else if g.collective > 0.4 {
            "Herd/school/flock — they move and feed together for safety, but each individual is independent."
        } else if g.collective > 0.2 {
            "Loosely social — small groups, pair bonds, or territorial neighbors. They cooperate when it benefits them."
        } else {
            "Solitary — each individual lives alone, meeting others only to mate."
        };
        lines.push(social.to_string());

        // Senses
        let senses = g.sense_list();
        if !senses.is_empty() {
            let richness = if senses.len() >= 5 {
                " A rich sensory world — it perceives reality in ways we can barely imagine."
            } else if senses.len() >= 3 {
                ""
            } else {
                " A simple sensory world, but sufficient for survival."
            };
            lines.push(format!(
                "Senses: {}.{}",
                senses.join(", "),
                richness
            ));
        }

        // Locomotion
        let motion = match g.motility {
            0 => "Sessile — rooted in place for its entire life, like a plant or coral.",
            1 => "Passive drifter — carried by currents of wind or water.",
            2 => "Flagellar propulsion — tiny whip-like appendages drive it through liquid.",
            3 => "Crawling — muscular contractions move it slowly across surfaces.",
            4 => "Swimming — fins, jets, or undulation propel it through water.",
            5 => "Walking or running — limbs carry it across solid ground.",
            6 => "Gliding or burrowing — it moves through air or soil with minimal energy.",
            _ => "Flight — wings or gas bladders lift it above the surface.",
        };
        lines.push(motion.to_string());

        // Reproduction
        let repro = match g.propagation {
            0 => "Reproduces by binary fission — splitting in two. No parents, no children, just copies.",
            1 => "Budding — new individuals grow from the parent's body like branches.",
            2 => "Spores — tiny, tough packets of genetic material scattered on the wind.",
            3 => "Fragmentation — pieces break off and grow into new organisms.",
            4 => "Sexual reproduction — two parents combine DNA, creating unique offspring every generation.",
            5 => "Parthenogenesis — females produce offspring without mating. Males are optional.",
            _ => "Reproduces by simple division.",
        };
        lines.push(repro.to_string());

        // Technology
        if self.bio.has_technology {
            lines.push("It has developed technology — tools, structures, perhaps even language and mathematics. One of the rarest achievements in the cosmos.".to_string());
        }

        // Home
        lines.push(format!(
            "Home: {} planet at {:.0}K, orbiting a {} star at {:.1} AU. {}",
            self.planet_type,
            self.planet_temp,
            self.star_class,
            self.orbital_radius_au,
            if self.planet_has_water { "Liquid water on the surface." }
            else if self.planet_temp > 500.0 { "Surface glows with heat." }
            else if self.planet_temp < 200.0 { "Locked in ice." }
            else { "Dry, airless — yet life endures." }
        ));

        lines.push(format!(
            "Age: {:.1} Gyr. {} species. Biomass: {:.1}.",
            self.bio.age, fmt_count(self.bio.species_count), self.bio.biomass
        ));

        lines.join("\n")
    }
}

fn fmt_count(n: u64) -> String {
    if n >= 1_000_000 { format!("{:.1}M", n as f64 / 1e6) }
    else if n >= 1_000 { format!("{:.1}K", n as f64 / 1e3) }
    else { format!("{}", n) }
}

fn spectral_name(temp: f64) -> String {
    if temp > 30000.0 { "O-type blue giant".into() }
    else if temp > 10000.0 { "B-type blue-white".into() }
    else if temp > 7500.0 { "A-type white".into() }
    else if temp > 6000.0 { "F-type yellow-white".into() }
    else if temp > 5200.0 { "G-type yellow (Sun-like)".into() }
    else if temp > 3700.0 { "K-type orange".into() }
    else { "M-type red dwarf".into() }
}

fn planet_type_name(pt: &matrix_core::PlanetType) -> &'static str {
    match pt {
        matrix_core::PlanetType::Rocky => "Rocky",
        matrix_core::PlanetType::GasGiant => "Gas Giant",
        matrix_core::PlanetType::IceGiant => "Ice Giant",
        matrix_core::PlanetType::Ocean => "Ocean",
        matrix_core::PlanetType::Lava => "Lava",
        matrix_core::PlanetType::Frozen => "Frozen",
    }
}

fn main() {
    let num_universes = 100;
    let ages = [8.0, 10.0, 13.8, 18.0, 25.0, 30.0];
    let sample_regions = 20;

    eprintln!("Simulating {} universes...", num_universes);

    let mut all_creatures: Vec<Creature> = Vec::new();

    // Track substrate counts for stats
    let mut substrate_counts = [0u32; 8];
    let mut total_life_planets = 0u32;
    let mut total_civ = 0u32;
    let mut universes_with_life = 0u32;
    let mut universes_with_civ = 0u32;

    for u in 0..num_universes {
        let seed = 1000 + u as u64 * 7919;
        let config = SimConfig { seed, ..SimConfig::default() };

        let mut found_life = false;
        let mut found_civ = false;

        for &age in &ages {
            let regions = procgen::generate_regions(&config, age);
            let mut sorted: Vec<_> = regions.iter().collect();
            sorted.sort_by(|a, b| b.density.partial_cmp(&a.density).unwrap());
            sorted.truncate(sample_regions);

            for region in &sorted {
                let stars = procgen::generate_stellar_detail(region, age);
                for star in &stars {
                    for planet in &star.planets {
                        if let Some(ref bio) = planet.life {
                            total_life_planets += 1;
                            found_life = true;

                            let sub = (bio.dominant_genome.substrate as usize).min(7);
                            substrate_counts[sub] += 1;

                            if bio.has_technology {
                                total_civ += 1;
                                found_civ = true;
                            }

                            all_creatures.push(Creature {
                                universe_id: u as u32,
                                universe_seed: seed,
                                age_gyr: age,
                                star_class: spectral_name(star.surface_temp),
                                planet_type: planet_type_name(&planet.planet_type).to_string(),
                                planet_temp: planet.surface_temp,
                                planet_has_water: planet.has_water,
                                orbital_radius_au: planet.orbital_radius,
                                bio: bio.clone(),
                            });
                        }
                    }
                }
            }
        }

        if found_life { universes_with_life += 1; }
        if found_civ { universes_with_civ += 1; }

        if (u + 1) % 20 == 0 {
            eprint!("  {}/{}...\r", u + 1, num_universes);
        }
    }
    eprintln!("Done. Found {} life forms across {} universes.", all_creatures.len(), num_universes);

    // Sort by uniqueness and pick the most interesting, but ensure diversity
    all_creatures.sort_by(|a, b| b.uniqueness_score().partial_cmp(&a.uniqueness_score()).unwrap());

    // Pick top creatures but ensure different substrates/structures are represented
    let mut selected: Vec<&Creature> = Vec::new();
    let mut seen_combos: HashMap<(u32, u32), u32> = HashMap::new(); // (substrate, structure) -> count

    for c in &all_creatures {
        let key = (c.bio.dominant_genome.substrate, c.bio.dominant_genome.structure);
        let count = seen_combos.entry(key).or_insert(0);
        if *count < 1 {
            selected.push(c);
            *count += 1;
            if selected.len() >= 12 {
                break;
            }
        }
    }

    // Print the catalogue
    println!();
    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║    CATALOGUE OF LIFE: 100 SIMULATED UNIVERSES              ║");
    println!("║    {} life forms found on {} life-bearing planets           ", all_creatures.len(), total_life_planets);
    println!("║    {}/{} universes developed life                           ", universes_with_life, num_universes);
    println!("║    {}/{} developed civilizations                            ", universes_with_civ, num_universes);
    println!("║    {} total technological civilizations                     ", total_civ);
    println!("╚══════════════════════════════════════════════════════════════╝");
    println!();

    // Substrate breakdown
    let total: u64 = substrate_counts.iter().map(|&c| c as u64).sum();
    let names = ["Carbon-water", "Carbon-ammonia", "Carbon-methane", "Silicon",
                  "Sulfur-iron", "Hydrocarbon", "(unused)", "(unused)"];
    println!("SUBSTRATE CENSUS:");
    for (i, name) in names.iter().enumerate() {
        let pct = if total > 0 { substrate_counts[i] as f64 / total as f64 * 100.0 } else { 0.0 };
        let bar = "█".repeat((pct * 0.4) as usize);
        println!("  {:14} {:>5.1}% {}", name, pct, bar);
    }
    println!();

    // Print detailed portraits
    println!("════════════════════════════════════════════════════════════════");
    println!("THE {} MOST REMARKABLE LIFE FORMS", selected.len());
    println!("════════════════════════════════════════════════════════════════");

    for (i, c) in selected.iter().enumerate() {
        let g = &c.bio.dominant_genome;
        println!();
        println!("━━━ SPECIES #{}: {} ━━━", i + 1, g.describe().to_uppercase());
        println!("Universe #{} (seed {}) | Age: {:.1} billion years", c.universe_id + 1, c.universe_seed, c.age_gyr);
        if c.bio.has_technology {
            println!("⚡ TECHNOLOGICAL CIVILIZATION");
        }
        println!();
        println!("{}", c.portrait());
        println!();
    }
}
