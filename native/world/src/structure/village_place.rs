use crate::ChunkCoord;
use crate::population::{PopulationNeighborhood, state};
use crate::population_finalizer::material_blocks_motion;
use crate::terrain_shape::noise::MtRandom;

use super::village_plan::{
    VillagePieceExtra, VillagePieceKind, VillagePiecePlan, VillagePlan, VillageStyle,
};
use super::{StructureBounds, StructureOrientation};

const AIR: u16 = 0;
const GRASS: u16 = 2;
const DIRT: u16 = 3;
const COBBLESTONE: u16 = 4;
const PLANKS: u16 = 5;
const WATER: u16 = 9;
const SANDSTONE: u16 = 24;
const WEB: u16 = 30;
const DOUBLE_STONE_SLAB: u16 = 43;
const STONE_SLAB: u16 = 44;
const BOOKSHELF: u16 = 47;
const MOSSY_COBBLESTONE: u16 = 48;
const TORCH: u16 = 50;
const OAK_STAIRS: u16 = 53;
const CRAFTING_TABLE: u16 = 58;
const WHEAT: u16 = 59;
const FARMLAND: u16 = 60;
const FURNACE: u16 = 61;
const LADDER: u16 = 65;
const LOG: u16 = 17;
const FENCE: u16 = 85;
const IRON_BARS: u16 = 101;
const GLASS_PANE: u16 = 102;
const SANDSTONE_STAIRS: u16 = 128;
const SPRUCE_STAIRS: u16 = 134;
const CARROTS: u16 = 141;
const POTATOES: u16 = 142;
const LOG2: u16 = 162;
const ACACIA_STAIRS: u16 = 163;
const SPRUCE_DOOR: u16 = 193;
const ACACIA_DOOR: u16 = 196;
const GRASS_PATH: u16 = 198;
const BEETROOT: u16 = 244;
const WOODEN_DOOR: u16 = 64;
const FLOWING_LAVA: u16 = 10;

const TORCH_WEST: u8 = 1;
const TORCH_EAST: u8 = 2;
const TORCH_NORTH: u8 = 3;
const TORCH_SOUTH: u8 = 4;

pub(crate) struct VillagePostProcessor {
    post_seed: u32,
}

impl VillagePostProcessor {
    pub(crate) const fn new(post_seed: u32) -> Self {
        Self { post_seed }
    }

    pub(crate) fn process_start(
        &self,
        plan: &mut VillagePlan,
        neighborhood: &mut PopulationNeighborhood,
        target: ChunkCoord,
        random: &mut MtRandom,
    ) -> bool {
        if !plan.core.should_post_process(target) {
            return false;
        }

        let chunk_box = chunk_bounds(target);
        let mut processed = false;
        for piece in &mut plan.pieces {
            if piece.bounds.intersects(chunk_box) {
                process_piece(
                    piece,
                    plan.style,
                    plan.abandoned,
                    neighborhood,
                    chunk_box,
                    random,
                    self.post_seed,
                );
                processed = true;
            }
        }

        plan.core.mark_post_processed(target);
        processed
    }
}

fn process_piece(
    piece: &mut VillagePiecePlan,
    style: VillageStyle,
    abandoned: bool,
    neighborhood: &mut PopulationNeighborhood,
    chunk_box: StructureBounds,
    random: &mut MtRandom,
    post_seed: u32,
) {
    if piece.kind == VillagePieceKind::StraightRoad {
        paint_road(piece.bounds, style, neighborhood, chunk_box);
        return;
    }

    let Some((_, height, _)) = piece.kind.dimensions() else {
        return;
    };

    if piece.height_position < 0 {
        let average = average_ground_height(neighborhood, piece.bounds, chunk_box);
        piece.height_position = average;
        if average < 0 {
            return;
        }

        let dy = match piece.kind {
            VillagePieceKind::Start => average - piece.bounds.y1 + 3,
            _ => average - piece.bounds.y1 + height - 1,
        };
        piece.bounds.move_by(0, dy, 0);
    }

    let mut writer = VillagePieceWriter {
        neighborhood,
        chunk_box,
        bounds: piece.bounds,
        orientation: piece.orientation,
        style,
        abandoned,
        post_seed,
    };

    match piece.kind {
        VillagePieceKind::Start => writer.well(random),
        VillagePieceKind::SimpleHouse => writer.simple_house(piece, random),
        VillagePieceKind::SmallTemple => writer.small_temple(random),
        VillagePieceKind::BookHouse => writer.book_house(random),
        VillagePieceKind::SmallHut => writer.small_hut(piece, random),
        VillagePieceKind::PigHouse => writer.pig_house(random),
        VillagePieceKind::DoubleFarmland => writer.double_farmland(piece, random),
        VillagePieceKind::Farmland => writer.farmland(piece, random),
        VillagePieceKind::Smithy => writer.smithy(piece, random),
        VillagePieceKind::TwoRoomHouse => writer.two_room_house(piece, random),
        VillagePieceKind::LightPost => writer.light_post(),
        VillagePieceKind::StraightRoad => {}
    }
}

fn paint_road(
    bounds: StructureBounds,
    style: VillageStyle,
    neighborhood: &mut PopulationNeighborhood,
    chunk_box: StructureBounds,
) {
    // The 0.15.10 StraightRoad::postProcess scans world X/Z inside its bounds, clipping
    // against the target chunk at Y=64. It paints the highest solid block with biome-adapted
    // grass path, or planks when the top material is liquid (APK ARM Thumb RVA 0xd6f748).
    let path = biome_block(style, GRASS_PATH, 0);
    let over_water = biome_block(style, PLANKS, 0);
    for x in bounds.x0..=bounds.x1 {
        for z in bounds.z0..=bounds.z1 {
            if !chunk_box.contains(x, 64, z) {
                continue;
            }
            let y = road_top_solid_y(neighborhood, x, z);
            let (block, data) = if is_liquid(neighborhood.block_id(x, y, z)) {
                over_water
            } else {
                path
            };
            neighborhood.set_state(x, y, z, state(block, data));
        }
    }
}

fn road_top_solid_y(neighborhood: &PopulationNeighborhood, x: i32, z: i32) -> i32 {
    // LevelChunk::getTopSolidBlock(x,z,true,false) in the 0.15.10 APK uses
    // Material::getBlocksMotion() or liquids, excluding leaves when the second flag is false.
    for y in (0..128).rev() {
        let id = neighborhood.block_id(x, y, z);
        if !matches!(id, 18 | 161) && (material_blocks_motion(id) || is_liquid(id)) {
            return y;
        }
    }
    -1
}

fn average_ground_height(
    neighborhood: &PopulationNeighborhood,
    bounds: StructureBounds,
    chunk_box: StructureBounds,
) -> i32 {
    let mut total = 0_i32;
    let mut count = 0_i32;
    for z in bounds.z0..=bounds.z1 {
        for x in bounds.x0..=bounds.x1 {
            if chunk_box.contains(x, 64, z) {
                total = total.wrapping_add(neighborhood.above_top_solid_block(x, z, true).max(64));
                count += 1;
            }
        }
    }
    if count == 0 { -1 } else { total / count }
}

struct VillagePieceWriter<'a> {
    neighborhood: &'a mut PopulationNeighborhood,
    chunk_box: StructureBounds,
    bounds: StructureBounds,
    orientation: StructureOrientation,
    style: VillageStyle,
    abandoned: bool,
    post_seed: u32,
}

impl VillagePieceWriter<'_> {
    fn world_x(&self, x: i32, z: i32) -> i32 {
        match self.orientation {
            StructureOrientation::North | StructureOrientation::South => {
                self.bounds.x0.wrapping_add(x)
            }
            StructureOrientation::West => self.bounds.x1.wrapping_sub(z),
            StructureOrientation::East => self.bounds.x0.wrapping_add(z),
        }
    }

    fn world_y(&self, y: i32) -> i32 {
        self.bounds.y0.wrapping_add(y)
    }

    fn world_z(&self, x: i32, z: i32) -> i32 {
        match self.orientation {
            StructureOrientation::North => self.bounds.z1.wrapping_sub(z),
            StructureOrientation::South => self.bounds.z0.wrapping_add(z),
            StructureOrientation::West | StructureOrientation::East => {
                self.bounds.z0.wrapping_add(x)
            }
        }
    }

    fn get(&self, x: i32, y: i32, z: i32) -> u16 {
        let world_x = self.world_x(x, z);
        let world_y = self.world_y(y);
        let world_z = self.world_z(x, z);
        if !self.chunk_box.contains(world_x, world_y, world_z) {
            return AIR;
        }
        self.neighborhood.block_id(world_x, world_y, world_z)
    }

    fn place(&mut self, block: u16, data: u8, x: i32, y: i32, z: i32) {
        let world_x = self.world_x(x, z);
        let world_y = self.world_y(y);
        let world_z = self.world_z(x, z);
        if !self.chunk_box.contains(world_x, world_y, world_z) {
            return;
        }
        let (block, data) = biome_block(self.style, block, data);
        self.neighborhood
            .set_state(world_x, world_y, world_z, state(block, data));
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
                    if skip_air && self.get(x, y, z) == AIR {
                        continue;
                    }
                    let is_edge = y == y0 || y == y1 || x == x0 || x == x1 || z == z0 || z == z1;
                    let (block, data) = if is_edge { edge } else { fill };
                    self.place(block, data, x, y, z);
                }
            }
        }
    }

    fn air_box(&mut self, x0: i32, y0: i32, z0: i32, x1: i32, y1: i32, z1: i32) {
        self.box_fill(x0, y0, z0, x1, y1, z1, (AIR, 0), (AIR, 0), false);
    }

    #[allow(clippy::too_many_arguments)]
    fn cobble_box(
        &mut self,
        random: &mut MtRandom,
        x0: i32,
        y0: i32,
        z0: i32,
        x1: i32,
        y1: i32,
        z1: i32,
    ) {
        if !self.abandoned {
            self.box_fill(
                x0,
                y0,
                z0,
                x1,
                y1,
                z1,
                (COBBLESTONE, 0),
                (COBBLESTONE, 0),
                false,
            );
            return;
        }

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

    fn single_cobble(&mut self, random: &MtRandom, x: i32, y: i32, z: i32) {
        if !self.abandoned {
            self.place(COBBLESTONE, 0, x, y, z);
            return;
        }
        // Target helper receives Random by value. Snapshot the current selector RNG state,
        // consume one draw in the copy, and leave the caller stream untouched.
        let mut copy = random.clone();
        let block = if copy.next_float() < 0.4 {
            COBBLESTONE
        } else {
            MOSSY_COBBLESTONE
        };
        self.place(block, 0, x, y, z);
    }

    fn air_column_up(&mut self, x: i32, start_y: i32, z: i32) {
        let world_x = self.world_x(x, z);
        let mut world_y = self.world_y(start_y);
        let world_z = self.world_z(x, z);
        if !self.chunk_box.contains(world_x, world_y, world_z) {
            return;
        }
        while world_y < 127 && self.neighborhood.block_id(world_x, world_y, world_z) != AIR {
            self.neighborhood
                .set_state(world_x, world_y, world_z, state(AIR, 0));
            world_y += 1;
        }
    }

    fn fill_column_down(&mut self, block: u16, data: u8, x: i32, start_y: i32, z: i32) {
        let world_x = self.world_x(x, z);
        let mut world_y = self.world_y(start_y);
        let world_z = self.world_z(x, z);
        if !self.chunk_box.contains(world_x, world_y, world_z) {
            return;
        }
        let (block, data) = biome_block(self.style, block, data);
        while world_y > 1 {
            let id = self.neighborhood.block_id(world_x, world_y, world_z);
            if id != AIR && !is_liquid(id) {
                break;
            }
            self.neighborhood
                .set_state(world_x, world_y, world_z, state(block, data));
            world_y -= 1;
        }
        if self.neighborhood.block_id(world_x, world_y, world_z) == GRASS {
            self.neighborhood
                .set_state(world_x, world_y, world_z, state(DIRT, 0));
        }
    }

    fn stair_data(&self, data: u8) -> u8 {
        match self.orientation {
            StructureOrientation::North => data,
            StructureOrientation::South => match data {
                2 => 3,
                3 => 2,
                _ => data,
            },
            StructureOrientation::West => match data {
                0 => 2,
                1 => 3,
                2 => 0,
                3 => 1,
                _ => data,
            },
            StructureOrientation::East => match data {
                0 => 2,
                1 => 3,
                2 => 1,
                3 => 0,
                _ => data,
            },
        }
    }

    fn ladder_data(&self, data: u8) -> u8 {
        match self.orientation {
            StructureOrientation::North => data,
            StructureOrientation::South => match data {
                2 => 3,
                3 => 2,
                _ => data,
            },
            StructureOrientation::West => match data {
                2 => 4,
                3 => 5,
                4 => 2,
                5 => 3,
                _ => data,
            },
            StructureOrientation::East => match data {
                2 => 5,
                3 => 4,
                4 => 2,
                5 => 3,
                _ => data,
            },
        }
    }

    fn torch_data(&self, direction: StructureOrientation) -> u8 {
        match (self.orientation, direction) {
            (StructureOrientation::North, StructureOrientation::North) => TORCH_NORTH,
            (StructureOrientation::North, StructureOrientation::South) => TORCH_SOUTH,
            (StructureOrientation::North, StructureOrientation::West) => TORCH_WEST,
            (StructureOrientation::North, StructureOrientation::East) => TORCH_EAST,
            (StructureOrientation::South, StructureOrientation::North) => TORCH_SOUTH,
            (StructureOrientation::South, StructureOrientation::South) => TORCH_NORTH,
            (StructureOrientation::South, StructureOrientation::West) => TORCH_WEST,
            (StructureOrientation::South, StructureOrientation::East) => TORCH_EAST,
            (StructureOrientation::West, StructureOrientation::North) => TORCH_WEST,
            (StructureOrientation::West, StructureOrientation::South) => TORCH_EAST,
            (StructureOrientation::West, StructureOrientation::West) => TORCH_NORTH,
            (StructureOrientation::West, StructureOrientation::East) => TORCH_SOUTH,
            (StructureOrientation::East, StructureOrientation::North) => TORCH_EAST,
            (StructureOrientation::East, StructureOrientation::South) => TORCH_WEST,
            (StructureOrientation::East, StructureOrientation::West) => TORCH_NORTH,
            (StructureOrientation::East, StructureOrientation::East) => TORCH_SOUTH,
        }
    }

    fn current_chunk_biome(&self) -> u8 {
        self.neighborhood
            .biome_id(self.chunk_box.x0 + 8, self.chunk_box.z0 + 8)
            .unwrap_or(1)
    }

    fn local_inside_chunk(&self, x: i32, y: i32, z: i32) -> bool {
        self.chunk_box
            .contains(self.world_x(x, z), self.world_y(y), self.world_z(x, z))
    }

    fn furnace_data(&self) -> u8 {
        match self.orientation {
            StructureOrientation::South => 2,
            StructureOrientation::West => 5,
            StructureOrientation::North => 3,
            StructureOrientation::East => 4,
        }
    }

    fn well(&mut self, random: &mut MtRandom) {
        let cobble = if self.abandoned && random.next_float() > 0.4 {
            MOSSY_COBBLESTONE
        } else {
            COBBLESTONE
        };
        self.box_fill(1, 0, 1, 4, 12, 4, (cobble, 0), (WATER, 0), false);
        for (x, z) in [(2, 2), (3, 2), (2, 3), (3, 3)] {
            self.place(AIR, 0, x, 12, z);
        }
        for (x, z) in [(1, 1), (4, 1), (1, 4), (4, 4)] {
            self.place(FENCE, 0, x, 13, z);
            self.place(FENCE, 0, x, 14, z);
        }
        let mut cobble_random = MtRandom::new(self.post_seed);
        self.cobble_box(&mut cobble_random, 1, 15, 1, 4, 15, 4);

        for z in 0..=5 {
            for x in 0..=5 {
                if x != 0 && x != 5 && z != 0 && z != 5 {
                    continue;
                }
                self.place(GRASS_PATH, 0, x, 11, z);
                self.air_column_up(x, 12, z);
            }
        }
    }

    fn simple_house(&mut self, piece: &VillagePiecePlan, random: &mut MtRandom) {
        let VillagePieceExtra::SimpleHouse { terrace } = piece.extra else {
            unreachable!("simple house has terrace state");
        };
        let mut cobble_random = MtRandom::new(self.post_seed);

        self.cobble_box(&mut cobble_random, 0, 0, 0, 4, 0, 4);
        self.box_fill(0, 4, 0, 4, 4, 4, (LOG, 0), (LOG, 0), false);
        self.box_fill(1, 4, 1, 3, 4, 3, (PLANKS, 0), (PLANKS, 0), false);

        for &(x, z) in &[(0, 0), (4, 0), (0, 4), (4, 4)] {
            for y in 1..=3 {
                self.single_cobble(&cobble_random, x, y, z);
            }
        }
        self.box_fill(0, 1, 1, 0, 3, 3, (PLANKS, 0), (PLANKS, 0), false);
        self.box_fill(4, 1, 1, 4, 3, 3, (PLANKS, 0), (PLANKS, 0), false);
        self.box_fill(1, 1, 4, 3, 3, 4, (PLANKS, 0), (PLANKS, 0), false);
        for (x, y, z) in [(0, 2, 2), (2, 2, 4), (4, 2, 2)] {
            self.place(GLASS_PANE, 0, x, y, z);
        }

        for (x, y) in [(1, 1), (1, 2), (1, 3), (2, 3), (3, 3), (3, 2), (3, 1)] {
            self.place(PLANKS, 0, x, y, 0);
        }
        if self.get(2, 0, -1) == AIR && self.get(2, -1, -1) != AIR {
            self.place(STONE_STAIRS, self.stair_data(3), 2, 0, -1);
        }

        self.air_box(1, 1, 1, 3, 3, 3);
        if self.abandoned {
            self.place(WEB, 0, random.next_int(2) as i32 + 1, 3, 3);
            self.place(WEB, 0, 1, 3, random.next_int(2) as i32 + 1);
        }

        if terrace {
            for x in 0..=4 {
                self.place(FENCE, 0, x, 5, 0);
                self.place(FENCE, 0, x, 5, 4);
            }
            for z in 1..=3 {
                self.place(FENCE, 0, 4, 5, z);
                self.place(FENCE, 0, 0, 5, z);
            }
            let ladder = self.ladder_data(3);
            for y in 1..=4 {
                self.place(65, ladder, 3, y, 3);
            }
        }

        if !self.abandoned {
            self.place(TORCH, self.torch_data(StructureOrientation::South), 2, 3, 1);
        }

        for z in 0..5 {
            for x in 0..5 {
                self.air_column_up(x, 6, z);
                self.fill_column_down(COBBLESTONE, 0, x, -1, z);
            }
        }
    }

    fn small_temple(&mut self, _random: &mut MtRandom) {
        let mut cobble_random = MtRandom::new(self.post_seed);

        self.air_box(1, 1, 1, 3, 3, 7);
        self.air_box(1, 5, 1, 3, 9, 3);
        self.cobble_box(&mut cobble_random, 1, 0, 0, 3, 0, 8);
        self.cobble_box(&mut cobble_random, 1, 1, 0, 3, 10, 0);
        self.cobble_box(&mut cobble_random, 0, 1, 1, 0, 10, 3);
        self.cobble_box(&mut cobble_random, 4, 1, 1, 4, 10, 3);
        self.cobble_box(&mut cobble_random, 0, 0, 4, 0, 4, 7);
        self.cobble_box(&mut cobble_random, 4, 0, 4, 4, 4, 7);
        self.cobble_box(&mut cobble_random, 1, 1, 8, 3, 4, 8);
        self.cobble_box(&mut cobble_random, 1, 5, 4, 3, 10, 4);
        self.cobble_box(&mut cobble_random, 1, 5, 5, 3, 5, 7);
        self.cobble_box(&mut cobble_random, 0, 9, 0, 4, 9, 4);
        self.cobble_box(&mut cobble_random, 0, 4, 0, 4, 4, 4);

        for (x, y, z) in [(0, 11, 2), (4, 11, 2), (2, 11, 0), (2, 11, 4)] {
            self.single_cobble(&cobble_random, x, y, z);
        }
        for (x, y, z) in [(1, 1, 6), (1, 1, 7), (2, 1, 7), (3, 1, 6), (3, 1, 7)] {
            self.single_cobble(&cobble_random, x, y, z);
        }
        for (x, y, z, data) in [
            (1, 1, 5, 3),
            (2, 1, 6, 3),
            (3, 1, 5, 3),
            (1, 2, 7, 1),
            (3, 2, 7, 0),
        ] {
            self.place(STONE_STAIRS, self.stair_data(data), x, y, z);
        }

        for (x, y, z) in [
            (0, 2, 2),
            (0, 3, 2),
            (4, 2, 2),
            (4, 3, 2),
            (0, 6, 2),
            (0, 7, 2),
            (4, 6, 2),
            (4, 7, 2),
            (2, 6, 0),
            (2, 7, 0),
            (2, 6, 4),
            (2, 7, 4),
            (0, 3, 6),
            (4, 3, 6),
            (2, 3, 8),
        ] {
            self.place(GLASS_PANE, 0, x, y, z);
        }

        if !self.abandoned {
            for (x, y, z, dir) in [
                (2, 4, 7, StructureOrientation::North),
                (1, 4, 6, StructureOrientation::West),
                (3, 4, 6, StructureOrientation::East),
                (2, 4, 5, StructureOrientation::South),
            ] {
                self.place(TORCH, self.torch_data(dir), x, y, z);
            }
        }

        let ladder = self.ladder_data(4);
        for y in 1..=9 {
            self.place(LADDER, ladder, 3, y, 3);
        }

        self.place(AIR, 0, 2, 1, 0);
        self.place(AIR, 0, 2, 2, 0);
        if self.get(2, 0, -1) == AIR && self.get(2, -1, -1) != AIR {
            self.place(STONE_STAIRS, self.stair_data(3), 2, 0, -1);
        }
        for z in 0..9 {
            for x in 0..5 {
                self.air_column_up(x, 12, z);
                self.fill_column_down(COBBLESTONE, 0, x, -1, z);
            }
        }
    }

    fn book_house(&mut self, _random: &mut MtRandom) {
        self.air_box(1, 1, 1, 7, 5, 4);
        let mut cobble_random = MtRandom::new(self.post_seed);
        self.cobble_box(&mut cobble_random, 0, 0, 0, 8, 0, 5);
        self.cobble_box(&mut cobble_random, 0, 5, 0, 8, 5, 5);
        self.cobble_box(&mut cobble_random, 0, 6, 1, 8, 6, 4);
        self.cobble_box(&mut cobble_random, 0, 7, 2, 8, 7, 3);

        let south = self.stair_data(3);
        let north = self.stair_data(2);
        for d in -1..=2 {
            for w in 0..=8 {
                self.place(OAK_STAIRS, south, w, 6 + d, d);
                self.place(OAK_STAIRS, north, w, 6 + d, 5 - d);
            }
        }
        if self.abandoned {
            self.place(WEB, 0, 4, 4, -1);
            self.place(WEB, 0, 2, 4, 6);
        }

        for (x0, y0, z0, x1, y1, z1) in [
            (0, 1, 0, 0, 1, 5),
            (1, 1, 5, 8, 1, 5),
            (8, 1, 0, 8, 1, 4),
            (2, 1, 0, 7, 1, 0),
            (0, 2, 0, 0, 4, 0),
            (0, 2, 5, 0, 4, 5),
            (8, 2, 5, 8, 4, 5),
            (8, 2, 0, 8, 4, 0),
        ] {
            self.cobble_box(&mut cobble_random, x0, y0, z0, x1, y1, z1);
        }
        self.box_fill(0, 2, 1, 0, 4, 4, (PLANKS, 0), (PLANKS, 0), false);
        self.box_fill(1, 2, 5, 7, 4, 5, (PLANKS, 0), (PLANKS, 0), false);
        self.box_fill(8, 2, 1, 8, 4, 4, (PLANKS, 0), (PLANKS, 0), false);
        self.box_fill(1, 2, 0, 7, 4, 0, (PLANKS, 0), (PLANKS, 0), false);

        for (x, y, z) in [
            (4, 2, 0),
            (5, 2, 0),
            (6, 2, 0),
            (4, 3, 0),
            (5, 3, 0),
            (6, 3, 0),
            (0, 2, 2),
            (0, 2, 3),
            (0, 3, 2),
            (0, 3, 3),
            (8, 2, 2),
            (8, 2, 3),
            (8, 3, 2),
            (8, 3, 3),
            (2, 2, 5),
            (3, 2, 5),
            (5, 2, 5),
            (6, 2, 5),
        ] {
            self.place(GLASS_PANE, 0, x, y, z);
        }
        self.box_fill(1, 4, 1, 7, 4, 1, (PLANKS, 0), (PLANKS, 0), false);
        self.box_fill(1, 4, 4, 7, 4, 4, (PLANKS, 0), (PLANKS, 0), false);
        self.box_fill(1, 3, 4, 7, 3, 4, (BOOKSHELF, 0), (BOOKSHELF, 0), false);
        if self.abandoned {
            self.place(WEB, 0, 1, 4, 2);
            self.place(WEB, 0, 7, 4, 2);
        }

        self.place(PLANKS, 0, 7, 1, 4);
        self.place(OAK_STAIRS, self.stair_data(0), 7, 1, 3);
        for x in 3..=6 {
            self.place(OAK_STAIRS, south, x, 1, 4);
        }
        self.place(FENCE, 0, 6, 1, 3);
        self.place(FENCE, 0, 4, 1, 3);
        self.place(CRAFTING_TABLE, 0, 7, 1, 1);

        self.place(AIR, 0, 1, 1, 0);
        self.place(AIR, 0, 1, 2, 0);
        if self.get(1, 0, -1) == AIR && self.get(1, -1, -1) != AIR {
            self.place(STONE_STAIRS, self.stair_data(3), 1, 0, -1);
        }
        for z in 0..6 {
            for x in 0..9 {
                self.air_column_up(x, 9, z);
                self.fill_column_down(COBBLESTONE, 0, x, -1, z);
            }
        }
    }

    fn small_hut(&mut self, piece: &VillagePiecePlan, random: &mut MtRandom) {
        let VillagePieceExtra::SmallHut { low_ceiling, table } = piece.extra else {
            unreachable!("small hut has hut constructor state");
        };
        self.air_box(1, 1, 1, 3, 5, 4);
        let mut cobble_random = MtRandom::new(self.post_seed);
        self.cobble_box(&mut cobble_random, 0, 0, 0, 3, 0, 4);
        self.box_fill(1, 0, 1, 2, 0, 3, (DIRT, 0), (DIRT, 0), false);

        let roof_y = if low_ceiling { 4 } else { 5 };
        self.box_fill(1, roof_y, 1, 2, roof_y, 3, (LOG, 0), (LOG, 0), false);
        if self.abandoned {
            let x = if random.next_u32() & 0x0800_0000 != 0 {
                1
            } else {
                2
            };
            let z = if random.next_u32() & 0x0800_0000 != 0 {
                1
            } else {
                3
            };
            self.place(WEB, 0, x, roof_y - 1, z);
        }
        for (x, z) in [(1, 0), (2, 0), (1, 4), (2, 4)] {
            self.place(LOG, 0, x, 4, z);
        }
        for z in 1..=3 {
            self.place(LOG, 0, 0, 4, z);
            self.place(LOG, 0, 3, 4, z);
        }
        for (x, z) in [(0, 0), (3, 0), (0, 4), (3, 4)] {
            self.box_fill(x, 1, z, x, 3, z, (LOG, 0), (LOG, 0), false);
        }
        self.box_fill(0, 1, 1, 0, 3, 3, (PLANKS, 0), (PLANKS, 0), false);
        self.box_fill(3, 1, 1, 3, 3, 3, (PLANKS, 0), (PLANKS, 0), false);
        self.box_fill(1, 1, 0, 2, 3, 0, (PLANKS, 0), (PLANKS, 0), false);
        self.box_fill(1, 1, 4, 2, 3, 4, (PLANKS, 0), (PLANKS, 0), false);
        self.place(GLASS_PANE, 0, 0, 2, 2);
        self.place(GLASS_PANE, 0, 3, 2, 2);
        if table > 0 {
            self.place(FENCE, 0, i32::from(table), 1, 3);
        }
        self.place(AIR, 0, 1, 1, 0);
        self.place(AIR, 0, 1, 2, 0);
        if self.get(1, 0, -1) == AIR && self.get(1, -1, -1) != AIR {
            self.place(STONE_STAIRS, self.stair_data(3), 1, 0, -1);
        }
        for z in 0..5 {
            for x in 0..4 {
                self.air_column_up(x, 6, z);
                self.fill_column_down(COBBLESTONE, 0, x, -1, z);
            }
        }
    }

    fn pig_house(&mut self, _random: &mut MtRandom) {
        self.air_box(1, 1, 1, 7, 4, 4);
        self.air_box(2, 1, 6, 8, 4, 10);
        let mut cobble_random = MtRandom::new(self.post_seed);
        self.box_fill(2, 0, 6, 8, 0, 10, (DIRT, 0), (DIRT, 0), false);
        self.single_cobble(&cobble_random, 6, 0, 6);
        self.box_fill(2, 1, 6, 2, 1, 10, (FENCE, 0), (FENCE, 0), false);
        self.box_fill(8, 1, 6, 8, 1, 10, (FENCE, 0), (FENCE, 0), false);
        self.box_fill(3, 1, 10, 7, 1, 10, (FENCE, 0), (FENCE, 0), false);
        self.box_fill(1, 0, 1, 7, 0, 4, (PLANKS, 0), (PLANKS, 0), false);
        self.cobble_box(&mut cobble_random, 0, 0, 0, 0, 3, 5);
        self.cobble_box(&mut cobble_random, 8, 0, 0, 8, 3, 5);
        self.cobble_box(&mut cobble_random, 1, 0, 0, 7, 1, 0);
        self.cobble_box(&mut cobble_random, 1, 0, 5, 7, 1, 5);
        self.box_fill(1, 2, 0, 7, 3, 0, (PLANKS, 0), (PLANKS, 0), false);
        self.box_fill(1, 2, 5, 7, 3, 5, (PLANKS, 0), (PLANKS, 0), false);
        self.box_fill(0, 4, 1, 8, 4, 1, (PLANKS, 0), (PLANKS, 0), false);
        self.box_fill(0, 4, 4, 8, 4, 4, (PLANKS, 0), (PLANKS, 0), false);
        self.box_fill(0, 5, 2, 8, 5, 3, (PLANKS, 0), (PLANKS, 0), false);
        for (x, z) in [(0, 2), (0, 3), (8, 2), (8, 3)] {
            self.place(PLANKS, 0, x, 4, z);
        }
        let south = self.stair_data(3);
        let north = self.stair_data(2);
        for d in -1..=2 {
            for w in 0..=8 {
                self.place(OAK_STAIRS, south, w, 4 + d, d);
                self.place(OAK_STAIRS, north, w, 4 + d, 5 - d);
            }
        }
        for (x, y, z) in [(0, 2, 1), (0, 2, 4), (8, 2, 1), (8, 2, 4)] {
            self.place(LOG, 0, x, y, z);
        }
        for (x, y, z) in [
            (0, 2, 2),
            (0, 2, 3),
            (8, 2, 2),
            (8, 2, 3),
            (2, 2, 5),
            (3, 2, 5),
            (5, 2, 0),
            (6, 2, 5),
        ] {
            self.place(GLASS_PANE, 0, x, y, z);
        }
        self.place(FENCE, 0, 2, 1, 3);
        self.place(PLANKS, 0, 1, 1, 4);
        self.place(OAK_STAIRS, self.stair_data(3), 2, 1, 4);
        self.place(OAK_STAIRS, self.stair_data(1), 1, 1, 3);
        self.box_fill(
            5,
            0,
            1,
            7,
            0,
            3,
            (DOUBLE_STONE_SLAB, 0),
            (DOUBLE_STONE_SLAB, 0),
            false,
        );
        self.place(DOUBLE_STONE_SLAB, 0, 6, 1, 1);
        self.place(DOUBLE_STONE_SLAB, 0, 6, 1, 2);
        self.place(AIR, 0, 2, 1, 0);
        self.place(AIR, 0, 2, 2, 0);
        if !self.abandoned {
            self.place(TORCH, self.torch_data(StructureOrientation::South), 2, 3, 1);
        }
        if self.get(2, 0, -1) == AIR && self.get(2, -1, -1) != AIR {
            self.place(STONE_STAIRS, self.stair_data(3), 2, 0, -1);
        }
        self.place(AIR, 0, 6, 1, 5);
        self.place(AIR, 0, 6, 2, 5);
        if !self.abandoned {
            self.place(TORCH, self.torch_data(StructureOrientation::North), 6, 3, 4);
        }
        for z in 0..5 {
            for x in 0..9 {
                self.air_column_up(x, 7, z);
                self.fill_column_down(COBBLESTONE, 0, x, -1, z);
            }
        }
    }

    fn farmland(&mut self, piece: &VillagePiecePlan, random: &mut MtRandom) {
        let VillagePieceExtra::Farmland { crops } = piece.extra else {
            unreachable!("farmland has crop constructor state");
        };
        let snow = is_snow_covered(self.current_chunk_biome());
        self.air_box(0, 1, 0, 6, 4, 8);
        let farmland = if snow { DIRT } else { FARMLAND };
        self.box_fill(1, 0, 1, 2, 0, 7, (farmland, 0), (FARMLAND, 0), false);
        self.box_fill(4, 0, 1, 5, 0, 7, (farmland, 0), (FARMLAND, 0), false);
        for (x0, z0, x1, z1) in [(0, 0, 0, 8), (6, 0, 6, 8), (1, 0, 5, 0), (1, 8, 5, 8)] {
            self.box_fill(x0, 0, z0, x1, 0, z1, (LOG, 0), (LOG, 0), false);
        }
        self.box_fill(3, 0, 1, 3, 0, 7, (WATER, 0), (WATER, 0), false);
        if !snow {
            for d in 1..=7 {
                for x in [1, 2] {
                    self.place(crop_id(crops[0]), 2 + random.next_int(5) as u8, x, 1, d);
                }
                for x in [4, 5] {
                    self.place(crop_id(crops[1]), 2 + random.next_int(5) as u8, x, 1, d);
                }
            }
        }
        for z in 0..9 {
            for x in 0..7 {
                self.air_column_up(x, 4, z);
                self.fill_column_down(DIRT, 0, x, -1, z);
            }
        }
    }

    fn double_farmland(&mut self, piece: &VillagePiecePlan, random: &mut MtRandom) {
        let VillagePieceExtra::DoubleFarmland { crops } = piece.extra else {
            unreachable!("double farmland has crop constructor state");
        };
        let snow = is_snow_covered(self.current_chunk_biome());
        self.air_box(0, 1, 0, 12, 4, 8);
        let farmland = if snow { DIRT } else { FARMLAND };
        for (x0, x1) in [(1, 2), (4, 5), (7, 8), (10, 11)] {
            self.box_fill(x0, 0, 1, x1, 0, 7, (farmland, 0), (FARMLAND, 0), false);
        }
        for x in [0, 6, 12] {
            self.box_fill(x, 0, 0, x, 0, 8, (LOG, 0), (LOG, 0), false);
        }
        self.box_fill(1, 0, 0, 11, 0, 0, (LOG, 0), (LOG, 0), false);
        self.box_fill(1, 0, 8, 11, 0, 8, (LOG, 0), (LOG, 0), false);
        for x in [3, 9] {
            self.box_fill(x, 0, 1, x, 0, 7, (WATER, 0), (WATER, 0), false);
        }
        if !snow {
            for d in 1..=7 {
                for (index, xs) in [[1, 2], [4, 5], [7, 8], [10, 11]].iter().enumerate() {
                    for x in xs {
                        self.place(
                            crop_id(crops[index]),
                            2 + random.next_int(5) as u8,
                            *x,
                            1,
                            d,
                        );
                    }
                }
            }
        }
        for z in 0..9 {
            for x in 0..13 {
                self.air_column_up(x, 4, z);
                self.fill_column_down(DIRT, 0, x, -1, z);
            }
        }
    }

    fn smithy(&mut self, piece: &mut VillagePiecePlan, _random: &mut MtRandom) {
        self.air_box(0, 1, 0, 9, 4, 6);
        let mut cobble_random = MtRandom::new(self.post_seed);
        self.cobble_box(&mut cobble_random, 0, 0, 0, 9, 0, 6);
        self.cobble_box(&mut cobble_random, 0, 4, 0, 9, 4, 6);
        self.box_fill(0, 5, 0, 9, 5, 6, (STONE_SLAB, 0), (STONE_SLAB, 0), false);
        self.air_box(1, 5, 1, 8, 5, 5);
        self.box_fill(1, 1, 0, 2, 3, 0, (PLANKS, 0), (PLANKS, 0), false);
        self.box_fill(0, 1, 0, 0, 4, 0, (LOG, 0), (LOG, 0), false);
        self.box_fill(3, 1, 0, 3, 4, 0, (LOG, 0), (LOG, 0), false);
        self.box_fill(0, 1, 6, 0, 4, 6, (LOG, 0), (LOG, 0), false);
        self.place(PLANKS, 0, 3, 3, 1);
        self.box_fill(3, 1, 2, 3, 3, 2, (PLANKS, 0), (PLANKS, 0), false);
        self.box_fill(4, 1, 3, 5, 3, 3, (PLANKS, 0), (PLANKS, 0), false);
        self.box_fill(0, 1, 1, 0, 3, 5, (PLANKS, 0), (PLANKS, 0), false);
        self.box_fill(1, 1, 6, 5, 3, 6, (PLANKS, 0), (PLANKS, 0), false);
        self.box_fill(5, 1, 0, 5, 3, 0, (FENCE, 0), (FENCE, 0), false);
        self.box_fill(9, 1, 0, 9, 3, 0, (FENCE, 0), (FENCE, 0), false);
        self.cobble_box(&mut cobble_random, 6, 1, 4, 9, 4, 6);
        self.place(FLOWING_LAVA, 0, 7, 1, 5);
        self.place(FLOWING_LAVA, 0, 8, 1, 5);
        self.place(IRON_BARS, 0, 9, 2, 5);
        self.place(IRON_BARS, 0, 9, 2, 4);
        self.air_box(7, 2, 4, 8, 2, 5);
        self.single_cobble(&cobble_random, 6, 1, 3);
        let furnace = self.furnace_data();
        self.place(FURNACE, furnace, 6, 2, 3);
        self.place(FURNACE, furnace, 6, 3, 3);
        self.place(DOUBLE_STONE_SLAB, 0, 8, 1, 1);
        for (x, y, z) in [(0, 2, 2), (0, 2, 4), (2, 2, 6), (4, 2, 6)] {
            self.place(GLASS_PANE, 0, x, y, z);
        }
        self.place(FENCE, 0, 2, 1, 4);
        self.place(PLANKS, 0, 1, 1, 5);
        self.place(OAK_STAIRS, self.stair_data(3), 2, 1, 5);
        self.place(OAK_STAIRS, self.stair_data(1), 1, 1, 4);
        if !piece.placed_chest && self.local_inside_chunk(5, 1, 5) {
            // Target createChest body is commented out, but this state flag is still persisted.
            piece.placed_chest = true;
        }
        for x in 6..=8 {
            if self.get(x, 0, -1) == AIR && self.get(x, -1, -1) != AIR {
                self.place(STONE_STAIRS, self.stair_data(3), x, 0, -1);
            }
        }
        for z in 0..7 {
            for x in 0..10 {
                self.air_column_up(x, 6, z);
                self.fill_column_down(COBBLESTONE, 0, x, -1, z);
            }
        }
    }

    fn two_room_house(&mut self, piece: &mut VillagePiecePlan, _random: &mut MtRandom) {
        let mut cobble_random = MtRandom::new(self.post_seed);
        self.air_box(1, 1, 1, 7, 4, 4);
        self.air_box(2, 1, 6, 8, 4, 10);
        self.box_fill(2, 0, 5, 8, 0, 10, (PLANKS, 0), (PLANKS, 0), false);
        self.box_fill(1, 0, 1, 7, 0, 4, (PLANKS, 0), (PLANKS, 0), false);
        for b in [
            (0, 0, 0, 0, 3, 5),
            (8, 0, 0, 8, 3, 10),
            (1, 0, 0, 7, 2, 0),
            (1, 0, 5, 2, 1, 5),
            (2, 0, 6, 2, 3, 10),
            (3, 0, 10, 7, 3, 10),
        ] {
            self.cobble_box(&mut cobble_random, b.0, b.1, b.2, b.3, b.4, b.5);
        }
        for b in [
            (1, 2, 0, 7, 3, 0),
            (1, 2, 5, 2, 3, 5),
            (0, 4, 1, 8, 4, 1),
            (0, 4, 4, 3, 4, 4),
            (0, 5, 2, 8, 5, 3),
        ] {
            self.box_fill(
                b.0,
                b.1,
                b.2,
                b.3,
                b.4,
                b.5,
                (PLANKS, 0),
                (PLANKS, 0),
                false,
            );
        }
        for (x, z) in [(0, 2), (0, 3), (8, 2), (8, 3), (8, 4)] {
            self.place(PLANKS, 0, x, 4, z);
        }
        let south = self.stair_data(3);
        let north = self.stair_data(2);
        for d in -1..=2 {
            for w in 0..=8 {
                self.place(OAK_STAIRS, south, w, 4 + d, d);
                if (d > -1 || w <= 1) && (d > 0 || w <= 3) && (d > 1 || w <= 4 || w >= 6) {
                    self.place(OAK_STAIRS, north, w, 4 + d, 5 - d);
                }
            }
        }
        for b in [
            (3, 4, 5, 3, 4, 10),
            (7, 4, 2, 7, 4, 10),
            (4, 5, 4, 4, 5, 10),
            (6, 5, 4, 6, 5, 10),
            (5, 6, 3, 5, 6, 10),
        ] {
            self.box_fill(
                b.0,
                b.1,
                b.2,
                b.3,
                b.4,
                b.5,
                (PLANKS, 0),
                (PLANKS, 0),
                false,
            );
        }
        if self.abandoned {
            for (x, y, z) in [(5, 5, 3), (5, 5, 4), (5, 5, 10)] {
                self.place(WEB, 0, x, y, z);
            }
        }
        let west = self.stair_data(0);
        for w in (1..5).rev() {
            self.place(PLANKS, 0, w, 2 + w, 7 - w);
            for d in 8 - w..=10 {
                self.place(OAK_STAIRS, west, w, 2 + w, d);
            }
        }
        let east = self.stair_data(1);
        self.place(PLANKS, 0, 6, 6, 3);
        self.place(PLANKS, 0, 7, 5, 4);
        self.place(OAK_STAIRS, east, 6, 6, 4);
        for w in 6..=8 {
            for d in 5..=10 {
                self.place(OAK_STAIRS, east, w, 12 - w, d);
            }
        }
        for (block, x, y, z) in [
            (LOG, 0, 2, 1),
            (LOG, 0, 2, 4),
            (GLASS_PANE, 0, 2, 2),
            (GLASS_PANE, 0, 2, 3),
            (LOG, 4, 2, 0),
            (GLASS_PANE, 5, 2, 0),
            (LOG, 6, 2, 0),
            (LOG, 8, 2, 1),
            (GLASS_PANE, 8, 2, 2),
            (GLASS_PANE, 8, 2, 3),
            (LOG, 8, 2, 4),
            (PLANKS, 8, 2, 5),
            (LOG, 8, 2, 6),
            (GLASS_PANE, 8, 2, 7),
            (GLASS_PANE, 8, 2, 8),
            (LOG, 8, 2, 9),
            (LOG, 2, 2, 6),
            (GLASS_PANE, 2, 2, 7),
            (GLASS_PANE, 2, 2, 8),
            (LOG, 2, 2, 9),
            (LOG, 4, 4, 10),
            (GLASS_PANE, 5, 4, 10),
            (LOG, 6, 4, 10),
            (PLANKS, 5, 5, 10),
        ] {
            self.place(block, 0, x, y, z);
        }
        self.place(AIR, 0, 2, 1, 0);
        self.place(AIR, 0, 2, 2, 0);
        if !self.abandoned {
            self.place(TORCH, self.torch_data(StructureOrientation::South), 2, 3, 1);
        }
        self.air_box(1, 0, -1, 3, 2, -1);
        if self.get(2, 0, -1) == AIR && self.get(2, -1, -1) != AIR {
            self.place(STONE_STAIRS, self.stair_data(3), 2, 0, -1);
        }
        for z in 0..5 {
            for x in 0..9 {
                self.air_column_up(x, 7, z);
                self.fill_column_down(COBBLESTONE, 0, x, -1, z);
            }
        }
        for z in 5..11 {
            for x in 2..9 {
                self.air_column_up(x, 7, z);
                self.fill_column_down(COBBLESTONE, 0, x, -1, z);
            }
        }
        if is_snow_covered(self.current_chunk_biome())
            && !piece.placed_chest
            && self.local_inside_chunk(5, 1, 9)
        {
            piece.placed_chest = true;
        }
    }

    fn light_post(&mut self) {
        self.air_box(0, 0, 0, 2, 3, 1);
        for y in 0..=2 {
            self.place(FENCE, 0, 1, y, 0);
        }
        if !self.abandoned {
            self.place(TORCH, self.torch_data(StructureOrientation::East), 0, 3, 0);
            self.place(TORCH, self.torch_data(StructureOrientation::South), 1, 3, 1);
            self.place(TORCH, self.torch_data(StructureOrientation::West), 2, 3, 0);
            self.place(
                TORCH,
                self.torch_data(StructureOrientation::North),
                1,
                3,
                -1,
            );
        }
    }
}

fn crop_id(crop: super::village_plan::VillageCrop) -> u16 {
    use super::village_plan::VillageCrop;
    match crop {
        VillageCrop::Wheat => WHEAT,
        VillageCrop::Potato => POTATOES,
        VillageCrop::Beetroot => BEETROOT,
        VillageCrop::Carrot => CARROTS,
    }
}

fn is_snow_covered(id: u8) -> bool {
    matches!(id, 10 | 11 | 12 | 13 | 26 | 30 | 31 | 140 | 158)
}

fn biome_block(style: VillageStyle, block: u16, data: u8) -> (u16, u8) {
    match style {
        VillageStyle::Desert => match block {
            LOG | LOG2 => (SANDSTONE, 0),
            COBBLESTONE | MOSSY_COBBLESTONE => (SANDSTONE, 0),
            PLANKS => (SANDSTONE, 2),
            OAK_STAIRS | 67 => (SANDSTONE_STAIRS, data),
            GRASS_PATH => (SANDSTONE, 0),
            _ => (block, data),
        },
        VillageStyle::Savanna => match block {
            LOG | LOG2 => (LOG2, 0),
            PLANKS => (PLANKS, 4),
            OAK_STAIRS => (ACACIA_STAIRS, data),
            FENCE => (FENCE, 4),
            WOODEN_DOOR => (ACACIA_DOOR, data),
            _ => (block, data),
        },
        VillageStyle::Taiga => match block {
            LOG | LOG2 => (LOG, 1),
            PLANKS => (PLANKS, 1),
            OAK_STAIRS => (SPRUCE_STAIRS, data),
            FENCE => (FENCE, 1),
            WOODEN_DOOR => (SPRUCE_DOOR, data),
            _ => (block, data),
        },
        VillageStyle::Plains => (block, data),
    }
}

fn is_liquid(id: u16) -> bool {
    matches!(id, 8..=11)
}

fn chunk_bounds(target: ChunkCoord) -> StructureBounds {
    let x0 = target.x().wrapping_mul(16);
    let z0 = target.z().wrapping_mul(16);
    StructureBounds::new(x0, 1, z0, x0.wrapping_add(15), 512, z0.wrapping_add(15))
}

const STONE_STAIRS: u16 = 67;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn biome_block_adaptation_matches_fixed_target_village_styles() {
        assert_eq!(biome_block(VillageStyle::Desert, PLANKS, 0), (SANDSTONE, 2));
        assert_eq!(
            biome_block(VillageStyle::Desert, GRASS_PATH, 0),
            (SANDSTONE, 0)
        );
        assert_eq!(biome_block(VillageStyle::Savanna, LOG, 0), (LOG2, 0));
        assert_eq!(biome_block(VillageStyle::Savanna, PLANKS, 0), (PLANKS, 4));
        assert_eq!(biome_block(VillageStyle::Taiga, LOG2, 0), (LOG, 1));
        assert_eq!(biome_block(VillageStyle::Taiga, FENCE, 0), (FENCE, 1));
    }

    #[test]
    fn target_01510_road_paints_cross_chunk_path_and_wood_over_water() {
        // MCPE 0.15.10 APK StraightRoad::postProcess (ARM Thumb RVA 0xd6f748)
        // calls getTopSolidBlock and writes GrassPath, or WoodPlanks above liquid.
        let mut neighborhood =
            PopulationNeighborhood::filled(ChunkCoord::new(0, 0), state(AIR, 0), 1);
        for x in 14..=18 {
            for z in 3..=5 {
                neighborhood.set_state(x, 62, z, state(1, 0));
                neighborhood.set_state(x, 63, z, state(GRASS, 0));
            }
        }
        neighborhood.set_state(15, 63, 3, state(WATER, 0));
        neighborhood.set_state(14, 64, 4, state(31, 1)); // tall grass does not block motion
        neighborhood.set_state(16, 70, 4, state(18, 0)); // leaves excluded by target query
        neighborhood.set_state(17, 63, 4, state(FLOWING_LAVA, 0));
        let mut road = VillagePiecePlan {
            kind: VillagePieceKind::StraightRoad,
            bounds: StructureBounds::new(14, 74, 3, 18, 76, 5),
            orientation: StructureOrientation::East,
            gen_depth: 0,
            extra: VillagePieceExtra::StraightRoad { length: 5 },
            height_position: -1,
            placed_chest: false,
        };
        let mut random = MtRandom::new(0);
        process_piece(
            &mut road,
            VillageStyle::Plains,
            false,
            &mut neighborhood,
            chunk_bounds(ChunkCoord::new(0, 0)),
            &mut random,
            0,
        );
        assert_eq!(neighborhood.state(14, 63, 4), Some(state(GRASS_PATH, 0)));
        assert_eq!(neighborhood.state(15, 63, 3), Some(state(PLANKS, 0)));
        assert_eq!(neighborhood.state(16, 63, 4), Some(state(GRASS, 0)));
        process_piece(
            &mut road,
            VillageStyle::Plains,
            false,
            &mut neighborhood,
            chunk_bounds(ChunkCoord::new(1, 0)),
            &mut random,
            0,
        );
        assert_eq!(neighborhood.state(16, 63, 4), Some(state(GRASS_PATH, 0)));
        assert_eq!(neighborhood.state(17, 63, 4), Some(state(PLANKS, 0)));
        assert_eq!(road.height_position, -1);
    }

    #[test]
    fn local_to_world_transform_matches_target_directions() {
        let center = ChunkCoord::new(0, 0);
        for (orientation, expected) in [
            (StructureOrientation::North, (12, 25, 34)),
            (StructureOrientation::South, (12, 25, 34)),
            (StructureOrientation::West, (14, 25, 32)),
            (StructureOrientation::East, (14, 25, 32)),
        ] {
            let mut neighborhood = PopulationNeighborhood::filled(center, state(AIR, 0), 1);
            let writer = VillagePieceWriter {
                neighborhood: &mut neighborhood,
                chunk_box: StructureBounds::new(-128, 1, -128, 128, 512, 128),
                bounds: StructureBounds::new(10, 20, 30, 18, 40, 38),
                orientation,
                style: VillageStyle::Plains,
                abandoned: false,
                post_seed: 0,
            };
            assert_eq!(
                (
                    writer.world_x(2, 4),
                    writer.world_y(5),
                    writer.world_z(2, 4)
                ),
                expected,
            );
        }
    }
}
