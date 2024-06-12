use std::time::Instant;

use tokio::sync::{MutexGuard, RwLockReadGuard, RwLockWriteGuard, TryLockError};

use crate::lock_info::{GuardKind, Location, LockGuard, LockInfo, LockKind};

#[derive(Debug)]
pub struct Mutex<T> {
    lock: tokio::sync::Mutex<T>,
    location: Location,
}

impl<T> Mutex<T> {
    pub fn new(item: T) -> Self {
        Self {
            lock: tokio::sync::Mutex::new(item),
            location: LockInfo::register(LockKind::Mutex),
        }
    }

    pub async fn lock(&self) -> LockGuard<MutexGuard<'_, T>> {
        let timestamp = Instant::now();
        let guard = self.lock.lock().await;
        let wait_time = timestamp.elapsed();
        LockGuard::new(guard, GuardKind::Lock, &self.location, wait_time)
    }

    pub fn try_lock(&self) -> Result<LockGuard<MutexGuard<'_, T>>, TryLockError> {
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
    lock: tokio::sync::RwLock<T>,
    location: Location,
}

impl<T> RwLock<T> {
    pub fn new(item: T) -> Self {
        Self {
            lock: tokio::sync::RwLock::new(item),
            location: LockInfo::register(LockKind::RwLock),
        }
    }

    pub async fn read(&self) -> LockGuard<RwLockReadGuard<'_, T>> {
        let timestamp = Instant::now();
        let guard = self.lock.read().await;
        let wait_time = timestamp.elapsed();
        LockGuard::new(guard, GuardKind::Read, &self.location, wait_time)
    }

    pub async fn write(&self) -> LockGuard<RwLockWriteGuard<'_, T>> {
        let timestamp = Instant::now();
        let guard = self.lock.write().await;
        let wait_time = timestamp.elapsed();
        LockGuard::new(guard, GuardKind::Write, &self.location, wait_time)
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
