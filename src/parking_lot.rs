use std::sync::Arc;

use parking_lot::{MutexGuard, RwLockReadGuard, RwLockWriteGuard};

use crate::lock_info::{GuardKind, LockGuard, LockInfo, LockKind};

#[derive(Debug)]
pub struct Mutex<T> {
    lock: parking_lot::Mutex<T>,
    location: Arc<str>,
}

impl<T> Mutex<T> {
    pub fn new(item: T) -> Self {
        Self {
            lock: parking_lot::Mutex::new(item),
            location: LockInfo::register(LockKind::Mutex),
        }
    }

    pub fn lock(&self) -> LockGuard<MutexGuard<'_, T>> {
        let guard = self.lock.lock();
        LockGuard::new(guard, GuardKind::Lock, &self.location)
    }

    pub fn try_lock(&self) -> Option<LockGuard<MutexGuard<'_, T>>> {
        let guard = self.lock.try_lock()?;
        Some(LockGuard::new(guard, GuardKind::Lock, &self.location))
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
    location: Arc<str>,
}

impl<T> RwLock<T> {
    pub fn new(item: T) -> Self {
        Self {
            lock: parking_lot::RwLock::new(item),
            location: LockInfo::register(LockKind::RwLock),
        }
    }

    pub fn read(&self) -> LockGuard<RwLockReadGuard<'_, T>> {
        let guard = self.lock.read();
        LockGuard::new(guard, GuardKind::Read, &self.location)
    }

    pub fn write(&self) -> LockGuard<RwLockWriteGuard<'_, T>> {
        let guard = self.lock.write();
        LockGuard::new(guard, GuardKind::Write, &self.location)
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

#[cfg(test)]
mod tests {
    use std::{thread::sleep, time::Duration};

    use super::*;
    use crate::lock_info::{LockState, LOCK_INFOS};

    #[test]
    fn rwlock() {
        let obj = String::from("derp");
        let lock = RwLock::new(obj);
        assert_eq!(
            *LOCK_INFOS
                .get()
                .unwrap()
                .read()
                .unwrap()
                .get(lock.info.location)
                .unwrap()
                .state
                .lock()
                .unwrap(),
            LockState::Locked
        );
        let read1 = lock.read();
        assert_eq!(
            *LOCK_INFOS
                .get()
                .unwrap()
                .read()
                .unwrap()
                .get(lock.info.location)
                .unwrap()
                .state
                .lock()
                .unwrap(),
            LockState::Reading(1)
        );
        // TODO: check location
        sleep(Duration::from_millis(1));
        let read2 = lock.read();
        assert_eq!(
            *LOCK_INFOS
                .get()
                .unwrap()
                .read()
                .unwrap()
                .get(lock.info.location)
                .unwrap()
                .state
                .lock()
                .unwrap(),
            LockState::Reading(2)
        );
        // TODO: check location
        sleep(Duration::from_millis(1));

        drop(read1);
        assert_eq!(
            *LOCK_INFOS
                .get()
                .unwrap()
                .read()
                .unwrap()
                .get(lock.info.location)
                .unwrap()
                .state
                .lock()
                .unwrap(),
            LockState::Reading(1)
        );
        drop(read2);
        assert_eq!(
            *LOCK_INFOS
                .get()
                .unwrap()
                .read()
                .unwrap()
                .get(lock.info.location)
                .unwrap()
                .state
                .lock()
                .unwrap(),
            LockState::Locked
        );

        let write = lock.write();
        assert_eq!(
            *LOCK_INFOS
                .get()
                .unwrap()
                .read()
                .unwrap()
                .get(lock.info.location)
                .unwrap()
                .state
                .lock()
                .unwrap(),
            LockState::Writing
        );
        // TODO: check location
        drop(write);
        assert_eq!(
            *LOCK_INFOS
                .get()
                .unwrap()
                .read()
                .unwrap()
                .get(lock.info.location)
                .unwrap()
                .state
                .lock()
                .unwrap(),
            LockState::Locked
        );

        // TODO: check LOCK_INFOS
    }
}
