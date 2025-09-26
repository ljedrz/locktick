# locktick
[![crates.io](https://img.shields.io/crates/v/locktick)](https://crates.io/crates/locktick)
[![docs.rs](https://docs.rs/locktick/badge.svg)](https://docs.rs/locktick)
[![actively developed](https://img.shields.io/badge/maintenance-actively--developed-brightgreen.svg)](https://gist.github.com/cheerfulstoic/d107229326a01ff0f333a1d3476e068d)

**locktick** provides the means to measure things like the average guard duration and wait time for different kinds of locks, just by substituting the applicable locks' imports.
It makes the detection of deadlocks trivial, and can point to all the locks involved.

## Example use
```rust
use std::{thread, time::Duration};

use locktick::lock_snapshots;

fn print_active_guards() {
    thread::spawn(|| {
        loop {
            let mut locks = lock_snapshots();
            locks.sort_unstable_by(|l1, l2| l1.location.cmp(&l2.location));

            // Check if any of the known guards are currently active.
            if locks.iter().flat_map(|lock| lock.known_guards.values()).all(|g| g.num_active_uses() == 0) {
                // It's possible that the program is halted for reasons different than a deadlock; print
                // something in order to ensure that the lock accounting thread is operational at all times.
                println!("there are no active guards");
            } else {
                println!("\nthere are active guards:");

                // Traverse all the known locks.
                for lock in locks {
                    // Collec the data on all the known guards of the given lock.
                    let mut active_guards = lock.known_guards.values().filter(|g| g.num_active_uses() != 0).collect::<Vec<_>>();
                    // Skip a lock if none of its known guards are currently active.
                    if active_guards.is_empty() {
                        continue;
                    }
                    active_guards.sort_unstable_by(|g1, g2| g1.location.cmp(&g2.location));

                    println!("{}:", lock.location);
                    // Traverse the known guards of the given lock.
                    for guard in &active_guards {
                        let location = &guard.location;
                        let kind = guard.kind;
                        let num_uses = guard.num_uses;
                        let avg_duration = guard.avg_duration();
                        let avg_wait_time = guard.avg_wait_time();
                        // Log the desired details.
                        println!(
                            "- {location} ({:?}): used {num_uses} time(s) so far; avg duration: {:?}; avg wait: {:?}",
                            kind, avg_duration, avg_wait_time
                        );
                    }
                }
            }
            // Check for active guards every 1 second.
            thread::sleep(Duration::from_secs(1));
        }
    });
}
```

## status

- the basic functionalities are complete
- API breakage can still happen
- some of the TODOs include additional tests, examples, and improvements to documentation
