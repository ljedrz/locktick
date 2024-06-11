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
    use std::{collections::HashMap, sync::Arc};

    use crate::{lock_info::*, parking_lot::*};

    fn lock_infos(
    ) -> std::sync::RwLockReadGuard<'static, HashMap<Arc<str>, std::sync::Mutex<LockInfo>>> {
        LOCK_INFOS.get().unwrap().read().unwrap()
    }

    #[test]
    fn rwlock() {
        let obj = String::from("derp");
        let lock = RwLock::new(obj);

        let read1 = lock.read();
        {
            assert_eq!(read1.lock_location, lock.location);
            let infos = lock_infos();
            assert_eq!(infos.len(), 1);
            let info = infos.values().next().unwrap().lock().unwrap();
            assert_eq!(
                info.accesses,
                Accesses::RwLock {
                    reads: 1,
                    writes: 0
                }
            );
            let guards = &info.guards;
            assert_eq!(guards.len(), 1);
            let _guard = guards.get(&read1.id).unwrap();
            // assert_ne!(guard.acquire_location, lock.location);
        }

        let read2 = lock.read();
        {
            assert_eq!(read2.lock_location, lock.location);
            let infos = lock_infos();
            assert_eq!(infos.len(), 1);
            let info = infos.values().next().unwrap().lock().unwrap();
            assert_eq!(
                info.accesses,
                Accesses::RwLock {
                    reads: 2,
                    writes: 0
                }
            );
            let guards = &info.guards;
            assert_eq!(guards.len(), 2);
            let _guard = guards.get(&read1.id).unwrap();
            // assert_ne!(guard.acquire_location, lock.location);
        }

        drop(read1);
        {
            let infos = lock_infos();
            assert_eq!(infos.len(), 1);
            let info = infos.values().next().unwrap().lock().unwrap();
            assert_eq!(
                info.accesses,
                Accesses::RwLock {
                    reads: 2,
                    writes: 0
                }
            );
            let guards = &info.guards;
            assert_eq!(guards.len(), 1);
        }

        drop(read2);
        {
            let infos = lock_infos();
            assert_eq!(infos.len(), 1);
            let info = infos.values().next().unwrap().lock().unwrap();
            assert_eq!(
                info.accesses,
                Accesses::RwLock {
                    reads: 2,
                    writes: 0
                }
            );
            let guards = &info.guards;
            assert_eq!(guards.len(), 0);
        }

        let write = lock.write();
        {
            assert_eq!(write.lock_location, lock.location);
            let infos = lock_infos();
            assert_eq!(infos.len(), 1);
            let info = infos.values().next().unwrap().lock().unwrap();
            assert_eq!(
                info.accesses,
                Accesses::RwLock {
                    reads: 2,
                    writes: 1
                }
            );
            let guards = &info.guards;
            assert_eq!(guards.len(), 1);
            let _guard = guards.get(&write.id).unwrap();
            // assert_ne!(guard.acquire_location, lock.location);
        }

        drop(write);
        {
            let infos = lock_infos();
            assert_eq!(infos.len(), 1);
            let info = infos.values().next().unwrap().lock().unwrap();
            assert_eq!(
                info.accesses,
                Accesses::RwLock {
                    reads: 2,
                    writes: 1
                }
            );
            let guards = &info.guards;
            assert_eq!(guards.len(), 0);
        }
    }
}
