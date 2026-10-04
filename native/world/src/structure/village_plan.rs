use crate::ChunkCoord;
use crate::terrain_shape::noise::MtRandom;

use super::{StructureBounds, StructureOrientation, StructureStartCore};

const LOWEST_Y_POSITION: i32 = 10;
const MAX_DEPTH: i32 = 50;
const BASE_ROAD_DEPTH: i32 = 3;
const MAX_OFFSET_FROM_START: i32 = 7 * 16;

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub(crate) enum VillageStyle {
    Plains,
    Desert,
    Savanna,
    Taiga,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
#[repr(u8)]
pub(crate) enum VillagePieceKind {
    Start = 0,
    SimpleHouse = 1,
    SmallTemple = 2,
    BookHouse = 3,
    SmallHut = 4,
    PigHouse = 5,
    DoubleFarmland = 6,
    Farmland = 7,
    Smithy = 8,
    TwoRoomHouse = 9,
    LightPost = 10,
    StraightRoad = 11,
}

impl VillagePieceKind {
    pub(crate) fn dimensions(self) -> Option<(i32, i32, i32)> {
        match self {
            Self::Start => Some((6, 15, 6)),
            Self::SimpleHouse => Some((5, 6, 5)),
            Self::SmallTemple => Some((5, 12, 9)),
            Self::BookHouse => Some((9, 9, 6)),
            Self::SmallHut => Some((4, 6, 5)),
            Self::PigHouse => Some((9, 7, 11)),
            Self::DoubleFarmland => Some((13, 4, 9)),
            Self::Farmland => Some((7, 4, 9)),
            Self::Smithy => Some((10, 6, 7)),
            Self::TwoRoomHouse => Some((9, 7, 12)),
            Self::LightPost => Some((3, 4, 2)),
            Self::StraightRoad => None,
        }
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub(crate) enum VillageCrop {
    Wheat,
    Potato,
    Beetroot,
    Carrot,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) enum VillagePieceExtra {
    None,
    SimpleHouse { terrace: bool },
    SmallHut { low_ceiling: bool, table: u8 },
    Farmland { crops: [VillageCrop; 2] },
    DoubleFarmland { crops: [VillageCrop; 4] },
    StraightRoad { length: i32 },
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct VillagePiecePlan {
    pub(crate) kind: VillagePieceKind,
    pub(crate) bounds: StructureBounds,
    pub(crate) orientation: StructureOrientation,
    pub(crate) gen_depth: i32,
    pub(crate) extra: VillagePieceExtra,
    pub(crate) height_position: i32,
    pub(crate) placed_chest: bool,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct VillagePlan {
    pub(crate) core: StructureStartCore,
    pub(crate) style: VillageStyle,
    pub(crate) abandoned: bool,
    pub(crate) pieces: Vec<VillagePiecePlan>,
}

#[derive(Debug, Clone)]
struct PieceWeight {
    kind: VillagePieceKind,
    weight: i32,
    place_count: i32,
    max_place_count: i32,
}

impl PieceWeight {
    fn do_place(&self, depth: i32) -> bool {
        self.place_count < self.max_place_count && depth >= 0
    }

    fn is_valid(&self) -> bool {
        self.place_count < self.max_place_count
    }
}

struct Planner {
    start_bounds: StructureBounds,
    village_size: i32,
    weights: Vec<PieceWeight>,
    previous_piece: Option<VillagePieceKind>,
    pieces: Vec<VillagePiecePlan>,
    pending_roads: Vec<usize>,
    pending_houses: Vec<usize>,
    start_locator: (i32, i32, i32),
}

impl VillagePlan {
    pub(crate) fn encode_semantics(&self, out: &mut Vec<u8>) {
        out.extend_from_slice(&self.core.encode_core_semantics());
        out.push(match self.style {
            VillageStyle::Plains => 0,
            VillageStyle::Desert => 1,
            VillageStyle::Savanna => 2,
            VillageStyle::Taiga => 3,
        });
        out.push(u8::from(self.abandoned));
        out.extend_from_slice(&(self.pieces.len() as u32).to_le_bytes());
        for piece in &self.pieces {
            out.push(piece.kind as u8);
            out.push(piece.orientation as u8);
            out.extend_from_slice(&piece.gen_depth.to_le_bytes());
            for value in [
                piece.bounds.x0,
                piece.bounds.y0,
                piece.bounds.z0,
                piece.bounds.x1,
                piece.bounds.y1,
                piece.bounds.z1,
                piece.height_position,
            ] {
                out.extend_from_slice(&value.to_le_bytes());
            }
            out.push(u8::from(piece.placed_chest));
            match &piece.extra {
                VillagePieceExtra::None => out.push(0),
                VillagePieceExtra::SimpleHouse { terrace } => {
                    out.push(1);
                    out.push(u8::from(*terrace));
                }
                VillagePieceExtra::SmallHut { low_ceiling, table } => {
                    out.push(2);
                    out.push(u8::from(*low_ceiling));
                    out.push(*table);
                }
                VillagePieceExtra::Farmland { crops } => {
                    out.push(3);
                    out.extend(crops.iter().map(|crop| crop_code(*crop)));
                }
                VillagePieceExtra::DoubleFarmland { crops } => {
                    out.push(4);
                    out.extend(crops.iter().map(|crop| crop_code(*crop)));
                }
                VillagePieceExtra::StraightRoad { length } => {
                    out.push(5);
                    out.extend_from_slice(&length.to_le_bytes());
                }
            }
        }
    }

    pub(crate) fn decode_semantics(cursor: &mut SemanticCursor<'_>) -> Option<Self> {
        let core = StructureStartCore::decode_core_semantics(cursor.take(32)?)?;
        let style = match cursor.u8()? {
            0 => VillageStyle::Plains,
            1 => VillageStyle::Desert,
            2 => VillageStyle::Savanna,
            3 => VillageStyle::Taiga,
            _ => return None,
        };
        let abandoned = cursor.u8()? != 0;
        let piece_count = cursor.u32()? as usize;
        if piece_count > 4096 {
            return None;
        }

        let mut pieces = Vec::with_capacity(piece_count);
        for _ in 0..piece_count {
            let kind = piece_kind(cursor.u8()?)?;
            let orientation = match cursor.u8()? {
                0 => StructureOrientation::South,
                1 => StructureOrientation::West,
                2 => StructureOrientation::North,
                3 => StructureOrientation::East,
                _ => return None,
            };
            let gen_depth = cursor.i32()?;
            let bounds = StructureBounds::new(
                cursor.i32()?,
                cursor.i32()?,
                cursor.i32()?,
                cursor.i32()?,
                cursor.i32()?,
                cursor.i32()?,
            );
            let height_position = cursor.i32()?;
            let placed_chest = cursor.u8()? != 0;
            let extra = match cursor.u8()? {
                0 => VillagePieceExtra::None,
                1 => VillagePieceExtra::SimpleHouse {
                    terrace: cursor.u8()? != 0,
                },
                2 => VillagePieceExtra::SmallHut {
                    low_ceiling: cursor.u8()? != 0,
                    table: cursor.u8()?,
                },
                3 => VillagePieceExtra::Farmland {
                    crops: [crop(cursor.u8()?)?, crop(cursor.u8()?)?],
                },
                4 => VillagePieceExtra::DoubleFarmland {
                    crops: [
                        crop(cursor.u8()?)?,
                        crop(cursor.u8()?)?,
                        crop(cursor.u8()?)?,
                        crop(cursor.u8()?)?,
                    ],
                },
                5 => VillagePieceExtra::StraightRoad {
                    length: cursor.i32()?,
                },
                _ => return None,
            };
            pieces.push(VillagePiecePlan {
                kind,
                bounds,
                orientation,
                gen_depth,
                extra,
                height_position,
                placed_chest,
            });
        }

        Some(Self {
            core,
            style,
            abandoned,
            pieces,
        })
    }

    pub(crate) fn generate(
        source: ChunkCoord,
        mut random: MtRandom,
        candidate_seed: u32,
        style: VillageStyle,
    ) -> Self {
        let mut weights = vec![
            PieceWeight::new(
                VillagePieceKind::SimpleHouse,
                4,
                next_inclusive(&mut random, 2, 4),
            ),
            PieceWeight::new(
                VillagePieceKind::SmallTemple,
                20,
                next_inclusive(&mut random, 0, 1),
            ),
            PieceWeight::new(
                VillagePieceKind::BookHouse,
                20,
                next_inclusive(&mut random, 0, 2),
            ),
            PieceWeight::new(
                VillagePieceKind::SmallHut,
                3,
                next_inclusive(&mut random, 2, 5),
            ),
            PieceWeight::new(
                VillagePieceKind::PigHouse,
                15,
                next_inclusive(&mut random, 0, 2),
            ),
            PieceWeight::new(
                VillagePieceKind::DoubleFarmland,
                3,
                next_inclusive(&mut random, 1, 4),
            ),
            PieceWeight::new(
                VillagePieceKind::Farmland,
                3,
                next_inclusive(&mut random, 2, 4),
            ),
            PieceWeight::new(
                VillagePieceKind::Smithy,
                15,
                next_inclusive(&mut random, 0, 1),
            ),
            PieceWeight::new(
                VillagePieceKind::TwoRoomHouse,
                8,
                next_inclusive(&mut random, 0, 3),
            ),
        ];
        weights.retain(|weight| weight.max_place_count != 0);

        let mut abandoned_random = MtRandom::new(candidate_seed);
        let abandoned = abandoned_random.next_int(50) == 0;

        let orientation = orientation(random.next_int(4));
        let west = source.x().wrapping_mul(16).wrapping_add(2);
        let north = source.z().wrapping_mul(16).wrapping_add(2);
        let start_bounds = StructureBounds::new(
            west,
            64,
            north,
            west.wrapping_add(5),
            78,
            north.wrapping_add(5),
        );
        let start_locator = (
            start_bounds.x0 + start_bounds.x_span() / 2,
            start_bounds.y0 + start_bounds.y_span() / 2,
            start_bounds.z0 + start_bounds.z_span() / 2,
        );

        let mut planner = Planner {
            start_bounds,
            village_size: 0,
            weights,
            previous_piece: None,
            pieces: vec![VillagePiecePlan {
                kind: VillagePieceKind::Start,
                bounds: start_bounds,
                orientation,
                gen_depth: 0,
                extra: VillagePieceExtra::None,
                height_position: -1,
                placed_chest: false,
            }],
            pending_roads: Vec::new(),
            pending_houses: Vec::new(),
            start_locator,
        };

        planner.add_start_children(&mut random);
        while !planner.pending_roads.is_empty() || !planner.pending_houses.is_empty() {
            let piece_index = if planner.pending_roads.is_empty() {
                let position = random.next_int(planner.pending_houses.len() as u32) as usize;
                planner.pending_houses.remove(position)
            } else {
                let position = random.next_int(planner.pending_roads.len() as u32) as usize;
                planner.pending_roads.remove(position)
            };
            planner.add_children(piece_index, &mut random);
        }

        let mut bounds = StructureBounds::unknown();
        for piece in &planner.pieces {
            bounds.expand(piece.bounds);
        }

        Self {
            core: StructureStartCore::new(source, bounds),
            style,
            abandoned,
            pieces: planner.pieces,
        }
    }

    #[cfg(test)]
    pub(crate) fn hash_topology(&self) -> u64 {
        let mut hash = 0xcbf2_9ce4_8422_2325_u64;
        for byte in [self.style as u8, u8::from(self.abandoned)] {
            hash = (hash ^ u64::from(byte)).wrapping_mul(0x100_0000_01b3);
        }
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
                VillagePieceExtra::None => {
                    hash = fnv_byte(hash, 0);
                }
                VillagePieceExtra::SimpleHouse { terrace } => {
                    hash = fnv_byte(hash, 1);
                    hash = fnv_byte(hash, u8::from(*terrace));
                }
                VillagePieceExtra::SmallHut { low_ceiling, table } => {
                    hash = fnv_byte(hash, 2);
                    hash = fnv_byte(hash, u8::from(*low_ceiling));
                    hash = fnv_byte(hash, *table);
                }
                VillagePieceExtra::Farmland { crops } => {
                    hash = fnv_byte(hash, 3);
                    for crop in crops {
                        hash = fnv_byte(hash, *crop as u8);
                    }
                }
                VillagePieceExtra::DoubleFarmland { crops } => {
                    hash = fnv_byte(hash, 4);
                    for crop in crops {
                        hash = fnv_byte(hash, *crop as u8);
                    }
                }
                VillagePieceExtra::StraightRoad { length } => {
                    hash = fnv_byte(hash, 5);
                    for byte in length.to_le_bytes() {
                        hash = fnv_byte(hash, byte);
                    }
                }
            }
        }
        hash
    }
}

impl PieceWeight {
    fn new(kind: VillagePieceKind, weight: i32, max_place_count: i32) -> Self {
        Self {
            kind,
            weight,
            place_count: 0,
            max_place_count,
        }
    }
}

impl Planner {
    fn add_start_children(&mut self, random: &mut MtRandom) {
        let bounds = self.start_bounds;
        let depth = 0;
        self.generate_road(
            random,
            bounds.x0.wrapping_sub(1),
            bounds.y1 - 4,
            bounds.z0 + 1,
            StructureOrientation::West,
            depth,
        );
        self.generate_road(
            random,
            bounds.x1.wrapping_add(1),
            bounds.y1 - 4,
            bounds.z0 + 1,
            StructureOrientation::East,
            depth,
        );
        self.generate_road(
            random,
            bounds.x0 + 1,
            bounds.y1 - 4,
            bounds.z0.wrapping_sub(1),
            StructureOrientation::North,
            depth,
        );
        self.generate_road(
            random,
            bounds.x0 + 1,
            bounds.y1 - 4,
            bounds.z1.wrapping_add(1),
            StructureOrientation::South,
            depth,
        );
    }

    fn add_children(&mut self, index: usize, random: &mut MtRandom) {
        if self.pieces[index].kind == VillagePieceKind::StraightRoad {
            self.add_road_children(index, random);
        }
    }

    fn add_road_children(&mut self, index: usize, random: &mut MtRandom) {
        let road = self.pieces[index].clone();
        let VillagePieceExtra::StraightRoad { length } = road.extra else {
            unreachable!("road piece has road extra");
        };
        let mut has_houses = false;

        let mut offset = random.next_int(5) as i32;
        while offset < length - 8 {
            if let Some(piece) = self.generate_house_left(&road, random, offset) {
                offset += self.pieces[piece]
                    .bounds
                    .x_span()
                    .max(self.pieces[piece].bounds.z_span());
                has_houses = true;
            }
            offset += 2 + random.next_int(5) as i32;
        }

        offset = random.next_int(5) as i32;
        while offset < length - 8 {
            if let Some(piece) = self.generate_house_right(&road, random, offset) {
                offset += self.pieces[piece]
                    .bounds
                    .x_span()
                    .max(self.pieces[piece].bounds.z_span());
                has_houses = true;
            }
            offset += 2 + random.next_int(5) as i32;
        }

        if has_houses && random.next_int(3) > 0 {
            match road.orientation {
                StructureOrientation::North => {
                    self.generate_road(
                        random,
                        road.bounds.x0 - 1,
                        road.bounds.y0,
                        road.bounds.z0,
                        StructureOrientation::West,
                        road.gen_depth,
                    );
                }
                StructureOrientation::South => {
                    self.generate_road(
                        random,
                        road.bounds.x0 - 1,
                        road.bounds.y0,
                        road.bounds.z1 - 2,
                        StructureOrientation::West,
                        road.gen_depth,
                    );
                }
                StructureOrientation::East => {
                    self.generate_road(
                        random,
                        road.bounds.x1 - 2,
                        road.bounds.y0,
                        road.bounds.z0 - 1,
                        StructureOrientation::North,
                        road.gen_depth,
                    );
                }
                StructureOrientation::West => {
                    self.generate_road(
                        random,
                        road.bounds.x0,
                        road.bounds.y0,
                        road.bounds.z0 - 1,
                        StructureOrientation::North,
                        road.gen_depth,
                    );
                }
            }
        }

        if has_houses && random.next_int(3) > 0 {
            match road.orientation {
                StructureOrientation::North => {
                    self.generate_road(
                        random,
                        road.bounds.x1 + 1,
                        road.bounds.y0,
                        road.bounds.z0,
                        StructureOrientation::East,
                        road.gen_depth,
                    );
                }
                StructureOrientation::South => {
                    self.generate_road(
                        random,
                        road.bounds.x1 + 1,
                        road.bounds.y0,
                        road.bounds.z1 - 2,
                        StructureOrientation::East,
                        road.gen_depth,
                    );
                }
                StructureOrientation::East => {
                    self.generate_road(
                        random,
                        road.bounds.x1 - 2,
                        road.bounds.y0,
                        road.bounds.z1 + 1,
                        StructureOrientation::South,
                        road.gen_depth,
                    );
                }
                StructureOrientation::West => {
                    self.generate_road(
                        random,
                        road.bounds.x0,
                        road.bounds.y0,
                        road.bounds.z1 + 1,
                        StructureOrientation::South,
                        road.gen_depth,
                    );
                }
            }
        }
    }

    fn generate_house_left(
        &mut self,
        road: &VillagePiecePlan,
        random: &mut MtRandom,
        offset: i32,
    ) -> Option<usize> {
        match road.orientation {
            StructureOrientation::North | StructureOrientation::South => self.generate_piece(
                random,
                road.bounds.x0 - 1,
                road.bounds.y0,
                road.bounds.z0 + offset,
                StructureOrientation::West,
                road.gen_depth,
            ),
            StructureOrientation::West | StructureOrientation::East => self.generate_piece(
                random,
                road.bounds.x0 + offset,
                road.bounds.y0,
                road.bounds.z0 - 1,
                StructureOrientation::North,
                road.gen_depth,
            ),
        }
    }

    fn generate_house_right(
        &mut self,
        road: &VillagePiecePlan,
        random: &mut MtRandom,
        offset: i32,
    ) -> Option<usize> {
        match road.orientation {
            StructureOrientation::North | StructureOrientation::South => self.generate_piece(
                random,
                road.bounds.x1 + 1,
                road.bounds.y0,
                road.bounds.z0 + offset,
                StructureOrientation::East,
                road.gen_depth,
            ),
            StructureOrientation::West | StructureOrientation::East => self.generate_piece(
                random,
                road.bounds.x0 + offset,
                road.bounds.y0,
                road.bounds.z1 + 1,
                StructureOrientation::South,
                road.gen_depth,
            ),
        }
    }

    fn generate_piece(
        &mut self,
        random: &mut MtRandom,
        foot_x: i32,
        foot_y: i32,
        foot_z: i32,
        orientation: StructureOrientation,
        depth: i32,
    ) -> Option<usize> {
        if depth > MAX_DEPTH
            || foot_x.wrapping_sub(self.start_bounds.x0).wrapping_abs() > MAX_OFFSET_FROM_START
            || foot_z.wrapping_sub(self.start_bounds.z0).wrapping_abs() > MAX_OFFSET_FROM_START
        {
            return None;
        }

        let index = self.generate_piece_from_small_door(
            random,
            foot_x,
            foot_y,
            foot_z,
            orientation,
            depth + 1,
        )?;
        self.pending_houses.push(index);
        Some(index)
    }

    fn generate_piece_from_small_door(
        &mut self,
        random: &mut MtRandom,
        foot_x: i32,
        foot_y: i32,
        foot_z: i32,
        orientation: StructureOrientation,
        depth: i32,
    ) -> Option<usize> {
        let total_weight = self.total_weight();
        if total_weight <= 0 {
            return None;
        }

        for _ in 0..5 {
            let mut selection = random.next_int(total_weight as u32) as i32;
            let mut weight_index = 0;

            while weight_index < self.weights.len() {
                selection -= self.weights[weight_index].weight;
                if selection < 0 {
                    let kind = self.weights[weight_index].kind;
                    if !self.weights[weight_index].do_place(depth)
                        || (self.previous_piece == Some(kind) && self.weights.len() > 1)
                    {
                        break;
                    }

                    if let Some(piece_index) =
                        self.create_leaf(kind, random, foot_x, foot_y, foot_z, orientation, depth)
                    {
                        self.weights[weight_index].place_count += 1;
                        self.previous_piece = Some(kind);
                        if !self.weights[weight_index].is_valid() {
                            self.weights.remove(weight_index);
                        }
                        return Some(piece_index);
                    }
                }
                weight_index += 1;
            }
        }

        self.create_leaf(
            VillagePieceKind::LightPost,
            random,
            foot_x,
            foot_y,
            foot_z,
            orientation,
            depth,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn create_leaf(
        &mut self,
        kind: VillagePieceKind,
        random: &mut MtRandom,
        foot_x: i32,
        foot_y: i32,
        foot_z: i32,
        orientation: StructureOrientation,
        depth: i32,
    ) -> Option<usize> {
        let (width, height, piece_depth) = kind.dimensions()?;
        let bounds = StructureBounds::orient_box(
            foot_x,
            foot_y,
            foot_z,
            0,
            0,
            0,
            width,
            height,
            piece_depth,
            orientation,
        );
        if self.collides(bounds) {
            return None;
        }

        let extra = match kind {
            VillagePieceKind::SimpleHouse => VillagePieceExtra::SimpleHouse {
                terrace: next_boolean(random),
            },
            VillagePieceKind::SmallHut => VillagePieceExtra::SmallHut {
                low_ceiling: next_boolean(random),
                table: random.next_int(3) as u8,
            },
            VillagePieceKind::Farmland => VillagePieceExtra::Farmland {
                crops: [self.select_crop(random), self.select_crop(random)],
            },
            VillagePieceKind::DoubleFarmland => VillagePieceExtra::DoubleFarmland {
                crops: [
                    self.select_crop(random),
                    self.select_crop(random),
                    self.select_crop(random),
                    self.select_crop(random),
                ],
            },
            _ => VillagePieceExtra::None,
        };

        let index = self.pieces.len();
        self.pieces.push(VillagePiecePlan {
            kind,
            bounds,
            orientation,
            gen_depth: depth,
            extra,
            height_position: -1,
            placed_chest: false,
        });
        Some(index)
    }

    fn generate_road(
        &mut self,
        random: &mut MtRandom,
        foot_x: i32,
        foot_y: i32,
        foot_z: i32,
        orientation: StructureOrientation,
        depth: i32,
    ) -> Option<usize> {
        if depth > BASE_ROAD_DEPTH + self.village_size
            || foot_x.wrapping_sub(self.start_bounds.x0).wrapping_abs() > MAX_OFFSET_FROM_START
            || foot_z.wrapping_sub(self.start_bounds.z0).wrapping_abs() > MAX_OFFSET_FROM_START
        {
            return None;
        }

        let mut length = 7 * next_inclusive(random, 3, 5);
        let bounds = loop {
            if length < 7 {
                return None;
            }
            let candidate = StructureBounds::orient_box(
                foot_x,
                foot_y,
                foot_z,
                0,
                0,
                0,
                3,
                3,
                length,
                orientation,
            );
            if !self.collides(candidate) {
                break candidate;
            }
            length -= 7;
        };
        if bounds.y0 <= LOWEST_Y_POSITION {
            return None;
        }

        let actual_length = bounds.x_span().max(bounds.z_span());
        let index = self.pieces.len();
        self.pieces.push(VillagePiecePlan {
            kind: VillagePieceKind::StraightRoad,
            bounds,
            orientation,
            gen_depth: depth,
            extra: VillagePieceExtra::StraightRoad {
                length: actual_length,
            },
            height_position: -1,
            placed_chest: false,
        });
        self.pending_roads.push(index);
        Some(index)
    }

    fn collides(&self, bounds: StructureBounds) -> bool {
        self.pieces
            .iter()
            .any(|piece| piece.bounds.intersects(bounds))
    }

    fn total_weight(&self) -> i32 {
        if !self.weights.iter().any(PieceWeight::is_valid) {
            return -1;
        }
        self.weights.iter().map(|weight| weight.weight).sum()
    }

    fn select_crop(&self, random: &mut MtRandom) -> VillageCrop {
        if random.next_int(3) != 1 {
            return VillageCrop::Wheat;
        }

        let (x, y, z) = self.start_locator;
        let raw = x
            .wrapping_mul(8_976_890)
            .wrapping_add(y.wrapping_mul(981_131))
            .wrapping_add(z);
        let hash = raw as i64 as u64;
        match (hash >> 3) % 3 {
            0 => VillageCrop::Potato,
            1 => VillageCrop::Beetroot,
            _ => VillageCrop::Carrot,
        }
    }
}

pub(crate) struct SemanticCursor<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> SemanticCursor<'a> {
    pub(crate) const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    pub(crate) const fn is_finished(&self) -> bool {
        self.offset == self.bytes.len()
    }

    fn take(&mut self, count: usize) -> Option<&'a [u8]> {
        let end = self.offset.checked_add(count)?;
        let value = self.bytes.get(self.offset..end)?;
        self.offset = end;
        Some(value)
    }

    fn u8(&mut self) -> Option<u8> {
        Some(*self.take(1)?.first()?)
    }

    fn u32(&mut self) -> Option<u32> {
        Some(u32::from_le_bytes(self.take(4)?.try_into().ok()?))
    }

    fn i32(&mut self) -> Option<i32> {
        Some(i32::from_le_bytes(self.take(4)?.try_into().ok()?))
    }
}

fn piece_kind(value: u8) -> Option<VillagePieceKind> {
    Some(match value {
        0 => VillagePieceKind::Start,
        1 => VillagePieceKind::SimpleHouse,
        2 => VillagePieceKind::SmallTemple,
        3 => VillagePieceKind::BookHouse,
        4 => VillagePieceKind::SmallHut,
        5 => VillagePieceKind::PigHouse,
        6 => VillagePieceKind::DoubleFarmland,
        7 => VillagePieceKind::Farmland,
        8 => VillagePieceKind::Smithy,
        9 => VillagePieceKind::TwoRoomHouse,
        10 => VillagePieceKind::LightPost,
        11 => VillagePieceKind::StraightRoad,
        _ => return None,
    })
}

const fn crop_code(value: VillageCrop) -> u8 {
    match value {
        VillageCrop::Wheat => 0,
        VillageCrop::Potato => 1,
        VillageCrop::Beetroot => 2,
        VillageCrop::Carrot => 3,
    }
}

fn crop(value: u8) -> Option<VillageCrop> {
    Some(match value {
        0 => VillageCrop::Wheat,
        1 => VillageCrop::Potato,
        2 => VillageCrop::Beetroot,
        3 => VillageCrop::Carrot,
        _ => return None,
    })
}

fn orientation(value: u32) -> StructureOrientation {
    match value {
        0 => StructureOrientation::South,
        1 => StructureOrientation::West,
        2 => StructureOrientation::North,
        3 => StructureOrientation::East,
        _ => unreachable!("orientation draw is bounded to four values"),
    }
}

fn next_inclusive(random: &mut MtRandom, min: i32, max: i32) -> i32 {
    if min >= max {
        min
    } else {
        min + random.next_int((max - min + 1) as u32) as i32
    }
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
    fn planner_preserves_piece_dimensions_and_constructor_rng() {
        let random = MtRandom::new(0x1234_5678);
        let plan = VillagePlan::generate(
            ChunkCoord::new(10, -12),
            random,
            0x1234_5678,
            VillageStyle::Plains,
        );
        assert!(!plan.pieces.is_empty());
        assert_eq!(plan.pieces[0].kind, VillagePieceKind::Start);
        assert!(
            plan.pieces
                .iter()
                .any(|piece| piece.kind == VillagePieceKind::StraightRoad)
        );

        // Ensure every non-road piece keeps the exact target dimensions before terrain-height
        // post-processing moves it vertically.
        for piece in &plan.pieces {
            if let Some((width, height, depth)) = piece.kind.dimensions() {
                let spans = (
                    piece.bounds.x_span(),
                    piece.bounds.y_span(),
                    piece.bounds.z_span(),
                );
                match piece.orientation {
                    StructureOrientation::South | StructureOrientation::North => {
                        assert_eq!(spans, (width, height, depth));
                    }
                    StructureOrientation::West | StructureOrientation::East => {
                        assert_eq!(spans, (depth, height, width));
                    }
                }
            }
        }
    }
}
