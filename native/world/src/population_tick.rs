use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hasher};

use crate::population::{PopulationNeighborhood, state};
use crate::population_feature::{
    AIR, COBBLESTONE, FLOWING_LAVA, FLOWING_WATER, STILL_LAVA, STILL_WATER, material_is_solid,
};
use crate::terrain_shape::noise::MtRandom;

const OBSIDIAN: u16 = 49;
const FIRE: u16 = 51;
const WATER_TICK_DELAY: u64 = 5;
const LAVA_TICK_DELAY: u64 = 30;

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub(crate) struct GenerationTick {
    pub(crate) x: i32,
    pub(crate) y: i32,
    pub(crate) z: i32,
    pub(crate) block_id: u16,
    pub(crate) due: u64,
    pub(crate) priority: i32,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct GenerationTickQueue {
    current_tick: u64,
    pending: Vec<GenerationTick>,
    random: MtRandom,
    instaticking: bool,
}

impl GenerationTickQueue {
    pub(crate) fn new_target_seeded() -> Self {
        // Win10 0.15.10 default-constructs Random inside BlockTickingQueue and seeds it through
        // std::random_device. RandomState gives this isolated generation queue an OS-seeded value
        // without coupling it to the deterministic population MT stream.
        let state = RandomState::new();
        let mut hasher = state.build_hasher();
        hasher.write_u64(0x434f_4242_4c45_5354);
        let seed = hasher.finish() as u32;
        Self::with_seed(seed)
    }

    pub(crate) fn with_seed(seed: u32) -> Self {
        Self {
            current_tick: 1,
            pending: Vec::new(),
            random: MtRandom::new(seed),
            instaticking: false,
        }
    }

    pub(crate) fn add(&mut self, x: i32, y: i32, z: i32, block_id: u16, delay: u64) {
        self.pending.push(GenerationTick {
            x,
            y,
            z,
            block_id,
            due: self.current_tick.wrapping_add(delay),
            priority: 0,
        });
    }

    #[cfg(test)]
    pub(crate) fn is_empty(&self) -> bool {
        self.pending.is_empty()
    }

    fn take_next(&mut self) -> Option<GenerationTick> {
        if self.pending.is_empty() {
            return None;
        }
        let mut best = 0_usize;
        for index in 1..self.pending.len() {
            let candidate = &self.pending[index];
            let current = &self.pending[best];
            if (candidate.due, candidate.priority) < (current.due, current.priority) {
                best = index;
            }
        }
        Some(self.pending.swap_remove(best))
    }
}

pub(crate) fn tick_dynamic_liquid(
    region: &mut PopulationNeighborhood,
    queue: &mut GenerationTickQueue,
    x: i32,
    y: i32,
    z: i32,
    liquid: u16,
    random: &mut MtRandom,
) {
    if !matches!(liquid, FLOWING_WATER | FLOWING_LAVA) {
        return;
    }
    let Some(current) = region.state(x, y, z) else {
        return;
    };
    if current >> 4 != liquid {
        return;
    }

    if liquid == FLOWING_LAVA {
        try_spread_fire(region, x, y, z, random);
    }

    let material = liquid_material(liquid);
    let mut depth = i32::from((current & 0x0f) as u8);
    let drop_off = if material == LiquidMaterial::Lava {
        2
    } else {
        1
    };
    let below = (x, y.wrapping_sub(1), z);

    if depth > 0 {
        let mut highest = -100_i32;
        let mut max_count = 0_i32;
        for (nx, nz) in [
            (x.wrapping_sub(1), z),
            (x.wrapping_add(1), z),
            (x, z.wrapping_add(1)),
            (x, z.wrapping_sub(1)),
        ] {
            highest = get_highest(region, nx, y, nz, material, highest, &mut max_count);
        }

        let mut new_depth = highest.wrapping_add(drop_off);
        if new_depth >= 8 || highest < 0 {
            new_depth = -1;
        }

        if let Some(above) = liquid_depth(region, x, y.wrapping_add(1), z, material) {
            new_depth = if above >= 8 { above } else { above + 8 };
        }

        if max_count >= 2
            && material == LiquidMaterial::Water
            && (material_is_solid(region.block_id(below.0, below.1, below.2))
                || liquid_depth(region, below.0, below.1, below.2, material) == Some(0))
        {
            new_depth = 0;
        }

        let mut tick_delay = tick_delay(material);
        if material == LiquidMaterial::Lava
            && depth < 8
            && new_depth < 8
            && new_depth > depth
            && random.next_int(4) != 0
        {
            tick_delay = tick_delay.wrapping_mul(4);
        }

        if new_depth != depth {
            depth = new_depth;
            if depth < 0 {
                set_with_liquid_updates(region, queue, x, y, z, AIR, 0);
            } else {
                set_with_liquid_updates(region, queue, x, y, z, liquid, depth as u8);
                queue.add(x, y, z, liquid, tick_delay);
            }
        } else {
            set_static(region, queue, x, y, z, liquid);
        }
    } else {
        set_static(region, queue, x, y, z, liquid);
    }

    if can_spread_to(region, below.0, below.1, below.2, material) {
        if material == LiquidMaterial::Lava
            && liquid_material_at(region, below.0, below.1, below.2) == Some(LiquidMaterial::Water)
        {
            set_with_liquid_updates(region, queue, below.0, below.1, below.2, 1, 0);
            return;
        }

        let below_depth = if depth >= 8 { depth } else { depth + 8 };
        try_spread_to(region, queue, below, liquid, material, below_depth);
    } else if depth >= 0
        && (depth == 0 || water_blocking(region.block_id(below.0, below.1, below.2)))
    {
        let spreads = get_spread(region, x, y, z, material);
        let mut neighbor = depth + drop_off;
        if depth >= 8 {
            neighbor = 1;
        }
        if neighbor < 8 {
            for (spread, (nx, nz)) in spreads.into_iter().zip([
                (x.wrapping_sub(1), z),
                (x.wrapping_add(1), z),
                (x, z.wrapping_sub(1)),
                (x, z.wrapping_add(1)),
            ]) {
                if spread {
                    try_spread_to(region, queue, (nx, y, nz), liquid, material, neighbor);
                }
            }
        }
    }
}

pub(crate) fn drain_generation_ticks(
    region: &mut PopulationNeighborhood,
    queue: &mut GenerationTickQueue,
) {
    queue.current_tick = u64::MAX;
    queue.instaticking = true;
    while let Some(tick) = queue.take_next() {
        if region.block_id(tick.x, tick.y, tick.z) != tick.block_id || tick.block_id == AIR {
            continue;
        }
        let mut random = queue.random.clone();
        tick_dynamic_liquid(
            region,
            queue,
            tick.x,
            tick.y,
            tick.z,
            tick.block_id,
            &mut random,
        );
        queue.random = random;
    }
    queue.instaticking = false;
}

pub(crate) fn on_liquid_placed(
    region: &mut PopulationNeighborhood,
    queue: &mut GenerationTickQueue,
    x: i32,
    y: i32,
    z: i32,
) {
    for (nx, ny, nz) in [
        (x, y, z.wrapping_add(1)),
        (x, y, z.wrapping_sub(1)),
        (x.wrapping_add(1), y, z),
        (x.wrapping_sub(1), y, z),
        (x, y.wrapping_add(1), z),
    ] {
        solidify_lava(region, x, y, z, nx, ny, nz);
        if !matches!(region.block_id(x, y, z), FLOWING_LAVA | STILL_LAVA) {
            break;
        }
    }

    let id = region.block_id(x, y, z);
    if matches!(id, FLOWING_WATER | FLOWING_LAVA) {
        queue.add(x, y, z, id, tick_delay(liquid_material(id)));
    }

    update_liquid_neighbors(region, queue, x, y, z);
}

fn set_static(
    region: &mut PopulationNeighborhood,
    queue: &mut GenerationTickQueue,
    x: i32,
    y: i32,
    z: i32,
    dynamic_id: u16,
) {
    let data = region
        .state(x, y, z)
        .map(|value| (value & 0x0f) as u8)
        .unwrap_or(0);
    // LiquidBlockDynamic::_setStatic uses UPDATE_CLIENTS, not UPDATE_ALL: this transition must
    // not notify neighbors or enqueue another dynamic conversion.
    let _ = region.set_state(x, y, z, state(dynamic_id + 1, data));
    let _ = queue;
}

fn set_with_liquid_updates(
    region: &mut PopulationNeighborhood,
    queue: &mut GenerationTickQueue,
    x: i32,
    y: i32,
    z: i32,
    id: u16,
    data: u8,
) {
    let _ = region.set_state(x, y, z, state(id, data));
    update_liquid_neighbors(region, queue, x, y, z);
}

fn update_liquid_neighbors(
    region: &mut PopulationNeighborhood,
    queue: &mut GenerationTickQueue,
    x: i32,
    y: i32,
    z: i32,
) {
    for (nx, ny, nz) in [
        (x.wrapping_sub(1), y, z),
        (x.wrapping_add(1), y, z),
        (x, y.wrapping_sub(1), z),
        (x, y.wrapping_add(1), z),
        (x, y, z.wrapping_sub(1)),
        (x, y, z.wrapping_add(1)),
    ] {
        let id = region.block_id(nx, ny, nz);
        if matches!(id, FLOWING_LAVA | STILL_LAVA) {
            solidify_lava(region, nx, ny, nz, x, y, z);
        }
        if matches!(id, STILL_WATER | STILL_LAVA) && region.block_id(nx, ny, nz) == id {
            let data = region
                .state(nx, ny, nz)
                .map(|v| (v & 0x0f) as u8)
                .unwrap_or(0);
            let dynamic = id - 1;
            let _ = region.set_state(nx, ny, nz, state(dynamic, data));
            queue.add(nx, ny, nz, dynamic, tick_delay(liquid_material(dynamic)));
        }
    }
}

fn solidify_lava(
    region: &mut PopulationNeighborhood,
    x: i32,
    y: i32,
    z: i32,
    changed_x: i32,
    changed_y: i32,
    changed_z: i32,
) {
    if changed_y.wrapping_sub(y) < 0 {
        return;
    }
    if !matches!(region.block_id(x, y, z), FLOWING_LAVA | STILL_LAVA) {
        return;
    }
    if !matches!(
        region.block_id(changed_x, changed_y, changed_z),
        FLOWING_WATER | STILL_WATER
    ) {
        return;
    }
    let data = region.state(x, y, z).map(|v| (v & 0x0f) as u8).unwrap_or(0);
    if data == 0 {
        let _ = region.set_state(x, y, z, state(OBSIDIAN, 0));
    } else if data <= 4 {
        let _ = region.set_state(x, y, z, state(COBBLESTONE, 0));
    }
}

fn try_spread_to(
    region: &mut PopulationNeighborhood,
    queue: &mut GenerationTickQueue,
    pos: (i32, i32, i32),
    liquid: u16,
    material: LiquidMaterial,
    depth: i32,
) {
    let (x, y, z) = pos;
    if !can_spread_to(region, x, y, z, material) {
        return;
    }
    let _ = region.set_state(x, y, z, state(liquid, depth as u8));
    if queue.instaticking {
        update_liquid_neighbors(region, queue, x, y, z);
    }
    queue.add(x, y, z, liquid, tick_delay(material));
}

fn get_highest(
    region: &PopulationNeighborhood,
    x: i32,
    y: i32,
    z: i32,
    material: LiquidMaterial,
    current: i32,
    max_count: &mut i32,
) -> i32 {
    let Some(mut depth) = liquid_depth(region, x, y, z, material) else {
        return current;
    };
    if depth == 0 {
        *max_count += 1;
    }
    if depth >= 8 {
        depth = 0;
    }
    if current < 0 || depth < current {
        depth
    } else {
        current
    }
}

fn get_spread(
    region: &PopulationNeighborhood,
    x: i32,
    y: i32,
    z: i32,
    material: LiquidMaterial,
) -> [bool; 4] {
    let mut distances = [1000_i32; 4];
    for (direction, distance) in distances.iter_mut().enumerate() {
        let (nx, nz) = direction_pos(x, z, direction);
        if water_blocking(region.block_id(nx, y, nz)) {
            continue;
        }
        if liquid_material_at(region, nx, y, nz) == Some(material) {
            if region.state(nx, y, nz).map(|v| v & 0x0f).unwrap_or(0) == 0 {
                continue;
            }
        } else if !water_blocking(region.block_id(nx, y.wrapping_sub(1), nz)) {
            *distance = 0;
            continue;
        }
        *distance = slope_distance(region, nx, y, nz, 1, direction, material);
    }
    let lowest = *distances.iter().min().unwrap_or(&1000);
    std::array::from_fn(|index| distances[index] == lowest)
}

fn slope_distance(
    region: &PopulationNeighborhood,
    x: i32,
    y: i32,
    z: i32,
    pass: i32,
    from: usize,
    material: LiquidMaterial,
) -> i32 {
    let mut lowest = 1000;
    for direction in 0..4 {
        if (direction == 0 && from == 1)
            || (direction == 1 && from == 0)
            || (direction == 2 && from == 3)
            || (direction == 3 && from == 2)
        {
            continue;
        }
        let (nx, nz) = direction_pos(x, z, direction);
        if water_blocking(region.block_id(nx, y, nz)) {
            continue;
        }
        if liquid_material_at(region, nx, y, nz) == Some(material) {
            if region.state(nx, y, nz).map(|v| v & 0x0f).unwrap_or(0) == 0 {
                continue;
            }
        } else if !water_blocking(region.block_id(nx, y.wrapping_sub(1), nz)) {
            return pass;
        } else if pass < 4 {
            lowest = lowest.min(slope_distance(
                region,
                nx,
                y,
                nz,
                pass + 1,
                direction,
                material,
            ));
        }
    }
    lowest
}

fn can_spread_to(
    region: &PopulationNeighborhood,
    x: i32,
    y: i32,
    z: i32,
    material: LiquidMaterial,
) -> bool {
    let Some(state) = region.state(x, y, z) else {
        return false;
    };
    let id = state >> 4;
    if liquid_material_id(id) == Some(material)
        || liquid_material_id(id) == Some(LiquidMaterial::Lava)
    {
        return false;
    }
    !water_blocking(id)
}

fn direction_pos(x: i32, z: i32, direction: usize) -> (i32, i32) {
    match direction {
        0 => (x.wrapping_sub(1), z),
        1 => (x.wrapping_add(1), z),
        2 => (x, z.wrapping_sub(1)),
        _ => (x, z.wrapping_add(1)),
    }
}

fn liquid_depth(
    region: &PopulationNeighborhood,
    x: i32,
    y: i32,
    z: i32,
    material: LiquidMaterial,
) -> Option<i32> {
    let state = region.state(x, y, z)?;
    (liquid_material_id(state >> 4) == Some(material)).then_some(i32::from((state & 0x0f) as u8))
}

fn liquid_material_at(
    region: &PopulationNeighborhood,
    x: i32,
    y: i32,
    z: i32,
) -> Option<LiquidMaterial> {
    liquid_material_id(region.block_id(x, y, z))
}

fn liquid_material(id: u16) -> LiquidMaterial {
    liquid_material_id(id).expect("dynamic liquid id")
}

const fn liquid_material_id(id: u16) -> Option<LiquidMaterial> {
    match id {
        FLOWING_WATER | STILL_WATER => Some(LiquidMaterial::Water),
        FLOWING_LAVA | STILL_LAVA => Some(LiquidMaterial::Lava),
        _ => None,
    }
}

const fn tick_delay(material: LiquidMaterial) -> u64 {
    match material {
        LiquidMaterial::Water => WATER_TICK_DELAY,
        LiquidMaterial::Lava => LAVA_TICK_DELAY,
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
enum LiquidMaterial {
    Water,
    Lava,
}

fn try_spread_fire(
    region: &mut PopulationNeighborhood,
    x: i32,
    y: i32,
    z: i32,
    random: &mut MtRandom,
) {
    for _ in 0..10 {
        let nx = x.wrapping_add(random.next_int(3) as i32 - 1);
        let ny = y.wrapping_add(random.next_int(3) as i32 - 1);
        let nz = z.wrapping_add(random.next_int(3) as i32 - 1);
        if !flammable(region.block_id(nx, ny, nz)) {
            continue;
        }
        for (fx, fy, fz) in [
            (nx.wrapping_add(1), ny, nz),
            (nx.wrapping_sub(1), ny, nz),
            (nx, ny.wrapping_add(1), nz),
            (nx, ny.wrapping_sub(1), nz),
            (nx, ny, nz.wrapping_add(1)),
            (nx, ny, nz.wrapping_sub(1)),
        ] {
            if region.block_id(fx, fy, fz) == AIR {
                let _ = region.set_state(fx, fy, fz, state(FIRE, 0));
                return;
            }
        }
    }
}

const fn flammable(id: u16) -> bool {
    matches!(
        id,
        5 | 17 | 18 | 31 | 35 | 47 | 85 | 107 | 161 | 162 | 170 | 171 | 183..=187 | 193..=197
    )
}

const fn water_blocking(id: u16) -> bool {
    if matches!(id, 55 | 66 | 78 | 171) {
        return false;
    }
    if matches!(id, 63 | 64 | 65 | 68 | 71 | 83 | 90 | 119 | 193..=197) {
        return true;
    }
    material_is_solid(id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ChunkCoord, population::PopulationNeighborhood};

    #[test]
    fn source_water_tick_becomes_static_and_spreads_down() {
        let mut region = PopulationNeighborhood::filled(ChunkCoord::new(0, 0), state(1, 0), 1);
        let _ = region.set_state(8, 40, 8, state(FLOWING_WATER, 0));
        let _ = region.set_state(8, 39, 8, state(AIR, 0));
        let mut queue = GenerationTickQueue::with_seed(0);
        let mut random = MtRandom::new(0);

        tick_dynamic_liquid(
            &mut region,
            &mut queue,
            8,
            40,
            8,
            FLOWING_WATER,
            &mut random,
        );

        assert_eq!(region.state(8, 40, 8), Some(state(STILL_WATER, 0)));
        assert_eq!(region.state(8, 39, 8), Some(state(FLOWING_WATER, 8)));
        assert!(!queue.is_empty());
    }

    #[test]
    fn queued_water_ticks_drain_until_stable() {
        let mut region = PopulationNeighborhood::filled(ChunkCoord::new(0, 0), state(1, 0), 1);
        for y in 36..=40 {
            let _ = region.set_state(8, y, 8, state(AIR, 0));
        }
        let _ = region.set_state(8, 40, 8, state(FLOWING_WATER, 0));
        let mut queue = GenerationTickQueue::with_seed(0);
        queue.add(8, 40, 8, FLOWING_WATER, 1);

        drain_generation_ticks(&mut region, &mut queue);

        assert!(queue.is_empty());
        assert_eq!(region.block_id(8, 40, 8), STILL_WATER);
        assert!(matches!(
            region.block_id(8, 39, 8),
            FLOWING_WATER | STILL_WATER
        ));
    }
}
