//! End-to-end tests for the public C-ABI surface (Phase E, Step 10).
//!
//! These link skev-runtime through its public API only — the same
//! surface a C consumer / the codegen-emitted binary sees.
//!
//! The runtime lifecycle is ONE-WAY and PROCESS-GLOBAL
//! (UNINIT→INITING→READY→SHUTDOWN, SHUTDOWN terminal). These tests
//! share one process, so we serialize with TEST_LOCK and each test
//! releases everything it allocates before returning. skev_alloc keeps
//! working after shutdown, so the repeated init/shutdown calls are
//! harmless; only the first shutdown runs the (empty) leak report.

use std::sync::Mutex;
use skev_runtime::*;

static TEST_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn entity_lifecycle() {
    let _g = TEST_LOCK.lock().unwrap();
    skev_init();
    let p = unsafe { skev_alloc(24, 1) }; // entity header = 24 B
    assert!(!p.is_null());
    unsafe { skev_retain(p) };  // rc = 2
    unsafe { skev_release(p) }; // rc = 1
    unsafe { skev_release(p) }; // rc = 0 → dealloc
    skev_shutdown();
}

#[test]
fn plain_data_lifecycle() {
    let _g = TEST_LOCK.lock().unwrap();
    skev_init();
    let p = unsafe { skev_alloc(8, 2) }; // plain-data header = 8 B
    assert!(!p.is_null());
    unsafe { skev_release(p) }; // rc = 0 → dealloc
    skev_shutdown();
}

#[test]
fn weak_upgrade_after_dealloc() {
    let _g = TEST_LOCK.lock().unwrap();
    skev_init();
    let p = unsafe { skev_alloc(8, 3) };
    assert!(!p.is_null());
    let w = unsafe { skev_weak_alloc(p) };
    assert!(!w.is_null());
    assert_eq!(unsafe { skev_weak_upgrade(w) }, p); // alive → target
    unsafe { skev_release(p) };                     // strong dies → on_dealloc
    assert!(unsafe { skev_weak_upgrade(w) }.is_null()); // dead → NULL
    unsafe { skev_weak_release(w) };
    skev_shutdown();
}

#[test]
fn null_safety() {
    let _g = TEST_LOCK.lock().unwrap();
    skev_init();
    let n = std::ptr::null_mut();
    unsafe { skev_retain(n) };
    unsafe { skev_release(n) };
    unsafe { skev_dealloc(n) };
    assert!(unsafe { skev_weak_alloc(n) }.is_null());
    assert!(unsafe { skev_weak_upgrade(std::ptr::null_mut()) }.is_null());
    unsafe { skev_weak_release(std::ptr::null_mut()) };
    skev_shutdown();
}

#[test]
fn concurrent_retain_release() {
    let _g = TEST_LOCK.lock().unwrap();
    skev_init();
    let p = unsafe { skev_alloc(8, 4) };
    assert!(!p.is_null());
    let addr = p as usize; // move the address, not the !Send raw ptr
    let handles: Vec<_> = (0..4)
        .map(|_| std::thread::spawn(move || {
            let p = addr as *mut u8;
            for _ in 0..10_000 {
                unsafe { skev_retain(p) };
                unsafe { skev_release(p) };
            }
        }))
        .collect();
    for h in handles { h.join().unwrap(); }
    unsafe { skev_release(p) }; // rc 1 → 0 → dealloc
    skev_shutdown();
}

// Test 6: real leak, detected across a process boundary.
// Re-execs THIS test binary running only this test, with a guard env
// var set so the child takes the leak path. No separate bin target.
#[cfg(feature = "leak-check")]
#[test]
fn leak_check_reports_leaks_and_exits_two() {
    const GUARD: &str = "SKEV_LEAK_CHILD";
    if std::env::var(GUARD).is_ok() {
        // CHILD: leak on purpose (no release/dealloc), then shut down.
        let _leaked = unsafe { skev_alloc(8, 0xDEAD_BEEF) };
        skev_shutdown();                      // → report_leaks() to stderr, sets code 2
        std::process::exit(leak_exit_code()); // = 2
    }
    // PARENT: spawn the child, assert it exited 2 and printed the marker.
    let exe = std::env::current_exe().unwrap();
    let out = std::process::Command::new(exe)
        .args(["--exact", "leak_check_reports_leaks_and_exits_two", "--nocapture"])
        .env(GUARD, "1")
        .output()
        .expect("spawn leak child");
    assert_eq!(out.status.code(), Some(2), "child should exit 2 on leak");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("skev: leak:"), "missing leak marker; stderr:\n{stderr}");
}
