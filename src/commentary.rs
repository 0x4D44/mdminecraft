use mdminecraft_world::{MobType, WeatherState};
use rand::{rngs::StdRng, Rng, SeedableRng};
use std::collections::VecDeque;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy)]
pub enum CommentaryStyle {
    Teen,
    Normal,
}

impl CommentaryStyle {
    pub fn parse(raw: &str) -> Option<Self> {
        match raw.trim().to_lowercase().as_str() {
            "teen" | "teenage" | "teen-slang" | "teen_slang" | "slang" => Some(Self::Teen),
            "normal" | "plain" | "neutral" | "standard" => Some(Self::Normal),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CommentaryConfig {
    pub log_path: PathBuf,
    pub style: CommentaryStyle,
    pub min_interval_ms: u64,
    pub max_interval_ms: u64,
}

#[derive(Debug, Clone, Copy)]
pub struct CommentarySample {
    pub tick: u64,
    pub time_of_day: f32,
    pub weather: WeatherState,
    pub mobs_nearby: u32,
    pub nearby_mob: Option<MobType>,
    pub pos: [f32; 3],
    pub visual: Option<VisualTags>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TimePhase {
    Dawn,
    Day,
    Sunset,
    Night,
}

#[derive(Debug, Clone, Copy)]
pub struct VisualTags {
    pub sky_ratio: f32,
    pub water_ratio: f32,
    pub grass_ratio: f32,
    pub sand_ratio: f32,
    pub stone_ratio: f32,
    pub wood_ratio: f32,
    pub dark_ratio: f32,
    pub bright_ratio: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LineKind {
    VisualSky,
    VisualWater,
    VisualGrass,
    VisualSand,
    VisualStone,
    VisualWood,
    VisualBright,
    VisualDark,
    VisualGeneric,
    Weather,
    Time,
    Teleport,
    MobCluster,
    Mob,
    ElevationHigh,
    ElevationLow,
    Filler,
}

pub struct CommentaryRuntime {
    cfg: CommentaryConfig,
    writer: BufWriter<File>,
    rng: StdRng,
    next_emit_ms: u64,
    last_weather: WeatherState,
    last_phase: TimePhase,
    last_pos: Option<[f32; 3]>,
    last_line_kind: Option<LineKind>,
    recent_lines: VecDeque<String>,
}

impl CommentaryRuntime {
    pub fn new(cfg: CommentaryConfig, seed: u64) -> anyhow::Result<Self> {
        if let Some(parent) = cfg.log_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let file = File::create(&cfg.log_path)?;
        let writer = BufWriter::new(file);
        let mut runtime = Self {
            rng: StdRng::seed_from_u64(seed ^ 0x434F_4D4D_454E_5441),
            next_emit_ms: 0,
            last_weather: WeatherState::Clear,
            last_phase: TimePhase::Day,
            last_pos: None,
            last_line_kind: None,
            recent_lines: VecDeque::new(),
            cfg,
            writer,
        };
        runtime.roll_next_interval(0);
        Ok(runtime)
    }

    pub fn tick(&mut self, sample: CommentarySample) {
        let time_ms = (sample.tick.saturating_mul(1000)) / 20;
        if time_ms < self.next_emit_ms {
            self.last_pos = Some(sample.pos);
            return;
        }

        let phase = time_phase(sample.time_of_day);
        let teleported = self
            .last_pos
            .map(|prev| distance_sq(prev, sample.pos) > 144.0)
            .unwrap_or(false);
        let high_elevation = sample.pos[1] > 85.0;
        let low_elevation = sample.pos[1] < 62.0;
        let mob_cluster = sample.mobs_nearby >= 5;

        let visual_line = sample.visual.and_then(|tags| self.visual_line(tags));
        let (kind, line) = if let Some((kind, line)) = visual_line {
            (kind, line)
        } else if sample.weather != self.last_weather {
            self.last_weather = sample.weather;
            (LineKind::Weather, self.weather_line(sample.weather))
        } else if phase != self.last_phase {
            self.last_phase = phase;
            (LineKind::Time, self.time_line(phase))
        } else if teleported {
            (LineKind::Teleport, self.teleport_line())
        } else if mob_cluster {
            (LineKind::MobCluster, self.mob_cluster_line())
        } else if let Some(mob) = sample.nearby_mob {
            (LineKind::Mob, self.mob_line(mob))
        } else if high_elevation {
            (LineKind::ElevationHigh, self.elevation_line(true))
        } else if low_elevation {
            (LineKind::ElevationLow, self.elevation_line(false))
        } else {
            (LineKind::Filler, self.filler_line())
        };

        let event = serde_json::json!({
            "t_ms": time_ms,
            "text": line,
        });
        if serde_json::to_writer(&mut self.writer, &event).is_ok() {
            let _ = writeln!(self.writer);
            let _ = self.writer.flush();
        }

        self.last_pos = Some(sample.pos);
        self.note_line(kind, &line);
        self.roll_next_interval(time_ms);
    }

    fn roll_next_interval(&mut self, now_ms: u64) {
        let min = self.cfg.min_interval_ms.max(500);
        let max = self.cfg.max_interval_ms.max(min + 500);
        let delta = if max <= min {
            min
        } else {
            self.rng.gen_range(min..=max)
        };
        self.next_emit_ms = now_ms.saturating_add(delta);
    }

    fn note_line(&mut self, kind: LineKind, line: &str) {
        self.last_line_kind = Some(kind);
        self.recent_lines.push_back(line.to_string());
        while self.recent_lines.len() > 5 {
            self.recent_lines.pop_front();
        }
    }

    fn pick_phrase(&mut self, pool: &[&str]) -> String {
        if pool.is_empty() {
            return String::new();
        }
        for _ in 0..6 {
            let candidate = pick(&mut self.rng, pool);
            if !self.recent_lines.iter().any(|line| line == candidate) {
                return candidate.to_string();
            }
        }
        pick(&mut self.rng, pool).to_string()
    }

    fn visual_line(&mut self, tags: VisualTags) -> Option<(LineKind, String)> {
        let (sky, water, grass, sand, stone, wood, bright, dark, generic) = match self.cfg.style {
            CommentaryStyle::Teen => (
                VISUAL_SKY_PHRASES,
                VISUAL_WATER_PHRASES,
                VISUAL_GRASS_PHRASES,
                VISUAL_SAND_PHRASES,
                VISUAL_STONE_PHRASES,
                VISUAL_WOOD_PHRASES,
                VISUAL_BRIGHT_PHRASES,
                VISUAL_DARK_PHRASES,
                VISUAL_GENERIC_PHRASES,
            ),
            CommentaryStyle::Normal => (
                VISUAL_SKY_NORMAL,
                VISUAL_WATER_NORMAL,
                VISUAL_GRASS_NORMAL,
                VISUAL_SAND_NORMAL,
                VISUAL_STONE_NORMAL,
                VISUAL_WOOD_NORMAL,
                VISUAL_BRIGHT_NORMAL,
                VISUAL_DARK_NORMAL,
                VISUAL_GENERIC_NORMAL,
            ),
        };

        let mut options: Vec<(LineKind, &[&str])> = Vec::new();
        if tags.sky_ratio > 0.14 {
            options.push((LineKind::VisualSky, sky));
        }
        if tags.water_ratio > 0.08 {
            options.push((LineKind::VisualWater, water));
        }
        if tags.grass_ratio > 0.10 {
            options.push((LineKind::VisualGrass, grass));
        }
        if tags.sand_ratio > 0.06 {
            options.push((LineKind::VisualSand, sand));
        }
        if tags.stone_ratio > 0.10 {
            options.push((LineKind::VisualStone, stone));
        }
        if tags.wood_ratio > 0.06 {
            options.push((LineKind::VisualWood, wood));
        }
        if tags.bright_ratio > 0.32 {
            options.push((LineKind::VisualBright, bright));
        }
        if tags.dark_ratio > 0.32 {
            options.push((LineKind::VisualDark, dark));
        }

        if options.is_empty() {
            options.push((LineKind::VisualGeneric, generic));
        }

        let mut filtered: Vec<(LineKind, &[&str])> = options
            .iter()
            .copied()
            .filter(|(kind, _)| Some(*kind) != self.last_line_kind)
            .collect();
        if filtered.is_empty() {
            filtered = options;
        }

        let idx = self.rng.gen_range(0..filtered.len());
        let (kind, pool) = filtered[idx];
        let line = self.pick_phrase(pool);
        Some((kind, line))
    }

    fn weather_line(&mut self, weather: WeatherState) -> String {
        match self.cfg.style {
            CommentaryStyle::Teen => match weather {
                WeatherState::Clear => self.pick_phrase(WEATHER_CLEAR_PHRASES),
                WeatherState::Precipitation => self.pick_phrase(WEATHER_RAIN_PHRASES),
                WeatherState::Thunderstorm => self.pick_phrase(WEATHER_THUNDER_PHRASES),
            },
            CommentaryStyle::Normal => match weather {
                WeatherState::Clear => self.pick_phrase(WEATHER_CLEAR_NORMAL),
                WeatherState::Precipitation => self.pick_phrase(WEATHER_RAIN_NORMAL),
                WeatherState::Thunderstorm => self.pick_phrase(WEATHER_THUNDER_NORMAL),
            },
        }
    }

    fn time_line(&mut self, phase: TimePhase) -> String {
        match self.cfg.style {
            CommentaryStyle::Teen => match phase {
                TimePhase::Dawn => self.pick_phrase(TIME_DAWN_PHRASES),
                TimePhase::Day => self.pick_phrase(TIME_DAY_PHRASES),
                TimePhase::Sunset => self.pick_phrase(TIME_SUNSET_PHRASES),
                TimePhase::Night => self.pick_phrase(TIME_NIGHT_PHRASES),
            },
            CommentaryStyle::Normal => match phase {
                TimePhase::Dawn => self.pick_phrase(TIME_DAWN_NORMAL),
                TimePhase::Day => self.pick_phrase(TIME_DAY_NORMAL),
                TimePhase::Sunset => self.pick_phrase(TIME_SUNSET_NORMAL),
                TimePhase::Night => self.pick_phrase(TIME_NIGHT_NORMAL),
            },
        }
    }

    fn teleport_line(&mut self) -> String {
        match self.cfg.style {
            CommentaryStyle::Teen => self.pick_phrase(TELEPORT_PHRASES),
            CommentaryStyle::Normal => self.pick_phrase(TELEPORT_NORMAL),
        }
    }

    fn mob_cluster_line(&mut self) -> String {
        match self.cfg.style {
            CommentaryStyle::Teen => self.pick_phrase(MOB_CLUSTER_PHRASES),
            CommentaryStyle::Normal => self.pick_phrase(MOB_CLUSTER_NORMAL),
        }
    }

    fn mob_line(&mut self, mob: MobType) -> String {
        match self.cfg.style {
            CommentaryStyle::Teen => match mob {
                MobType::Sheep => self.pick_phrase(MOB_SHEEP_PHRASES),
                MobType::Cow => self.pick_phrase(MOB_COW_PHRASES),
                MobType::Chicken => self.pick_phrase(MOB_CHICKEN_PHRASES),
                MobType::Pig => self.pick_phrase(MOB_PIG_PHRASES),
                MobType::Villager => self.pick_phrase(MOB_VILLAGER_PHRASES),
                MobType::Zombie | MobType::Skeleton | MobType::Spider | MobType::Creeper => {
                    self.pick_phrase(MOB_HOSTILE_PHRASES)
                }
                _ => self.pick_phrase(MOB_GENERIC_PHRASES),
            },
            CommentaryStyle::Normal => match mob {
                MobType::Sheep => self.pick_phrase(MOB_SHEEP_NORMAL),
                MobType::Cow => self.pick_phrase(MOB_COW_NORMAL),
                MobType::Chicken => self.pick_phrase(MOB_CHICKEN_NORMAL),
                MobType::Pig => self.pick_phrase(MOB_PIG_NORMAL),
                MobType::Villager => self.pick_phrase(MOB_VILLAGER_NORMAL),
                MobType::Zombie | MobType::Skeleton | MobType::Spider | MobType::Creeper => {
                    self.pick_phrase(MOB_HOSTILE_NORMAL)
                }
                _ => self.pick_phrase(MOB_GENERIC_NORMAL),
            },
        }
    }

    fn elevation_line(&mut self, high: bool) -> String {
        match self.cfg.style {
            CommentaryStyle::Teen => {
                if high {
                    self.pick_phrase(ELEVATION_HIGH_PHRASES)
                } else {
                    self.pick_phrase(ELEVATION_LOW_PHRASES)
                }
            }
            CommentaryStyle::Normal => {
                if high {
                    self.pick_phrase(ELEVATION_HIGH_NORMAL)
                } else {
                    self.pick_phrase(ELEVATION_LOW_NORMAL)
                }
            }
        }
    }

    fn filler_line(&mut self) -> String {
        match self.cfg.style {
            CommentaryStyle::Teen => self.pick_phrase(FILLER_PHRASES),
            CommentaryStyle::Normal => self.pick_phrase(FILLER_NORMAL),
        }
    }
}

pub fn analyze_visuals(size: (u32, u32), rgba: &[u8]) -> VisualTags {
    let (width, height) = size;
    if width == 0 || height == 0 || rgba.len() < (width * height * 4) as usize {
        return VisualTags {
            sky_ratio: 0.0,
            water_ratio: 0.0,
            grass_ratio: 0.0,
            sand_ratio: 0.0,
            stone_ratio: 0.0,
            wood_ratio: 0.0,
            dark_ratio: 0.0,
            bright_ratio: 0.0,
        };
    }

    let step = ((width.min(height) / 90).max(2)) as usize;
    let mut total = 0u32;
    let mut sky = 0u32;
    let mut water = 0u32;
    let mut grass = 0u32;
    let mut sand = 0u32;
    let mut stone = 0u32;
    let mut wood = 0u32;
    let mut dark = 0u32;
    let mut bright = 0u32;

    let row_stride = (width * 4) as usize;
    for y in (0..height as usize).step_by(step) {
        let row_offset = y * row_stride;
        for x in (0..width as usize).step_by(step) {
            let idx = row_offset + x * 4;
            if idx + 2 >= rgba.len() {
                continue;
            }
            let r = rgba[idx] as i32;
            let g = rgba[idx + 1] as i32;
            let b = rgba[idx + 2] as i32;
            let brightness = (r + g + b) / 3;

            total = total.saturating_add(1);
            if brightness < 55 {
                dark = dark.saturating_add(1);
            }
            if brightness > 200 {
                bright = bright.saturating_add(1);
            }

            if b > 120 && b > r + 20 && b > g + 10 && brightness > 100 {
                sky = sky.saturating_add(1);
            }
            if b > 80 && b > r + 15 && b > g + 5 && brightness < 130 {
                water = water.saturating_add(1);
            }
            if g > 90 && g > r + 20 && g > b + 10 {
                grass = grass.saturating_add(1);
            }
            if r > 150 && g > 140 && b < 130 && r + g > 320 {
                sand = sand.saturating_add(1);
            }
            if (r - g).abs() < 12 && (g - b).abs() < 12 && (r - b).abs() < 12 {
                if brightness > 60 && brightness < 170 {
                    stone = stone.saturating_add(1);
                }
            }
            if r > 85 && g > 55 && b < 95 && r > g + 10 {
                wood = wood.saturating_add(1);
            }
        }
    }

    let denom = total.max(1) as f32;
    VisualTags {
        sky_ratio: sky as f32 / denom,
        water_ratio: water as f32 / denom,
        grass_ratio: grass as f32 / denom,
        sand_ratio: sand as f32 / denom,
        stone_ratio: stone as f32 / denom,
        wood_ratio: wood as f32 / denom,
        dark_ratio: dark as f32 / denom,
        bright_ratio: bright as f32 / denom,
    }
}

fn time_phase(time_of_day: f32) -> TimePhase {
    if (0.20..0.30).contains(&time_of_day) {
        TimePhase::Dawn
    } else if (0.30..0.70).contains(&time_of_day) {
        TimePhase::Day
    } else if (0.70..0.80).contains(&time_of_day) {
        TimePhase::Sunset
    } else {
        TimePhase::Night
    }
}

fn distance_sq(a: [f32; 3], b: [f32; 3]) -> f32 {
    let dx = a[0] - b[0];
    let dy = a[1] - b[1];
    let dz = a[2] - b[2];
    dx * dx + dy * dy + dz * dz
}

fn pick<'a>(rng: &mut StdRng, pool: &'a [&'a str]) -> &'a str {
    let idx = rng.gen_range(0..pool.len());
    pool[idx]
}

const VISUAL_SKY_PHRASES: &[&str] = &[
    "Yo the sky color is straight up clean.",
    "Okay that sky gradient is kinda fire.",
    "Look at that blue sky, it's popping.",
];
const VISUAL_WATER_PHRASES: &[&str] = &[
    "Water spotted, that shimmer is sick.",
    "Yo the water reflection is kinda clean.",
    "That water looks chill, no cap.",
];
const VISUAL_GRASS_PHRASES: &[&str] = &[
    "Grass blocks everywhere, looks super fresh.",
    "Yo that green vibe is clean.",
    "The grass textures are popping right now.",
];
const VISUAL_SAND_PHRASES: &[&str] = &[
    "Sand over there, beach vibes fr.",
    "Yo that sandy patch looks clean.",
    "Okay, sand blocks? Kinda nice.",
];
const VISUAL_STONE_PHRASES: &[&str] = &[
    "Stone everywhere, that rocky vibe hits.",
    "Yo the stone texture is crisp.",
    "This area is super stony, kinda epic.",
];
const VISUAL_WOOD_PHRASES: &[&str] = &[
    "Wood blocks in view, those textures slap.",
    "Yo the wood grain looks clean.",
    "That wood tone is kinda fire.",
];
const VISUAL_BRIGHT_PHRASES: &[&str] = &[
    "Okay it's bright out, the lighting is wild.",
    "Yo the scene is super bright, love it.",
    "This lighting is clean and sunny, no cap.",
];
const VISUAL_DARK_PHRASES: &[&str] = &[
    "It's pretty dark, kinda spooky vibes.",
    "Yo the shadows are deep right now.",
    "This scene is moody and dark, fr.",
];
const VISUAL_GENERIC_PHRASES: &[&str] = &[
    "These textures look clean from here.",
    "Yo the colors in this scene are popping.",
    "Okay the block detail is low-key fire.",
];

const VISUAL_SKY_NORMAL: &[&str] = &[
    "The skybox shader is handling that gradient.",
    "Clear sky in view; the skybox pass is doing the heavy lifting.",
    "You can see the sky color ramp from the skybox shader.",
];
const VISUAL_WATER_NORMAL: &[&str] = &[
    "Water in view; the renderer shades it with the fluid material.",
    "Those water pixels are coming from the fluid surface pass.",
    "Water reflections here come from the water shading stage.",
];
const VISUAL_GRASS_NORMAL: &[&str] = &[
    "Grass blocks dominate this view; the chunk mesh is doing its job.",
    "Terrain mesh is mostly grass here, built per chunk around the camera.",
    "Lots of grass in frame; chunk meshing is stitching this together.",
];
const VISUAL_SAND_NORMAL: &[&str] = &[
    "Sandy terrain in view; those blocks are part of the chunk mesh.",
    "Sand shows up here as part of the chunk voxel mesh.",
    "That sand patch is coming through the same chunk render path.",
];
const VISUAL_STONE_NORMAL: &[&str] = &[
    "Stone-heavy terrain; the chunk mesh is mostly rock right now.",
    "Stone textures are in focus; voxel meshing is showing through.",
    "Rocky blocks dominate; this is the chunk renderer at work.",
];
const VISUAL_WOOD_NORMAL: &[&str] = &[
    "Wood blocks in view; those are part of the chunk mesh too.",
    "Wood textures stand out; they ride the same voxel pipeline.",
    "You can see the wood tones coming from the block textures.",
];
const VISUAL_BRIGHT_NORMAL: &[&str] = &[
    "Bright lighting here; time-of-day lighting is keeping it vivid.",
    "High brightness on screen; the lighting pass is in full daylight.",
    "This is a bright frame; lighting is driven by the sim time.",
];
const VISUAL_DARK_NORMAL: &[&str] = &[
    "Darker scene now; the time-of-day lighting is dropping.",
    "Shadows deepen here as the lighting shifts with time.",
    "This looks dim; the lighting system is dialing things down.",
];
const VISUAL_GENERIC_NORMAL: &[&str] = &[
    "General terrain view; the renderer is drawing chunk meshes and skybox.",
    "You can see the voxel terrain pass and skybox working together.",
    "This frame is a clean example of the chunk + skybox pipeline.",
];

const WEATHER_CLEAR_PHRASES: &[&str] = &[
    "Yo the skies are clear, this view is clean.",
    "No cap, the lighting is crisp right now.",
    "Okay, blue skies? That's kinda fire.",
];
const WEATHER_RAIN_PHRASES: &[&str] = &[
    "Yo it's raining, that vibe is crazy.",
    "Rain just hit, low-key dramatic.",
    "Bro the rain makes this look so cinematic.",
];
const WEATHER_THUNDER_PHRASES: &[&str] = &[
    "Thunderstorm? That's wild, no cap.",
    "Yo this storm is intense, fr.",
    "Bro the thunder vibe is actually insane.",
];

const WEATHER_CLEAR_NORMAL: &[&str] = &[
    "Weather is clear; the sky render is stable.",
    "Clear weather right now, no precipitation effects.",
    "Skies are clear; the weather system is idle.",
];
const WEATHER_RAIN_NORMAL: &[&str] = &[
    "Rain is active; precipitation effects are on.",
    "You can see rain effects; the weather system switched to precipitation.",
    "Rainfall now; weather rendering is adding the wet look.",
];
const WEATHER_THUNDER_NORMAL: &[&str] = &[
    "Thunderstorm active; weather effects are intensified.",
    "Storm conditions; the weather system is in thunder mode.",
    "Thunderstorm rendering is kicking in here.",
];

const TIME_DAWN_PHRASES: &[&str] = &[
    "Sun's coming up, that glow is fire.",
    "Dawn lighting? Kinda perfect.",
    "Yo sunrise hits different.",
];
const TIME_DAY_PHRASES: &[&str] = &[
    "Daylight is popping right now.",
    "Okay, full day vibes, let's go.",
    "This daylight is clean, no cap.",
];
const TIME_SUNSET_PHRASES: &[&str] = &[
    "Sunset glow is insane.",
    "Yo that sunset is straight up fire.",
    "Low-key the sunset lighting is gorgeous.",
];
const TIME_NIGHT_PHRASES: &[&str] = &[
    "Night time hits different, kinda spooky.",
    "Yo the night vibe is wild right now.",
    "No cap, night mode is dramatic.",
];

const TIME_DAWN_NORMAL: &[&str] = &[
    "Dawn lighting here; time-of-day just rolled forward.",
    "The sim clock is hitting dawn; lighting shifts accordingly.",
    "Dawn phase in the sim; lighting and skybox update.",
];
const TIME_DAY_NORMAL: &[&str] = &[
    "Full daylight now; time-of-day is mid-day.",
    "Day phase; lighting is at its brightest range.",
    "Daylight phase from the sim clock is driving this look.",
];
const TIME_SUNSET_NORMAL: &[&str] = &[
    "Sunset phase; the time-of-day system is tinting the scene.",
    "Sunset lighting now; the sky color is shifting with the sim clock.",
    "Evening phase; time-of-day is warming the lighting.",
];
const TIME_NIGHT_NORMAL: &[&str] = &[
    "Night phase; lighting is dropping with the sim clock.",
    "Night mode; the time-of-day system is dimming the scene.",
    "The sim clock is in night; lighting is subdued.",
];

const TELEPORT_PHRASES: &[&str] = &[
    "Yo new spot, this terrain looks wild.",
    "Teleporting over here, let's see what's up.",
    "Okay fresh area, this is kinda sick.",
];
const TELEPORT_NORMAL: &[&str] = &[
    "We moved to a new spot; the camera position jumped.",
    "New area loaded; the view just shifted.",
    "Camera relocated; new terrain is now in view.",
];

const MOB_SHEEP_PHRASES: &[&str] = &[
    "Look at that sheep, the render is clean.",
    "Yo that sheep looks awesome, fr.",
    "Sheep graphics are straight up fire.",
];
const MOB_COW_PHRASES: &[&str] = &[
    "That cow is vibing, no cap.",
    "Yo the cow model is clean.",
    "Bro this cow is just chilling, love it.",
];
const MOB_CHICKEN_PHRASES: &[&str] = &[
    "That chicken is tiny but kinda iconic.",
    "Yo chicken spotted, cute vibes.",
    "No cap, that chicken looks sick.",
];
const MOB_PIG_PHRASES: &[&str] = &[
    "Pig over there, classic vibe.",
    "Yo the pig looks clean.",
    "Bro pig model is actually nice.",
];
const MOB_VILLAGER_PHRASES: &[&str] = &[
    "Villager? That's such a vibe.",
    "Yo villager spotted, let's go.",
    "No cap, the villager model looks great.",
];
const MOB_HOSTILE_PHRASES: &[&str] = &[
    "Uh oh, mobs nearby, this is sketch.",
    "Bro hostile mobs? That's intense.",
    "Yo this got dangerous real quick.",
];
const MOB_CLUSTER_PHRASES: &[&str] = &[
    "Yo there's a bunch of mobs over here, that's wild.",
    "Bro this area is packed with mobs, no cap.",
    "Okay that's a crowd, kinda intense.",
];
const MOB_GENERIC_PHRASES: &[&str] = &[
    "Mobs over there, that's hype.",
    "Yo creatures nearby, looks awesome.",
    "No cap, the mobs look clean.",
];

const MOB_SHEEP_NORMAL: &[&str] = &[
    "Sheep in view; entity rendering is active.",
    "A sheep is nearby; the mob renderer is drawing it.",
    "Sheep spotted; this is the entity render pass.",
];
const MOB_COW_NORMAL: &[&str] = &[
    "Cow in sight; entity rendering is doing its work.",
    "Cow nearby; the mob render path is active.",
    "A cow is visible; entity draw calls are in play.",
];
const MOB_CHICKEN_NORMAL: &[&str] = &[
    "Chicken in view; entity rendering is active.",
    "Chicken nearby; mob rendering is handling it.",
    "A chicken is visible; entity render pass is running.",
];
const MOB_PIG_NORMAL: &[&str] = &[
    "Pig in view; entity rendering is active.",
    "Pig nearby; the mob renderer is drawing it.",
    "A pig is visible; entity draw calls are happening.",
];
const MOB_VILLAGER_NORMAL: &[&str] = &[
    "Villager in view; entity rendering is active.",
    "Villager nearby; the mob renderer is handling it.",
    "A villager is visible; entity render pass is engaged.",
];
const MOB_HOSTILE_NORMAL: &[&str] = &[
    "Hostile mob nearby; entity rendering is active.",
    "Hostile entity in view; the render pass is drawing it.",
    "Dangerous mob visible; the entity pipeline is at work.",
];
const MOB_CLUSTER_NORMAL: &[&str] = &[
    "Multiple mobs in view; entity rendering load is up.",
    "This area has several mobs; the entity pass is busy.",
    "Mob cluster nearby; lots of entities to render.",
];
const MOB_GENERIC_NORMAL: &[&str] = &[
    "Creature nearby; the entity render path is active.",
    "Mobs in view; entity rendering is handling them.",
    "Entities nearby; the renderer is drawing them now.",
];

const ELEVATION_HIGH_PHRASES: &[&str] = &[
    "We're up high, this view is wild.",
    "Yo this height is crazy, look at that.",
    "High ground vibes, no cap.",
];
const ELEVATION_LOW_PHRASES: &[&str] = &[
    "Low ground but the detail is sick.",
    "Yo down here the terrain is clean.",
    "Low-key loving this lowland vibe.",
];

const ELEVATION_HIGH_NORMAL: &[&str] = &[
    "High elevation view; more terrain fits in the frustum.",
    "We’re up high, so more chunks are visible at once.",
    "Higher altitude here; the camera frustum covers more terrain.",
];
const ELEVATION_LOW_NORMAL: &[&str] = &[
    "Low elevation view; terrain fills most of the frame.",
    "Closer to the ground; the chunk mesh dominates the view.",
    "Low altitude; the camera sees mostly nearby blocks.",
];

const FILLER_PHRASES: &[&str] = &[
    "Okay this is actually sick.",
    "Yo the vibe is unreal right now.",
    "No cap, this world looks clean.",
    "Bro this is looking fire.",
];

const FILLER_NORMAL: &[&str] = &[
    "The renderer is drawing the chunk meshes and the skybox.",
    "Simulation ticks advance world time and then render the frame.",
    "Chunks around the camera are being streamed and rendered.",
    "We’re stepping the sim tick and then capturing a frame.",
];
