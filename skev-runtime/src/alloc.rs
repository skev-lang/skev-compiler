//! Pluggable allocator backend — Phase E Decision 3.
//!
//! v1.0 ships libc `malloc`/`free` behind a static function-pointer
//! indirection. Cost: one `Relaxed` atomic load + one indirect call
//! per alloc/free (≈1 ns on modern hardware). The indirection unlocks
//! v1.1 mimalloc and v1.x `#[skev_allocator]` paths without changing
//! callers.

use std::sync::atomic::{AtomicBool, AtomicPtr, Ordering};

/// Allocator function signature — matches the C ABI Skev codegen
/// emits for `skev_alloc(size, type_id)`. `type_id` is handled at
/// the `arc.rs` layer; this module deals only with the raw byte
/// allocator.
pub type SkevAllocFn = extern "C" fn(size: u64) -> *mut u8;

/// Free function signature — counterpart to `SkevAllocFn`.
pub type SkevFreeFn = extern "C" fn(ptr: *mut u8);

extern "C" fn default_alloc(size: u64) -> *mut u8 {
    // SAFETY: FFI to libc::malloc. Returns NULL on OOM — we propagate.
    unsafe { libc::malloc(size as usize) as *mut u8 }
}

extern "C" fn default_free(ptr: *mut u8) {
    // SAFETY: FFI to libc::free. NULL is allowed by C99 (no-op).
    unsafe { libc::free(ptr as *mut libc::c_void) }
}

static SKEV_ALLOC_FN: AtomicPtr<()> =
    AtomicPtr::new(default_alloc as *mut ());
static SKEV_FREE_FN: AtomicPtr<()> =
    AtomicPtr::new(default_free as *mut ());
static INSTALLED: AtomicBool = AtomicBool::new(false);

/// Allocate `size` bytes via the currently-installed allocator.
/// Returns NULL on OOM — the caller decides how to react.
pub fn skev_alloc_raw(size: u64) -> *mut u8 {
    let raw = SKEV_ALLOC_FN.load(Ordering::Relaxed);
    // SAFETY: SKEV_ALLOC_FN is initialised from a valid
    // `extern "C" fn(u64) -> *mut u8` and only ever overwritten with
    // another such pointer via `install_allocator`.
    unsafe {
        let f: SkevAllocFn = core::mem::transmute(raw);
        f(size)
    }
}

/// Free a pointer obtained from `skev_alloc_raw`.
pub fn skev_free_raw(ptr: *mut u8) {
    let raw = SKEV_FREE_FN.load(Ordering::Relaxed);
    // SAFETY: see SKEV_ALLOC_FN — identical invariant for the free fn.
    unsafe {
        let f: SkevFreeFn = core::mem::transmute(raw);
        f(ptr)
    }
}

/// Install a custom allocator pair. Set-once: subsequent calls
/// return `false` without modifying state.
pub fn install_allocator(alloc: SkevAllocFn, free: SkevFreeFn) -> bool {
    if INSTALLED
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Relaxed)
        .is_err()
    {
        return false;
    }
    SKEV_ALLOC_FN.store(alloc as *mut (), Ordering::Release);
    SKEV_FREE_FN.store(free as *mut (), Ordering::Release);
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    use std::sync::atomic::AtomicU64;

    // Cargo runs tests in parallel — serialise them because they
    // touch the same process-wide allocator state.
    static TEST_LOCK: Mutex<()> = Mutex::new(());

    static CUSTOM_ALLOC_CALLS: AtomicU64 = AtomicU64::new(0);
    static CUSTOM_FREE_CALLS: AtomicU64 = AtomicU64::new(0);

    extern "C" fn custom_alloc(size: u64) -> *mut u8 {
        CUSTOM_ALLOC_CALLS.fetch_add(1, Ordering::Relaxed);
        // Delegate to libc so the rest of the test suite still gets
        // valid memory after this allocator is installed.
        unsafe { libc::malloc(size as usize) as *mut u8 }
    }

    extern "C" fn custom_free(ptr: *mut u8) {
        CUSTOM_FREE_CALLS.fetch_add(1, Ordering::Relaxed);
        unsafe { libc::free(ptr as *mut libc::c_void) }
    }

    #[test]
    fn default_allocator_alloc_and_free() {
        let _g = TEST_LOCK.lock().unwrap();
        let p = skev_alloc_raw(100);
        assert!(!p.is_null(), "alloc returned NULL");
        skev_free_raw(p);
    }

    #[test]
    fn install_allocator_swaps_pointers() {
        let _g = TEST_LOCK.lock().unwrap();
        // Whether this test or an earlier one wins the set-once race,
        // after this call the active allocator IS custom_alloc.
        let _ = install_allocator(custom_alloc, custom_free);
        let before = CUSTOM_ALLOC_CALLS.load(Ordering::Relaxed);
        let p = skev_alloc_raw(50);
        assert!(!p.is_null());
        skev_free_raw(p);
        let after = CUSTOM_ALLOC_CALLS.load(Ordering::Relaxed);
        assert!(after > before, "custom allocator was not used after install");
    }

    #[test]
    fn install_allocator_second_call_is_noop() {
        let _g = TEST_LOCK.lock().unwrap();
        // Two installs back-to-back: the second must always be false,
        // independent of whether other tests already installed.
        let _first = install_allocator(custom_alloc, custom_free);
        let second = install_allocator(custom_alloc, custom_free);
        assert!(!second, "second install_allocator call must return false");
    }
}
