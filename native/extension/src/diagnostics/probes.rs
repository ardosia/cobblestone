use std::collections::HashMap;
use std::sync::{Mutex, MutexGuard, OnceLock};

use cobblestone_core::{Arena, Handle};
use ext_php_rs::exception::PhpResult;
use ext_php_rs::prelude::*;

use crate::boundary::{php_boundary, php_error};

static PROBES: OnceLock<Mutex<ProbeRegistry>> = OnceLock::new();

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

#[php_function]
pub fn cobblestone_core_probe_create() -> PhpResult<i64> {
    php_boundary(|| probes().create().map_err(php_error))
}

#[php_function]
pub fn cobblestone_core_probe_valid(token: i64) -> PhpResult<bool> {
    php_boundary(|| Ok(probes().is_valid(token)))
}

#[php_function]
pub fn cobblestone_core_probe_drop(token: i64) -> PhpResult<()> {
    php_boundary(|| probes().remove(token).map_err(php_error))
}

#[php_function]
pub fn cobblestone_core_probe_panic() -> PhpResult<()> {
    php_boundary(|| panic!("intentional Cobblestone C002 diagnostic panic"))
}

pub(super) fn register(module: ModuleBuilder) -> ModuleBuilder {
    module
        .function(wrap_function!(cobblestone_core_probe_create))
        .function(wrap_function!(cobblestone_core_probe_valid))
        .function(wrap_function!(cobblestone_core_probe_drop))
        .function(wrap_function!(cobblestone_core_probe_panic))
}
