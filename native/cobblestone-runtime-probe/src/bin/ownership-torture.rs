use std::error::Error;
use std::io;
use std::time::Instant;

use cobblestone_core::RuntimeId;
use cobblestone_runtime_probe::{OwnedProbeArena, OwnershipError, ProbeHandle};

const ITERATIONS: u64 = 250_000;

fn main() -> Result<(), Box<dyn Error>> {
    let runtime_one = RuntimeId::new(1).ok_or_else(|| io::Error::other("runtime id 1 invalid"))?;
    let runtime_two = RuntimeId::new(2).ok_or_else(|| io::Error::other("runtime id 2 invalid"))?;
    let mut arena = OwnedProbeArena::new();
    let mut retained_stale = Vec::with_capacity(usize::try_from(ITERATIONS)?);
    let mut wrong_owner_rejections = 0_u64;
    let mut stale_epoch_rejections = 0_u64;
    let started = Instant::now();

    for sequence in 0..ITERATIONS {
        let handle = arena
            .create(runtime_one, sequence)
            .map_err(|error| io::Error::other(format!("probe insert failed: {error:?}")))?;
        let initial = arena.snapshot(handle)?;
        if initial.owner() != runtime_one || initial.epoch() != 0 || initial.value() != sequence {
            return Err(io::Error::other("new ownership probe state was corrupted").into());
        }

        match arena.mutate(handle, runtime_two, 0, sequence + 1) {
            Err(OwnershipError::WrongOwner { .. }) => wrong_owner_rejections += 1,
            other => {
                return Err(io::Error::other(format!(
                    "wrong-owner mutation was not rejected: {other:?}"
                ))
                .into());
            }
        }

        arena.mutate(handle, runtime_one, 0, sequence + 2)?;
        let new_epoch = arena.transfer(handle, runtime_one, 0, runtime_two)?;
        if new_epoch != 1 {
            return Err(io::Error::other(format!(
                "unexpected transfer epoch {new_epoch} at sequence {sequence}"
            ))
            .into());
        }

        match arena.mutate(handle, runtime_two, 0, sequence + 3) {
            Err(OwnershipError::StaleEpoch { .. }) => stale_epoch_rejections += 1,
            other => {
                return Err(io::Error::other(format!(
                    "stale ownership epoch was not rejected: {other:?}"
                ))
                .into());
            }
        }
        match arena.mutate(handle, runtime_one, new_epoch, sequence + 4) {
            Err(OwnershipError::WrongOwner { .. }) => wrong_owner_rejections += 1,
            other => {
                return Err(io::Error::other(format!(
                    "previous owner mutation was not rejected: {other:?}"
                ))
                .into());
            }
        }

        arena.mutate(handle, runtime_two, new_epoch, sequence + 5)?;
        let removed = arena.remove(handle, runtime_two, new_epoch)?;
        if removed.owner() != runtime_two
            || removed.epoch() != new_epoch
            || removed.value() != sequence + 5
        {
            return Err(io::Error::other("removed ownership probe state was corrupted").into());
        }
        assert_stale(&arena, handle)?;
        retained_stale.push(handle);

        let replacement = arena
            .create(runtime_one, sequence)
            .map_err(|error| io::Error::other(format!("replacement insert failed: {error:?}")))?;
        if replacement.index() != handle.index()
            || replacement.generation() == handle.generation()
        {
            return Err(io::Error::other("generational slot reuse invariant failed").into());
        }
        arena.remove(replacement, runtime_one, 0)?;
    }

    if !arena.is_empty() {
        return Err(io::Error::other("ownership probe arena leaked live values").into());
    }
    for handle in retained_stale {
        assert_stale(&arena, handle)?;
    }

    println!(
        "ownership-torture: iterations={ITERATIONS} wrong_owner_rejections={wrong_owner_rejections} stale_epoch_rejections={stale_epoch_rejections} elapsed_ms={}",
        started.elapsed().as_millis()
    );
    Ok(())
}

fn assert_stale(arena: &OwnedProbeArena, handle: ProbeHandle) -> Result<(), Box<dyn Error>> {
    match arena.snapshot(handle) {
        Err(OwnershipError::StaleHandle) => Ok(()),
        other => Err(io::Error::other(format!(
            "stale handle unexpectedly resolved: {other:?}"
        ))
        .into()),
    }
}
