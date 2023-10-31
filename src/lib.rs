use std::{
    fmt::{self, Display},
    time::Instant,
};

use tracing::info;

pub struct LockTick {
    kind: LockKind,
    name: String,
    opened: Instant,
}

pub enum LockKind {
    Mutex,
    Read,
    Write,
}

impl fmt::Display for LockKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Mutex => write!(f, "mutex"),
            Self::Read => write!(f, "read"),
            Self::Write => write!(f, "write"),
        }
    }
}

impl LockTick {
    pub fn new<T: Display + Into<String>>(kind: LockKind, name: T) -> Self {
        info!("lock {} ({}) was opened", name, kind);
        Self {
            kind,
            name: name.into(),
            opened: Instant::now(),
        }
    }
}

impl Drop for LockTick {
    fn drop(&mut self) {
        info!(
            "lock {} ({}) was closed after {:?}",
            self.name,
            self.kind,
            self.opened.elapsed()
        );
    }
}
