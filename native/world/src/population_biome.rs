use crate::population::{PopulationNeighborhood, population_random, state};
use crate::population_feature::{
    AIR, BROWN_MUSHROOM, BlockPos, DOUBLE_FERN, DOUBLE_GRASS, DOUBLE_PAEONIA, DOUBLE_ROSE,
    DOUBLE_SUNFLOWER, DOUBLE_SYRINGA, EMERALD_ORE, FLOWING_LAVA, FLOWING_WATER, GRAVEL,
    MOSSY_COBBLESTONE, RED_FLOWER, RED_MUSHROOM, SAND, TALL_GRASS_FERN, TALL_GRASS_TALL,
    YELLOW_FLOWER, block_id, is_empty, place_block_blob, place_cactus, place_clay_disk,
    place_dead_bush, place_desert_well, place_double_plant, place_flower_patch,
    place_huge_mushroom, place_ice_patch, place_ice_spike, place_melon, place_pumpkin, place_reeds,
    place_sand_disk, place_spring, place_tall_grass, place_vines, place_waterlily,
};
use crate::population_ore::{OverworldOreDecorator, place_ore_feature};
use crate::population_tree::{TreeKind, place_tree};
use crate::terrain_shape::noise::{MtRandom, PerlinSimplexNoise};

const STONE: u16 = 1;
const MONSTER_EGG: u16 = 97;

#[derive(Debug, Copy, Clone)]
struct DecoratorConfig {
    tree_count: f32,
    flower_count: i32,
    grass_count: i32,
    dead_bush_count: i32,
    mushroom_count: i32,
    reeds_count: i32,
    cactus_count: i32,
    gravel_count: i32,
    sand_count: i32,
    clay_count: i32,
    huge_mushrooms: i32,
    waterlily_count: i32,
}

impl DecoratorConfig {
    const DEFAULT: Self = Self {
        tree_count: 0.0,
        flower_count: 2,
        grass_count: 1,
        dead_bush_count: 0,
        mushroom_count: 0,
        reeds_count: 0,
        cactus_count: 0,
        gravel_count: 1,
        sand_count: 3,
        clay_count: 1,
        huge_mushrooms: 0,
        waterlily_count: 0,
    };
}

/// Exact fixed-target Infinite Overworld biome decoration stage.
///
/// The target reseeds the population MT stream immediately before `Biome::decorate`. This owner
/// therefore runs the already-recovered ore decorator and every remaining 0.15.10 biome feature
/// on one shared stream, preserving target feature order and RNG consumption.
pub struct OverworldBiomeDecorator {
    seed: u32,
}

impl OverworldBiomeDecorator {
    pub fn new(seed: i32) -> Self {
        Self {
            seed: u32::from_ne_bytes(seed.to_ne_bytes()),
        }
    }

    pub fn decorate(&self, neighborhood: &mut PopulationNeighborhood) {
        let center = neighborhood.center();
        let biome_id = neighborhood.center_biome_ids()[15 + 15 * 16];
        let origin = BlockPos::new(center.x().wrapping_mul(16), 0, center.z().wrapping_mul(16));
        let mut random = population_random(self.seed, center);
        let mut info_random = MtRandom::new(2345);
        let biome_info_noise = PerlinSimplexNoise::new(&mut info_random, 1);

        decorate_biome_override_before(
            neighborhood,
            &mut random,
            &biome_info_noise,
            biome_id,
            origin,
        );
        decorate_common(
            neighborhood,
            &mut random,
            &biome_info_noise,
            biome_id,
            origin,
            self.seed,
        );
        decorate_biome_override_after(neighborhood, &mut random, biome_id, origin);
    }
}

fn decorate_common(
    region: &mut PopulationNeighborhood,
    random: &mut MtRandom,
    biome_info_noise: &PerlinSimplexNoise,
    biome_id: u8,
    origin: BlockPos,
    seed: u32,
) {
    let config = decorator_config(biome_id);

    // Mesa's decorator overrides decorateOres, so the shared ore owner must see the same biome.
    OverworldOreDecorator::new(i32::from_ne_bytes(seed.to_ne_bytes()))
        .decorate_with_random(region, random);

    for _ in 0..config.sand_count.max(0) {
        let x = origin.x.wrapping_add(random.next_int(16) as i32);
        let z = origin.z.wrapping_add(random.next_int(16) as i32);
        let y = region.above_top_solid_block(x, z, false);
        let _ = place_sand_disk(region, BlockPos::new(x, y, z), random, SAND, 7);
    }

    for _ in 0..config.clay_count.max(0) {
        let x = origin.x.wrapping_add(random.next_int(16) as i32);
        let z = origin.z.wrapping_add(random.next_int(16) as i32);
        let y = region.above_top_solid_block(x, z, false);
        let _ = place_clay_disk(region, BlockPos::new(x, y, z), random);
    }

    for _ in 0..config.gravel_count.max(0) {
        let x = origin.x.wrapping_add(random.next_int(16) as i32);
        let z = origin.z.wrapping_add(random.next_int(16) as i32);
        let y = region.above_top_solid_block(x, z, false);
        let _ = place_sand_disk(region, BlockPos::new(x, y, z), random, GRAVEL, 6);
    }

    let mut grass_count = config.grass_count;
    let mut forests = 0_i32;
    if config.tree_count > 0.0 {
        if random.next_int(10) == 0 {
            forests = (random.next_float() * config.tree_count) as i32;
            // Target explicitly borrows the Plains decorator's grass count on this branch.
            grass_count = 10;
        } else if config.tree_count >= 1.0 {
            forests = config.tree_count as i32 + (random.next_int(10) as i32 - 7).max(0);
        } else {
            forests = i32::from(random.next_float() < config.tree_count);
        }
    }

    for _ in 0..forests {
        let kind = choose_tree_feature(biome_id, random);
        let pos = random_tree_position(region, origin, random);
        match kind {
            SelectedTree::Tree(kind) => {
                let _ = place_tree(kind, region, pos, random);
            }
            SelectedTree::HugeMushroom => {
                let swamp = local_is_swamp(region, pos);
                let _ = place_huge_mushroom(region, pos, random, None, swamp);
            }
        }
        let _ = place_tall_grass(region, pos, TALL_GRASS_TALL, random, 23, 3);
    }

    for _ in 0..config.huge_mushrooms.max(0) {
        let x = origin.x.wrapping_add(random.next_int(16) as i32);
        let z = origin.z.wrapping_add(random.next_int(16) as i32);
        let y = region.above_top_solid_block(x, z, false);
        let pos = BlockPos::new(x, y, z);
        let swamp = local_is_swamp(region, pos);
        let _ = place_huge_mushroom(region, pos, random, None, swamp);
    }

    for _ in 0..config.flower_count.max(0) {
        let x_off = random.next_int(16) as i32;
        let z_off = random.next_int(16) as i32;
        let height =
            region.generation_height(origin.x.wrapping_add(x_off), origin.z.wrapping_add(z_off));
        let y = random.next_int((height + 32) as u32) as i32;
        let pos = origin.offset(x_off, y, z_off);
        let (flower_id, flower_data) = flower_for_biome(biome_id, pos, random, biome_info_noise);
        // Restored target quirk: flowers only fan out when the selected starting material is
        // non-air, even though FlowerFeature itself then seeks nearby air.
        if block_id(region, pos) != AIR {
            let _ = place_flower_patch(region, pos, flower_id, flower_data, random);
        }
    }

    for _ in 0..grass_count.max(0) {
        let x_off = random.next_int(16) as i32;
        let z_off = random.next_int(16) as i32;
        let y = random_height(region, origin, x_off, z_off, random);
        let data = grass_for_biome(biome_id, random);
        let _ = place_tall_grass(region, origin.offset(x_off, y, z_off), data, random, 90, 8);
    }

    for _ in 0..config.dead_bush_count.max(0) {
        let x_off = random.next_int(16) as i32;
        let z_off = random.next_int(16) as i32;
        let y = random_height(region, origin, x_off, z_off, random);
        let _ = place_dead_bush(region, origin.offset(x_off, y, z_off), random);
    }

    for _ in 0..config.waterlily_count.max(0) {
        let x_off = random.next_int(16) as i32;
        let z_off = random.next_int(16) as i32;
        let y = random_height(region, origin, x_off, z_off, random);
        let mut pos = origin.offset(x_off, y, z_off);
        while pos.y > 0 {
            let below = pos.below(1);
            if !is_empty(region, below) {
                break;
            }
            pos = below;
        }
        let _ = place_waterlily(region, pos, random);
    }

    for _ in 0..config.mushroom_count.max(0) {
        if random.next_int(4) == 0 {
            let x_off = random.next_int(16) as i32;
            let z_off = random.next_int(16) as i32;
            let pos = origin.offset(
                x_off,
                region
                    .generation_height(origin.x.wrapping_add(x_off), origin.z.wrapping_add(z_off)),
                z_off,
            );
            let _ = place_flower_patch(region, pos, BROWN_MUSHROOM, 0, random);
        }
        if random.next_int(8) == 0 {
            let x_off = random.next_int(16) as i32;
            let z_off = random.next_int(16) as i32;
            let y = random_height(region, origin, x_off, z_off, random);
            let _ = place_flower_patch(
                region,
                origin.offset(x_off, y, z_off),
                RED_MUSHROOM,
                0,
                random,
            );
        }
    }

    if random.next_int(4) == 0 {
        let x_off = random.next_int(16) as i32;
        let z_off = random.next_int(16) as i32;
        let y = random_height(region, origin, x_off, z_off, random);
        let _ = place_flower_patch(
            region,
            origin.offset(x_off, y, z_off),
            BROWN_MUSHROOM,
            0,
            random,
        );
    }

    if random.next_int(8) == 0 {
        let x_off = random.next_int(16) as i32;
        let z_off = random.next_int(16) as i32;
        let y = random_height(region, origin, x_off, z_off, random);
        let _ = place_flower_patch(
            region,
            origin.offset(x_off, y, z_off),
            RED_MUSHROOM,
            0,
            random,
        );
    }

    for _ in 0..config.reeds_count.max(0) {
        place_random_reeds(region, origin, random);
    }
    for _ in 0..10 {
        place_random_reeds(region, origin, random);
    }

    if random.next_int(32) == 0 {
        let pos = random_surface_position(region, origin, random);
        let _ = place_pumpkin(region, pos, random);
    }

    for _ in 0..config.cactus_count.max(0) {
        let x_off = random.next_int(16) as i32;
        let z_off = random.next_int(16) as i32;
        let y = random_height(region, origin, x_off, z_off, random);
        let _ = place_cactus(region, origin.offset(x_off, y, z_off), random);
    }

    // Infinite passes FLT_MAX as the decorator limit, so the target "less springs in the end"
    // distance gate can never disable liquids here.
    for _ in 0..50 {
        let z = random.next_int(16) as i32;
        let y_param = random.next_int(120) as i32 + 8;
        let y = random.next_int(y_param as u32) as i32;
        let x = random.next_int(16) as i32;
        let _ = place_spring(region, origin.offset(x, y, z), FLOWING_WATER);
    }
    for _ in 0..20 {
        let z = random.next_int(16) as i32;
        let y_param_1 = random.next_int(112) as i32 + 8;
        let y_param_2 = random.next_int(y_param_1 as u32) as i32 + 8;
        let y = random.next_int(y_param_2 as u32) as i32;
        let x = random.next_int(16) as i32;
        let _ = place_spring(region, origin.offset(x, y, z), FLOWING_LAVA);
    }
}

fn decorate_biome_override_before(
    region: &mut PopulationNeighborhood,
    random: &mut MtRandom,
    noise: &PerlinSimplexNoise,
    biome_id: u8,
    origin: BlockPos,
) {
    match biome_id {
        1 | 129 => decorate_plains_before(region, random, noise, biome_id, origin),
        4 | 18 | 27 | 28 | 29 | 132 => decorate_forest_before(region, random, biome_id, origin),
        5 | 19 | 30 | 31 | 32 | 33 | 160 => {
            decorate_taiga_before(region, random, biome_id, origin);
        }
        35 | 36 => {
            let x = random.next_int(16) as i32;
            let z = random.next_int(16) as i32;
            let pos = origin.offset(x, region.generation_height(origin.x + x, origin.z + z), z);
            let _ = place_double_plant(region, pos, DOUBLE_GRASS, random);
        }
        140 => {
            for _ in 0..3 {
                let x = origin.x.wrapping_add(random.next_int(16) as i32);
                let z = origin.z.wrapping_add(random.next_int(16) as i32);
                let y = region.generation_height(x, z);
                let _ = place_ice_spike(region, BlockPos::new(x, y, z), random);
            }
            for _ in 0..2 {
                let x = origin.x.wrapping_add(random.next_int(16) as i32);
                let z = origin.z.wrapping_add(random.next_int(16) as i32);
                let y = region.generation_height(x, z);
                let _ = place_ice_patch(region, BlockPos::new(x, y, z), random);
            }
        }
        _ => {}
    }
}

fn decorate_biome_override_after(
    region: &mut PopulationNeighborhood,
    random: &mut MtRandom,
    biome_id: u8,
    origin: BlockPos,
) {
    match biome_id {
        2 | 17 => {
            if random.next_int(500) == 0 {
                let z = random.next_int(16) as i32;
                let x = random.next_int(16) as i32;
                // Target computes a local-coordinate height and then ignores it, placing from
                // origin + (x, 128, z). Preserve the observable placement and RNG behavior.
                let _ = place_desert_well(region, origin.offset(x, 128, z));
            }
        }
        3 | 20 | 34 => decorate_extreme_hills_after(region, random, origin),
        21..=23 => decorate_jungle_after(region, random, origin),
        _ => {}
    }
}

fn decorate_plains_before(
    region: &mut PopulationNeighborhood,
    random: &mut MtRandom,
    noise: &PerlinSimplexNoise,
    biome_id: u8,
    origin: BlockPos,
) {
    let flower_noise = noise.value(
        origin.x.wrapping_add(8) as f32 / 200.0,
        origin.z.wrapping_add(8) as f32 / 200.0,
    );
    if flower_noise >= -0.8 {
        for _ in 0..7 {
            let x = random.next_int(16) as i32;
            let z = random.next_int(16) as i32;
            let height = region.generation_height(origin.x + x, origin.z + z);
            let y = random.next_int((height + 32) as u32) as i32;
            let _ = place_double_plant(region, origin.offset(x, y, z), DOUBLE_GRASS, random);
        }
    }
    if biome_id == 129 {
        for _ in 0..10 {
            let x = random.next_int(16) as i32;
            let z = random.next_int(16) as i32;
            let height = region.generation_height(origin.x + x, origin.z + z);
            let y = random.next_int((height + 32) as u32) as i32;
            let _ = place_double_plant(region, origin.offset(x, y, z), DOUBLE_SUNFLOWER, random);
        }
    }
}

fn decorate_forest_before(
    region: &mut PopulationNeighborhood,
    random: &mut MtRandom,
    biome_id: u8,
    origin: BlockPos,
) {
    if biome_id == 29 {
        for bx in 0..4 {
            for bz in 0..4 {
                let x = bx * 4 + 1 + random.next_int(3) as i32;
                let z = bz * 4 + 1 + random.next_int(3) as i32;
                let pos = origin.offset(x, region.generation_height(origin.x + x, origin.z + z), z);
                if random.next_int(20) == 0 {
                    let swamp = local_is_swamp(region, pos);
                    let _ = place_huge_mushroom(region, pos, random, None, swamp);
                } else {
                    match choose_tree_feature(biome_id, random) {
                        SelectedTree::Tree(kind) => {
                            let _ = place_tree(kind, region, pos, random);
                        }
                        SelectedTree::HugeMushroom => unreachable!(),
                    }
                    let _ = place_tall_grass(region, pos, TALL_GRASS_TALL, random, 20, 3);
                }
            }
        }
    }

    let mut double_plant_count = random.next_int(5) as i32 - 3;
    if biome_id == 132 {
        double_plant_count += 2;
    }
    if double_plant_count <= 0 {
        return;
    }

    for _ in 0..double_plant_count {
        let select = random.next_int(3);
        let plant = match select {
            0 => DOUBLE_SYRINGA,
            1 => DOUBLE_ROSE,
            _ => DOUBLE_PAEONIA,
        };
        for _ in 0..5 {
            let x = random.next_int(16) as i32;
            let z = random.next_int(16) as i32;
            let height = region.generation_height(origin.x + x, origin.z + z);
            let y = random.next_int((height + 32) as u32) as i32;
            if place_double_plant(region, origin.offset(x, y, z), plant, random) {
                break;
            }
        }
    }
}

fn decorate_taiga_before(
    region: &mut PopulationNeighborhood,
    random: &mut MtRandom,
    biome_id: u8,
    origin: BlockPos,
) {
    if matches!(biome_id, 32 | 33 | 160) {
        let stone_count = random.next_int(3);
        for _ in 0..stone_count {
            // Target source swaps origin X/Z while constructing this position.
            let rand_z = origin.x.wrapping_add(random.next_int(16) as i32);
            let rand_x = origin.z.wrapping_add(random.next_int(16) as i32);
            let pos = BlockPos::new(rand_x, region.generation_height(rand_x, rand_z), rand_z);
            let _ = place_block_blob(region, pos, random, MOSSY_COBBLESTONE, 0);
        }
    }

    // Same X/Z swap exists in the unconditional Taiga double-fern attempt.
    let rand_z = origin.x.wrapping_add(random.next_int(16) as i32);
    let rand_x = origin.z.wrapping_add(random.next_int(16) as i32);
    let pos = BlockPos::new(rand_x, region.generation_height(rand_x, rand_z), rand_z);
    let _ = place_double_plant(region, pos, DOUBLE_FERN, random);
}

fn decorate_jungle_after(
    region: &mut PopulationNeighborhood,
    random: &mut MtRandom,
    origin: BlockPos,
) {
    let x = random.next_int(16) as i32;
    let z = random.next_int(16) as i32;
    let height = region.generation_height(origin.x + x, origin.z + z) * 2;
    let y = if height != 0 {
        random.next_int(height as u32) as i32
    } else {
        0
    };
    let _ = place_melon(region, origin.offset(x, y, z), random);

    for _ in 0..50 {
        let x = random.next_int(16) as i32 + 8;
        let z = random.next_int(16) as i32 + 8;
        let _ = place_vines(region, origin.offset(x, 64, z), random);
    }
}

fn decorate_extreme_hills_after(
    region: &mut PopulationNeighborhood,
    random: &mut MtRandom,
    origin: BlockPos,
) {
    let emerald_count = 3 + random.next_int(6);
    for _ in 0..emerald_count {
        let x = random.next_int(16) as i32;
        let y = random.next_int(28) as i32 + 4;
        let z = random.next_int(16) as i32;
        let pos = origin.offset(x, y, z);
        if block_id(region, pos) == STONE {
            let _ = region.set_state(pos.x, pos.y, pos.z, state(EMERALD_ORE, 0));
        }
    }

    for _ in 0..7 {
        let x = random.next_int(16) as i32;
        let y = random.next_int(32) as i32;
        let z = random.next_int(16) as i32;
        place_ore_feature(
            region,
            random,
            (origin.x.wrapping_add(x), y, origin.z.wrapping_add(z)),
            MONSTER_EGG,
            0,
            8,
        );
    }
}

fn place_random_reeds(
    region: &mut PopulationNeighborhood,
    origin: BlockPos,
    random: &mut MtRandom,
) {
    let x = random.next_int(16) as i32;
    let z = random.next_int(16) as i32;
    let y = random_height(region, origin, x, z, random);
    let _ = place_reeds(region, origin.offset(x, y, z), random);
}

fn random_height(
    region: &PopulationNeighborhood,
    origin: BlockPos,
    x_off: i32,
    z_off: i32,
    random: &mut MtRandom,
) -> i32 {
    let height = region
        .generation_height(origin.x.wrapping_add(x_off), origin.z.wrapping_add(z_off))
        .wrapping_mul(2);
    if height != 0 {
        random.next_int(height as u32) as i32
    } else {
        0
    }
}

fn random_tree_position(
    region: &PopulationNeighborhood,
    origin: BlockPos,
    random: &mut MtRandom,
) -> BlockPos {
    // Target helper draws Z first, then X.
    let z = origin.z.wrapping_add(1 + random.next_int(14) as i32);
    let x = origin.x.wrapping_add(1 + random.next_int(14) as i32);
    BlockPos::new(x, region.generation_height(x, z), z)
}

fn random_surface_position(
    region: &PopulationNeighborhood,
    origin: BlockPos,
    random: &mut MtRandom,
) -> BlockPos {
    // Target helper also draws Z before X here.
    let z = origin.z.wrapping_add(random.next_int(16) as i32);
    let x = origin.x.wrapping_add(random.next_int(16) as i32);
    let height = region.generation_height(x, z);
    let y = if height != 0 {
        random.next_int(height as u32) as i32
    } else {
        0
    };
    BlockPos::new(x, y, z)
}

fn flower_for_biome(
    biome_id: u8,
    pos: BlockPos,
    random: &mut MtRandom,
    noise: &PerlinSimplexNoise,
) -> (u16, u8) {
    match flower_biome_kind(biome_id) {
        FlowerBiome::Plains => {
            let flower_noise = noise.value(pos.x as f32 / 200.0, pos.z as f32 / 200.0);
            if flower_noise < -0.8 {
                (RED_FLOWER, 4 + random.next_int(4) as u8)
            } else if random.next_int(3) > 0 {
                let data = match random.next_int(3) {
                    0 => 0,
                    1 => 3,
                    _ => 8,
                };
                (RED_FLOWER, data)
            } else {
                (YELLOW_FLOWER, 0)
            }
        }
        FlowerBiome::FlowerForest => {
            let placement = ((1.0 + noise.value(pos.x as f32 / 48.0, pos.z as f32 / 48.0)) / 2.0)
                .clamp(0.0, 0.9999);
            let mut data = (placement * 9.0) as u8;
            if data == 1 {
                data = 0;
            }
            (RED_FLOWER, data)
        }
        FlowerBiome::Swamp => (RED_FLOWER, 1),
        FlowerBiome::Default => (YELLOW_FLOWER, 0),
    }
}

fn grass_for_biome(biome_id: u8, random: &mut MtRandom) -> u8 {
    match tree_source_biome(biome_id) {
        5 | 19 | 30 | 31 | 32 | 33 => {
            if random.next_int(5) > 0 {
                TALL_GRASS_FERN
            } else {
                TALL_GRASS_TALL
            }
        }
        21..=23 => {
            if random.next_int(4) == 0 {
                TALL_GRASS_FERN
            } else {
                TALL_GRASS_TALL
            }
        }
        _ => TALL_GRASS_TALL,
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
enum SelectedTree {
    Tree(TreeKind),
    HugeMushroom,
}

fn choose_tree_feature(biome_id: u8, random: &mut MtRandom) -> SelectedTree {
    if matches!(biome_id, 155 | 156) {
        return SelectedTree::Tree(if random.next_boolean() {
            TreeKind::SuperBirch
        } else {
            TreeKind::Birch
        });
    }
    if biome_id == 160 {
        if random.next_int(3) == 0 {
            return SelectedTree::Tree(TreeKind::MegaSpruce);
        }
        return SelectedTree::Tree(if random.next_int(3) == 0 {
            TreeKind::Pine
        } else {
            TreeKind::Spruce
        });
    }

    match tree_source_biome(biome_id) {
        1 => SelectedTree::Tree(if random.next_int(3) == 0 {
            TreeKind::FancyOak
        } else {
            TreeKind::Oak
        }),
        4 | 18 | 132 => {
            if random.next_int(5) == 0 {
                SelectedTree::Tree(TreeKind::Birch)
            } else {
                default_tree(random)
            }
        }
        27 | 28 => SelectedTree::Tree(TreeKind::Birch),
        29 => {
            if random.next_int(3) == 0 {
                SelectedTree::Tree(TreeKind::RoofedOak)
            } else if random.next_int(5) == 0 {
                SelectedTree::Tree(TreeKind::Birch)
            } else {
                default_tree(random)
            }
        }
        5 | 19 | 30 | 31 => SelectedTree::Tree(if random.next_int(3) == 0 {
            TreeKind::Pine
        } else {
            TreeKind::Spruce
        }),
        32 | 33 => {
            if random.next_int(3) == 0 {
                if random.next_int(13) == 0 {
                    SelectedTree::Tree(TreeKind::MegaSpruce)
                } else {
                    SelectedTree::Tree(TreeKind::MegaPine)
                }
            } else if random.next_int(3) == 0 {
                SelectedTree::Tree(TreeKind::Pine)
            } else {
                SelectedTree::Tree(TreeKind::Spruce)
            }
        }
        6 => {
            if random.next_int(20) == 0 {
                SelectedTree::HugeMushroom
            } else {
                SelectedTree::Tree(TreeKind::Swamp)
            }
        }
        12 | 13 | 140 => SelectedTree::Tree(TreeKind::Spruce),
        21 | 22 => {
            if random.next_int(10) == 0 {
                SelectedTree::Tree(TreeKind::FancyOak)
            } else if random.next_int(2) == 0 {
                SelectedTree::Tree(TreeKind::JungleBush)
            } else if random.next_int(10) == 0 {
                SelectedTree::Tree(TreeKind::MegaJungle)
            } else {
                SelectedTree::Tree(TreeKind::Jungle)
            }
        }
        23 => {
            if random.next_int(10) == 0 {
                SelectedTree::Tree(TreeKind::FancyOak)
            } else if random.next_int(2) == 0 {
                SelectedTree::Tree(TreeKind::JungleBush)
            } else {
                SelectedTree::Tree(TreeKind::Jungle)
            }
        }
        3 | 20 | 34 => {
            if random.next_int(3) > 0 {
                SelectedTree::Tree(TreeKind::Spruce)
            } else {
                default_tree(random)
            }
        }
        35 | 36 => SelectedTree::Tree(if random.next_int(5) > 0 {
            TreeKind::Savanna
        } else {
            TreeKind::Oak
        }),
        37..=39 => SelectedTree::Tree(TreeKind::Oak),
        _ => default_tree(random),
    }
}

fn default_tree(random: &mut MtRandom) -> SelectedTree {
    SelectedTree::Tree(if random.next_int(10) == 0 {
        TreeKind::FancyOak
    } else {
        TreeKind::Oak
    })
}

fn local_is_swamp(region: &PopulationNeighborhood, pos: BlockPos) -> bool {
    matches!(region.biome_id(pos.x, pos.z), Some(6 | 134))
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
enum FlowerBiome {
    Default,
    Plains,
    FlowerForest,
    Swamp,
}

const fn flower_biome_kind(id: u8) -> FlowerBiome {
    match id {
        1 | 129 => FlowerBiome::Plains,
        132 => FlowerBiome::FlowerForest,
        6 | 134 => FlowerBiome::Swamp,
        _ => FlowerBiome::Default,
    }
}

const fn tree_source_biome(id: u8) -> u8 {
    match id {
        129 => 1,
        130 => 2,
        131 => 3,
        133 => 5,
        134 => 6,
        149 => 21,
        151 => 23,
        155 => 27,
        156 => 28,
        157 => 29,
        158 => 30,
        161 => 33,
        162 => 34,
        163 => 35,
        164 => 36,
        165 => 37,
        166 => 38,
        167 => 39,
        _ => id,
    }
}

const fn decorator_config(id: u8) -> DecoratorConfig {
    let source = tree_source_biome(id);
    match source {
        1 | 129 => DecoratorConfig {
            tree_count: 0.05,
            flower_count: 4,
            grass_count: 10,
            ..DecoratorConfig::DEFAULT
        },
        2 | 17 | 130 => DecoratorConfig {
            tree_count: -999.0,
            dead_bush_count: 2,
            reeds_count: 50,
            cactus_count: 10,
            ..DecoratorConfig::DEFAULT
        },
        4 | 18 | 27 | 28 => DecoratorConfig {
            tree_count: 10.0,
            grass_count: 0,
            ..DecoratorConfig::DEFAULT
        },
        132 => DecoratorConfig {
            tree_count: 1.0,
            flower_count: 100,
            grass_count: 1,
            ..DecoratorConfig::DEFAULT
        },
        29 => DecoratorConfig {
            tree_count: -999.0,
            grass_count: 0,
            ..DecoratorConfig::DEFAULT
        },
        5 | 19 | 30 | 31 => DecoratorConfig {
            tree_count: 10.0,
            grass_count: 1,
            mushroom_count: 1,
            ..DecoratorConfig::DEFAULT
        },
        32 | 33 | 160 => DecoratorConfig {
            tree_count: 10.0,
            grass_count: 7,
            dead_bush_count: 1,
            mushroom_count: 3,
            ..DecoratorConfig::DEFAULT
        },
        6 => DecoratorConfig {
            tree_count: 2.0,
            flower_count: 1,
            dead_bush_count: 1,
            mushroom_count: 8,
            reeds_count: 10,
            sand_count: 0,
            gravel_count: 0,
            clay_count: 1,
            waterlily_count: 4,
            grass_count: 5,
            ..DecoratorConfig::DEFAULT
        },
        12 | 13 => DecoratorConfig {
            tree_count: 0.01,
            ..DecoratorConfig::DEFAULT
        },
        140 => DecoratorConfig::DEFAULT,
        14 | 15 => DecoratorConfig {
            tree_count: -100.0,
            flower_count: -100,
            grass_count: -100,
            mushroom_count: 1,
            huge_mushrooms: 1,
            ..DecoratorConfig::DEFAULT
        },
        16 | 25 | 26 => DecoratorConfig {
            tree_count: -999.0,
            ..DecoratorConfig::DEFAULT
        },
        20 | 34 => DecoratorConfig {
            tree_count: 3.0,
            ..DecoratorConfig::DEFAULT
        },
        21 | 22 => DecoratorConfig {
            tree_count: 25.0,
            flower_count: 4,
            grass_count: 25,
            ..DecoratorConfig::DEFAULT
        },
        23 => DecoratorConfig {
            tree_count: 2.0,
            flower_count: 4,
            grass_count: 25,
            ..DecoratorConfig::DEFAULT
        },
        35 | 36 => DecoratorConfig {
            tree_count: 1.0,
            flower_count: 4,
            grass_count: 20,
            ..DecoratorConfig::DEFAULT
        },
        37 | 39 => DecoratorConfig {
            tree_count: -999.0,
            flower_count: 0,
            dead_bush_count: 20,
            reeds_count: 3,
            cactus_count: 5,
            ..DecoratorConfig::DEFAULT
        },
        38 => DecoratorConfig {
            tree_count: 5.0,
            flower_count: 0,
            dead_bush_count: 20,
            reeds_count: 3,
            cactus_count: 5,
            ..DecoratorConfig::DEFAULT
        },
        _ => DecoratorConfig::DEFAULT,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::population_feature::{CACTUS, DEAD_BUSH, SANDSTONE, STILL_WATER, STONE_SLAB};

    #[test]
    fn mutated_biomes_preserve_target_decorator_ownership_quirks() {
        assert_eq!(tree_source_biome(129), 1);
        assert_eq!(tree_source_biome(149), 21);
        assert_eq!(tree_source_biome(155), 27);
        assert_eq!(tree_source_biome(156), 28);
        assert_eq!(tree_source_biome(157), 29);
        assert_eq!(tree_source_biome(161), 33);
        assert_eq!(tree_source_biome(163), 35);

        assert_eq!(decorator_config(163).tree_count, 1.0);
        assert_eq!(decorator_config(163).flower_count, 4);
        assert_eq!(decorator_config(163).grass_count, 20);
        assert_eq!(decorator_config(160).grass_count, 7);
        assert_eq!(decorator_config(140).tree_count, 0.0);
    }

    #[test]
    fn fixed_biome_decorator_counts_match_recovered_target_table() {
        let swamp = decorator_config(6);
        assert_eq!(
            (swamp.tree_count, swamp.flower_count, swamp.grass_count),
            (2.0, 1, 5)
        );
        assert_eq!((swamp.dead_bush_count, swamp.mushroom_count), (1, 8));
        assert_eq!((swamp.reeds_count, swamp.waterlily_count), (10, 4));
        assert_eq!(
            (swamp.sand_count, swamp.gravel_count, swamp.clay_count),
            (0, 0, 1)
        );

        let mesa_forest = decorator_config(38);
        assert_eq!(mesa_forest.tree_count, 5.0);
        assert_eq!(mesa_forest.flower_count, 0);
        assert_eq!(
            (
                mesa_forest.dead_bush_count,
                mesa_forest.reeds_count,
                mesa_forest.cactus_count
            ),
            (20, 3, 5)
        );
    }

    #[test]
    fn target_biome_info_noise_is_world_seed_independent() {
        let mut first_random = MtRandom::new(2345);
        let first = PerlinSimplexNoise::new(&mut first_random, 1);
        let mut second_random = MtRandom::new(2345);
        let second = PerlinSimplexNoise::new(&mut second_random, 1);

        for (x, z) in [(-200.0, -200.0), (0.0, 0.0), (48.0, 96.0), (200.0, -400.0)] {
            assert_eq!(first.value(x, z).to_bits(), second.value(x, z).to_bits());
        }
    }

    fn hash_neighborhood(neighborhood: &PopulationNeighborhood) -> u64 {
        neighborhood
            .state_planes()
            .flat_map(|states| states.iter())
            .fold(0xcbf2_9ce4_8422_2325_u64, |mut hash, value| {
                for byte in value.to_le_bytes() {
                    hash = (hash ^ u64::from(byte)).wrapping_mul(0x100_0000_01b3);
                }
                hash
            })
    }

    fn block_count(neighborhood: &PopulationNeighborhood, id: u16) -> usize {
        neighborhood
            .state_planes()
            .flat_map(|states| states.iter())
            .filter(|value| (**value >> 4) == id)
            .count()
    }

    #[test]
    fn independent_cpp_common_ocean_fixture_matches() {
        // Independent standalone C++ oracle translated directly from the recovered 0.15.10
        // Random/BiomeDecorator/Feature routines. The fixture deliberately contains no stone so
        // ore geometry mutates nothing while still consuming the full target ore RNG stream.
        let center = crate::ChunkCoord::new(0, 0);
        let mut neighborhood = PopulationNeighborhood::filled(center, state(AIR, 0), 0);
        for z in -16..32 {
            for x in -16..32 {
                assert!(neighborhood.set_state(x, 62, z, state(3, 0)));
                assert!(neighborhood.set_state(x, 63, z, state(2, 0)));
                neighborhood.set_generation_height_for_test(x, z, 64);
            }
        }

        OverworldBiomeDecorator::new(36).decorate(&mut neighborhood);

        assert_eq!(hash_neighborhood(&neighborhood), 0x20ef_402f_27d4_95ff);
        assert_eq!(block_count(&neighborhood, YELLOW_FLOWER), 7);
    }

    #[test]
    fn independent_cpp_desert_well_fixture_matches() {
        // Standalone C++ oracle: full common RNG stream on a synthetic sand plateau followed by
        // DesertBiome's 1/500 well override. Seed 2471 also exercises dead-bush and cactus writes.
        let center = crate::ChunkCoord::new(0, 0);
        let mut neighborhood = PopulationNeighborhood::filled(center, state(AIR, 0), 2);
        for z in -16..32 {
            for x in -16..32 {
                assert!(neighborhood.set_state(x, 62, z, state(SAND, 0)));
                assert!(neighborhood.set_state(x, 63, z, state(SAND, 0)));
                neighborhood.set_generation_height_for_test(x, z, 64);
            }
        }

        OverworldBiomeDecorator::new(2471).decorate(&mut neighborhood);

        assert_eq!(hash_neighborhood(&neighborhood), 0xe4bd_f751_cb87_6a87);
        assert_eq!(block_count(&neighborhood, DEAD_BUSH), 1);
        assert_eq!(block_count(&neighborhood, CACTUS), 4);
        assert_eq!(block_count(&neighborhood, SANDSTONE), 70);
        assert_eq!(block_count(&neighborhood, STONE_SLAB), 12);
        assert_eq!(block_count(&neighborhood, STILL_WATER), 5);
    }
}
