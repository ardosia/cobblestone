#![cfg_attr(windows, feature(abi_vectorcall))]

use std::cell::Cell;
use std::collections::HashMap;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::mpsc::TryRecvError;
use std::sync::{Mutex, MutexGuard, OnceLock};

use cobblestone_core::{Arena, Completion, Handle, NativeBuffer, RuntimeId, WorkerPool};
use ext_php_rs::exception::{PhpException, PhpResult};
use ext_php_rs::prelude::*;

static NEXT_RUNTIME_ID: AtomicU32 = AtomicU32::new(1);
static PROBES: OnceLock<Mutex<ProbeRegistry>> = OnceLock::new();
static ASYNC: Mutex<Option<AsyncRegistry>> = Mutex::new(None);

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

fn probes() -> MutexGuard<'static, ProbeRegistry> {
    let registry = PROBES.get_or_init(|| Mutex::new(ProbeRegistry::new()));
    match registry.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

fn with_async_registry<T>(
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

fn php_error(message: &'static str) -> PhpException {
    PhpException::default(message.to_owned())
}

/// Contains every Cobblestone-owned panic before control returns to the generated Zend handler.
///
/// ext-php-rs 0.15.15 protects Zend bailouts in its generated handler but resumes Rust panics.
/// Cobblestone therefore catches its own entry-body panics explicitly and translates them into a
/// stable PHP exception. Argument marshalling remains part of the pinned ext-php-rs substrate.
fn php_boundary<T>(operation: impl FnOnce() -> PhpResult<T>) -> PhpResult<T> {
    match catch_unwind(AssertUnwindSafe(operation)) {
        Ok(result) => result,
        Err(_) => Err(php_error("Cobblestone native panic contained")),
    }
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

/// Empty diagnostic PHP-to-native call used only for C002 boundary measurement.
#[php_function]
pub fn cobblestone_core_ping() -> PhpResult<i64> {
    php_boundary(|| Ok(0))
}

/// Copies a PHP string into an immutable native buffer and returns its length.
///
/// This is a diagnostic measurement surface, not a gameplay API.
#[php_function]
pub fn cobblestone_core_buffer_copy_len(value: String) -> PhpResult<i64> {
    php_boundary(|| {
        let buffer = NativeBuffer::copy_from_slice(value.as_bytes());
        i64::try_from(buffer.len())
            .map_err(|_| php_error("Cobblestone native buffer length exceeds PHP integer range"))
    })
}

/// Returns the internal runtime identity attached to the current PHP execution thread.
///
/// This is a diagnostic proof surface for C002, not part of the normal plugin API.
#[php_function]
pub fn cobblestone_core_runtime_id() -> PhpResult<u32> {
    php_boundary(|| current_runtime_id().map(RuntimeId::get).map_err(php_error))
}

/// Creates an opaque diagnostic native handle.
#[php_function]
pub fn cobblestone_core_probe_create() -> PhpResult<i64> {
    php_boundary(|| probes().create().map_err(php_error))
}

/// Returns whether an opaque diagnostic native handle is currently live.
#[php_function]
pub fn cobblestone_core_probe_valid(token: i64) -> PhpResult<bool> {
    php_boundary(|| Ok(probes().is_valid(token)))
}

/// Releases an opaque diagnostic native handle.
///
/// Releasing an unknown or stale token becomes a PHP exception rather than unchecked memory
/// access or a native crash.
#[php_function]
pub fn cobblestone_core_probe_drop(token: i64) -> PhpResult<()> {
    php_boundary(|| probes().remove(token).map_err(php_error))
}

/// Deliberately triggers a Rust panic inside the Cobblestone-owned PHP boundary.
///
/// This exists only to validate C002 panic containment. It must never become a normal plugin API.
#[php_function]
pub fn cobblestone_core_probe_panic() -> PhpResult<()> {
    php_boundary(|| panic!("intentional Cobblestone C002 diagnostic panic"))
}

/// Submits a tiny diagnostic native job owned by the current PHP runtime.
///
/// The worker receives only owned native values and never touches Zend/PHP state.
#[php_function]
pub fn cobblestone_core_async_submit(value: i64) -> PhpResult<i64> {
    php_boundary(|| {
        let runtime = current_runtime_id().map_err(php_error)?;
        with_async_registry(|registry| registry.submit(runtime, value)).map_err(php_error)
    })
}

/// Returns whether the current runtime's diagnostic task has a native completion ready.
#[php_function]
pub fn cobblestone_core_async_ready(task: i64) -> PhpResult<bool> {
    php_boundary(|| {
        let runtime = current_runtime_id().map_err(php_error)?;
        with_async_registry(|registry| registry.is_ready(runtime, task)).map_err(php_error)
    })
}

/// Takes a ready diagnostic completion on its owning PHP runtime.
#[php_function]
pub fn cobblestone_core_async_take(task: i64) -> PhpResult<i64> {
    php_boundary(|| {
        let runtime = current_runtime_id().map_err(php_error)?;
        with_async_registry(|registry| registry.take(runtime, task)).map_err(php_error)
    })
}

/// Stops the diagnostic native worker pool while PHP is still inside module shutdown.
///
/// No Rust panic is permitted to leave this raw Zend callback.
unsafe extern "C" fn cobblestone_core_shutdown(_type: i32, _module_number: i32) -> i32 {
    match catch_unwind(AssertUnwindSafe(|| {
        let registry = {
            let mut state = match ASYNC.lock() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
            state.take()
        };
        drop(registry);
    })) {
        Ok(()) => 0,
        Err(_) => -1,
    }
}

/// Registers the diagnostic C002 extension proof.
#[php_module]
pub fn get_module(module: ModuleBuilder) -> ModuleBuilder {
    module
        .name("cobblestone_core_php")
        .version(env!("CARGO_PKG_VERSION"))
        .shutdown_function(cobblestone_core_shutdown)
        .function(wrap_function!(cobblestone_core_ping))
        .function(wrap_function!(cobblestone_core_buffer_copy_len))
        .function(wrap_function!(cobblestone_core_runtime_id))
        .function(wrap_function!(cobblestone_core_probe_create))
        .function(wrap_function!(cobblestone_core_probe_valid))
        .function(wrap_function!(cobblestone_core_probe_drop))
        .function(wrap_function!(cobblestone_core_probe_panic))
        .function(wrap_function!(cobblestone_core_async_submit))
        .function(wrap_function!(cobblestone_core_async_ready))
        .function(wrap_function!(cobblestone_core_async_take))
}
