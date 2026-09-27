use std::cell::Cell;
use std::sync::atomic::{AtomicU32, Ordering};

use cobblestone_core::RuntimeId;

static NEXT_RUNTIME_ID: AtomicU32 = AtomicU32::new(1);

thread_local! {
    static RUNTIME_ID: Cell<Option<RuntimeId>> = const { Cell::new(None) };
}

fn allocate_runtime_id() -> Result<RuntimeId, &'static str> {
    let raw = NEXT_RUNTIME_ID
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
            current.checked_add(1)
        })
        .map_err(|_| "Cobblestone runtime identity space exhausted")?;

    RuntimeId::new(raw).ok_or("Cobblestone runtime identity allocator produced zero")
}

pub(crate) fn current_runtime_id() -> Result<RuntimeId, &'static str> {
    RUNTIME_ID.with(|slot| {
        if let Some(runtime_id) = slot.get() {
            return Ok(runtime_id);
        }

        let runtime_id = allocate_runtime_id()?;
        slot.set(Some(runtime_id));
        Ok(runtime_id)
    })
}
