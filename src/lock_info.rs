use std::{
    cmp,
    collections::{hash_map::Entry, HashMap},
    fmt,
    ops::{Deref, DerefMut},
    sync::{Arc, Mutex, OnceLock, RwLock},
    time::{Duration, Instant},
};

use rand_core::{RngCore, SeedableRng};
use rand_xorshift::XorShiftRng;
use simple_moving_average::{NoSumSMA, SMA};
use tracing::trace;

fn call_location() -> Arc<str> {
    let backtrace = backtrace::Backtrace::new();
    let frames = backtrace.frames();
    let symbol = frames
        .iter()
        .flat_map(|frame| frame.symbols())
        .find(|symbol| {
            if let Some(filename) = symbol.filename().and_then(|path| path.to_str()) {
                if cfg!(test) {
                    filename.contains("locktick")
                } else {
                    !filename.contains("locktick") && !filename.contains("rustc")
                }
            } else {
                false
            }
        })
        .unwrap();

    let filename = symbol.filename().unwrap().to_str().unwrap();
    format!(
        "{}@{}:{}",
        filename,
        symbol.lineno().unwrap(),
        symbol.colno().unwrap()
    )
    .into()
}

pub static LOCK_INFOS: OnceLock<RwLock<HashMap<Arc<str>, Mutex<LockInfo>>>> = OnceLock::new();

pub fn lock_snapshots() -> Vec<LockInfo> {
    let mut snapshots = LOCK_INFOS
        .get_or_init(Default::default)
        .read()
        .unwrap()
        .values()
        .map(|info| info.lock().unwrap().clone())
        .collect::<Vec<_>>();
    snapshots.sort_unstable_by_key(|s| s.accesses);

    snapshots
}

/// This object contains all the details related to a given lock, and it can only
/// be found in the `LOCK_INFOS` static.
#[derive(Debug, Clone)]
pub struct LockInfo {
    pub(crate) location: Arc<str>,
    pub(crate) accesses: Accesses,
    pub(crate) rng: XorShiftRng,
    pub(crate) guards: HashMap<u64, GuardDetails>,
    avg_duration: NoSumSMA<Duration, u32, 20>,
}

impl LockInfo {
    pub fn register(kind: LockKind) -> Arc<str> {
        let location = call_location();

        match LOCK_INFOS
            .get_or_init(Default::default)
            .write()
            .unwrap()
            .entry(location.clone())
        {
            Entry::Vacant(entry) => {
                let info = Mutex::new(Self {
                    location: location.clone(),
                    accesses: Accesses::new(kind),
                    rng: XorShiftRng::seed_from_u64(0),
                    guards: Default::default(),
                    avg_duration: NoSumSMA::from_zero(Duration::ZERO),
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
        write!(
            f,
            "{}: {}; avg guard duration: {:?}",
            self.location,
            self.accesses,
            self.avg_duration.get_average(),
        )?;

        for guard in self.guards.values() {
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
    pub(crate) guard: T,
    pub lock_location: Arc<str>,
    pub id: u64,
}

impl<T> LockGuard<T> {
    pub(crate) fn new(guard: T, guard_kind: GuardKind, lock_location: &Arc<str>) -> Self {
        let acquire_location = call_location();
        let acquire_time = Instant::now();
        trace!("Acquiring a {:?} guard at {}", guard_kind, acquire_location);

        let details = GuardDetails {
            guard_kind,
            acquire_location,
            acquire_time,
        };

        let id = if let Some(info) = LOCK_INFOS
            .get_or_init(Default::default) // TODO: check if this is really needed
            .read()
            .unwrap()
            .get(lock_location)
        {
            let mut info = info.lock().unwrap();
            let id = info.rng.next_u64();
            info.guards.insert(id, details);

            let accesses = &mut info.accesses;
            match guard_kind {
                GuardKind::Lock => {
                    if let Accesses::Mutex(unlocks) = accesses {
                        *unlocks += 1;
                    } else {
                        unreachable!();
                    }
                }
                GuardKind::Read => {
                    if let Accesses::RwLock { reads, writes: _ } = accesses {
                        *reads += 1;
                    } else {
                        unreachable!();
                    }
                }
                GuardKind::Write => {
                    if let Accesses::RwLock { reads: _, writes } = accesses {
                        *writes += 1;
                    } else {
                        unreachable!();
                    }
                }
            }

            id
        } else {
            unreachable!();
        };

        LockGuard {
            guard,
            lock_location: lock_location.clone(),
            id,
        }
    }
}

/// Guard-related information which - when paired with the corresponding
/// `LockGuard` - provides a full set of data related to a single guard.
#[derive(Debug, Clone)]
pub struct GuardDetails {
    pub guard_kind: GuardKind,
    pub acquire_location: Arc<str>,
    acquire_time: Instant,
}

impl fmt::Display for GuardDetails {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:?} guard acquired {:?} ago at {}",
            self.guard_kind,
            Instant::now() - self.acquire_time,
            self.acquire_location,
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

        if let Some(info) = LOCK_INFOS
            .get()
            .unwrap()
            .read()
            .unwrap()
            .get(&self.lock_location)
        {
            let mut info = info.lock().unwrap();
            let details = info.guards.remove(&self.id).unwrap();
            let duration = timestamp - details.acquire_time;
            info.avg_duration.add_sample(duration);

            trace!(
                "The {:?} guard for lock {} acquired at {} was dropped after {:?}",
                details.guard_kind,
                self.lock_location,
                details.acquire_location,
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

/// Contains information on how many times the lock was accessed; for `RwLock`
/// it is broken down into reads and writes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Accesses {
    Mutex(usize),
    RwLock { reads: usize, writes: usize },
}

impl Accesses {
    pub fn was_used(&self) -> bool {
        match self {
            Accesses::Mutex(n) => *n != 0,
            Accesses::RwLock { reads, writes } => reads + writes != 0,
        }
    }
}

impl Ord for Accesses {
    fn cmp(&self, other: &Self) -> cmp::Ordering {
        match (self, other) {
            (Self::Mutex(a), Self::Mutex(b)) => a.cmp(b),
            (
                Self::RwLock {
                    reads: r1,
                    writes: w1,
                },
                Self::RwLock {
                    reads: r2,
                    writes: w2,
                },
            ) => {
                if w1.cmp(w2) != cmp::Ordering::Equal {
                    w1.cmp(w2)
                } else {
                    r1.cmp(r2)
                }
            }
            (
                Self::Mutex(_),
                Self::RwLock {
                    reads: _,
                    writes: _,
                },
            ) => cmp::Ordering::Greater,
            _ => cmp::Ordering::Less,
        }
    }
}

impl PartialOrd for Accesses {
    fn partial_cmp(&self, other: &Self) -> Option<cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Accesses {
    pub(crate) fn new(lock_kind: LockKind) -> Self {
        match lock_kind {
            LockKind::Mutex => Self::Mutex(0),
            LockKind::RwLock => Self::RwLock {
                reads: 0,
                writes: 0,
            },
        }
    }
}

impl fmt::Display for Accesses {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Mutex(unlocks) => write!(f, "{unlocks} unlocks"),
            Self::RwLock { reads, writes } => write!(f, "{reads} reads, {writes} writes"),
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn location() {
        // TODO
    }
}
