use crate::chunk::{world_y_to_local_y, Chunk, ChunkPos, Voxel, CHUNK_SIZE_X, CHUNK_SIZE_Z};
use crate::persist::BlockEntityKey;
use crate::redstone::{mechanical_blocks, RedstonePos, RedstoneSimulator};
use crate::{interactive_blocks, ChestState, DispenserState, HopperState, ItemManager, ItemType};
use mdminecraft_core::{DimensionId, ItemStack as CoreItemStack};
use std::collections::{BTreeMap, HashMap};

fn stacks_match_for_merge(a: &CoreItemStack, b: &CoreItemStack) -> bool {
    a.item_type == b.item_type && a.durability == b.durability && a.enchantments == b.enchantments
}

pub fn comparator_signal_from_core_slots(slots: &[Option<CoreItemStack>]) -> u8 {
    if slots.is_empty() {
        return 0;
    }

    let mut total_fill_64ths: u64 = 0;
    let mut has_any = false;

    for stack in slots.iter().flatten() {
        if stack.count == 0 {
            continue;
        }
        has_any = true;
        let max = stack.max_stack_size().max(1) as u64;
        total_fill_64ths = total_fill_64ths.saturating_add((stack.count as u64) * 64 / max);
    }

    if !has_any {
        return 0;
    }

    let denom = (slots.len() as u64) * 64;
    let base = (total_fill_64ths.saturating_mul(14) / denom) as u8;
    base.saturating_add(1).min(15)
}

pub fn update_container_signal(
    chunks: &mut HashMap<ChunkPos, Chunk>,
    redstone_sim: &mut RedstoneSimulator,
    pos: RedstonePos,
    slots: &[Option<CoreItemStack>],
) {
    let Some(local_y) = world_y_to_local_y(pos.y) else {
        return;
    };

    let desired = comparator_signal_from_core_slots(slots);

    let chunk_pos = ChunkPos::new(
        pos.x.div_euclid(CHUNK_SIZE_X as i32),
        pos.z.div_euclid(CHUNK_SIZE_Z as i32),
    );
    let local_x = pos.x.rem_euclid(CHUNK_SIZE_X as i32) as usize;
    let local_z = pos.z.rem_euclid(CHUNK_SIZE_Z as i32) as usize;

    let Some(chunk) = chunks.get_mut(&chunk_pos) else {
        return;
    };

    let mut voxel = chunk.voxel(local_x, local_y, local_z);
    if !matches!(
        voxel.id,
        interactive_blocks::CHEST
            | mechanical_blocks::HOPPER
            | mechanical_blocks::DISPENSER
            | mechanical_blocks::DROPPER
    ) {
        return;
    }

    let current = crate::get_power_level(voxel.state);
    if current == desired {
        return;
    }

    voxel.state = crate::set_power_level(voxel.state, desired);
    chunk.set_voxel(local_x, local_y, local_z, voxel);

    redstone_sim.schedule_update(pos);
    for neighbor in pos.neighbors() {
        redstone_sim.schedule_update(neighbor);
    }
}

pub fn can_insert_one_into_core_slots(
    slots: &[Option<CoreItemStack>],
    stack: &CoreItemStack,
) -> bool {
    debug_assert_eq!(stack.count, 1);

    for existing in slots.iter().flatten() {
        if stacks_match_for_merge(existing, stack) && existing.count < existing.max_stack_size() {
            return true;
        }
    }

    slots.iter().any(|slot| slot.is_none())
}

pub fn insert_one_into_core_slots(
    slots: &mut [Option<CoreItemStack>],
    stack: CoreItemStack,
) -> bool {
    debug_assert_eq!(stack.count, 1);

    for existing in slots.iter_mut().flatten() {
        if stacks_match_for_merge(existing, &stack) && existing.count < existing.max_stack_size() {
            existing.count = existing.count.saturating_add(1);
            return true;
        }
    }

    for slot in slots.iter_mut() {
        if slot.is_none() {
            *slot = Some(stack);
            return true;
        }
    }

    false
}

pub fn take_one_from_core_slots(
    slots: &mut [Option<CoreItemStack>],
) -> Option<(usize, CoreItemStack)> {
    for (idx, slot) in slots.iter_mut().enumerate() {
        let Some(existing) = slot.as_mut() else {
            continue;
        };

        let mut taken = existing.clone();
        taken.count = 1;

        existing.count = existing.count.saturating_sub(1);
        if existing.count == 0 {
            *slot = None;
        }

        return Some((idx, taken));
    }

    None
}

fn restore_one_into_core_slot(
    slots: &mut [Option<CoreItemStack>],
    idx: usize,
    stack: CoreItemStack,
) {
    debug_assert_eq!(stack.count, 1);

    if idx >= slots.len() {
        return;
    }

    match slots[idx].as_mut() {
        Some(existing) if stacks_match_for_merge(existing, &stack) => {
            existing.count = existing.count.saturating_add(1);
        }
        None => {
            slots[idx] = Some(stack);
        }
        Some(_) => {
            // Fallback: try to insert anywhere (should be extremely rare for deterministic transfers).
            let _ = insert_one_into_core_slots(slots, stack);
        }
    }
}

pub fn try_transfer_one_between_core_slots(
    source: &mut [Option<CoreItemStack>],
    dest: &mut [Option<CoreItemStack>],
) -> bool {
    let Some((source_idx, one)) = take_one_from_core_slots(source) else {
        return false;
    };

    if insert_one_into_core_slots(dest, one.clone()) {
        true
    } else {
        restore_one_into_core_slot(source, source_idx, one);
        false
    }
}

pub struct HopperTickContext<'a> {
    pub chunks: &'a mut HashMap<ChunkPos, Chunk>,
    pub redstone_sim: &'a mut RedstoneSimulator,
    pub item_manager: &'a mut ItemManager,
    pub chests: &'a mut BTreeMap<BlockEntityKey, ChestState>,
    pub hoppers: &'a mut BTreeMap<BlockEntityKey, HopperState>,
    pub dispensers: &'a mut BTreeMap<BlockEntityKey, DispenserState>,
    pub droppers: &'a mut BTreeMap<BlockEntityKey, DispenserState>,
}

pub fn tick_hoppers<ConvertDroppedFn>(ctx: HopperTickContext<'_>, convert_dropped: ConvertDroppedFn)
where
    ConvertDroppedFn: Fn(ItemType) -> Option<CoreItemStack>,
{
    const HOPPER_COOLDOWN_TICKS: u8 = 8;
    const HOPPER_PICKUP_RADIUS: f64 = 0.65;

    let HopperTickContext {
        chunks,
        redstone_sim,
        item_manager,
        chests,
        hoppers,
        dispensers,
        droppers,
    } = ctx;

    let voxel_at = |chunks: &HashMap<ChunkPos, Chunk>, pos: RedstonePos| -> Option<Voxel> {
        let local_y = world_y_to_local_y(pos.y)?;

        let chunk_pos = ChunkPos::new(
            pos.x.div_euclid(CHUNK_SIZE_X as i32),
            pos.z.div_euclid(CHUNK_SIZE_Z as i32),
        );
        let chunk = chunks.get(&chunk_pos)?;
        let local_x = pos.x.rem_euclid(CHUNK_SIZE_X as i32) as usize;
        let local_z = pos.z.rem_euclid(CHUNK_SIZE_Z as i32) as usize;
        Some(chunk.voxel(local_x, local_y, local_z))
    };

    let hopper_keys: Vec<_> = hoppers.keys().copied().collect();

    for key in hopper_keys {
        let Some(mut hopper) = hoppers.remove(&key) else {
            continue;
        };

        if key.dimension != DimensionId::Overworld {
            hoppers.insert(key, hopper);
            continue;
        }

        let pos = RedstonePos::new(key.x, key.y, key.z);
        let Some(voxel) = voxel_at(chunks, pos) else {
            hoppers.insert(key, hopper);
            continue;
        };

        if voxel.id != mechanical_blocks::HOPPER {
            continue;
        }

        if hopper.cooldown_ticks > 0 {
            hopper.cooldown_ticks = hopper.cooldown_ticks.saturating_sub(1);
            hoppers.insert(key, hopper);
            continue;
        }

        // Hopper locking: treat the redstone active bit as "powered/disabled".
        if crate::is_active(voxel.state) {
            hoppers.insert(key, hopper);
            continue;
        }

        let output_pos = if crate::hopper_outputs_down(voxel.state) {
            RedstonePos::new(pos.x, pos.y - 1, pos.z)
        } else {
            let facing = crate::hopper_facing(voxel.state);
            let (dx, dz) = facing.offset();
            RedstonePos::new(pos.x + dx, pos.y, pos.z + dz)
        };

        let mut moved_any = false;

        // Push one item into the container in front.
        if let Some(out_voxel) = voxel_at(chunks, output_pos) {
            if out_voxel.id == interactive_blocks::CHEST {
                let chest_key = BlockEntityKey {
                    dimension: DimensionId::Overworld,
                    x: output_pos.x,
                    y: output_pos.y,
                    z: output_pos.z,
                };
                let chest = chests.entry(chest_key).or_default();
                if try_transfer_one_between_core_slots(&mut hopper.slots, &mut chest.slots) {
                    moved_any = true;
                    update_container_signal(chunks, redstone_sim, output_pos, &chest.slots);
                }
            } else if out_voxel.id == mechanical_blocks::HOPPER {
                let target_key = BlockEntityKey {
                    dimension: DimensionId::Overworld,
                    x: output_pos.x,
                    y: output_pos.y,
                    z: output_pos.z,
                };
                let target = hoppers.entry(target_key).or_default();
                if try_transfer_one_between_core_slots(&mut hopper.slots, &mut target.slots) {
                    moved_any = true;
                    target.cooldown_ticks = target.cooldown_ticks.max(HOPPER_COOLDOWN_TICKS);
                    update_container_signal(chunks, redstone_sim, output_pos, &target.slots);
                }
            } else if out_voxel.id == mechanical_blocks::DISPENSER {
                let target_key = BlockEntityKey {
                    dimension: DimensionId::Overworld,
                    x: output_pos.x,
                    y: output_pos.y,
                    z: output_pos.z,
                };
                let target = dispensers.entry(target_key).or_default();
                if try_transfer_one_between_core_slots(&mut hopper.slots, &mut target.slots) {
                    moved_any = true;
                    update_container_signal(chunks, redstone_sim, output_pos, &target.slots);
                }
            } else if out_voxel.id == mechanical_blocks::DROPPER {
                let target_key = BlockEntityKey {
                    dimension: DimensionId::Overworld,
                    x: output_pos.x,
                    y: output_pos.y,
                    z: output_pos.z,
                };
                let target = droppers.entry(target_key).or_default();
                if try_transfer_one_between_core_slots(&mut hopper.slots, &mut target.slots) {
                    moved_any = true;
                    update_container_signal(chunks, redstone_sim, output_pos, &target.slots);
                }
            }
        }

        // Pull one item from the container above.
        let above_pos = RedstonePos::new(pos.x, pos.y + 1, pos.z);
        if !moved_any {
            if let Some(above_voxel) = voxel_at(chunks, above_pos) {
                if above_voxel.id == interactive_blocks::CHEST {
                    let chest_key = BlockEntityKey {
                        dimension: key.dimension,
                        x: above_pos.x,
                        y: above_pos.y,
                        z: above_pos.z,
                    };
                    let chest = chests.entry(chest_key).or_default();
                    if try_transfer_one_between_core_slots(&mut chest.slots, &mut hopper.slots) {
                        moved_any = true;
                        update_container_signal(chunks, redstone_sim, above_pos, &chest.slots);
                    }
                } else if above_voxel.id == mechanical_blocks::HOPPER {
                    let source_key = BlockEntityKey {
                        dimension: key.dimension,
                        x: above_pos.x,
                        y: above_pos.y,
                        z: above_pos.z,
                    };
                    let source = hoppers.entry(source_key).or_default();
                    if try_transfer_one_between_core_slots(&mut source.slots, &mut hopper.slots) {
                        moved_any = true;
                        update_container_signal(chunks, redstone_sim, above_pos, &source.slots);
                    }
                } else if above_voxel.id == mechanical_blocks::DISPENSER {
                    let source_key = BlockEntityKey {
                        dimension: key.dimension,
                        x: above_pos.x,
                        y: above_pos.y,
                        z: above_pos.z,
                    };
                    let source = dispensers.entry(source_key).or_default();
                    if try_transfer_one_between_core_slots(&mut source.slots, &mut hopper.slots) {
                        moved_any = true;
                        update_container_signal(chunks, redstone_sim, above_pos, &source.slots);
                    }
                } else if above_voxel.id == mechanical_blocks::DROPPER {
                    let source_key = BlockEntityKey {
                        dimension: key.dimension,
                        x: above_pos.x,
                        y: above_pos.y,
                        z: above_pos.z,
                    };
                    let source = droppers.entry(source_key).or_default();
                    if try_transfer_one_between_core_slots(&mut source.slots, &mut hopper.slots) {
                        moved_any = true;
                        update_container_signal(chunks, redstone_sim, above_pos, &source.slots);
                    }
                }
            }
        }

        // Pull one dropped item from above (vanilla-ish).
        if !moved_any {
            let pickup_x = pos.x as f64 + 0.5;
            let pickup_y = pos.y as f64 + 1.0;
            let pickup_z = pos.z as f64 + 0.5;

            let taken = item_manager.take_one_near_if(
                key.dimension,
                pickup_x,
                pickup_y,
                pickup_z,
                HOPPER_PICKUP_RADIUS,
                |drop_type| {
                    let Some(stack) = convert_dropped(drop_type) else {
                        return false;
                    };
                    can_insert_one_into_core_slots(&hopper.slots, &stack)
                },
            );

            if let Some((drop_type, _count)) = taken {
                let Some(stack) = convert_dropped(drop_type) else {
                    hoppers.insert(key, hopper);
                    continue;
                };

                if insert_one_into_core_slots(&mut hopper.slots, stack) {
                    moved_any = true;
                }
            }
        }

        if moved_any {
            hopper.cooldown_ticks = HOPPER_COOLDOWN_TICKS;
            update_container_signal(chunks, redstone_sim, pos, &hopper.slots);
        }

        hoppers.insert(key, hopper);
    }
}
