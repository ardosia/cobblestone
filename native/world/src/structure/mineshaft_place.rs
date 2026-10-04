use crate::ChunkCoord;
use crate::population::{PopulationNeighborhood, state};
use crate::terrain_shape::noise::MtRandom;

use super::mineshaft_plan::{
    MineshaftPieceExtra, MineshaftPieceKind, MineshaftPiecePlan, MineshaftPlan,
};
use super::{StructureBounds, StructureOrientation};

const AIR: u16 = 0;
const DIRT: u16 = 3;
const PLANKS: u16 = 5;
const WEB: u16 = 30;
const TORCH: u16 = 50;
const MOB_SPAWNER: u16 = 52;
const RAIL: u16 = 66;
const FENCE: u16 = 85;

const SHAFT_WIDTH: i32 = 3;
const SHAFT_HEIGHT: i32 = 3;
const SHAFT_LENGTH: i32 = 5;

pub(crate) struct MineshaftPostProcessor;

impl MineshaftPostProcessor {
    pub(crate) fn process_start(
        plan: &mut MineshaftPlan,
        neighborhood: &mut PopulationNeighborhood,
        target: ChunkCoord,
        random: &mut MtRandom,
    ) -> bool {
        if !plan.core.should_post_process(target) {
            return false;
        }

        let chunk_box = chunk_bounds(target);
        let mut processed = false;
        let mut index = 0;
        while index < plan.pieces.len() {
            if !plan.pieces[index].bounds.intersects(chunk_box) {
                index += 1;
                continue;
            }

            let ok = process_piece(
                &mut plan.pieces[index],
                plan.surface,
                neighborhood,
                chunk_box,
                random,
            );
            if !ok {
                plan.pieces.remove(index);
                continue;
            }
            processed = true;
            index += 1;
        }

        plan.core.mark_post_processed(target);
        processed
    }
}

fn process_piece(
    piece: &mut MineshaftPiecePlan,
    surface: bool,
    neighborhood: &mut PopulationNeighborhood,
    chunk_box: StructureBounds,
    random: &mut MtRandom,
) -> bool {
    if edges_liquid(neighborhood, piece.bounds, chunk_box) {
        return false;
    }

    match piece.kind {
        MineshaftPieceKind::Room => room(piece, neighborhood, chunk_box),
        MineshaftPieceKind::Corridor => corridor(piece, surface, neighborhood, chunk_box, random),
        MineshaftPieceKind::Crossing => crossing(piece, surface, neighborhood, chunk_box),
        MineshaftPieceKind::Stairs => stairs(piece, neighborhood, chunk_box),
    }
    true
}

fn room(
    piece: &MineshaftPiecePlan,
    neighborhood: &mut PopulationNeighborhood,
    chunk_box: StructureBounds,
) {
    let MineshaftPieceExtra::Room { entrances } = &piece.extra else {
        unreachable!();
    };
    let b = piece.bounds;
    world_box(
        neighborhood,
        chunk_box,
        b.x0,
        b.y0,
        b.z0,
        b.x1,
        b.y0,
        b.z1,
        (DIRT, 0),
        (AIR, 0),
        true,
    );
    world_box(
        neighborhood,
        chunk_box,
        b.x0,
        b.y0 + 1,
        b.z0,
        b.x1,
        (b.y0 + 3).min(b.y1),
        b.z1,
        (AIR, 0),
        (AIR, 0),
        false,
    );
    for entrance in entrances {
        world_box(
            neighborhood,
            chunk_box,
            entrance.x0,
            entrance.y1 - (SHAFT_HEIGHT - 1),
            entrance.z0,
            entrance.x1,
            entrance.y1,
            entrance.z1,
            (AIR, 0),
            (AIR, 0),
            false,
        );
    }
    upper_half_sphere(
        neighborhood,
        chunk_box,
        b.x0,
        b.y0 + 4,
        b.z0,
        b.x1,
        b.y1,
        b.z1,
    );
}

fn corridor(
    piece: &mut MineshaftPiecePlan,
    surface: bool,
    neighborhood: &mut PopulationNeighborhood,
    chunk_box: StructureBounds,
    random: &mut MtRandom,
) {
    let MineshaftPieceExtra::Corridor {
        has_rails,
        spider_corridor,
        has_placed_spider,
        num_sections,
    } = &mut piece.extra
    else {
        unreachable!();
    };

    let length = *num_sections * SHAFT_LENGTH - 1;
    let mut writer = OrientedWriter::new(neighborhood, chunk_box, piece.bounds, piece.orientation);

    writer.box_fill(
        0,
        0,
        0,
        SHAFT_WIDTH - 1,
        SHAFT_HEIGHT - 2,
        length,
        (AIR, 0),
        (AIR, 0),
        false,
    );
    writer.maybe_box(
        random,
        0.8,
        0,
        SHAFT_HEIGHT - 1,
        0,
        SHAFT_WIDTH - 1,
        SHAFT_HEIGHT - 1,
        length,
        (AIR, 0),
        (AIR, 0),
        false,
    );

    if *spider_corridor {
        writer.maybe_box(
            random,
            0.6,
            0,
            0,
            0,
            SHAFT_WIDTH - 1,
            SHAFT_HEIGHT - 2,
            length,
            (WEB, 0),
            (AIR, 0),
            false,
        );
    }

    for section in 0..*num_sections {
        let z = 2 + section * SHAFT_LENGTH;
        writer.place_support(random, surface, z);

        writer.place_cobweb(random, 0.1, 0, SHAFT_HEIGHT - 1, z);
        writer.place_cobweb(random, 0.1, SHAFT_WIDTH - 1, SHAFT_HEIGHT - 1, z - 1);
        writer.place_cobweb(random, 0.1, 0, SHAFT_HEIGHT - 1, z + 1);
        writer.place_cobweb(random, 0.1, SHAFT_WIDTH - 1, SHAFT_HEIGHT - 1, z + 1);
        writer.place_cobweb(random, 0.05, 0, SHAFT_HEIGHT - 1, z - 2);
        writer.place_cobweb(random, 0.05, SHAFT_WIDTH - 1, SHAFT_HEIGHT - 1, z - 2);
        writer.place_cobweb(random, 0.05, 0, SHAFT_HEIGHT - 1, z + 2);
        writer.place_cobweb(random, 0.05, SHAFT_WIDTH - 1, SHAFT_HEIGHT - 1, z + 2);

        if *spider_corridor && !*has_placed_spider {
            let local_z = z - 1 + random.next_int(3) as i32;
            let (world_x, world_y, world_z) = writer.world_pos(1, 0, local_z);
            if chunk_box.contains(world_x, world_y, world_z) {
                *has_placed_spider = true;
                writer.place_local(MOB_SPAWNER, 0, 1, 0, local_z);
            }
        }
    }

    let wood_data = if surface { 5 } else { 0 };
    for x in 0..SHAFT_WIDTH {
        for z in 0..=length {
            if writer.get_local(x, -1, z) == AIR {
                writer.place_local(PLANKS, wood_data, x, -1, z);
            }
        }
    }

    if *has_rails {
        for z in 0..=length {
            let floor = writer.get_local(1, -1, z);
            if is_solid_for_rail(floor) {
                let data = if matches!(
                    piece.orientation,
                    StructureOrientation::West | StructureOrientation::East
                ) {
                    1
                } else {
                    0
                };
                writer.maybe_block(random, 0.7, RAIL, data, 1, 0, z);
            }
        }
    }
}

fn crossing(
    piece: &MineshaftPiecePlan,
    surface: bool,
    neighborhood: &mut PopulationNeighborhood,
    chunk_box: StructureBounds,
) {
    let MineshaftPieceExtra::Crossing { two_floored, .. } = piece.extra else {
        unreachable!();
    };
    let b = piece.bounds;

    if two_floored {
        world_box(
            neighborhood,
            chunk_box,
            b.x0 + 1,
            b.y0,
            b.z0,
            b.x1 - 1,
            b.y0 + SHAFT_HEIGHT - 1,
            b.z1,
            (AIR, 0),
            (AIR, 0),
            false,
        );
        world_box(
            neighborhood,
            chunk_box,
            b.x0,
            b.y0,
            b.z0 + 1,
            b.x1,
            b.y0 + SHAFT_HEIGHT - 1,
            b.z1 - 1,
            (AIR, 0),
            (AIR, 0),
            false,
        );
        world_box(
            neighborhood,
            chunk_box,
            b.x0 + 1,
            b.y1 - (SHAFT_HEIGHT - 1),
            b.z0,
            b.x1 - 1,
            b.y1,
            b.z1,
            (AIR, 0),
            (AIR, 0),
            false,
        );
        world_box(
            neighborhood,
            chunk_box,
            b.x0,
            b.y1 - (SHAFT_HEIGHT - 1),
            b.z0 + 1,
            b.x1,
            b.y1,
            b.z1 - 1,
            (AIR, 0),
            (AIR, 0),
            false,
        );
        world_box(
            neighborhood,
            chunk_box,
            b.x0 + 1,
            b.y0 + SHAFT_HEIGHT,
            b.z0 + 1,
            b.x1 - 1,
            b.y0 + SHAFT_HEIGHT,
            b.z1 - 1,
            (AIR, 0),
            (AIR, 0),
            false,
        );
    } else {
        world_box(
            neighborhood,
            chunk_box,
            b.x0 + 1,
            b.y0,
            b.z0,
            b.x1 - 1,
            b.y1,
            b.z1,
            (AIR, 0),
            (AIR, 0),
            false,
        );
        world_box(
            neighborhood,
            chunk_box,
            b.x0,
            b.y0,
            b.z0 + 1,
            b.x1,
            b.y1,
            b.z1 - 1,
            (AIR, 0),
            (AIR, 0),
            false,
        );
    }

    let wood_data = if surface { 5 } else { 0 };
    for (x, z) in [
        (b.x0 + 1, b.z0 + 1),
        (b.x0 + 1, b.z1 - 1),
        (b.x1 - 1, b.z0 + 1),
        (b.x1 - 1, b.z1 - 1),
    ] {
        if get_world(neighborhood, chunk_box, x, b.y1 + 1, z) != AIR {
            world_box(
                neighborhood,
                chunk_box,
                x,
                b.y0,
                z,
                x,
                b.y1,
                z,
                (PLANKS, wood_data),
                (AIR, 0),
                false,
            );
        }
    }

    for x in b.x0..=b.x1 {
        for z in b.z0..=b.z1 {
            if get_world(neighborhood, chunk_box, x, b.y0 - 1, z) == AIR {
                place_world(neighborhood, chunk_box, PLANKS, wood_data, x, b.y0 - 1, z);
            }
        }
    }
}

fn stairs(
    piece: &MineshaftPiecePlan,
    neighborhood: &mut PopulationNeighborhood,
    chunk_box: StructureBounds,
) {
    let mut writer = OrientedWriter::new(neighborhood, chunk_box, piece.bounds, piece.orientation);
    writer.box_fill(
        0,
        5,
        0,
        SHAFT_WIDTH - 1,
        5 + SHAFT_HEIGHT - 1,
        1,
        (AIR, 0),
        (AIR, 0),
        false,
    );
    writer.box_fill(
        0,
        0,
        7,
        SHAFT_WIDTH - 1,
        SHAFT_HEIGHT - 1,
        8,
        (AIR, 0),
        (AIR, 0),
        false,
    );
    for step in 0..5 {
        writer.box_fill(
            0,
            5 - step - i32::from(step < 4),
            2 + step,
            SHAFT_WIDTH - 1,
            5 + SHAFT_HEIGHT - 1 - step,
            2 + step,
            (AIR, 0),
            (AIR, 0),
            false,
        );
    }
}

struct OrientedWriter<'a> {
    neighborhood: &'a mut PopulationNeighborhood,
    chunk_box: StructureBounds,
    bounds: StructureBounds,
    orientation: StructureOrientation,
}

impl<'a> OrientedWriter<'a> {
    fn new(
        neighborhood: &'a mut PopulationNeighborhood,
        chunk_box: StructureBounds,
        bounds: StructureBounds,
        orientation: StructureOrientation,
    ) -> Self {
        Self {
            neighborhood,
            chunk_box,
            bounds,
            orientation,
        }
    }

    fn world_pos(&self, x: i32, y: i32, z: i32) -> (i32, i32, i32) {
        let world_x = match self.orientation {
            StructureOrientation::North | StructureOrientation::South => self.bounds.x0 + x,
            StructureOrientation::West => self.bounds.x1 - z,
            StructureOrientation::East => self.bounds.x0 + z,
        };
        let world_z = match self.orientation {
            StructureOrientation::North => self.bounds.z1 - z,
            StructureOrientation::South => self.bounds.z0 + z,
            StructureOrientation::West | StructureOrientation::East => self.bounds.z0 + x,
        };
        (world_x, self.bounds.y0 + y, world_z)
    }

    fn get_local(&self, x: i32, y: i32, z: i32) -> u16 {
        let (wx, wy, wz) = self.world_pos(x, y, z);
        get_world(self.neighborhood, self.chunk_box, wx, wy, wz)
    }

    fn place_local(&mut self, block: u16, data: u8, x: i32, y: i32, z: i32) {
        let (wx, wy, wz) = self.world_pos(x, y, z);
        place_world(self.neighborhood, self.chunk_box, block, data, wx, wy, wz);
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
                    let block = if y == y0 || y == y1 || x == x0 || x == x1 || z == z0 || z == z1 {
                        edge
                    } else {
                        fill
                    };
                    self.place_local(block.0, block.1, x, y, z);
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn maybe_box(
        &mut self,
        random: &mut MtRandom,
        probability: f32,
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
                    if random.next_float() > probability {
                        continue;
                    }
                    if skip_air && self.get_local(x, y, z) == AIR {
                        continue;
                    }
                    let block = if y == y0 || y == y1 || x == x0 || x == x1 || z == z0 || z == z1 {
                        edge
                    } else {
                        fill
                    };
                    self.place_local(block.0, block.1, x, y, z);
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn maybe_block(
        &mut self,
        random: &mut MtRandom,
        probability: f32,
        block: u16,
        data: u8,
        x: i32,
        y: i32,
        z: i32,
    ) {
        if random.next_float() < probability {
            self.place_local(block, data, x, y, z);
        }
    }

    fn place_cobweb(&mut self, random: &mut MtRandom, probability: f32, x: i32, y: i32, z: i32) {
        self.maybe_block(random, probability, WEB, 0, x, y, z - 1);
    }

    fn place_support(&mut self, random: &mut MtRandom, surface: bool, z: i32) {
        if !self.supporting_box(0, SHAFT_WIDTH - 1, SHAFT_HEIGHT - 1, z) {
            return;
        }
        let wood_data = if surface { 5 } else { 0 };
        self.box_fill(
            0,
            0,
            z,
            0,
            SHAFT_HEIGHT - 2,
            z,
            (FENCE, wood_data),
            (AIR, 0),
            false,
        );
        self.box_fill(
            SHAFT_WIDTH - 1,
            0,
            z,
            SHAFT_WIDTH - 1,
            SHAFT_HEIGHT - 2,
            z,
            (FENCE, wood_data),
            (AIR, 0),
            false,
        );
        if random.next_int(4) == 0 {
            self.place_local(PLANKS, wood_data, 0, SHAFT_HEIGHT - 1, z);
            self.place_local(PLANKS, wood_data, SHAFT_WIDTH - 1, SHAFT_HEIGHT - 1, z);
        } else {
            self.box_fill(
                0,
                SHAFT_HEIGHT - 1,
                z,
                SHAFT_WIDTH - 1,
                SHAFT_HEIGHT - 1,
                z,
                (PLANKS, wood_data),
                (AIR, 0),
                false,
            );
            self.maybe_block(
                random,
                0.05,
                TORCH,
                self.torch_data(StructureOrientation::North),
                1,
                SHAFT_HEIGHT - 1,
                z - 1,
            );
            self.maybe_block(
                random,
                0.05,
                TORCH,
                self.torch_data(StructureOrientation::South),
                1,
                SHAFT_HEIGHT - 1,
                z + 1,
            );
        }
    }

    fn supporting_box(&self, x0: i32, x1: i32, y: i32, z: i32) -> bool {
        let (min_x, min_y, min_z) = self.world_pos(x0, y + 1, z);
        let (max_x, _, max_z) = self.world_pos(x1, y + 1, z);
        for x in min_x..=max_x {
            for z in min_z..=max_z {
                if self.neighborhood.block_id(x, min_y, z) == AIR {
                    return false;
                }
            }
        }
        true
    }

    fn torch_data(&self, direction: StructureOrientation) -> u8 {
        match (self.orientation, direction) {
            (StructureOrientation::North, StructureOrientation::North) => 3,
            (StructureOrientation::North, StructureOrientation::South) => 4,
            (StructureOrientation::North, StructureOrientation::West) => 1,
            (StructureOrientation::North, StructureOrientation::East) => 2,
            (StructureOrientation::South, StructureOrientation::North) => 4,
            (StructureOrientation::South, StructureOrientation::South) => 3,
            (StructureOrientation::South, StructureOrientation::West) => 1,
            (StructureOrientation::South, StructureOrientation::East) => 2,
            (StructureOrientation::West, StructureOrientation::North) => 1,
            (StructureOrientation::West, StructureOrientation::South) => 2,
            (StructureOrientation::West, StructureOrientation::West) => 3,
            (StructureOrientation::West, StructureOrientation::East) => 4,
            (StructureOrientation::East, StructureOrientation::North) => 2,
            (StructureOrientation::East, StructureOrientation::South) => 1,
            (StructureOrientation::East, StructureOrientation::West) => 3,
            (StructureOrientation::East, StructureOrientation::East) => 4,
        }
    }
}

fn edges_liquid(
    neighborhood: &PopulationNeighborhood,
    bounds: StructureBounds,
    chunk_box: StructureBounds,
) -> bool {
    let x0 = (bounds.x0 - 1).max(chunk_box.x0);
    let y0 = (bounds.y0 - 1).max(chunk_box.y0);
    let z0 = (bounds.z0 - 1).max(chunk_box.z0);
    let x1 = (bounds.x1 + 1).min(chunk_box.x1);
    let y1 = (bounds.y1 + 1).min(chunk_box.y1);
    let z1 = (bounds.z1 + 1).min(chunk_box.z1);

    for x in x0..=x1 {
        for z in z0..=z1 {
            if is_liquid(neighborhood.block_id(x, y0, z))
                || is_liquid(neighborhood.block_id(x, y1, z))
            {
                return true;
            }
        }
    }
    for x in x0..=x1 {
        for y in y0..=y1 {
            if is_liquid(neighborhood.block_id(x, y, z0))
                || is_liquid(neighborhood.block_id(x, y, z1))
            {
                return true;
            }
        }
    }
    for z in z0..=z1 {
        for y in y0..=y1 {
            if is_liquid(neighborhood.block_id(x0, y, z))
                || is_liquid(neighborhood.block_id(x1, y, z))
            {
                return true;
            }
        }
    }
    false
}

#[allow(clippy::too_many_arguments)]
fn world_box(
    neighborhood: &mut PopulationNeighborhood,
    chunk_box: StructureBounds,
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
                if skip_air && get_world(neighborhood, chunk_box, x, y, z) == AIR {
                    continue;
                }
                let block = if y == y0 || y == y1 || x == x0 || x == x1 || z == z0 || z == z1 {
                    edge
                } else {
                    fill
                };
                place_world(neighborhood, chunk_box, block.0, block.1, x, y, z);
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn upper_half_sphere(
    neighborhood: &mut PopulationNeighborhood,
    chunk_box: StructureBounds,
    x0: i32,
    y0: i32,
    z0: i32,
    x1: i32,
    y1: i32,
    z1: i32,
) {
    let dx = (x1 - x0 + 1) as f32;
    let dy = (y1 - y0 + 1) as f32;
    let dz = (z1 - z0 + 1) as f32;
    let cx = x0 as f32 + dx / 2.0;
    let cz = z0 as f32 + dz / 2.0;

    for y in y0..=y1 {
        let ny = (y - y0) as f32 / dy;
        for x in x0..=x1 {
            let nx = (x as f32 - cx) / (dx * 0.5);
            for z in z0..=z1 {
                let nz = (z as f32 - cz) / (dz * 0.5);
                if nx * nx + ny * ny + nz * nz <= 1.05 {
                    place_world(neighborhood, chunk_box, AIR, 0, x, y, z);
                }
            }
        }
    }
}

fn get_world(
    neighborhood: &PopulationNeighborhood,
    chunk_box: StructureBounds,
    x: i32,
    y: i32,
    z: i32,
) -> u16 {
    if !chunk_box.contains(x, y, z) {
        return AIR;
    }
    neighborhood.block_id(x, y, z)
}

fn place_world(
    neighborhood: &mut PopulationNeighborhood,
    chunk_box: StructureBounds,
    block: u16,
    data: u8,
    x: i32,
    y: i32,
    z: i32,
) {
    if chunk_box.contains(x, y, z) {
        neighborhood.set_state(x, y, z, state(block, data));
    }
}

fn is_liquid(id: u16) -> bool {
    matches!(id, 8..=11)
}

fn is_solid_for_rail(id: u16) -> bool {
    id != AIR && !is_liquid(id) && !matches!(id, WEB | TORCH | RAIL)
}

fn chunk_bounds(target: ChunkCoord) -> StructureBounds {
    let x0 = target.x().wrapping_mul(16);
    let z0 = target.z().wrapping_mul(16);
    StructureBounds::new(x0, 1, z0, x0.wrapping_add(15), 512, z0.wrapping_add(15))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stone_neighborhood() -> PopulationNeighborhood {
        PopulationNeighborhood::filled(ChunkCoord::new(0, 0), state(1, 0), 1)
    }

    fn single_piece_plan(piece: MineshaftPiecePlan, surface: bool) -> MineshaftPlan {
        MineshaftPlan {
            core: crate::structure::StructureStartCore::new(ChunkCoord::new(0, 0), piece.bounds),
            surface,
            pieces: vec![piece],
        }
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

    fn process_fixture(
        piece: MineshaftPiecePlan,
        surface: bool,
        random_seed: u32,
    ) -> PopulationNeighborhood {
        let mut neighborhood = stone_neighborhood();
        let mut plan = single_piece_plan(piece, surface);
        let mut random = MtRandom::new(random_seed);
        assert!(MineshaftPostProcessor::process_start(
            &mut plan,
            &mut neighborhood,
            ChunkCoord::new(0, 0),
            &mut random,
        ));
        neighborhood
    }

    #[test]
    fn independent_room_block_fixture_matches() {
        let piece = MineshaftPiecePlan {
            kind: MineshaftPieceKind::Room,
            bounds: StructureBounds::new(2, 20, 2, 10, 25, 10),
            orientation: StructureOrientation::South,
            gen_depth: 0,
            extra: MineshaftPieceExtra::Room {
                entrances: vec![StructureBounds::new(4, 21, 2, 6, 23, 3)],
            },
        };
        let neighborhood = process_fixture(piece, false, 0x1234_5678);
        assert_eq!(center_hash(&neighborhood), 0x3565_90a7_f938_8e75);
        assert_eq!(block_count(&neighborhood, AIR), 359);
        assert_eq!(block_count(&neighborhood, DIRT), 81);
    }

    #[test]
    fn independent_rail_corridor_block_fixture_matches() {
        let piece = MineshaftPiecePlan {
            kind: MineshaftPieceKind::Corridor,
            bounds: StructureBounds::new(4, 20, 1, 6, 22, 10),
            orientation: StructureOrientation::South,
            gen_depth: 1,
            extra: MineshaftPieceExtra::Corridor {
                has_rails: true,
                spider_corridor: false,
                has_placed_spider: false,
                num_sections: 2,
            },
        };
        let neighborhood = process_fixture(piece, false, 0x1234_5678);
        assert_eq!(center_hash(&neighborhood), 0x4f51_f5f8_0585_29da);
        assert_eq!(block_count(&neighborhood, PLANKS), 6);
        assert_eq!(block_count(&neighborhood, FENCE), 8);
        assert_eq!(block_count(&neighborhood, WEB), 3);
        assert_eq!(block_count(&neighborhood, RAIL), 8);
    }

    #[test]
    fn independent_spider_corridor_block_fixture_matches() {
        let piece = MineshaftPiecePlan {
            kind: MineshaftPieceKind::Corridor,
            bounds: StructureBounds::new(4, 20, 1, 6, 22, 10),
            orientation: StructureOrientation::South,
            gen_depth: 1,
            extra: MineshaftPieceExtra::Corridor {
                has_rails: false,
                spider_corridor: true,
                has_placed_spider: false,
                num_sections: 2,
            },
        };
        let neighborhood = process_fixture(piece, false, 0x1234_5678);
        assert_eq!(center_hash(&neighborhood), 0xcdb5_abb3_ac87_2cd7);
        assert_eq!(block_count(&neighborhood, WEB), 31);
        assert_eq!(block_count(&neighborhood, MOB_SPAWNER), 1);
    }

    #[test]
    fn independent_surface_crossing_block_fixture_matches() {
        let piece = MineshaftPiecePlan {
            kind: MineshaftPieceKind::Crossing,
            bounds: StructureBounds::new(5, 20, 5, 9, 26, 9),
            orientation: StructureOrientation::South,
            gen_depth: 1,
            extra: MineshaftPieceExtra::Crossing {
                direction: StructureOrientation::South,
                two_floored: true,
            },
        };
        let neighborhood = process_fixture(piece, true, 0x1234_5678);
        assert_eq!(center_hash(&neighborhood), 0xc6a6_fc86_cdcb_aa55);
        assert_eq!(block_count(&neighborhood, AIR), 107);
        assert_eq!(block_count(&neighborhood, PLANKS), 28);
        assert_eq!(
            neighborhood
                .center_states()
                .iter()
                .filter(|value| (**value >> 4) == PLANKS && (**value & 0x0f) == 5)
                .count(),
            28,
        );
    }

    #[test]
    fn independent_rotated_stairs_block_fixture_matches() {
        let piece = MineshaftPiecePlan {
            kind: MineshaftPieceKind::Stairs,
            bounds: StructureBounds::new(2, 15, 4, 10, 22, 6),
            orientation: StructureOrientation::East,
            gen_depth: 1,
            extra: MineshaftPieceExtra::Stairs,
        };
        let neighborhood = process_fixture(piece, false, 0x1234_5678);
        assert_eq!(center_hash(&neighborhood), 0x576c_e847_80f9_b795);
        assert_eq!(block_count(&neighborhood, AIR), 93);
    }

    #[test]
    fn liquid_abort_erases_piece_and_empty_start_round_trips() {
        let center = ChunkCoord::new(0, 0);
        let mut neighborhood = stone_neighborhood();
        neighborhood.set_state(1, 19, 1, state(9, 0));

        let piece = MineshaftPiecePlan {
            kind: MineshaftPieceKind::Room,
            bounds: StructureBounds::new(2, 20, 2, 5, 23, 5),
            orientation: StructureOrientation::South,
            gen_depth: 0,
            extra: MineshaftPieceExtra::Room {
                entrances: Vec::new(),
            },
        };
        let mut plan = single_piece_plan(piece, false);
        let mut random = MtRandom::new(0);
        assert!(!MineshaftPostProcessor::process_start(
            &mut plan,
            &mut neighborhood,
            center,
            &mut random,
        ));
        assert!(plan.pieces.is_empty());

        let mut state_bytes = Vec::new();
        super::super::mineshaft::encode_plan(&plan, &mut state_bytes);
        let mut cursor = super::super::mineshaft::Cursor::new(&state_bytes);
        let decoded = super::super::mineshaft::decode_plan(&mut cursor)
            .expect("empty target-valid Mineshaft start");
        assert!(decoded.pieces.is_empty());
        assert!(cursor.is_finished());
    }

    #[test]
    fn spider_placement_mutable_state_survives_piece_codec() {
        let piece = MineshaftPiecePlan {
            kind: MineshaftPieceKind::Corridor,
            bounds: StructureBounds::new(4, 20, 1, 6, 22, 10),
            orientation: StructureOrientation::South,
            gen_depth: 1,
            extra: MineshaftPieceExtra::Corridor {
                has_rails: false,
                spider_corridor: true,
                has_placed_spider: false,
                num_sections: 2,
            },
        };
        let mut neighborhood = stone_neighborhood();
        let mut plan = single_piece_plan(piece, false);
        let mut random = MtRandom::new(0x1234_5678);
        assert!(MineshaftPostProcessor::process_start(
            &mut plan,
            &mut neighborhood,
            ChunkCoord::new(0, 0),
            &mut random,
        ));
        let MineshaftPieceExtra::Corridor {
            has_placed_spider, ..
        } = plan.pieces[0].extra
        else {
            unreachable!();
        };
        assert!(has_placed_spider);

        let mut bytes = Vec::new();
        super::super::mineshaft::encode_plan(&plan, &mut bytes);
        let mut cursor = super::super::mineshaft::Cursor::new(&bytes);
        let decoded = super::super::mineshaft::decode_plan(&mut cursor).expect("valid plan");
        let MineshaftPieceExtra::Corridor {
            has_placed_spider, ..
        } = decoded.pieces[0].extra
        else {
            unreachable!();
        };
        assert!(has_placed_spider);
    }

    #[test]
    fn spanning_corridor_processes_each_intersecting_chunk_once() {
        let piece = MineshaftPiecePlan {
            kind: MineshaftPieceKind::Corridor,
            bounds: StructureBounds::new(14, 20, 4, 23, 22, 6),
            orientation: StructureOrientation::East,
            gen_depth: 1,
            extra: MineshaftPieceExtra::Corridor {
                has_rails: false,
                spider_corridor: false,
                has_placed_spider: false,
                num_sections: 2,
            },
        };
        let mut plan = single_piece_plan(piece, false);

        let mut left = PopulationNeighborhood::filled(ChunkCoord::new(0, 0), state(1, 0), 1);
        let mut left_random = MtRandom::new(1);
        assert!(MineshaftPostProcessor::process_start(
            &mut plan,
            &mut left,
            ChunkCoord::new(0, 0),
            &mut left_random,
        ));
        assert!(block_count(&left, AIR) > 0);

        let mut left_again = left.clone();
        let mut left_again_random = MtRandom::new(1);
        assert!(!MineshaftPostProcessor::process_start(
            &mut plan,
            &mut left_again,
            ChunkCoord::new(0, 0),
            &mut left_again_random,
        ));
        assert_eq!(left_again, left);

        let mut right = PopulationNeighborhood::filled(ChunkCoord::new(1, 0), state(1, 0), 1);
        let mut right_random = MtRandom::new(2);
        assert!(MineshaftPostProcessor::process_start(
            &mut plan,
            &mut right,
            ChunkCoord::new(1, 0),
            &mut right_random,
        ));
        assert!(block_count(&right, AIR) > 0);
    }

    #[test]
    fn liquid_edge_aborts_piece_before_mutation() {
        let center = ChunkCoord::new(0, 0);
        let mut neighborhood = PopulationNeighborhood::filled(center, state(1, 0), 1);
        neighborhood.set_state(0, 20, 0, state(9, 0));
        let bounds = StructureBounds::new(1, 19, 1, 4, 22, 4);
        assert!(edges_liquid(&neighborhood, bounds, chunk_bounds(center)));
    }
}
