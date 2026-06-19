mod common;

#[cfg(feature = "tokio")]
mod tests {
    use locktick::{clear_lock_infos, lock_snapshots, tokio::*, GuardKind};
    use serial_test::serial;

    use super::*;
    use common::*;

    #[tokio::test]
    #[serial]
    async fn mutex() {
        clear_lock_infos();

        let lock1 = Mutex::new(Object);
        check_locks!(1, 0, 0);

        let lock2 = Mutex::new(Object);
        check_locks!(2, 0, 0);

        let guard1 = lock1.lock().await;
        check_guard!(guard1, 1, 1);
        check_locks!(2, 1, 1);

        let guard2 = lock2.lock().await;
        check_guard!(guard2, 1, 1);
        check_locks!(2, 2, 2);
    }

    #[tokio::test]
    #[serial]
    async fn rwlock() {
        clear_lock_infos();

        let lock1 = RwLock::new(Object);
        check_locks!(1, 0, 0);

        let read1 = lock1.read().await;
        check_guard!(read1, 1, 1);

        let read2 = lock1.read().await;
        check_guard!(read2, 1, 1);

        drop(read1);
        check_locks!(1, 2, 1);

        drop(read2);
        check_locks!(1, 2, 0);

        let write = lock1.write().await;
        check_guard!(write, 1, 1);

        drop(write);
        check_locks!(1, 3, 0);

        let _lock2 = RwLock::new(Object);
        check_locks!(2, 3, 0);
    }

    #[tokio::test]
    #[serial]
    async fn lock_locations() {
        clear_lock_infos();

        let lock = Mutex::new(Object);
        let lock_line = line!() - 1;
        let guard = lock.lock().await;
        check_lock_loc!(guard, lock_line);
        assert!(guard.guard_location.col > 0);
        assert_eq!(guard.lock_location.line, lock_line);
        drop(guard);

        let guard2 = lock.lock().await;
        assert_eq!(guard2.lock_location.line, lock_line);
    }

    #[tokio::test]
    #[serial]
    async fn try_lock_location() {
        clear_lock_infos();

        let lock = Mutex::new(Object);
        let lock_line = line!() - 1;
        let guard = lock.try_lock().unwrap();
        check_guard!(guard, 1, 1);
        check_lock_loc!(guard, lock_line);
        assert!(guard.guard_location.col > 0);
    }

    #[tokio::test]
    #[serial]
    async fn default_location() {
        clear_lock_infos();

        let lock: Mutex<Object> = Default::default();
        let lock_line = line!() - 1;
        let guard = lock.lock().await;
        check_lock_loc!(guard, lock_line);
    }

    #[tokio::test]
    #[serial]
    async fn rwlock_guard_kinds() {
        clear_lock_infos();

        let lock = RwLock::new(Object);
        let _read = lock.read().await;
        let locks = lock_snapshots();
        let lock_info = locks
            .iter()
            .find(|l| l.kind == locktick::LockKind::RwLock)
            .unwrap();
        let read_guard = lock_info
            .known_guards
            .values()
            .find(|g| g.kind == GuardKind::Read)
            .unwrap();
        assert_eq!(read_guard.kind, GuardKind::Read);
        drop(_read);

        let _write = lock.write().await;
        let locks = lock_snapshots();
        let lock_info = locks
            .iter()
            .find(|l| l.kind == locktick::LockKind::RwLock)
            .unwrap();
        let write_guard = lock_info
            .known_guards
            .values()
            .find(|g| g.kind == GuardKind::Write)
            .unwrap();
        assert_eq!(write_guard.kind, GuardKind::Write);
    }

    #[tokio::test]
    #[serial]
    async fn distinct_lock_locations() {
        clear_lock_infos();

        let lock_a = Mutex::new(Object);
        let lock_a_line = line!() - 1;
        let lock_b = Mutex::new(Object);
        let lock_b_line = line!() - 1;

        let guard_a = lock_a.lock().await;
        let guard_b = lock_b.lock().await;

        assert_eq!(guard_a.lock_location.line, lock_a_line);
        assert_eq!(guard_b.lock_location.line, lock_b_line);
        assert_ne!(guard_a.lock_location, guard_b.lock_location);
    }

    #[tokio::test]
    #[serial]
    async fn rwlock_read_locations() {
        clear_lock_infos();

        let lock = RwLock::new(Object);
        let lock_line = line!() - 1;
        let read = lock.read().await;
        check_guard!(read, 1, 1);
        check_lock_loc!(read, lock_line);
        assert!(read.guard_location.col > 0);
    }

    #[tokio::test]
    #[serial]
    async fn rwlock_write_locations() {
        clear_lock_infos();

        let lock = RwLock::new(Object);
        let lock_line = line!() - 1;
        let write = lock.write().await;
        check_guard!(write, 1, 1);
        check_lock_loc!(write, lock_line);
        assert!(write.guard_location.col > 0);
    }

    #[tokio::test]
    #[serial]
    async fn rwlock_try_read_location() {
        clear_lock_infos();

        let lock = RwLock::new(Object);
        let read = lock.try_read().unwrap();
        check_guard!(read, 1, 1);
    }

    #[tokio::test]
    #[serial]
    async fn rwlock_try_write_location() {
        clear_lock_infos();

        let lock = RwLock::new(Object);
        let write = lock.try_write().unwrap();
        check_guard!(write, 1, 1);
    }

    #[tokio::test]
    #[serial]
    async fn rwlock_default_location() {
        clear_lock_infos();

        let lock: RwLock<Object> = Default::default();
        let lock_line = line!() - 1;
        let read = lock.read().await;
        check_lock_loc!(read, lock_line);
    }
}
