use crate::ChunkCoord;
use crate::terrain_shape::noise::MtRandom;

use super::{StructureBounds, StructureOrientation, StructureStartCore};

const MAX_DEPTH: i32 = 50;
const MAX_DISTANCE: i32 = 7 * 16;
const LOWEST_Y: i32 = 10;
const SEA_LEVEL_MINUS_FIVE: i32 = 58;

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
#[repr(u8)]
pub(crate) enum StrongholdPieceKind {
    Straight = 0,
    PrisonHall = 1,
    LeftTurn = 2,
    RightTurn = 3,
    RoomCrossing = 4,
    StraightStairsDown = 5,
    StairsDown = 6,
    FiveCrossing = 7,
    ChestCorridor = 8,
    Library = 9,
    PortalRoom = 10,
    FillerCorridor = 11,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
#[repr(u8)]
pub(crate) enum StrongholdDoor {
    Opening = 0,
    WoodDoor = 1,
    Grates = 2,
    IronDoor = 3,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) enum StrongholdPieceExtra {
    None,
    StairsDown {
        is_source: bool,
    },
    ChestCorridor {
        has_placed_chest: bool,
    },
    FillerCorridor {
        steps: i32,
    },
    FiveCrossing {
        left_high: bool,
        left_low: bool,
        right_high: bool,
        right_low: bool,
    },
    Library {
        is_tall: bool,
    },
    PortalRoom {
        has_placed_mob_spawner: bool,
    },
    RoomCrossing {
        room_type: i32,
    },
    Straight {
        left_child: bool,
        right_child: bool,
    },
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct StrongholdPiecePlan {
    pub(crate) kind: StrongholdPieceKind,
    pub(crate) bounds: StructureBounds,
    pub(crate) orientation: StructureOrientation,
    pub(crate) gen_depth: i32,
    pub(crate) entry_door: StrongholdDoor,
    pub(crate) extra: StrongholdPieceExtra,
}

impl StrongholdPiecePlan {
    fn move_by(&mut self, dx: i32, dy: i32, dz: i32) {
        self.bounds.move_by(dx, dy, dz);
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct StrongholdPlan {
    pub(crate) core: StructureStartCore,
    pub(crate) pieces: Vec<StrongholdPiecePlan>,
}

impl StrongholdPlan {
    pub(crate) fn generate(source: ChunkCoord, mut random: MtRandom) -> Self {
        let west = source.x().wrapping_mul(16).wrapping_add(2);
        let north = source.z().wrapping_mul(16).wrapping_add(2);
        let orientation = orientation_from_u32(random.next_int(4));
        let start = StrongholdPiecePlan {
            kind: StrongholdPieceKind::StairsDown,
            bounds: StructureBounds::new(west, 64, north, west + 4, 74, north + 4),
            orientation,
            gen_depth: 0,
            entry_door: StrongholdDoor::Opening,
            extra: StrongholdPieceExtra::StairsDown { is_source: true },
        };

        let mut builder = StrongholdPlanBuilder::new(start);
        builder.add_children(0, &mut random);

        while !builder.pending_children.is_empty() {
            let pending_index = random.next_int(builder.pending_children.len() as u32) as usize;
            let piece_index = builder.pending_children.remove(pending_index);
            builder.add_children(piece_index, &mut random);
        }

        let mut bounds = calculate_bounds(&builder.pieces);
        let mut y1_pos = bounds.y_span() + 1;
        if y1_pos < SEA_LEVEL_MINUS_FIVE {
            y1_pos += random.next_int((SEA_LEVEL_MINUS_FIVE - y1_pos) as u32) as i32;
        }
        let dy = y1_pos - bounds.y1;
        bounds.move_by(0, dy, 0);
        for piece in &mut builder.pieces {
            piece.move_by(0, dy, 0);
        }

        Self {
            core: StructureStartCore::new(source, bounds),
            pieces: builder.pieces,
        }
    }

    #[cfg(test)]
    pub(crate) fn topology_hash(&self) -> u64 {
        let mut hash = 0xcbf2_9ce4_8422_2325_u64;
        for piece in &self.pieces {
            hash = fnv_byte(hash, piece.kind as u8);
            hash = fnv_byte(hash, piece.orientation as u8);
            hash = fnv_byte(hash, piece.entry_door as u8);
            for value in [
                piece.gen_depth,
                piece.bounds.x0,
                piece.bounds.y0,
                piece.bounds.z0,
                piece.bounds.x1,
                piece.bounds.y1,
                piece.bounds.z1,
            ] {
                for byte in value.to_le_bytes() {
                    hash = fnv_byte(hash, byte);
                }
            }
            match piece.extra {
                StrongholdPieceExtra::None => {
                    hash = fnv_byte(hash, 0);
                }
                StrongholdPieceExtra::StairsDown { is_source } => {
                    hash = fnv_byte(hash, 1);
                    hash = fnv_byte(hash, u8::from(is_source));
                }
                StrongholdPieceExtra::ChestCorridor { has_placed_chest } => {
                    hash = fnv_byte(hash, 2);
                    hash = fnv_byte(hash, u8::from(has_placed_chest));
                }
                StrongholdPieceExtra::FillerCorridor { steps } => {
                    hash = fnv_byte(hash, 3);
                    for byte in steps.to_le_bytes() {
                        hash = fnv_byte(hash, byte);
                    }
                }
                StrongholdPieceExtra::FiveCrossing {
                    left_high,
                    left_low,
                    right_high,
                    right_low,
                } => {
                    hash = fnv_byte(hash, 4);
                    for flag in [left_high, left_low, right_high, right_low] {
                        hash = fnv_byte(hash, u8::from(flag));
                    }
                }
                StrongholdPieceExtra::Library { is_tall } => {
                    hash = fnv_byte(hash, 5);
                    hash = fnv_byte(hash, u8::from(is_tall));
                }
                StrongholdPieceExtra::PortalRoom {
                    has_placed_mob_spawner,
                } => {
                    hash = fnv_byte(hash, 6);
                    hash = fnv_byte(hash, u8::from(has_placed_mob_spawner));
                }
                StrongholdPieceExtra::RoomCrossing { room_type } => {
                    hash = fnv_byte(hash, 7);
                    for byte in room_type.to_le_bytes() {
                        hash = fnv_byte(hash, byte);
                    }
                }
                StrongholdPieceExtra::Straight {
                    left_child,
                    right_child,
                } => {
                    hash = fnv_byte(hash, 8);
                    hash = fnv_byte(hash, u8::from(left_child));
                    hash = fnv_byte(hash, u8::from(right_child));
                }
            }
        }
        hash
    }
}

#[derive(Debug, Clone)]
struct PieceWeight {
    kind: StrongholdPieceKind,
    weight: i32,
    place_count: i32,
    max_place_count: i32,
    min_depth: i32,
}

impl PieceWeight {
    const fn new(
        kind: StrongholdPieceKind,
        weight: i32,
        max_place_count: i32,
        min_depth: i32,
    ) -> Self {
        Self {
            kind,
            weight,
            place_count: 0,
            max_place_count,
            min_depth,
        }
    }

    fn do_place(&self, depth: i32) -> bool {
        (self.max_place_count == 0 || self.place_count < self.max_place_count)
            && depth >= self.min_depth
    }

    fn is_valid(&self) -> bool {
        self.max_place_count == 0 || self.place_count < self.max_place_count
    }
}

struct StrongholdPlanBuilder {
    pieces: Vec<StrongholdPiecePlan>,
    pending_children: Vec<usize>,
    weights: Vec<PieceWeight>,
    imposed_piece: Option<StrongholdPieceKind>,
    previous_piece: Option<StrongholdPieceKind>,
    start_bounds: StructureBounds,
}

impl StrongholdPlanBuilder {
    fn new(start: StrongholdPiecePlan) -> Self {
        let start_bounds = start.bounds;
        Self {
            pieces: vec![start],
            pending_children: Vec::new(),
            weights: vec![
                PieceWeight::new(StrongholdPieceKind::Straight, 40, 0, 0),
                PieceWeight::new(StrongholdPieceKind::PrisonHall, 5, 5, 0),
                PieceWeight::new(StrongholdPieceKind::LeftTurn, 20, 0, 0),
                PieceWeight::new(StrongholdPieceKind::RightTurn, 20, 0, 0),
                PieceWeight::new(StrongholdPieceKind::RoomCrossing, 10, 6, 0),
                PieceWeight::new(StrongholdPieceKind::StraightStairsDown, 5, 5, 0),
                PieceWeight::new(StrongholdPieceKind::StairsDown, 5, 5, 0),
                PieceWeight::new(StrongholdPieceKind::FiveCrossing, 5, 4, 0),
                PieceWeight::new(StrongholdPieceKind::ChestCorridor, 5, 4, 0),
                PieceWeight::new(StrongholdPieceKind::Library, 10, 2, 5),
                PieceWeight::new(StrongholdPieceKind::PortalRoom, 10, 1, 6),
            ],
            imposed_piece: None,
            previous_piece: None,
            start_bounds,
        }
    }

    fn add_children(&mut self, index: usize, random: &mut MtRandom) {
        let piece = self.pieces[index].clone();
        match piece.kind {
            StrongholdPieceKind::StairsDown => {
                let StrongholdPieceExtra::StairsDown { is_source } = piece.extra else {
                    unreachable!();
                };
                if is_source {
                    self.imposed_piece = Some(StrongholdPieceKind::FiveCrossing);
                }
                self.child_forward(&piece, random, 1, 1);
            }
            StrongholdPieceKind::ChestCorridor
            | StrongholdPieceKind::PrisonHall
            | StrongholdPieceKind::StraightStairsDown => {
                self.child_forward(&piece, random, 1, 1);
            }
            StrongholdPieceKind::FillerCorridor | StrongholdPieceKind::Library => {}
            StrongholdPieceKind::PortalRoom => {}
            StrongholdPieceKind::FiveCrossing => {
                let StrongholdPieceExtra::FiveCrossing {
                    left_high,
                    left_low,
                    right_high,
                    right_low,
                } = piece.extra
                else {
                    unreachable!();
                };
                let mut z_off_a = 3;
                let mut z_off_b = 5;
                if matches!(
                    piece.orientation,
                    StructureOrientation::West | StructureOrientation::North
                ) {
                    z_off_a = 11 - 3 - z_off_a;
                    z_off_b = 11 - 3 - z_off_b;
                }
                self.child_forward(&piece, random, 5, 1);
                if left_low {
                    self.child_left(&piece, random, z_off_a, 1);
                }
                if left_high {
                    self.child_left(&piece, random, z_off_b, 7);
                }
                if right_low {
                    self.child_right(&piece, random, z_off_a, 1);
                }
                if right_high {
                    self.child_right(&piece, random, z_off_b, 7);
                }
            }
            StrongholdPieceKind::LeftTurn => {
                if matches!(
                    piece.orientation,
                    StructureOrientation::North | StructureOrientation::East
                ) {
                    self.child_left(&piece, random, 1, 1);
                } else {
                    self.child_right(&piece, random, 1, 1);
                }
            }
            StrongholdPieceKind::RightTurn => {
                if matches!(
                    piece.orientation,
                    StructureOrientation::North | StructureOrientation::East
                ) {
                    self.child_right(&piece, random, 1, 1);
                } else {
                    self.child_left(&piece, random, 1, 1);
                }
            }
            StrongholdPieceKind::RoomCrossing => {
                self.child_forward(&piece, random, 4, 1);
                self.child_left(&piece, random, 1, 4);
                self.child_right(&piece, random, 1, 4);
            }
            StrongholdPieceKind::Straight => {
                let StrongholdPieceExtra::Straight {
                    left_child,
                    right_child,
                } = piece.extra
                else {
                    unreachable!();
                };
                self.child_forward(&piece, random, 1, 1);
                if left_child {
                    self.child_left(&piece, random, 1, 2);
                }
                if right_child {
                    self.child_right(&piece, random, 1, 2);
                }
            }
        }
    }

    fn child_forward(
        &mut self,
        piece: &StrongholdPiecePlan,
        random: &mut MtRandom,
        x_off: i32,
        y_off: i32,
    ) {
        let b = piece.bounds;
        let (x, y, z, orientation) = match piece.orientation {
            StructureOrientation::North => (
                b.x0 + x_off,
                b.y0 + y_off,
                b.z0 - 1,
                StructureOrientation::North,
            ),
            StructureOrientation::South => (
                b.x0 + x_off,
                b.y0 + y_off,
                b.z1 + 1,
                StructureOrientation::South,
            ),
            StructureOrientation::West => (
                b.x0 - 1,
                b.y0 + y_off,
                b.z0 + x_off,
                StructureOrientation::West,
            ),
            StructureOrientation::East => (
                b.x1 + 1,
                b.y0 + y_off,
                b.z0 + x_off,
                StructureOrientation::East,
            ),
        };
        self.generate_and_add_piece(random, x, y, z, orientation, piece.gen_depth);
    }

    fn child_left(
        &mut self,
        piece: &StrongholdPiecePlan,
        random: &mut MtRandom,
        y_off: i32,
        z_off: i32,
    ) {
        let b = piece.bounds;
        let (x, y, z, orientation) = match piece.orientation {
            StructureOrientation::North | StructureOrientation::South => (
                b.x0 - 1,
                b.y0 + y_off,
                b.z0 + z_off,
                StructureOrientation::West,
            ),
            StructureOrientation::West | StructureOrientation::East => (
                b.x0 + z_off,
                b.y0 + y_off,
                b.z0 - 1,
                StructureOrientation::North,
            ),
        };
        self.generate_and_add_piece(random, x, y, z, orientation, piece.gen_depth);
    }

    fn child_right(
        &mut self,
        piece: &StrongholdPiecePlan,
        random: &mut MtRandom,
        y_off: i32,
        z_off: i32,
    ) {
        let b = piece.bounds;
        let (x, y, z, orientation) = match piece.orientation {
            StructureOrientation::North | StructureOrientation::South => (
                b.x1 + 1,
                b.y0 + y_off,
                b.z0 + z_off,
                StructureOrientation::East,
            ),
            StructureOrientation::West | StructureOrientation::East => (
                b.x0 + z_off,
                b.y0 + y_off,
                b.z1 + 1,
                StructureOrientation::South,
            ),
        };
        self.generate_and_add_piece(random, x, y, z, orientation, piece.gen_depth);
    }

    fn generate_and_add_piece(
        &mut self,
        random: &mut MtRandom,
        foot_x: i32,
        foot_y: i32,
        foot_z: i32,
        orientation: StructureOrientation,
        depth: i32,
    ) -> Option<usize> {
        if depth > MAX_DEPTH
            || foot_x.wrapping_sub(self.start_bounds.x0).wrapping_abs() > MAX_DISTANCE
            || foot_z.wrapping_sub(self.start_bounds.z0).wrapping_abs() > MAX_DISTANCE
        {
            return None;
        }

        let piece = self.generate_piece_from_small_door(
            random,
            foot_x,
            foot_y,
            foot_z,
            orientation,
            depth + 1,
        )?;
        let index = self.pieces.len();
        self.pieces.push(piece);
        self.pending_children.push(index);
        Some(index)
    }

    fn generate_piece_from_small_door(
        &mut self,
        random_ref: &MtRandom,
        foot_x: i32,
        foot_y: i32,
        foot_z: i32,
        orientation: StructureOrientation,
        depth: i32,
    ) -> Option<StrongholdPiecePlan> {
        let mut random = random_ref.clone();

        if let Some(kind) = self.imposed_piece.take()
            && let Some(piece) = self.create_piece(
                kind,
                &mut random,
                foot_x,
                foot_y,
                foot_z,
                orientation,
                depth,
            )
        {
            return Some(piece);
        }

        let total_weight = self.total_weight();
        if total_weight <= 0 {
            return None;
        }

        for _ in 0..5 {
            let mut selection = random.next_int(total_weight as u32) as i32;
            let mut weight_index = 0_usize;
            while weight_index < self.weights.len() {
                selection -= self.weights[weight_index].weight;
                if selection < 0 {
                    let eligible = self.weights[weight_index].do_place(depth)
                        && !(Some(self.weights[weight_index].kind) == self.previous_piece
                            && self.weights.len() > 1);
                    if !eligible {
                        break;
                    }

                    let kind = self.weights[weight_index].kind;
                    if let Some(piece) = self.create_piece(
                        kind,
                        &mut random,
                        foot_x,
                        foot_y,
                        foot_z,
                        orientation,
                        depth,
                    ) {
                        self.weights[weight_index].place_count += 1;
                        self.previous_piece = Some(kind);
                        if !self.weights[weight_index].is_valid() {
                            self.weights.remove(weight_index);
                        }
                        return Some(piece);
                    }
                }
                weight_index += 1;
            }
        }

        let bounds = self.find_filler_corridor_box(foot_x, foot_y, foot_z, orientation)?;
        if bounds.y0 <= 1 {
            return None;
        }
        let entry_door = random_small_door(&mut random);
        let steps = if matches!(
            orientation,
            StructureOrientation::North | StructureOrientation::South
        ) {
            bounds.z_span()
        } else {
            bounds.x_span()
        };
        Some(StrongholdPiecePlan {
            kind: StrongholdPieceKind::FillerCorridor,
            bounds,
            orientation,
            gen_depth: depth,
            entry_door,
            extra: StrongholdPieceExtra::FillerCorridor { steps },
        })
    }

    fn total_weight(&self) -> i32 {
        let has_any_finite = self.weights.iter().any(|weight| {
            weight.max_place_count > 0 && weight.place_count < weight.max_place_count
        });
        if !has_any_finite {
            return -1;
        }
        self.weights.iter().map(|weight| weight.weight).sum()
    }

    #[allow(clippy::too_many_arguments)]
    fn create_piece(
        &self,
        kind: StrongholdPieceKind,
        random: &mut MtRandom,
        foot_x: i32,
        foot_y: i32,
        foot_z: i32,
        orientation: StructureOrientation,
        gen_depth: i32,
    ) -> Option<StrongholdPiecePlan> {
        let (bounds, entry_door, extra) = match kind {
            StrongholdPieceKind::Straight => {
                let bounds =
                    self.simple_box(foot_x, foot_y, foot_z, -1, -1, 0, 5, 5, 7, orientation)?;
                let entry_door = random_small_door(random);
                let left_child = random.next_int(2) == 0;
                let right_child = random.next_int(2) == 0;
                (
                    bounds,
                    entry_door,
                    StrongholdPieceExtra::Straight {
                        left_child,
                        right_child,
                    },
                )
            }
            StrongholdPieceKind::PrisonHall => (
                self.simple_box(foot_x, foot_y, foot_z, -1, -1, 0, 9, 5, 11, orientation)?,
                random_small_door(random),
                StrongholdPieceExtra::None,
            ),
            StrongholdPieceKind::LeftTurn | StrongholdPieceKind::RightTurn => (
                self.simple_box(foot_x, foot_y, foot_z, -1, -1, 0, 5, 5, 5, orientation)?,
                random_small_door(random),
                StrongholdPieceExtra::None,
            ),
            StrongholdPieceKind::RoomCrossing => {
                let bounds =
                    self.simple_box(foot_x, foot_y, foot_z, -4, -1, 0, 11, 7, 11, orientation)?;
                let entry_door = random_small_door(random);
                let room_type = random.next_int(5) as i32;
                (
                    bounds,
                    entry_door,
                    StrongholdPieceExtra::RoomCrossing { room_type },
                )
            }
            StrongholdPieceKind::StraightStairsDown => (
                self.simple_box(foot_x, foot_y, foot_z, -1, -7, 0, 5, 11, 8, orientation)?,
                random_small_door(random),
                StrongholdPieceExtra::None,
            ),
            StrongholdPieceKind::StairsDown => (
                self.simple_box(foot_x, foot_y, foot_z, -1, -7, 0, 5, 11, 5, orientation)?,
                random_small_door(random),
                StrongholdPieceExtra::StairsDown { is_source: false },
            ),
            StrongholdPieceKind::FiveCrossing => {
                let bounds =
                    self.simple_box(foot_x, foot_y, foot_z, -4, -3, 0, 10, 9, 11, orientation)?;
                let entry_door = random_small_door(random);
                let left_low = next_boolean(random);
                let left_high = next_boolean(random);
                let right_low = next_boolean(random);
                let right_high = random.next_int(3) > 0;
                (
                    bounds,
                    entry_door,
                    StrongholdPieceExtra::FiveCrossing {
                        left_high,
                        left_low,
                        right_high,
                        right_low,
                    },
                )
            }
            StrongholdPieceKind::ChestCorridor => (
                self.simple_box(foot_x, foot_y, foot_z, -1, -1, 0, 5, 5, 7, orientation)?,
                random_small_door(random),
                StrongholdPieceExtra::ChestCorridor {
                    has_placed_chest: false,
                },
            ),
            StrongholdPieceKind::Library => {
                let tall = StructureBounds::orient_box(
                    foot_x,
                    foot_y,
                    foot_z,
                    -4,
                    -1,
                    0,
                    14,
                    11,
                    15,
                    orientation,
                );
                let bounds = if self.box_ok(tall) {
                    tall
                } else {
                    let short = StructureBounds::orient_box(
                        foot_x,
                        foot_y,
                        foot_z,
                        -4,
                        -1,
                        0,
                        14,
                        6,
                        15,
                        orientation,
                    );
                    if !self.box_ok(short) {
                        return None;
                    }
                    short
                };
                (
                    bounds,
                    random_small_door(random),
                    StrongholdPieceExtra::Library {
                        is_tall: bounds.y_span() > 6,
                    },
                )
            }
            StrongholdPieceKind::PortalRoom => (
                self.simple_box(foot_x, foot_y, foot_z, -4, -1, 0, 11, 8, 16, orientation)?,
                StrongholdDoor::Opening,
                StrongholdPieceExtra::PortalRoom {
                    has_placed_mob_spawner: false,
                },
            ),
            StrongholdPieceKind::FillerCorridor => return None,
        };

        Some(StrongholdPiecePlan {
            kind,
            bounds,
            orientation,
            gen_depth,
            entry_door,
            extra,
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn simple_box(
        &self,
        foot_x: i32,
        foot_y: i32,
        foot_z: i32,
        off_x: i32,
        off_y: i32,
        off_z: i32,
        width: i32,
        height: i32,
        depth: i32,
        orientation: StructureOrientation,
    ) -> Option<StructureBounds> {
        let bounds = StructureBounds::orient_box(
            foot_x,
            foot_y,
            foot_z,
            off_x,
            off_y,
            off_z,
            width,
            height,
            depth,
            orientation,
        );
        self.box_ok(bounds).then_some(bounds)
    }

    fn box_ok(&self, bounds: StructureBounds) -> bool {
        bounds.y0 > LOWEST_Y && !self.collides(bounds)
    }

    fn collides(&self, bounds: StructureBounds) -> bool {
        self.pieces
            .iter()
            .any(|piece| piece.bounds.intersects(bounds))
    }

    fn first_collision(&self, bounds: StructureBounds) -> Option<StructureBounds> {
        self.pieces
            .iter()
            .find(|piece| piece.bounds.intersects(bounds))
            .map(|piece| piece.bounds)
    }

    fn find_filler_corridor_box(
        &self,
        foot_x: i32,
        foot_y: i32,
        foot_z: i32,
        orientation: StructureOrientation,
    ) -> Option<StructureBounds> {
        let full =
            StructureBounds::orient_box(foot_x, foot_y, foot_z, -1, -1, 0, 5, 5, 4, orientation);
        let collision = self.first_collision(full)?;
        if collision.y0 != full.y0 {
            return None;
        }

        for depth in (1..4).rev() {
            let probe = StructureBounds::orient_box(
                foot_x,
                foot_y,
                foot_z,
                -1,
                -1,
                0,
                5,
                5,
                depth - 1,
                orientation,
            );
            if !collision.intersects(probe) {
                return Some(StructureBounds::orient_box(
                    foot_x,
                    foot_y,
                    foot_z,
                    -1,
                    -1,
                    0,
                    5,
                    5,
                    depth,
                    orientation,
                ));
            }
        }
        None
    }
}

fn random_small_door(random: &mut MtRandom) -> StrongholdDoor {
    match random.next_int(5) {
        0 | 1 | 4 => StrongholdDoor::Opening,
        2 => StrongholdDoor::WoodDoor,
        3 => StrongholdDoor::Grates,
        _ => unreachable!(),
    }
}

fn next_boolean(random: &mut MtRandom) -> bool {
    random.next_u32() & 0x0800_0000 != 0
}

fn orientation_from_u32(value: u32) -> StructureOrientation {
    match value {
        0 => StructureOrientation::South,
        1 => StructureOrientation::West,
        2 => StructureOrientation::North,
        3 => StructureOrientation::East,
        _ => unreachable!(),
    }
}

fn calculate_bounds(pieces: &[StrongholdPiecePlan]) -> StructureBounds {
    let mut bounds = StructureBounds::unknown();
    for piece in pieces {
        bounds.expand(piece.bounds);
    }
    bounds
}

#[cfg(test)]
fn fnv_byte(hash: u64, byte: u8) -> u64 {
    (hash ^ u64::from(byte)).wrapping_mul(0x100_0000_01b3)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_forces_five_crossing_as_first_child() {
        let plan = StrongholdPlan::generate(ChunkCoord::new(0, 0), MtRandom::new(0x1234_5678));
        assert!(plan.pieces.len() >= 2);
        assert_eq!(plan.pieces[0].kind, StrongholdPieceKind::StairsDown);
        assert!(matches!(
            plan.pieces[0].extra,
            StrongholdPieceExtra::StairsDown { is_source: true }
        ));
        assert_eq!(plan.pieces[1].kind, StrongholdPieceKind::FiveCrossing);
        assert_ne!(plan.topology_hash(), 0);
    }

    #[test]
    fn generated_graph_respects_target_depth_distance_and_floor_limits() {
        let plan = StrongholdPlan::generate(ChunkCoord::new(-55, -25), MtRandom::new(0xdead_beef));
        let start = plan.pieces[0].bounds;
        for piece in &plan.pieces {
            assert!(piece.gen_depth <= MAX_DEPTH + 1);
            assert!(piece.bounds.y0 >= 1);
            assert!(piece.bounds.x0.wrapping_sub(start.x0).wrapping_abs() <= 128);
            assert!(piece.bounds.z0.wrapping_sub(start.z0).wrapping_abs() <= 128);
        }
    }

    #[test]
    fn copied_piece_rng_leaves_main_stream_unchanged_until_pending_selection() {
        let source = ChunkCoord::new(0, 0);
        let mut random = MtRandom::new(7);
        let orientation = orientation_from_u32(random.next_int(4));
        let start = StrongholdPiecePlan {
            kind: StrongholdPieceKind::StairsDown,
            bounds: StructureBounds::new(2, 64, 2, 6, 74, 6),
            orientation,
            gen_depth: 0,
            entry_door: StrongholdDoor::Opening,
            extra: StrongholdPieceExtra::StairsDown { is_source: true },
        };
        let mut builder = StrongholdPlanBuilder::new(start);
        let mut expected = random.clone();
        builder.add_children(0, &mut random);
        assert_eq!(random.next_u32(), expected.next_u32());
        assert_eq!(source, ChunkCoord::new(0, 0));
    }
}
