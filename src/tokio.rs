use std::future::Future;
use std::time::Instant;

use tokio::sync::{MutexGuard, RwLockReadGuard, RwLockWriteGuard, TryLockError};
#[cfg(feature = "tracing")]
use tracing::trace;

use crate::lock_info::{
    call_location, GuardKind, Location, LockGuard, LockInfo, LockKind, WaitGuard,
};

#[derive(Debug)]
pub struct Mutex<T> {
    lock: tokio::sync::Mutex<T>,
    location: Location,
}

impl<T> Mutex<T> {
    #[track_caller]
    pub fn new(item: T) -> Self {
        Self {
            lock: tokio::sync::Mutex::new(item),
            location: LockInfo::register(LockKind::Mutex),
        }
    }

    #[track_caller]
    pub fn lock(&self) -> impl Future<Output = LockGuard<MutexGuard<'_, T>>> {
        let guard_kind = GuardKind::Lock;
        let guard_location = call_location();

        let lock_ref = &self.lock;
        let loc = &self.location;
        async move {
            #[cfg(feature = "tracing")]
            trace!("Acquiring a {:?} guard at {}", guard_kind, guard_location);

            // Fast path -- try to acquire lock without blocking first
            let timestamp = Instant::now();
            if let Ok(guard) = lock_ref.try_lock() {
                let wait_time = timestamp.elapsed();
                LockGuard::new(guard, guard_kind, loc, guard_location, wait_time)
            } else {
                // Lock is contended, create WaitGuard and block
                let wait_guard = WaitGuard::new(guard_kind, loc, guard_location);
                let guard = lock_ref.lock().await;
                let wait_time = timestamp.elapsed();
                LockGuard::from_wait_guard(guard, wait_guard, wait_time)
            }
        }
    }

    #[track_caller]
    pub fn try_lock(&self) -> Result<LockGuard<MutexGuard<'_, T>>, TryLockError> {
        let guard_kind = GuardKind::Lock;
        let guard_location = call_location();
        #[cfg(feature = "tracing")]
        trace!(
            "Attempting to acquire a {:?} guard at {}",
            guard_kind,
            guard_location
        );
        let timestamp = Instant::now();
        let guard = self.lock.try_lock().inspect_err(|_e| {
            #[cfg(feature = "tracing")]
            trace!(
                "Failed to acquire a {:?} guard at {guard_location}: {_e}",
                guard_kind,
            );
        })?;
        let wait_time = timestamp.elapsed();
        Ok(LockGuard::new(
            guard,
            guard_kind,
            &self.location,
            guard_location,
            wait_time,
        ))
    }
}

impl<T: Default> Default for Mutex<T> {
    #[track_caller]
    fn default() -> Self {
        Self {
            lock: Default::default(),
            location: LockInfo::register(LockKind::Mutex),
        }
    }
}

#[derive(Debug)]
pub struct RwLock<T> {
    lock: tokio::sync::RwLock<T>,
    location: Location,
}

impl<T> RwLock<T> {
    #[track_caller]
    pub fn new(item: T) -> Self {
        Self {
            lock: tokio::sync::RwLock::new(item),
            location: LockInfo::register(LockKind::RwLock),
        }
    }

    #[track_caller]
    pub fn read(&self) -> impl Future<Output = LockGuard<RwLockReadGuard<'_, T>>> {
        let guard_kind = GuardKind::Read;
        let guard_location = call_location();

        let lock_ref = &self.lock;
        let loc = &self.location;
        async move {
            #[cfg(feature = "tracing")]
            trace!("Acquiring a {:?} guard at {}", guard_kind, guard_location);

            // Fast path -- try to acquire lock without blocking first
            let timestamp = Instant::now();
            if let Ok(guard) = lock_ref.try_read() {
                let wait_time = timestamp.elapsed();
                LockGuard::new(guard, guard_kind, loc, guard_location, wait_time)
            } else {
                // Lock is contended, create WaitGuard and block
                let wait_guard = WaitGuard::new(guard_kind, loc, guard_location);
                let guard = lock_ref.read().await;
                let wait_time = timestamp.elapsed();
                LockGuard::from_wait_guard(guard, wait_guard, wait_time)
            }
        }
    }

    #[track_caller]
    pub fn try_read(&self) -> Result<LockGuard<RwLockReadGuard<'_, T>>, TryLockError> {
        let guard_kind = GuardKind::Read;
        let guard_location = call_location();
        #[cfg(feature = "tracing")]
        trace!(
            "Attempting to acquire a {:?} guard at {}",
            guard_kind,
            guard_location
        );
        let timestamp = Instant::now();
        let guard = self.lock.try_read().inspect_err(|_e| {
            #[cfg(feature = "tracing")]
            trace!(
                "Failed to acquire a {:?} guard at {guard_location}: {_e}",
                guard_kind,
            );
        })?;
        let wait_time = timestamp.elapsed();
        Ok(LockGuard::new(
            guard,
            guard_kind,
            &self.location,
            guard_location,
            wait_time,
        ))
    }

    #[track_caller]
    pub fn write(&self) -> impl Future<Output = LockGuard<RwLockWriteGuard<'_, T>>> {
        let guard_kind = GuardKind::Write;
        let guard_location = call_location();

        let lock_ref = &self.lock;
        let loc = &self.location;
        async move {
            #[cfg(feature = "tracing")]
            trace!("Acquiring a {:?} guard at {}", guard_kind, guard_location);

            // Fast path -- try to acquire lock without blocking first
            let timestamp = Instant::now();
            if let Ok(guard) = lock_ref.try_write() {
                let wait_time = timestamp.elapsed();
                LockGuard::new(guard, guard_kind, loc, guard_location, wait_time)
            } else {
                // Lock is contended, create WaitGuard and block
                let wait_guard = WaitGuard::new(guard_kind, loc, guard_location);
                let guard = lock_ref.write().await;
                let wait_time = timestamp.elapsed();
                LockGuard::from_wait_guard(guard, wait_guard, wait_time)
            }
        }
    }

    #[track_caller]
    pub fn try_write(&self) -> Result<LockGuard<RwLockWriteGuard<'_, T>>, TryLockError> {
        let guard_kind = GuardKind::Write;
        let guard_location = call_location();
        #[cfg(feature = "tracing")]
        trace!(
            "Attempting to acquire a {:?} guard at {}",
            guard_kind,
            guard_location
        );
        let timestamp = Instant::now();
        let guard = self.lock.try_write().inspect_err(|_e| {
            #[cfg(feature = "tracing")]
            trace!(
                "Failed to acquire a {:?} guard at {guard_location}: {_e}",
                guard_kind,
            );
        })?;
        let wait_time = timestamp.elapsed();
        Ok(LockGuard::new(
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
    #[track_caller]
    fn default() -> Self {
        Self {
            lock: Default::default(),
            location: LockInfo::register(LockKind::RwLock),
        }
    }
}
