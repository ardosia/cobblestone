use crate::ChunkCoord;
use crate::chunk::WORLD_HEIGHT;
use crate::population::{PopulationNeighborhood, state};
use crate::terrain_shape::noise::MtRandom;

use super::StructureBounds;
use super::scattered::{ScatteredKind, ScatteredPieceState, ScatteredPlan};

const AIR: u16 = 0;
const GRASS: u16 = 2;
const DIRT: u16 = 3;
const COBBLESTONE: u16 = 4;
const PLANKS: u16 = 5;
const LOG: u16 = 17;
const DISPENSER: u16 = 23;
const SANDSTONE: u16 = 24;
const STICKY_PISTON: u16 = 29;
const STONE_SLAB: u16 = 44;
const TNT: u16 = 46;
const MOSSY_COBBLESTONE: u16 = 48;
const STONE_BRICK: u16 = 98;
const REDSTONE_WIRE: u16 = 55;
const CRAFTING_TABLE: u16 = 58;
const STONE_STAIRS: u16 = 67;
const LEVER: u16 = 69;
const STONE_PRESSURE_PLATE: u16 = 70;
const FENCE: u16 = 85;
const UNPOWERED_REPEATER: u16 = 93;
const VINE: u16 = 106;
const SANDSTONE_STAIRS: u16 = 128;
const TRIPWIRE_HOOK: u16 = 131;
const TRIPWIRE: u16 = 132;
const SPRUCE_STAIRS: u16 = 134;
const FLOWER_POT: u16 = 140;
const STAINED_HARDENED_CLAY: u16 = 159;

const SANDSTONE_HIEROGLYPHS: u8 = 1;
const SANDSTONE_SMOOTH: u8 = 2;
const SANDSTONE_SLAB: u8 = 1;
const SPRUCE_WOOD: u8 = 1;
const VINE_ALL: u8 = 0x0f;
const TRIPWIRE_ATTACHED: u8 = 0x04;

pub(crate) struct ScatteredPostProcessor;

impl ScatteredPostProcessor {
    pub(crate) fn process_start(
        plan: &mut ScatteredPlan,
        neighborhood: &mut PopulationNeighborhood,
        target: ChunkCoord,
        random: &mut MtRandom,
    ) -> bool {
        let chunk_box = chunk_bounds(target);
        if !plan
            .core
            .bounds
            .intersects_xz(chunk_box.x0, chunk_box.z0, chunk_box.x1, chunk_box.z1)
            || !plan.core.should_post_process(target)
        {
            return false;
        }

        let Some(piece) = plan.piece.as_mut() else {
            plan.core.mark_post_processed(target);
            return false;
        };

        let source = plan.core.source();
        let west = source.x().wrapping_mul(16);
        let north = source.z().wrapping_mul(16);

        let ok = match piece.kind {
            ScatteredKind::DesertPyramid => {
                desert_pyramid(piece, neighborhood, chunk_box, west, 64, north);
                true
            }
            ScatteredKind::JunglePyramid => {
                if !update_average_ground_height(
                    piece,
                    neighborhood,
                    chunk_box,
                    west,
                    north,
                    12,
                    15,
                ) {
                    false
                } else {
                    jungle_pyramid(
                        piece,
                        neighborhood,
                        chunk_box,
                        random,
                        west,
                        piece.height_position,
                        north,
                    );
                    true
                }
            }
            ScatteredKind::SwamplandHut => {
                if !update_average_ground_height(piece, neighborhood, chunk_box, west, north, 7, 9)
                {
                    false
                } else {
                    swampland_hut(
                        neighborhood,
                        chunk_box,
                        west,
                        piece.height_position + 1,
                        north,
                    );
                    true
                }
            }
        };

        if !ok {
            plan.piece = None;
        }
        plan.core.mark_post_processed(target);
        ok
    }
}

fn update_average_ground_height(
    piece: &mut ScatteredPieceState,
    neighborhood: &PopulationNeighborhood,
    chunk_box: StructureBounds,
    west: i32,
    north: i32,
    width: i32,
    depth: i32,
) -> bool {
    if piece.height_position >= 0 {
        return true;
    }

    let mut total = 0_i32;
    let mut count = 0_i32;
    for z in north..north + depth {
        for x in west..west + width {
            if x < chunk_box.x0 || x > chunk_box.x1 || z < chunk_box.z0 || z > chunk_box.z1 {
                continue;
            }
            total += neighborhood.above_top_solid_block(x, z, true);
            count += 1;
        }
    }

    if count == 0 {
        return false;
    }
    piece.height_position = total / count;
    true
}

fn desert_pyramid(
    piece: &mut ScatteredPieceState,
    neighborhood: &mut PopulationNeighborhood,
    chunk_box: StructureBounds,
    west: i32,
    base_y: i32,
    north: i32,
) {
    let mut w = Writer::new(neighborhood, chunk_box, west, base_y, north);

    w.box_fill(0, -4, 0, 20, 0, 20, (SANDSTONE, 0), (SANDSTONE, 0), false);
    for pos in 1..=9 {
        w.box_fill(
            pos,
            pos,
            pos,
            20 - pos,
            pos,
            20 - pos,
            (SANDSTONE, 0),
            (SANDSTONE, 0),
            false,
        );
        w.box_fill(
            pos + 1,
            pos,
            pos + 1,
            19 - pos,
            pos,
            19 - pos,
            (AIR, 0),
            (AIR, 0),
            false,
        );
    }
    for x in 0..21 {
        for z in 0..21 {
            w.fill_column_down(SANDSTONE, 0, x, -5, z);
        }
    }

    let stairs_north = 2;
    let stairs_south = 3;
    let stairs_east = 0;
    let stairs_west = 1;
    let base_deco = 0;
    let blue = 0;

    w.box_fill(0, 0, 0, 4, 9, 4, (SANDSTONE, 0), (AIR, 0), false);
    w.box_fill(1, 10, 1, 3, 10, 3, (SANDSTONE, 0), (SANDSTONE, 0), false);
    w.place(SANDSTONE_STAIRS, stairs_north, 2, 10, 0);
    w.place(SANDSTONE_STAIRS, stairs_south, 2, 10, 4);
    w.place(SANDSTONE_STAIRS, stairs_east, 0, 10, 2);
    w.place(SANDSTONE_STAIRS, stairs_west, 4, 10, 2);

    w.box_fill(16, 0, 0, 20, 9, 4, (SANDSTONE, 0), (AIR, 0), false);
    w.box_fill(17, 10, 1, 19, 10, 3, (SANDSTONE, 0), (SANDSTONE, 0), false);
    w.place(SANDSTONE_STAIRS, stairs_north, 18, 10, 0);
    w.place(SANDSTONE_STAIRS, stairs_south, 18, 10, 4);
    w.place(SANDSTONE_STAIRS, stairs_east, 16, 10, 2);
    w.place(SANDSTONE_STAIRS, stairs_west, 20, 10, 2);

    w.box_fill(8, 0, 0, 12, 4, 4, (SANDSTONE, 0), (AIR, 0), false);
    w.box_fill(9, 1, 0, 11, 3, 4, (AIR, 0), (AIR, 0), false);
    for (x, y) in [(9, 1), (9, 2), (9, 3), (10, 3), (11, 3), (11, 2), (11, 1)] {
        w.place(SANDSTONE, SANDSTONE_SMOOTH, x, y, 1);
    }

    w.box_fill(4, 1, 1, 8, 3, 3, (SANDSTONE, 0), (AIR, 0), false);
    w.box_fill(4, 1, 2, 8, 2, 2, (AIR, 0), (AIR, 0), false);
    w.box_fill(12, 1, 1, 16, 3, 3, (SANDSTONE, 0), (AIR, 0), false);
    w.box_fill(12, 1, 2, 16, 2, 2, (AIR, 0), (AIR, 0), false);

    w.box_fill(5, 4, 5, 15, 4, 15, (SANDSTONE, 0), (SANDSTONE, 0), false);
    w.box_fill(9, 4, 9, 11, 4, 11, (AIR, 0), (AIR, 0), false);
    for (x, z) in [(8, 8), (12, 8), (8, 12), (12, 12)] {
        w.box_fill(
            x,
            1,
            z,
            x,
            3,
            z,
            (SANDSTONE, SANDSTONE_SMOOTH),
            (SANDSTONE, 0),
            false,
        );
    }

    w.box_fill(1, 1, 5, 4, 4, 11, (SANDSTONE, 0), (SANDSTONE, 0), false);
    w.box_fill(16, 1, 5, 19, 4, 11, (SANDSTONE, 0), (SANDSTONE, 0), false);
    w.box_fill(6, 7, 9, 6, 7, 11, (SANDSTONE, 0), (SANDSTONE, 0), false);
    w.box_fill(14, 7, 9, 14, 7, 11, (SANDSTONE, 0), (SANDSTONE, 0), false);
    w.box_fill(
        5,
        5,
        9,
        5,
        7,
        11,
        (SANDSTONE, SANDSTONE_SMOOTH),
        (SANDSTONE, SANDSTONE_SMOOTH),
        false,
    );
    w.box_fill(
        15,
        5,
        9,
        15,
        7,
        11,
        (SANDSTONE, SANDSTONE_SMOOTH),
        (SANDSTONE, SANDSTONE_SMOOTH),
        false,
    );
    for (x, y, z) in [
        (5, 5, 10),
        (5, 6, 10),
        (6, 6, 10),
        (15, 5, 10),
        (15, 6, 10),
        (14, 6, 10),
    ] {
        w.place(AIR, 0, x, y, z);
    }

    w.box_fill(2, 4, 4, 2, 6, 4, (AIR, 0), (AIR, 0), false);
    w.box_fill(18, 4, 4, 18, 6, 4, (AIR, 0), (AIR, 0), false);
    for (x, y, z) in [(2, 4, 5), (2, 3, 4), (18, 4, 5), (18, 3, 4)] {
        w.place(SANDSTONE_STAIRS, stairs_north, x, y, z);
    }
    w.box_fill(1, 1, 3, 2, 2, 3, (SANDSTONE, 0), (SANDSTONE, 0), false);
    w.box_fill(18, 1, 3, 19, 2, 3, (SANDSTONE, 0), (SANDSTONE, 0), false);
    w.place(SANDSTONE_STAIRS, 0, 1, 1, 2);
    w.place(SANDSTONE_STAIRS, 0, 19, 1, 2);
    w.place(STONE_SLAB, SANDSTONE_SLAB, 1, 2, 2);
    w.place(STONE_SLAB, SANDSTONE_SLAB, 19, 2, 2);
    w.place(SANDSTONE_STAIRS, stairs_west, 2, 1, 2);
    w.place(SANDSTONE_STAIRS, stairs_east, 18, 1, 2);

    w.box_fill(4, 3, 5, 4, 3, 18, (SANDSTONE, 0), (SANDSTONE, 0), false);
    w.box_fill(16, 3, 5, 16, 3, 17, (SANDSTONE, 0), (SANDSTONE, 0), false);
    w.box_fill(3, 1, 5, 4, 2, 16, (AIR, 0), (AIR, 0), false);
    w.box_fill(15, 1, 5, 16, 2, 16, (AIR, 0), (AIR, 0), false);
    for z in (5..=17).step_by(2) {
        for x in [4, 16] {
            w.place(SANDSTONE, SANDSTONE_SMOOTH, x, 1, z);
            w.place(SANDSTONE, SANDSTONE_HIEROGLYPHS, x, 2, z);
        }
    }

    for (x, z, data) in [
        (10, 7, base_deco),
        (10, 8, base_deco),
        (9, 9, base_deco),
        (11, 9, base_deco),
        (8, 10, base_deco),
        (12, 10, base_deco),
        (7, 10, base_deco),
        (13, 10, base_deco),
        (9, 11, base_deco),
        (11, 11, base_deco),
        (10, 12, base_deco),
        (10, 13, base_deco),
        (10, 10, blue),
    ] {
        w.place(STAINED_HARDENED_CLAY, data, x, 0, z);
    }

    for x in [0, 20] {
        for (y, z, block, data) in [
            (2, 1, SANDSTONE, SANDSTONE_SMOOTH),
            (2, 2, STAINED_HARDENED_CLAY, base_deco),
            (2, 3, SANDSTONE, SANDSTONE_SMOOTH),
            (3, 1, SANDSTONE, SANDSTONE_SMOOTH),
            (3, 2, STAINED_HARDENED_CLAY, base_deco),
            (3, 3, SANDSTONE, SANDSTONE_SMOOTH),
            (4, 1, STAINED_HARDENED_CLAY, base_deco),
            (4, 2, SANDSTONE, SANDSTONE_HIEROGLYPHS),
            (4, 3, STAINED_HARDENED_CLAY, base_deco),
            (5, 1, SANDSTONE, SANDSTONE_SMOOTH),
            (5, 2, STAINED_HARDENED_CLAY, base_deco),
            (5, 3, SANDSTONE, SANDSTONE_SMOOTH),
            (6, 1, STAINED_HARDENED_CLAY, base_deco),
            (6, 2, SANDSTONE, SANDSTONE_HIEROGLYPHS),
            (6, 3, STAINED_HARDENED_CLAY, base_deco),
            (7, 1, STAINED_HARDENED_CLAY, base_deco),
            (7, 2, STAINED_HARDENED_CLAY, base_deco),
            (7, 3, STAINED_HARDENED_CLAY, base_deco),
            (8, 1, SANDSTONE, SANDSTONE_SMOOTH),
            (8, 2, SANDSTONE, SANDSTONE_SMOOTH),
            (8, 3, SANDSTONE, SANDSTONE_SMOOTH),
        ] {
            w.place(block, data, x, y, z);
        }
    }

    for x in [2, 18] {
        for (dx, y, block, data) in [
            (-1, 2, SANDSTONE, SANDSTONE_SMOOTH),
            (0, 2, STAINED_HARDENED_CLAY, base_deco),
            (1, 2, SANDSTONE, SANDSTONE_SMOOTH),
            (-1, 3, SANDSTONE, SANDSTONE_SMOOTH),
            (0, 3, STAINED_HARDENED_CLAY, base_deco),
            (1, 3, SANDSTONE, SANDSTONE_SMOOTH),
            (-1, 4, STAINED_HARDENED_CLAY, base_deco),
            (0, 4, SANDSTONE, SANDSTONE_HIEROGLYPHS),
            (1, 4, STAINED_HARDENED_CLAY, base_deco),
            (-1, 5, SANDSTONE, SANDSTONE_SMOOTH),
            (0, 5, STAINED_HARDENED_CLAY, base_deco),
            (1, 5, SANDSTONE, SANDSTONE_SMOOTH),
            (-1, 6, STAINED_HARDENED_CLAY, base_deco),
            (0, 6, SANDSTONE, SANDSTONE_HIEROGLYPHS),
            (1, 6, STAINED_HARDENED_CLAY, base_deco),
            (-1, 7, STAINED_HARDENED_CLAY, base_deco),
            (0, 7, STAINED_HARDENED_CLAY, base_deco),
            (1, 7, STAINED_HARDENED_CLAY, base_deco),
            (-1, 8, SANDSTONE, SANDSTONE_SMOOTH),
            (0, 8, SANDSTONE, SANDSTONE_SMOOTH),
            (1, 8, SANDSTONE, SANDSTONE_SMOOTH),
        ] {
            w.place(block, data, x + dx, y, 0);
        }
    }
    w.box_fill(
        8,
        4,
        0,
        12,
        6,
        0,
        (SANDSTONE, SANDSTONE_SMOOTH),
        (SANDSTONE, SANDSTONE_SMOOTH),
        false,
    );
    w.place(AIR, 0, 8, 6, 0);
    w.place(AIR, 0, 12, 6, 0);
    w.place(STAINED_HARDENED_CLAY, base_deco, 9, 5, 0);
    w.place(SANDSTONE, SANDSTONE_HIEROGLYPHS, 10, 5, 0);
    w.place(STAINED_HARDENED_CLAY, base_deco, 11, 5, 0);

    w.box_fill(
        8,
        -14,
        8,
        12,
        -11,
        12,
        (SANDSTONE, SANDSTONE_SMOOTH),
        (SANDSTONE, SANDSTONE_SMOOTH),
        false,
    );
    w.box_fill(
        8,
        -10,
        8,
        12,
        -10,
        12,
        (SANDSTONE, SANDSTONE_HIEROGLYPHS),
        (SANDSTONE, SANDSTONE_HIEROGLYPHS),
        false,
    );
    w.box_fill(
        8,
        -9,
        8,
        12,
        -9,
        12,
        (SANDSTONE, SANDSTONE_SMOOTH),
        (SANDSTONE, SANDSTONE_SMOOTH),
        false,
    );
    w.box_fill(8, -8, 8, 12, -1, 12, (SANDSTONE, 0), (SANDSTONE, 0), false);
    w.box_fill(9, -11, 9, 11, -1, 11, (AIR, 0), (AIR, 0), false);
    w.place(STONE_PRESSURE_PLATE, 0, 10, -11, 10);
    w.box_fill(9, -13, 9, 11, -13, 11, (TNT, 0), (AIR, 0), false);

    for (x, y, z, block, data) in [
        (8, -11, 10, AIR, 0),
        (8, -10, 10, AIR, 0),
        (7, -10, 10, SANDSTONE, SANDSTONE_HIEROGLYPHS),
        (7, -11, 10, SANDSTONE, SANDSTONE_SMOOTH),
        (12, -11, 10, AIR, 0),
        (12, -10, 10, AIR, 0),
        (13, -10, 10, SANDSTONE, SANDSTONE_HIEROGLYPHS),
        (13, -11, 10, SANDSTONE, SANDSTONE_SMOOTH),
        (10, -11, 8, AIR, 0),
        (10, -10, 8, AIR, 0),
        (10, -10, 7, SANDSTONE, SANDSTONE_HIEROGLYPHS),
        (10, -11, 7, SANDSTONE, SANDSTONE_SMOOTH),
        (10, -11, 12, AIR, 0),
        (10, -10, 12, AIR, 0),
        (10, -10, 13, SANDSTONE, SANDSTONE_HIEROGLYPHS),
        (10, -11, 13, SANDSTONE, SANDSTONE_SMOOTH),
    ] {
        w.place(block, data, x, y, z);
    }

    // createChest() is commented out in the fixed target and always returns false.
    let _ = &mut piece.desert_chests;
}

fn jungle_pyramid(
    piece: &mut ScatteredPieceState,
    neighborhood: &mut PopulationNeighborhood,
    chunk_box: StructureBounds,
    random: &mut MtRandom,
    west: i32,
    base_y: i32,
    north: i32,
) {
    let mut w = Writer::new(neighborhood, chunk_box, west, base_y, north);
    let stairs_north = 2;
    let stairs_south = 3;
    let stairs_east = 0;
    let stairs_west = 1;

    w.selector_box(random, 0, -4, 0, 11, 0, 14);
    w.selector_box(random, 2, 1, 2, 9, 2, 2);
    w.selector_box(random, 2, 1, 12, 9, 2, 12);
    w.selector_box(random, 2, 1, 3, 2, 2, 11);
    w.selector_box(random, 9, 1, 3, 9, 2, 11);

    w.selector_box(random, 1, 3, 1, 10, 6, 1);
    w.selector_box(random, 1, 3, 13, 10, 6, 13);
    w.selector_box(random, 1, 3, 2, 1, 6, 12);
    w.selector_box(random, 10, 3, 2, 10, 6, 12);

    w.selector_box(random, 2, 3, 2, 9, 3, 12);
    w.selector_box(random, 2, 6, 2, 9, 6, 12);
    w.selector_box(random, 3, 7, 3, 8, 7, 11);
    w.selector_box(random, 4, 8, 4, 7, 8, 10);

    for (x0, y0, z0, x1, y1, z1) in [
        (3, 1, 3, 8, 2, 11),
        (4, 3, 6, 7, 3, 9),
        (2, 4, 2, 9, 5, 12),
        (4, 6, 5, 7, 6, 9),
        (5, 7, 6, 6, 7, 8),
        (5, 1, 2, 6, 2, 2),
        (5, 2, 12, 6, 2, 12),
        (5, 5, 1, 6, 5, 1),
        (5, 5, 13, 6, 5, 13),
    ] {
        w.air_box(x0, y0, z0, x1, y1, z1);
    }
    for (x, y, z) in [(1, 5, 5), (10, 5, 5), (1, 5, 9), (10, 5, 9)] {
        w.place(AIR, 0, x, y, z);
    }

    for z in [0, 14] {
        for x in [2, 4, 7, 9] {
            w.selector_box(random, x, 4, z, x, 5, z);
        }
    }
    w.selector_box(random, 5, 6, 0, 6, 6, 0);
    for x in [0, 11] {
        for z in (2..=12).step_by(2) {
            w.selector_box(random, x, 4, z, x, 5, z);
        }
        w.selector_box(random, x, 6, 5, x, 6, 5);
        w.selector_box(random, x, 6, 9, x, 6, 9);
    }
    for (x, z) in [(2, 2), (9, 2), (2, 12), (9, 12)] {
        w.selector_box(random, x, 7, z, x, 9, z);
    }
    for (x, z) in [(4, 4), (7, 4), (4, 10), (7, 10)] {
        w.selector_box(random, x, 9, z, x, 9, z);
    }
    w.selector_box(random, 5, 9, 7, 6, 9, 7);

    for (block, data, x, y, z) in [
        (STONE_STAIRS, stairs_north, 5, 9, 6),
        (STONE_STAIRS, stairs_north, 6, 9, 6),
        (STONE_STAIRS, stairs_south, 5, 9, 8),
        (STONE_STAIRS, stairs_south, 6, 9, 8),
        (STONE_STAIRS, stairs_north, 4, 0, 0),
        (STONE_STAIRS, stairs_north, 5, 0, 0),
        (STONE_STAIRS, stairs_north, 6, 0, 0),
        (STONE_STAIRS, stairs_north, 7, 0, 0),
        (STONE_STAIRS, stairs_north, 4, 1, 8),
        (STONE_STAIRS, stairs_north, 4, 2, 9),
        (STONE_STAIRS, stairs_north, 4, 3, 10),
        (STONE_STAIRS, stairs_north, 7, 1, 8),
        (STONE_STAIRS, stairs_north, 7, 2, 9),
        (STONE_STAIRS, stairs_north, 7, 3, 10),
    ] {
        w.place(block, data, x, y, z);
    }
    w.selector_box(random, 4, 1, 9, 4, 1, 9);
    w.selector_box(random, 7, 1, 9, 7, 1, 9);
    w.selector_box(random, 4, 1, 10, 7, 2, 10);

    w.selector_box(random, 5, 4, 5, 6, 4, 5);
    w.place(STONE_STAIRS, stairs_east, 4, 4, 5);
    w.place(STONE_STAIRS, stairs_west, 7, 4, 5);

    for i in 0..4 {
        w.place(STONE_STAIRS, stairs_south, 5, -i, 6 + i);
        w.place(STONE_STAIRS, stairs_south, 6, -i, 6 + i);
        w.air_box(5, -i, 7 + i, 6, -i, 9 + i);
    }

    w.air_box(1, -3, 12, 10, -1, 13);
    w.air_box(1, -3, 1, 3, -1, 13);
    w.air_box(1, -3, 1, 9, -1, 5);
    for z in (1..=13).step_by(2) {
        w.selector_box(random, 1, -3, z, 1, -2, z);
    }
    for z in (2..=12).step_by(2) {
        w.selector_box(random, 1, -1, z, 3, -1, z);
    }
    w.selector_box(random, 2, -2, 1, 5, -2, 1);
    w.selector_box(random, 7, -2, 1, 9, -2, 1);
    w.selector_box(random, 6, -3, 1, 6, -3, 1);
    w.selector_box(random, 6, -1, 1, 6, -1, 1);

    w.place(TRIPWIRE_HOOK, TRIPWIRE_ATTACHED | 3, 1, -3, 8);
    w.place(TRIPWIRE_HOOK, TRIPWIRE_ATTACHED | 1, 4, -3, 8);
    w.place(TRIPWIRE, TRIPWIRE_ATTACHED, 2, -3, 8);
    w.place(TRIPWIRE, TRIPWIRE_ATTACHED, 3, -3, 8);
    for (x, z) in [
        (5, 7),
        (5, 6),
        (5, 5),
        (5, 4),
        (5, 3),
        (5, 2),
        (5, 1),
        (4, 1),
    ] {
        w.place(REDSTONE_WIRE, 0, x, -3, z);
    }
    w.place(MOSSY_COBBLESTONE, 0, 3, -3, 1);
    if !piece.jungle_traps[0] && w.create_dispenser(3, -2, 1, 3) {
        piece.jungle_traps[0] = true;
    }
    w.place(VINE, VINE_ALL, 3, -2, 2);

    w.place(TRIPWIRE_HOOK, TRIPWIRE_ATTACHED, 7, -3, 1);
    w.place(TRIPWIRE_HOOK, TRIPWIRE_ATTACHED | 2, 7, -3, 5);
    for z in 2..=4 {
        w.place(TRIPWIRE, TRIPWIRE_ATTACHED, 7, -3, z);
    }
    for (x, y, z) in [(8, -3, 6), (9, -3, 6), (9, -3, 5), (9, -2, 4)] {
        w.place(REDSTONE_WIRE, 0, x, y, z);
    }
    w.place(MOSSY_COBBLESTONE, 0, 9, -3, 4);
    if !piece.jungle_traps[1] && w.create_dispenser(9, -2, 3, 4) {
        piece.jungle_traps[1] = true;
    }
    w.place(VINE, VINE_ALL, 8, -1, 3);
    w.place(VINE, VINE_ALL, 8, -2, 3);

    // createChest() is target-disabled; main/hidden chest flags remain false.
    w.place(MOSSY_COBBLESTONE, 0, 9, -3, 2);
    w.place(MOSSY_COBBLESTONE, 0, 8, -3, 1);
    w.place(MOSSY_COBBLESTONE, 0, 4, -3, 5);
    w.place(MOSSY_COBBLESTONE, 0, 5, -2, 5);
    w.place(MOSSY_COBBLESTONE, 0, 5, -1, 5);
    w.place(MOSSY_COBBLESTONE, 0, 6, -3, 5);
    w.place(MOSSY_COBBLESTONE, 0, 7, -2, 5);
    w.place(MOSSY_COBBLESTONE, 0, 7, -1, 5);
    w.place(MOSSY_COBBLESTONE, 0, 8, -3, 5);
    w.selector_box(random, 9, -1, 1, 9, -1, 5);

    w.air_box(8, -3, 8, 10, -1, 10);
    for x in 8..=10 {
        w.place(STONE_BRICK, 3, x, -2, 11);
    }
    for x in 8..=10 {
        w.place(LEVER, 3, x, -2, 12);
    }
    w.selector_box(random, 8, -3, 8, 8, -3, 10);
    w.selector_box(random, 10, -3, 8, 10, -3, 10);
    w.place(MOSSY_COBBLESTONE, 0, 10, -2, 9);
    w.place(REDSTONE_WIRE, 0, 8, -2, 9);
    w.place(REDSTONE_WIRE, 0, 8, -2, 10);
    w.place(REDSTONE_WIRE, 0, 10, -1, 9);
    w.place(STICKY_PISTON, 1, 9, -2, 8);
    w.place(STICKY_PISTON, 5, 10, -2, 8);
    w.place(STICKY_PISTON, 5, 10, -1, 8);
    w.place(UNPOWERED_REPEATER, 0, 10, -2, 10);

    let _ = (piece.jungle_main_chest, piece.jungle_hidden_chest);
}

fn swampland_hut(
    neighborhood: &mut PopulationNeighborhood,
    chunk_box: StructureBounds,
    west: i32,
    base_y: i32,
    north: i32,
) {
    let mut w = Writer::new(neighborhood, chunk_box, west, base_y, north);

    w.box_fill(0, 2, 0, 6, 3, 8, (AIR, 0), (AIR, 0), true);
    w.box_fill(
        1,
        1,
        1,
        5,
        1,
        7,
        (PLANKS, SPRUCE_WOOD),
        (PLANKS, SPRUCE_WOOD),
        false,
    );
    w.box_fill(
        1,
        5,
        2,
        5,
        5,
        7,
        (PLANKS, SPRUCE_WOOD),
        (PLANKS, SPRUCE_WOOD),
        false,
    );
    w.box_fill(
        2,
        1,
        0,
        4,
        1,
        0,
        (PLANKS, SPRUCE_WOOD),
        (PLANKS, SPRUCE_WOOD),
        false,
    );

    w.box_fill(
        2,
        2,
        2,
        3,
        4,
        2,
        (PLANKS, SPRUCE_WOOD),
        (PLANKS, SPRUCE_WOOD),
        false,
    );
    w.box_fill(
        1,
        2,
        3,
        1,
        4,
        6,
        (PLANKS, SPRUCE_WOOD),
        (PLANKS, SPRUCE_WOOD),
        false,
    );
    w.box_fill(
        5,
        2,
        3,
        5,
        4,
        6,
        (PLANKS, SPRUCE_WOOD),
        (PLANKS, SPRUCE_WOOD),
        false,
    );
    w.box_fill(
        2,
        2,
        7,
        4,
        4,
        7,
        (PLANKS, SPRUCE_WOOD),
        (PLANKS, SPRUCE_WOOD),
        false,
    );

    for (x, z) in [(1, 2), (5, 2), (1, 7), (5, 7)] {
        w.box_fill(x, 0, z, x, 4, z, (LOG, 0), (LOG, 0), false);
    }

    w.place(FENCE, 0, 2, 3, 2);
    w.place(FENCE, 0, 3, 3, 7);
    w.place(AIR, 0, 1, 3, 4);
    w.place(AIR, 0, 5, 3, 4);
    w.place(AIR, 0, 5, 3, 5);
    w.place(FLOWER_POT, 0, 1, 3, 5);
    w.place(PLANKS, SPRUCE_WOOD, 4, 4, 2);
    w.place(CRAFTING_TABLE, 0, 3, 2, 6);
    // placeCauldron() is target-disabled.

    w.place(FENCE, 0, 1, 2, 1);
    w.place(FENCE, 0, 5, 2, 1);

    let south = 2;
    let east = 1;
    let west_data = 0;
    let north_data = 3;
    w.box_fill(
        0,
        5,
        1,
        6,
        5,
        1,
        (SPRUCE_STAIRS, south),
        (SPRUCE_STAIRS, south),
        false,
    );
    w.box_fill(
        0,
        5,
        2,
        0,
        5,
        7,
        (SPRUCE_STAIRS, west_data),
        (SPRUCE_STAIRS, west_data),
        false,
    );
    w.box_fill(
        6,
        5,
        2,
        6,
        5,
        7,
        (SPRUCE_STAIRS, east),
        (SPRUCE_STAIRS, east),
        false,
    );
    w.box_fill(
        0,
        5,
        8,
        6,
        5,
        8,
        (SPRUCE_STAIRS, north_data),
        (SPRUCE_STAIRS, north_data),
        false,
    );

    for z in [2, 7] {
        for x in [1, 5] {
            w.fill_column_down(LOG, 0, x, -1, z);
        }
    }
}

struct Writer<'a> {
    neighborhood: &'a mut PopulationNeighborhood,
    chunk_box: StructureBounds,
    west: i32,
    base_y: i32,
    north: i32,
}

impl<'a> Writer<'a> {
    fn new(
        neighborhood: &'a mut PopulationNeighborhood,
        chunk_box: StructureBounds,
        west: i32,
        base_y: i32,
        north: i32,
    ) -> Self {
        Self {
            neighborhood,
            chunk_box,
            west,
            base_y,
            north,
        }
    }

    fn world_pos(&self, x: i32, y: i32, z: i32) -> (i32, i32, i32) {
        (self.west + x, self.base_y + y, self.north + z)
    }

    fn local_inside_chunk(&self, x: i32, y: i32, z: i32) -> bool {
        let (wx, wy, wz) = self.world_pos(x, y, z);
        self.chunk_box.contains(wx, wy, wz)
    }

    fn get_local(&self, x: i32, y: i32, z: i32) -> u16 {
        let (wx, wy, wz) = self.world_pos(x, y, z);
        self.neighborhood.block_id(wx, wy, wz)
    }

    fn place(&mut self, block: u16, data: u8, x: i32, y: i32, z: i32) {
        let (wx, wy, wz) = self.world_pos(x, y, z);
        if self.chunk_box.contains(wx, wy, wz) {
            self.neighborhood.set_state(wx, wy, wz, state(block, data));
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn box_fill(
        &mut self,
        x0: i32,
        y0: i32,
        z0: i32,
        x1: i32,
        y1: i32,
        z1: i32,
        edge: (u16, u8),
        fill: (u16, u8),
        skip_air: bool,
    ) {
        for y in y0..=y1 {
            for x in x0..=x1 {
                for z in z0..=z1 {
                    if skip_air && self.get_local(x, y, z) == AIR {
                        continue;
                    }
                    let value = if y == y0 || y == y1 || x == x0 || x == x1 || z == z0 || z == z1 {
                        edge
                    } else {
                        fill
                    };
                    self.place(value.0, value.1, x, y, z);
                }
            }
        }
    }

    fn air_box(&mut self, x0: i32, y0: i32, z0: i32, x1: i32, y1: i32, z1: i32) {
        self.box_fill(x0, y0, z0, x1, y1, z1, (AIR, 0), (AIR, 0), false);
    }

    #[allow(clippy::too_many_arguments)]
    fn selector_box(
        &mut self,
        random: &mut MtRandom,
        x0: i32,
        y0: i32,
        z0: i32,
        x1: i32,
        y1: i32,
        z1: i32,
    ) {
        for y in y0..=y1 {
            for x in x0..=x1 {
                for z in z0..=z1 {
                    let block = if random.next_float() < 0.4 {
                        COBBLESTONE
                    } else {
                        MOSSY_COBBLESTONE
                    };
                    self.place(block, 0, x, y, z);
                }
            }
        }
    }

    fn fill_column_down(&mut self, block: u16, data: u8, x: i32, start_y: i32, z: i32) {
        let (wx, mut wy, wz) = self.world_pos(x, start_y, z);
        if !self.chunk_box.contains(wx, wy, wz) {
            return;
        }

        while wy > 1 {
            let id = self.neighborhood.block_id(wx, wy, wz);
            if id != AIR && !is_liquid(id) {
                break;
            }
            self.neighborhood.set_state(wx, wy, wz, state(block, data));
            wy -= 1;
        }
        if self.neighborhood.block_id(wx, wy, wz) == GRASS {
            self.neighborhood.set_state(wx, wy, wz, state(DIRT, 0));
        }
    }

    fn create_dispenser(&mut self, x: i32, y: i32, z: i32, facing: u8) -> bool {
        if !self.local_inside_chunk(x, y, z) {
            return false;
        }
        if self.get_local(x, y, z) == DISPENSER {
            return false;
        }
        self.place(DISPENSER, facing, x, y, z);
        true
    }
}

fn is_liquid(id: u16) -> bool {
    matches!(id, 8..=11)
}

fn chunk_bounds(target: ChunkCoord) -> StructureBounds {
    let x0 = target.x().wrapping_mul(16);
    let z0 = target.z().wrapping_mul(16);
    StructureBounds::new(
        x0,
        0,
        z0,
        x0.wrapping_add(15),
        WORLD_HEIGHT as i32 - 1,
        z0.wrapping_add(15),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const JUNGLE_BIOME: u8 = 21;

    fn flat_neighborhood(center: ChunkCoord, top_y: i32, biome: u8) -> PopulationNeighborhood {
        let mut neighborhood = PopulationNeighborhood::filled(center, state(AIR, 0), biome);
        let x0 = center.x().wrapping_sub(1).wrapping_mul(16);
        let z0 = center.z().wrapping_sub(1).wrapping_mul(16);
        for x in x0..x0 + 48 {
            for z in z0..z0 + 48 {
                for y in 0..=top_y {
                    neighborhood.set_state(
                        x,
                        y,
                        z,
                        state(if y == top_y { GRASS } else { DIRT }, 0),
                    );
                }
            }
        }
        neighborhood
    }

    fn center_hash(neighborhood: &PopulationNeighborhood) -> u64 {
        neighborhood
            .center_states()
            .iter()
            .fold(0xcbf2_9ce4_8422_2325_u64, |mut hash, value| {
                for byte in value.to_le_bytes() {
                    hash = (hash ^ u64::from(byte)).wrapping_mul(0x100_0000_01b3);
                }
                hash
            })
    }

    fn block_count(neighborhood: &PopulationNeighborhood, id: u16) -> usize {
        neighborhood
            .center_states()
            .iter()
            .filter(|value| (**value >> 4) == id)
            .count()
    }

    fn fixture_plan(kind: ScatteredKind, bounds: StructureBounds) -> ScatteredPlan {
        ScatteredPlan {
            core: super::super::StructureStartCore::new(ChunkCoord::new(0, 0), bounds),
            piece: Some(ScatteredPieceState::new(kind)),
        }
    }

    #[test]
    fn independent_desert_pyramid_chunk_fixture_matches() {
        let mut plan = fixture_plan(
            ScatteredKind::DesertPyramid,
            StructureBounds::new(0, 64, 0, 20, 78, 20),
        );
        let mut neighborhood = flat_neighborhood(ChunkCoord::new(0, 0), 63, 2);
        let mut random = MtRandom::new(0x1234_5678);
        assert!(ScatteredPostProcessor::process_start(
            &mut plan,
            &mut neighborhood,
            ChunkCoord::new(0, 0),
            &mut random,
        ));
        assert_eq!(center_hash(&neighborhood), 0x8e10_fa9a_2eb3_f1d5);
        assert_eq!(block_count(&neighborhood, SANDSTONE), 2032);
        assert_eq!(block_count(&neighborhood, TNT), 9);
        assert_eq!(block_count(&neighborhood, STAINED_HARDENED_CLAY), 35);
        assert_eq!(
            plan.piece.as_ref().expect("piece").desert_chests,
            [false; 4],
            "target createChest() is a no-op",
        );
    }

    #[test]
    fn independent_jungle_pyramid_chunk_fixture_matches() {
        let mut plan = fixture_plan(
            ScatteredKind::JunglePyramid,
            StructureBounds::new(0, 64, 0, 11, 73, 14),
        );
        let mut neighborhood = flat_neighborhood(ChunkCoord::new(0, 0), 70, JUNGLE_BIOME);
        let mut random = MtRandom::new(0x1234_5678);
        assert!(ScatteredPostProcessor::process_start(
            &mut plan,
            &mut neighborhood,
            ChunkCoord::new(0, 0),
            &mut random,
        ));
        assert_eq!(center_hash(&neighborhood), 0x1789_bd78_f317_c071);
        assert_eq!(block_count(&neighborhood, COBBLESTONE), 480);
        assert_eq!(block_count(&neighborhood, MOSSY_COBBLESTONE), 677);
        assert_eq!(block_count(&neighborhood, DISPENSER), 2);
        assert_eq!(block_count(&neighborhood, TRIPWIRE), 5);
        assert_eq!(block_count(&neighborhood, TRIPWIRE_HOOK), 4);
        let piece = plan.piece.as_ref().expect("piece");
        assert_eq!(piece.jungle_traps, [true, true]);
        assert!(!piece.jungle_main_chest);
        assert!(!piece.jungle_hidden_chest);
    }

    #[test]
    fn independent_swampland_hut_chunk_fixture_matches() {
        let mut plan = fixture_plan(
            ScatteredKind::SwamplandHut,
            StructureBounds::new(0, 64, 0, 6, 70, 8),
        );
        let mut neighborhood = flat_neighborhood(ChunkCoord::new(0, 0), 70, 6);
        let mut random = MtRandom::new(0x1234_5678);
        assert!(ScatteredPostProcessor::process_start(
            &mut plan,
            &mut neighborhood,
            ChunkCoord::new(0, 0),
            &mut random,
        ));
        assert_eq!(center_hash(&neighborhood), 0x2f7e_83e9_379a_9ced);
        assert_eq!(block_count(&neighborhood, PLANKS), 98);
        assert_eq!(block_count(&neighborhood, LOG), 24);
        assert!(!plan.piece.as_ref().expect("piece").spawned_witch);
    }

    #[test]
    fn first_chunk_alignment_is_persisted_for_later_chunks() {
        let source = ChunkCoord::new(0, 0);
        let mut plan = ScatteredPlan {
            core: super::super::StructureStartCore::new(
                source,
                StructureBounds::new(0, 64, 0, 11, 73, 14),
            ),
            piece: Some(ScatteredPieceState::new(ScatteredKind::JunglePyramid)),
        };
        let mut neighborhood = flat_neighborhood(source, 70, JUNGLE_BIOME);
        let mut random = MtRandom::new(0);

        assert!(ScatteredPostProcessor::process_start(
            &mut plan,
            &mut neighborhood,
            source,
            &mut random,
        ));
        assert_eq!(plan.piece.as_ref().expect("piece").height_position, 71);
    }

    #[test]
    fn active_jungle_dispenser_sets_durable_trap_flag() {
        let source = ChunkCoord::new(0, 0);
        let mut plan = ScatteredPlan {
            core: super::super::StructureStartCore::new(
                source,
                StructureBounds::new(0, 64, 0, 11, 73, 14),
            ),
            piece: Some(ScatteredPieceState::new(ScatteredKind::JunglePyramid)),
        };
        let mut neighborhood = flat_neighborhood(source, 70, JUNGLE_BIOME);
        let mut random = MtRandom::new(0x1234_5678);

        assert!(ScatteredPostProcessor::process_start(
            &mut plan,
            &mut neighborhood,
            source,
            &mut random,
        ));
        assert_eq!(
            plan.piece.as_ref().expect("piece").jungle_traps,
            [true, true]
        );
    }
}
