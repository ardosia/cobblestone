use crate::population::{PopulationNeighborhood, population_random, state};
use crate::terrain_shape::noise::MtRandom;

const AIR: u16 = 0;
const COBBLESTONE: u16 = 4;
const MOSSY_COBBLESTONE: u16 = 48;
const MOB_SPAWNER: u16 = 52;
const CHEST: u16 = 54;

const NORTH: u8 = 2;
const SOUTH: u8 = 3;
const WEST: u8 = 4;
const EAST: u8 = 5;

const ATTEMPTS: usize = 8;
const ROOM_HEIGHT: i32 = 3;

/// Exact fixed-target MonsterRoomFeature population stage.
///
/// The target runs eight attempts after structure post-processing on the same population RNG
/// stream. The public method is the isolated/reseeded entrypoint used by focused tests; the final
/// Infinite composer must use the crate-internal shared-random method after the structure families.
pub struct OverworldMonsterRoomPopulator {
    seed: u32,
}

impl OverworldMonsterRoomPopulator {
    pub fn new(seed: i32) -> Self {
        Self {
            seed: u32::from_ne_bytes(seed.to_ne_bytes()),
        }
    }

    pub fn populate(&self, neighborhood: &mut PopulationNeighborhood) {
        let mut random = population_random(self.seed, neighborhood.center());
        self.populate_with_random(neighborhood, &mut random);
    }

    pub(crate) fn populate_with_random(
        &self,
        neighborhood: &mut PopulationNeighborhood,
        random: &mut MtRandom,
    ) {
        let center = neighborhood.center();
        let origin_x = center.x().wrapping_mul(16);
        let origin_z = center.z().wrapping_mul(16);

        for _ in 0..ATTEMPTS {
            let pos = BlockPos {
                x: origin_x
                    .wrapping_add(random.next_int(16) as i32)
                    .wrapping_add(8),
                y: random.next_int(128) as i32,
                z: origin_z
                    .wrapping_add(random.next_int(16) as i32)
                    .wrapping_add(8),
            };
            let _ = place_monster_room(neighborhood, pos, random);
        }
    }
}

fn place_monster_room(
    neighborhood: &mut PopulationNeighborhood,
    pos: BlockPos,
    random: &mut MtRandom,
) -> bool {
    let x_radius = random.next_int(2) as i32 + 2;
    let z_radius = random.next_int(2) as i32 + 2;
    let min_dx = -x_radius - 1;
    let max_dx = x_radius + 1;
    let min_dz = -z_radius - 1;
    let max_dz = z_radius + 1;

    let mut hole_count = 0;
    for dx in min_dx..=max_dx {
        let x = pos.x.wrapping_add(dx);
        for dy in -1..=ROOM_HEIGHT + 1 {
            let y = pos.y.wrapping_add(dy);
            for dz in min_dz..=max_dz {
                let z = pos.z.wrapping_add(dz);
                if dy == -1 && !material_is_solid(neighborhood.block_id(x, y, z)) {
                    return false;
                }
                if dy == ROOM_HEIGHT + 1 && !material_is_solid(neighborhood.block_id(x, y, z)) {
                    return false;
                }
                if (dx == min_dx || dx == max_dx || dz == min_dz || dz == max_dz)
                    && dy == 0
                    && neighborhood.block_id(x, y, z) == AIR
                    && neighborhood.block_id(x, y.wrapping_add(1), z) == AIR
                {
                    hole_count += 1;
                }
            }
        }
    }

    if !(1..=5).contains(&hole_count) {
        return false;
    }

    for dx in min_dx..=max_dx {
        let x = pos.x.wrapping_add(dx);
        for dy in (-1..=ROOM_HEIGHT).rev() {
            let y = pos.y.wrapping_add(dy);
            for dz in min_dz..=max_dz {
                let z = pos.z.wrapping_add(dz);
                let boundary = dx == min_dx
                    || dy == -1
                    || dz == min_dz
                    || dx == max_dx
                    // Source-shape parity: unreachable because placement stops at +3 while the
                    // validated ceiling is +4.
                    || dy == ROOM_HEIGHT + 1
                    || dz == max_dz;

                if boundary {
                    if y >= 0 && !material_is_solid(neighborhood.block_id(x, y.wrapping_sub(1), z))
                    {
                        let _ = neighborhood.set_state(x, y, z, state(AIR, 0));
                    } else if material_is_solid(neighborhood.block_id(x, y, z)) {
                        let block = if dy == -1 && random.next_int(4) != 0 {
                            MOSSY_COBBLESTONE
                        } else {
                            COBBLESTONE
                        };
                        let _ = neighborhood.set_state(x, y, z, state(block, 0));
                    }
                } else {
                    let _ = neighborhood.set_state(x, y, z, state(AIR, 0));
                }
            }
        }
    }

    for _ in 0..2 {
        for _ in 0..3 {
            let offset_z = random.next_int((z_radius * 2 + 1) as u32) as i32 - z_radius;
            let offset_x = random.next_int((x_radius * 2 + 1) as u32) as i32 - x_radius;
            let chest = BlockPos {
                x: pos.x.wrapping_add(offset_x),
                y: pos.y,
                z: pos.z.wrapping_add(offset_z),
            };

            if neighborhood.block_id(chest.x, chest.y, chest.z) != AIR {
                continue;
            }

            let solid_neighbors = [
                (chest.x.wrapping_sub(1), chest.z),
                (chest.x.wrapping_add(1), chest.z),
                (chest.x, chest.z.wrapping_sub(1)),
                (chest.x, chest.z.wrapping_add(1)),
            ]
            .into_iter()
            .filter(|(x, z)| material_is_solid(neighborhood.block_id(*x, chest.y, *z)))
            .count();

            if solid_neighbors != 1 {
                continue;
            }

            let facing = chest_facing(neighborhood, chest, offset_x, offset_z);
            let _ = neighborhood.set_state(chest.x, chest.y, chest.z, state(CHEST, facing));

            // The target's ChestBlockEntity loot-table fill is commented out in this branch.
            break;
        }
    }

    let _ = neighborhood.set_state(pos.x, pos.y, pos.z, state(MOB_SPAWNER, 0));

    // The target places the generic spawner block, but its MobSpawnerBlockEntity entity-id
    // assignment (and therefore its nextInt(4) draw) is commented out.
    true
}

fn chest_facing(
    neighborhood: &PopulationNeighborhood,
    pos: BlockPos,
    offset_x: i32,
    offset_z: i32,
) -> u8 {
    if offset_z > 0 && neighborhood.block_id(pos.x, pos.y, pos.z.wrapping_add(1)) != AIR {
        return NORTH;
    }
    if offset_z < 0 && neighborhood.block_id(pos.x, pos.y, pos.z.wrapping_sub(1)) != AIR {
        return SOUTH;
    }
    if offset_x > 0 && neighborhood.block_id(pos.x.wrapping_add(1), pos.y, pos.z) != AIR {
        return WEST;
    }
    if offset_x < 0 && neighborhood.block_id(pos.x.wrapping_sub(1), pos.y, pos.z) != AIR {
        return EAST;
    }

    if offset_z > 0 {
        NORTH
    } else if offset_z < 0 {
        SOUTH
    } else if offset_x > 0 {
        WEST
    } else if offset_x < 0 {
        EAST
    } else {
        NORTH
    }
}

/// Material::isSolid for the IDs that can exist before the fixed-target dungeon stage.
///
/// Infinite population reaches MonsterRoomFeature before freeze/frost and biome decoration, so its
/// input domain is base/surface/cave/lake output plus the four target structure families.
const fn material_is_solid(id: u16) -> bool {
    !matches!(
        id,
        AIR | 8
            | 9
            | 10
            | 11
            | 30
            | 50
            | 55
            | 59
            | 65
            | 66
            | 69
            | 93
            | 106
            | 119
            | 131
            | 132
            | 140
            | 141
            | 142
            | 143
            | 244
    )
}

#[derive(Debug, Copy, Clone)]
struct BlockPos {
    x: i32,
    y: i32,
    z: i32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ChunkCoord;

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

    fn changed_neighbor_states(neighborhood: &PopulationNeighborhood) -> usize {
        (-1..=1)
            .flat_map(|offset_z| (-1..=1).map(move |offset_x| (offset_x, offset_z)))
            .filter(|(offset_x, offset_z)| *offset_x != 0 || *offset_z != 0)
            .flat_map(|(offset_x, offset_z)| {
                neighborhood
                    .chunk_states(offset_x, offset_z)
                    .expect("population neighbor exists")
                    .iter()
            })
            .filter(|value| **value != state(1, 0))
            .count()
    }

    fn carve_west_door(neighborhood: &mut PopulationNeighborhood, pos: BlockPos, x_radius: i32) {
        let x = pos.x.wrapping_sub(x_radius + 1);
        let _ = neighborhood.set_state(x, pos.y, pos.z, state(AIR, 0));
        let _ = neighborhood.set_state(x, pos.y.wrapping_add(1), pos.z, state(AIR, 0));
    }

    #[test]
    fn independent_direct_cross_chunk_room_fixture_matches() {
        // Standalone C++ std::mt19937/block-array oracle implementing the restored target
        // MonsterRoomFeature recipe. The fixture begins as all stone with one two-block doorway.
        let center = ChunkCoord::new(0, 0);
        let pos = BlockPos {
            x: 15,
            y: 40,
            z: 15,
        };
        let mut neighborhood = PopulationNeighborhood::filled(center, state(1, 0), 1);
        carve_west_door(&mut neighborhood, pos, 3);

        let mut random = MtRandom::new(0x1234_5678);
        assert!(place_monster_room(&mut neighborhood, pos, &mut random));

        assert_eq!(hash_neighborhood(&neighborhood), 0x08f7_a006_e781_72d2);
        assert_eq!(block_count(&neighborhood, COBBLESTONE), 123);
        assert_eq!(block_count(&neighborhood, MOSSY_COBBLESTONE), 49);
        assert_eq!(block_count(&neighborhood, CHEST), 1);
        assert_eq!(block_count(&neighborhood, MOB_SPAWNER), 1);
        assert_eq!(changed_neighbor_states(&neighborhood), 215);
        assert_eq!(
            neighborhood.state(16, 40, 17),
            Some(state(CHEST, NORTH)),
            "target chest facing metadata changed",
        );
    }

    #[test]
    fn independent_population_fixtures_match_signed_seed_and_negative_coordinates() {
        // Standalone C++ oracle drives the target population reseed, all eight attempts, room
        // validation/geometry, chest facing, and commented-out spawner-entity RNG behavior.
        let fixtures = [
            (
                -1,
                ChunkCoord::new(-1, -1),
                BlockPos { x: 4, y: 11, z: -5 },
                2,
                0x77ac_d863_5b99_db16_u64,
                106,
                36,
                2,
                245,
            ),
            (
                i32::MIN,
                ChunkCoord::new(7, -9),
                BlockPos {
                    x: 129,
                    y: 8,
                    z: -135,
                },
                3,
                0xa7ba_4f35_cb19_96bf_u64,
                141,
                65,
                2,
                270,
            ),
        ];

        for (
            seed,
            center,
            first_pos,
            first_x_radius,
            expected_hash,
            cobble,
            mossy,
            chests,
            neighbor_writes,
        ) in fixtures
        {
            let mut neighborhood = PopulationNeighborhood::filled(center, state(1, 0), 1);
            carve_west_door(&mut neighborhood, first_pos, first_x_radius);

            OverworldMonsterRoomPopulator::new(seed).populate(&mut neighborhood);

            assert_eq!(
                hash_neighborhood(&neighborhood),
                expected_hash,
                "seed={seed} center={center:?}",
            );
            assert_eq!(block_count(&neighborhood, COBBLESTONE), cobble);
            assert_eq!(block_count(&neighborhood, MOSSY_COBBLESTONE), mossy);
            assert_eq!(block_count(&neighborhood, CHEST), chests);
            assert_eq!(block_count(&neighborhood, MOB_SPAWNER), 1);
            assert_eq!(
                changed_neighbor_states(&neighborhood),
                neighbor_writes,
                "cross-chunk writes seed={seed} center={center:?}",
            );
        }
    }

    #[test]
    fn invalid_cavity_does_not_mutate_the_neighborhood() {
        let center = ChunkCoord::new(0, 0);
        let mut neighborhood = PopulationNeighborhood::filled(center, state(1, 0), 1);
        let before = neighborhood.clone();
        let mut random = MtRandom::new(0);

        assert!(!place_monster_room(
            &mut neighborhood,
            BlockPos {
                x: 12,
                y: 32,
                z: 12,
            },
            &mut random,
        ));
        assert_eq!(neighborhood, before);
    }

    #[test]
    fn target_generated_material_solidity_matches_dungeon_inputs() {
        for id in [
            0, 8, 9, 10, 11, 30, 50, 55, 59, 65, 66, 69, 93, 106, 119, 131, 132, 140, 141, 142,
            143, 244,
        ] {
            assert!(!material_is_solid(id), "id={id}");
        }
        for id in [
            1, 2, 3, 4, 5, 17, 23, 24, 29, 43, 44, 46, 47, 48, 52, 54, 58, 60, 61, 64, 67, 70, 71,
            85, 97, 98, 101, 102, 109, 120, 128, 134, 159, 162, 163, 172, 179, 193, 196, 198, 243,
        ] {
            assert!(material_is_solid(id), "id={id}");
        }
    }
}
