mod common;

#[cfg(feature = "tokio")]
mod tests {
    use locktick::{clear_lock_infos, lock_snapshots, tokio::*};
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
}
