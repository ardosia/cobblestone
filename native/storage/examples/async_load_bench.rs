use std::collections::HashMap;
use std::fs;
use std::time::{Duration, Instant};

use cobblestone_storage::{
    AsyncLoadConfig, AsyncLoadService, CompressionPolicy, LoadCompletion, RegionCoord, RegionFile,
};
use cobblestone_world::{
    CHUNK_LIFECYCLE_GENERATED, CHUNK_LIFECYCLE_LIGHT_POPULATED, CHUNK_LIFECYCLE_POPULATED,
    ChunkCoord, WorldStore,
};

const CHUNKS: usize = 64;

fn snapshots() -> Vec<cobblestone_world::ChunkSnapshot> {
    let store = WorldStore::new();
    let flags =
        CHUNK_LIFECYCLE_GENERATED | CHUNK_LIFECYCLE_POPULATED | CHUNK_LIFECYCLE_LIGHT_POPULATED;
    let mut snapshots = Vec::with_capacity(CHUNKS);

    for i in 0..CHUNKS {
        let region = i % 16;
        let local = i / 16;
        let position = ChunkCoord::new(
            ((region % 4) * 16 + local) as i32,
            ((region / 4) * 16) as i32,
        );
        store.ensure_chunk(position, 1);
        store.fill_layers(position, 0, 1, 7 << 4).unwrap();
        store.fill_layers(position, 1, 2, 3 << 4).unwrap();
        store.fill_layers(position, 3, 1, 2 << 4).unwrap();
        store.fill_sky_light_from(position, 4, 15).unwrap();
        store.set_lifecycle_flags(position, flags).unwrap();
        snapshots.push(store.snapshot(position).unwrap());
    }

    snapshots
}

fn region_path(root: &std::path::Path, region: RegionCoord) -> std::path::PathBuf {
    root.join("regions")
        .join(format!("r.{}.{}.cwr", region.x, region.z))
}

fn populate(
    root: &std::path::Path,
    world_uuid: [u8; 16],
    snapshots: &[cobblestone_world::ChunkSnapshot],
) {
    let mut regions = HashMap::<RegionCoord, RegionFile>::new();
    for snapshot in snapshots {
        let region = RegionCoord::for_chunk(snapshot.position());
        let file = match regions.entry(region) {
            std::collections::hash_map::Entry::Occupied(entry) => entry.into_mut(),
            std::collections::hash_map::Entry::Vacant(entry) => entry.insert(
                RegionFile::open_or_create(region_path(root, region), world_uuid, region).unwrap(),
            ),
        };
        file.save_chunk(snapshot, CompressionPolicy::Adaptive)
            .unwrap();
    }
}

fn main() {
    let snapshots = snapshots();
    let world_uuid = [0x84; 16];

    for workers in [1, 2, 4] {
        let root = std::env::temp_dir().join(format!(
            "cobblestone-async-load-bench-{}-{workers}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        populate(&root, world_uuid, &snapshots);

        let mut config = AsyncLoadConfig::new(&root, world_uuid);
        config.workers = workers;
        config.queue_capacity = CHUNKS * 2;
        config.completion_capacity = CHUNKS * 2;

        let mut service = AsyncLoadService::start(config).unwrap();
        let start = Instant::now();

        for snapshot in &snapshots {
            service.request(snapshot.position()).unwrap();
        }

        let mut completed = 0;
        while completed < CHUNKS {
            let completion = service
                .recv_completion_timeout(Duration::from_secs(30))
                .unwrap()
                .expect("load worker stopped before all completions");
            match completion {
                LoadCompletion::Loaded(_) => completed += 1,
                LoadCompletion::Missing(position) => panic!("missing chunk {position:?}"),
                LoadCompletion::Failed(failure) => {
                    panic!("async load failed: {}", failure.error);
                }
            }
        }

        let elapsed = start.elapsed();
        assert!(service.shutdown().is_empty());

        let seconds = elapsed.as_secs_f64();
        println!(
            "async_load_bench workers={workers} chunks={CHUNKS} total_ms={:.3} us_per_chunk={:.3} chunks_per_sec={:.1}",
            seconds * 1000.0,
            seconds * 1_000_000.0 / CHUNKS as f64,
            CHUNKS as f64 / seconds,
        );

        fs::remove_dir_all(root).unwrap();
    }
}
