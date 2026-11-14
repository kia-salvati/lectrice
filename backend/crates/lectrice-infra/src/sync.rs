//! The one poison-tolerant lock of this crate.

use std::sync::{Mutex, MutexGuard, PoisonError};

/// Locks `mutex`, taking the data even if a holder panicked: nothing here panics halfway through changing shared state.
pub(crate) fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}
