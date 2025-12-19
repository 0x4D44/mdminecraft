use std::fmt;

use mdminecraft_assets::BlockRegistry;
use mdminecraft_core::{item::FoodType, ItemType};
use mdminecraft_world::{MobType, WeatherState};

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
    Setblock {
        x: BlockCoordArg,
        y: BlockCoordArg,
        z: BlockCoordArg,
        block_id: u16,
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

pub trait CommandContext {
    fn player_position(&self) -> (f64, f64, f64);
    fn teleport_player(&mut self, x: f64, y: f64, z: f64) -> anyhow::Result<()>;

    /// Attempt to give items to the player. Returns leftover count if inventory is full.
    fn give_item(&mut self, item: ItemType, count: u32) -> u32;

    fn time_tick(&self) -> u64;
    fn set_time_tick(&mut self, tick: u64);

    fn set_weather(&mut self, state: WeatherState);
    fn set_gamemode(&mut self, mode: Gamemode);

    fn set_block(&mut self, x: i32, y: i32, z: i32, block_id: u16) -> anyhow::Result<()>;
    fn summon_mob(&mut self, mob: MobType, x: f64, y: f64, z: f64) -> anyhow::Result<()>;
}

pub fn execute_command(ctx: &mut impl CommandContext, cmd: GameCommand) -> CommandOutput {
    let mut out = CommandOutput::default();
    match cmd {
        GameCommand::Help => {
            out.lines.extend(help_lines());
        }
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
        GameCommand::Setblock { x, y, z, block_id } => {
            let (bx, by, bz) = ctx.player_position();
            let base_x = bx.floor() as i32;
            let base_y = by.floor() as i32;
            let base_z = bz.floor() as i32;
            let wx = x.resolve(base_x);
            let wy = y.resolve(base_y);
            let wz = z.resolve(base_z);
            match ctx.set_block(wx, wy, wz, block_id) {
                Ok(()) => out
                    .lines
                    .push(format!("Set block at {wx} {wy} {wz} to id={block_id}")),
                Err(err) => out.lines.push(format!("Error: {err:#}")),
            }
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
        "setblock" => parse_setblock_command(&args, blocks),
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

fn parse_setblock_command(
    args: &[&str],
    blocks: &BlockRegistry,
) -> Result<GameCommand, CommandError> {
    if args.len() != 4 {
        return Err(CommandError::new("Usage: /setblock <x> <y> <z> <block>"));
    }
    let x = parse_block_coord(args[0])?;
    let y = parse_block_coord(args[1])?;
    let z = parse_block_coord(args[2])?;
    let block_token = args[3].trim();
    let block_id = if let Ok(id) = block_token.parse::<u16>() {
        id
    } else {
        blocks
            .id_by_name(block_token)
            .ok_or_else(|| CommandError::new("Unknown block name"))?
    };
    Ok(GameCommand::Setblock { x, y, z, block_id })
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

fn help_lines() -> Vec<String> {
    vec![
        "Commands:".to_string(),
        "  /help".to_string(),
        "  /tp <x> <y> <z>            (supports ~offset)".to_string(),
        "  /give <item> [count]        item = block:<name> | item:<id> | tool:<type>:<material>"
            .to_string(),
        "  /time set <tick|day|noon|night|midnight>".to_string(),
        "  /time add <delta>".to_string(),
        "  /weather <clear|rain>".to_string(),
        "  /gamemode <survival|creative>".to_string(),
        "  /setblock <x> <y> <z> <block|id>   (supports ~offset; block coords are ints)"
            .to_string(),
        "  /summon <mob> [x y z]       (supports ~offset)".to_string(),
        "Note: commands are local-only in this build.".to_string(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use mdminecraft_assets::BlockDescriptor;

    #[derive(Default)]
    struct FakeCtx {
        pos: (f64, f64, f64),
        time: u64,
        weather: WeatherState,
        gamemode: Gamemode,
        given: Vec<(ItemType, u32)>,
        blocks: std::collections::HashMap<(i32, i32, i32), u16>,
        mobs: Vec<(MobType, f64, f64, f64)>,
    }

    impl CommandContext for FakeCtx {
        fn player_position(&self) -> (f64, f64, f64) {
            self.pos
        }

        fn teleport_player(&mut self, x: f64, y: f64, z: f64) -> anyhow::Result<()> {
            self.pos = (x, y, z);
            Ok(())
        }

        fn give_item(&mut self, item: ItemType, count: u32) -> u32 {
            self.given.push((item, count));
            0
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

        fn set_block(&mut self, x: i32, y: i32, z: i32, block_id: u16) -> anyhow::Result<()> {
            self.blocks.insert((x, y, z), block_id);
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
        assert_eq!(ctx.blocks.get(&(11, 64, 8)).copied(), Some(2));
        assert_eq!(out.lines.len(), 1);
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
                "Teleported to 11.00 64.00 8.00".to_string(),
                "Summoned Cow at 11.00 64.00 8.00".to_string(),
            ]
        );
    }
}
