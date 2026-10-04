use crate::ChunkCoord;
use crate::population::{PopulationNeighborhood, state};
use crate::terrain_shape::noise::MtRandom;

use super::stronghold_plan::{
    StrongholdDoor, StrongholdPieceExtra, StrongholdPieceKind, StrongholdPiecePlan, StrongholdPlan,
};
use super::{StructureBounds, StructureOrientation};

const AIR: u16 = 0;
const COBBLESTONE: u16 = 4;
const PLANKS: u16 = 5;
const FLOWING_WATER: u16 = 8;
const LAVA: u16 = 11;
const WEB: u16 = 30;
const DOUBLE_STONE_SLAB: u16 = 43;
const STONE_SLAB: u16 = 44;
const BOOKSHELF: u16 = 47;
const TORCH: u16 = 50;
const MOB_SPAWNER: u16 = 52;
const WOODEN_DOOR: u16 = 64;
const LADDER: u16 = 65;
const STONE_STAIRS: u16 = 67;
const IRON_DOOR: u16 = 71;
const FENCE: u16 = 85;
const MONSTER_EGG: u16 = 97;
const STONE_BRICK: u16 = 98;
const IRON_BARS: u16 = 101;
const STONE_BRICK_STAIRS: u16 = 109;
const END_PORTAL: u16 = 119;
const END_PORTAL_FRAME: u16 = 120;
const WOOD_BUTTON: u16 = 143;

const STONE_BRICK_SLAB_DATA: u8 = 5;
const MONSTER_EGG_STONE_BRICK_DATA: u8 = 2;
const DOOR_UPPER_DATA: u8 = 8;
const END_PORTAL_EYE_BIT: u8 = 4;

pub(crate) struct StrongholdPostProcessor;

impl StrongholdPostProcessor {
    pub(crate) fn process_start(
        plan: &mut StrongholdPlan,
        neighborhood: &mut PopulationNeighborhood,
        target: ChunkCoord,
        random: &mut MtRandom,
    ) -> bool {
        if !plan.core.should_post_process(target) {
            return false;
        }

        let chunk_box = chunk_bounds(target);
        let mut changed = false;
        for piece in &mut plan.pieces {
            if !piece.bounds.intersects(chunk_box) {
                continue;
            }
            process_piece(piece, neighborhood, chunk_box, random);
            changed = true;
        }

        plan.core.mark_post_processed(target);
        changed
    }
}

fn process_piece(
    piece: &mut StrongholdPiecePlan,
    neighborhood: &mut PopulationNeighborhood,
    chunk_box: StructureBounds,
    random: &mut MtRandom,
) {
    match piece.kind {
        StrongholdPieceKind::StairsDown => stairs_down(piece, neighborhood, chunk_box, random),
        StrongholdPieceKind::ChestCorridor => {
            chest_corridor(piece, neighborhood, chunk_box, random)
        }
        StrongholdPieceKind::FillerCorridor => filler_corridor(piece, neighborhood, chunk_box),
        StrongholdPieceKind::FiveCrossing => five_crossing(piece, neighborhood, chunk_box, random),
        StrongholdPieceKind::LeftTurn => turn(piece, neighborhood, chunk_box, random, true),
        StrongholdPieceKind::RightTurn => turn(piece, neighborhood, chunk_box, random, false),
        StrongholdPieceKind::Library => library(piece, neighborhood, chunk_box, random),
        StrongholdPieceKind::PortalRoom => portal_room(piece, neighborhood, chunk_box, random),
        StrongholdPieceKind::PrisonHall => prison_hall(piece, neighborhood, chunk_box, random),
        StrongholdPieceKind::RoomCrossing => room_crossing(piece, neighborhood, chunk_box, random),
        StrongholdPieceKind::Straight => straight(piece, neighborhood, chunk_box, random),
        StrongholdPieceKind::StraightStairsDown => {
            straight_stairs_down(piece, neighborhood, chunk_box, random)
        }
    }
}

fn stairs_down(
    piece: &StrongholdPiecePlan,
    neighborhood: &mut PopulationNeighborhood,
    chunk_box: StructureBounds,
    random: &mut MtRandom,
) {
    let mut w = Writer::new(neighborhood, chunk_box, piece.bounds, piece.orientation);
    w.selector_box(random, 0, 0, 0, 4, 10, 4);
    w.small_door(piece.entry_door, 1, 7, 0);
    w.small_door(StrongholdDoor::Opening, 1, 1, 4);

    for (block, data, x, y, z) in [
        (STONE_BRICK, 0, 2, 6, 1),
        (STONE_BRICK, 0, 1, 5, 1),
        (STONE_SLAB, STONE_BRICK_SLAB_DATA, 1, 6, 1),
        (STONE_BRICK, 0, 1, 5, 2),
        (STONE_BRICK, 0, 1, 4, 3),
        (STONE_SLAB, STONE_BRICK_SLAB_DATA, 1, 5, 3),
        (STONE_BRICK, 0, 2, 4, 3),
        (STONE_BRICK, 0, 3, 3, 3),
        (STONE_SLAB, STONE_BRICK_SLAB_DATA, 3, 4, 3),
        (STONE_BRICK, 0, 3, 3, 2),
        (STONE_BRICK, 0, 3, 2, 1),
        (STONE_SLAB, STONE_BRICK_SLAB_DATA, 3, 3, 1),
        (STONE_BRICK, 0, 2, 2, 1),
        (STONE_BRICK, 0, 1, 1, 1),
        (STONE_SLAB, STONE_BRICK_SLAB_DATA, 1, 2, 1),
        (STONE_BRICK, 0, 1, 1, 2),
        (STONE_SLAB, STONE_BRICK_SLAB_DATA, 1, 1, 3),
    ] {
        w.place(block, data, x, y, z);
    }
}

fn chest_corridor(
    piece: &mut StrongholdPiecePlan,
    neighborhood: &mut PopulationNeighborhood,
    chunk_box: StructureBounds,
    random: &mut MtRandom,
) {
    let bounds = piece.bounds;
    let orientation = piece.orientation;
    let entry = piece.entry_door;
    let StrongholdPieceExtra::ChestCorridor { has_placed_chest } = &mut piece.extra else {
        unreachable!();
    };

    let mut w = Writer::new(neighborhood, chunk_box, bounds, orientation);
    w.selector_box(random, 0, 0, 0, 4, 4, 6);
    w.small_door(entry, 1, 2, 0);
    w.small_door(StrongholdDoor::Opening, 1, 1, 6);

    w.box_fill(3, 1, 2, 3, 1, 4, (STONE_BRICK, 0), (AIR, 0));
    for (x, y, z) in [(3, 1, 1), (3, 1, 5), (3, 2, 2), (3, 2, 4)] {
        w.place(STONE_SLAB, STONE_BRICK_SLAB_DATA, x, y, z);
    }
    for z in 2..=4 {
        w.place(STONE_SLAB, STONE_BRICK_SLAB_DATA, 2, 1, z);
    }

    if !*has_placed_chest && w.local_inside_chunk(3, 2, 3) {
        // Target createChest() is fully commented out and always returns false,
        // but the piece still flips its durable placement flag.
        *has_placed_chest = true;
    }
}

fn filler_corridor(
    piece: &StrongholdPiecePlan,
    neighborhood: &mut PopulationNeighborhood,
    chunk_box: StructureBounds,
) {
    let StrongholdPieceExtra::FillerCorridor { steps } = piece.extra else {
        unreachable!();
    };
    let mut w = Writer::new(neighborhood, chunk_box, piece.bounds, piece.orientation);
    for z in 0..steps {
        for x in 0..=4 {
            w.place(STONE_BRICK, 0, x, 0, z);
            w.place(STONE_BRICK, 0, x, 4, z);
        }
        for y in 1..=3 {
            w.place(STONE_BRICK, 0, 0, y, z);
            w.place(AIR, 0, 1, y, z);
            w.place(AIR, 0, 2, y, z);
            w.place(AIR, 0, 3, y, z);
            w.place(STONE_BRICK, 0, 4, y, z);
        }
    }
}

fn five_crossing(
    piece: &StrongholdPiecePlan,
    neighborhood: &mut PopulationNeighborhood,
    chunk_box: StructureBounds,
    random: &mut MtRandom,
) {
    let StrongholdPieceExtra::FiveCrossing {
        left_high,
        left_low,
        right_high,
        right_low,
    } = piece.extra
    else {
        unreachable!();
    };

    let mut w = Writer::new(neighborhood, chunk_box, piece.bounds, piece.orientation);
    w.selector_box(random, 0, 0, 0, 9, 8, 10);
    w.small_door(piece.entry_door, 4, 3, 0);

    if left_low {
        w.box_fill(0, 3, 1, 0, 5, 3, (AIR, 0), (AIR, 0));
    }
    if right_low {
        w.box_fill(9, 3, 1, 9, 5, 3, (AIR, 0), (AIR, 0));
    }
    if left_high {
        w.box_fill(0, 5, 7, 0, 7, 9, (AIR, 0), (AIR, 0));
    }
    if right_high {
        w.box_fill(9, 5, 7, 9, 7, 9, (AIR, 0), (AIR, 0));
    }
    w.box_fill(5, 1, 10, 7, 3, 10, (AIR, 0), (AIR, 0));

    w.selector_box(random, 1, 2, 1, 8, 2, 6);
    w.selector_box(random, 4, 1, 5, 4, 4, 9);
    w.selector_box(random, 8, 1, 5, 8, 4, 9);
    w.selector_box(random, 1, 4, 7, 3, 4, 9);
    w.selector_box(random, 1, 3, 5, 3, 3, 6);
    w.box_fill(
        1,
        3,
        4,
        3,
        3,
        4,
        (STONE_SLAB, STONE_BRICK_SLAB_DATA),
        (AIR, 0),
    );
    w.box_fill(
        1,
        4,
        6,
        3,
        4,
        6,
        (STONE_SLAB, STONE_BRICK_SLAB_DATA),
        (AIR, 0),
    );

    w.selector_box(random, 5, 1, 7, 7, 1, 8);
    w.box_fill(
        5,
        1,
        9,
        7,
        1,
        9,
        (STONE_SLAB, STONE_BRICK_SLAB_DATA),
        (AIR, 0),
    );
    w.box_fill(
        5,
        2,
        7,
        7,
        2,
        7,
        (STONE_SLAB, STONE_BRICK_SLAB_DATA),
        (AIR, 0),
    );

    w.box_fill(
        4,
        5,
        7,
        4,
        5,
        9,
        (STONE_SLAB, STONE_BRICK_SLAB_DATA),
        (AIR, 0),
    );
    w.box_fill(
        8,
        5,
        7,
        8,
        5,
        9,
        (STONE_SLAB, STONE_BRICK_SLAB_DATA),
        (AIR, 0),
    );
    w.box_fill(5, 5, 7, 7, 5, 9, (DOUBLE_STONE_SLAB, 0), (AIR, 0));
    let torch = w.torch_data(Some(StructureOrientation::North));
    w.place(TORCH, torch, 6, 5, 6);
}

fn turn(
    piece: &StrongholdPiecePlan,
    neighborhood: &mut PopulationNeighborhood,
    chunk_box: StructureBounds,
    random: &mut MtRandom,
    left: bool,
) {
    let mut w = Writer::new(neighborhood, chunk_box, piece.bounds, piece.orientation);
    w.selector_box(random, 0, 0, 0, 4, 4, 4);
    w.small_door(piece.entry_door, 1, 2, 0);

    let north_or_east = matches!(
        piece.orientation,
        StructureOrientation::North | StructureOrientation::East
    );
    let use_x0 = if left { north_or_east } else { !north_or_east };
    let x = if use_x0 { 0 } else { 4 };
    w.box_fill(x, 1, 1, x, 3, 3, (AIR, 0), (AIR, 0));
}

fn prison_hall(
    piece: &StrongholdPiecePlan,
    neighborhood: &mut PopulationNeighborhood,
    chunk_box: StructureBounds,
    random: &mut MtRandom,
) {
    let mut w = Writer::new(neighborhood, chunk_box, piece.bounds, piece.orientation);
    w.selector_box(random, 0, 0, 0, 8, 4, 10);
    w.small_door(piece.entry_door, 1, 1, 0);
    w.box_fill(1, 1, 10, 3, 3, 10, (AIR, 0), (AIR, 0));

    for z in [1, 3, 7, 9] {
        w.selector_box(random, 4, 1, z, 4, 3, z);
    }
    w.box_fill(4, 1, 4, 4, 3, 6, (IRON_BARS, 0), (IRON_BARS, 0));
    w.box_fill(5, 1, 5, 7, 3, 5, (IRON_BARS, 0), (IRON_BARS, 0));
    w.place(IRON_BARS, 0, 4, 3, 2);
    w.place(IRON_BARS, 0, 4, 3, 8);
}

fn room_crossing(
    piece: &StrongholdPiecePlan,
    neighborhood: &mut PopulationNeighborhood,
    chunk_box: StructureBounds,
    random: &mut MtRandom,
) {
    let StrongholdPieceExtra::RoomCrossing { room_type } = piece.extra else {
        unreachable!();
    };
    let mut w = Writer::new(neighborhood, chunk_box, piece.bounds, piece.orientation);
    w.selector_box(random, 0, 0, 0, 10, 6, 10);
    w.small_door(piece.entry_door, 4, 1, 0);
    w.box_fill(4, 1, 10, 6, 3, 10, (AIR, 0), (AIR, 0));
    w.box_fill(0, 1, 4, 0, 3, 6, (AIR, 0), (AIR, 0));
    w.box_fill(10, 1, 4, 10, 3, 6, (AIR, 0), (AIR, 0));

    match room_type {
        0 => {
            for y in 1..=3 {
                w.place(STONE_BRICK, 0, 5, y, 5);
            }
            let east = w.torch_data(Some(StructureOrientation::East));
            let west = w.torch_data(Some(StructureOrientation::West));
            let north = w.torch_data(Some(StructureOrientation::North));
            let south = w.torch_data(Some(StructureOrientation::South));
            w.place(TORCH, east, 4, 3, 5);
            w.place(TORCH, west, 6, 3, 5);
            w.place(TORCH, north, 5, 3, 4);
            w.place(TORCH, south, 5, 3, 6);
            for (x, z) in [
                (4, 4),
                (4, 5),
                (4, 6),
                (6, 4),
                (6, 5),
                (6, 6),
                (5, 4),
                (5, 6),
            ] {
                w.place(STONE_SLAB, 0, x, 1, z);
            }
        }
        1 => {
            for i in 0..5 {
                w.place(STONE_BRICK, 0, 3, 1, 3 + i);
                w.place(STONE_BRICK, 0, 7, 1, 3 + i);
                w.place(STONE_BRICK, 0, 3 + i, 1, 3);
                w.place(STONE_BRICK, 0, 3 + i, 1, 7);
            }
            for y in 1..=3 {
                w.place(STONE_BRICK, 0, 5, y, 5);
            }
            w.place(FLOWING_WATER, 0, 5, 4, 5);
        }
        2 => {
            for z in 1..=9 {
                w.place(COBBLESTONE, 0, 1, 3, z);
                w.place(COBBLESTONE, 0, 9, 3, z);
            }
            for x in 1..=9 {
                w.place(COBBLESTONE, 0, x, 3, 1);
                w.place(COBBLESTONE, 0, x, 3, 9);
            }
            for (x, y, z) in [
                (5, 1, 4),
                (5, 1, 6),
                (5, 3, 4),
                (5, 3, 6),
                (4, 1, 5),
                (6, 1, 5),
                (4, 3, 5),
                (6, 3, 5),
            ] {
                w.place(COBBLESTONE, 0, x, y, z);
            }
            for y in 1..=3 {
                for (x, z) in [(4, 4), (6, 4), (4, 6), (6, 6)] {
                    w.place(COBBLESTONE, 0, x, y, z);
                }
            }
            w.place(TORCH, 0, 5, 3, 5);

            for z in 2..=8 {
                w.place(PLANKS, 0, 2, 3, z);
                w.place(PLANKS, 0, 3, 3, z);
                if z <= 3 || z >= 7 {
                    w.place(PLANKS, 0, 4, 3, z);
                    w.place(PLANKS, 0, 5, 3, z);
                    w.place(PLANKS, 0, 6, 3, z);
                }
                w.place(PLANKS, 0, 7, 3, z);
                w.place(PLANKS, 0, 8, 3, z);
            }
            let ladder_data = w.orient_ladder(4);
            for y in 1..=3 {
                w.place(LADDER, ladder_data, 9, y, 3);
            }
            // Target createChest() is a no-op in this branch.
        }
        _ => {}
    }
}

fn straight(
    piece: &StrongholdPiecePlan,
    neighborhood: &mut PopulationNeighborhood,
    chunk_box: StructureBounds,
    random: &mut MtRandom,
) {
    let StrongholdPieceExtra::Straight {
        left_child,
        right_child,
    } = piece.extra
    else {
        unreachable!();
    };
    let mut w = Writer::new(neighborhood, chunk_box, piece.bounds, piece.orientation);
    w.selector_box(random, 0, 0, 0, 4, 4, 6);
    w.small_door(piece.entry_door, 1, 2, 0);
    w.small_door(StrongholdDoor::Opening, 1, 1, 6);

    let west = w.torch_data(Some(StructureOrientation::West));
    let east = w.torch_data(Some(StructureOrientation::East));
    for (x, z, data) in [(1, 1, west), (3, 1, east), (1, 5, west), (3, 5, east)] {
        w.maybe_block(random, 0.1, TORCH, data, x, 2, z);
    }

    if left_child {
        w.box_fill(0, 1, 2, 0, 3, 4, (AIR, 0), (AIR, 0));
    }
    if right_child {
        w.box_fill(4, 1, 2, 4, 3, 4, (AIR, 0), (AIR, 0));
    }
}

fn straight_stairs_down(
    piece: &StrongholdPiecePlan,
    neighborhood: &mut PopulationNeighborhood,
    chunk_box: StructureBounds,
    random: &mut MtRandom,
) {
    let mut w = Writer::new(neighborhood, chunk_box, piece.bounds, piece.orientation);
    w.selector_box(random, 0, 0, 0, 4, 10, 7);
    w.small_door(piece.entry_door, 1, 7, 0);
    w.small_door(StrongholdDoor::Opening, 1, 1, 7);

    let stair_data = w.orient_stair(2);
    for i in 0..6 {
        for x in 1..=3 {
            w.place(STONE_STAIRS, stair_data, x, 6 - i, 1 + i);
            if i < 5 {
                w.place(STONE_BRICK, 0, x, 5 - i, 1 + i);
            }
        }
    }
}

fn library(
    piece: &StrongholdPiecePlan,
    neighborhood: &mut PopulationNeighborhood,
    chunk_box: StructureBounds,
    random: &mut MtRandom,
) {
    let StrongholdPieceExtra::Library { is_tall } = piece.extra else {
        unreachable!();
    };
    let current_height = if is_tall { 11 } else { 6 };
    let mut w = Writer::new(neighborhood, chunk_box, piece.bounds, piece.orientation);
    w.selector_box(random, 0, 0, 0, 13, current_height - 1, 14);
    w.small_door(piece.entry_door, 4, 1, 0);
    w.maybe_box(random, 0.07, 2, 1, 1, 11, 4, 13, (WEB, 0), (WEB, 0));

    let book_left = 1;
    let book_right = 12;
    for d in 1..=13 {
        if (d - 1) % 4 == 0 {
            w.box_fill(book_left, 1, d, book_left, 4, d, (PLANKS, 0), (PLANKS, 0));
            w.box_fill(book_right, 1, d, book_right, 4, d, (PLANKS, 0), (PLANKS, 0));
            let west = w.torch_data(Some(StructureOrientation::West));
            let east = w.torch_data(Some(StructureOrientation::East));
            w.place(TORCH, west, 2, 3, d);
            w.place(TORCH, east, 11, 3, d);
            if is_tall {
                w.box_fill(book_left, 6, d, book_left, 9, d, (PLANKS, 0), (PLANKS, 0));
                w.box_fill(book_right, 6, d, book_right, 9, d, (PLANKS, 0), (PLANKS, 0));
            }
        } else {
            w.box_fill(
                book_left,
                1,
                d,
                book_left,
                4,
                d,
                (BOOKSHELF, 0),
                (BOOKSHELF, 0),
            );
            w.box_fill(
                book_right,
                1,
                d,
                book_right,
                4,
                d,
                (BOOKSHELF, 0),
                (BOOKSHELF, 0),
            );
            if is_tall {
                w.box_fill(
                    book_left,
                    6,
                    d,
                    book_left,
                    9,
                    d,
                    (BOOKSHELF, 0),
                    (BOOKSHELF, 0),
                );
                w.box_fill(
                    book_right,
                    6,
                    d,
                    book_right,
                    9,
                    d,
                    (BOOKSHELF, 0),
                    (BOOKSHELF, 0),
                );
            }
        }
    }

    for d in (3..12).step_by(2) {
        w.box_fill(3, 1, d, 4, 3, d, (BOOKSHELF, 0), (BOOKSHELF, 0));
        w.box_fill(6, 1, d, 7, 3, d, (BOOKSHELF, 0), (BOOKSHELF, 0));
        w.box_fill(9, 1, d, 10, 3, d, (BOOKSHELF, 0), (BOOKSHELF, 0));
    }

    if is_tall {
        w.box_fill(1, 5, 1, 3, 5, 13, (PLANKS, 0), (PLANKS, 0));
        w.box_fill(10, 5, 1, 12, 5, 13, (PLANKS, 0), (PLANKS, 0));
        w.box_fill(4, 5, 1, 9, 5, 2, (PLANKS, 0), (PLANKS, 0));
        w.box_fill(4, 5, 12, 9, 5, 13, (PLANKS, 0), (PLANKS, 0));
        for (x, z) in [(9, 11), (8, 11), (9, 10)] {
            w.place(PLANKS, 0, x, 5, z);
        }

        w.box_fill(3, 6, 2, 3, 6, 12, (FENCE, 0), (FENCE, 0));
        w.box_fill(10, 6, 2, 10, 6, 10, (FENCE, 0), (FENCE, 0));
        w.box_fill(4, 6, 2, 9, 6, 2, (FENCE, 0), (FENCE, 0));
        w.box_fill(4, 6, 12, 8, 6, 12, (FENCE, 0), (FENCE, 0));
        for (x, z) in [(9, 11), (8, 11), (9, 10)] {
            w.place(FENCE, 0, x, 6, z);
        }

        let ladder = w.orient_ladder(3);
        for y in 1..=7 {
            w.place(LADDER, ladder, 10, y, 13);
        }

        let x = 7;
        let z = 7;
        for (px, py, pz) in [
            (x - 1, 9, z),
            (x, 9, z),
            (x - 1, 8, z),
            (x, 8, z),
            (x - 1, 7, z),
            (x, 7, z),
            (x - 2, 7, z),
            (x + 1, 7, z),
            (x - 1, 7, z - 1),
            (x - 1, 7, z + 1),
            (x, 7, z - 1),
            (x, 7, z + 1),
        ] {
            w.place(FENCE, 0, px, py, pz);
        }
        let top_torch = w.torch_data(None);
        for (px, pz) in [
            (x - 2, z),
            (x + 1, z),
            (x - 1, z - 1),
            (x - 1, z + 1),
            (x, z - 1),
            (x, z + 1),
        ] {
            w.place(TORCH, top_torch, px, 8, pz);
        }
    }

    // Both target library createChest() calls are no-ops.
}

fn portal_room(
    piece: &mut StrongholdPiecePlan,
    neighborhood: &mut PopulationNeighborhood,
    chunk_box: StructureBounds,
    random: &mut MtRandom,
) {
    let bounds = piece.bounds;
    let orientation = piece.orientation;
    let StrongholdPieceExtra::PortalRoom {
        has_placed_mob_spawner,
    } = &mut piece.extra
    else {
        unreachable!();
    };
    let mut w = Writer::new(neighborhood, chunk_box, bounds, orientation);

    w.selector_box(random, 0, 0, 0, 10, 7, 15);
    w.small_door(StrongholdDoor::Grates, 4, 1, 0);

    let y = 6;
    w.selector_box(random, 1, y, 1, 1, y, 14);
    w.selector_box(random, 9, y, 1, 9, y, 14);
    w.selector_box(random, 2, y, 1, 8, y, 2);
    w.selector_box(random, 2, y, 14, 8, y, 14);

    w.selector_box(random, 1, 1, 1, 2, 1, 4);
    w.selector_box(random, 8, 1, 1, 9, 1, 4);
    w.box_fill(1, 1, 1, 1, 1, 3, (LAVA, 0), (LAVA, 0));
    w.box_fill(9, 1, 1, 9, 1, 3, (LAVA, 0), (LAVA, 0));

    w.selector_box(random, 3, 1, 8, 7, 1, 12);
    w.box_fill(4, 1, 9, 6, 1, 11, (LAVA, 0), (LAVA, 0));

    for z in (3..14).step_by(2) {
        w.box_fill(0, 3, z, 0, 4, z, (IRON_BARS, 0), (IRON_BARS, 0));
        w.box_fill(10, 3, z, 10, 4, z, (IRON_BARS, 0), (IRON_BARS, 0));
    }
    for x in (2..9).step_by(2) {
        w.box_fill(x, 3, 15, x, 4, 15, (IRON_BARS, 0), (IRON_BARS, 0));
    }

    let stair_data = w.orient_stair(3);
    w.selector_box(random, 4, 1, 5, 6, 1, 7);
    w.selector_box(random, 4, 2, 6, 6, 2, 7);
    w.selector_box(random, 4, 3, 7, 6, 3, 7);
    for x in 4..=6 {
        w.place(STONE_BRICK_STAIRS, stair_data, x, 1, 4);
        w.place(STONE_BRICK_STAIRS, stair_data, x, 2, 5);
        w.place(STONE_BRICK_STAIRS, stair_data, x, 3, 6);
    }

    let (north, south, east, west) = portal_directions(orientation);
    let mut eyes = [false; 12];
    let mut all_eyes = true;
    for eye in &mut eyes {
        *eye = random.next_float() > 0.9;
        all_eyes &= *eye;
    }

    for (x, z, direction, eye) in [
        (4, 8, north, eyes[0]),
        (5, 8, north, eyes[1]),
        (6, 8, north, eyes[2]),
        (4, 12, south, eyes[3]),
        (5, 12, south, eyes[4]),
        (6, 12, south, eyes[5]),
        (3, 9, east, eyes[6]),
        (3, 10, east, eyes[7]),
        (3, 11, east, eyes[8]),
        (7, 9, west, eyes[9]),
        (7, 10, west, eyes[10]),
        (7, 11, west, eyes[11]),
    ] {
        let data = direction | if eye { END_PORTAL_EYE_BIT } else { 0 };
        w.place(END_PORTAL_FRAME, data, x, 3, z);
    }

    if all_eyes {
        for x in 4..=6 {
            for z in 9..=11 {
                w.place(END_PORTAL, 0, x, 3, z);
            }
        }
    }

    if !*has_placed_mob_spawner && w.local_inside_chunk(5, 3, 6) {
        *has_placed_mob_spawner = true;
        w.place(MOB_SPAWNER, 0, 5, 3, 6);
    }
}

fn portal_directions(orientation: StructureOrientation) -> (u8, u8, u8, u8) {
    let mut north = StructureOrientation::North as u8;
    let mut south = StructureOrientation::South as u8;
    let mut east = StructureOrientation::East as u8;
    let mut west = StructureOrientation::West as u8;
    match orientation {
        StructureOrientation::South => {
            north = StructureOrientation::South as u8;
            south = StructureOrientation::North as u8;
        }
        StructureOrientation::East => {
            north = StructureOrientation::East as u8;
            south = StructureOrientation::West as u8;
            east = StructureOrientation::South as u8;
            west = StructureOrientation::North as u8;
        }
        StructureOrientation::West => {
            north = StructureOrientation::West as u8;
            south = StructureOrientation::East as u8;
            east = StructureOrientation::South as u8;
            west = StructureOrientation::North as u8;
        }
        StructureOrientation::North => {}
    }
    (north, south, east, west)
}

struct Writer<'a> {
    neighborhood: &'a mut PopulationNeighborhood,
    chunk_box: StructureBounds,
    bounds: StructureBounds,
    orientation: StructureOrientation,
}

impl<'a> Writer<'a> {
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

    fn local_inside_chunk(&self, x: i32, y: i32, z: i32) -> bool {
        let (wx, wy, wz) = self.world_pos(x, y, z);
        self.chunk_box.contains(wx, wy, wz)
    }

    fn place(&mut self, block: u16, data: u8, x: i32, y: i32, z: i32) {
        let (wx, wy, wz) = self.world_pos(x, y, z);
        if self.chunk_box.contains(wx, wy, wz) {
            self.neighborhood.set_state(wx, wy, wz, state(block, data));
        }
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
                    let edge = y == y0 || y == y1 || x == x0 || x == x1 || z == z0 || z == z1;
                    if edge {
                        let selection = random.next_float();
                        if selection < 0.2 {
                            self.place(STONE_BRICK, 2, x, y, z);
                        } else if selection < 0.5 {
                            self.place(STONE_BRICK, 1, x, y, z);
                        } else if selection < 0.55 {
                            self.place(MONSTER_EGG, MONSTER_EGG_STONE_BRICK_DATA, x, y, z);
                        } else {
                            self.place(STONE_BRICK, 0, x, y, z);
                        }
                    } else {
                        self.place(AIR, 0, x, y, z);
                    }
                }
            }
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
    ) {
        for y in y0..=y1 {
            for x in x0..=x1 {
                for z in z0..=z1 {
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
    ) {
        for y in y0..=y1 {
            for x in x0..=x1 {
                for z in z0..=z1 {
                    if random.next_float() > probability {
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
            self.place(block, data, x, y, z);
        }
    }

    fn small_door(&mut self, door: StrongholdDoor, x: i32, y: i32, z: i32) {
        match door {
            StrongholdDoor::Opening => {
                self.box_fill(x, y, z, x + 2, y + 2, z, (AIR, 0), (AIR, 0));
            }
            StrongholdDoor::WoodDoor => {
                for (px, py) in [
                    (x, y),
                    (x, y + 1),
                    (x, y + 2),
                    (x + 1, y + 2),
                    (x + 2, y + 2),
                    (x + 2, y + 1),
                    (x + 2, y),
                ] {
                    self.place(STONE_BRICK, 0, px, py, z);
                }
                self.place(WOODEN_DOOR, 0, x + 1, y, z);
                self.place(WOODEN_DOOR, DOOR_UPPER_DATA, x + 1, y + 1, z);
            }
            StrongholdDoor::Grates => {
                self.place(AIR, 0, x + 1, y, z);
                self.place(AIR, 0, x + 1, y + 1, z);
                for (px, py) in [
                    (x, y),
                    (x, y + 1),
                    (x, y + 2),
                    (x + 1, y + 2),
                    (x + 2, y + 2),
                    (x + 2, y + 1),
                    (x + 2, y),
                ] {
                    self.place(IRON_BARS, 0, px, py, z);
                }
            }
            StrongholdDoor::IronDoor => {
                for (px, py) in [
                    (x, y),
                    (x, y + 1),
                    (x, y + 2),
                    (x + 1, y + 2),
                    (x + 2, y + 2),
                    (x + 2, y + 1),
                    (x + 2, y),
                ] {
                    self.place(STONE_BRICK, 0, px, py, z);
                }
                self.place(IRON_DOOR, 0, x + 1, y, z);
                let button_a = self.orient_button(4);
                let button_b = self.orient_button(3);
                self.place(WOOD_BUTTON, button_a, x + 2, y + 1, z + 1);
                self.place(WOOD_BUTTON, button_b, x + 2, y + 1, z - 1);
            }
        }
    }

    fn torch_data(&self, direction: Option<StructureOrientation>) -> u8 {
        let Some(direction) = direction else {
            return 5;
        };
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

    fn orient_stair(&self, data: u8) -> u8 {
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

    fn orient_ladder(&self, data: u8) -> u8 {
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

    fn orient_button(&self, data: u8) -> u8 {
        match self.orientation {
            StructureOrientation::North => data,
            StructureOrientation::South => match data {
                3 => 4,
                4 => 3,
                _ => data,
            },
            StructureOrientation::West => match data {
                3 => 1,
                4 => 2,
                2 => 3,
                1 => 4,
                _ => data,
            },
            StructureOrientation::East => match data {
                3 => 2,
                4 => 1,
                2 => 3,
                1 => 4,
                _ => data,
            },
        }
    }
}

fn chunk_bounds(target: ChunkCoord) -> StructureBounds {
    let x0 = target.x().wrapping_mul(16);
    let z0 = target.z().wrapping_mul(16);
    StructureBounds::new(x0, 0, z0, x0.wrapping_add(15), 512, z0.wrapping_add(15))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_piece(
        kind: StrongholdPieceKind,
        bounds: StructureBounds,
        orientation: StructureOrientation,
        entry_door: StrongholdDoor,
        extra: StrongholdPieceExtra,
    ) -> StrongholdPiecePlan {
        StrongholdPiecePlan {
            kind,
            bounds,
            orientation,
            gen_depth: 1,
            entry_door,
            extra,
        }
    }

    fn process_fixture(piece: StrongholdPiecePlan) -> (PopulationNeighborhood, StrongholdPlan) {
        let target = ChunkCoord::new(0, 0);
        let mut neighborhood = PopulationNeighborhood::filled(target, state(1, 0), 1);
        let mut plan = StrongholdPlan {
            core: crate::structure::StructureStartCore::new(target, piece.bounds),
            pieces: vec![piece],
        };
        let mut random = MtRandom::new(0x1234_5678);
        assert!(StrongholdPostProcessor::process_start(
            &mut plan,
            &mut neighborhood,
            target,
            &mut random,
        ));
        (neighborhood, plan)
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

    fn count_block(neighborhood: &PopulationNeighborhood, id: u16) -> usize {
        neighborhood
            .center_states()
            .iter()
            .filter(|state| (**state >> 4) == id)
            .count()
    }

    #[test]
    fn independent_simple_piece_block_fixtures_match() {
        // Standalone C++ std::mt19937/block-array oracle implementing the target
        // selector, local transforms, door helper, and each recipe directly.
        let fixtures = vec![
            (
                "stairs",
                fixture_piece(
                    StrongholdPieceKind::StairsDown,
                    StructureBounds::new(4, 20, 4, 8, 30, 8),
                    StructureOrientation::South,
                    StrongholdDoor::Opening,
                    StrongholdPieceExtra::StairsDown { is_source: false },
                ),
                0x81d7_a603_31b3_ce8b_u64,
            ),
            (
                "chest",
                fixture_piece(
                    StrongholdPieceKind::ChestCorridor,
                    StructureBounds::new(4, 20, 4, 8, 24, 10),
                    StructureOrientation::South,
                    StrongholdDoor::WoodDoor,
                    StrongholdPieceExtra::ChestCorridor {
                        has_placed_chest: false,
                    },
                ),
                0x63c4_fa84_5b6a_2523_u64,
            ),
            (
                "filler",
                fixture_piece(
                    StrongholdPieceKind::FillerCorridor,
                    StructureBounds::new(4, 20, 4, 8, 24, 6),
                    StructureOrientation::South,
                    StrongholdDoor::Opening,
                    StrongholdPieceExtra::FillerCorridor { steps: 3 },
                ),
                0xd47a_ba58_6166_dc75_u64,
            ),
            (
                "five",
                fixture_piece(
                    StrongholdPieceKind::FiveCrossing,
                    StructureBounds::new(3, 20, 2, 12, 28, 12),
                    StructureOrientation::South,
                    StrongholdDoor::WoodDoor,
                    StrongholdPieceExtra::FiveCrossing {
                        left_high: true,
                        left_low: true,
                        right_high: true,
                        right_low: true,
                    },
                ),
                0x9a7a_9002_e2b6_b9b5_u64,
            ),
            (
                "left",
                fixture_piece(
                    StrongholdPieceKind::LeftTurn,
                    StructureBounds::new(4, 20, 4, 8, 24, 8),
                    StructureOrientation::South,
                    StrongholdDoor::Grates,
                    StrongholdPieceExtra::None,
                ),
                0x0e81_e75d_d126_856f_u64,
            ),
            (
                "right",
                fixture_piece(
                    StrongholdPieceKind::RightTurn,
                    StructureBounds::new(4, 20, 4, 8, 24, 8),
                    StructureOrientation::South,
                    StrongholdDoor::Grates,
                    StrongholdPieceExtra::None,
                ),
                0x55ce_8ea4_fb17_5a2c_u64,
            ),
            (
                "prison",
                fixture_piece(
                    StrongholdPieceKind::PrisonHall,
                    StructureBounds::new(3, 20, 2, 11, 24, 12),
                    StructureOrientation::South,
                    StrongholdDoor::Grates,
                    StrongholdPieceExtra::None,
                ),
                0x0cf6_6de2_568d_5c21_u64,
            ),
            (
                "straight",
                fixture_piece(
                    StrongholdPieceKind::Straight,
                    StructureBounds::new(4, 20, 4, 8, 24, 10),
                    StructureOrientation::South,
                    StrongholdDoor::WoodDoor,
                    StrongholdPieceExtra::Straight {
                        left_child: true,
                        right_child: true,
                    },
                ),
                0x8377_83a9_0c6a_2c69_u64,
            ),
            (
                "straight-stairs-east",
                fixture_piece(
                    StrongholdPieceKind::StraightStairsDown,
                    StructureBounds::new(3, 15, 4, 10, 25, 8),
                    StructureOrientation::East,
                    StrongholdDoor::Grates,
                    StrongholdPieceExtra::None,
                ),
                0x3ccf_03dc_df56_954e_u64,
            ),
        ];

        for (name, piece, expected) in fixtures {
            let (neighborhood, _) = process_fixture(piece);
            assert_eq!(center_hash(&neighborhood), expected, "{name}");
        }
    }

    #[test]
    fn independent_complex_piece_block_fixtures_match() {
        let fixtures = vec![
            (
                "room0",
                fixture_piece(
                    StrongholdPieceKind::RoomCrossing,
                    StructureBounds::new(2, 20, 2, 12, 26, 12),
                    StructureOrientation::South,
                    StrongholdDoor::WoodDoor,
                    StrongholdPieceExtra::RoomCrossing { room_type: 0 },
                ),
                0xbeba_bea5_2f19_9d7a_u64,
            ),
            (
                "room1",
                fixture_piece(
                    StrongholdPieceKind::RoomCrossing,
                    StructureBounds::new(2, 20, 2, 12, 26, 12),
                    StructureOrientation::South,
                    StrongholdDoor::WoodDoor,
                    StrongholdPieceExtra::RoomCrossing { room_type: 1 },
                ),
                0xed98_70ee_a0d1_ec1a_u64,
            ),
            (
                "room2",
                fixture_piece(
                    StrongholdPieceKind::RoomCrossing,
                    StructureBounds::new(2, 20, 2, 12, 26, 12),
                    StructureOrientation::South,
                    StrongholdDoor::WoodDoor,
                    StrongholdPieceExtra::RoomCrossing { room_type: 2 },
                ),
                0x78e0_808e_7e3b_4b35_u64,
            ),
            (
                "library-short",
                fixture_piece(
                    StrongholdPieceKind::Library,
                    StructureBounds::new(1, 20, 0, 14, 25, 14),
                    StructureOrientation::South,
                    StrongholdDoor::WoodDoor,
                    StrongholdPieceExtra::Library { is_tall: false },
                ),
                0xb107_bd55_b337_43d4_u64,
            ),
            (
                "library-tall",
                fixture_piece(
                    StrongholdPieceKind::Library,
                    StructureBounds::new(1, 20, 0, 14, 30, 14),
                    StructureOrientation::South,
                    StrongholdDoor::WoodDoor,
                    StrongholdPieceExtra::Library { is_tall: true },
                ),
                0x66d1_8ed7_5e24_2484_u64,
            ),
            (
                "portal",
                fixture_piece(
                    StrongholdPieceKind::PortalRoom,
                    StructureBounds::new(2, 20, 0, 12, 27, 15),
                    StructureOrientation::South,
                    StrongholdDoor::Opening,
                    StrongholdPieceExtra::PortalRoom {
                        has_placed_mob_spawner: false,
                    },
                ),
                0xc558_fc78_c6cf_94f6_u64,
            ),
        ];

        for (name, piece, expected) in fixtures {
            let (neighborhood, _) = process_fixture(piece);
            assert_eq!(center_hash(&neighborhood), expected, "{name}");
        }
    }

    #[test]
    fn portal_room_matches_independent_eye_pattern_and_mutable_spawner_state() {
        let piece = fixture_piece(
            StrongholdPieceKind::PortalRoom,
            StructureBounds::new(2, 20, 0, 12, 27, 15),
            StructureOrientation::South,
            StrongholdDoor::Opening,
            StrongholdPieceExtra::PortalRoom {
                has_placed_mob_spawner: false,
            },
        );
        let (neighborhood, plan) = process_fixture(piece);

        let frames: Vec<u16> = neighborhood
            .center_states()
            .iter()
            .copied()
            .filter(|state| (*state >> 4) == END_PORTAL_FRAME)
            .collect();
        assert_eq!(frames.len(), 12);
        assert_eq!(
            frames
                .iter()
                .filter(|state| (**state & END_PORTAL_EYE_BIT as u16) != 0)
                .count(),
            1,
        );
        assert_eq!(count_block(&neighborhood, END_PORTAL), 0);
        assert_eq!(count_block(&neighborhood, MOB_SPAWNER), 1);

        let StrongholdPieceExtra::PortalRoom {
            has_placed_mob_spawner,
        } = plan.pieces[0].extra
        else {
            unreachable!();
        };
        assert!(has_placed_mob_spawner);
    }

    #[test]
    fn chest_corridor_preserves_target_noop_chest_flag_behavior() {
        let piece = fixture_piece(
            StrongholdPieceKind::ChestCorridor,
            StructureBounds::new(4, 20, 4, 8, 24, 10),
            StructureOrientation::South,
            StrongholdDoor::Opening,
            StrongholdPieceExtra::ChestCorridor {
                has_placed_chest: false,
            },
        );
        let (neighborhood, plan) = process_fixture(piece);
        let StrongholdPieceExtra::ChestCorridor { has_placed_chest } = plan.pieces[0].extra else {
            unreachable!();
        };
        assert!(has_placed_chest);
        assert_eq!(count_block(&neighborhood, 54), 0);
    }

    #[test]
    fn spanning_piece_processes_each_intersecting_chunk_once() {
        let piece = fixture_piece(
            StrongholdPieceKind::Straight,
            StructureBounds::new(14, 20, 4, 18, 24, 10),
            StructureOrientation::South,
            StrongholdDoor::Opening,
            StrongholdPieceExtra::Straight {
                left_child: false,
                right_child: false,
            },
        );
        let mut plan = StrongholdPlan {
            core: crate::structure::StructureStartCore::new(ChunkCoord::new(0, 0), piece.bounds),
            pieces: vec![piece],
        };

        let mut left = PopulationNeighborhood::filled(ChunkCoord::new(0, 0), state(1, 0), 1);
        let mut left_random = MtRandom::new(1);
        assert!(StrongholdPostProcessor::process_start(
            &mut plan,
            &mut left,
            ChunkCoord::new(0, 0),
            &mut left_random,
        ));
        assert!(count_block(&left, AIR) > 0);

        let mut left_again = left.clone();
        let mut repeated_random = MtRandom::new(1);
        assert!(!StrongholdPostProcessor::process_start(
            &mut plan,
            &mut left_again,
            ChunkCoord::new(0, 0),
            &mut repeated_random,
        ));
        assert_eq!(left_again, left);

        let mut right = PopulationNeighborhood::filled(ChunkCoord::new(1, 0), state(1, 0), 1);
        let mut right_random = MtRandom::new(2);
        assert!(StrongholdPostProcessor::process_start(
            &mut plan,
            &mut right,
            ChunkCoord::new(1, 0),
            &mut right_random,
        ));
        assert!(count_block(&right, AIR) > 0);
    }

    #[test]
    fn public_catalog_hole_end_portal_is_valid_internal_structure_output() {
        assert!(!crate::block_state_id_is_supported(END_PORTAL << 4));
        assert!(crate::world_block_state_id_is_supported(END_PORTAL << 4));
    }

    #[test]
    fn selector_rng_runs_for_full_piece_before_chunk_clipping() {
        let target = ChunkCoord::new(0, 0);
        let mut neighborhood = PopulationNeighborhood::filled(target, state(1, 0), 1);
        let mut writer = Writer::new(
            &mut neighborhood,
            chunk_bounds(target),
            StructureBounds::new(14, 20, 2, 18, 24, 8),
            StructureOrientation::South,
        );
        let mut clipped = MtRandom::new(7);
        let mut expected = clipped.clone();
        writer.selector_box(&mut clipped, 0, 0, 0, 4, 4, 6);
        for y in 0..=4 {
            for x in 0..=4 {
                for z in 0..=6 {
                    if y == 0 || y == 4 || x == 0 || x == 4 || z == 0 || z == 6 {
                        let _ = expected.next_float();
                    }
                }
            }
        }
        assert_eq!(clipped.next_u32(), expected.next_u32());
    }
}
