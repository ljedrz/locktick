use std::{
    sync::{MutexGuard, PoisonError, RwLockReadGuard, RwLockWriteGuard, TryLockError},
    time::Instant,
};

#[cfg(feature = "tracing")]
use tracing::trace;

use crate::lock_info::{
    call_location, GuardKind, Location, LockGuard, LockInfo, LockKind, WaitGuard,
};

#[derive(Debug)]
pub struct Mutex<T> {
    lock: std::sync::Mutex<T>,
    location: Location,
}

impl<T> Mutex<T> {
    pub fn new(item: T) -> Self {
        Self {
            lock: std::sync::Mutex::new(item),
            location: LockInfo::register(LockKind::Mutex),
        }
    }

    pub fn lock(&self) -> Result<LockGuard<MutexGuard<'_, T>>, PoisonError<MutexGuard<'_, T>>> {
        let guard_kind = GuardKind::Lock;
        let guard_location = call_location();
        #[cfg(feature = "tracing")]
        trace!("Acquiring a {:?} guard at {}", guard_kind, guard_location);

        // Fast path -- try to acquire lock without blocking first
        let timestamp = Instant::now();
        match self.lock.try_lock() {
            Ok(guard) => {
                let wait_time = timestamp.elapsed();
                return Ok(LockGuard::new(
                    guard,
                    guard_kind,
                    &self.location,
                    guard_location,
                    wait_time,
                ));
            }
            Err(std::sync::TryLockError::Poisoned(e)) => {
                return Err(PoisonError::new(e.into_inner()));
            }
            Err(std::sync::TryLockError::WouldBlock) => {
                // Lock is contended, fall through to blocking path
            }
        }

        // Lock is contended, create WaitGuard and block
        let wait_guard = WaitGuard::new(guard_kind, &self.location, guard_location);
        let guard = self.lock.lock()?;
        let wait_time = timestamp.elapsed();
        Ok(LockGuard::from_wait_guard(guard, wait_guard, wait_time))
    }

    pub fn try_lock(
        &self,
    ) -> Result<LockGuard<MutexGuard<'_, T>>, TryLockError<MutexGuard<'_, T>>> {
        let guard_kind = GuardKind::Lock;
        let guard_location = call_location();
        #[cfg(feature = "tracing")]
        trace!(
            "Attempting to acquire a {:?} guard at {}",
            guard_kind,
            self.location
        );
        let timestamp = Instant::now();
        #[allow(clippy::map_identity)]
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
    fn default() -> Self {
        Self {
            lock: Default::default(),
            location: LockInfo::register(LockKind::Mutex),
        }
    }
}

#[derive(Debug)]
pub struct RwLock<T> {
    lock: std::sync::RwLock<T>,
    location: Location,
}

impl<T> RwLock<T> {
    pub fn new(item: T) -> Self {
        Self {
            lock: std::sync::RwLock::new(item),
            location: LockInfo::register(LockKind::RwLock),
        }
    }

    pub fn read(
        &self,
    ) -> Result<LockGuard<RwLockReadGuard<'_, T>>, PoisonError<RwLockReadGuard<'_, T>>> {
        let guard_kind = GuardKind::Read;
        let guard_location = call_location();
        #[cfg(feature = "tracing")]
        trace!("Acquiring a {:?} guard at {}", guard_kind, guard_location);

        // Fast path -- try to acquire lock without blocking first
        let timestamp = Instant::now();
        match self.lock.try_read() {
            Ok(guard) => {
                let wait_time = timestamp.elapsed();
                return Ok(LockGuard::new(
                    guard,
                    guard_kind,
                    &self.location,
                    guard_location,
                    wait_time,
                ));
            }
            Err(std::sync::TryLockError::Poisoned(e)) => {
                return Err(PoisonError::new(e.into_inner()));
            }
            Err(std::sync::TryLockError::WouldBlock) => {
                // Lock is contended, fall through to blocking path
            }
        }

        // Lock is contended, create WaitGuard and block
        let wait_guard = WaitGuard::new(guard_kind, &self.location, guard_location);
        let guard = self.lock.read()?;
        let wait_time = timestamp.elapsed();
        Ok(LockGuard::from_wait_guard(guard, wait_guard, wait_time))
    }

    pub fn try_read(
        &self,
    ) -> Result<LockGuard<RwLockReadGuard<'_, T>>, TryLockError<RwLockReadGuard<'_, T>>> {
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

    pub fn write(
        &self,
    ) -> Result<LockGuard<RwLockWriteGuard<'_, T>>, PoisonError<RwLockWriteGuard<'_, T>>> {
        let guard_kind = GuardKind::Write;
        let guard_location = call_location();
        #[cfg(feature = "tracing")]
        trace!("Acquiring a {:?} guard at {}", guard_kind, guard_location);

        // Fast path -- try to acquire lock without blocking first
        let timestamp = Instant::now();
        match self.lock.try_write() {
            Ok(guard) => {
                let wait_time = timestamp.elapsed();
                return Ok(LockGuard::new(
                    guard,
                    guard_kind,
                    &self.location,
                    guard_location,
                    wait_time,
                ));
            }
            Err(std::sync::TryLockError::Poisoned(e)) => {
                return Err(PoisonError::new(e.into_inner()));
            }
            Err(std::sync::TryLockError::WouldBlock) => {
                // Lock is contended, fall through to blocking path
            }
        }

        // Lock is contended, create WaitGuard and block
        let wait_guard = WaitGuard::new(guard_kind, &self.location, guard_location);
        let guard = self.lock.write()?;
        let wait_time = timestamp.elapsed();
        Ok(LockGuard::from_wait_guard(guard, wait_guard, wait_time))
    }

    pub fn try_write(
        &self,
    ) -> Result<LockGuard<RwLockWriteGuard<'_, T>>, TryLockError<RwLockWriteGuard<'_, T>>> {
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
}

impl<T: Default> Default for RwLock<T> {
    fn default() -> Self {
        Self {
            lock: Default::default(),
            location: LockInfo::register(LockKind::RwLock),
        }
    }
}
