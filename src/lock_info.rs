use std::{
    collections::{hash_map::Entry, HashMap},
    fmt,
    ops::{Deref, DerefMut},
    path::Path,
    sync::{Arc, Mutex, OnceLock, RwLock},
    time::{Duration, Instant},
};

use rand_core::{RngCore, SeedableRng};
use rand_xorshift::XorShiftRng;
use simple_moving_average::{SingleSumSMA, SMA};
use tracing::trace;

static LOCK_INFOS: OnceLock<RwLock<HashMap<Location, Mutex<LockInfo>>>> = OnceLock::new();

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
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

fn call_location() -> Location {
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

pub fn lock_snapshots() -> Vec<LockInfo> {
    let snapshots = LOCK_INFOS
        .get_or_init(Default::default)
        .read()
        .unwrap()
        .values()
        .map(|info| info.lock().unwrap().clone())
        .collect::<Vec<_>>();

    snapshots
}

/// This object contains all the details related to a given lock, and it can only
/// be found in the `LOCK_INFOS` static.
#[derive(Debug, Clone)]
pub struct LockInfo {
    pub kind: LockKind,
    pub location: Location,
    pub known_guards: HashMap<Location, GuardInfo>,
    rng: XorShiftRng,
}

impl LockInfo {
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
                    rng: XorShiftRng::seed_from_u64(0),
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
            write!(f, "\n- {}", guard)?;
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

/// A wrapper for the lock guard produced when working with the lock. It
/// only contains the guard itself and metadata that allows it to be uniquely
/// identified in the `LOCK_INFOS` static.
pub struct LockGuard<T> {
    guard: T,
    pub lock_location: Location,
    pub guard_location: Location,
    id: u64,
}

impl<T> LockGuard<T> {
    pub(crate) fn new(guard: T, guard_kind: GuardKind, lock_location: &Location) -> Self {
        let acquire_time = Instant::now();
        let guard_location = call_location();
        trace!("Acquiring a {:?} guard at {}", guard_kind, guard_location);

        let id = if let Some(lock_info) = LOCK_INFOS
            .get_or_init(Default::default) // TODO: check if this is really needed
            .read()
            .unwrap()
            .get(lock_location)
        {
            let mut lock_info = lock_info.lock().unwrap();

            let guard_id = lock_info.rng.next_u64();
            let guard_info = lock_info
                .known_guards
                .entry(guard_location.clone())
                .or_insert_with(|| GuardInfo::new(guard_kind, guard_location.clone()));
            guard_info.active_uses.insert(guard_id, acquire_time);
            guard_info.num_uses += 1;

            guard_id
        } else {
            unreachable!();
        };

        LockGuard {
            guard,
            lock_location: lock_location.clone(),
            guard_location,
            id,
        }
    }
}

/// Guard-related information which - when paired with the corresponding
/// `LockGuard` - provides a full set of data related to a single guard.
#[derive(Debug, Clone)]
pub struct GuardInfo {
    pub kind: GuardKind,
    pub location: Location,
    pub num_uses: usize,
    active_uses: HashMap<u64, Instant>,
    avg_duration: SingleSumSMA<Duration, u32, 50>,
}

impl GuardInfo {
    fn new(kind: GuardKind, location: Location) -> Self {
        Self {
            kind,
            location,
            num_uses: 0,
            active_uses: Default::default(),
            avg_duration: SingleSumSMA::from_zero(Duration::ZERO),
        }
    }

    pub fn num_active_uses(&self) -> usize {
        self.active_uses.len()
    }

    pub fn avg_duration(&self) -> Duration {
        self.avg_duration.get_average()
    }
}

impl fmt::Display for GuardInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} ({:?}): curr users: {}; calls: {}; avg duration: {:?}",
            self.location,
            self.kind,
            self.active_uses.len(),
            self.num_uses,
            self.avg_duration.get_average(),
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
            let guard_timestamp = known_guard.active_uses.remove(&self.id).unwrap();
            let duration = timestamp - guard_timestamp;
            known_guard.avg_duration.add_sample(duration);

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
