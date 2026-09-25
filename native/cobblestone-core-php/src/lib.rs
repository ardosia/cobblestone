#![cfg_attr(windows, feature(abi_vectorcall))]

use std::cell::Cell;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Mutex, MutexGuard, OnceLock};

use cobblestone_core::{Arena, Handle, RuntimeId};
use ext_php_rs::exception::{PhpException, PhpResult};
use ext_php_rs::prelude::*;

static NEXT_RUNTIME_ID: AtomicU32 = AtomicU32::new(1);
static PROBES: OnceLock<Mutex<ProbeRegistry>> = OnceLock::new();

thread_local! {
    static RUNTIME_ID: Cell<Option<RuntimeId>> = const { Cell::new(None) };
}

struct Probe;

struct ProbeRegistry {
    arena: Arena<Probe>,
    handles: HashMap<i64, Handle<Probe>>,
    next_token: i64,
}

impl ProbeRegistry {
    fn new() -> Self {
        Self {
            arena: Arena::new(),
            handles: HashMap::new(),
            next_token: 1,
        }
    }

    fn create(&mut self) -> Result<i64, &'static str> {
        let token = self.next_token;
        self.next_token = self
            .next_token
            .checked_add(1)
            .ok_or("diagnostic probe token space exhausted")?;

        let handle = self
            .arena
            .insert(Probe)
            .map_err(|_| "diagnostic probe arena capacity exhausted")?;
        self.handles.insert(token, handle);
        Ok(token)
    }

    fn is_valid(&self, token: i64) -> bool {
        self.handles
            .get(&token)
            .is_some_and(|handle| self.arena.contains(*handle))
    }

    fn remove(&mut self, token: i64) -> Result<(), &'static str> {
        let handle = self
            .handles
            .remove(&token)
            .ok_or("invalid or stale Cobblestone diagnostic handle")?;

        if self.arena.remove(handle).is_none() {
            return Err("Cobblestone diagnostic handle registry became stale");
        }

        Ok(())
    }
}

fn probes() -> MutexGuard<'static, ProbeRegistry> {
    let registry = PROBES.get_or_init(|| Mutex::new(ProbeRegistry::new()));
    match registry.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

fn php_error(message: &'static str) -> PhpException {
    PhpException::default(message.to_owned())
}

fn allocate_runtime_id() -> Result<RuntimeId, &'static str> {
    let raw = NEXT_RUNTIME_ID
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
            current.checked_add(1)
        })
        .map_err(|_| "Cobblestone runtime identity space exhausted")?;

    RuntimeId::new(raw).ok_or("Cobblestone runtime identity allocator produced zero")
}

fn current_runtime_id() -> Result<RuntimeId, &'static str> {
    RUNTIME_ID.with(|slot| {
        if let Some(runtime_id) = slot.get() {
            return Ok(runtime_id);
        }

        let runtime_id = allocate_runtime_id()?;
        slot.set(Some(runtime_id));
        Ok(runtime_id)
    })
}

/// Returns the internal runtime identity attached to the current PHP execution thread.
///
/// This is a diagnostic proof surface for C002, not part of the normal plugin API.
#[php_function]
pub fn cobblestone_core_runtime_id() -> PhpResult<u32> {
    current_runtime_id().map(RuntimeId::get).map_err(php_error)
}

/// Creates an opaque diagnostic native handle.
#[php_function]
pub fn cobblestone_core_probe_create() -> PhpResult<i64> {
    probes().create().map_err(php_error)
}

/// Returns whether an opaque diagnostic native handle is currently live.
#[php_function]
pub fn cobblestone_core_probe_valid(token: i64) -> bool {
    probes().is_valid(token)
}

/// Releases an opaque diagnostic native handle.
///
/// Releasing an unknown or stale token becomes a PHP exception rather than unchecked memory
/// access or a native crash.
#[php_function]
pub fn cobblestone_core_probe_drop(token: i64) -> PhpResult<()> {
    probes().remove(token).map_err(php_error)
}

/// Registers the diagnostic C002 extension proof.
#[php_module]
pub fn get_module(module: ModuleBuilder) -> ModuleBuilder {
    module
        .name("cobblestone_core_php")
        .version(env!("CARGO_PKG_VERSION"))
        .function(wrap_function!(cobblestone_core_runtime_id))
        .function(wrap_function!(cobblestone_core_probe_create))
        .function(wrap_function!(cobblestone_core_probe_valid))
        .function(wrap_function!(cobblestone_core_probe_drop))
}
