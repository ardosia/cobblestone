use crate::WORLD_HEIGHT;
use crate::population::{PopulationNeighborhood, state};
use crate::terrain_shape::noise::MtRandom;

pub(crate) const AIR: u16 = 0;
pub(crate) const STONE: u16 = 1;
pub(crate) const GRASS: u16 = 2;
pub(crate) const DIRT: u16 = 3;
pub(crate) const SAPLING: u16 = 6;
pub(crate) const FLOWING_WATER: u16 = 8;
pub(crate) const STILL_WATER: u16 = 9;
pub(crate) const FLOWING_LAVA: u16 = 10;
pub(crate) const STILL_LAVA: u16 = 11;
pub(crate) const SAND: u16 = 12;
pub(crate) const GRAVEL: u16 = 13;
pub(crate) const LOG: u16 = 17;
pub(crate) const LEAVES: u16 = 18;
pub(crate) const TALL_GRASS: u16 = 31;
pub(crate) const DEAD_BUSH: u16 = 32;
pub(crate) const YELLOW_FLOWER: u16 = 37;
pub(crate) const RED_FLOWER: u16 = 38;
pub(crate) const BROWN_MUSHROOM: u16 = 39;
pub(crate) const RED_MUSHROOM: u16 = 40;
pub(crate) const MOSSY_COBBLESTONE: u16 = 48;
pub(crate) const FARMLAND: u16 = 60;
pub(crate) const SNOW_LAYER: u16 = 78;
pub(crate) const ICE: u16 = 79;
pub(crate) const SNOW: u16 = 80;
pub(crate) const CACTUS: u16 = 81;
pub(crate) const CLAY: u16 = 82;
pub(crate) const REEDS: u16 = 83;
pub(crate) const PUMPKIN: u16 = 86;
pub(crate) const BROWN_MUSHROOM_BLOCK: u16 = 99;
pub(crate) const RED_MUSHROOM_BLOCK: u16 = 100;
pub(crate) const MELON: u16 = 103;
pub(crate) const VINE: u16 = 106;
pub(crate) const MYCELIUM: u16 = 110;
pub(crate) const WATERLILY: u16 = 111;
pub(crate) const EMERALD_ORE: u16 = 129;
pub(crate) const STONE_SLAB: u16 = 44;
pub(crate) const SANDSTONE: u16 = 24;
pub(crate) const HARDENED_CLAY: u16 = 172;
pub(crate) const PACKED_ICE: u16 = 174;
pub(crate) const DOUBLE_PLANT: u16 = 175;
pub(crate) const PODZOL: u16 = 243;

pub(crate) const TALL_GRASS_TALL: u8 = 1;
pub(crate) const TALL_GRASS_FERN: u8 = 2;
pub(crate) const DOUBLE_SUNFLOWER: u8 = 0;
pub(crate) const DOUBLE_SYRINGA: u8 = 1;
pub(crate) const DOUBLE_GRASS: u8 = 2;
pub(crate) const DOUBLE_FERN: u8 = 3;
pub(crate) const DOUBLE_ROSE: u8 = 4;
pub(crate) const DOUBLE_PAEONIA: u8 = 5;
pub(crate) const DOUBLE_UPPER_BIT: u8 = 8;
pub(crate) const SANDSTONE_SLAB_DATA: u8 = 1;

pub(crate) const VINE_SOUTH: u8 = 1;
pub(crate) const VINE_WEST: u8 = 2;
pub(crate) const VINE_NORTH: u8 = 4;
pub(crate) const VINE_EAST: u8 = 8;

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub(crate) struct BlockPos {
    pub(crate) x: i32,
    pub(crate) y: i32,
    pub(crate) z: i32,
}

impl BlockPos {
    pub(crate) const fn new(x: i32, y: i32, z: i32) -> Self {
        Self { x, y, z }
    }

    pub(crate) const fn offset(self, x: i32, y: i32, z: i32) -> Self {
        Self {
            x: self.x.wrapping_add(x),
            y: self.y.wrapping_add(y),
            z: self.z.wrapping_add(z),
        }
    }

    pub(crate) const fn above(self, n: i32) -> Self {
        self.offset(0, n, 0)
    }

    pub(crate) const fn below(self, n: i32) -> Self {
        self.offset(0, -n, 0)
    }
}

pub(crate) fn next_gaussian_int(random: &mut MtRandom, bound: u32) -> i32 {
    random.next_gaussian_int(bound)
}

pub(crate) fn is_empty(region: &PopulationNeighborhood, pos: BlockPos) -> bool {
    region.block_id(pos.x, pos.y, pos.z) == AIR
}

pub(crate) fn set_block(
    region: &mut PopulationNeighborhood,
    pos: BlockPos,
    id: u16,
    data: u8,
) -> bool {
    region.set_state(pos.x, pos.y, pos.z, state(id, data))
}

pub(crate) fn block_id(region: &PopulationNeighborhood, pos: BlockPos) -> u16 {
    region.block_id(pos.x, pos.y, pos.z)
}

pub(crate) const fn is_water(id: u16) -> bool {
    matches!(id, FLOWING_WATER | STILL_WATER)
}

pub(crate) const fn is_leaves(id: u16) -> bool {
    matches!(id, LEAVES | 161)
}

pub(crate) const fn bush_support(id: u16) -> bool {
    matches!(id, GRASS | DIRT | FARMLAND | PODZOL)
}

pub(crate) const fn dead_bush_support(id: u16) -> bool {
    matches!(id, SAND | HARDENED_CLAY | 159 | PODZOL)
}

pub(crate) const fn material_is_solid(id: u16) -> bool {
    !matches!(
        id,
        AIR | FLOWING_WATER
            | STILL_WATER
            | FLOWING_LAVA
            | STILL_LAVA
            | SAPLING
            | 30
            | TALL_GRASS
            | DEAD_BUSH
            | YELLOW_FLOWER
            | RED_FLOWER
            | BROWN_MUSHROOM
            | RED_MUSHROOM
            | 50
            | 55
            | 59
            | 65
            | 66
            | 69
            | 90
            | 93
            | 104
            | 105
            | VINE
            | WATERLILY
            | 115
            | 119
            | 127
            | 131
            | 132
            | 140
            | 141
            | 142
            | 143
            | 171
            | DOUBLE_PLANT
            | 244
    )
}

pub(crate) const fn block_solid_flag(id: u16) -> bool {
    !matches!(
        id,
        AIR | FLOWING_WATER
            | STILL_WATER
            | FLOWING_LAVA
            | STILL_LAVA
            | SAPLING
            | LEAVES
            | 30
            | TALL_GRASS
            | DEAD_BUSH
            | YELLOW_FLOWER
            | RED_FLOWER
            | BROWN_MUSHROOM
            | RED_MUSHROOM
            | 50
            | 51
            | 55
            | 59
            | 63
            | 64
            | 65
            | 66
            | 68
            | 69
            | 70
            | 71
            | SNOW_LAYER
            | CACTUS
            | REEDS
            | 90
            | 93
            | 96
            | 104
            | 105
            | VINE
            | 107
            | WATERLILY
            | 115
            | 117
            | 118
            | 119
            | 120
            | 127
            | 131
            | 132
            | 140
            | 141
            | 142
            | 143
            | 144
            | 147
            | 148
            | 149
            | 151
            | 157
            | 161
            | 167
            | 171
            | DOUBLE_PLANT
            | 176
            | 177
            | 178
            | 183
            | 184
            | 185
            | 186
            | 187
            | 188
            | 189
            | 190
            | 191
            | 192
            | 193
            | 194
            | 195
            | 196
            | 197
            | 244
    )
}

pub(crate) fn place_sand_disk(
    region: &mut PopulationNeighborhood,
    pos: BlockPos,
    random: &mut MtRandom,
    target: u16,
    max_radius: u32,
) -> bool {
    if !is_water(block_id(region, pos)) {
        return false;
    }
    let radius = random.next_int(max_radius - 2) as i32 + 2;
    for x in pos.x - radius..=pos.x + radius {
        for z in pos.z - radius..=pos.z + radius {
            let dx = x - pos.x;
            let dz = z - pos.z;
            if dx * dx + dz * dz > radius * radius {
                continue;
            }
            for y in pos.y - 2..=pos.y + 2 {
                if matches!(region.block_id(x, y, z), DIRT | GRASS) {
                    let _ = region.set_state(x, y, z, state(target, 0));
                }
            }
        }
    }
    true
}

pub(crate) fn place_clay_disk(
    region: &mut PopulationNeighborhood,
    pos: BlockPos,
    random: &mut MtRandom,
) -> bool {
    let id = block_id(region, pos);
    if !is_water(id) && id != CLAY {
        return false;
    }
    let radius = random.next_int(2) as i32 + 2;
    for x in pos.x - radius..=pos.x + radius {
        for z in pos.z - radius..=pos.z + radius {
            let dx = x - pos.x;
            let dz = z - pos.z;
            if dx * dx + dz * dz > radius * radius {
                continue;
            }
            for y in pos.y - 1..=pos.y + 1 {
                if region.block_id(x, y, z) == DIRT {
                    let _ = region.set_state(x, y, z, state(CLAY, 0));
                }
            }
        }
    }
    true
}

pub(crate) fn place_spring(
    region: &mut PopulationNeighborhood,
    pos: BlockPos,
    liquid: u16,
) -> bool {
    if block_id(region, pos.above(1)) != STONE || block_id(region, pos.below(1)) != STONE {
        return false;
    }
    let current = block_id(region, pos);
    if current != AIR && current != STONE {
        return false;
    }

    let neighbors = [
        pos.offset(-1, 0, 0),
        pos.offset(1, 0, 0),
        pos.offset(0, 0, -1),
        pos.offset(0, 0, 1),
    ];
    let rock_count = neighbors
        .iter()
        .filter(|p| block_id(region, **p) == STONE)
        .count();
    let hole_count = neighbors.iter().filter(|p| is_empty(region, **p)).count();
    if rock_count == 3 && hole_count == 1 {
        return set_block(region, pos, liquid, 0);
    }
    true
}

fn mushroom_survives(region: &PopulationNeighborhood, pos: BlockPos) -> bool {
    if !(0..WORLD_HEIGHT as i32).contains(&pos.y) {
        return false;
    }
    let below = block_id(region, pos.below(1));
    matches!(below, PODZOL | MYCELIUM) || block_solid_flag(below)
}

fn ordinary_bush_survives(region: &PopulationNeighborhood, pos: BlockPos) -> bool {
    bush_support(block_id(region, pos.below(1)))
}

pub(crate) fn place_flower_patch(
    region: &mut PopulationNeighborhood,
    pos: BlockPos,
    flower_id: u16,
    flower_data: u8,
    random: &mut MtRandom,
) -> bool {
    for _ in 0..64 {
        let z = next_gaussian_int(random, 8);
        let y = next_gaussian_int(random, 4);
        let x = next_gaussian_int(random, 8);
        let at = pos.offset(x, y, z);
        if is_empty(region, at)
            && if matches!(flower_id, BROWN_MUSHROOM | RED_MUSHROOM) {
                mushroom_survives(region, at)
            } else {
                ordinary_bush_survives(region, at)
            }
        {
            let _ = set_block(region, at, flower_id, flower_data);
        }
    }
    true
}

pub(crate) fn place_tall_grass(
    region: &mut PopulationNeighborhood,
    pos: BlockPos,
    data: u8,
    random: &mut MtRandom,
    tries: usize,
    radius: u32,
) -> bool {
    let mut scan = pos;
    while (block_id(region, scan) == AIR || is_leaves(block_id(region, scan))) && scan.y > 0 {
        scan = scan.below(1);
    }
    let _ = scan;

    for _ in 0..tries {
        let z = next_gaussian_int(random, radius);
        let y = next_gaussian_int(random, 4);
        let x = next_gaussian_int(random, radius);
        let at = pos.offset(x, y, z);
        if is_empty(region, at) && ordinary_bush_survives(region, at) {
            let _ = set_block(region, at, TALL_GRASS, data);
        }
    }
    true
}

pub(crate) fn place_double_plant(
    region: &mut PopulationNeighborhood,
    pos: BlockPos,
    plant_type: u8,
    random: &mut MtRandom,
) -> bool {
    let mut placed = false;
    for _ in 0..64 {
        let z = next_gaussian_int(random, 8);
        let y = next_gaussian_int(random, 4);
        let x = next_gaussian_int(random, 8);
        let at = pos.offset(x, y, z);
        if at.y >= WORLD_HEIGHT as i32 - 2 {
            continue;
        }
        if is_empty(region, at)
            && is_empty(region, at.above(1))
            && ordinary_bush_survives(region, at)
        {
            let _ = set_block(region, at, DOUBLE_PLANT, plant_type);
            let _ = set_block(
                region,
                at.above(1),
                DOUBLE_PLANT,
                plant_type | DOUBLE_UPPER_BIT,
            );
            placed = true;
        }
    }
    placed
}

pub(crate) fn place_dead_bush(
    region: &mut PopulationNeighborhood,
    pos: BlockPos,
    random: &mut MtRandom,
) -> bool {
    let mut base = pos;
    while (is_empty(region, base) || is_leaves(block_id(region, base))) && base.y > 0 {
        base = base.below(1);
    }
    for _ in 0..4 {
        let z = next_gaussian_int(random, 8);
        let y = next_gaussian_int(random, 4);
        let x = next_gaussian_int(random, 8);
        let at = base.offset(x, y, z);
        if is_empty(region, at) && dead_bush_support(block_id(region, at.below(1))) {
            let _ = set_block(region, at, DEAD_BUSH, 0);
        }
    }
    true
}

fn reed_survives(region: &PopulationNeighborhood, pos: BlockPos) -> bool {
    let below = block_id(region, pos.below(1));
    if below == REEDS {
        return true;
    }
    if !matches!(below, GRASS | DIRT | SAND) {
        return false;
    }
    [
        pos.offset(-1, -1, 0),
        pos.offset(1, -1, 0),
        pos.offset(0, -1, -1),
        pos.offset(0, -1, 1),
    ]
    .into_iter()
    .any(|p| is_water(block_id(region, p)))
}

pub(crate) fn place_reeds(
    region: &mut PopulationNeighborhood,
    pos: BlockPos,
    random: &mut MtRandom,
) -> bool {
    for _ in 0..20 {
        let x = next_gaussian_int(random, 8);
        let z = next_gaussian_int(random, 8);
        let at = pos.offset(x, 0, z);
        if !is_empty(region, at) {
            continue;
        }
        let below = at.below(1);
        if ![
            below.offset(-1, 0, 0),
            below.offset(1, 0, 0),
            below.offset(0, 0, -1),
            below.offset(0, 0, 1),
        ]
        .into_iter()
        .any(|p| is_water(block_id(region, p)))
        {
            continue;
        }

        let random_height = random.next_int(3) + 1;
        let height = 2 + random.next_int(random_height);
        for dy in 0..height as i32 {
            let cane = at.above(dy);
            if reed_survives(region, cane) {
                let _ = set_block(region, cane, REEDS, 0);
            }
        }
    }
    true
}

fn cactus_survives(region: &PopulationNeighborhood, pos: BlockPos) -> bool {
    for side in [
        pos.offset(-1, 0, 0),
        pos.offset(1, 0, 0),
        pos.offset(0, 0, -1),
        pos.offset(0, 0, 1),
    ] {
        if material_is_solid(block_id(region, side)) {
            return false;
        }
    }
    matches!(block_id(region, pos.below(1)), CACTUS | SAND)
}

pub(crate) fn place_cactus(
    region: &mut PopulationNeighborhood,
    pos: BlockPos,
    random: &mut MtRandom,
) -> bool {
    for _ in 0..10 {
        let z = next_gaussian_int(random, 8);
        let y = next_gaussian_int(random, 4);
        let x = next_gaussian_int(random, 8);
        let at = pos.offset(x, y, z);
        if !is_empty(region, at) {
            continue;
        }

        let random_height = random.next_int(3) + 1;
        let height = 1 + random.next_int(random_height);
        for dy in 0..height as i32 {
            let cactus = at.above(dy);
            if cactus_survives(region, cactus) {
                let _ = set_block(region, cactus, CACTUS, 0);
            }
        }
    }
    true
}

pub(crate) fn place_waterlily(
    region: &mut PopulationNeighborhood,
    pos: BlockPos,
    random: &mut MtRandom,
) -> bool {
    for _ in 0..10 {
        let z = next_gaussian_int(random, 8);
        let y = next_gaussian_int(random, 4);
        let x = next_gaussian_int(random, 8);
        let at = pos.offset(x, y, z);
        if is_empty(region, at) && block_id(region, at.below(1)) == STILL_WATER {
            let _ = set_block(region, at, WATERLILY, 0);
        }
    }
    true
}

pub(crate) fn place_pumpkin(
    region: &mut PopulationNeighborhood,
    pos: BlockPos,
    random: &mut MtRandom,
) -> bool {
    for _ in 0..64 {
        let x = next_gaussian_int(random, 8);
        let z = next_gaussian_int(random, 8);
        let y = next_gaussian_int(random, 4);
        let at = pos.offset(x, y, z);
        if is_empty(region, at)
            && block_id(region, at.below(1)) == GRASS
            && block_solid_flag(block_id(region, at.below(1)))
        {
            let data = random.next_int(4) as u8;
            let _ = set_block(region, at, PUMPKIN, data);
        }
    }
    true
}

pub(crate) fn place_melon(
    region: &mut PopulationNeighborhood,
    pos: BlockPos,
    random: &mut MtRandom,
) -> bool {
    for _ in 0..64 {
        let z = next_gaussian_int(random, 8);
        let y = next_gaussian_int(random, 4);
        let x = next_gaussian_int(random, 8);
        let at = pos.offset(x, y, z);
        if is_empty(region, at)
            && block_id(region, at.below(1)) == GRASS
            && block_solid_flag(block_id(region, at.below(1)))
        {
            let _ = set_block(region, at, MELON, 0);
        }
    }
    true
}

fn acceptable_vine_neighbor(id: u16) -> bool {
    id != AIR && block_solid_flag(id) && material_is_solid(id) && id != 95
}

pub(crate) fn place_vines(
    region: &mut PopulationNeighborhood,
    pos: BlockPos,
    random: &mut MtRandom,
) -> bool {
    let mut at = pos;
    while at.y < WORLD_HEIGHT as i32 {
        if is_empty(region, at) {
            let candidates = [
                (at.offset(0, 0, 1), VINE_NORTH),
                (at.offset(0, 0, -1), VINE_SOUTH),
                (at.offset(1, 0, 0), VINE_WEST),
                (at.offset(-1, 0, 0), VINE_EAST),
            ];
            for (neighbor, data) in candidates {
                if acceptable_vine_neighbor(block_id(region, neighbor)) {
                    let _ = set_block(region, at, VINE, data);
                    break;
                }
            }
        } else {
            at.x = pos.x.wrapping_add(next_gaussian_int(random, 4));
            at.z = pos.z.wrapping_add(next_gaussian_int(random, 4));
        }
        at.y += 1;
    }
    true
}

pub(crate) fn place_desert_well(region: &mut PopulationNeighborhood, pos: BlockPos) -> bool {
    let mut base = pos;
    while is_empty(region, base) && base.y > 2 {
        base = base.below(1);
    }
    if block_id(region, base) != SAND {
        return false;
    }

    for dx in -2..=2 {
        for dz in -2..=2 {
            if is_empty(region, base.offset(dx, -1, dz))
                && is_empty(region, base.offset(dx, -2, dz))
            {
                return false;
            }
        }
    }

    for dy in -1..=0 {
        for dx in -2..=2 {
            for dz in -2..=2 {
                let _ = set_block(region, base.offset(dx, dy, dz), SANDSTONE, 0);
            }
        }
    }
    for at in [
        base,
        base.offset(-1, 0, 0),
        base.offset(1, 0, 0),
        base.offset(0, 0, -1),
        base.offset(0, 0, 1),
    ] {
        let _ = set_block(region, at, STILL_WATER, 0);
    }
    for dx in -2_i32..=2_i32 {
        for dz in -2_i32..=2_i32 {
            if dx.abs() == 2 || dz.abs() == 2 {
                let _ = set_block(region, base.offset(dx, 1, dz), SANDSTONE, 0);
            }
        }
    }
    for at in [
        base.offset(2, 1, 0),
        base.offset(-2, 1, 0),
        base.offset(0, 1, 2),
        base.offset(0, 1, -2),
    ] {
        let _ = set_block(region, at, STONE_SLAB, SANDSTONE_SLAB_DATA);
    }
    for dx in -1_i32..=1_i32 {
        for dz in -1_i32..=1_i32 {
            if dx == 0 && dz == 0 {
                let _ = set_block(region, base.offset(dx, 4, dz), SANDSTONE, 0);
            } else {
                let _ = set_block(
                    region,
                    base.offset(dx, 4, dz),
                    STONE_SLAB,
                    SANDSTONE_SLAB_DATA,
                );
            }
        }
    }
    for dy in 1..=3 {
        for (dx, dz) in [(-1, -1), (-1, 1), (1, -1), (1, 1)] {
            let _ = set_block(region, base.offset(dx, dy, dz), SANDSTONE, 0);
        }
    }
    true
}

pub(crate) fn place_block_blob(
    region: &mut PopulationNeighborhood,
    pos: BlockPos,
    random: &mut MtRandom,
    target: u16,
    start_radius: i32,
) -> bool {
    let mut center = pos;
    while center.y > 3 {
        if !is_empty(region, center.below(1))
            && matches!(block_id(region, center.below(1)), GRASS | DIRT | STONE)
        {
            break;
        }
        center = center.below(1);
    }
    if center.y <= 3 {
        return false;
    }

    let mut count = 0;
    while start_radius >= 0 && count < 3 {
        let xr = start_radius + random.next_int(2) as i32;
        let yr = start_radius + random.next_int(2) as i32;
        let zr = start_radius + random.next_int(2) as i32;
        let threshold = (xr + yr + zr) as f32 * 0.333 + 0.5;
        let threshold_sq = threshold * threshold;
        for x in center.x - xr..=center.x + xr {
            for y in center.y - yr..=center.y + yr {
                for z in center.z - zr..=center.z + zr {
                    let dx = x - center.x;
                    let dy = y - center.y;
                    let dz = z - center.z;
                    if (dx * dx + dy * dy + dz * dz) as f32 <= threshold_sq {
                        let _ = region.set_state(x, y, z, state(target, 0));
                    }
                }
            }
        }
        let dx = -(start_radius + 1) + random.next_int((2 + start_radius * 2) as u32) as i32;
        let dy = -(random.next_int(2) as i32);
        let dz = -(start_radius + 1) + random.next_int((2 + start_radius * 2) as u32) as i32;
        center = center.offset(dx, dy, dz);
        count += 1;
    }
    true
}

pub(crate) fn place_ice_patch(
    region: &mut PopulationNeighborhood,
    pos: BlockPos,
    random: &mut MtRandom,
) -> bool {
    let mut base = pos;
    while is_empty(region, base) && base.y > 2 {
        base = base.below(1);
    }
    let current = block_id(region, base);
    if current != SNOW || current == PACKED_ICE {
        return false;
    }

    let radius = random.next_int(2) as i32 + 2;
    for x in base.x - radius..=base.x + radius {
        for z in base.z - radius..=base.z + radius {
            let dx = x - base.x;
            let dz = z - base.z;
            if dx * dx + dz * dz > radius * radius {
                continue;
            }
            for y in base.y - 1..=base.y + 1 {
                if matches!(region.block_id(x, y, z), DIRT | SNOW | ICE) {
                    let _ = region.set_state(x, y, z, state(PACKED_ICE, 0));
                }
            }
        }
    }
    true
}

fn ice_spike_replaceable(id: u16) -> bool {
    matches!(id, AIR | DIRT | SNOW | ICE)
}

pub(crate) fn place_ice_spike(
    region: &mut PopulationNeighborhood,
    pos: BlockPos,
    random: &mut MtRandom,
) -> bool {
    let mut base = pos;
    while is_empty(region, base) && base.y > 2 {
        base = base.below(1);
    }
    if block_id(region, base) != SNOW {
        return false;
    }

    base = base.above(random.next_int(4) as i32);
    let height = random.next_int(4) as i32 + 7;
    let width = height / 4 + random.next_int(2) as i32;
    if width > 1 && random.next_int(60) == 0 {
        base = base.above(10 + random.next_int(30) as i32);
    }

    for y_off in 0..height {
        let scale = (1.0_f32 - y_off as f32 / height as f32) * width as f32;
        let new_width = scale.ceil() as i32;
        for x_off in -new_width..=new_width {
            let dx = x_off.abs() as f32 - 0.25;
            for z_off in -new_width..=new_width {
                let dz = z_off.abs() as f32 - 0.25;
                if (x_off != 0 || z_off != 0) && dx * dx + dz * dz > scale * scale {
                    continue;
                }
                if (x_off == -new_width
                    || x_off == new_width
                    || z_off == -new_width
                    || z_off == new_width)
                    && random.next_float() > 0.75
                {
                    continue;
                }
                let above = base.offset(x_off, y_off, z_off);
                if ice_spike_replaceable(block_id(region, above)) {
                    let _ = set_block(region, above, PACKED_ICE, 0);
                }
                if y_off != 0 && new_width > 1 {
                    let below = base.offset(x_off, -y_off, z_off);
                    if ice_spike_replaceable(block_id(region, below)) {
                        let _ = set_block(region, below, PACKED_ICE, 0);
                    }
                }
            }
        }
    }

    let pillar_width = (width - 1).clamp(0, 1);
    for x_off in -pillar_width..=pillar_width {
        for z_off in -pillar_width..=pillar_width {
            let mut at = base.offset(x_off, -1, z_off);
            let mut run = if x_off.abs() == 1 && z_off.abs() == 1 {
                random.next_int(5) as i32
            } else {
                50
            };
            while at.y > 50 {
                let id = block_id(region, at);
                if ice_spike_replaceable(id) || id == PACKED_ICE {
                    let _ = set_block(region, at, PACKED_ICE, 0);
                } else {
                    break;
                }
                at = at.below(1);
                run -= 1;
                if run <= 0 {
                    at = at.below(random.next_int(5) as i32 + 1);
                    run = random.next_int(5) as i32;
                }
            }
        }
    }
    true
}

fn huge_mushroom_ground(id: u16) -> bool {
    matches!(id, DIRT | GRASS | MYCELIUM | PODZOL)
}

pub(crate) fn place_huge_mushroom(
    region: &mut PopulationNeighborhood,
    pos: BlockPos,
    random: &mut MtRandom,
    forced_type: Option<u8>,
    swamp: bool,
) -> bool {
    let mut base = pos;
    let kind = forced_type.unwrap_or_else(|| random.next_int(2) as u8);
    let mut height = random.next_int(3) as i32 + 4;
    if random.next_int(12) == 0 {
        height *= 2;
    }
    if base.y < 1 || base.y + height + 1 >= WORLD_HEIGHT as i32 {
        return false;
    }

    for y in base.y..=base.y + 1 + height {
        let radius = if y <= base.y + 3 { 0 } else { 3 };
        for x in base.x - radius..=base.x + radius {
            for z in base.z - radius..=base.z + radius {
                let id = region.block_id(x, y, z);
                if id != AIR && !is_leaves(id) {
                    return false;
                }
            }
        }
    }

    let below1 = block_id(region, base.below(1));
    if !huge_mushroom_ground(below1) {
        if !swamp {
            return false;
        }
        let below2 = block_id(region, base.below(2));
        let below3 = block_id(region, base.below(3));
        if below1 == STILL_WATER
            && (huge_mushroom_ground(below2)
                || (below2 == STILL_WATER && huge_mushroom_ground(below3)))
        {
            let moved = if below2 == STILL_WATER { 2 } else { 1 };
            base.y -= moved;
            height += moved;
        } else {
            return false;
        }
    }

    let low = if kind == 1 {
        base.y + height - 3
    } else {
        base.y + height
    };
    for y in low..=base.y + height {
        let mut radius = 1;
        if y < base.y + height {
            radius += 1;
        }
        if kind == 0 {
            radius = 3;
        }

        for x in base.x - radius..=base.x + radius {
            for z in base.z - radius..=base.z + radius {
                let mut data = 5_i32;
                if x == base.x - radius {
                    data -= 1;
                }
                if x == base.x + radius {
                    data += 1;
                }
                if z == base.z - radius {
                    data -= 3;
                }
                if z == base.z + radius {
                    data += 3;
                }

                if kind == 0 || y < base.y + height {
                    if (x == base.x - radius || x == base.x + radius)
                        && (z == base.z - radius || z == base.z + radius)
                    {
                        continue;
                    }
                    if (x == base.x - (radius - 1) && z == base.z - radius)
                        || (x == base.x - radius && z == base.z - (radius - 1))
                    {
                        data = 1;
                    }
                    if (x == base.x + (radius - 1) && z == base.z - radius)
                        || (x == base.x + radius && z == base.z - (radius - 1))
                    {
                        data = 3;
                    }
                    if (x == base.x - (radius - 1) && z == base.z + radius)
                        || (x == base.x - radius && z == base.z + (radius - 1))
                    {
                        data = 7;
                    }
                    if (x == base.x + (radius - 1) && z == base.z + radius)
                        || (x == base.x + radius && z == base.z + (radius - 1))
                    {
                        data = 9;
                    }
                }

                if data == 5 && y < base.y + height {
                    data = 0;
                }
                if data != 0 || base.y >= base.y + height - 1 {
                    let at = BlockPos::new(x, y, z);
                    if !block_solid_flag(block_id(region, at)) {
                        let block = if kind == 0 {
                            BROWN_MUSHROOM_BLOCK
                        } else {
                            RED_MUSHROOM_BLOCK
                        };
                        let _ = set_block(region, at, block, data as u8);
                    }
                }
            }
        }
    }

    for dy in 0..height {
        let at = base.above(dy);
        if !block_solid_flag(block_id(region, at)) {
            let block = if kind == 0 {
                BROWN_MUSHROOM_BLOCK
            } else {
                RED_MUSHROOM_BLOCK
            };
            let _ = set_block(region, at, block, 10);
        }
    }
    true
}
