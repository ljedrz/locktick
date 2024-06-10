use std::{
    cmp,
    collections::{hash_map::Entry, HashMap},
    fmt,
    ops::{Deref, DerefMut},
    sync::{Arc, Mutex, OnceLock, RwLock},
    time::{Duration, Instant},
};

use simple_moving_average::{NoSumSMA, SMA};
use tracing::trace;

fn call_location() -> Arc<str> {
    let backtrace = backtrace::Backtrace::new();
    let frames = backtrace.frames();
    let symbol = frames
        .iter()
        .flat_map(|frame| frame.symbols())
        .filter(|symbol| {
            if let Some(filename) = symbol.filename().and_then(|path| path.to_str()) {
                !filename.contains("locktick") && !filename.contains("rustc")
            } else {
                false
            }
        })
        .next()
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

pub static LOCK_INFOS: OnceLock<RwLock<HashMap<Arc<str>, LockInfo>>> = OnceLock::new();

#[derive(Clone, Debug)]
pub struct LockInfo(Arc<LockInfoInner>);

#[derive(Debug)]
pub struct LockInfoInner {
    kind: LockKind,
    pub(crate) location: Arc<str>,
    pub(crate) accesses: Mutex<Accesses>,
    pub(crate) guards: Mutex<HashMap<Instant, GuardDetails>>,
    avg_duration: Mutex<NoSumSMA<Duration, u32, 20>>,
}

impl Deref for LockInfo {
    type Target = LockInfoInner;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl LockInfo {
    pub fn new(kind: LockKind) -> Self {
        let location = call_location();

        match LOCK_INFOS
            .get_or_init(Default::default)
            .write()
            .unwrap()
            .entry(location.clone())
        {
            Entry::Vacant(entry) => {
                let info = Self(Arc::new(LockInfoInner {
                    kind,
                    location,
                    accesses: Mutex::new(Accesses::new(kind)),
                    guards: Default::default(),
                    avg_duration: Mutex::new(NoSumSMA::from_zero(Duration::ZERO)),
                }));

                entry.insert(info.clone());
                info
            }
            Entry::Occupied(entry) => entry.get().clone(),
        }
    }
}

impl LockInfoInner {
    pub(crate) fn guard<T>(&self, guard: T, guard_kind: GuardKind) -> LockGuard<T> {
        let acquire_location = call_location();
        let acquire_time = Instant::now();
        trace!(
            "Acquiring a {:?} guard at {:?}",
            guard_kind,
            acquire_location
        );

        let details = GuardDetails {
            guard_kind,
            lock_location: self.location.clone(),
            acquire_location,
            acquire_time,
        };

        if let Some(info) = LOCK_INFOS
            .get_or_init(Default::default) // TODO: check if this is really needed
            .read()
            .unwrap()
            .get(&self.location)
        {
            info.guards
                .lock()
                .unwrap()
                .insert(details.acquire_time, details.clone());
            let accesses = &mut *info.accesses.lock().unwrap();

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
        }

        LockGuard { guard, details }
    }

    pub fn was_used(&self) -> bool {
        match &*self.accesses.lock().unwrap() {
            Accesses::Mutex(n) => *n != 0,
            Accesses::RwLock { reads, writes } => reads + writes != 0,
        }
    }

    pub fn is_active(&self) -> bool {
        !self.guards.lock().unwrap().is_empty()
    }
}

impl fmt::Display for LockInfoInner {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let accesses = &*self.accesses.lock().unwrap();
        write!(
            f,
            "{:?} {:?}: {:?}; {}; avg: {:?}",
            self.kind,
            self.location,
            self.guards.lock().unwrap(),
            accesses,
            self.avg_duration.lock().unwrap().get_average(),
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LockKind {
    Mutex,
    RwLock,
}

pub struct LockGuard<T> {
    pub(crate) guard: T,
    details: GuardDetails,
}

#[derive(Clone)]
pub struct GuardDetails {
    guard_kind: GuardKind,
    lock_location: Arc<str>,
    acquire_location: Arc<str>,
    acquire_time: Instant,
}

impl fmt::Debug for GuardDetails {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GuardDetails")
            .field("guard_kind", &self.guard_kind)
            // .field("lock_location", &self.lock_location)
            .field("acquire_location", &self.acquire_location)
            .field("acquire_time", &self.acquire_time)
            .finish()
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
            .get(&self.details.lock_location)
        {
            let duration = timestamp - self.details.acquire_time;

            info.guards
                .lock()
                .unwrap()
                .remove(&self.details.acquire_time);

            let mut avg_duration = info.avg_duration.lock().unwrap();
            avg_duration.add_sample(duration);

            trace!(
                "The {:?} guard for lock {:?} acquired at {:?} was dropped after {:?} (avg: {:?})",
                self.details.guard_kind,
                self.details.lock_location,
                self.details.acquire_location,
                duration,
                avg_duration.get_average(),
            );
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GuardKind {
    Lock,
    Read,
    Write,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Accesses {
    Mutex(usize),
    RwLock { reads: usize, writes: usize },
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
    use super::*;

    #[test]
    fn location() {
        let info = LockInfo::new(LockKind::Mutex);

        // FIXME
        // assert_eq!(info.location.line(), 249);
        // assert_eq!(info.location.column(), 20);
    }
}
