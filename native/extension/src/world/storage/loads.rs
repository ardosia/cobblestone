use cobblestone_storage::LoadCompletion;
use cobblestone_world::{ChunkCoord, WorldStore};
use ext_php_rs::exception::PhpResult;

use crate::boundary::php_error;

use super::NativeWorldPersistence;

pub(super) fn poll_load_completions(
    store: &WorldStore,
    persistence: &mut NativeWorldPersistence,
    budget: usize,
) -> PhpResult<usize> {
    let mut completed = 0;
    while completed < budget {
        let Some(completion) = persistence.loads.try_recv_completion() else {
            break;
        };

        match completion {
            LoadCompletion::Loaded(chunk) => {
                persistence.load_missing.remove(&chunk.position);
                store
                    .import_chunk_if_absent(chunk.position, chunk.import)
                    .map_err(|error| php_error(error.to_string()))?;
            }
            LoadCompletion::Missing(position) => {
                persistence.load_missing.insert(position);
            }
            LoadCompletion::Failed(failure) => {
                return Err(php_error(format!(
                    "native world load failed for {}:{}: {}",
                    failure.position.x(),
                    failure.position.z(),
                    failure.error
                )));
            }
        }
        completed += 1;
    }

    Ok(completed)
}

pub(super) fn decode_chunk_positions(input: &[u8]) -> PhpResult<Vec<ChunkCoord>> {
    const HEADER_BYTES: usize = 4;
    const ENTRY_BYTES: usize = 8;
    const MAX_POSITIONS: usize = 4096;

    if input.len() < HEADER_BYTES {
        return Err(php_error("chunk position batch is truncated"));
    }
    let count = u32::from_le_bytes(
        input[0..4]
            .try_into()
            .expect("fixed four-byte chunk batch header"),
    ) as usize;
    if count > MAX_POSITIONS {
        return Err(php_error("chunk position batch exceeds 4096 entries"));
    }
    let expected = HEADER_BYTES
        .checked_add(
            count
                .checked_mul(ENTRY_BYTES)
                .ok_or_else(|| php_error("chunk position batch length overflow"))?,
        )
        .ok_or_else(|| php_error("chunk position batch length overflow"))?;
    if input.len() != expected {
        return Err(php_error("chunk position batch length mismatch"));
    }

    let mut positions = Vec::with_capacity(count);
    let mut offset = HEADER_BYTES;
    for _ in 0..count {
        let x = i32::from_le_bytes(
            input[offset..offset + 4]
                .try_into()
                .expect("fixed chunk x slice"),
        );
        let z = i32::from_le_bytes(
            input[offset + 4..offset + 8]
                .try_into()
                .expect("fixed chunk z slice"),
        );
        positions.push(ChunkCoord::new(x, z));
        offset += ENTRY_BYTES;
    }

    Ok(positions)
}
