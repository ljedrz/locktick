mod common;

#[cfg(feature = "parking_lot")]
mod tests {
    use locktick::{clear_lock_infos, lock_snapshots, parking_lot::*, GuardKind};
    use serial_test::serial;

    use super::*;
    use common::*;

    #[test]
    #[serial]
    fn mutex() {
        clear_lock_infos();

        let lock1 = Mutex::new(Object);
        check_locks!(1, 0, 0);

        let lock2 = Mutex::new(Object);
        check_locks!(2, 0, 0);

        let guard1 = lock1.lock();
        check_guard!(guard1, 1, 1);
        check_locks!(2, 1, 1);

        let guard2 = lock2.lock();
        check_guard!(guard2, 1, 1);
        check_locks!(2, 2, 2);
    }

    #[test]
    #[serial]
    fn rwlock() {
        clear_lock_infos();

        let lock1 = RwLock::new(Object);
        check_locks!(1, 0, 0);

        let read1 = lock1.read();
        check_guard!(read1, 1, 1);

        let read2 = lock1.read();
        check_guard!(read2, 1, 1);

        drop(read1);
        check_locks!(1, 2, 1);

        drop(read2);
        check_locks!(1, 2, 0);

        let write = lock1.write();
        check_guard!(write, 1, 1);

        drop(write);
        check_locks!(1, 3, 0);

        let _lock2 = RwLock::new(Object);
        check_locks!(2, 3, 0);
    }

    #[test]
    #[serial]
    fn lock_locations() {
        clear_lock_infos();

        let lock = Mutex::new(Object);
        let lock_line = line!() - 1;
        let guard = lock.lock();
        check_lock_loc!(guard, lock_line);
        assert!(guard.guard_location.col > 0);
        assert_eq!(guard.lock_location.line, lock_line);
        drop(guard);

        let guard2 = lock.lock();
        assert_eq!(guard2.lock_location.line, lock_line);
    }

    #[test]
    #[serial]
    fn try_lock_location() {
        clear_lock_infos();

        let lock = Mutex::new(Object);
        let lock_line = line!() - 1;
        let guard = lock.try_lock().unwrap();
        check_guard!(guard, 1, 1);
        check_lock_loc!(guard, lock_line);
        assert!(guard.guard_location.col > 0);
    }

    #[test]
    #[serial]
    fn default_location() {
        clear_lock_infos();

        let lock: Mutex<Object> = Default::default();
        let lock_line = line!() - 1;
        let guard = lock.lock();
        check_lock_loc!(guard, lock_line);
    }

    #[test]
    #[serial]
    fn rwlock_guard_kinds() {
        clear_lock_infos();

        let lock = RwLock::new(Object);
        let _read = lock.read();
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

        let _write = lock.write();
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

    #[test]
    #[serial]
    fn distinct_lock_locations() {
        clear_lock_infos();

        let lock_a = Mutex::new(Object);
        let lock_a_line = line!() - 1;
        let lock_b = Mutex::new(Object);
        let lock_b_line = line!() - 1;

        let guard_a = lock_a.lock();
        let guard_b = lock_b.lock();

        assert_eq!(guard_a.lock_location.line, lock_a_line);
        assert_eq!(guard_b.lock_location.line, lock_b_line);
        assert_ne!(guard_a.lock_location, guard_b.lock_location);
    }

    #[test]
    #[serial]
    fn rwlock_read_locations() {
        clear_lock_infos();

        let lock = RwLock::new(Object);
        let lock_line = line!() - 1;
        let read = lock.read();
        check_guard!(read, 1, 1);
        check_lock_loc!(read, lock_line);
        assert!(read.guard_location.col > 0);
    }

    #[test]
    #[serial]
    fn rwlock_write_locations() {
        clear_lock_infos();

        let lock = RwLock::new(Object);
        let lock_line = line!() - 1;
        let write = lock.write();
        check_guard!(write, 1, 1);
        check_lock_loc!(write, lock_line);
        assert!(write.guard_location.col > 0);
    }

    #[test]
    #[serial]
    fn rwlock_try_read_location() {
        clear_lock_infos();

        let lock = RwLock::new(Object);
        let read = lock.try_read().unwrap();
        check_guard!(read, 1, 1);
    }

    #[test]
    #[serial]
    fn rwlock_try_write_location() {
        clear_lock_infos();

        let lock = RwLock::new(Object);
        let write = lock.try_write().unwrap();
        check_guard!(write, 1, 1);
    }

    #[test]
    #[serial]
    fn rwlock_default_location() {
        clear_lock_infos();

        let lock: RwLock<Object> = Default::default();
        let lock_line = line!() - 1;
        let read = lock.read();
        check_lock_loc!(read, lock_line);
    }

    #[test]
    #[serial]
    fn contended_mutex() {
        use std::sync::Arc;
        use std::thread;
        use std::time::Duration;

        clear_lock_infos();

        let lock = Arc::new(Mutex::new(0u32));
        let held = lock.lock();
        check_guard!(held, 1, 1);

        let lock_for_thread = Arc::clone(&lock);
        let handle = thread::spawn(move || {
            let _guard = lock_for_thread.lock();
        });

        thread::sleep(Duration::from_millis(100));

        let info = lock_snapshots().into_iter().next().unwrap();
        let held_guard_info = info.known_guards.get(&held.guard_location).unwrap();
        assert_eq!(held_guard_info.num_active_uses(), 1);
        assert_eq!(held_guard_info.num_waiting(), 0);
        assert!(held_guard_info.is_in_use());

        let waiting_guard_info = info
            .known_guards
            .values()
            .find(|g| g.num_waiting() == 1)
            .expect("a thread should be waiting on the lock");
        assert_eq!(waiting_guard_info.num_active_uses(), 0);
        assert_eq!(waiting_guard_info.num_waiting(), 1);
        let waiting = waiting_guard_info.waiting_call_indices();
        let active = held_guard_info.active_call_indices();
        assert_eq!(waiting.len(), 1);
        assert_eq!(active.len(), 1);
        assert!(waiting[0] > active[0]);

        drop(held);
        handle.join().unwrap();

        let info = lock_snapshots().into_iter().next().unwrap();
        assert_eq!(info.known_guards.len(), 2);
        for g in info.known_guards.values() {
            assert_eq!(g.num_active_uses(), 0);
            assert_eq!(g.num_waiting(), 0);
            assert!(!g.is_in_use());
        }
        let thread_guard_info = info
            .known_guards
            .values()
            .find(|g| g.max_wait_time > Duration::ZERO)
            .expect("the waiting thread's guard_info should have recorded a wait time");
        assert_eq!(thread_guard_info.num_uses, 1);
    }

    #[test]
    #[serial]
    fn try_lock_for_success() {
        use std::time::Duration;

        clear_lock_infos();
        let lock = Mutex::new(Object);
        let guard = lock.try_lock_for(Duration::from_secs(1)).unwrap();
        check_guard!(guard, 1, 1);
    }

    #[test]
    #[serial]
    fn try_lock_for_timeout() {
        use std::sync::Arc;
        use std::thread;
        use std::time::Duration;

        clear_lock_infos();
        let lock = Arc::new(Mutex::new(Object));
        let _held = lock.lock();
        let lock_clone = Arc::clone(&lock);
        let handle = thread::spawn(move || {
            assert!(lock_clone.try_lock_for(Duration::from_millis(50)).is_none());
        });
        handle.join().unwrap();
        drop(_held);
    }

    #[test]
    #[serial]
    fn try_read_for_success() {
        use std::time::Duration;

        clear_lock_infos();
        let lock = RwLock::new(Object);
        let guard = lock.try_read_for(Duration::from_secs(1)).unwrap();
        check_guard!(guard, 1, 1);
    }

    #[test]
    #[serial]
    fn try_read_for_timeout() {
        use std::sync::Arc;
        use std::thread;
        use std::time::Duration;

        clear_lock_infos();
        let lock = Arc::new(RwLock::new(Object));
        let _held = lock.write();
        let lock_clone = Arc::clone(&lock);
        let handle = thread::spawn(move || {
            assert!(lock_clone.try_read_for(Duration::from_millis(50)).is_none());
        });
        handle.join().unwrap();
        drop(_held);
    }

    #[test]
    #[serial]
    fn try_write_for_success() {
        use std::time::Duration;

        clear_lock_infos();
        let lock = RwLock::new(Object);
        let guard = lock.try_write_for(Duration::from_secs(1)).unwrap();
        check_guard!(guard, 1, 1);
    }

    #[test]
    #[serial]
    fn try_write_for_timeout() {
        use std::sync::Arc;
        use std::thread;
        use std::time::Duration;

        clear_lock_infos();
        let lock = Arc::new(RwLock::new(Object));
        let _held = lock.write();
        let lock_clone = Arc::clone(&lock);
        let handle = thread::spawn(move || {
            assert!(lock_clone.try_write_for(Duration::from_millis(50)).is_none());
        });
        handle.join().unwrap();
        drop(_held);
    }
}
