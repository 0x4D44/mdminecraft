use std::fmt;

use mdminecraft_assets::BlockRegistry;
use mdminecraft_core::{item::FoodType, ItemType, RegistryKey};
use mdminecraft_world::{
    BrewingStandState, ChestState, DispenserState, EnchantingTableState, FurnaceState, HopperState,
    MobType, StatusEffectType, WeatherState,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandError {
    message: String,
}

impl CommandError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for CommandError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for CommandError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Gamemode {
    #[default]
    Survival,
    Creative,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SetblockMode {
    #[default]
    Replace,
    Keep,
    Destroy,
}

impl SetblockMode {
    fn as_str(self) -> &'static str {
        match self {
            Self::Replace => "replace",
            Self::Keep => "keep",
            Self::Destroy => "destroy",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FillMode {
    #[default]
    Replace,
    Outline,
    Hollow,
    Keep,
    Destroy,
}

impl FillMode {
    fn as_str(self) -> &'static str {
        match self {
            Self::Replace => "replace",
            Self::Outline => "outline",
            Self::Hollow => "hollow",
            Self::Keep => "keep",
            Self::Destroy => "destroy",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CloneMode {
    #[default]
    Replace,
    Masked,
}

impl CloneMode {
    fn as_str(self) -> &'static str {
        match self {
            Self::Replace => "replace",
            Self::Masked => "masked",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CoordArg {
    Absolute(f64),
    Relative(f64),
}

impl CoordArg {
    pub fn resolve(self, base: f64) -> f64 {
        match self {
            Self::Absolute(v) => v,
            Self::Relative(delta) => base + delta,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockCoordArg {
    Absolute(i32),
    Relative(i32),
}

impl BlockCoordArg {
    pub fn resolve(self, base: i32) -> i32 {
        match self {
            Self::Absolute(v) => v,
            Self::Relative(delta) => base.saturating_add(delta),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum GameCommand {
    Help,
    Seed,
    Say {
        message: String,
    },
    Clear {
        item: Option<ItemType>,
        max_count: Option<u32>,
    },
    Kill,
    Tp {
        x: CoordArg,
        y: CoordArg,
        z: CoordArg,
    },
    Give {
        item: ItemType,
        count: u32,
    },
    TimeSet {
        tick: u64,
    },
    TimeAdd {
        delta: u64,
    },
    WeatherSet {
        state: WeatherState,
    },
    Gamemode {
        mode: Gamemode,
    },
    EffectGive {
        effect: StatusEffectType,
        seconds: u32,
        amplifier: u8,
    },
    EffectClear {
        effect: Option<StatusEffectType>,
    },
    Setblock {
        x: BlockCoordArg,
        y: BlockCoordArg,
        z: BlockCoordArg,
        block_id: u16,
        state: u16,
        mode: SetblockMode,
    },
    Fill {
        x1: BlockCoordArg,
        y1: BlockCoordArg,
        z1: BlockCoordArg,
        x2: BlockCoordArg,
        y2: BlockCoordArg,
        z2: BlockCoordArg,
        block_id: u16,
        state: u16,
        mode: FillMode,
        filter: Option<BlockFilter>,
    },
    Clone {
        x1: BlockCoordArg,
        y1: BlockCoordArg,
        z1: BlockCoordArg,
        x2: BlockCoordArg,
        y2: BlockCoordArg,
        z2: BlockCoordArg,
        x: BlockCoordArg,
        y: BlockCoordArg,
        z: BlockCoordArg,
        mode: CloneMode,
    },
    Summon {
        mob: MobType,
        x: CoordArg,
        y: CoordArg,
        z: CoordArg,
    },
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct CommandOutput {
    pub lines: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlockFilter {
    pub block_id: u16,
    pub state: Option<u16>,
}

#[derive(Debug, Clone)]
pub enum BlockEntityData {
    Chest(Box<ChestState>),
    Hopper(Box<HopperState>),
    Dispenser(Box<DispenserState>),
    Dropper(Box<DispenserState>),
    Furnace(Box<FurnaceState>),
    BrewingStand(Box<BrewingStandState>),
    EnchantingTable(Box<EnchantingTableState>),
}

pub trait CommandContext {
    fn player_position(&self) -> (f64, f64, f64);
    fn teleport_player(&mut self, x: f64, y: f64, z: f64) -> anyhow::Result<()>;
    fn world_seed(&self) -> u64;

    /// Attempt to give items to the player. Returns leftover count if inventory is full.
    fn give_item(&mut self, item: ItemType, count: u32) -> u32;

    /// Clear items from player storage.
    ///
    /// Returns the number of items cleared. If `max_count == Some(0)`, performs a dry-run
    /// and returns the number of matching items without removing anything.
    fn clear_items(&mut self, item: Option<ItemType>, max_count: Option<u32>) -> u32;

    fn kill_player(&mut self) -> anyhow::Result<()>;

    fn destroy_block(&mut self, x: i32, y: i32, z: i32) -> anyhow::Result<()>;

    fn destroy_blocks(&mut self, positions: &[(i32, i32, i32)]) -> anyhow::Result<()> {
        for &(x, y, z) in positions {
            self.destroy_block(x, y, z)?;
        }
        Ok(())
    }

    fn time_tick(&self) -> u64;
    fn set_time_tick(&mut self, tick: u64);

    fn set_weather(&mut self, state: WeatherState);
    fn set_gamemode(&mut self, mode: Gamemode);

    fn apply_status_effect(
        &mut self,
        effect: StatusEffectType,
        seconds: u32,
        amplifier: u8,
    ) -> anyhow::Result<()>;
    fn clear_status_effects(&mut self);
    fn remove_status_effect(&mut self, effect: StatusEffectType);

    fn get_block(&mut self, x: i32, y: i32, z: i32) -> anyhow::Result<(u16, u16)>;
    fn get_block_entity(
        &mut self,
        x: i32,
        y: i32,
        z: i32,
        block_id: u16,
    ) -> anyhow::Result<Option<BlockEntityData>>;
    fn set_block_entity(
        &mut self,
        x: i32,
        y: i32,
        z: i32,
        data: BlockEntityData,
    ) -> anyhow::Result<()>;

    fn set_block(
        &mut self,
        x: i32,
        y: i32,
        z: i32,
        block_id: u16,
        state: u16,
    ) -> anyhow::Result<()>;

    fn set_blocks(&mut self, blocks: &[(i32, i32, i32, u16, u16)]) -> anyhow::Result<()> {
        for &(x, y, z, block_id, state) in blocks {
            self.set_block(x, y, z, block_id, state)?;
        }
        Ok(())
    }

    fn summon_mob(&mut self, mob: MobType, x: f64, y: f64, z: f64) -> anyhow::Result<()>;
}

pub fn execute_command(ctx: &mut impl CommandContext, cmd: GameCommand) -> CommandOutput {
    let mut out = CommandOutput::default();
    match cmd {
        GameCommand::Help => {
            out.lines.extend(help_lines());
        }
        GameCommand::Seed => {
            let seed = ctx.world_seed();
            out.lines.push(format!("Seed: {seed}"));
        }
        GameCommand::Say { message } => {
            out.lines.push(format!("[Server] {message}"));
        }
        GameCommand::Clear { item, max_count } => {
            let cleared = ctx.clear_items(item, max_count);
            if max_count == Some(0) {
                out.lines.push(format!("Found {cleared} matching items"));
            } else {
                out.lines.push(format!("Cleared {cleared} items"));
            }
        }
        GameCommand::Kill => match ctx.kill_player() {
            Ok(()) => out.lines.push("Killed player".to_string()),
            Err(err) => out.lines.push(format!("Error: {err:#}")),
        },
        GameCommand::Tp { x, y, z } => {
            let (bx, by, bz) = ctx.player_position();
            let x = x.resolve(bx);
            let y = y.resolve(by);
            let z = z.resolve(bz);
            match ctx.teleport_player(x, y, z) {
                Ok(()) => out
                    .lines
                    .push(format!("Teleported to {:.2} {:.2} {:.2}", x, y, z)),
                Err(err) => out.lines.push(format!("Error: {err:#}")),
            }
        }
        GameCommand::Give { item, count } => {
            if count == 0 {
                out.lines.push("Error: give count must be > 0".to_string());
                return out;
            }
            let leftover = ctx.give_item(item, count);
            let given = count.saturating_sub(leftover);
            if given > 0 {
                out.lines.push(format!("Gave {given}× {item:?}"));
            }
            if leftover > 0 {
                out.lines
                    .push(format!("Inventory full; {leftover}× not given"));
            }
        }
        GameCommand::TimeSet { tick } => {
            ctx.set_time_tick(tick);
            out.lines.push(format!("Time set to tick {tick}"));
        }
        GameCommand::TimeAdd { delta } => {
            let new_tick = ctx.time_tick().saturating_add(delta);
            ctx.set_time_tick(new_tick);
            out.lines
                .push(format!("Time advanced by {delta} to {new_tick}"));
        }
        GameCommand::WeatherSet { state } => {
            ctx.set_weather(state);
            out.lines.push(format!("Weather set to {state:?}"));
        }
        GameCommand::Gamemode { mode } => {
            ctx.set_gamemode(mode);
            out.lines.push(format!("Gamemode set to {mode:?}"));
        }
        GameCommand::EffectGive {
            effect,
            seconds,
            amplifier,
        } => match ctx.apply_status_effect(effect, seconds, amplifier) {
            Ok(()) => {
                if effect.is_instant() {
                    out.lines
                        .push(format!("Applied {effect:?} (amplifier={amplifier})"));
                } else {
                    out.lines.push(format!(
                        "Applied {effect:?} for {seconds}s (amplifier={amplifier})"
                    ));
                }
            }
            Err(err) => out.lines.push(format!("Error: {err:#}")),
        },
        GameCommand::EffectClear { effect } => {
            if let Some(effect) = effect {
                ctx.remove_status_effect(effect);
                out.lines.push(format!("Cleared {effect:?}"));
            } else {
                ctx.clear_status_effects();
                out.lines.push("Cleared all effects".to_string());
            }
        }
        GameCommand::Setblock {
            x,
            y,
            z,
            block_id,
            state,
            mode,
        } => {
            let (bx, by, bz) = ctx.player_position();
            let base_x = bx.floor() as i32;
            let base_y = by.floor() as i32;
            let base_z = bz.floor() as i32;
            let wx = x.resolve(base_x);
            let wy = y.resolve(base_y);
            let wz = z.resolve(base_z);

            if mode == SetblockMode::Keep {
                match ctx.get_block(wx, wy, wz) {
                    Ok((existing_id, _)) => {
                        if existing_id != 0 {
                            out.lines.push(format!(
                                "Skipped setblock at {wx} {wy} {wz} ({}; existing id={existing_id})",
                                mode.as_str()
                            ));
                            return out;
                        }
                    }
                    Err(err) => {
                        out.lines.push(format!("Error: {err:#}"));
                        return out;
                    }
                }
            }

            if mode == SetblockMode::Destroy {
                if let Err(err) = ctx.destroy_block(wx, wy, wz) {
                    out.lines.push(format!("Error: {err:#}"));
                    return out;
                }
                if block_id == 0 && state == 0 {
                    out.lines.push(format!("Destroyed block at {wx} {wy} {wz}"));
                    return out;
                }
            }

            match ctx.set_block(wx, wy, wz, block_id, state) {
                Ok(()) => out.lines.push(format!(
                    "Set block at {wx} {wy} {wz} to id={block_id} state={state}"
                )),
                Err(err) => out.lines.push(format!("Error: {err:#}")),
            }
        }
        GameCommand::Fill {
            x1,
            y1,
            z1,
            x2,
            y2,
            z2,
            block_id,
            state,
            mode,
            filter,
        } => {
            const MAX_FILL_BLOCKS: i64 = 32_768;

            let (bx, by, bz) = ctx.player_position();
            let base_x = bx.floor() as i32;
            let base_y = by.floor() as i32;
            let base_z = bz.floor() as i32;

            let wx1 = x1.resolve(base_x);
            let wy1 = y1.resolve(base_y);
            let wz1 = z1.resolve(base_z);
            let wx2 = x2.resolve(base_x);
            let wy2 = y2.resolve(base_y);
            let wz2 = z2.resolve(base_z);

            let min_x = wx1.min(wx2);
            let max_x = wx1.max(wx2);
            let min_y = wy1.min(wy2);
            let max_y = wy1.max(wy2);
            let min_z = wz1.min(wz2);
            let max_z = wz1.max(wz2);

            let dx = (max_x - min_x) as i64 + 1;
            let dy = (max_y - min_y) as i64 + 1;
            let dz = (max_z - min_z) as i64 + 1;
            let volume = dx.saturating_mul(dy).saturating_mul(dz);
            if volume > MAX_FILL_BLOCKS {
                out.lines.push(format!(
                    "Error: fill volume {volume} exceeds limit {MAX_FILL_BLOCKS}"
                ));
                return out;
            }

            let mut blocks_to_set = Vec::with_capacity(volume as usize);
            let mut destroy_positions: Option<Vec<(i32, i32, i32)>> = None;
            match mode {
                FillMode::Replace => {
                    if let Some(filter) = filter {
                        for y in min_y..=max_y {
                            for z in min_z..=max_z {
                                for x in min_x..=max_x {
                                    let (existing_id, existing_state) = match ctx.get_block(x, y, z)
                                    {
                                        Ok(v) => v,
                                        Err(err) => {
                                            out.lines.push(format!("Error: {err:#}"));
                                            return out;
                                        }
                                    };
                                    if existing_id != filter.block_id {
                                        continue;
                                    }
                                    if let Some(state_filter) = filter.state {
                                        if existing_state != state_filter {
                                            continue;
                                        }
                                    }
                                    blocks_to_set.push((x, y, z, block_id, state));
                                }
                            }
                        }
                    } else {
                        for y in min_y..=max_y {
                            for z in min_z..=max_z {
                                for x in min_x..=max_x {
                                    blocks_to_set.push((x, y, z, block_id, state));
                                }
                            }
                        }
                    }
                }
                FillMode::Destroy => {
                    let mut positions = Vec::with_capacity(volume as usize);
                    for y in min_y..=max_y {
                        for z in min_z..=max_z {
                            for x in min_x..=max_x {
                                positions.push((x, y, z));
                                if block_id != 0 || state != 0 {
                                    blocks_to_set.push((x, y, z, block_id, state));
                                }
                            }
                        }
                    }
                    destroy_positions = Some(positions);
                }
                FillMode::Keep => {
                    for y in min_y..=max_y {
                        for z in min_z..=max_z {
                            for x in min_x..=max_x {
                                let (existing_id, _) = match ctx.get_block(x, y, z) {
                                    Ok(v) => v,
                                    Err(err) => {
                                        out.lines.push(format!("Error: {err:#}"));
                                        return out;
                                    }
                                };
                                if existing_id == 0 {
                                    blocks_to_set.push((x, y, z, block_id, state));
                                }
                            }
                        }
                    }
                }
                FillMode::Outline => {
                    for y in min_y..=max_y {
                        for z in min_z..=max_z {
                            for x in min_x..=max_x {
                                let on_boundary = x == min_x
                                    || x == max_x
                                    || y == min_y
                                    || y == max_y
                                    || z == min_z
                                    || z == max_z;
                                if on_boundary {
                                    blocks_to_set.push((x, y, z, block_id, state));
                                }
                            }
                        }
                    }
                }
                FillMode::Hollow => {
                    for y in min_y..=max_y {
                        for z in min_z..=max_z {
                            for x in min_x..=max_x {
                                let on_boundary = x == min_x
                                    || x == max_x
                                    || y == min_y
                                    || y == max_y
                                    || z == min_z
                                    || z == max_z;
                                if on_boundary {
                                    blocks_to_set.push((x, y, z, block_id, state));
                                } else {
                                    blocks_to_set.push((x, y, z, 0, 0));
                                }
                            }
                        }
                    }
                }
            }

            if let Some(positions) = destroy_positions.as_deref() {
                if let Err(err) = ctx.destroy_blocks(positions) {
                    out.lines.push(format!("Error: {err:#}"));
                    return out;
                }
                if block_id == 0 && state == 0 {
                    out.lines.push(format!(
                        "Destroyed {} blocks ({}) from {min_x} {min_y} {min_z} to {max_x} {max_y} {max_z}",
                        positions.len(),
                        mode.as_str()
                    ));
                    return out;
                }
            }

            match ctx.set_blocks(&blocks_to_set) {
                Ok(()) => {}
                Err(err) => {
                    out.lines.push(format!("Error: {err:#}"));
                    return out;
                }
            }

            let placed = blocks_to_set.len();
            out.lines.push(format!(
                "Filled {placed} blocks ({}) from {min_x} {min_y} {min_z} to {max_x} {max_y} {max_z} with id={block_id} state={state}",
                mode.as_str()
            ));
        }
        GameCommand::Clone {
            x1,
            y1,
            z1,
            x2,
            y2,
            z2,
            x,
            y,
            z,
            mode,
        } => {
            const MAX_CLONE_BLOCKS: i64 = 32_768;

            let (bx, by, bz) = ctx.player_position();
            let base_x = bx.floor() as i32;
            let base_y = by.floor() as i32;
            let base_z = bz.floor() as i32;

            let sx1 = x1.resolve(base_x);
            let sy1 = y1.resolve(base_y);
            let sz1 = z1.resolve(base_z);
            let sx2 = x2.resolve(base_x);
            let sy2 = y2.resolve(base_y);
            let sz2 = z2.resolve(base_z);

            let min_x = sx1.min(sx2);
            let max_x = sx1.max(sx2);
            let min_y = sy1.min(sy2);
            let max_y = sy1.max(sy2);
            let min_z = sz1.min(sz2);
            let max_z = sz1.max(sz2);

            let dx = (max_x - min_x) as i64 + 1;
            let dy = (max_y - min_y) as i64 + 1;
            let dz = (max_z - min_z) as i64 + 1;
            let volume = dx.saturating_mul(dy).saturating_mul(dz);
            if volume > MAX_CLONE_BLOCKS {
                out.lines.push(format!(
                    "Error: clone volume {volume} exceeds limit {MAX_CLONE_BLOCKS}"
                ));
                return out;
            }

            let dest_x = x.resolve(base_x);
            let dest_y = y.resolve(base_y);
            let dest_z = z.resolve(base_z);

            let mut blocks_to_set = Vec::with_capacity(volume as usize);
            let mut block_entities_to_set: Vec<(i32, i32, i32, BlockEntityData)> = Vec::new();
            for y in min_y..=max_y {
                for z in min_z..=max_z {
                    for x in min_x..=max_x {
                        let (block_id, state) = match ctx.get_block(x, y, z) {
                            Ok(v) => v,
                            Err(err) => {
                                out.lines.push(format!("Error: {err:#}"));
                                return out;
                            }
                        };
                        if mode == CloneMode::Masked && block_id == 0 {
                            continue;
                        }

                        let tx = dest_x + (x - min_x);
                        let ty = dest_y + (y - min_y);
                        let tz = dest_z + (z - min_z);
                        blocks_to_set.push((tx, ty, tz, block_id, state));

                        if matches!(
                            block_id,
                            mdminecraft_world::interactive_blocks::CHEST
                                | mdminecraft_world::mechanical_blocks::HOPPER
                                | mdminecraft_world::mechanical_blocks::DISPENSER
                                | mdminecraft_world::mechanical_blocks::DROPPER
                                | mdminecraft_world::BLOCK_FURNACE
                                | mdminecraft_world::BLOCK_FURNACE_LIT
                                | mdminecraft_world::BLOCK_BREWING_STAND
                                | mdminecraft_world::BLOCK_ENCHANTING_TABLE
                        ) {
                            match ctx.get_block_entity(x, y, z, block_id) {
                                Ok(Some(data)) => block_entities_to_set.push((tx, ty, tz, data)),
                                Ok(None) => {}
                                Err(err) => {
                                    out.lines.push(format!("Error: {err:#}"));
                                    return out;
                                }
                            }
                        }
                    }
                }
            }

            match ctx.set_blocks(&blocks_to_set) {
                Ok(()) => {}
                Err(err) => {
                    out.lines.push(format!("Error: {err:#}"));
                    return out;
                }
            }

            for (x, y, z, data) in block_entities_to_set {
                if let Err(err) = ctx.set_block_entity(x, y, z, data) {
                    out.lines.push(format!("Error: {err:#}"));
                    return out;
                }
            }

            let placed = blocks_to_set.len();
            out.lines.push(format!(
                "Cloned {placed} blocks ({}) from {min_x} {min_y} {min_z} to {max_x} {max_y} {max_z} into {dest_x} {dest_y} {dest_z}",
                mode.as_str()
            ));
        }
        GameCommand::Summon { mob, x, y, z } => {
            let (bx, by, bz) = ctx.player_position();
            let x = x.resolve(bx);
            let y = y.resolve(by);
            let z = z.resolve(bz);
            match ctx.summon_mob(mob, x, y, z) {
                Ok(()) => out
                    .lines
                    .push(format!("Summoned {mob:?} at {x:.2} {y:.2} {z:.2}")),
                Err(err) => out.lines.push(format!("Error: {err:#}")),
            }
        }
    }
    out
}

pub fn parse_command(input: &str, blocks: &BlockRegistry) -> Result<GameCommand, CommandError> {
    let input = input.trim();
    if input.is_empty() {
        return Ok(GameCommand::Help);
    }

    let input = input.strip_prefix('/').unwrap_or(input).trim();
    if input.is_empty() {
        return Ok(GameCommand::Help);
    }

    let mut parts = input.split_whitespace();
    let cmd = parts
        .next()
        .ok_or_else(|| CommandError::new("Missing command"))?
        .to_ascii_lowercase();
    let args: Vec<&str> = parts.collect();

    match cmd.as_str() {
        "help" | "?" => Ok(GameCommand::Help),
        "seed" => {
            if !args.is_empty() {
                return Err(CommandError::new("Usage: /seed"));
            }
            Ok(GameCommand::Seed)
        }
        "say" => {
            if args.is_empty() {
                return Err(CommandError::new("Usage: /say <message...>"));
            }
            Ok(GameCommand::Say {
                message: args.join(" "),
            })
        }
        "clear" => {
            if args.len() > 2 {
                return Err(CommandError::new("Usage: /clear [item] [maxCount]"));
            }
            let item = args
                .first()
                .copied()
                .map(|token| parse_item(token, blocks))
                .transpose()?;
            let max_count = args
                .get(1)
                .copied()
                .map(|token| {
                    token
                        .parse::<u32>()
                        .map_err(|_| CommandError::new("Invalid clear maxCount (expected u32)"))
                })
                .transpose()?;
            Ok(GameCommand::Clear { item, max_count })
        }
        "kill" => {
            if !args.is_empty() {
                return Err(CommandError::new("Usage: /kill"));
            }
            Ok(GameCommand::Kill)
        }
        "tp" | "teleport" => {
            if args.len() != 3 {
                return Err(CommandError::new("Usage: /tp <x> <y> <z>"));
            }
            Ok(GameCommand::Tp {
                x: parse_coord(args[0])?,
                y: parse_coord(args[1])?,
                z: parse_coord(args[2])?,
            })
        }
        "give" => {
            if !(1..=2).contains(&args.len()) {
                return Err(CommandError::new("Usage: /give <item> [count]"));
            }
            let item = parse_item(args[0], blocks)?;
            let count = if args.len() == 2 {
                parse_positive_u32(args[1]).map_err(|_| CommandError::new("Invalid give count"))?
            } else {
                1
            };
            Ok(GameCommand::Give { item, count })
        }
        "time" => parse_time_command(&args),
        "weather" => parse_weather_command(&args),
        "gamemode" | "gm" => parse_gamemode_command(&args),
        "effect" => parse_effect_command(&args),
        "setblock" => parse_setblock_command(&args, blocks),
        "fill" => parse_fill_command(&args, blocks),
        "clone" => parse_clone_command(&args),
        "summon" => parse_summon_command(&args),
        _ => Err(CommandError::new(format!(
            "Unknown command: {cmd}. Try /help"
        ))),
    }
}

fn parse_positive_u32(s: &str) -> Result<u32, ()> {
    let value = s.parse::<u32>().map_err(|_| ())?;
    if value == 0 {
        return Err(());
    }
    Ok(value)
}

fn parse_coord(s: &str) -> Result<CoordArg, CommandError> {
    let s = s.trim();
    if let Some(rest) = s.strip_prefix('~') {
        if rest.is_empty() {
            return Ok(CoordArg::Relative(0.0));
        }
        let delta = rest
            .parse::<f64>()
            .map_err(|_| CommandError::new(format!("Invalid relative coordinate: {s}")))?;
        return Ok(CoordArg::Relative(delta));
    }
    let value = s
        .parse::<f64>()
        .map_err(|_| CommandError::new(format!("Invalid coordinate: {s}")))?;
    Ok(CoordArg::Absolute(value))
}

fn parse_block_coord(s: &str) -> Result<BlockCoordArg, CommandError> {
    let s = s.trim();
    if let Some(rest) = s.strip_prefix('~') {
        if rest.is_empty() {
            return Ok(BlockCoordArg::Relative(0));
        }
        let delta = rest
            .parse::<i32>()
            .map_err(|_| CommandError::new(format!("Invalid relative block coordinate: {s}")))?;
        return Ok(BlockCoordArg::Relative(delta));
    }
    let value = s
        .parse::<i32>()
        .map_err(|_| CommandError::new(format!("Invalid block coordinate: {s}")))?;
    Ok(BlockCoordArg::Absolute(value))
}

fn parse_item(token: &str, blocks: &BlockRegistry) -> Result<ItemType, CommandError> {
    let token = token.trim();
    if token.is_empty() {
        return Err(CommandError::new("Missing item"));
    }

    // Convenience: treat a bare token as a block key if possible.
    if !token.contains(':') {
        if let Some(block_id) = blocks.id_by_name(token) {
            return Ok(ItemType::Block(block_id));
        }
    }

    if let Some(item) = mdminecraft_assets::parse_item_type_with_blocks(token, Some(blocks)) {
        return Ok(item);
    }

    // Extra item syntaxes for commands (not used by recipes.json).
    if let Some(rest) = token.strip_prefix("food:") {
        return parse_food(rest).ok_or_else(|| CommandError::new("Unknown food type"));
    }
    if let Some(rest) = token.strip_prefix("potion:") {
        let id = rest
            .parse::<u16>()
            .map_err(|_| CommandError::new("Invalid potion id"))?;
        return Ok(ItemType::Potion(id));
    }
    if let Some(rest) = token.strip_prefix("splash_potion:") {
        let id = rest
            .parse::<u16>()
            .map_err(|_| CommandError::new("Invalid splash potion id"))?;
        return Ok(ItemType::SplashPotion(id));
    }

    Err(CommandError::new(format!(
        "Unknown item: {token}. Try 'block:<name>', 'item:<id>', or 'tool:<type>:<material>'"
    )))
}

fn parse_food(token: &str) -> Option<ItemType> {
    let food = match token.trim().to_ascii_lowercase().as_str() {
        "apple" => FoodType::Apple,
        "bread" => FoodType::Bread,
        "raw_meat" | "rawmeat" => FoodType::RawMeat,
        "cooked_meat" | "cookedmeat" => FoodType::CookedMeat,
        "carrot" => FoodType::Carrot,
        "potato" => FoodType::Potato,
        "baked_potato" | "bakedpotato" => FoodType::BakedPotato,
        "golden_carrot" | "goldencarrot" => FoodType::GoldenCarrot,
        _ => return None,
    };
    Some(ItemType::Food(food))
}

fn parse_time_command(args: &[&str]) -> Result<GameCommand, CommandError> {
    if args.len() != 2 {
        return Err(CommandError::new("Usage: /time <set|add> <value>"));
    }
    let sub = args[0].to_ascii_lowercase();
    let value = args[1].to_ascii_lowercase();
    match sub.as_str() {
        "set" => {
            let tick = match value.as_str() {
                "day" => 1000,
                "noon" => 6000,
                "night" => 13000,
                "midnight" => 18000,
                _ => value
                    .parse::<u64>()
                    .map_err(|_| CommandError::new("Invalid time value"))?,
            };
            Ok(GameCommand::TimeSet { tick })
        }
        "add" => {
            let delta = value
                .parse::<u64>()
                .map_err(|_| CommandError::new("Invalid time delta"))?;
            Ok(GameCommand::TimeAdd { delta })
        }
        _ => Err(CommandError::new("Usage: /time <set|add> <value>")),
    }
}

fn parse_weather_command(args: &[&str]) -> Result<GameCommand, CommandError> {
    if args.len() != 1 {
        return Err(CommandError::new("Usage: /weather <clear|rain>"));
    }
    let value = args[0].to_ascii_lowercase();
    let state = match value.as_str() {
        "clear" | "sun" => WeatherState::Clear,
        "rain" | "precipitation" | "precip" => WeatherState::Precipitation,
        _ => return Err(CommandError::new("Usage: /weather <clear|rain>")),
    };
    Ok(GameCommand::WeatherSet { state })
}

fn parse_gamemode_command(args: &[&str]) -> Result<GameCommand, CommandError> {
    if args.len() != 1 {
        return Err(CommandError::new("Usage: /gamemode <survival|creative>"));
    }
    let value = args[0].to_ascii_lowercase();
    let mode = match value.as_str() {
        "0" | "survival" | "s" => Gamemode::Survival,
        "1" | "creative" | "c" => Gamemode::Creative,
        _ => return Err(CommandError::new("Usage: /gamemode <survival|creative>")),
    };
    Ok(GameCommand::Gamemode { mode })
}

fn parse_effect_command(args: &[&str]) -> Result<GameCommand, CommandError> {
    if args.is_empty() {
        return Err(CommandError::new(
            "Usage: /effect <give|clear> <effect> [seconds] [amplifier]",
        ));
    }

    let sub = args[0].to_ascii_lowercase();
    match sub.as_str() {
        "give" => {
            if !(2..=4).contains(&args.len()) {
                return Err(CommandError::new(
                    "Usage: /effect give <effect> [seconds] [amplifier]",
                ));
            }

            let effect = parse_status_effect_type(args[1])?;
            let seconds = if args.len() >= 3 {
                args[2]
                    .parse::<u32>()
                    .map_err(|_| CommandError::new("Invalid effect duration (expected seconds)"))?
            } else {
                30
            };
            let amplifier = if args.len() >= 4 {
                args[3]
                    .parse::<u8>()
                    .map_err(|_| CommandError::new("Invalid effect amplifier (expected u8)"))?
            } else {
                0
            };

            Ok(GameCommand::EffectGive {
                effect,
                seconds,
                amplifier,
            })
        }
        "clear" => match args.len() {
            1 => Ok(GameCommand::EffectClear { effect: None }),
            2 => Ok(GameCommand::EffectClear {
                effect: Some(parse_status_effect_type(args[1])?),
            }),
            _ => Err(CommandError::new("Usage: /effect clear [effect]")),
        },
        _ => Err(CommandError::new(
            "Usage: /effect <give|clear> <effect> [seconds] [amplifier]",
        )),
    }
}

fn parse_setblock_command(
    args: &[&str],
    blocks: &BlockRegistry,
) -> Result<GameCommand, CommandError> {
    if !(4..=6).contains(&args.len()) {
        return Err(CommandError::new(
            "Usage: /setblock <x> <y> <z> <block>[props] [state] [replace|keep|destroy]",
        ));
    }
    let x = parse_block_coord(args[0])?;
    let y = parse_block_coord(args[1])?;
    let z = parse_block_coord(args[2])?;
    let block_token = args[3].trim();
    let mut explicit_state = None;
    let mut mode = SetblockMode::Replace;
    match args.len() {
        4 => {}
        5 => {
            if args[4].parse::<u16>().is_ok() {
                explicit_state = Some(args[4]);
            } else {
                mode = parse_setblock_mode(args[4])?;
            }
        }
        6 => {
            explicit_state = Some(args[4]);
            mode = parse_setblock_mode(args[5])?;
        }
        _ => unreachable!("validated above"),
    }
    let (block_id, state) = parse_block_and_state(block_token, explicit_state, blocks)?;

    Ok(GameCommand::Setblock {
        x,
        y,
        z,
        block_id,
        state,
        mode,
    })
}

fn parse_fill_command(args: &[&str], blocks: &BlockRegistry) -> Result<GameCommand, CommandError> {
    if !(7..=10).contains(&args.len()) {
        return Err(CommandError::new(
            "Usage: /fill <x1> <y1> <z1> <x2> <y2> <z2> <block>[props] [state] [replace|outline|hollow|keep|destroy] [filter]",
        ));
    }

    let x1 = parse_block_coord(args[0])?;
    let y1 = parse_block_coord(args[1])?;
    let z1 = parse_block_coord(args[2])?;
    let x2 = parse_block_coord(args[3])?;
    let y2 = parse_block_coord(args[4])?;
    let z2 = parse_block_coord(args[5])?;
    let block_token = args[6].trim();

    let mut explicit_state = None;
    let mut mode = FillMode::Replace;
    let mut filter = None;
    match args.len() {
        7 => {}
        8 => {
            if args[7].parse::<u16>().is_ok() {
                explicit_state = Some(args[7]);
            } else {
                mode = parse_fill_mode(args[7])?;
            }
        }
        9 => {
            if args[7].parse::<u16>().is_ok() {
                explicit_state = Some(args[7]);
                mode = parse_fill_mode(args[8])?;
            } else {
                mode = parse_fill_mode(args[7])?;
                filter = Some(parse_fill_filter(mode, args[8], blocks)?);
            }
        }
        10 => {
            explicit_state = Some(args[7]);
            mode = parse_fill_mode(args[8])?;
            filter = Some(parse_fill_filter(mode, args[9], blocks)?);
        }
        _ => unreachable!("validated above"),
    }
    let (block_id, state) = parse_block_and_state(block_token, explicit_state, blocks)?;

    Ok(GameCommand::Fill {
        x1,
        y1,
        z1,
        x2,
        y2,
        z2,
        block_id,
        state,
        mode,
        filter,
    })
}

fn parse_clone_command(args: &[&str]) -> Result<GameCommand, CommandError> {
    if args.len() != 9 && args.len() != 10 {
        return Err(CommandError::new(
            "Usage: /clone <x1> <y1> <z1> <x2> <y2> <z2> <x> <y> <z> [replace|masked]",
        ));
    }

    let x1 = parse_block_coord(args[0])?;
    let y1 = parse_block_coord(args[1])?;
    let z1 = parse_block_coord(args[2])?;
    let x2 = parse_block_coord(args[3])?;
    let y2 = parse_block_coord(args[4])?;
    let z2 = parse_block_coord(args[5])?;
    let x = parse_block_coord(args[6])?;
    let y = parse_block_coord(args[7])?;
    let z = parse_block_coord(args[8])?;
    let mode = args
        .get(9)
        .copied()
        .map(parse_clone_mode)
        .transpose()?
        .unwrap_or(CloneMode::Replace);

    Ok(GameCommand::Clone {
        x1,
        y1,
        z1,
        x2,
        y2,
        z2,
        x,
        y,
        z,
        mode,
    })
}

fn parse_block_and_state(
    token: &str,
    explicit_state: Option<&str>,
    blocks: &BlockRegistry,
) -> Result<(u16, u16), CommandError> {
    let token = token.trim();
    if token.is_empty() {
        return Err(CommandError::new("Missing block"));
    }

    let (base, props) = split_block_properties(token)?;

    if explicit_state.is_some() && props.is_some() {
        return Err(CommandError::new(
            "Cannot specify both [blockstate] properties and a numeric state",
        ));
    }

    let block_id = if let Ok(id) = base.parse::<u16>() {
        id
    } else {
        blocks
            .id_by_name(base)
            .ok_or_else(|| CommandError::new("Unknown block name"))?
    };

    if let Some(token) = explicit_state {
        let state = token
            .parse::<u16>()
            .map_err(|_| CommandError::new("Invalid block state (expected u16)"))?;
        return Ok((block_id, state));
    }

    if let Some(props) = props {
        let state = parse_block_state_properties(block_id, blocks, props)?;
        return Ok((block_id, state));
    }

    Ok((block_id, 0))
}

fn split_block_properties(token: &str) -> Result<(&str, Option<&str>), CommandError> {
    let Some((base, rest)) = token.split_once('[') else {
        return Ok((token, None));
    };

    let base = base.trim();
    if base.is_empty() {
        return Err(CommandError::new("Missing block name before '['"));
    }

    if !rest.ends_with(']') {
        return Err(CommandError::new("Blockstate properties must end with ']'"));
    }

    let inside = &rest[..rest.len() - 1];
    Ok((base, Some(inside)))
}

fn parse_block_state_properties(
    block_id: u16,
    blocks: &BlockRegistry,
    props: &str,
) -> Result<u16, CommandError> {
    let Some(block_key) = blocks.key_by_id(block_id) else {
        return Err(CommandError::new("Unknown block id"));
    };

    let props = props.trim();
    if props.is_empty() {
        return Ok(0);
    }

    let pairs = props
        .split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(|pair| {
            let (k, v) = pair.split_once('=').ok_or_else(|| {
                CommandError::new("Invalid blockstate entry (expected key=value)")
            })?;
            Ok((k.trim().to_ascii_lowercase(), v.trim().to_ascii_lowercase()))
        })
        .collect::<Result<Vec<(String, String)>, CommandError>>()?;

    let mut state: u16 = 0;
    for (k, v) in pairs {
        state = apply_block_state_property(state, block_key, &k, &v)?;
    }
    Ok(state)
}

fn apply_block_state_property(
    state: u16,
    block_key: &RegistryKey,
    key: &str,
    value: &str,
) -> Result<u16, CommandError> {
    let namespace = block_key.namespace();
    let path = block_key.path();

    let mut state = state;

    if namespace != "mdm" {
        return Err(CommandError::new(
            "Blockstate properties are only supported for built-in blocks (mdm:*). Use numeric state instead.",
        ));
    }

    match path {
        "dispenser" => match key {
            "facing" => {
                state = mdminecraft_world::set_dispenser_facing(state, parse_facing(value)?)
            }
            _ => return Err(CommandError::new("dispenser supports: facing")),
        },
        "dropper" => match key {
            "facing" => state = mdminecraft_world::set_dropper_facing(state, parse_facing(value)?),
            _ => return Err(CommandError::new("dropper supports: facing")),
        },
        "hopper" => match key {
            "facing" => {
                if value == "down" {
                    state = mdminecraft_world::set_hopper_outputs_down(state, true);
                } else {
                    state = mdminecraft_world::set_hopper_outputs_down(state, false);
                    state = mdminecraft_world::set_hopper_facing(state, parse_facing(value)?);
                }
            }
            "down" | "outputs_down" => {
                state = mdminecraft_world::set_hopper_outputs_down(state, parse_bool(value)?);
            }
            _ => return Err(CommandError::new("hopper supports: facing, outputs_down")),
        },
        "piston" => match key {
            "facing" => state = mdminecraft_world::set_piston_facing(state, parse_facing(value)?),
            _ => return Err(CommandError::new("piston supports: facing")),
        },
        "redstone_repeater" => match key {
            "facing" => {
                state = mdminecraft_world::set_repeater_facing(state, parse_facing(value)?);
            }
            "delay" => {
                let ticks = value
                    .parse::<u8>()
                    .map_err(|_| CommandError::new("Invalid repeater delay (expected 1-4)"))?;
                state = mdminecraft_world::set_repeater_delay_ticks(state, ticks);
            }
            _ => {
                return Err(CommandError::new(
                    "redstone_repeater supports: facing, delay",
                ))
            }
        },
        "redstone_comparator" => match key {
            "facing" => {
                state = mdminecraft_world::set_comparator_facing(state, parse_facing(value)?);
            }
            "mode" => {
                let subtract = match value {
                    "compare" => false,
                    "subtract" => true,
                    _ => {
                        return Err(CommandError::new(
                            "Invalid comparator mode (expected compare|subtract)",
                        ))
                    }
                };
                state = mdminecraft_world::set_comparator_subtract_mode(state, subtract);
            }
            "subtract" => {
                state = mdminecraft_world::set_comparator_subtract_mode(state, parse_bool(value)?);
            }
            _ => {
                return Err(CommandError::new(
                    "redstone_comparator supports: facing, mode",
                ))
            }
        },
        "redstone_observer" => match key {
            "facing" => state = mdminecraft_world::set_observer_facing(state, parse_facing(value)?),
            _ => return Err(CommandError::new("redstone_observer supports: facing")),
        },
        "redstone_wire" => match key {
            "power" => {
                let power = value
                    .parse::<u8>()
                    .map_err(|_| CommandError::new("Invalid power (expected 0-15)"))?;
                if power > mdminecraft_world::MAX_POWER {
                    return Err(CommandError::new("Invalid power (expected 0-15)"));
                }
                state = mdminecraft_world::set_power_level(state, power);
            }
            _ => return Err(CommandError::new("redstone_wire supports: power")),
        },
        "trapdoor" => match key {
            "open" => state = mdminecraft_world::set_trapdoor_open(state, parse_bool(value)?),
            "half" => {
                let top = match value {
                    "top" => true,
                    "bottom" => false,
                    _ => return Err(CommandError::new("Invalid trapdoor half (top|bottom)")),
                };
                state = mdminecraft_world::set_trapdoor_top(state, top);
            }
            _ => return Err(CommandError::new("trapdoor supports: open, half")),
        },
        "oak_fence_gate" => match key {
            "open" => state = mdminecraft_world::set_fence_gate_open(state, parse_bool(value)?),
            _ => return Err(CommandError::new("oak_fence_gate supports: open")),
        },
        p if p.ends_with("_door_lower") || p.ends_with("_door_upper") => match key {
            "open" => state = mdminecraft_world::set_door_open(state, parse_bool(value)?),
            _ => return Err(CommandError::new("door blocks support: open")),
        },
        "stone_slab" | "oak_slab" | "stone_brick_slab" => match key {
            "half" => {
                let pos = match value {
                    "top" => mdminecraft_world::SlabPosition::Top,
                    "bottom" => mdminecraft_world::SlabPosition::Bottom,
                    _ => return Err(CommandError::new("Invalid slab half (top|bottom)")),
                };
                state = pos.to_state(state);
            }
            _ => return Err(CommandError::new("slab supports: half")),
        },
        "stone_stairs" | "oak_stairs" | "stone_brick_stairs" => match key {
            "facing" => {
                let facing = parse_facing(value)?;
                state = (state & !0x03) | facing.to_state();
            }
            "half" => match value {
                "top" => state |= 0x04,
                "bottom" => state &= !0x04,
                _ => return Err(CommandError::new("Invalid stairs half (top|bottom)")),
            },
            _ => return Err(CommandError::new("stairs supports: facing, half")),
        },
        "water" | "water_flowing" | "lava" | "lava_flowing" => match key {
            "level" => {
                let level = value
                    .parse::<u8>()
                    .map_err(|_| CommandError::new("Invalid fluid level (expected 0-15)"))?;
                if level > 15 {
                    return Err(CommandError::new("Invalid fluid level (expected 0-15)"));
                }
                state = mdminecraft_world::set_fluid_level(state, level);
            }
            "falling" => {
                state = mdminecraft_world::set_falling(state, parse_bool(value)?);
            }
            _ => return Err(CommandError::new("fluids support: level, falling")),
        },
        _ => {
            return Err(CommandError::new(
                "Blockstate properties not supported for this block; use numeric state instead.",
            ))
        }
    }

    Ok(state)
}

fn parse_bool(value: &str) -> Result<bool, CommandError> {
    match value {
        "true" | "1" | "yes" | "on" => Ok(true),
        "false" | "0" | "no" | "off" => Ok(false),
        _ => Err(CommandError::new("Invalid boolean (expected true/false)")),
    }
}

fn parse_facing(value: &str) -> Result<mdminecraft_world::Facing, CommandError> {
    match value {
        "north" => Ok(mdminecraft_world::Facing::North),
        "south" => Ok(mdminecraft_world::Facing::South),
        "east" => Ok(mdminecraft_world::Facing::East),
        "west" => Ok(mdminecraft_world::Facing::West),
        _ => Err(CommandError::new(
            "Invalid facing (expected north|south|east|west)",
        )),
    }
}

fn parse_summon_command(args: &[&str]) -> Result<GameCommand, CommandError> {
    if args.is_empty() {
        return Err(CommandError::new("Usage: /summon <mob> [x y z]"));
    }
    let mob = parse_mob(args[0])?;
    let (x, y, z) = match args.len() {
        1 => (
            CoordArg::Relative(0.0),
            CoordArg::Relative(0.0),
            CoordArg::Relative(0.0),
        ),
        4 => (
            parse_coord(args[1])?,
            parse_coord(args[2])?,
            parse_coord(args[3])?,
        ),
        _ => return Err(CommandError::new("Usage: /summon <mob> [x y z]")),
    };
    Ok(GameCommand::Summon { mob, x, y, z })
}

fn parse_mob(token: &str) -> Result<MobType, CommandError> {
    let mob = match token.trim().to_ascii_lowercase().as_str() {
        "pig" => MobType::Pig,
        "cow" => MobType::Cow,
        "sheep" => MobType::Sheep,
        "chicken" => MobType::Chicken,
        "villager" => MobType::Villager,
        "zombie" => MobType::Zombie,
        "skeleton" => MobType::Skeleton,
        "spider" => MobType::Spider,
        "creeper" => MobType::Creeper,
        _ => return Err(CommandError::new("Unknown mob type")),
    };
    Ok(mob)
}

fn parse_fill_mode(token: &str) -> Result<FillMode, CommandError> {
    match token.trim().to_ascii_lowercase().as_str() {
        "replace" => Ok(FillMode::Replace),
        "outline" => Ok(FillMode::Outline),
        "hollow" => Ok(FillMode::Hollow),
        "keep" => Ok(FillMode::Keep),
        "destroy" => Ok(FillMode::Destroy),
        _ => Err(CommandError::new(
            "Invalid fill mode (expected replace|outline|hollow|keep|destroy)",
        )),
    }
}

fn parse_fill_filter(
    mode: FillMode,
    token: &str,
    blocks: &BlockRegistry,
) -> Result<BlockFilter, CommandError> {
    if mode != FillMode::Replace {
        return Err(CommandError::new(
            "Fill filter is only supported with mode 'replace'",
        ));
    }

    let token = token.trim();
    if token.is_empty() {
        return Err(CommandError::new("Missing fill filter block"));
    }

    let (base, props) = split_block_properties(token)?;
    let block_id = if let Ok(id) = base.parse::<u16>() {
        id
    } else {
        blocks
            .id_by_name(base)
            .ok_or_else(|| CommandError::new("Unknown block name"))?
    };

    let state = if let Some(props) = props {
        Some(parse_block_state_properties(block_id, blocks, props)?)
    } else {
        None
    };

    Ok(BlockFilter { block_id, state })
}

fn parse_setblock_mode(token: &str) -> Result<SetblockMode, CommandError> {
    match token.trim().to_ascii_lowercase().as_str() {
        "replace" => Ok(SetblockMode::Replace),
        "keep" => Ok(SetblockMode::Keep),
        "destroy" => Ok(SetblockMode::Destroy),
        _ => Err(CommandError::new(
            "Invalid setblock mode (expected replace|keep|destroy)",
        )),
    }
}

fn parse_clone_mode(token: &str) -> Result<CloneMode, CommandError> {
    match token.trim().to_ascii_lowercase().as_str() {
        "replace" => Ok(CloneMode::Replace),
        "masked" => Ok(CloneMode::Masked),
        _ => Err(CommandError::new(
            "Invalid clone mode (expected replace|masked)",
        )),
    }
}

fn parse_status_effect_type(token: &str) -> Result<StatusEffectType, CommandError> {
    let token = token.trim().to_ascii_lowercase();
    let token = token.strip_prefix("minecraft:").unwrap_or(&token);

    let effect = match token {
        "speed" | "swiftness" => StatusEffectType::Speed,
        "haste" => StatusEffectType::Haste,
        "strength" => StatusEffectType::Strength,
        "jump_boost" | "jumpboost" => StatusEffectType::JumpBoost,
        "regeneration" => StatusEffectType::Regeneration,
        "resistance" => StatusEffectType::Resistance,
        "fire_resistance" | "fireresistance" => StatusEffectType::FireResistance,
        "water_breathing" | "waterbreathing" => StatusEffectType::WaterBreathing,
        "invisibility" => StatusEffectType::Invisibility,
        "night_vision" | "nightvision" => StatusEffectType::NightVision,
        "slow_falling" | "slowfalling" => StatusEffectType::SlowFalling,
        "slowness" => StatusEffectType::Slowness,
        "mining_fatigue" | "miningfatigue" => StatusEffectType::MiningFatigue,
        "weakness" => StatusEffectType::Weakness,
        "poison" => StatusEffectType::Poison,
        "instant_health" | "instanthealth" | "health" => StatusEffectType::InstantHealth,
        "instant_damage" | "instantdamage" | "harm" => StatusEffectType::InstantDamage,
        _ => return Err(CommandError::new("Unknown effect type")),
    };

    Ok(effect)
}

fn help_lines() -> Vec<String> {
    vec![
        "Commands:".to_string(),
        "  /help".to_string(),
        "  /seed".to_string(),
        "  /say <message...>".to_string(),
        "  /clear [item] [maxCount]".to_string(),
        "  /kill".to_string(),
        "  /tp <x> <y> <z>            (supports ~offset)".to_string(),
        "  /give <item> [count]        item = block:<name> | item:<id> | tool:<type>:<material>"
            .to_string(),
        "  /time set <tick|day|noon|night|midnight>".to_string(),
        "  /time add <delta>".to_string(),
        "  /weather <clear|rain>".to_string(),
        "  /gamemode <survival|creative>".to_string(),
        "  /effect give <effect> [seconds] [amplifier]".to_string(),
        "  /effect clear [effect]".to_string(),
        "  /setblock <x> <y> <z> <block|id>[props] [state] [replace|keep|destroy]   (supports ~offset; coords are ints)"
            .to_string(),
        "  /fill <x1> <y1> <z1> <x2> <y2> <z2> <block|id>[props] [state] [replace|outline|hollow|keep|destroy] [filter]"
            .to_string(),
        "       (limit: 32768 blocks)".to_string(),
        "  /clone <x1> <y1> <z1> <x2> <y2> <z2> <x> <y> <z> [replace|masked]".to_string(),
        "       (limit: 32768 blocks)".to_string(),
        "  /summon <mob> [x y z]       (supports ~offset)".to_string(),
        "Note: commands are local-only in this build.".to_string(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use mdminecraft_assets::BlockDescriptor;
    use mdminecraft_core::ItemStack;

    #[derive(Default)]
    struct FakeCtx {
        pos: (f64, f64, f64),
        seed: u64,
        time: u64,
        weather: WeatherState,
        gamemode: Gamemode,
        given: Vec<(ItemType, u32)>,
        inventory: Vec<ItemStack>,
        killed: bool,
        destroyed: Vec<(i32, i32, i32)>,
        blocks: std::collections::HashMap<(i32, i32, i32), (u16, u16)>,
        block_entities: std::collections::HashMap<(i32, i32, i32), BlockEntityData>,
        mobs: Vec<(MobType, f64, f64, f64)>,
        effects: mdminecraft_world::StatusEffects,
        instant_effects: Vec<(StatusEffectType, u8)>,
    }

    impl CommandContext for FakeCtx {
        fn player_position(&self) -> (f64, f64, f64) {
            self.pos
        }

        fn teleport_player(&mut self, x: f64, y: f64, z: f64) -> anyhow::Result<()> {
            self.pos = (x, y, z);
            Ok(())
        }

        fn world_seed(&self) -> u64 {
            self.seed
        }

        fn give_item(&mut self, item: ItemType, count: u32) -> u32 {
            self.given.push((item, count));
            0
        }

        fn clear_items(&mut self, item: Option<ItemType>, max_count: Option<u32>) -> u32 {
            let matches = |stack: &ItemStack| item.is_none_or(|value| stack.item_type == value);

            if max_count == Some(0) {
                let mut total = 0u32;
                for stack in &self.inventory {
                    if matches(stack) {
                        total = total.saturating_add(stack.count);
                    }
                }
                return total;
            }

            let mut remaining = max_count.unwrap_or(u32::MAX);
            let mut cleared = 0u32;
            for stack in &mut self.inventory {
                if remaining == 0 {
                    break;
                }
                if !matches(stack) || stack.count == 0 {
                    continue;
                }
                let to_remove = stack.count.min(remaining);
                stack.count -= to_remove;
                remaining -= to_remove;
                cleared += to_remove;
            }
            self.inventory.retain(|stack| stack.count > 0);
            cleared
        }

        fn kill_player(&mut self) -> anyhow::Result<()> {
            self.killed = true;
            Ok(())
        }

        fn destroy_block(&mut self, x: i32, y: i32, z: i32) -> anyhow::Result<()> {
            self.destroyed.push((x, y, z));
            self.blocks.remove(&(x, y, z));
            self.block_entities.remove(&(x, y, z));
            Ok(())
        }

        fn time_tick(&self) -> u64 {
            self.time
        }

        fn set_time_tick(&mut self, tick: u64) {
            self.time = tick;
        }

        fn set_weather(&mut self, state: WeatherState) {
            self.weather = state;
        }

        fn set_gamemode(&mut self, mode: Gamemode) {
            self.gamemode = mode;
        }

        fn apply_status_effect(
            &mut self,
            effect: StatusEffectType,
            seconds: u32,
            amplifier: u8,
        ) -> anyhow::Result<()> {
            if effect.is_instant() {
                self.instant_effects.push((effect, amplifier));
                return Ok(());
            }

            let ticks = seconds.saturating_mul(20);
            self.effects.add(mdminecraft_world::StatusEffect::new(
                effect, amplifier, ticks,
            ));
            Ok(())
        }

        fn clear_status_effects(&mut self) {
            self.effects.clear();
        }

        fn remove_status_effect(&mut self, effect: StatusEffectType) {
            self.effects.remove(effect);
        }

        fn get_block(&mut self, x: i32, y: i32, z: i32) -> anyhow::Result<(u16, u16)> {
            Ok(self.blocks.get(&(x, y, z)).copied().unwrap_or((0, 0)))
        }

        fn get_block_entity(
            &mut self,
            x: i32,
            y: i32,
            z: i32,
            _block_id: u16,
        ) -> anyhow::Result<Option<BlockEntityData>> {
            Ok(self.block_entities.get(&(x, y, z)).cloned())
        }

        fn set_block_entity(
            &mut self,
            x: i32,
            y: i32,
            z: i32,
            data: BlockEntityData,
        ) -> anyhow::Result<()> {
            self.block_entities.insert((x, y, z), data);
            Ok(())
        }

        fn set_block(
            &mut self,
            x: i32,
            y: i32,
            z: i32,
            block_id: u16,
            state: u16,
        ) -> anyhow::Result<()> {
            self.block_entities.remove(&(x, y, z));
            self.blocks.insert((x, y, z), (block_id, state));
            Ok(())
        }

        fn summon_mob(&mut self, mob: MobType, x: f64, y: f64, z: f64) -> anyhow::Result<()> {
            self.mobs.push((mob, x, y, z));
            Ok(())
        }
    }

    fn test_blocks() -> BlockRegistry {
        BlockRegistry::new(vec![
            BlockDescriptor::simple("air", false),
            BlockDescriptor::simple("stone", true),
            BlockDescriptor::simple("dirt", true),
            BlockDescriptor::simple("dispenser", true),
        ])
    }

    #[test]
    fn parses_tp_with_relative_coords() {
        let blocks = test_blocks();
        let cmd = parse_command("/tp ~1 64 ~-2", &blocks).unwrap();
        assert_eq!(
            cmd,
            GameCommand::Tp {
                x: CoordArg::Relative(1.0),
                y: CoordArg::Absolute(64.0),
                z: CoordArg::Relative(-2.0),
            }
        );
    }

    #[test]
    fn executes_tp_with_relative_coords() {
        let mut ctx = FakeCtx {
            pos: (10.0, 60.0, 10.0),
            ..Default::default()
        };
        let out = execute_command(
            &mut ctx,
            GameCommand::Tp {
                x: CoordArg::Relative(1.0),
                y: CoordArg::Absolute(64.0),
                z: CoordArg::Relative(-2.0),
            },
        );
        assert_eq!(ctx.pos, (11.0, 64.0, 8.0));
        assert_eq!(out.lines.len(), 1);
    }

    #[test]
    fn parses_give_block_by_name() {
        let blocks = test_blocks();
        let cmd = parse_command("/give stone 3", &blocks).unwrap();
        assert_eq!(
            cmd,
            GameCommand::Give {
                item: ItemType::Block(1),
                count: 3
            }
        );
    }

    #[test]
    fn executes_setblock_relative() {
        let blocks = test_blocks();
        let cmd = parse_command("/setblock ~1 ~0 ~-2 dirt", &blocks).unwrap();
        let mut ctx = FakeCtx {
            pos: (10.9, 64.2, 10.1),
            ..Default::default()
        };
        let out = execute_command(&mut ctx, cmd);
        assert_eq!(ctx.blocks.get(&(11, 64, 8)).copied(), Some((2, 0)));
        assert_eq!(out.lines.len(), 1);
    }

    #[test]
    fn parses_setblock_with_explicit_state() {
        let blocks = test_blocks();
        let cmd = parse_command("/setblock 10 64 10 stone 7", &blocks).unwrap();
        assert_eq!(
            cmd,
            GameCommand::Setblock {
                x: BlockCoordArg::Absolute(10),
                y: BlockCoordArg::Absolute(64),
                z: BlockCoordArg::Absolute(10),
                block_id: 1,
                state: 7,
                mode: SetblockMode::Replace,
            }
        );
    }

    #[test]
    fn parses_setblock_with_blockstate_properties() {
        let blocks = test_blocks();
        let cmd = parse_command("/setblock 10 64 10 dispenser[facing=east]", &blocks).unwrap();
        assert_eq!(
            cmd,
            GameCommand::Setblock {
                x: BlockCoordArg::Absolute(10),
                y: BlockCoordArg::Absolute(64),
                z: BlockCoordArg::Absolute(10),
                block_id: 3,
                state: mdminecraft_world::set_dispenser_facing(0, mdminecraft_world::Facing::East),
                mode: SetblockMode::Replace,
            }
        );
    }

    #[test]
    fn parses_setblock_keep() {
        let blocks = test_blocks();
        let cmd = parse_command("/setblock 10 64 10 stone keep", &blocks).unwrap();
        assert_eq!(
            cmd,
            GameCommand::Setblock {
                x: BlockCoordArg::Absolute(10),
                y: BlockCoordArg::Absolute(64),
                z: BlockCoordArg::Absolute(10),
                block_id: 1,
                state: 0,
                mode: SetblockMode::Keep,
            }
        );
    }

    #[test]
    fn parses_setblock_destroy() {
        let blocks = test_blocks();
        let cmd = parse_command("/setblock 10 64 10 stone destroy", &blocks).unwrap();
        assert_eq!(
            cmd,
            GameCommand::Setblock {
                x: BlockCoordArg::Absolute(10),
                y: BlockCoordArg::Absolute(64),
                z: BlockCoordArg::Absolute(10),
                block_id: 1,
                state: 0,
                mode: SetblockMode::Destroy,
            }
        );
    }

    #[test]
    fn executes_setblock_keep_skips_when_occupied() {
        let blocks = test_blocks();
        let cmd = parse_command("/setblock ~0 ~0 ~0 stone keep", &blocks).unwrap();
        let mut ctx = FakeCtx {
            pos: (10.0, 64.0, 10.0),
            ..Default::default()
        };
        ctx.blocks.insert((10, 64, 10), (2, 0));

        let out = execute_command(&mut ctx, cmd);
        assert_eq!(out.lines.len(), 1);
        assert_eq!(ctx.blocks.get(&(10, 64, 10)).copied(), Some((2, 0)));
    }

    #[test]
    fn executes_setblock_destroy_calls_destroy_hook() {
        let blocks = test_blocks();
        let cmd = parse_command("/setblock ~0 ~0 ~0 stone destroy", &blocks).unwrap();
        let mut ctx = FakeCtx {
            pos: (10.0, 64.0, 10.0),
            ..Default::default()
        };
        ctx.blocks.insert((10, 64, 10), (2, 0));

        let out = execute_command(&mut ctx, cmd);
        assert_eq!(out.lines.len(), 1);
        assert_eq!(ctx.blocks.get(&(10, 64, 10)).copied(), Some((1, 0)));
        assert_eq!(ctx.destroyed, vec![(10, 64, 10)]);
    }

    #[test]
    fn executes_fill_relative() {
        let blocks = test_blocks();
        let cmd = parse_command("/fill ~0 ~0 ~0 ~1 ~0 ~1 stone", &blocks).unwrap();
        let mut ctx = FakeCtx {
            pos: (10.9, 64.2, 10.1),
            ..Default::default()
        };
        let out = execute_command(&mut ctx, cmd);

        assert_eq!(out.lines.len(), 1);
        for x in 10..=11 {
            for z in 10..=11 {
                assert_eq!(ctx.blocks.get(&(x, 64, z)).copied(), Some((1, 0)));
            }
        }
    }

    #[test]
    fn parses_fill_mode_outline() {
        let blocks = test_blocks();
        let cmd = parse_command("/fill 0 0 0 2 2 2 stone outline", &blocks).unwrap();
        assert_eq!(
            cmd,
            GameCommand::Fill {
                x1: BlockCoordArg::Absolute(0),
                y1: BlockCoordArg::Absolute(0),
                z1: BlockCoordArg::Absolute(0),
                x2: BlockCoordArg::Absolute(2),
                y2: BlockCoordArg::Absolute(2),
                z2: BlockCoordArg::Absolute(2),
                block_id: 1,
                state: 0,
                mode: FillMode::Outline,
                filter: None,
            }
        );
    }

    #[test]
    fn parses_fill_mode_destroy() {
        let blocks = test_blocks();
        let cmd = parse_command("/fill 0 0 0 2 2 2 stone destroy", &blocks).unwrap();
        assert_eq!(
            cmd,
            GameCommand::Fill {
                x1: BlockCoordArg::Absolute(0),
                y1: BlockCoordArg::Absolute(0),
                z1: BlockCoordArg::Absolute(0),
                x2: BlockCoordArg::Absolute(2),
                y2: BlockCoordArg::Absolute(2),
                z2: BlockCoordArg::Absolute(2),
                block_id: 1,
                state: 0,
                mode: FillMode::Destroy,
                filter: None,
            }
        );
    }

    #[test]
    fn executes_fill_outline_sets_only_boundary() {
        let blocks = test_blocks();
        let cmd = parse_command("/fill ~0 ~0 ~0 ~2 ~2 ~2 stone outline", &blocks).unwrap();
        let mut ctx = FakeCtx {
            pos: (10.0, 64.0, 10.0),
            ..Default::default()
        };
        let out = execute_command(&mut ctx, cmd);
        assert_eq!(out.lines.len(), 1);

        // 3×3×3 outline sets all blocks except the center.
        assert_eq!(ctx.blocks.len(), 26);
        assert_eq!(ctx.blocks.get(&(11, 65, 11)), None);
        assert_eq!(ctx.blocks.get(&(10, 64, 10)).copied(), Some((1, 0)));
        assert_eq!(ctx.blocks.get(&(12, 66, 12)).copied(), Some((1, 0)));
    }

    #[test]
    fn executes_fill_hollow_clears_interior() {
        let blocks = test_blocks();
        let cmd = parse_command("/fill ~0 ~0 ~0 ~2 ~2 ~2 stone hollow", &blocks).unwrap();
        let mut ctx = FakeCtx {
            pos: (10.0, 64.0, 10.0),
            ..Default::default()
        };
        let out = execute_command(&mut ctx, cmd);
        assert_eq!(out.lines.len(), 1);

        // 3×3×3 hollow sets boundary to stone and interior to air.
        assert_eq!(ctx.blocks.len(), 27);
        assert_eq!(ctx.blocks.get(&(11, 65, 11)).copied(), Some((0, 0)));
        assert_eq!(ctx.blocks.get(&(10, 64, 10)).copied(), Some((1, 0)));
        assert_eq!(ctx.blocks.get(&(12, 66, 12)).copied(), Some((1, 0)));
    }

    #[test]
    fn executes_fill_keep_only_sets_air() {
        let blocks = test_blocks();
        let cmd = parse_command("/fill ~0 ~0 ~0 ~1 ~0 ~1 stone keep", &blocks).unwrap();
        let mut ctx = FakeCtx {
            pos: (10.0, 64.0, 10.0),
            ..Default::default()
        };
        // Pre-fill one block; keep should skip it.
        ctx.blocks.insert((11, 64, 10), (2, 0));

        let out = execute_command(&mut ctx, cmd);
        assert_eq!(out.lines.len(), 1);
        assert_eq!(ctx.blocks.get(&(10, 64, 10)).copied(), Some((1, 0)));
        assert_eq!(ctx.blocks.get(&(11, 64, 10)).copied(), Some((2, 0)));
        assert_eq!(ctx.blocks.get(&(10, 64, 11)).copied(), Some((1, 0)));
        assert_eq!(ctx.blocks.get(&(11, 64, 11)).copied(), Some((1, 0)));
    }

    #[test]
    fn executes_fill_replace_filter_only_replaces_matching_blocks() {
        let blocks = test_blocks();
        let cmd = parse_command("/fill ~0 ~0 ~0 ~1 ~0 ~1 dirt replace stone", &blocks).unwrap();
        let mut ctx = FakeCtx {
            pos: (10.0, 64.0, 10.0),
            ..Default::default()
        };
        // Source area: mix of stone/dirt/air.
        ctx.blocks.insert((10, 64, 10), (1, 0)); // stone -> replaced
        ctx.blocks.insert((11, 64, 10), (2, 0)); // dirt -> preserved
        ctx.blocks.insert((10, 64, 11), (1, 3)); // stone (different state) -> replaced (id match)

        let out = execute_command(&mut ctx, cmd);
        assert_eq!(out.lines.len(), 1);

        assert_eq!(ctx.blocks.get(&(10, 64, 10)).copied(), Some((2, 0)));
        assert_eq!(ctx.blocks.get(&(11, 64, 10)).copied(), Some((2, 0)));
        assert_eq!(ctx.blocks.get(&(10, 64, 11)).copied(), Some((2, 0)));
        assert_eq!(ctx.blocks.get(&(11, 64, 11)).copied(), None);
    }

    #[test]
    fn executes_fill_destroy_calls_destroy_hook_and_sets_blocks() {
        let blocks = test_blocks();
        let cmd = parse_command("/fill ~0 ~0 ~0 ~1 ~0 ~1 stone destroy", &blocks).unwrap();
        let mut ctx = FakeCtx {
            pos: (10.0, 64.0, 10.0),
            ..Default::default()
        };
        ctx.blocks.insert((10, 64, 10), (2, 0));

        let out = execute_command(&mut ctx, cmd);
        assert_eq!(out.lines.len(), 1);
        assert_eq!(
            ctx.destroyed,
            vec![(10, 64, 10), (11, 64, 10), (10, 64, 11), (11, 64, 11)]
        );
        for x in 10..=11 {
            for z in 10..=11 {
                assert_eq!(ctx.blocks.get(&(x, 64, z)).copied(), Some((1, 0)));
            }
        }
    }

    #[test]
    fn executes_fill_destroy_air_only_calls_destroy_hook() {
        let blocks = test_blocks();
        let cmd = parse_command("/fill ~0 ~0 ~0 ~1 ~0 ~1 air destroy", &blocks).unwrap();
        let mut ctx = FakeCtx {
            pos: (10.0, 64.0, 10.0),
            ..Default::default()
        };
        ctx.blocks.insert((11, 64, 10), (2, 0));

        let out = execute_command(&mut ctx, cmd);
        assert_eq!(out.lines.len(), 1);
        assert_eq!(
            out.lines[0],
            "Destroyed 4 blocks (destroy) from 10 64 10 to 11 64 11"
        );
        assert_eq!(
            ctx.destroyed,
            vec![(10, 64, 10), (11, 64, 10), (10, 64, 11), (11, 64, 11)]
        );
        for x in 10..=11 {
            for z in 10..=11 {
                assert_eq!(ctx.blocks.get(&(x, 64, z)), None);
            }
        }
    }

    #[test]
    fn parses_clone_mode_masked() {
        let blocks = test_blocks();
        let cmd = parse_command("/clone 0 0 0 1 0 1 10 0 10 masked", &blocks).unwrap();
        assert_eq!(
            cmd,
            GameCommand::Clone {
                x1: BlockCoordArg::Absolute(0),
                y1: BlockCoordArg::Absolute(0),
                z1: BlockCoordArg::Absolute(0),
                x2: BlockCoordArg::Absolute(1),
                y2: BlockCoordArg::Absolute(0),
                z2: BlockCoordArg::Absolute(1),
                x: BlockCoordArg::Absolute(10),
                y: BlockCoordArg::Absolute(0),
                z: BlockCoordArg::Absolute(10),
                mode: CloneMode::Masked,
            }
        );
    }

    #[test]
    fn executes_clone_replace_copies_air() {
        let blocks = test_blocks();
        let cmd = parse_command("/clone ~0 ~0 ~0 ~1 ~0 ~1 ~10 ~0 ~10", &blocks).unwrap();
        let mut ctx = FakeCtx {
            pos: (10.0, 64.0, 10.0),
            ..Default::default()
        };

        // Source region: one block is air (not present in the map).
        ctx.blocks.insert((10, 64, 10), (1, 0));
        ctx.blocks.insert((11, 64, 10), (2, 7));
        ctx.blocks.insert((10, 64, 11), (1, 0));

        // Destination region pre-filled; replace mode should overwrite even with air.
        for x in 20..=21 {
            for z in 20..=21 {
                ctx.blocks.insert((x, 64, z), (2, 0));
            }
        }

        let out = execute_command(&mut ctx, cmd);
        assert_eq!(out.lines.len(), 1);

        assert_eq!(ctx.blocks.get(&(20, 64, 20)).copied(), Some((1, 0)));
        assert_eq!(ctx.blocks.get(&(21, 64, 20)).copied(), Some((2, 7)));
        assert_eq!(ctx.blocks.get(&(20, 64, 21)).copied(), Some((1, 0)));
        assert_eq!(ctx.blocks.get(&(21, 64, 21)).copied(), Some((0, 0)));
    }

    #[test]
    fn executes_clone_masked_skips_air() {
        let blocks = test_blocks();
        let cmd = parse_command("/clone ~0 ~0 ~0 ~1 ~0 ~1 ~10 ~0 ~10 masked", &blocks).unwrap();
        let mut ctx = FakeCtx {
            pos: (10.0, 64.0, 10.0),
            ..Default::default()
        };

        // Source region: one block is air (not present in the map).
        ctx.blocks.insert((10, 64, 10), (1, 0));
        ctx.blocks.insert((11, 64, 10), (2, 7));
        ctx.blocks.insert((10, 64, 11), (1, 0));

        // Destination region pre-filled; masked mode should not overwrite with air.
        for x in 20..=21 {
            for z in 20..=21 {
                ctx.blocks.insert((x, 64, z), (2, 0));
            }
        }

        let out = execute_command(&mut ctx, cmd);
        assert_eq!(out.lines.len(), 1);

        assert_eq!(ctx.blocks.get(&(20, 64, 20)).copied(), Some((1, 0)));
        assert_eq!(ctx.blocks.get(&(21, 64, 20)).copied(), Some((2, 7)));
        assert_eq!(ctx.blocks.get(&(20, 64, 21)).copied(), Some((1, 0)));
        assert_eq!(ctx.blocks.get(&(21, 64, 21)).copied(), Some((2, 0)));
    }

    #[test]
    fn executes_clone_copies_block_entities() {
        let blocks = test_blocks();
        let cmd = parse_command("/clone ~0 ~0 ~0 ~0 ~0 ~0 ~10 ~0 ~0", &blocks).unwrap();
        let mut ctx = FakeCtx {
            pos: (10.0, 64.0, 10.0),
            ..Default::default()
        };

        let mut chest = ChestState::new();
        chest.slots[0] = Some(mdminecraft_core::ItemStack::new(ItemType::Block(1), 5));

        ctx.blocks.insert(
            (10, 64, 10),
            (mdminecraft_world::interactive_blocks::CHEST, 0),
        );
        ctx.block_entities.insert(
            (10, 64, 10),
            BlockEntityData::Chest(Box::new(chest.clone())),
        );

        let out = execute_command(&mut ctx, cmd);
        assert_eq!(out.lines.len(), 1);
        assert_eq!(
            ctx.blocks.get(&(20, 64, 10)).copied(),
            Some((mdminecraft_world::interactive_blocks::CHEST, 0))
        );

        let cloned = ctx.block_entities.get(&(20, 64, 10)).cloned();
        let Some(BlockEntityData::Chest(cloned_chest)) = cloned else {
            panic!("Expected cloned chest state");
        };
        assert_eq!(cloned_chest.slots[0], chest.slots[0]);
    }

    #[test]
    fn executes_summon_defaults_to_player_pos() {
        let mut ctx = FakeCtx {
            pos: (1.0, 2.0, 3.0),
            ..Default::default()
        };
        let out = execute_command(
            &mut ctx,
            GameCommand::Summon {
                mob: MobType::Cow,
                x: CoordArg::Relative(0.0),
                y: CoordArg::Relative(0.0),
                z: CoordArg::Relative(0.0),
            },
        );
        assert_eq!(ctx.mobs.len(), 1);
        assert_eq!(out.lines.len(), 1);
    }

    #[test]
    fn parses_effect_give_with_defaults() {
        let blocks = test_blocks();
        let cmd = parse_command("/effect give speed", &blocks).unwrap();
        assert_eq!(
            cmd,
            GameCommand::EffectGive {
                effect: StatusEffectType::Speed,
                seconds: 30,
                amplifier: 0,
            }
        );
    }

    #[test]
    fn executes_effect_give_adds_non_instant_effect() {
        let mut ctx = FakeCtx::default();
        let out = execute_command(
            &mut ctx,
            GameCommand::EffectGive {
                effect: StatusEffectType::Speed,
                seconds: 10,
                amplifier: 1,
            },
        );
        assert_eq!(out.lines.len(), 1);
        assert!(ctx.effects.has(StatusEffectType::Speed));
        assert_eq!(ctx.effects.amplifier(StatusEffectType::Speed), Some(1));
    }

    #[test]
    fn executes_effect_give_records_instant_effect() {
        let mut ctx = FakeCtx::default();
        let out = execute_command(
            &mut ctx,
            GameCommand::EffectGive {
                effect: StatusEffectType::InstantHealth,
                seconds: 5,
                amplifier: 0,
            },
        );
        assert_eq!(out.lines.len(), 1);
        assert_eq!(
            ctx.instant_effects,
            vec![(StatusEffectType::InstantHealth, 0)]
        );
    }

    #[test]
    fn executes_effect_clear_removes_effect() {
        let mut ctx = FakeCtx::default();
        ctx.effects.add(mdminecraft_world::StatusEffect::new(
            StatusEffectType::Speed,
            0,
            100,
        ));
        let out = execute_command(
            &mut ctx,
            GameCommand::EffectClear {
                effect: Some(StatusEffectType::Speed),
            },
        );
        assert_eq!(out.lines.len(), 1);
        assert!(!ctx.effects.has(StatusEffectType::Speed));
    }

    #[test]
    fn executes_effect_clear_all_removes_all_effects() {
        let mut ctx = FakeCtx::default();
        ctx.effects.add(mdminecraft_world::StatusEffect::new(
            StatusEffectType::Speed,
            0,
            100,
        ));
        ctx.effects.add(mdminecraft_world::StatusEffect::new(
            StatusEffectType::Strength,
            0,
            100,
        ));
        let out = execute_command(&mut ctx, GameCommand::EffectClear { effect: None });
        assert_eq!(out.lines.len(), 1);
        assert!(!ctx.effects.has(StatusEffectType::Speed));
        assert!(!ctx.effects.has(StatusEffectType::Strength));
    }

    #[test]
    fn golden_command_session_outputs_are_stable() {
        let blocks = test_blocks();
        let mut ctx = FakeCtx {
            pos: (10.0, 64.0, 10.0),
            ..Default::default()
        };

        let mut transcript = Vec::new();
        for input in [
            "/time set day",
            "/weather rain",
            "/give stone 2",
            "/effect give speed 10 0",
            "/tp ~1 64 ~-2",
            "/summon cow",
        ] {
            let cmd = parse_command(input, &blocks).unwrap();
            let out = execute_command(&mut ctx, cmd);
            transcript.extend(out.lines);
        }

        assert_eq!(
            transcript,
            vec![
                "Time set to tick 1000".to_string(),
                "Weather set to Precipitation".to_string(),
                "Gave 2× Block(1)".to_string(),
                "Applied Speed for 10s (amplifier=0)".to_string(),
                "Teleported to 11.00 64.00 8.00".to_string(),
                "Summoned Cow at 11.00 64.00 8.00".to_string(),
            ]
        );
    }

    #[test]
    fn parses_seed() {
        let blocks = test_blocks();
        let cmd = parse_command("/seed", &blocks).unwrap();
        assert_eq!(cmd, GameCommand::Seed);
    }

    #[test]
    fn executes_seed() {
        let blocks = test_blocks();
        let cmd = parse_command("/seed", &blocks).unwrap();
        let mut ctx = FakeCtx {
            seed: 123,
            ..Default::default()
        };
        let out = execute_command(&mut ctx, cmd);
        assert_eq!(out.lines, vec!["Seed: 123".to_string()]);
    }

    #[test]
    fn parses_say() {
        let blocks = test_blocks();
        let cmd = parse_command("/say hello world", &blocks).unwrap();
        assert_eq!(
            cmd,
            GameCommand::Say {
                message: "hello world".to_string()
            }
        );
    }

    #[test]
    fn executes_say() {
        let blocks = test_blocks();
        let cmd = parse_command("/say hello", &blocks).unwrap();
        let mut ctx = FakeCtx::default();
        let out = execute_command(&mut ctx, cmd);
        assert_eq!(out.lines, vec!["[Server] hello".to_string()]);
    }

    #[test]
    fn executes_clear_removes_items() {
        let blocks = test_blocks();
        let cmd = parse_command("/clear stone 3", &blocks).unwrap();
        let mut ctx = FakeCtx {
            inventory: vec![
                ItemStack::new(ItemType::Block(1), 5),
                ItemStack::new(ItemType::Block(2), 4),
                ItemStack::new(ItemType::Block(1), 2),
            ],
            ..Default::default()
        };
        let out = execute_command(&mut ctx, cmd);
        assert_eq!(out.lines, vec!["Cleared 3 items".to_string()]);
        assert_eq!(
            ctx.inventory,
            vec![
                ItemStack::new(ItemType::Block(1), 2),
                ItemStack::new(ItemType::Block(2), 4),
                ItemStack::new(ItemType::Block(1), 2),
            ]
        );
    }

    #[test]
    fn executes_clear_query_does_not_remove_items() {
        let blocks = test_blocks();
        let cmd = parse_command("/clear stone 0", &blocks).unwrap();
        let mut ctx = FakeCtx {
            inventory: vec![
                ItemStack::new(ItemType::Block(1), 5),
                ItemStack::new(ItemType::Block(2), 4),
            ],
            ..Default::default()
        };
        let out = execute_command(&mut ctx, cmd);
        assert_eq!(out.lines, vec!["Found 5 matching items".to_string()]);
        assert_eq!(
            ctx.inventory,
            vec![
                ItemStack::new(ItemType::Block(1), 5),
                ItemStack::new(ItemType::Block(2), 4)
            ]
        );
    }

    #[test]
    fn executes_kill_marks_player_dead() {
        let blocks = test_blocks();
        let cmd = parse_command("/kill", &blocks).unwrap();
        let mut ctx = FakeCtx::default();
        let out = execute_command(&mut ctx, cmd);
        assert_eq!(out.lines, vec!["Killed player".to_string()]);
        assert!(ctx.killed);
    }
}
