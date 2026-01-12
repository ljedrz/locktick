# 0.5.0

### Added
- `WaitGuard`, providing the means to track the number of users waiting to acquire guards
- `GuardInfo::{is_in_use, num_waiting, waiting_call_indices}`

### Changed

- bumped the MSRV to 1.80 due to the use of `LazyLock`

# 0.4.0

### Added
- `LockGuard` is now re-exported

# 0.3.0

### Added
- `try_read` and `try_write` for all the supported crates

# 0.2.0

### Added
- `GuardInfo::{active_call_indices, max_duration, max_wait_time}`

### Changed
- simplified guard identifiers
- all the guards share a common counter now, so their call order can be deduced
- the `tracing` feature now also produces TRACE logs before creating a guard and if `try_lock` fails
