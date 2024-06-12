use std::time::Instant;

use std::sync::{MutexGuard, PoisonError, RwLockReadGuard, RwLockWriteGuard, TryLockError};

use crate::lock_info::{GuardKind, Location, LockGuard, LockInfo, LockKind};

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
        let timestamp = Instant::now();
        let guard = self.lock.lock()?;
        let wait_time = timestamp.elapsed();
        Ok(LockGuard::new(
            guard,
            GuardKind::Lock,
            &self.location,
            wait_time,
        ))
    }

    pub fn try_lock(&self) -> Result<LockGuard<MutexGuard<'_, T>>, TryLockError<MutexGuard<T>>> {
        let timestamp = Instant::now();
        let guard = self.lock.try_lock()?;
        let wait_time = timestamp.elapsed();
        Ok(LockGuard::new(
            guard,
            GuardKind::Lock,
            &self.location,
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
        let timestamp = Instant::now();
        let guard = self.lock.read()?;
        let wait_time = timestamp.elapsed();
        Ok(LockGuard::new(
            guard,
            GuardKind::Read,
            &self.location,
            wait_time,
        ))
    }

    pub fn write(
        &self,
    ) -> Result<LockGuard<RwLockWriteGuard<'_, T>>, PoisonError<RwLockWriteGuard<'_, T>>> {
        let timestamp = Instant::now();
        let guard = self.lock.write()?;
        let wait_time = timestamp.elapsed();
        Ok(LockGuard::new(
            guard,
            GuardKind::Write,
            &self.location,
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
