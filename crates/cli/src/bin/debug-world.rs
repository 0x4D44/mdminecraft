//! Debug World Tool
//!
//! Debugging utility for world generation visualization and validation.
//!
//! Features:
//! - Heightmap visualization (ASCII art)
//! - Biome map display
//! - Seam validation between chunks
//! - Chunk data inspection
//!
//! Usage:
//!   debug-world heightmap --seed 12345 --region -2,-2,2,2
//!   debug-world biomes --seed 12345 --region -5,-5,5,5
//!   debug-world validate-seams --seed 12345 --region -3,-3,3,3

use mdminecraft_world::{BiomeAssigner, BiomeId, Heightmap, CHUNK_SIZE_X, CHUNK_SIZE_Z};
use std::env;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;

#[derive(Debug)]
struct Config {
    command: Command,
    seed: u64,
    output: Option<PathBuf>,
}

#[derive(Debug)]
enum Command {
    Heightmap {
        min_x: i32,
        min_z: i32,
        max_x: i32,
        max_z: i32,
    },
    Biomes {
        min_x: i32,
        min_z: i32,
        max_x: i32,
        max_z: i32,
    },
    ValidateSeams {
        min_x: i32,
        min_z: i32,
        max_x: i32,
        max_z: i32,
    },
    Help,
}

fn parse_args() -> Result<Config, String> {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        return Ok(Config {
            command: Command::Help,
            seed: 0,
            output: None,
        });
    }

    let command_str = &args[1];

    // Parse common options
    let mut seed = 12345u64;
    let mut output = None;
    let mut region: Option<(i32, i32, i32, i32)> = None;

    let mut i = 2;
    while i < args.len() {
        match args[i].as_str() {
            "--seed" => {
                if i + 1 >= args.len() {
                    return Err("--seed requires an argument".to_string());
                }
                seed = args[i + 1]
                    .parse()
                    .map_err(|e| format!("Invalid seed: {}", e))?;
                i += 2;
            }
            "--region" => {
                if i + 1 >= args.len() {
                    return Err("--region requires an argument".to_string());
                }
                // Parse format: min_x,min_z,max_x,max_z
                let parts: Vec<&str> = args[i + 1].split(',').collect();
                if parts.len() != 4 {
                    return Err("--region format: min_x,min_z,max_x,max_z".to_string());
                }
                let min_x: i32 = parts[0]
                    .parse()
                    .map_err(|e| format!("Invalid min_x: {}", e))?;
                let min_z: i32 = parts[1]
                    .parse()
                    .map_err(|e| format!("Invalid min_z: {}", e))?;
                let max_x: i32 = parts[2]
                    .parse()
                    .map_err(|e| format!("Invalid max_x: {}", e))?;
                let max_z: i32 = parts[3]
                    .parse()
                    .map_err(|e| format!("Invalid max_z: {}", e))?;
                region = Some((min_x, min_z, max_x, max_z));
                i += 2;
            }
            "--output" | "-o" => {
                if i + 1 >= args.len() {
                    return Err("--output requires an argument".to_string());
                }
                output = Some(PathBuf::from(&args[i + 1]));
                i += 2;
            }
            _ => {
                return Err(format!("Unknown option: {}", args[i]));
            }
        }
    }

    let command = match command_str.as_str() {
        "heightmap" => {
            let (min_x, min_z, max_x, max_z) =
                region.ok_or("heightmap requires --region option")?;
            Command::Heightmap {
                min_x,
                min_z,
                max_x,
                max_z,
            }
        }
        "biomes" => {
            let (min_x, min_z, max_x, max_z) = region.ok_or("biomes requires --region option")?;
            Command::Biomes {
                min_x,
                min_z,
                max_x,
                max_z,
            }
        }
        "validate-seams" => {
            let (min_x, min_z, max_x, max_z) =
                region.ok_or("validate-seams requires --region option")?;
            Command::ValidateSeams {
                min_x,
                min_z,
                max_x,
                max_z,
            }
        }
        "help" | "--help" | "-h" => Command::Help,
        _ => {
            return Err(format!(
                "Unknown command: {}\nRun 'debug-world help' for usage",
                command_str
            ));
        }
    };

    Ok(Config {
        command,
        seed,
        output,
    })
}

fn print_help() {
    println!("Debug World Tool - World generation debugging utility");
    println!();
    println!("Usage:");
    println!("  debug-world <command> [options]");
    println!();
    println!("Commands:");
    println!("  heightmap          Generate and visualize heightmap (ASCII art)");
    println!("  biomes             Display biome distribution map");
    println!("  validate-seams     Validate heightmap continuity at chunk boundaries");
    println!("  help               Show this help message");
    println!();
    println!("Options:");
    println!("  --seed <number>           World seed (default: 12345)");
    println!("  --region <x1,z1,x2,z2>    Chunk region to process");
    println!("  --output <file>           Output file (default: stdout)");
    println!();
    println!("Examples:");
    println!("  # Visualize heightmap for 5x5 chunk region");
    println!("  debug-world heightmap --seed 12345 --region -2,-2,2,2");
    println!();
    println!("  # Show biome distribution");
    println!("  debug-world biomes --seed 12345 --region -5,-5,5,5 --output biomes.txt");
    println!();
    println!("  # Validate seams in region");
    println!("  debug-world validate-seams --seed 12345 --region -10,-10,10,10");
}

fn visualize_heightmap(
    seed: u64,
    min_x: i32,
    min_z: i32,
    max_x: i32,
    max_z: i32,
    output: Option<PathBuf>,
) {
    println!("Generating heightmap visualization...");
    println!("Seed: {}", seed);
    println!(
        "Region: chunks ({}, {}) to ({}, {})",
        min_x, min_z, max_x, max_z
    );
    println!();

    let width = ((max_x - min_x + 1) * CHUNK_SIZE_X as i32) as usize;
    let height = ((max_z - min_z + 1) * CHUNK_SIZE_Z as i32) as usize;

    println!("Map size: {} x {} blocks", width, height);
    println!();

    // Generate heightmaps for all chunks in region
    let mut heightmaps = Vec::new();
    for chunk_z in min_z..=max_z {
        for chunk_x in min_x..=max_x {
            let hm = Heightmap::generate(seed, chunk_x, chunk_z);
            heightmaps.push(((chunk_x, chunk_z), hm));
        }
    }

    // Find global min/max for better visualization
    let global_min = heightmaps
        .iter()
        .map(|(_, hm)| hm.min_height())
        .min()
        .unwrap_or(0);
    let global_max = heightmaps
        .iter()
        .map(|(_, hm)| hm.max_height())
        .max()
        .unwrap_or(255);

    println!("Height range: {} to {}", global_min, global_max);
    println!();

    // Create ASCII visualization
    let mut visualization = String::new();

    // Header
    visualization.push_str(&format!("Heightmap Visualization (Seed: {})\n", seed));
    visualization.push_str(&format!(
        "Region: chunks ({}, {}) to ({}, {})\n",
        min_x, min_z, max_x, max_z
    ));
    visualization.push_str(&format!("Height range: {} to {}\n", global_min, global_max));
    visualization
        .push_str("\nLegend: █ = high, ▓ = med-high, ▒ = med-low, ░ = low, · = very low\n\n");

    // Generate visualization
    for world_z in 0..height {
        for world_x in 0..width {
            // Calculate which chunk this belongs to
            let chunk_x = min_x + (world_x as i32 / CHUNK_SIZE_X as i32);
            let chunk_z = min_z + (world_z as i32 / CHUNK_SIZE_Z as i32);
            let local_x = world_x % CHUNK_SIZE_X;
            let local_z = world_z % CHUNK_SIZE_Z;

            // Find the heightmap
            let hm = heightmaps
                .iter()
                .find(|((cx, cz), _)| *cx == chunk_x && *cz == chunk_z)
                .map(|(_, hm)| hm);

            if let Some(hm) = hm {
                let h = hm.get(local_x, local_z);
                let normalized = if global_max > global_min {
                    ((h - global_min) as f32 / (global_max - global_min) as f32)
                } else {
                    0.5
                };

                let char = if normalized > 0.8 {
                    '█'
                } else if normalized > 0.6 {
                    '▓'
                } else if normalized > 0.4 {
                    '▒'
                } else if normalized > 0.2 {
                    '░'
                } else {
                    '·'
                };

                visualization.push(char);
            } else {
                visualization.push('?');
            }
        }
        visualization.push('\n');
    }

    // Output
    if let Some(path) = output {
        let mut file = File::create(&path).expect("Failed to create output file");
        file.write_all(visualization.as_bytes())
            .expect("Failed to write to file");
        println!("Heightmap saved to: {}", path.display());
    } else {
        print!("{}", visualization);
    }
}

fn visualize_biomes(
    seed: u64,
    min_x: i32,
    min_z: i32,
    max_x: i32,
    max_z: i32,
    output: Option<PathBuf>,
) {
    println!("Generating biome map...");
    println!("Seed: {}", seed);
    println!(
        "Region: chunks ({}, {}) to ({}, {})",
        min_x, min_z, max_x, max_z
    );
    println!();

    let biome_assigner = BiomeAssigner::new(seed);

    let width = ((max_x - min_x + 1) * CHUNK_SIZE_X as i32) as usize;
    let height = ((max_z - min_z + 1) * CHUNK_SIZE_Z as i32) as usize;

    println!("Map size: {} x {} blocks", width, height);
    println!();

    // Create biome visualization
    let mut visualization = String::new();

    // Header
    visualization.push_str(&format!("Biome Map (Seed: {})\n", seed));
    visualization.push_str(&format!(
        "Region: chunks ({}, {}) to ({}, {})\n\n",
        min_x, min_z, max_x, max_z
    ));
    visualization.push_str("Legend:\n");
    visualization.push_str("  O = Ocean         Ø = DeepOcean     D = Desert        P = Plains\n");
    visualization.push_str("  F = Forest        B = BirchForest   M = Mountains     H = Hills\n");
    visualization.push_str("  I = IcePlains     Î = IceMountains  T = Tundra        W = Swamp\n");
    visualization.push_str("  R = RainForest    S = Savanna\n\n");

    // Generate visualization
    for world_z in 0..height {
        for world_x in 0..width {
            let block_x = min_x * CHUNK_SIZE_X as i32 + world_x as i32;
            let block_z = min_z * CHUNK_SIZE_Z as i32 + world_z as i32;

            let biome = biome_assigner.get_biome(block_x, block_z);
            let char = biome_to_char(biome);
            visualization.push(char);
        }
        visualization.push('\n');
    }

    // Output
    if let Some(path) = output {
        let mut file = File::create(&path).expect("Failed to create output file");
        file.write_all(visualization.as_bytes())
            .expect("Failed to write to file");
        println!("Biome map saved to: {}", path.display());
    } else {
        print!("{}", visualization);
    }
}

fn biome_to_char(biome: BiomeId) -> char {
    match biome {
        BiomeId::Ocean => 'O',
        BiomeId::DeepOcean => 'Ø',
        BiomeId::Desert => 'D',
        BiomeId::Plains => 'P',
        BiomeId::Forest => 'F',
        BiomeId::BirchForest => 'B',
        BiomeId::Mountains => 'M',
        BiomeId::Hills => 'H',
        BiomeId::IcePlains => 'I',
        BiomeId::IceMountains => 'Î',
        BiomeId::Tundra => 'T',
        BiomeId::Swamp => 'W',
        BiomeId::RainForest => 'R',
        BiomeId::Savanna => 'S',
    }
}

fn validate_seams(seed: u64, min_x: i32, min_z: i32, max_x: i32, max_z: i32) {
    println!("Validating heightmap seams...");
    println!("Seed: {}", seed);
    println!(
        "Region: chunks ({}, {}) to ({}, {})",
        min_x, min_z, max_x, max_z
    );
    println!();

    let mut total_seams = 0;
    let mut mismatches = 0;
    let mut max_discrepancy = 0i32;

    // Validate horizontal seams (X direction)
    for chunk_z in min_z..=max_z {
        for chunk_x in min_x..max_x {
            let hm1 = Heightmap::generate(seed, chunk_x, chunk_z);
            let hm2 = Heightmap::generate(seed, chunk_x + 1, chunk_z);

            // Check seam at eastern edge of chunk1 vs western edge of chunk2
            for local_z in 0..CHUNK_SIZE_Z {
                total_seams += 1;
                let h1 = hm1.get(CHUNK_SIZE_X - 1, local_z);
                let h2 = hm2.get(0, local_z);

                if h1 != h2 {
                    mismatches += 1;
                    let discrepancy = (h1 - h2).abs();
                    if discrepancy > max_discrepancy {
                        max_discrepancy = discrepancy;
                    }
                }
            }
        }
    }

    // Validate vertical seams (Z direction)
    for chunk_x in min_x..=max_x {
        for chunk_z in min_z..max_z {
            let hm1 = Heightmap::generate(seed, chunk_x, chunk_z);
            let hm2 = Heightmap::generate(seed, chunk_x, chunk_z + 1);

            // Check seam at southern edge of chunk1 vs northern edge of chunk2
            for local_x in 0..CHUNK_SIZE_X {
                total_seams += 1;
                let h1 = hm1.get(local_x, CHUNK_SIZE_Z - 1);
                let h2 = hm2.get(local_x, 0);

                if h1 != h2 {
                    mismatches += 1;
                    let discrepancy = (h1 - h2).abs();
                    if discrepancy > max_discrepancy {
                        max_discrepancy = discrepancy;
                    }
                }
            }
        }
    }

    // Report results
    println!("╔══════════════════════════════════════════════════════════╗");
    println!("║              Seam Validation Results                    ║");
    println!("╚══════════════════════════════════════════════════════════╝");
    println!();
    println!("  Total seams checked:     {}", total_seams);
    println!("  Mismatches found:        {}", mismatches);
    println!(
        "  Match rate:              {:.2}%",
        (total_seams - mismatches) as f64 / total_seams as f64 * 100.0
    );

    if mismatches > 0 {
        println!("  Max discrepancy:         {} blocks", max_discrepancy);
        println!();
        println!("❌ FAILED: Heightmap seams are not continuous");
        std::process::exit(1);
    } else {
        println!();
        println!("✅ SUCCESS: All heightmap seams are perfectly continuous");
    }
}

fn main() {
    let config = match parse_args() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Error: {}", e);
            eprintln!();
            print_help();
            std::process::exit(1);
        }
    };

    match config.command {
        Command::Heightmap {
            min_x,
            min_z,
            max_x,
            max_z,
        } => {
            visualize_heightmap(config.seed, min_x, min_z, max_x, max_z, config.output);
        }
        Command::Biomes {
            min_x,
            min_z,
            max_x,
            max_z,
        } => {
            visualize_biomes(config.seed, min_x, min_z, max_x, max_z, config.output);
        }
        Command::ValidateSeams {
            min_x,
            min_z,
            max_x,
            max_z,
        } => {
            validate_seams(config.seed, min_x, min_z, max_x, max_z);
        }
        Command::Help => {
            print_help();
        }
    }
}
