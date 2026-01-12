use std::time::Instant;

use parking_lot::{MutexGuard, RwLockReadGuard, RwLockWriteGuard};
#[cfg(feature = "tracing")]
use tracing::trace;

use crate::lock_info::{
    call_location, GuardKind, Location, LockGuard, LockInfo, LockKind, WaitGuard,
};

#[derive(Debug)]
pub struct Mutex<T> {
    lock: parking_lot::Mutex<T>,
    location: Location,
}

impl<T> Mutex<T> {
    pub fn new(item: T) -> Self {
        Self {
            lock: parking_lot::Mutex::new(item),
            location: LockInfo::register(LockKind::Mutex),
        }
    }

    pub fn lock(&self) -> LockGuard<MutexGuard<'_, T>> {
        let guard_kind = GuardKind::Lock;
        let guard_location = call_location();
        #[cfg(feature = "tracing")]
        trace!("Acquiring a {:?} guard at {}", guard_kind, guard_location);

        // Fast path -- try to acquire lock without blocking first
        let timestamp = Instant::now();
        if let Some(guard) = self.lock.try_lock() {
            let wait_time = timestamp.elapsed();
            return LockGuard::new(guard, guard_kind, &self.location, guard_location, wait_time);
        }

        // Lock is contended, create WaitGuard and block
        let wait_guard = WaitGuard::new(guard_kind, &self.location, guard_location);
        let guard = self.lock.lock();
        let wait_time = timestamp.elapsed();
        LockGuard::from_wait_guard(guard, wait_guard, wait_time)
    }

    pub fn try_lock(&self) -> Option<LockGuard<MutexGuard<'_, T>>> {
        let guard_kind = GuardKind::Lock;
        let guard_location = call_location();
        #[cfg(feature = "tracing")]
        trace!(
            "Attempting to acquire a {:?} guard at {}",
            guard_kind,
            guard_location
        );
        let timestamp = Instant::now();
        let guard = self.lock.try_lock().or_else(|| {
            #[cfg(feature = "tracing")]
            trace!(
                "Failed to acquire a {:?} guard at {}",
                guard_kind,
                guard_location,
            );
            None
        })?;
        let wait_time = timestamp.elapsed();
        Some(LockGuard::new(
            guard,
            guard_kind,
            &self.location,
            guard_location,
            wait_time,
        ))
    }
}

impl<T: Default> Default for Mutex<T> {
    fn default() -> Self {
        Self {
            lock: Default::default(),
            location: LockInfo::register(LockKind::Mutex),
        }
    }
}

#[derive(Debug)]
pub struct RwLock<T> {
    lock: parking_lot::RwLock<T>,
    location: Location,
}

impl<T> RwLock<T> {
    pub fn new(item: T) -> Self {
        Self {
            lock: parking_lot::RwLock::new(item),
            location: LockInfo::register(LockKind::RwLock),
        }
    }

    pub fn read(&self) -> LockGuard<RwLockReadGuard<'_, T>> {
        let guard_kind = GuardKind::Read;
        let guard_location = call_location();
        #[cfg(feature = "tracing")]
        trace!("Acquiring a {:?} guard at {}", guard_kind, guard_location);

        // Fast path -- try to acquire lock without blocking first
        let timestamp = Instant::now();
        if let Some(guard) = self.lock.try_read() {
            let wait_time = timestamp.elapsed();
            return LockGuard::new(guard, guard_kind, &self.location, guard_location, wait_time);
        }

        // Lock is contended, create WaitGuard and block
        let wait_guard = WaitGuard::new(guard_kind, &self.location, guard_location);
        let guard = self.lock.read();
        let wait_time = timestamp.elapsed();
        LockGuard::from_wait_guard(guard, wait_guard, wait_time)
    }

    pub fn try_read(&self) -> Option<LockGuard<RwLockReadGuard<'_, T>>> {
        let guard_kind = GuardKind::Read;
        let guard_location = call_location();
        #[cfg(feature = "tracing")]
        trace!(
            "Attempting to acquire a {:?} guard at {}",
            guard_kind,
            guard_location
        );
        let timestamp = Instant::now();
        let guard = self.lock.try_read().or_else(|| {
            #[cfg(feature = "tracing")]
            trace!(
                "Failed to acquire a {:?} guard at {}",
                guard_kind,
                guard_location,
            );
            None
        })?;
        let wait_time = timestamp.elapsed();
        Some(LockGuard::new(
            guard,
            guard_kind,
            &self.location,
            guard_location,
            wait_time,
        ))
    }

    pub fn write(&self) -> LockGuard<RwLockWriteGuard<'_, T>> {
        let guard_kind = GuardKind::Write;
        let guard_location = call_location();
        #[cfg(feature = "tracing")]
        trace!("Acquiring a {:?} guard at {}", guard_kind, guard_location);

        // Fast path -- try to acquire lock without blocking first
        let timestamp = Instant::now();
        if let Some(guard) = self.lock.try_write() {
            let wait_time = timestamp.elapsed();
            return LockGuard::new(guard, guard_kind, &self.location, guard_location, wait_time);
        }

        // Lock is contended, create WaitGuard and block
        let wait_guard = WaitGuard::new(guard_kind, &self.location, guard_location);
        let guard = self.lock.write();
        let wait_time = timestamp.elapsed();
        LockGuard::from_wait_guard(guard, wait_guard, wait_time)
    }

    pub fn try_write(&self) -> Option<LockGuard<RwLockWriteGuard<'_, T>>> {
        let guard_kind = GuardKind::Write;
        let guard_location = call_location();
        #[cfg(feature = "tracing")]
        trace!(
            "Attempting to acquire a {:?} guard at {}",
            guard_kind,
            guard_location
        );
        let timestamp = Instant::now();
        let guard = self.lock.try_write().or_else(|| {
            #[cfg(feature = "tracing")]
            trace!(
                "Failed to acquire a {:?} guard at {}",
                guard_kind,
                guard_location,
            );
            None
        })?;
        let wait_time = timestamp.elapsed();
        Some(LockGuard::new(
            guard,
            guard_kind,
            &self.location,
            guard_location,
            wait_time,
        ))
    }

    pub fn into_inner(self) -> T {
        self.lock.into_inner()
    }
}

impl<T: Default> Default for RwLock<T> {
    fn default() -> Self {
        Self {
            lock: Default::default(),
            location: LockInfo::register(LockKind::RwLock),
        }
    }
}
