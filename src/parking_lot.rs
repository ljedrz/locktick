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
    use crate::{lock_info::*, parking_lot::*};

    // FIXME: make locations work
    #[test]
    fn rwlock() {
        let obj = String::from("derp");
        let lock = RwLock::new(obj);

        let read1 = lock.read();
        {
            let infos = lock_snapshots();
            assert_eq!(infos.len(), 1);
            let info = &infos[0];
            assert_eq!(info.known_guards.len(), 1);
            let known_guard = info.known_guards.values().next().unwrap();
            assert_eq!(known_guard.num_uses, 1);
            assert_eq!(known_guard.active_uses.len(), 1);
        }

        let read2 = lock.read();
        {
            let infos = lock_snapshots();
            assert_eq!(infos.len(), 1);
            let info = &infos[0];
            assert_eq!(info.known_guards.len(), 2);
            for known_guard in info.known_guards.values() {
                assert_eq!(known_guard.num_uses, 1);
                assert_eq!(known_guard.active_uses.len(), 1);
            }
        }

        drop(read1);
        {
            let infos = lock_snapshots();
            assert_eq!(infos.len(), 1);
            let info = &infos[0];
            assert_eq!(info.known_guards.len(), 2);
            for known_guard in info.known_guards.values() {
                assert_eq!(known_guard.num_uses, 1);
                // TODO: check that only one is active now
            }
        }

        drop(read2);
        {
            let infos = lock_snapshots();
            assert_eq!(infos.len(), 1);
            let info = &infos[0];
            assert_eq!(info.known_guards.len(), 2);
            for known_guard in info.known_guards.values() {
                assert_eq!(known_guard.num_uses, 1);
                assert_eq!(known_guard.active_uses.len(), 0);
            }
        }

        let write = lock.write();
        {
            let infos = lock_snapshots();
            assert_eq!(infos.len(), 1);
            let info = &infos[0];
            assert_eq!(info.known_guards.len(), 3);
            for known_guard in info.known_guards.values() {
                assert_eq!(known_guard.num_uses, 1);
                // TODO: check that only one is active now
            }
        }

        drop(write);
        {
            let infos = lock_snapshots();
            assert_eq!(infos.len(), 1);
            let info = &infos[0];
            assert_eq!(info.known_guards.len(), 3);
            for known_guard in info.known_guards.values() {
                assert_eq!(known_guard.num_uses, 1);
                assert_eq!(known_guard.active_uses.len(), 0);
            }
        }
    }
}
