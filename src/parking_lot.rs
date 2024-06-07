use parking_lot::{MutexGuard, RwLockReadGuard, RwLockWriteGuard};

use crate::lock_info::{GuardKind, LockGuard, LockInfo, LockKind};

#[derive(Debug)]
pub struct Mutex<T> {
    lock: parking_lot::Mutex<T>,
    info: LockInfo,
}

impl<T> Mutex<T> {
    pub fn new(item: T) -> Self {
        Self {
            lock: parking_lot::Mutex::new(item),
            info: LockInfo::new(LockKind::Mutex),
        }
    }

    pub fn lock(&self) -> LockGuard<MutexGuard<'_, T>> {
        let guard = self.lock.lock();
        self.info.guard(guard, GuardKind::Lock)
    }

    pub fn try_lock(&self) -> Option<LockGuard<MutexGuard<'_, T>>> {
        let guard = self.lock.try_lock()?;
        Some(self.info.guard(guard, GuardKind::Lock))
    }
}

impl<T: Default> Default for Mutex<T> {
    fn default() -> Self {
        Self {
            lock: Default::default(),
            info: LockInfo::new(LockKind::Mutex),
        }
    }
}

#[derive(Debug)]
pub struct RwLock<T> {
    lock: parking_lot::RwLock<T>,
    info: LockInfo,
}

impl<T> RwLock<T> {
    pub fn new(item: T) -> Self {
        Self {
            lock: parking_lot::RwLock::new(item),
            info: LockInfo::new(LockKind::RwLock),
        }
    }

    pub fn read(&self) -> LockGuard<RwLockReadGuard<'_, T>> {
        let guard = self.lock.read();
        self.info.guard(guard, GuardKind::Read)
    }

    pub fn write(&self) -> LockGuard<RwLockWriteGuard<'_, T>> {
        let guard = self.lock.write();
        self.info.guard(guard, GuardKind::Write)
    }

    pub fn into_inner(self) -> T {
        self.lock.into_inner()
    }
}

impl<T: Default> Default for RwLock<T> {
    fn default() -> Self {
        Self {
            lock: Default::default(),
            info: LockInfo::new(LockKind::RwLock),
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
