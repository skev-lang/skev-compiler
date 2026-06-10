//! Runtime lifecycle — Phase E Decision 9.
//!
//! Four-state machine: UNINIT → INITING → READY → SHUTDOWN.
//! Idempotent: only the CAS winner runs the init/shutdown body;
//! every other caller returns immediately.
//!
//! `ensure_init()` is called from `arc.rs::skev_alloc`'s hot path —
//! Relaxed load + predicted-not-taken branch makes it ~1 cycle once
//! the runtime is READY.

use core::sync::atomic::{AtomicI32, AtomicU8, Ordering};

pub const STATE_UNINIT: u8 = 0;
pub const STATE_INITING: u8 = 1;
pub const STATE_READY: u8 = 2;
pub const STATE_SHUTDOWN: u8 = 3;

static LIFECYCLE_STATE: AtomicU8 = AtomicU8::new(STATE_UNINIT);

/// Process exit code set by `skev_shutdown` when leaks are detected
/// under `--features leak-check`. 0 by default, 2 on leak detection.
/// Returned by `leak_exit_code()` for use by the generated `main()`.
static LEAK_EXIT_CODE: AtomicI32 = AtomicI32::new(0);

/// Initialise the runtime. Idempotent — the first call wins the CAS
/// and runs the init body; subsequent calls return immediately.
/// Called automatically by `skev_alloc` via `ensure_init` for FFI
/// safety (a C++ static initialiser that allocates a Skev type
/// before `main` runs still works).
#[unsafe(no_mangle)]
pub extern "C" fn skev_init() {
    if LIFECYCLE_STATE
        .compare_exchange(
            STATE_UNINIT,
            STATE_INITING,
            Ordering::AcqRel,
            Ordering::Relaxed,
        )
        .is_err()
    {
        return; // already initing / ready / shutdown
    }

    // Steps 2–4 (allocator / weak side-table / leak tracker) all
    // self-initialise lazily; nothing to do here.
    install_panic_sink();
    init_main_thread_tls();

    LIFECYCLE_STATE.store(STATE_READY, Ordering::Release);
}

/// Tear down the runtime. Idempotent — the first call wins the CAS
/// and runs the shutdown body; subsequent calls return immediately.
/// After shutdown, `skev_init` is a no-op (the state is one-way).
#[unsafe(no_mangle)]
pub extern "C" fn skev_shutdown() {
    if LIFECYCLE_STATE
        .compare_exchange(
            STATE_READY,
            STATE_SHUTDOWN,
            Ordering::AcqRel,
            Ordering::Relaxed,
        )
        .is_err()
    {
        return;
    }

    drain_tasks();

    #[cfg(feature = "leak-check")]
    {
        let count = crate::leak::report_leaks();
        if count > 0 {
            LEAK_EXIT_CODE.store(2, Ordering::Release);
        }
    }

    teardown_tls();
}

/// Hot-path guard called from `arc.rs::skev_alloc`. Triggers
/// `skev_init` if the runtime hasn't been initialised yet. Once
/// state is READY (or SHUTDOWN), this short-circuits via a single
/// Relaxed load.
#[inline(always)]
pub fn ensure_init() {
    if LIFECYCLE_STATE.load(Ordering::Relaxed) < STATE_READY {
        skev_init();
    }
}

/// Returns 2 if the runtime detected leaks during shutdown
/// (under `--features leak-check`), 0 otherwise.
/// Used by the codegen-emitted `main()` to produce the process
/// exit code.
pub fn leak_exit_code() -> i32 {
    LEAK_EXIT_CODE.load(Ordering::Acquire)
}

// ---------------------------------------------------------------
// Stubs — Phase F / v1.x fills these in.
// ---------------------------------------------------------------

/// v1.0 stub. D9 calls for a SIGABRT handler (Unix) /
/// SetUnhandledExceptionFilter (Windows). Returning from a SIGABRT
/// handler is C-level UB and the Windows API requires the `winapi`
/// crate (not in our libc-only dep list). For v1.0 the OS default
/// reporters (ReportCrash / WER / systemd-coredump per D10) handle
/// abort() correctly — installing nothing satisfies D9's "be a good
/// citizen, don't fight other libraries" clause. v1.x revisits once
/// Chapter 11's skev.log machinery is online.
fn install_panic_sink() {}

/// v1.0 stub — thread-local task_id / realtime_flag / alloc-context
/// initialise to their defaults on first access. Phase F's task
/// system fills these in.
fn init_main_thread_tls() {}

/// v1.0 stub — D9 specifies a 100ms drain to let outstanding tasks
/// finish. There are no tasks in v1.0; Phase F's runtime wires this
/// up with the actual task pool.
fn drain_tasks() {}

/// v1.0 stub — TLS teardown. Best-effort per D9. Phase F adds the
/// real cleanup once there's per-thread state to clean.
fn teardown_tls() {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    // Tests touch process-wide LIFECYCLE_STATE / LEAK_EXIT_CODE — serialise.
    static TEST_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn ensure_init_when_uninit_transitions_to_ready() {
        let _g = TEST_LOCK.lock().unwrap();
        let save = LIFECYCLE_STATE.swap(STATE_UNINIT, Ordering::AcqRel);
        ensure_init();
        assert_eq!(LIFECYCLE_STATE.load(Ordering::Acquire), STATE_READY);
        LIFECYCLE_STATE.store(save, Ordering::Release);
    }

    #[test]
    fn skev_init_when_ready_is_idempotent() {
        let _g = TEST_LOCK.lock().unwrap();
        let save = LIFECYCLE_STATE.swap(STATE_READY, Ordering::AcqRel);
        skev_init();
        assert_eq!(LIFECYCLE_STATE.load(Ordering::Acquire), STATE_READY);
        LIFECYCLE_STATE.store(save, Ordering::Release);
    }

    #[test]
    fn skev_shutdown_transitions_then_is_idempotent() {
        let _g = TEST_LOCK.lock().unwrap();
        let save_state = LIFECYCLE_STATE.swap(STATE_READY, Ordering::AcqRel);
        let save_exit = LEAK_EXIT_CODE.load(Ordering::Acquire);
        skev_shutdown();
        assert_eq!(LIFECYCLE_STATE.load(Ordering::Acquire), STATE_SHUTDOWN);
        skev_shutdown(); // second call — should be no-op
        assert_eq!(LIFECYCLE_STATE.load(Ordering::Acquire), STATE_SHUTDOWN);
        LIFECYCLE_STATE.store(save_state, Ordering::Release);
        LEAK_EXIT_CODE.store(save_exit, Ordering::Release);
    }

    #[test]
    fn skev_init_after_shutdown_is_noop() {
        let _g = TEST_LOCK.lock().unwrap();
        let save = LIFECYCLE_STATE.swap(STATE_SHUTDOWN, Ordering::AcqRel);
        skev_init();
        assert_eq!(LIFECYCLE_STATE.load(Ordering::Acquire), STATE_SHUTDOWN);
        LIFECYCLE_STATE.store(save, Ordering::Release);
    }

    #[test]
    fn leak_exit_code_default_zero_and_settable_to_two() {
        let _g = TEST_LOCK.lock().unwrap();
        let save = LEAK_EXIT_CODE.swap(0, Ordering::AcqRel);
        assert_eq!(leak_exit_code(), 0);
        LEAK_EXIT_CODE.store(2, Ordering::Release);
        assert_eq!(leak_exit_code(), 2);
        LEAK_EXIT_CODE.store(save, Ordering::Release);
    }
}
