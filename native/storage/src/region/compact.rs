use std::fs::{self, File, OpenOptions};
use std::io::{self, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use crate::StorageError;

use super::format::{
    IndexEntry, RECORD_AREA_OFFSET, RegionIndex, encode_index_page, index_page_offset,
    initialize_region, sync_parent_directory,
};
use super::{RegionCompactionResult, RegionFile};

impl RegionFile {
    /// Rewrites only active indexed records into a sibling file, syncs its data and dual
    /// indexes, atomically replaces the region path, then fsyncs the parent directory.
    ///
    /// Production callers must run this on a storage/maintenance worker, never the gameplay
    /// owner thread. A zero-dead-byte region is returned unchanged.
    pub fn compact(&mut self) -> Result<RegionCompactionResult, StorageError> {
        let before = self.stats()?;
        if before.dead_bytes == 0 {
            return Ok(RegionCompactionResult {
                generation: self.index.generation,
                records: before.indexed_chunks,
                bytes_reclaimed: 0,
                before,
                after: before,
            });
        }

        let next_generation = self
            .index
            .generation
            .checked_add(1)
            .ok_or(StorageError::IndexGenerationExhausted)?;
        let (temp_path, mut temp_file) = create_compaction_temp(&self.path, next_generation)?;
        if let Err(error) = initialize_region(&mut temp_file, self.world_uuid, self.coord) {
            drop(temp_file);
            let _ = fs::remove_file(&temp_path);
            return Err(error);
        }

        let current_entries = self.index.entries;
        let build = (|| -> Result<RegionIndex, StorageError> {
            let mut compact_index = RegionIndex::empty(next_generation);
            let mut end = RECORD_AREA_OFFSET;

            for (slot, entry) in current_entries.into_iter().enumerate() {
                let Some(entry) = entry else {
                    continue;
                };
                let (record, _decoded) = self.read_indexed_record(slot, entry)?;
                temp_file.seek(SeekFrom::Start(end))?;
                temp_file.write_all(&record)?;
                compact_index.entries[slot] = Some(IndexEntry {
                    record_offset: end,
                    ..entry
                });
                end = end.checked_add(u64::from(entry.record_len)).ok_or(
                    StorageError::CorruptChunkRecord("compacted record offset overflow"),
                )?;
            }

            temp_file.set_len(end)?;
            temp_file.sync_data()?;

            let fallback_index = RegionIndex {
                generation: self.index.generation,
                entries: compact_index.entries,
            };
            temp_file.seek(SeekFrom::Start(index_page_offset(1)))?;
            temp_file.write_all(&encode_index_page(&fallback_index, self.coord))?;
            temp_file.seek(SeekFrom::Start(index_page_offset(0)))?;
            temp_file.write_all(&encode_index_page(&compact_index, self.coord))?;
            temp_file.sync_data()?;

            Ok(compact_index)
        })();

        let compact_index = match build {
            Ok(index) => index,
            Err(error) => {
                drop(temp_file);
                let _ = fs::remove_file(&temp_path);
                return Err(error);
            }
        };

        let replacement = RegionFile {
            file: temp_file,
            path: self.path.clone(),
            world_uuid: self.world_uuid,
            coord: self.coord,
            active_page: 0,
            index: compact_index,
        };
        let after = match replacement.stats() {
            Ok(stats) => stats,
            Err(error) => {
                drop(replacement);
                let _ = fs::remove_file(&temp_path);
                return Err(error);
            }
        };
        let bytes_reclaimed = before.file_bytes.checked_sub(after.file_bytes).ok_or(
            StorageError::CorruptChunkRecord("compaction increased region file size"),
        )?;

        if let Err(error) = fs::rename(&temp_path, &self.path) {
            drop(replacement);
            let _ = fs::remove_file(&temp_path);
            return Err(error.into());
        }

        *self = replacement;
        sync_parent_directory(&self.path)?;

        Ok(RegionCompactionResult {
            generation: self.index.generation,
            records: self.indexed_chunks(),
            bytes_reclaimed,
            before,
            after,
        })
    }
}

fn create_compaction_temp(path: &Path, generation: u64) -> Result<(PathBuf, File), StorageError> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let file_name = path.file_name().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "region path has no file name for compaction",
        )
    })?;

    for attempt in 0..64_u32 {
        let mut candidate_name = file_name.to_os_string();
        candidate_name.push(format!(
            ".compact-{}-{generation}-{attempt}.tmp",
            std::process::id(),
        ));
        let candidate = parent.join(candidate_name);
        match OpenOptions::new()
            .create_new(true)
            .read(true)
            .write(true)
            .open(&candidate)
        {
            Ok(file) => return Ok((candidate, file)),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error.into()),
        }
    }

    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "could not allocate a unique region compaction temp file",
    )
    .into())
}
