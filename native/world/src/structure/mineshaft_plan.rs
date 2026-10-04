use crate::ChunkCoord;
use crate::terrain_shape::noise::MtRandom;

use super::{StructureBounds, StructureOrientation, StructureStartCore};

const SHAFT_WIDTH: i32 = 3;
const SHAFT_HEIGHT: i32 = 3;
const SHAFT_LENGTH: i32 = 5;
const MAX_DEPTH: i32 = 8;
const SEA_LEVEL: i32 = 63;

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
#[repr(u8)]
pub(crate) enum MineshaftPieceKind {
    Room = 0,
    Corridor = 1,
    Crossing = 2,
    Stairs = 3,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) enum MineshaftPieceExtra {
    Room {
        entrances: Vec<StructureBounds>,
    },
    Corridor {
        has_rails: bool,
        spider_corridor: bool,
        has_placed_spider: bool,
        num_sections: i32,
    },
    Crossing {
        direction: StructureOrientation,
        two_floored: bool,
    },
    Stairs,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct MineshaftPiecePlan {
    pub(crate) kind: MineshaftPieceKind,
    pub(crate) bounds: StructureBounds,
    pub(crate) orientation: StructureOrientation,
    pub(crate) gen_depth: i32,
    pub(crate) extra: MineshaftPieceExtra,
}

impl MineshaftPiecePlan {
    fn move_by(&mut self, dx: i32, dy: i32, dz: i32) {
        self.bounds.move_by(dx, dy, dz);
        if let MineshaftPieceExtra::Room { entrances } = &mut self.extra {
            for entrance in entrances {
                entrance.move_by(dx, dy, dz);
            }
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct MineshaftPlan {
    pub(crate) core: StructureStartCore,
    pub(crate) surface: bool,
    pub(crate) pieces: Vec<MineshaftPiecePlan>,
}

impl MineshaftPlan {
    pub(crate) fn generate(source: ChunkCoord, mut random: MtRandom, surface: bool) -> Self {
        let west = source.x().wrapping_mul(16).wrapping_add(2);
        let north = source.z().wrapping_mul(16).wrapping_add(2);
        let room_z1 = north.wrapping_add(7 + random.next_int(6) as i32);
        let room_y1 = 54 + random.next_int(6) as i32;
        let room_x1 = west.wrapping_add(7 + random.next_int(6) as i32);
        let room = MineshaftPiecePlan {
            kind: MineshaftPieceKind::Room,
            bounds: StructureBounds::new(west, 50, north, room_x1, room_y1, room_z1),
            orientation: StructureOrientation::South,
            gen_depth: 0,
            extra: MineshaftPieceExtra::Room {
                entrances: Vec::new(),
            },
        };

        let start_bounds = room.bounds;
        let mut builder = MineshaftPlanBuilder {
            surface,
            start_bounds,
            pieces: vec![room],
        };
        builder.add_children(0, &mut random);

        let mut bounds = calculate_bounds(&builder.pieces);
        let dy = if surface {
            SEA_LEVEL
                .wrapping_sub(bounds.y1)
                .wrapping_add(bounds.y_span() / 2)
                .wrapping_add(5)
        } else {
            let max_y = SEA_LEVEL - 10;
            let mut y1_pos = bounds.y_span() + 1;
            if y1_pos < max_y {
                y1_pos += random.next_int((max_y - y1_pos) as u32) as i32;
            }
            y1_pos - bounds.y1
        };

        bounds.move_by(0, dy, 0);
        for piece in &mut builder.pieces {
            piece.move_by(0, dy, 0);
        }

        Self {
            core: StructureStartCore::new(source, bounds),
            surface,
            pieces: builder.pieces,
        }
    }

    #[cfg(test)]
    pub(crate) fn hash_topology(&self) -> u64 {
        let mut hash = 0xcbf2_9ce4_8422_2325_u64;
        hash = fnv_byte(hash, u8::from(self.surface));
        for piece in &self.pieces {
            hash = fnv_byte(hash, piece.kind as u8);
            hash = fnv_byte(hash, piece.orientation as u8);
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
            match &piece.extra {
                MineshaftPieceExtra::Room { entrances } => {
                    hash = fnv_byte(hash, 0);
                    for entrance in entrances {
                        for value in [
                            entrance.x0,
                            entrance.y0,
                            entrance.z0,
                            entrance.x1,
                            entrance.y1,
                            entrance.z1,
                        ] {
                            for byte in value.to_le_bytes() {
                                hash = fnv_byte(hash, byte);
                            }
                        }
                    }
                }
                MineshaftPieceExtra::Corridor {
                    has_rails,
                    spider_corridor,
                    has_placed_spider,
                    num_sections,
                } => {
                    hash = fnv_byte(hash, 1);
                    hash = fnv_byte(hash, u8::from(*has_rails));
                    hash = fnv_byte(hash, u8::from(*spider_corridor));
                    hash = fnv_byte(hash, u8::from(*has_placed_spider));
                    for byte in num_sections.to_le_bytes() {
                        hash = fnv_byte(hash, byte);
                    }
                }
                MineshaftPieceExtra::Crossing {
                    direction,
                    two_floored,
                } => {
                    hash = fnv_byte(hash, 2);
                    hash = fnv_byte(hash, *direction as u8);
                    hash = fnv_byte(hash, u8::from(*two_floored));
                }
                MineshaftPieceExtra::Stairs => {
                    hash = fnv_byte(hash, 3);
                }
            }
        }
        hash
    }
}

struct MineshaftPlanBuilder {
    surface: bool,
    start_bounds: StructureBounds,
    pieces: Vec<MineshaftPiecePlan>,
}

impl MineshaftPlanBuilder {
    fn add_children(&mut self, index: usize, random: &mut MtRandom) {
        let piece = self.pieces[index].clone();
        match piece.kind {
            MineshaftPieceKind::Room => self.add_room_children(index, piece, random),
            MineshaftPieceKind::Corridor => self.add_corridor_children(piece, random),
            MineshaftPieceKind::Crossing => self.add_crossing_children(piece, random),
            MineshaftPieceKind::Stairs => self.add_stairs_children(piece, random),
        }
    }

    fn add_room_children(
        &mut self,
        room_index: usize,
        room: MineshaftPiecePlan,
        random: &mut MtRandom,
    ) {
        let room_chance = if self.surface { 0.5 } else { 1.0 };
        if random.next_float() > room_chance {
            return;
        }

        let depth = room.gen_depth;
        let mut height_space = room.bounds.y_span() - SHAFT_HEIGHT - 1;
        if height_space <= 0 {
            height_space = 1;
        }

        let mut pos = 0;
        while pos < room.bounds.x_span() {
            pos += random.next_int(room.bounds.x_span() as u32) as i32;
            if pos + SHAFT_WIDTH > room.bounds.x_span() {
                break;
            }
            let child = self.generate_with_random_y(
                random,
                room.bounds.x0 + pos,
                room.bounds.y0 + 1,
                height_space as u32,
                room.bounds.z0 - 1,
                StructureOrientation::North,
                depth,
            );
            if let Some(child_index) = child {
                let child_box = self.pieces[child_index].bounds;
                self.room_entrances_mut(room_index)
                    .push(StructureBounds::new(
                        child_box.x0,
                        child_box.y0,
                        room.bounds.z0,
                        child_box.x1,
                        child_box.y1,
                        room.bounds.z0 + 1,
                    ));
            }
            pos += SHAFT_WIDTH + 1;
        }

        pos = 0;
        while pos < room.bounds.x_span() {
            pos += random.next_int(room.bounds.x_span() as u32) as i32;
            if pos + SHAFT_WIDTH > room.bounds.x_span() {
                break;
            }
            let child = self.generate_with_random_y(
                random,
                room.bounds.x0 + pos,
                room.bounds.y0 + 1,
                height_space as u32,
                room.bounds.z1 + 1,
                StructureOrientation::South,
                depth,
            );
            if let Some(child_index) = child {
                let child_box = self.pieces[child_index].bounds;
                self.room_entrances_mut(room_index)
                    .push(StructureBounds::new(
                        child_box.x0,
                        child_box.y0,
                        room.bounds.z1 - 1,
                        child_box.x1,
                        child_box.y1,
                        room.bounds.z1,
                    ));
            }
            pos += SHAFT_WIDTH + 1;
        }

        pos = 0;
        while pos < room.bounds.z_span() {
            pos += random.next_int(room.bounds.z_span() as u32) as i32;
            if pos + SHAFT_WIDTH > room.bounds.z_span() {
                break;
            }
            let child = self.generate_with_random_y(
                random,
                room.bounds.x0 - 1,
                room.bounds.y0 + 1,
                height_space as u32,
                room.bounds.z0 + pos,
                StructureOrientation::West,
                depth,
            );
            if let Some(child_index) = child {
                let child_box = self.pieces[child_index].bounds;
                self.room_entrances_mut(room_index)
                    .push(StructureBounds::new(
                        room.bounds.x0,
                        child_box.y0,
                        child_box.z0,
                        room.bounds.x0 + 1,
                        child_box.y1,
                        child_box.z1,
                    ));
            }
            pos += SHAFT_WIDTH + 1;
        }

        pos = 0;
        while pos < room.bounds.z_span() {
            pos += random.next_int(room.bounds.z_span() as u32) as i32;
            if pos + SHAFT_WIDTH > room.bounds.z_span() {
                break;
            }
            let child = self.generate_with_random_y(
                random,
                room.bounds.x1 + 1,
                room.bounds.y0 + 1,
                height_space as u32,
                room.bounds.z0 + pos,
                StructureOrientation::East,
                depth,
            );
            if let Some(child_index) = child {
                let child_box = self.pieces[child_index].bounds;
                self.room_entrances_mut(room_index)
                    .push(StructureBounds::new(
                        room.bounds.x1 - 1,
                        child_box.y0,
                        child_box.z0,
                        room.bounds.x1,
                        child_box.y1,
                        child_box.z1,
                    ));
            }
            pos += SHAFT_WIDTH + 1;
        }
    }

    fn add_corridor_children(&mut self, corridor: MineshaftPiecePlan, random: &mut MtRandom) {
        let depth = corridor.gen_depth;
        let end_selection = random.next_int(4);
        let b = corridor.bounds;
        match corridor.orientation {
            StructureOrientation::North => {
                if end_selection <= 1 {
                    self.generate_with_random_y(
                        random,
                        b.x0,
                        b.y0 - 1,
                        3,
                        b.z0 - 1,
                        StructureOrientation::North,
                        depth,
                    );
                } else if end_selection == 2 {
                    self.generate_with_random_y(
                        random,
                        b.x0 - 1,
                        b.y0 - 1,
                        3,
                        b.z0,
                        StructureOrientation::West,
                        depth,
                    );
                } else {
                    self.generate_with_random_y(
                        random,
                        b.x1 + 1,
                        b.y0 - 1,
                        3,
                        b.z0,
                        StructureOrientation::East,
                        depth,
                    );
                }
            }
            StructureOrientation::South => {
                if end_selection <= 1 {
                    self.generate_with_random_y(
                        random,
                        b.x0,
                        b.y0 - 1,
                        3,
                        b.z1 + 1,
                        StructureOrientation::South,
                        depth,
                    );
                } else if end_selection == 2 {
                    self.generate_with_random_y(
                        random,
                        b.x0 - 1,
                        b.y0 - 1,
                        3,
                        b.z1 - SHAFT_WIDTH,
                        StructureOrientation::West,
                        depth,
                    );
                } else {
                    self.generate_with_random_y(
                        random,
                        b.x1 + 1,
                        b.y0 - 1,
                        3,
                        b.z1 - SHAFT_WIDTH,
                        StructureOrientation::East,
                        depth,
                    );
                }
            }
            StructureOrientation::West => {
                if end_selection <= 1 {
                    self.generate_with_random_y(
                        random,
                        b.x0 - 1,
                        b.y0 - 1,
                        3,
                        b.z0,
                        StructureOrientation::West,
                        depth,
                    );
                } else if end_selection == 2 {
                    self.generate_with_random_y(
                        random,
                        b.x0,
                        b.y0 - 1,
                        3,
                        b.z0 - 1,
                        StructureOrientation::North,
                        depth,
                    );
                } else {
                    self.generate_with_random_y(
                        random,
                        b.x0,
                        b.y0 - 1,
                        3,
                        b.z1 + 1,
                        StructureOrientation::South,
                        depth,
                    );
                }
            }
            StructureOrientation::East => {
                if end_selection <= 1 {
                    self.generate_with_random_y(
                        random,
                        b.x1 + 1,
                        b.y0 - 1,
                        3,
                        b.z0,
                        StructureOrientation::East,
                        depth,
                    );
                } else if end_selection == 2 {
                    self.generate_with_random_y(
                        random,
                        b.x1 - SHAFT_WIDTH,
                        b.y0 - 1,
                        3,
                        b.z0 - 1,
                        StructureOrientation::North,
                        depth,
                    );
                } else {
                    self.generate_with_random_y(
                        random,
                        b.x1 - SHAFT_WIDTH,
                        b.y0 - 1,
                        3,
                        b.z1 + 1,
                        StructureOrientation::South,
                        depth,
                    );
                }
            }
        }

        if depth >= MAX_DEPTH {
            return;
        }
        if matches!(
            corridor.orientation,
            StructureOrientation::North | StructureOrientation::South
        ) {
            let mut z = b.z0 + 3;
            while z + SHAFT_WIDTH <= b.z1 {
                match random.next_int(5) {
                    0 => {
                        self.generate_and_add_piece(
                            random,
                            b.x0 - 1,
                            b.y0,
                            z,
                            StructureOrientation::West,
                            depth + 1,
                        );
                    }
                    1 => {
                        self.generate_and_add_piece(
                            random,
                            b.x1 + 1,
                            b.y0,
                            z,
                            StructureOrientation::East,
                            depth + 1,
                        );
                    }
                    _ => {}
                }
                z += SHAFT_LENGTH;
            }
        } else {
            let mut x = b.x0 + 3;
            while x + SHAFT_WIDTH <= b.x1 {
                match random.next_int(5) {
                    0 => {
                        self.generate_and_add_piece(
                            random,
                            x,
                            b.y0,
                            b.z0 - 1,
                            StructureOrientation::North,
                            depth + 1,
                        );
                    }
                    1 => {
                        self.generate_and_add_piece(
                            random,
                            x,
                            b.y0,
                            b.z1 + 1,
                            StructureOrientation::South,
                            depth + 1,
                        );
                    }
                    _ => {}
                }
                x += SHAFT_LENGTH;
            }
        }
    }

    fn add_crossing_children(&mut self, crossing: MineshaftPiecePlan, random: &mut MtRandom) {
        let MineshaftPieceExtra::Crossing {
            direction,
            two_floored,
        } = crossing.extra
        else {
            unreachable!();
        };
        let depth = crossing.gen_depth;
        let b = crossing.bounds;
        match direction {
            StructureOrientation::North => {
                self.generate_and_add_piece(
                    random,
                    b.x0 + 1,
                    b.y0,
                    b.z0 - 1,
                    StructureOrientation::North,
                    depth,
                );
                self.generate_and_add_piece(
                    random,
                    b.x0 - 1,
                    b.y0,
                    b.z0 + 1,
                    StructureOrientation::West,
                    depth,
                );
                self.generate_and_add_piece(
                    random,
                    b.x1 + 1,
                    b.y0,
                    b.z0 + 1,
                    StructureOrientation::East,
                    depth,
                );
            }
            StructureOrientation::South => {
                self.generate_and_add_piece(
                    random,
                    b.x0 + 1,
                    b.y0,
                    b.z1 + 1,
                    StructureOrientation::South,
                    depth,
                );
                self.generate_and_add_piece(
                    random,
                    b.x0 - 1,
                    b.y0,
                    b.z0 + 1,
                    StructureOrientation::West,
                    depth,
                );
                self.generate_and_add_piece(
                    random,
                    b.x1 + 1,
                    b.y0,
                    b.z0 + 1,
                    StructureOrientation::East,
                    depth,
                );
            }
            StructureOrientation::West => {
                self.generate_and_add_piece(
                    random,
                    b.x0 + 1,
                    b.y0,
                    b.z0 - 1,
                    StructureOrientation::North,
                    depth,
                );
                self.generate_and_add_piece(
                    random,
                    b.x0 + 1,
                    b.y0,
                    b.z1 + 1,
                    StructureOrientation::South,
                    depth,
                );
                self.generate_and_add_piece(
                    random,
                    b.x0 - 1,
                    b.y0,
                    b.z0 + 1,
                    StructureOrientation::West,
                    depth,
                );
            }
            StructureOrientation::East => {
                self.generate_and_add_piece(
                    random,
                    b.x0 + 1,
                    b.y0,
                    b.z0 - 1,
                    StructureOrientation::North,
                    depth,
                );
                self.generate_and_add_piece(
                    random,
                    b.x0 + 1,
                    b.y0,
                    b.z1 + 1,
                    StructureOrientation::South,
                    depth,
                );
                self.generate_and_add_piece(
                    random,
                    b.x1 + 1,
                    b.y0,
                    b.z0 + 1,
                    StructureOrientation::East,
                    depth,
                );
            }
        }

        if !two_floored {
            return;
        }
        let upper_y = b.y0 + SHAFT_HEIGHT + 1;
        if next_boolean(random) {
            self.generate_and_add_piece(
                random,
                b.x0 + 1,
                upper_y,
                b.z0 - 1,
                StructureOrientation::North,
                depth,
            );
        }
        if next_boolean(random) {
            self.generate_and_add_piece(
                random,
                b.x0 - 1,
                upper_y,
                b.z0 + 1,
                StructureOrientation::West,
                depth,
            );
        }
        if next_boolean(random) {
            self.generate_and_add_piece(
                random,
                b.x1 + 1,
                upper_y,
                b.z0 + 1,
                StructureOrientation::East,
                depth,
            );
        }
        if next_boolean(random) {
            self.generate_and_add_piece(
                random,
                b.x0 + 1,
                upper_y,
                b.z1 + 1,
                StructureOrientation::South,
                depth,
            );
        }
    }

    fn add_stairs_children(&mut self, stairs: MineshaftPiecePlan, random: &mut MtRandom) {
        let depth = stairs.gen_depth;
        let b = stairs.bounds;
        match stairs.orientation {
            StructureOrientation::North => {
                self.generate_and_add_piece(
                    random,
                    b.x0,
                    b.y0,
                    b.z0 - 1,
                    StructureOrientation::North,
                    depth,
                );
            }
            StructureOrientation::South => {
                self.generate_and_add_piece(
                    random,
                    b.x0,
                    b.y0,
                    b.z1 + 1,
                    StructureOrientation::South,
                    depth,
                );
            }
            StructureOrientation::West => {
                self.generate_and_add_piece(
                    random,
                    b.x0 - 1,
                    b.y0,
                    b.z0,
                    StructureOrientation::West,
                    depth,
                );
            }
            StructureOrientation::East => {
                self.generate_and_add_piece(
                    random,
                    b.x1 + 1,
                    b.y0,
                    b.z0,
                    StructureOrientation::East,
                    depth,
                );
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn generate_with_random_y(
        &mut self,
        random: &mut MtRandom,
        foot_x: i32,
        base_y: i32,
        bound: u32,
        foot_z: i32,
        orientation: StructureOrientation,
        depth: i32,
    ) -> Option<usize> {
        let foot_y = base_y + random.next_int(bound) as i32;
        self.generate_and_add_piece(random, foot_x, foot_y, foot_z, orientation, depth)
    }

    #[allow(clippy::too_many_arguments)]
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
            || foot_x.wrapping_sub(self.start_bounds.x0).wrapping_abs() > 5 * 16
            || foot_z.wrapping_sub(self.start_bounds.z0).wrapping_abs() > 5 * 16
        {
            return None;
        }

        let piece =
            self.create_random_piece(random, foot_x, foot_y, foot_z, orientation, depth + 1)?;
        let index = self.pieces.len();
        self.pieces.push(piece);
        self.add_children(index, random);
        Some(index)
    }

    #[allow(clippy::too_many_arguments)]
    fn create_random_piece(
        &self,
        random: &mut MtRandom,
        foot_x: i32,
        foot_y: i32,
        foot_z: i32,
        orientation: StructureOrientation,
        gen_depth: i32,
    ) -> Option<MineshaftPiecePlan> {
        let selection = random.next_int(100);
        if selection >= 80 {
            let bounds = self.find_crossing(random, foot_x, foot_y, foot_z, orientation)?;
            return Some(MineshaftPiecePlan {
                kind: MineshaftPieceKind::Crossing,
                bounds,
                orientation,
                gen_depth,
                extra: MineshaftPieceExtra::Crossing {
                    direction: orientation,
                    two_floored: bounds.y_span() > SHAFT_HEIGHT,
                },
            });
        }
        if selection >= 70 {
            let bounds = self.find_stairs(foot_x, foot_y, foot_z, orientation)?;
            return Some(MineshaftPiecePlan {
                kind: MineshaftPieceKind::Stairs,
                bounds,
                orientation,
                gen_depth,
                extra: MineshaftPieceExtra::Stairs,
            });
        }

        let bounds = self.find_corridor(random, foot_x, foot_y, foot_z, orientation)?;
        let has_rails = random.next_int(3) == 0;
        let spider_corridor = !has_rails && random.next_int(23) == 0 && bounds.y1 < 50;
        let num_sections = if matches!(
            orientation,
            StructureOrientation::North | StructureOrientation::South
        ) {
            bounds.z_span() / SHAFT_LENGTH
        } else {
            bounds.x_span() / SHAFT_LENGTH
        };
        Some(MineshaftPiecePlan {
            kind: MineshaftPieceKind::Corridor,
            bounds,
            orientation,
            gen_depth,
            extra: MineshaftPieceExtra::Corridor {
                has_rails,
                spider_corridor,
                has_placed_spider: false,
                num_sections,
            },
        })
    }

    fn find_corridor(
        &self,
        random: &mut MtRandom,
        foot_x: i32,
        foot_y: i32,
        foot_z: i32,
        orientation: StructureOrientation,
    ) -> Option<StructureBounds> {
        let mut sections = random.next_int(3) as i32 + 2;
        let mut bounds = StructureBounds::new(
            foot_x,
            foot_y,
            foot_z,
            foot_x,
            foot_y + SHAFT_HEIGHT - 1,
            foot_z,
        );

        while sections > 0 {
            let length = sections * SHAFT_LENGTH;
            bounds = match orientation {
                StructureOrientation::North => StructureBounds::new(
                    foot_x,
                    foot_y,
                    foot_z - (length - 1),
                    foot_x + SHAFT_WIDTH - 1,
                    foot_y + SHAFT_HEIGHT - 1,
                    foot_z,
                ),
                StructureOrientation::South => StructureBounds::new(
                    foot_x,
                    foot_y,
                    foot_z,
                    foot_x + SHAFT_WIDTH - 1,
                    foot_y + SHAFT_HEIGHT - 1,
                    foot_z + length - 1,
                ),
                StructureOrientation::West => StructureBounds::new(
                    foot_x - (length - 1),
                    foot_y,
                    foot_z,
                    foot_x,
                    foot_y + SHAFT_HEIGHT - 1,
                    foot_z + SHAFT_WIDTH - 1,
                ),
                StructureOrientation::East => StructureBounds::new(
                    foot_x,
                    foot_y,
                    foot_z,
                    foot_x + length - 1,
                    foot_y + SHAFT_HEIGHT - 1,
                    foot_z + SHAFT_WIDTH - 1,
                ),
            };
            if !self.collides(bounds) {
                break;
            }
            sections -= 1;
        }
        (sections > 0).then_some(bounds)
    }

    fn find_crossing(
        &self,
        random: &mut MtRandom,
        foot_x: i32,
        foot_y: i32,
        foot_z: i32,
        orientation: StructureOrientation,
    ) -> Option<StructureBounds> {
        let extra_height = if random.next_int(4) == 0 {
            SHAFT_HEIGHT + 1
        } else {
            0
        };
        let y1 = foot_y + SHAFT_HEIGHT - 1 + extra_height;
        let bounds = match orientation {
            StructureOrientation::North => StructureBounds::new(
                foot_x - 1,
                foot_y,
                foot_z - (SHAFT_WIDTH + 1),
                foot_x + SHAFT_WIDTH,
                y1,
                foot_z,
            ),
            StructureOrientation::South => StructureBounds::new(
                foot_x - 1,
                foot_y,
                foot_z,
                foot_x + SHAFT_WIDTH,
                y1,
                foot_z + SHAFT_WIDTH + 1,
            ),
            StructureOrientation::West => StructureBounds::new(
                foot_x - (SHAFT_WIDTH + 1),
                foot_y,
                foot_z - 1,
                foot_x,
                y1,
                foot_z + SHAFT_WIDTH,
            ),
            StructureOrientation::East => StructureBounds::new(
                foot_x,
                foot_y,
                foot_z - 1,
                foot_x + SHAFT_WIDTH + 1,
                y1,
                foot_z + SHAFT_WIDTH,
            ),
        };
        (!self.collides(bounds)).then_some(bounds)
    }

    fn find_stairs(
        &self,
        foot_x: i32,
        foot_y: i32,
        foot_z: i32,
        orientation: StructureOrientation,
    ) -> Option<StructureBounds> {
        let bounds = match orientation {
            StructureOrientation::North => StructureBounds::new(
                foot_x,
                foot_y - 5,
                foot_z - 8,
                foot_x + SHAFT_WIDTH - 1,
                foot_y + SHAFT_HEIGHT - 1,
                foot_z,
            ),
            StructureOrientation::South => StructureBounds::new(
                foot_x,
                foot_y - 5,
                foot_z,
                foot_x + SHAFT_WIDTH - 1,
                foot_y + SHAFT_HEIGHT - 1,
                foot_z + 8,
            ),
            StructureOrientation::West => StructureBounds::new(
                foot_x - 8,
                foot_y - 5,
                foot_z,
                foot_x,
                foot_y + SHAFT_HEIGHT - 1,
                foot_z + SHAFT_WIDTH - 1,
            ),
            StructureOrientation::East => StructureBounds::new(
                foot_x,
                foot_y - 5,
                foot_z,
                foot_x + 8,
                foot_y + SHAFT_HEIGHT - 1,
                foot_z + SHAFT_WIDTH - 1,
            ),
        };
        (!self.collides(bounds)).then_some(bounds)
    }

    fn collides(&self, bounds: StructureBounds) -> bool {
        self.pieces
            .iter()
            .any(|piece| piece.bounds.intersects(bounds))
    }

    fn room_entrances_mut(&mut self, index: usize) -> &mut Vec<StructureBounds> {
        let MineshaftPieceExtra::Room { entrances } = &mut self.pieces[index].extra else {
            unreachable!();
        };
        entrances
    }
}

fn calculate_bounds(pieces: &[MineshaftPiecePlan]) -> StructureBounds {
    let mut bounds = StructureBounds::unknown();
    for piece in pieces {
        bounds.expand(piece.bounds);
    }
    bounds
}

fn next_boolean(random: &mut MtRandom) -> bool {
    random.next_u32() & 0x0800_0000 != 0
}

#[cfg(test)]
fn fnv_byte(hash: u64, byte: u8) -> u64 {
    (hash ^ u64::from(byte)).wrapping_mul(0x100_0000_01b3)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normal_and_surface_vertical_moves_stay_inside_target_generation_height() {
        for (surface, source) in [
            (false, ChunkCoord::new(4, -9)),
            (true, ChunkCoord::new(-8, 7)),
        ] {
            let plan = MineshaftPlan::generate(source, MtRandom::new(0x1234_5678), surface);
            assert!(!plan.pieces.is_empty());
            assert!(plan.core.bounds().y0 >= 1);
            assert!(plan.core.bounds().y1 < 128);
        }
    }

    #[test]
    fn every_piece_respects_target_depth_and_distance_limits() {
        let plan =
            MineshaftPlan::generate(ChunkCoord::new(-13, 19), MtRandom::new(0xdead_beef), false);
        let room = plan.pieces[0].bounds;
        for piece in &plan.pieces {
            assert!(piece.gen_depth <= MAX_DEPTH + 1);
            assert!(piece.bounds.x0.wrapping_sub(room.x0).wrapping_abs() <= 96);
            assert!(piece.bounds.z0.wrapping_sub(room.z0).wrapping_abs() <= 96);
        }
    }
}
