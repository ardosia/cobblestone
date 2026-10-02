use std::collections::HashMap;

use cobblestone_core::{CHUNK_NIBBLE_BYTES, ChunkCoord};
use cobblestone_protocol84::{
    Protocol84ChunkSnapshot, RawPacket, encode_protocol84_full_chunk_data,
};
use ext_php_rs::exception::PhpResult;

use crate::boundary::php_error;

use super::resolve_world_state;

const MAX_PROTOCOL84_CACHE_ENTRIES: usize = 4096;

struct CachedProtocol84Chunk {
    terrain_revision: u64,
    light_revision: u64,
    packet: RawPacket,
}

#[derive(Default)]
pub(super) struct Protocol84Cache {
    entries: HashMap<ChunkCoord, CachedProtocol84Chunk>,
}

impl Protocol84Cache {
    pub(super) fn remove(&mut self, position: &ChunkCoord) {
        self.entries.remove(position);
    }
}

pub(crate) fn protocol84_chunk(handle_value: i64, position: ChunkCoord) -> PhpResult<RawPacket> {
    let state = resolve_world_state(handle_value)?;
    let snapshot = state
        .store
        .snapshot(position)
        .map_err(|error| php_error(error.to_string()))?;

    {
        let cache = match state.protocol84_cache.lock() {
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
    let packet = encode_protocol84_full_chunk_data(Protocol84ChunkSnapshot {
        chunk_x: position.x(),
        chunk_z: position.z(),
        block_ids: &block_ids,
        block_data: &block_data,
        sky_light: snapshot.sky_light(),
        block_light: snapshot.block_light(),
        biomes: snapshot.biomes(),
        height_map: snapshot.height_map(),
        extra_data: &extra_data,
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
        let mut cache = match state.protocol84_cache.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        if cache.entries.len() >= MAX_PROTOCOL84_CACHE_ENTRIES
            && !cache.entries.contains_key(&position)
        {
            cache.entries.clear();
        }
        cache.entries.insert(
            position,
            CachedProtocol84Chunk {
                terrain_revision: snapshot.terrain_revision(),
                light_revision: snapshot.light_revision(),
                packet: packet.clone(),
            },
        );
    }

    Ok(packet)
}
