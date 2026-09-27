use std::fs;
use std::time::{Duration, Instant};

use cobblestone_core::{
    CHUNK_LIFECYCLE_GENERATED, CHUNK_LIFECYCLE_LIGHT_POPULATED, CHUNK_LIFECYCLE_POPULATED,
    ChunkCoord, WorldStore,
};
use cobblestone_storage::{AsyncSaveConfig, AsyncSaveService, SaveCompletion};

const CHUNKS: usize = 64;

fn snapshots() -> Vec<cobblestone_core::ChunkSnapshot> {
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

fn main() {
    let snapshots = snapshots();

    for workers in [1, 2, 4] {
        let root = std::env::temp_dir().join(format!(
            "cobblestone-async-save-bench-{}-{workers}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);

        let mut config = AsyncSaveConfig::new(&root, [0x84; 16]);
        config.workers = workers;
        config.queue_capacity = CHUNKS * 2;
        config.completion_capacity = CHUNKS * 2;

        let mut service = AsyncSaveService::start(config).unwrap();
        let start = Instant::now();

        for snapshot in &snapshots {
            service.try_save(snapshot.clone()).unwrap();
        }

        let mut completed = 0;
        let mut bytes = 0_u64;
        while completed < CHUNKS {
            let completion = service
                .recv_completion_timeout(Duration::from_secs(30))
                .unwrap()
                .expect("save worker stopped before all completions");
            match completion {
                SaveCompletion::Saved(receipt) => {
                    completed += 1;
                    bytes += receipt.bytes_appended;
                }
                SaveCompletion::Failed(failure) => {
                    panic!("async save failed: {}", failure.error);
                }
            }
        }

        let elapsed = start.elapsed();
        assert!(service.shutdown().is_empty());

        let seconds = elapsed.as_secs_f64();
        println!(
            "async_save_bench workers={workers} chunks={CHUNKS} total_ms={:.3} us_per_chunk={:.3} chunks_per_sec={:.1} appended_bytes={bytes}",
            seconds * 1000.0,
            seconds * 1_000_000.0 / CHUNKS as f64,
            CHUNKS as f64 / seconds,
        );

        fs::remove_dir_all(root).unwrap();
    }
}
