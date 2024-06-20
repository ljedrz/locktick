# 0.2.0

### Added
- `GuardInfo::{active_call_indices, max_duration, max_wait_time}`

### Changed
- simplified guard identifiers
- all the guards share a common counter now, so their call order can be deduced
- the `tracing` feature now also produces TRACE logs before creating a guard and if `try_lock` fails
