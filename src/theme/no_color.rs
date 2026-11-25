//! `NO_COLOR` handling, with a test-only lock around the env var.

use std::sync::{OnceLock, RwLock};

/// Guards `MANTIS_TEST_NO_COLOR`, which is process-global and read on every
/// call to [`no_color_active`] in test builds. Tests that flip the var must
/// hold the guard for their whole critical section (via
/// [`lock_no_color_test_env`]) so they don't leak a transient value into an
/// unrelated test's `no_color_active()` read running on another thread
/// (cargo test runs tests in parallel within one process).
static NO_COLOR_TEST_ENV_LOCK: RwLock<()> = RwLock::new(());

thread_local! {
    // Set while this thread holds `NO_COLOR_TEST_ENV_LOCK`'s write side, so
    // `no_color_active()` can tell it's being called re-entrantly by the
    // lock holder itself (e.g. a test exercising code that reads the var it
    // just set) and read the env var directly instead of taking the read
    // side — `RwLock` isn't reentrant, so doing otherwise self-deadlocks.
    static HOLDS_NO_COLOR_TEST_WRITE_LOCK: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// RAII guard returned by [`lock_no_color_test_env`]; see that function.
/// The held write guard is never read — only kept alive so its `Drop` runs
/// when the caller's guard goes out of scope.
#[cfg(test)]
pub struct NoColorTestEnvGuard(#[allow(dead_code)] std::sync::RwLockWriteGuard<'static, ()>);

#[cfg(test)]
impl Drop for NoColorTestEnvGuard {
    fn drop(&mut self) {
        HOLDS_NO_COLOR_TEST_WRITE_LOCK.with(|c| c.set(false));
    }
}

/// Acquires exclusive access to `MANTIS_TEST_NO_COLOR` for a test's whole
/// critical section (set the var, exercise code, assert, unset). Hold the
/// returned guard for as long as the var must keep its test-specific value,
/// so no concurrently-running test's `no_color_active()` call observes it
/// transiently.
///
/// Only callable from unit tests (`#[cfg(test)]`): the only callers are the
/// `theme_test.rs`/`highlight_test.rs` modules compiled into this crate.
#[cfg(test)]
pub fn lock_no_color_test_env() -> NoColorTestEnvGuard {
    let guard = NO_COLOR_TEST_ENV_LOCK
        .write()
        .unwrap_or_else(|e| e.into_inner());
    HOLDS_NO_COLOR_TEST_WRITE_LOCK.with(|c| c.set(true));
    NoColorTestEnvGuard(guard)
}

pub fn no_color_active() -> bool {
    static IS_TEST_ENV: OnceLock<bool> = OnceLock::new();
    let is_test_env = *IS_TEST_ENV.get_or_init(|| {
        cfg!(test)
            || std::env::current_exe()
                .map(|p| {
                    p.parent()
                        .and_then(|parent| parent.file_name())
                        .map(|name| name == "deps")
                        .unwrap_or(false)
                })
                .unwrap_or(false)
    });
    if is_test_env {
        if HOLDS_NO_COLOR_TEST_WRITE_LOCK.with(|c| c.get()) {
            std::env::var_os("MANTIS_TEST_NO_COLOR").is_some()
        } else {
            let _guard = NO_COLOR_TEST_ENV_LOCK
                .read()
                .unwrap_or_else(|e| e.into_inner());
            std::env::var_os("MANTIS_TEST_NO_COLOR").is_some()
        }
    } else {
        std::env::var_os("NO_COLOR").is_some() || std::env::var("TERM").as_deref() == Ok("dumb")
    }
}
