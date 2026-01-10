use std::{
    collections::{hash_map::Entry, HashMap},
    fmt,
    ops::{Deref, DerefMut},
    path::Path,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex, OnceLock, RwLock,
    },
    time::{Duration, Instant},
};

use simple_moving_average::{SingleSumSMA, SMA};
#[cfg(feature = "tracing")]
use tracing::trace;

// Contains data on all created locks and their guards.
static LOCK_INFOS: OnceLock<RwLock<HashMap<Location, Mutex<LockInfo>>>> = OnceLock::new();

// Provides a common source of indices for all the guards.
static GUARD_COUNTER: AtomicUsize = AtomicUsize::new(0);

/// Points to the filesystem location where a lock or guard was created.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Location {
    pub path: Arc<Path>,
    pub line: u32,
    pub col: u32,
}

impl fmt::Display for Location {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}@{}:{}", self.path.display(), self.line, self.col)
    }
}

// Provides the means to procure the location of a lock or its guard.
pub(crate) fn call_location() -> Location {
    let backtrace = backtrace::Backtrace::new();
    let frames = backtrace.frames();
    let symbol = frames
        .iter()
        .flat_map(|frame| frame.symbols())
        .find(|symbol| {
            if let Some(filename) = symbol.filename().and_then(|path| path.to_str()) {
                if cfg!(feature = "test") {
                    filename.contains("tests")
                } else {
                    !filename.contains("locktick") && !filename.contains("rustc")
                }
            } else {
                false
            }
        })
        .unwrap();
    let path = symbol.filename().unwrap().into();

    Location {
        path,
        line: symbol.lineno().unwrap(),
        col: symbol.colno().unwrap(),
    }
}

/// Returns a vector containing snapshots of the data related to all the locks.
pub fn lock_snapshots() -> Vec<LockInfo> {
    LOCK_INFOS
        .get_or_init(Default::default)
        .read()
        .unwrap()
        .values()
        .map(|info| info.lock().unwrap().clone())
        .collect()
}

#[cfg(feature = "test")]
pub fn clear_lock_infos() {
    LOCK_INFOS
        .get_or_init(Default::default)
        .write()
        .unwrap()
        .clear();
}

/// Contains all the details related to a given lock, and it can only
/// be obtained through a call to `lock_snapshots`.
#[derive(Debug, Clone)]
pub struct LockInfo {
    pub kind: LockKind,
    pub location: Location,
    pub known_guards: HashMap<Location, GuardInfo>,
}

impl LockInfo {
    /// Registers the creation of a lock; this is meant to be called
    /// when creating wrapper objects for different kinds of locks.
    pub(crate) fn register(kind: LockKind) -> Location {
        let location = call_location();

        match LOCK_INFOS
            .get_or_init(Default::default)
            .write()
            .unwrap()
            .entry(location.clone())
        {
            Entry::Vacant(entry) => {
                let info = Mutex::new(Self {
                    kind,
                    location: location.clone(),
                    known_guards: Default::default(),
                });

                entry.insert(info);
                location
            }
            Entry::Occupied(entry) => entry.get().lock().unwrap().location.clone(),
        }
    }
}

impl fmt::Display for LockInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({:?}):", self.location, self.kind)?;

        for guard in self.known_guards.values() {
            write!(f, "\n- {guard}")?;
        }

        Ok(())
    }
}

/// The type of the lock; either a `Mutex` or an `RwLock`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LockKind {
    Mutex,
    RwLock,
}

/// A wrapper for the lock guard produced when working with a lock. It
/// only contains the guard itself and metadata that allows it to be
/// distinguished from other guards belonging to a single lock.
pub struct LockGuard<T> {
    guard: T,
    pub lock_location: Location,
    pub guard_location: Location,
    pub guard_index: usize,
}

impl<T> LockGuard<T> {
    /// Registers the creation of a guard and returns it wrapped in an object
    /// used to perform relevant accounting when the guard is dropped.
    pub(crate) fn new(
        guard: T,
        guard_kind: GuardKind,
        lock_location: &Location,
        guard_location: Location,
        wait_time: Duration,
    ) -> Self {
        #[cfg(feature = "tracing")]
        trace!("Acquired a {:?} guard at {}", guard_kind, guard_location);

        let guard_index = if let Some(lock_info) = LOCK_INFOS
            .get_or_init(Default::default) // TODO: check if this is really needed
            .read()
            .unwrap()
            .get(lock_location)
        {
            let guard_idx = GUARD_COUNTER.fetch_add(1, Ordering::Relaxed);
            let mut lock_info = lock_info.lock().unwrap();

            let guard_info = lock_info
                .known_guards
                .entry(guard_location.clone())
                .or_insert_with(|| GuardInfo::new(guard_kind, guard_location.clone()));
            guard_info.num_uses += 1;
            guard_info.avg_wait_time.add_sample(wait_time);
            if wait_time > guard_info.max_wait_time {
                guard_info.max_wait_time = wait_time;
            }
            guard_info.active_uses.insert(guard_idx, Instant::now());

            guard_idx
        } else {
            unreachable!();
        };

        LockGuard {
            guard,
            lock_location: lock_location.clone(),
            guard_location,
            guard_index,
        }
    }

    /// Registers the creation of a guard from a WaitGuard, reusing the wait index.
    /// This is called when a waiting task successfully acquires the lock.
    pub(crate) fn from_wait_guard(guard: T, wait_guard: WaitGuard, wait_time: Duration) -> Self {
        let guard_kind = wait_guard.guard_kind;
        let lock_location = wait_guard.lock_location.clone();
        let guard_location = wait_guard.guard_location.clone();
        let guard_index = wait_guard.wait_index;

        // Consume the wait guard without running its Drop impl
        wait_guard.finish();

        #[cfg(feature = "tracing")]
        trace!("Acquired a {:?} guard at {}", guard_kind, guard_location);

        if let Some(lock_info) = LOCK_INFOS
            .get_or_init(Default::default)
            .read()
            .unwrap()
            .get(&lock_location)
        {
            let mut lock_info = lock_info.lock().unwrap();

            let guard_info = lock_info
                .known_guards
                .entry(guard_location.clone())
                .or_insert_with(|| GuardInfo::new(guard_kind, guard_location.clone()));

            // Remove from waiting, add to active
            guard_info.waiting_tasks.remove(&guard_index);
            guard_info.num_uses += 1;
            guard_info.avg_wait_time.add_sample(wait_time);
            if wait_time > guard_info.max_wait_time {
                guard_info.max_wait_time = wait_time;
            }
            guard_info.active_uses.insert(guard_index, Instant::now());
        } else {
            unreachable!();
        }

        LockGuard {
            guard,
            lock_location,
            guard_location,
            guard_index,
        }
    }
}

/// A RAII guard that tracks when a task is waiting for a lock.
/// When dropped, it automatically unregisters the waiting task.
pub struct WaitGuard {
    pub(crate) lock_location: Location,
    pub(crate) guard_location: Location,
    pub(crate) guard_kind: GuardKind,
    pub(crate) wait_index: usize,
    finished: bool,
}

impl WaitGuard {
    /// Creates a new WaitGuard and registers the waiting task.
    pub(crate) fn new(
        guard_kind: GuardKind,
        lock_location: &Location,
        guard_location: Location,
    ) -> Self {
        #[cfg(feature = "tracing")]
        trace!(
            "Task waiting for {:?} guard at {}",
            guard_kind,
            guard_location
        );

        let wait_index = GUARD_COUNTER.fetch_add(1, Ordering::Relaxed);

        if let Some(lock_info) = LOCK_INFOS
            .get_or_init(Default::default)
            .read()
            .unwrap()
            .get(lock_location)
        {
            let mut lock_info = lock_info.lock().unwrap();

            let guard_info = lock_info
                .known_guards
                .entry(guard_location.clone())
                .or_insert_with(|| GuardInfo::new(guard_kind, guard_location.clone()));
            guard_info.waiting_tasks.insert(wait_index, Instant::now());
        } else {
            unreachable!();
        }

        WaitGuard {
            lock_location: lock_location.clone(),
            guard_location,
            guard_kind,
            wait_index,
            finished: false,
        }
    }

    /// Marks this WaitGuard as finished, preventing the Drop impl from running.
    /// This should be called when the lock has been successfully acquired.
    pub(crate) fn finish(mut self) {
        self.finished = true;
    }
}

impl Drop for WaitGuard {
    fn drop(&mut self) {
        if self.finished {
            return;
        }

        #[cfg(feature = "tracing")]
        trace!(
            "Task stopped waiting for {:?} guard at {} (cancelled or failed)",
            self.guard_kind,
            self.guard_location
        );

        if let Some(lock_info) = LOCK_INFOS
            .get()
            .unwrap()
            .read()
            .unwrap()
            .get(&self.lock_location)
        {
            let mut lock_info = lock_info.lock().unwrap();
            if let Some(guard_info) = lock_info.known_guards.get_mut(&self.guard_location) {
                guard_info.waiting_tasks.remove(&self.wait_index);
            }
        }
    }
}

/// Contains data and statistics related to a single guard.
#[derive(Debug, Clone)]
pub struct GuardInfo {
    pub kind: GuardKind,
    pub location: Location,
    pub num_uses: usize,
    active_uses: HashMap<usize, Instant>,
    waiting_tasks: HashMap<usize, Instant>,
    avg_wait_time: SingleSumSMA<Duration, u32, 50>,
    pub max_wait_time: Duration,
    avg_duration: SingleSumSMA<Duration, u32, 50>,
    pub max_duration: Duration,
}

impl GuardInfo {
    fn new(kind: GuardKind, location: Location) -> Self {
        Self {
            kind,
            location,
            num_uses: 0,
            active_uses: Default::default(),
            waiting_tasks: Default::default(),
            avg_wait_time: SingleSumSMA::from_zero(Duration::ZERO),
            max_wait_time: Duration::ZERO,
            avg_duration: SingleSumSMA::from_zero(Duration::ZERO),
            max_duration: Duration::ZERO,
        }
    }

    /// Returns the number of current uses of the guard. It can
    /// be greater than `1` only in case of a read guard, and `0`
    /// indicates that the guard is currently inactive.
    pub fn num_active_uses(&self) -> usize {
        self.active_uses.len()
    }

    /// Returns numbers corresponding to the order in which the currently
    /// active guards were called, which can be useful for debugging purposes.
    pub fn active_call_indices(&self) -> Vec<usize> {
        let mut indices = self.active_uses.keys().copied().collect::<Vec<_>>();
        indices.sort_unstable();
        indices
    }

    /// Returns the number of tasks currently waiting to acquire this guard.
    pub fn num_waiting(&self) -> usize {
        self.waiting_tasks.len()
    }

    /// Returns numbers corresponding to the order in which the currently
    /// waiting tasks started waiting, which can be useful for debugging purposes.
    pub fn waiting_call_indices(&self) -> Vec<usize> {
        let mut indices = self.waiting_tasks.keys().copied().collect::<Vec<_>>();
        indices.sort_unstable();
        indices
    }

    /// Returns the average wait time for the guard. It is a moving
    /// average that gets updated with each use.
    pub fn avg_wait_time(&self) -> Duration {
        self.avg_wait_time.get_average()
    }

    /// Returns the average duration of the guard. It is a moving
    /// average that gets updated with each use.
    pub fn avg_duration(&self) -> Duration {
        self.avg_duration.get_average()
    }
}

impl fmt::Display for GuardInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} ({:?}): curr users: {}; waiting: {}; calls: {}; duration: {:?} avg, {:?} max; wait: {:?} avg, {:?} max",
            self.location,
            self.kind,
            self.active_uses.len(),
            self.waiting_tasks.len(),
            self.num_uses,
            self.avg_duration.get_average(),
            self.max_duration,
            self.avg_wait_time.get_average(),
            self.max_wait_time,
        )
    }
}

impl<T: Deref> Deref for LockGuard<T> {
    type Target = T::Target;

    fn deref(&self) -> &Self::Target {
        self.guard.deref()
    }
}

impl<T: DerefMut> DerefMut for LockGuard<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.guard.deref_mut()
    }
}

impl<T> Drop for LockGuard<T> {
    fn drop(&mut self) {
        let timestamp = Instant::now();

        if let Some(lock_info) = LOCK_INFOS
            .get()
            .unwrap()
            .read()
            .unwrap()
            .get(&self.lock_location)
        {
            let mut lock_info = lock_info.lock().unwrap();
            let known_guard = lock_info
                .known_guards
                .get_mut(&self.guard_location)
                .unwrap();
            let guard_timestamp = known_guard.active_uses.remove(&self.guard_index).unwrap();
            let duration = timestamp - guard_timestamp;
            known_guard.avg_duration.add_sample(duration);
            if duration > known_guard.max_duration {
                known_guard.max_duration = duration;
            }

            #[cfg(feature = "tracing")]
            trace!(
                "The {:?} guard for lock {} acquired at {} was dropped after {:?}",
                known_guard.kind,
                self.lock_location,
                known_guard.location,
                duration,
            );
        }
    }
}

/// The type of the guard that was created when working with a lock.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GuardKind {
    Lock,
    Read,
    Write,
}
