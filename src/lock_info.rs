use std::{
    cmp,
    collections::HashMap,
    fmt,
    ops::{Deref, DerefMut},
    sync::{Arc, Mutex, OnceLock, RwLock},
    time::{Duration, Instant},
};

use simple_moving_average::{NoSumSMA, SMA};
use tracing::trace;

// FIXME: paths
fn call_location() -> Arc<str> {
    let backtrace = backtrace::Backtrace::new();
    let frames = backtrace.frames();
    let symbol = frames
        .iter()
        .flat_map(|frame| frame.symbols())
        .find(|symbol| {
            symbol
                .filename()
                .is_some_and(|name| name.to_str().is_some_and(|s| s.contains("snark")))
        })
        .unwrap();

    let filename = symbol.filename().unwrap().to_str().unwrap();
    let filename = filename.trim_start_matches("/home/ljedrz/git/aleo/");
    format!(
        "{}@{}:{}",
        filename,
        symbol.lineno().unwrap(),
        symbol.colno().unwrap()
    )
    .into()
}

pub static LOCK_INFOS: OnceLock<RwLock<HashMap<Arc<str>, LockInfo>>> = OnceLock::new();

#[derive(Debug, Clone)]
pub struct LockInfo {
    kind: LockKind,
    pub(crate) state: Arc<Mutex<LockState>>,
    pub(crate) location: Arc<str>,
    pub(crate) accesses: Arc<Mutex<Accesses>>,
    avg_duration: Arc<Mutex<NoSumSMA<Duration, u32, 20>>>,
}

impl LockInfo {
    pub fn new(kind: LockKind) -> Self {
        let location = call_location();
        let info = Self {
            kind,
            state: Arc::new(Mutex::new(LockState::Locked)),
            location: location.clone(),
            accesses: Arc::new(Mutex::new(Accesses::new(kind))),
            avg_duration: Arc::new(Mutex::new(NoSumSMA::from_zero(Duration::ZERO))),
        };

        LOCK_INFOS
            .get_or_init(Default::default)
            .write()
            .unwrap()
            .insert(location, info.clone());

        info
    }

    pub(crate) fn guard<T>(&self, actual_guard: T, guard_kind: GuardKind) -> LockGuard<T> {
        if let Some(info) = LOCK_INFOS
            .get_or_init(Default::default) // TODO: check if this is really needed
            .read()
            .unwrap()
            .get(&self.location)
        {
            let curr_state = &mut *info.state.lock().unwrap();
            let accesses = &mut *info.accesses.lock().unwrap();

            match guard_kind {
                GuardKind::Lock => {
                    match curr_state {
                        LockState::Locked => {
                            *curr_state = LockState::Unlocked;
                        }
                        _ => unreachable!(),
                    }

                    if let Accesses::Mutex(unlocks) = accesses {
                        *unlocks += 1;
                    } else {
                        unreachable!();
                    }
                }
                GuardKind::Read => {
                    match curr_state {
                        LockState::Reading(num_readers) => {
                            *num_readers += 1;
                        }
                        curr_state => {
                            *curr_state = LockState::Reading(1);
                        }
                    }

                    if let Accesses::RwLock { reads, writes: _ } = accesses {
                        *reads += 1;
                    } else {
                        unreachable!();
                    }
                }
                GuardKind::Write => {
                    match curr_state {
                        LockState::Locked => {
                            *curr_state = LockState::Writing;
                        }
                        _ => unreachable!(),
                    }

                    if let Accesses::RwLock { reads: _, writes } = accesses {
                        *writes += 1;
                    } else {
                        unreachable!();
                    }
                }
            }
        }

        LockGuard::new(actual_guard, self.location.clone(), guard_kind)
    }
}

impl fmt::Display for LockInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:?} {:?}: {:?}; {}; avg: {:?}",
            self.kind,
            self.location,
            &*self.state.lock().unwrap(),
            &*self.accesses.lock().unwrap(),
            self.avg_duration.lock().unwrap().get_average(),
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LockKind {
    Mutex,
    RwLock,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LockState {
    Locked,
    Unlocked,
    Reading(usize),
    Writing,
}

pub struct LockGuard<T> {
    pub(crate) lock: T,
    kind: GuardKind,
    lock_location: Arc<str>,
    acquire_location: Arc<str>,
    acquire_time: Instant,
}

impl<T> LockGuard<T> {
    pub(crate) fn new(lock: T, lock_location: Arc<str>, kind: GuardKind) -> Self {
        let acquire_location = call_location();
        trace!("Acquiring a {:?} guard at {:?}", kind, acquire_location);

        Self {
            lock,
            kind,
            lock_location,
            acquire_location,
            acquire_time: Instant::now(),
        }
    }
}

impl<T: Deref> Deref for LockGuard<T> {
    type Target = T::Target;

    fn deref(&self) -> &Self::Target {
        self.lock.deref()
    }
}

impl<T: DerefMut> DerefMut for LockGuard<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.lock.deref_mut()
    }
}

impl<T> Drop for LockGuard<T> {
    fn drop(&mut self) {
        if let Some(info) = LOCK_INFOS
            .get()
            .unwrap()
            .read()
            .unwrap()
            .get(&self.lock_location)
        {
            let duration = Instant::now() - self.acquire_time;

            // TODO: considering providing info on number of current readers
            match &mut *info.state.lock().unwrap() {
                LockState::Reading(num_readers) if *num_readers > 1 => {
                    *num_readers -= 1;
                }
                curr_state => {
                    *curr_state = LockState::Locked;
                }
            }

            let mut avg_duration = info.avg_duration.lock().unwrap();
            avg_duration.add_sample(duration);

            trace!(
                "The {:?} guard for lock {:?} acquired at {:?} was dropped after {:?} (avg: {:?})",
                self.kind,
                self.lock_location,
                self.acquire_location,
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
