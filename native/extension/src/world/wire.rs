use std::collections::HashMap;

use cobblestone_wire::{
    ChunkWireView, NamedNbt, NbtDocument, NbtTag, NbtValue, RawPacket, encode_full_chunk,
};
use cobblestone_world::{CHUNK_NIBBLE_BYTES, ChestBlockEntity, ChunkCoord};
use ext_php_rs::exception::PhpResult;

use crate::boundary::php_error;

use super::resolve_world_state;

const MAX_CHUNK_WIRE_CACHE_ENTRIES: usize = 4096;

struct CachedChunkWire {
    terrain_revision: u64,
    light_revision: u64,
    packet: RawPacket,
}

#[derive(Default)]
pub(super) struct ChunkWireCache {
    entries: HashMap<ChunkCoord, CachedChunkWire>,
}

impl ChunkWireCache {
    pub(super) fn remove(&mut self, position: &ChunkCoord) {
        self.entries.remove(position);
    }
}

fn chest_nbt(chest: &ChestBlockEntity) -> NbtDocument {
    let items = chest
        .items
        .iter()
        .map(|item| {
            NbtValue::Compound(vec![
                NamedNbt::new("id", NbtValue::Short(item.item_id)),
                NamedNbt::new("Count", NbtValue::Byte(item.count as i8)),
                NamedNbt::new("Damage", NbtValue::Short(item.damage)),
                NamedNbt::new("Slot", NbtValue::Byte(item.slot as i8)),
            ])
        })
        .collect();
    NbtDocument::new(NamedNbt::new(
        "",
        NbtValue::Compound(vec![
            NamedNbt::new("id", NbtValue::String("Chest".to_owned())),
            NamedNbt::new("x", NbtValue::Int(chest.x)),
            NamedNbt::new("y", NbtValue::Int(chest.y)),
            NamedNbt::new("z", NbtValue::Int(chest.z)),
            NamedNbt::new(
                "Items",
                NbtValue::List {
                    element_type: NbtTag::Compound,
                    values: items,
                },
            ),
        ]),
    ))
}

pub(crate) fn chunk_wire_packet(handle_value: i64, position: ChunkCoord) -> PhpResult<RawPacket> {
    let state = resolve_world_state(handle_value)?;
    let snapshot = state
        .store
        .snapshot(position)
        .map_err(|error| php_error(error.to_string()))?;

    {
        let cache = match state.chunk_wire_cache.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        if let Some(cached) = cache.entries.get(&position)
            && cached.terrain_revision == snapshot.terrain_revision()
            && cached.light_revision == snapshot.light_revision()
        {
            return Ok(cached.packet.clone());
        }
    }

    let mut block_ids = Vec::with_capacity(snapshot.states().len());
    let mut block_data = vec![0_u8; CHUNK_NIBBLE_BYTES];
    for (index, &state_id) in snapshot.states().iter().enumerate() {
        block_ids.push((state_id >> 4) as u8);
        let data = (state_id & 0x0f) as u8;
        let byte = &mut block_data[index >> 1];
        if index & 1 == 0 {
            *byte = (*byte & 0xf0) | data;
        } else {
            *byte = (*byte & 0x0f) | (data << 4);
        }
    }

    let extra_data = snapshot
        .extra_data()
        .iter()
        .map(|(&key, &value)| (u32::from(key), value))
        .collect::<Vec<_>>();
    let block_entities = snapshot
        .chest_block_entities()
        .iter()
        .map(chest_nbt)
        .collect::<Vec<_>>();
    let packet = encode_full_chunk(ChunkWireView {
        chunk_x: position.x(),
        chunk_z: position.z(),
        block_ids: &block_ids,
        block_data: &block_data,
        sky_light: snapshot.sky_light(),
        block_light: snapshot.block_light(),
        biome_words: snapshot.biomes(),
        height_map: snapshot.height_map(),
        extra_data: &extra_data,
        block_entities: &block_entities,
    })
    .map_err(|error| php_error(error.to_string()))?;

    let current_terrain = state
        .store
        .terrain_revision(position)
        .map_err(|error| php_error(error.to_string()))?;
    let current_light = state
        .store
        .light_revision(position)
        .map_err(|error| php_error(error.to_string()))?;
    if current_terrain == snapshot.terrain_revision() && current_light == snapshot.light_revision()
    {
        let mut cache = match state.chunk_wire_cache.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        if cache.entries.len() >= MAX_CHUNK_WIRE_CACHE_ENTRIES
            && !cache.entries.contains_key(&position)
        {
            cache.entries.clear();
        }
        cache.entries.insert(
            position,
            CachedChunkWire {
                terrain_revision: snapshot.terrain_revision(),
                light_revision: snapshot.light_revision(),
                packet: packet.clone(),
            },
        );
    }

    Ok(packet)
}

#[cfg(test)]
mod tests {
    use super::*;
    use cobblestone_world::ChestItemStack;

    #[test]
    fn target_01510_smithy_chest_nbt_matches_saved_block_entity_shape() {
        // ChestBlockEntity::save (x86 0xf48b90) writes base id/x/y/z, then an
        // Items list; ItemInstance::save writes id/Count/Damage and the chest
        // adds Slot to each non-empty inventory entry.
        let chest = ChestBlockEntity {
            x: 5,
            y: 67,
            z: 9,
            items: vec![ChestItemStack {
                slot: 7,
                item_id: 265,
                damage: 0,
                count: 3,
            }],
        };

        assert_eq!(
            chest_nbt(&chest),
            NbtDocument::new(NamedNbt::new(
                "",
                NbtValue::Compound(vec![
                    NamedNbt::new("id", NbtValue::String("Chest".to_owned())),
                    NamedNbt::new("x", NbtValue::Int(5)),
                    NamedNbt::new("y", NbtValue::Int(67)),
                    NamedNbt::new("z", NbtValue::Int(9)),
                    NamedNbt::new(
                        "Items",
                        NbtValue::List {
                            element_type: NbtTag::Compound,
                            values: vec![NbtValue::Compound(vec![
                                NamedNbt::new("id", NbtValue::Short(265)),
                                NamedNbt::new("Count", NbtValue::Byte(3)),
                                NamedNbt::new("Damage", NbtValue::Short(0)),
                                NamedNbt::new("Slot", NbtValue::Byte(7)),
                            ])],
                        },
                    ),
                ]),
            ))
        );
    }
}
