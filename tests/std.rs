#[cfg(feature = "std")]
mod tests {
    use locktick::{lock_info::*, std::*};

    #[test]
    fn rwlock() {
        let obj = String::from("derp");
        let lock = RwLock::new(obj);

        let read1 = lock.read().unwrap();
        assert_eq!(read1.guard_location.line, line!() - 1);
        {
            let infos = lock_snapshots();
            assert_eq!(infos.len(), 1);
            let info = &infos[0];
            assert_eq!(info.known_guards.len(), 1);
            let known_guard = info.known_guards.values().next().unwrap();
            assert_eq!(known_guard.num_uses, 1);
            assert_eq!(known_guard.num_active_uses(), 1);
        }

        let read2 = lock.read().unwrap();
        assert_eq!(read2.guard_location.line, line!() - 1);
        {
            let infos = lock_snapshots();
            assert_eq!(infos.len(), 1);
            let info = &infos[0];
            assert_eq!(info.known_guards.len(), 2);
            for known_guard in info.known_guards.values() {
                assert_eq!(known_guard.num_uses, 1);
                assert_eq!(known_guard.num_active_uses(), 1);
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
                assert_eq!(known_guard.num_active_uses(), 0);
            }
        }

        let write = lock.write().unwrap();
        assert_eq!(write.guard_location.line, line!() - 1);
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
                assert_eq!(known_guard.num_active_uses(), 0);
            }
        }
    }
}