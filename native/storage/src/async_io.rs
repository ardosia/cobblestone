use std::path::{Path, PathBuf};

use cobblestone_world::ChunkCoord;

use crate::RegionCoord;

pub(super) fn route_chunk_worker(position: ChunkCoord, workers: usize) -> usize {
    route_region_worker(RegionCoord::for_chunk(position), workers)
}

pub(super) fn route_region_worker(region: RegionCoord, workers: usize) -> usize {
    let x = region.x as i64 as u64;
    let z = region.z as i64 as u64;
    let mixed = x.wrapping_mul(0x9e37_79b9_7f4a_7c15).rotate_left(17)
        ^ z.wrapping_mul(0xc2b2_ae3d_27d4_eb4f);
    mixed as usize % workers
}

pub(super) fn region_path(root: &Path, region: RegionCoord) -> PathBuf {
    root.join("regions")
        .join(format!("r.{}.{}.cwr", region.x, region.z))
}
