#[macro_export]
macro_rules! check_locks {
    ($lock_count:expr, $guard_count:expr, $active_guards:expr) => {{
        {
            let locks = lock_snapshots();
            assert_eq!($lock_count, locks.len());
            let guard_count = locks.iter().map(|l| l.known_guards.len()).sum::<usize>();
            assert_eq!($guard_count, guard_count);
            let active_guards = locks
                .iter()
                .map(|l| l.known_guards.values())
                .flatten()
                .map(|g| g.num_active_uses())
                .sum::<usize>();
            assert_eq!($active_guards, active_guards);
        }
    }};
}

#[macro_export]
macro_rules! check_guard {
    ($guard:expr, $uses:expr, $active:expr) => {{
        {
            let lock_location = &$guard.lock_location;
            let guard_location = &$guard.guard_location;
            assert_eq!(guard_location.line, line!() - 1);
            let locks = lock_snapshots();
            let lock = locks.iter().find(|l| l.location == *lock_location).unwrap();
            let guard = lock.known_guards.get(guard_location).unwrap();
            assert_eq!($uses, guard.num_uses);
            assert_eq!($active, guard.num_active_uses());
        }
    }};
}

#[allow(unused)]
pub struct Object;
