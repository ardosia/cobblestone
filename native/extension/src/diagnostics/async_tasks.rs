use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::mpsc::TryRecvError;

use cobblestone_runtime::{Completion, RuntimeId, WorkerPool};
use ext_php_rs::exception::PhpResult;
use ext_php_rs::prelude::*;

use crate::boundary::{php_boundary, php_error};
use crate::runtime::current_runtime_id;

static ASYNC: Mutex<Option<AsyncRegistry>> = Mutex::new(None);

#[derive(Copy, Clone)]
struct AsyncJob {
    owner: RuntimeId,
    value: i64,
}

#[derive(Copy, Clone)]
struct AsyncResult {
    owner: RuntimeId,
    value: i64,
}

enum AsyncReady {
    Completed { owner: RuntimeId, value: i64 },
    Cancelled { owner: RuntimeId },
    Panicked { owner: RuntimeId },
}

impl AsyncReady {
    const fn owner(&self) -> RuntimeId {
        match self {
            Self::Completed { owner, .. }
            | Self::Cancelled { owner }
            | Self::Panicked { owner } => *owner,
        }
    }
}

struct AsyncRegistry {
    pool: WorkerPool<AsyncJob, AsyncResult>,
    owners: HashMap<i64, RuntimeId>,
    ready: HashMap<i64, AsyncReady>,
}

impl AsyncRegistry {
    fn new() -> Result<Self, &'static str> {
        let pool = WorkerPool::new(2, 64, 64, |job: AsyncJob, _| AsyncResult {
            owner: job.owner,
            value: job.value.wrapping_mul(2),
        })
        .map_err(|_| "failed to initialize Cobblestone diagnostic worker pool")?;

        Ok(Self {
            pool,
            owners: HashMap::new(),
            ready: HashMap::new(),
        })
    }

    fn submit(&mut self, owner: RuntimeId, value: i64) -> Result<i64, &'static str> {
        let handle = self
            .pool
            .try_submit(AsyncJob { owner, value })
            .map_err(|_| "Cobblestone diagnostic worker queue is saturated or unavailable")?;
        let task = i64::try_from(handle.id().get())
            .map_err(|_| "Cobblestone diagnostic task identity exceeds PHP integer range")?;
        self.owners.insert(task, owner);
        Ok(task)
    }

    fn refresh(&mut self) -> Result<(), &'static str> {
        loop {
            let completion = match self.pool.try_recv_completion() {
                Ok(completion) => completion,
                Err(TryRecvError::Empty) => return Ok(()),
                Err(TryRecvError::Disconnected) => {
                    return Err("Cobblestone diagnostic completion channel disconnected");
                }
            };

            let task = i64::try_from(completion.id().get())
                .map_err(|_| "Cobblestone diagnostic task identity exceeds PHP integer range")?;
            let owner = *self
                .owners
                .get(&task)
                .ok_or("Cobblestone diagnostic completion referenced an unknown task")?;

            let ready = match completion {
                Completion::Completed { result, .. } => {
                    if result.owner != owner {
                        return Err("Cobblestone diagnostic completion owner mismatch");
                    }
                    AsyncReady::Completed {
                        owner,
                        value: result.value,
                    }
                }
                Completion::Cancelled { .. } => AsyncReady::Cancelled { owner },
                Completion::Panicked { .. } => AsyncReady::Panicked { owner },
            };

            if self.ready.insert(task, ready).is_some() {
                return Err("Cobblestone diagnostic task completed more than once");
            }
        }
    }

    fn assert_owner(&self, runtime: RuntimeId, task: i64) -> Result<(), &'static str> {
        let owner = self
            .owners
            .get(&task)
            .ok_or("unknown Cobblestone diagnostic async task")?;
        if *owner != runtime {
            return Err("Cobblestone diagnostic async task belongs to another runtime");
        }
        Ok(())
    }

    fn is_ready(&mut self, runtime: RuntimeId, task: i64) -> Result<bool, &'static str> {
        self.refresh()?;
        self.assert_owner(runtime, task)?;
        Ok(self.ready.contains_key(&task))
    }

    fn take(&mut self, runtime: RuntimeId, task: i64) -> Result<i64, &'static str> {
        self.refresh()?;
        self.assert_owner(runtime, task)?;

        let ready = self
            .ready
            .remove(&task)
            .ok_or("Cobblestone diagnostic async task is not ready")?;
        if ready.owner() != runtime {
            return Err("Cobblestone diagnostic ready completion owner mismatch");
        }
        self.owners.remove(&task);

        match ready {
            AsyncReady::Completed { value, .. } => Ok(value),
            AsyncReady::Cancelled { .. } => Err("Cobblestone diagnostic async task was cancelled"),
            AsyncReady::Panicked { .. } => Err("Cobblestone diagnostic async task panicked"),
        }
    }
}

fn with_registry<T>(
    operation: impl FnOnce(&mut AsyncRegistry) -> Result<T, &'static str>,
) -> Result<T, &'static str> {
    let mut state = match ASYNC.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };

    if state.is_none() {
        *state = Some(AsyncRegistry::new()?);
    }

    let registry = state
        .as_mut()
        .ok_or("Cobblestone diagnostic async registry was unavailable")?;
    operation(registry)
}

#[php_function]
pub fn cobblestone_core_async_submit(value: i64) -> PhpResult<i64> {
    php_boundary(|| {
        let runtime = current_runtime_id().map_err(php_error)?;
        with_registry(|registry| registry.submit(runtime, value)).map_err(php_error)
    })
}

#[php_function]
pub fn cobblestone_core_async_ready(task: i64) -> PhpResult<bool> {
    php_boundary(|| {
        let runtime = current_runtime_id().map_err(php_error)?;
        with_registry(|registry| registry.is_ready(runtime, task)).map_err(php_error)
    })
}

#[php_function]
pub fn cobblestone_core_async_take(task: i64) -> PhpResult<i64> {
    php_boundary(|| {
        let runtime = current_runtime_id().map_err(php_error)?;
        with_registry(|registry| registry.take(runtime, task)).map_err(php_error)
    })
}

pub(super) fn register(module: ModuleBuilder) -> ModuleBuilder {
    module
        .function(wrap_function!(cobblestone_core_async_submit))
        .function(wrap_function!(cobblestone_core_async_ready))
        .function(wrap_function!(cobblestone_core_async_take))
}

pub(super) fn shutdown() {
    let registry = {
        let mut state = match ASYNC.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        state.take()
    };
    drop(registry);
}
