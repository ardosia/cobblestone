use crate::WORLD_HEIGHT;
use crate::population::PopulationNeighborhood;
use crate::population_feature::{
    AIR, BROWN_MUSHROOM, BlockPos, DIRT, GRASS, LEAVES, LOG, PODZOL, RED_MUSHROOM, VINE, VINE_EAST,
    VINE_NORTH, VINE_SOUTH, VINE_WEST, block_id, block_solid_flag, bush_support, is_empty,
    is_leaves, material_is_solid, set_block,
};
use crate::terrain_shape::noise::MtRandom;

const LEAVES2: u16 = 161;
const LOG2: u16 = 162;
const COCOA: u16 = 127;

const OLD_OAK: u8 = 0;
const OLD_SPRUCE: u8 = 1;
const OLD_BIRCH: u8 = 2;
const OLD_JUNGLE: u8 = 3;
const NEW_ACACIA: u8 = 0;
const NEW_DARK_OAK: u8 = 1;

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub(crate) enum TreeKind {
    Oak,
    FancyOak,
    Birch,
    SuperBirch,
    Spruce,
    Pine,
    RoofedOak,
    Swamp,
    JungleBush,
    Jungle,
    MegaJungle,
    MegaPine,
    MegaSpruce,
    Savanna,
}

pub(crate) fn place_tree(
    kind: TreeKind,
    region: &mut PopulationNeighborhood,
    pos: BlockPos,
    random: &mut MtRandom,
) -> bool {
    match kind {
        TreeKind::Oak => {
            let height = random.next_int(3) as i32 + 4;
            place_standard_tree(region, pos, random, OLD_OAK, OLD_OAK, false, height)
        }
        TreeKind::FancyOak => place_fancy_tree(region, pos, random),
        TreeKind::Birch => {
            let height = random.next_int(3) as i32 + 5;
            place_standard_tree(region, pos, random, OLD_BIRCH, OLD_BIRCH, false, height)
        }
        TreeKind::SuperBirch => {
            let height = random.next_int(3) as i32 + 5 + random.next_int(7) as i32;
            place_standard_tree(region, pos, random, OLD_BIRCH, OLD_BIRCH, false, height)
        }
        TreeKind::Spruce => place_spruce(region, pos, random),
        TreeKind::Pine => place_pine(region, pos, random),
        TreeKind::RoofedOak => place_roofed(region, pos, random),
        TreeKind::Swamp => place_swamp(region, pos, random),
        TreeKind::JungleBush => place_ground_bush(region, pos, random),
        TreeKind::Jungle => {
            let height = random.next_int(7) as i32 + 4;
            place_standard_tree(region, pos, random, OLD_JUNGLE, OLD_JUNGLE, true, height)
        }
        TreeKind::MegaJungle => place_mega_jungle(region, pos, random),
        TreeKind::MegaPine => place_mega_pine(region, pos, random, false),
        TreeKind::MegaSpruce => place_mega_pine(region, pos, random, true),
        TreeKind::Savanna => place_savanna(region, pos, random),
    }
}

fn tree_free_id(id: u16) -> bool {
    id == AIR || is_leaves(id) || matches!(id, GRASS | DIRT)
}

fn tree_free_material(id: u16) -> bool {
    id == AIR || is_leaves(id)
}

fn solid_blocking(id: u16) -> bool {
    block_solid_flag(id) && material_is_solid(id)
}

fn prepare_spawn(region: &mut PopulationNeighborhood, pos: BlockPos, tree_height: i32) -> bool {
    if pos.y < 1 || pos.y + tree_height + 1 > WORLD_HEIGHT as i32 {
        return false;
    }
    if !bush_support(block_id(region, pos.below(1))) {
        return false;
    }

    let mut free = true;
    for y in pos.y..=pos.y + 1 + tree_height {
        let radius = if y == pos.y {
            0
        } else if y >= pos.y + 1 + tree_height - 2 {
            2
        } else {
            1
        };
        for x in pos.x - radius..=pos.x + radius {
            for z in pos.z - radius..=pos.z + radius {
                if !(0..WORLD_HEIGHT as i32).contains(&y) || !tree_free_id(region.block_id(x, y, z))
                {
                    free = false;
                    break;
                }
            }
            if !free {
                break;
            }
        }
        if !free {
            break;
        }
    }

    if free {
        let _ = set_block(region, pos.below(1), DIRT, 0);
    }
    free
}

fn old_log_data(kind: u8, axis: u8) -> u8 {
    kind | (axis << 2)
}

fn place_fallen_trunk(
    region: &mut PopulationNeighborhood,
    pos: BlockPos,
    random: &mut MtRandom,
    height: i32,
    trunk_type: u8,
) {
    let facing = random.next_int(4) as i32 + 2;
    let (dx, dz, axis) = match facing {
        2 => (0, -1, 2_u8),
        3 => (0, 1, 2_u8),
        4 => (-1, 0, 1_u8),
        5 => (1, 0, 1_u8),
        _ => unreachable!(),
    };
    let distance = 2 + random.next_int(2) as i32;
    let mut start = pos.offset(dx * distance, 0, dz * distance);
    start.y = region.generation_height(start.x, start.z);
    if start.y > pos.y + 1 {
        return;
    }

    let length = height - 2;
    let mut at = start;
    let mut empty_blocks = 0;
    for _ in 0..length {
        if !is_empty(region, at) {
            return;
        }
        if !solid_blocking(block_id(region, at.below(1))) {
            empty_blocks += 1;
            if empty_blocks > 2 {
                return;
            }
        } else {
            empty_blocks = 0;
        }
        at = at.offset(dx, 0, dz);
    }

    at = start;
    let data = old_log_data(trunk_type, axis);
    for _ in 0..length {
        let _ = set_block(region, at, LOG, data);
        if random.next_int(10) == 0 && is_empty(region, at.above(1)) {
            let mushroom = if random.next_float() < 0.5 {
                BROWN_MUSHROOM
            } else {
                RED_MUSHROOM
            };
            let _ = set_block(region, at.above(1), mushroom, 0);
        }
        at = at.offset(dx, 0, dz);
    }
}

fn place_trunk(
    region: &mut PopulationNeighborhood,
    pos: BlockPos,
    random: &mut MtRandom,
    mut tree_height: i32,
    trunk_type: u8,
    jungle: bool,
) -> bool {
    let stump = random.next_int(80) == 0;
    let vine_chance = if trunk_type == OLD_BIRCH {
        0.0
    } else if stump {
        0.75
    } else if jungle {
        1.0 / 3.0
    } else if random.next_int(12) == 0 {
        1.0
    } else {
        0.0
    };

    if stump {
        place_fallen_trunk(region, pos, random, tree_height, trunk_type);
        tree_height = 1;
    }

    for dy in 0..tree_height {
        let at = pos.above(dy);
        let id = block_id(region, at);
        if matches!(id, AIR | LEAVES | DIRT | GRASS) {
            let _ = set_block(region, at, LOG, trunk_type);
            if vine_chance > 0.0 {
                for (side, data) in [
                    (at.offset(-1, 0, 0), VINE_EAST),
                    (at.offset(1, 0, 0), VINE_WEST),
                    (at.offset(0, 0, -1), VINE_SOUTH),
                    (at.offset(0, 0, 1), VINE_NORTH),
                ] {
                    if random.next_float() < vine_chance && is_empty(region, side) {
                        let _ = set_block(region, side, VINE, data);
                    }
                }
            }
        }
    }
    !stump
}

fn place_leaf(region: &mut PopulationNeighborhood, pos: BlockPos, leaf_id: u16, leaf_data: u8) {
    if !block_solid_flag(block_id(region, pos)) {
        let _ = set_block(region, pos, leaf_id, leaf_data);
    }
}

fn add_hanging_vine(region: &mut PopulationNeighborhood, mut pos: BlockPos, data: u8) {
    let _ = set_block(region, pos, VINE, data);
    let mut remaining = 4;
    pos = pos.below(1);
    while is_empty(region, pos) && remaining > 0 {
        let _ = set_block(region, pos, VINE, data);
        remaining -= 1;
        pos = pos.below(1);
    }
}

fn add_jungle_leaf_vines(
    region: &mut PopulationNeighborhood,
    pos: BlockPos,
    random: &mut MtRandom,
) {
    if block_id(region, pos) != LEAVES {
        return;
    }
    for (side, data) in [
        (pos.offset(-1, 0, 0), VINE_EAST),
        (pos.offset(1, 0, 0), VINE_WEST),
        (pos.offset(0, 0, -1), VINE_SOUTH),
        (pos.offset(0, 0, 1), VINE_NORTH),
    ] {
        if random.next_int(4) == 0 && is_empty(region, side) {
            add_hanging_vine(region, side, data);
        }
    }
}

fn add_cocoa(
    region: &mut PopulationNeighborhood,
    pos: BlockPos,
    random: &mut MtRandom,
    height: i32,
) {
    const STEP_X: [i32; 4] = [0, -1, 0, 1];
    const STEP_Z: [i32; 4] = [1, 0, -1, 0];
    const OPPOSITE: [usize; 4] = [2, 3, 0, 1];
    for row in 0..2 {
        for (dir, opposite) in OPPOSITE.into_iter().enumerate() {
            if random.next_int((4 - row) as u32) == 0 {
                let age = random.next_int(3) as u8;
                let at = pos.offset(STEP_X[opposite], height - 5 + row, STEP_Z[opposite]);
                let _ = set_block(region, at, COCOA, (age << 2) | dir as u8);
            }
        }
    }
}

fn place_standard_tree(
    region: &mut PopulationNeighborhood,
    pos: BlockPos,
    random: &mut MtRandom,
    trunk_type: u8,
    leaf_type: u8,
    jungle: bool,
    tree_height: i32,
) -> bool {
    if !prepare_spawn(region, pos, tree_height) {
        return false;
    }
    if !place_trunk(region, pos, random, tree_height, trunk_type, jungle) {
        return true;
    }

    for y in pos.y - 3 + tree_height..=pos.y + tree_height {
        let yo = y - (pos.y + tree_height);
        let radius = 1 - yo / 2;
        for x in pos.x - radius..=pos.x + radius {
            let xo = x - pos.x;
            for z in pos.z - radius..=pos.z + radius {
                let zo = z - pos.z;
                if xo.abs() == radius && zo.abs() == radius && (random.next_int(2) == 0 || yo == 0)
                {
                    continue;
                }
                place_leaf(region, BlockPos::new(x, y, z), LEAVES, leaf_type);
            }
        }
    }

    if jungle {
        for y in pos.y - 3 + tree_height..=pos.y + tree_height {
            let yo = y - (pos.y + tree_height);
            let radius = 2 - yo / 2;
            for x in pos.x - radius..=pos.x + radius {
                for z in pos.z - radius..=pos.z + radius {
                    add_jungle_leaf_vines(region, BlockPos::new(x, y, z), random);
                }
            }
        }
        if random.next_int(5) == 0 && tree_height > 5 {
            add_cocoa(region, pos, random, tree_height);
        }
    }

    let _ = set_block(region, pos.below(1), DIRT, 0);
    true
}

fn place_spruce(region: &mut PopulationNeighborhood, pos: BlockPos, random: &mut MtRandom) -> bool {
    let height = random.next_int(4) as i32 + 6;
    if !prepare_spawn(region, pos, height) {
        return false;
    }
    let trunk_height = 1 + random.next_int(2) as i32;
    let top_height = height - trunk_height;
    let leaf_radius = 2 + random.next_int(2) as i32;
    let trunk_place_height = height - random.next_int(3) as i32;
    if !place_trunk(region, pos, random, trunk_place_height, OLD_SPRUCE, false) {
        return true;
    }

    let mut current_radius = random.next_int(2) as i32;
    let mut max_radius = 1;
    let mut min_radius = 0;
    for height_pos in 0..=top_height {
        let y = pos.y + height - height_pos;
        for x in pos.x - current_radius..=pos.x + current_radius {
            let xo = x - pos.x;
            for z in pos.z - current_radius..=pos.z + current_radius {
                let zo = z - pos.z;
                if xo.abs() == current_radius && zo.abs() == current_radius && current_radius > 0 {
                    continue;
                }
                place_leaf(region, BlockPos::new(x, y, z), LEAVES, OLD_SPRUCE);
            }
        }
        if current_radius >= max_radius {
            current_radius = min_radius;
            min_radius = 1;
            max_radius += 1;
            if max_radius > leaf_radius {
                max_radius = leaf_radius;
            }
        } else {
            current_radius += 1;
        }
    }
    true
}

fn place_pine(region: &mut PopulationNeighborhood, pos: BlockPos, random: &mut MtRandom) -> bool {
    let height = random.next_int(5) as i32 + 7;
    let trunk_height = height - random.next_int(2) as i32 - 3;
    let top_height = height - trunk_height;
    let top_radius = 1 + random.next_int((top_height + 1) as u32) as i32;

    if pos.y < 1 || pos.y + height + 1 > WORLD_HEIGHT as i32 {
        return false;
    }
    for y in pos.y..=pos.y + 1 + height {
        let radius = if y - pos.y < trunk_height {
            0
        } else {
            top_radius
        };
        for x in pos.x - radius..=pos.x + radius {
            for z in pos.z - radius..=pos.z + radius {
                if !(0..WORLD_HEIGHT as i32).contains(&y) {
                    return false;
                }
                let id = region.block_id(x, y, z);
                if id != AIR && id != LEAVES {
                    return false;
                }
            }
        }
    }

    let below = block_id(region, pos.below(1));
    if !matches!(below, GRASS | DIRT) || pos.y >= WORLD_HEIGHT as i32 - height - 1 {
        return false;
    }
    let _ = set_block(region, pos.below(1), DIRT, 0);

    let mut radius = 0;
    for y in (pos.y + trunk_height..=pos.y + height).rev() {
        for x in pos.x - radius..=pos.x + radius {
            let xo = x - pos.x;
            for z in pos.z - radius..=pos.z + radius {
                let zo = z - pos.z;
                if xo.abs() == radius && zo.abs() == radius && radius > 0 {
                    continue;
                }
                let at = BlockPos::new(x, y, z);
                if !block_solid_flag(block_id(region, at)) {
                    let _ = set_block(region, at, LEAVES, OLD_SPRUCE);
                }
            }
        }
        if radius >= 1 && y == pos.y + trunk_height + 1 {
            radius -= 1;
        } else if radius < top_radius {
            radius += 1;
        }
    }

    for dy in 0..height - 1 {
        let at = pos.above(dy);
        if matches!(block_id(region, at), AIR | LEAVES) {
            let _ = set_block(region, at, LOG, OLD_SPRUCE);
        }
    }
    true
}

fn place_ground_bush(
    region: &mut PopulationNeighborhood,
    pos: BlockPos,
    random: &mut MtRandom,
) -> bool {
    let mut base = pos;
    while tree_free_material(block_id(region, base)) && base.y > 0 {
        base = base.below(1);
    }
    if matches!(block_id(region, base), DIRT | GRASS) {
        // Restored target bug: GroundBush writes the leaf type as the log type.
        let _ = set_block(region, base, LOG, OLD_JUNGLE);
        for y in base.y..=base.y + 2 {
            let yo = y - base.y;
            let radius = 2 - yo;
            for x in base.x - radius..=base.x + radius {
                let xo = x - base.x;
                for z in base.z - radius..=base.z + radius {
                    let zo = z - base.z;
                    if xo.abs() == radius && zo.abs() == radius && random.next_int(2) == 0 {
                        continue;
                    }
                    let at = BlockPos::new(x, y, z);
                    if !block_solid_flag(block_id(region, at)) {
                        let _ = set_block(region, at, LEAVES, OLD_JUNGLE);
                    }
                }
            }
        }
    }
    true
}

fn place_swamp(region: &mut PopulationNeighborhood, pos: BlockPos, random: &mut MtRandom) -> bool {
    let mut base = pos;
    let height = random.next_int(4) as i32 + 5;
    while matches!(block_id(region, base.below(1)), 8 | 9 | LOG) {
        base = base.below(1);
    }
    if base.y < 1 || base.y + height + 1 > WORLD_HEIGHT as i32 {
        return false;
    }

    for y in base.y..=base.y + 1 + height {
        let radius = if y == base.y {
            0
        } else if y >= base.y + 1 + height - 2 {
            3
        } else {
            1
        };
        for x in base.x - radius..=base.x + radius {
            for z in base.z - radius..=base.z + radius {
                if !(0..WORLD_HEIGHT as i32).contains(&y) {
                    return false;
                }
                let id = region.block_id(x, y, z);
                if id != LOG && !tree_free_material(id) {
                    if matches!(id, 8 | 9) && y <= base.y {
                        continue;
                    }
                    return false;
                }
            }
        }
    }

    if !matches!(block_id(region, base.below(1)), GRASS | DIRT)
        || base.y >= WORLD_HEIGHT as i32 - height - 1
    {
        return false;
    }
    let _ = set_block(region, base.below(1), DIRT, 0);

    for y in base.y - 3 + height..=base.y + height {
        let yo = y - (base.y + height);
        let radius = 2 - yo / 2;
        for x in base.x - radius..=base.x + radius {
            let xo = x - base.x;
            for z in base.z - radius..=base.z + radius {
                let zo = z - base.z;
                if xo.abs() == radius && zo.abs() == radius && (random.next_int(2) == 0 || yo == 0)
                {
                    continue;
                }
                let at = BlockPos::new(x, y, z);
                if !solid_blocking(block_id(region, at)) {
                    let _ = set_block(region, at, LEAVES, OLD_OAK);
                }
            }
        }
    }

    for dy in 0..height {
        let at = base.above(dy);
        if tree_free_material(block_id(region, at)) || matches!(block_id(region, at), 8 | 9) {
            let _ = set_block(region, at, LOG, OLD_OAK);
        }
    }

    for y in base.y - 3 + height..=base.y + height {
        let yo = y - (base.y + height);
        let radius = 2 - yo / 2;
        for x in base.x - radius..=base.x + radius {
            for z in base.z - radius..=base.z + radius {
                let leaf = BlockPos::new(x, y, z);
                if block_id(region, leaf) != LEAVES {
                    continue;
                }
                for (side, data) in [
                    (leaf.offset(-1, 0, 0), VINE_EAST),
                    (leaf.offset(1, 0, 0), VINE_WEST),
                    (leaf.offset(0, 0, -1), VINE_SOUTH),
                    (leaf.offset(0, 0, 1), VINE_NORTH),
                ] {
                    if random.next_int(4) == 0 && is_empty(region, side) {
                        add_hanging_vine(region, side, data);
                    }
                }
            }
        }
    }
    true
}

fn place_new_leaf(region: &mut PopulationNeighborhood, pos: BlockPos, data: u8) {
    if tree_free_material(block_id(region, pos)) {
        let _ = set_block(region, pos, LEAVES2, data);
    }
}

fn place_savanna(
    region: &mut PopulationNeighborhood,
    pos: BlockPos,
    random: &mut MtRandom,
) -> bool {
    let height = random.next_int(3) as i32 + random.next_int(3) as i32 + 5;
    if !prepare_spawn(region, pos, height) || pos.y >= WORLD_HEIGHT as i32 - height - 1 {
        return false;
    }
    let _ = set_block(region, pos.below(1), DIRT, 0);

    const DX: [i32; 4] = [0, -1, 0, 1];
    const DZ: [i32; 4] = [1, 0, -1, 0];
    let lean_direction = random.next_int(4) as usize;
    let lean_height = height - random.next_int(4) as i32 - 1;
    let mut lean_steps = 3 - random.next_int(3) as i32;
    let mut at = pos;
    let mut last_y = 0;
    for yo in 0..height {
        let y = pos.y + yo;
        if yo >= lean_height && lean_steps > 0 {
            at.x += DX[lean_direction];
            at.z += DZ[lean_direction];
            lean_steps -= 1;
        }
        at.y = y;
        if tree_free_material(block_id(region, at)) {
            let _ = set_block(region, at, LOG2, NEW_ACACIA);
            last_y = y;
        }
    }

    at.y = last_y;
    for dx in -1..=1 {
        for dz in -1..=1 {
            place_new_leaf(region, at.offset(dx, 1, dz), NEW_ACACIA);
        }
    }
    for leaf in [
        at.offset(2, 1, 0),
        at.offset(-2, 1, 0),
        at.offset(0, 1, 2),
        at.offset(0, 1, -2),
    ] {
        place_new_leaf(region, leaf, NEW_ACACIA);
    }
    for dx in -3_i32..=3_i32 {
        for dz in -3_i32..=3_i32 {
            if dx.abs() == 3 && dz.abs() == 3 {
                continue;
            }
            place_new_leaf(region, at.offset(dx, 0, dz), NEW_ACACIA);
        }
    }

    at.x = pos.x;
    at.z = pos.z;
    let branch_direction = random.next_int(4) as usize;
    if branch_direction != lean_direction {
        let branch_pos = lean_height - random.next_int(2) as i32 - 1;
        let mut branch_steps = 1 + random.next_int(3) as i32;
        last_y = 0;
        let mut yo = branch_pos;
        while yo < height && branch_steps > 0 {
            if yo >= 1 {
                at.x += DX[branch_direction];
                at.z += DZ[branch_direction];
                at.y = pos.y + yo;
                if tree_free_material(block_id(region, at)) {
                    let _ = set_block(region, at, LOG2, NEW_ACACIA);
                    last_y = at.y;
                }
                branch_steps -= 1;
            }
            yo += 1;
        }
        if last_y > 0 {
            at.y = last_y;
            for dx in -1..=1 {
                for dz in -1..=1 {
                    place_new_leaf(region, at.offset(dx, 1, dz), NEW_ACACIA);
                }
            }
            for dx in -2_i32..=2_i32 {
                for dz in -2_i32..=2_i32 {
                    if dx.abs() == 2 && dz.abs() == 2 {
                        continue;
                    }
                    place_new_leaf(region, at.offset(dx, 0, dz), NEW_ACACIA);
                }
            }
        }
    }
    true
}

fn place_roof_leaf(region: &mut PopulationNeighborhood, pos: BlockPos) {
    // RoofTree uses MaterialType::Vegetable in addition to air. Pre-decoration generated
    // vegetable states are all replaceable plant-like blocks on this path.
    let id = block_id(region, pos);
    if id == AIR
        || matches!(
            id,
            6 | 30
                | 31
                | 32
                | 37
                | 38
                | 39
                | 40
                | 59
                | 83
                | 104
                | 105
                | 106
                | 111
                | 115
                | 127
                | 141
                | 142
                | 175
                | 244
        )
    {
        let _ = set_block(region, pos, LEAVES2, NEW_DARK_OAK);
    }
}

fn place_roofed(region: &mut PopulationNeighborhood, pos: BlockPos, random: &mut MtRandom) -> bool {
    let height = random.next_int(3) as i32 + random.next_int(2) as i32 + 6;
    if !prepare_spawn(region, pos, height) || pos.y >= WORLD_HEIGHT as i32 - height - 1 {
        return false;
    }
    for floor in [
        pos.offset(0, -1, 0),
        pos.offset(1, -1, 0),
        pos.offset(1, -1, 1),
        pos.offset(0, -1, 1),
    ] {
        let _ = set_block(region, floor, DIRT, 0);
    }

    const DX: [i32; 4] = [0, -1, 0, 1];
    const DZ: [i32; 4] = [1, 0, -1, 0];
    let lean_direction = random.next_int(4) as usize;
    let lean_height = height - random.next_int(4) as i32;
    let mut lean_steps = 2 - random.next_int(3) as i32;
    let mut trunk = pos;
    let mut last_y = 0;
    let viney = random.next_int(6) == 0;

    for dy in 0..height {
        let y = pos.y + dy;
        if dy >= lean_height && lean_steps > 0 {
            trunk.x += DX[lean_direction];
            trunk.z += DZ[lean_direction];
            lean_steps -= 1;
        }
        trunk.y = y;
        if tree_free_material(block_id(region, trunk)) {
            for (stem, x_side, x_data, z_side, z_data) in [
                (
                    pos.above(dy),
                    pos.above(dy).offset(-1, 0, 0),
                    VINE_EAST,
                    pos.above(dy).offset(0, 0, -1),
                    VINE_SOUTH,
                ),
                (
                    pos.above(dy).offset(1, 0, 0),
                    pos.above(dy).offset(2, 0, 0),
                    VINE_WEST,
                    pos.above(dy).offset(1, 0, -1),
                    VINE_SOUTH,
                ),
                (
                    pos.above(dy).offset(1, 0, 1),
                    pos.above(dy).offset(2, 0, 1),
                    VINE_WEST,
                    pos.above(dy).offset(1, 0, 2),
                    VINE_NORTH,
                ),
                (
                    pos.above(dy).offset(0, 0, 1),
                    pos.above(dy).offset(-1, 0, 1),
                    VINE_EAST,
                    pos.above(dy).offset(0, 0, 2),
                    VINE_NORTH,
                ),
            ] {
                if tree_free_material(block_id(region, stem)) {
                    let _ = set_block(region, stem, LOG2, NEW_DARK_OAK);
                    if viney {
                        for (side, data) in [(x_side, x_data), (z_side, z_data)] {
                            if random.next_int(7) > 0 && is_empty(region, side) {
                                let _ = set_block(region, side, VINE, data);
                            }
                        }
                    }
                }
            }
            last_y = y;
        }
    }

    trunk.y = last_y;
    for dx in -2_i32..=0_i32 {
        for dz in -2_i32..=0_i32 {
            place_roof_leaf(region, trunk.offset(dx, -1, dz));
            place_roof_leaf(region, trunk.offset(1 - dx, -1, dz));
            place_roof_leaf(region, trunk.offset(dx, -1, 1 - dz));
            place_roof_leaf(region, trunk.offset(1 - dx, -1, 1 - dz));
            if !((dx <= -2 && dz <= -1) || (dx == -1 && dz == -2)) {
                place_roof_leaf(region, trunk.offset(dx, 1, dz));
                place_roof_leaf(region, trunk.offset(1 - dx, 1, dz));
                place_roof_leaf(region, trunk.offset(dx, 1, 1 - dz));
                place_roof_leaf(region, trunk.offset(1 - dx, 1, 1 - dz));
            }
        }
    }
    if random.next_boolean() {
        for (dx, dz) in [(0, 0), (1, 0), (1, 1), (0, 1)] {
            place_roof_leaf(region, trunk.offset(dx, 2, dz));
        }
    }
    for dx in -3_i32..=4_i32 {
        for dz in -3_i32..=4_i32 {
            if (dx == -3 || dx == 4) && (dz == -3 || dz == 4) {
                continue;
            }
            if dx.abs() >= 3 && dz.abs() >= 3 {
                continue;
            }
            place_roof_leaf(region, trunk.offset(dx, 0, dz));
        }
    }
    for dx in -1_i32..=2_i32 {
        for dz in -1_i32..=2_i32 {
            if (0..=1).contains(&dx) && (0..=1).contains(&dz) {
                continue;
            }
            if random.next_int(3) > 0 {
                continue;
            }
            let length = random.next_int(3) as i32 + 2;
            for branch_y in 0..length {
                let _ = set_block(
                    region,
                    BlockPos::new(pos.x + dx, trunk.y - branch_y - 1, pos.z + dz),
                    LOG2,
                    NEW_DARK_OAK,
                );
            }
            for lx in -1..=1 {
                for lz in -1..=1 {
                    place_roof_leaf(region, trunk.offset(dx + lx, 0, dz + lz));
                }
            }
            for lx in -2_i32..=2_i32 {
                for lz in -2_i32..=2_i32 {
                    if lx.abs() == 2 && lz.abs() == 2 {
                        continue;
                    }
                    place_roof_leaf(region, trunk.offset(dx + lx, -1, dz + lz));
                }
            }
        }
    }
    true
}

fn mega_check_free(region: &PopulationNeighborhood, pos: BlockPos, height: i32) -> bool {
    if pos.y < 1 || pos.y + height + 1 > WORLD_HEIGHT as i32 {
        return false;
    }
    for dy in 0..=1 + height {
        let radius = if dy == 0 { 1 } else { 2 };
        for dx in -radius..=radius {
            for dz in -radius..=radius {
                let y = pos.y + dy;
                if !(0..WORLD_HEIGHT as i32).contains(&y)
                    || !tree_free_id(region.block_id(pos.x + dx, y, pos.z + dz))
                {
                    return false;
                }
            }
        }
    }
    true
}

fn mega_prepare(region: &mut PopulationNeighborhood, pos: BlockPos, height: i32) -> bool {
    if !mega_check_free(region, pos, height) {
        return false;
    }
    let below = block_id(region, pos.below(1));
    if !matches!(below, GRASS | DIRT | PODZOL) || pos.y < 2 {
        return false;
    }
    for floor in [
        pos.below(1),
        pos.offset(1, -1, 0),
        pos.offset(0, -1, 1),
        pos.offset(1, -1, 1),
    ] {
        let _ = set_block(region, floor, DIRT, 0);
    }
    true
}

fn mega_leaf_double(
    region: &mut PopulationNeighborhood,
    origin: BlockPos,
    radius: i32,
    leaf_type: u8,
) {
    let r2 = radius * radius;
    for dx in -radius..=radius + 1 {
        for dz in -radius..=radius + 1 {
            let odx = dx - 1;
            let odz = dz - 1;
            if dx * dx + dz * dz > r2
                && odx * odx + odz * odz > r2
                && dx * dx + odz * odz > r2
                && odx * odx + dz * dz > r2
            {
                continue;
            }
            let at = origin.offset(dx, 0, dz);
            let current = region.state(at.x, at.y, at.z).unwrap_or(0);
            let id = current >> 4;
            let data = (current & 0xf) as u8;
            if (id == LEAVES && data == leaf_type) || tree_free_material(id) {
                let _ = set_block(region, at, LEAVES, leaf_type);
            }
        }
    }
}

fn mega_leaf_single(
    region: &mut PopulationNeighborhood,
    origin: BlockPos,
    radius: i32,
    leaf_type: u8,
) {
    let r2 = radius * radius;
    for dx in -radius..=radius {
        for dz in -radius..=radius {
            if dx * dx + dz * dz <= r2 {
                let at = origin.offset(dx, 0, dz);
                if tree_free_material(block_id(region, at)) {
                    let _ = set_block(region, at, LEAVES, leaf_type);
                }
            }
        }
    }
}

fn calc_mega_height(random: &mut MtRandom, base: i32, interval: u32) -> i32 {
    let mut height = random.next_int(3) as i32 + base;
    if interval > 1 {
        height += random.next_int(interval) as i32;
    }
    height
}

fn place_mega_pine(
    region: &mut PopulationNeighborhood,
    pos: BlockPos,
    random: &mut MtRandom,
    spruce: bool,
) -> bool {
    let height = calc_mega_height(random, 13, 15);
    if !mega_prepare(region, pos, height) {
        return false;
    }

    let crown_height = random.next_int(5) as i32 + if spruce { 13 } else { 3 };
    let mut previous_radius = 0;
    for y in pos.y + height - crown_height..=pos.y + height {
        let yo = pos.y + height - y;
        let radius = ((yo as f32 / crown_height as f32) * 3.5).floor() as i32;
        let extra = if yo > 0 && radius == previous_radius && y & 1 == 0 {
            1
        } else {
            0
        };
        mega_leaf_double(
            region,
            BlockPos::new(pos.x, y, pos.z),
            radius + extra,
            OLD_SPRUCE,
        );
        previous_radius = radius;
    }

    for dy in 0..height {
        let at = pos.above(dy);
        if tree_free_material(block_id(region, at)) {
            let _ = set_block(region, at, LOG, OLD_SPRUCE);
        }
        if dy < height - 1 {
            for extra in [(1, 0), (1, 1), (0, 1)] {
                let side = pos.offset(extra.0, dy, extra.1);
                if tree_free_material(block_id(region, side)) {
                    let _ = set_block(region, side, LOG, OLD_SPRUCE);
                }
            }
        }
    }
    true
}

fn mega_jungle_vine(
    region: &mut PopulationNeighborhood,
    pos: BlockPos,
    random: &mut MtRandom,
    data: u8,
) {
    if random.next_int(3) > 0 && is_empty(region, pos) {
        let _ = set_block(region, pos, VINE, data);
    }
}

fn place_mega_jungle(
    region: &mut PopulationNeighborhood,
    pos: BlockPos,
    random: &mut MtRandom,
) -> bool {
    let height = calc_mega_height(random, 10, 20);
    if !mega_prepare(region, pos, height) {
        return false;
    }

    for dy in -2..=0 {
        mega_leaf_double(region, pos.above(height + dy), 3 - dy, OLD_JUNGLE);
    }

    let mut branch_height = pos.y + height - 2 - random.next_int(4) as i32;
    while branch_height > pos.y + height / 2 {
        let angle = random.next_float() * std::f32::consts::PI * 2.0;
        let mut bx = pos.x + (0.5 + angle.cos() * 4.0) as i32;
        let mut bz = pos.z + (0.5 + angle.sin() * 4.0) as i32;
        for step in 0..5 {
            bx = pos.x + (1.5 + angle.cos() * step as f32) as i32;
            bz = pos.z + (1.5 + angle.sin() * step as f32) as i32;
            let _ = set_block(
                region,
                BlockPos::new(bx, branch_height - 3 + step / 2, bz),
                LOG,
                OLD_JUNGLE,
            );
        }
        let leaf_height = 1 + random.next_int(2) as i32;
        for y in branch_height - leaf_height..=branch_height {
            mega_leaf_single(
                region,
                BlockPos::new(bx, y, bz),
                1 - (y - branch_height),
                OLD_JUNGLE,
            );
        }
        branch_height -= 2 + random.next_int(4) as i32;
    }

    for dy in 0..height {
        let stem = pos.above(dy);
        if tree_free_material(block_id(region, stem)) {
            let _ = set_block(region, stem, LOG, OLD_JUNGLE);
            if dy > 0 {
                mega_jungle_vine(region, stem.offset(-1, 0, 0), random, VINE_EAST);
                mega_jungle_vine(region, stem.offset(0, 0, -1), random, VINE_SOUTH);
            }
        }
        if dy < height - 1 {
            for (stem2, xside, xdata, zside, zdata) in [
                (
                    stem.offset(1, 0, 0),
                    stem.offset(2, 0, 0),
                    VINE_WEST,
                    stem.offset(1, 0, -1),
                    VINE_SOUTH,
                ),
                (
                    stem.offset(1, 0, 1),
                    stem.offset(2, 0, 1),
                    VINE_WEST,
                    stem.offset(1, 0, 2),
                    VINE_NORTH,
                ),
                (
                    stem.offset(0, 0, 1),
                    stem.offset(-1, 0, 1),
                    VINE_EAST,
                    stem.offset(0, 0, 2),
                    VINE_NORTH,
                ),
            ] {
                if tree_free_material(block_id(region, stem2)) {
                    let _ = set_block(region, stem2, LOG, OLD_JUNGLE);
                    if dy > 0 {
                        mega_jungle_vine(region, xside, random, xdata);
                        mega_jungle_vine(region, zside, random, zdata);
                    }
                }
            }
        }
    }
    true
}

#[derive(Clone, Copy)]
struct FoliageCoord {
    pos: BlockPos,
    branch_base: i32,
}

fn fancy_steps(delta: BlockPos) -> i32 {
    let ax = delta.x.abs();
    let ay = delta.y.abs();
    let az = delta.z.abs();
    if az > ax && az > ay {
        az
    } else if ay > ax {
        ay
    } else {
        ax
    }
}

fn fancy_check_line(region: &PopulationNeighborhood, start: BlockPos, end: BlockPos) -> i32 {
    let delta = BlockPos::new(end.x - start.x, end.y - start.y, end.z - start.z);
    let steps = fancy_steps(delta);
    if steps == 0 {
        return if tree_free_id(block_id(region, start)) {
            -1
        } else {
            0
        };
    }
    let dx = delta.x as f32 / steps as f32;
    let dy = delta.y as f32 / steps as f32;
    let dz = delta.z as f32 / steps as f32;
    for i in 0..=steps {
        let at = start.offset(
            (0.5 + i as f32 * dx) as i32,
            (0.5 + i as f32 * dy) as i32,
            (0.5 + i as f32 * dz) as i32,
        );
        if !tree_free_id(block_id(region, at)) {
            return i;
        }
    }
    -1
}

fn fancy_limb(region: &mut PopulationNeighborhood, start: BlockPos, end: BlockPos) {
    let delta = BlockPos::new(end.x - start.x, end.y - start.y, end.z - start.z);
    let steps = fancy_steps(delta);
    if steps == 0 {
        let _ = set_block(region, start, LOG, 0);
        return;
    }
    let dx = delta.x as f32 / steps as f32;
    let dy = delta.y as f32 / steps as f32;
    let dz = delta.z as f32 / steps as f32;
    for i in 0..=steps {
        let at = start.offset(
            (0.5 + i as f32 * dx) as i32,
            (0.5 + i as f32 * dy) as i32,
            (0.5 + i as f32 * dz) as i32,
        );
        let xdiff = (at.x - start.x).abs();
        let zdiff = (at.z - start.z).abs();
        let maxdiff = xdiff.max(zdiff);
        // Restored source returns the Direction enum itself rather than the packed block-state
        // value it computes, preserving the fixed-family bug for angled fancy-oak limbs.
        let data = if maxdiff > 0 {
            if xdiff == maxdiff { 1 } else { 2 }
        } else {
            0
        };
        let _ = set_block(region, at, LOG, data);
    }
}

fn fancy_cross_section(region: &mut PopulationNeighborhood, pos: BlockPos, radius: f32) {
    let r = (radius + 0.618) as i32;
    for dx in -r..=r {
        for dz in -r..=r {
            let x = dx.abs() as f32 + 0.5;
            let z = dz.abs() as f32 + 0.5;
            if x * x + z * z <= radius * radius {
                let at = pos.offset(dx, 0, dz);
                if tree_free_material(block_id(region, at)) {
                    let _ = set_block(region, at, LEAVES, OLD_OAK);
                }
            }
        }
    }
}

fn fancy_tree_shape(height: i32, y: i32) -> f32 {
    if (y as f32) < height as f32 * 0.3 {
        return -1.0;
    }
    let radius = height as f32 / 2.0;
    let adjacent = radius - y as f32;
    if adjacent.abs() >= radius {
        return 0.0;
    }
    let distance = if adjacent == 0.0 {
        radius
    } else {
        (radius * radius - adjacent * adjacent).sqrt()
    };
    distance * 0.5
}

fn place_fancy_tree(
    region: &mut PopulationNeighborhood,
    pos: BlockPos,
    random: &mut MtRandom,
) -> bool {
    let mut height = 5 + random.next_int(12) as i32;
    let below = block_id(region, pos.below(1));
    if !matches!(below, DIRT | GRASS | 60) {
        return false;
    }
    let allowed = fancy_check_line(region, pos, pos.above(height - 1));
    if allowed != -1 {
        if allowed < 6 {
            return false;
        }
        height = allowed;
    }

    let mut trunk_height = (height as f32 * 0.618) as i32;
    if trunk_height >= height {
        trunk_height = height - 1;
    }
    let mut clusters_per_y = (1.382 + (height as f32 / 13.0).powi(2)) as i32;
    if clusters_per_y < 1 {
        clusters_per_y = 1;
    }
    let trunk_top = pos.y + trunk_height;
    let mut relative_y = height - 4;
    let mut foliage = vec![FoliageCoord {
        pos: pos.above(relative_y),
        branch_base: trunk_top,
    }];

    while relative_y >= 0 {
        let shape = fancy_tree_shape(height, relative_y);
        if shape >= 0.0 {
            for _ in 0..clusters_per_y {
                let radius = shape * (random.next_float() + 0.328);
                let angle = random.next_float() * 2.0 * std::f32::consts::PI;
                let x = radius * angle.sin() + 0.5;
                let z = radius * angle.cos() + 0.5;
                let start = pos.offset(x as i32, relative_y - 1, z as i32);
                let end = start.above(4);
                if fancy_check_line(region, start, end) == -1 {
                    let dx = pos.x - start.x;
                    let dz = pos.z - start.z;
                    let cluster_height =
                        start.y as f32 - ((dx * dx + dz * dz) as f32).sqrt() * 0.381;
                    let branch_top = if cluster_height > trunk_top as f32 {
                        trunk_top
                    } else {
                        cluster_height as i32
                    };
                    let branch = BlockPos::new(pos.x, branch_top, pos.z);
                    if fancy_check_line(region, branch, start) == -1 {
                        foliage.push(FoliageCoord {
                            pos: start,
                            branch_base: branch.y,
                        });
                    }
                }
            }
        }
        relative_y -= 1;
    }

    for cluster in &foliage {
        for y in 0..4 {
            let radius = if y == 0 || y == 3 { 2.0 } else { 3.0 };
            fancy_cross_section(region, cluster.pos.above(y), radius);
        }
    }
    fancy_limb(region, pos, pos.above(trunk_height));
    for cluster in foliage {
        let local_y = cluster.branch_base - pos.y;
        if local_y as f32 >= height as f32 * 0.2 {
            fancy_limb(
                region,
                BlockPos::new(pos.x, cluster.branch_base, pos.z),
                cluster.pos,
            );
        }
    }
    true
}
