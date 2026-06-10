//! ARC retain / release / dealloc — Phase E Decisions 1, 2, 4, 6 (contract 1).
//!
//! Header layout (D1):
//!   Entity:     24 B = [u64 ref_count][u64 dispatch_ptr][u64 component_mask]
//!   Plain data:  8 B = [u64 ref_count]
//!
//! Ordering (D2 — Rust Arc model):
//!   retain  — fetch_add(1, Relaxed)
//!   release — fetch_sub(1, Release); on old==1: fence(Acquire) + dealloc
//!
//! Overflow at isize::MAX/2 (D4) → panic(REFCOUNT_OVERFLOW)
//! Underflow on old==0 (D4)      → panic(REFCOUNT_UNDERFLOW)
//! NULL pointers — defensive no-op on retain/release/dealloc.
//!
//! The runtime only initialises `ref_count` at offset 0. `dispatch_ptr`,
//! `component_mask`, and the payload bytes are the caller's
//! responsibility (codegen emits stores for those right after `skev_alloc`).

use core::sync::atomic::{AtomicU64, Ordering, fence};

use crate::alloc::{skev_alloc_raw, skev_free_raw};
use crate::panic::{REFCOUNT_OVERFLOW, REFCOUNT_UNDERFLOW, skev_runtime_panic};

pub const ENTITY_HEADER_SIZE: usize = 24;
pub const PLAIN_DATA_HEADER_SIZE: usize = 8;
pub const REFCOUNT_OFFSET: usize = 0;

/// Half-of-isize::MAX threshold (D4). Leaves headroom for racing
/// attacker-driven retains across cores before u64 would wrap.
const MAX_REFCOUNT: u64 = (isize::MAX as u64) / 2;

use crate::init::ensure_init;

/// Allocate a Skev-managed heap object.
///
/// `size` includes the header. `type_id` is reserved for the
/// leak-check tracker (Step 6) and currently unused.
///
/// Returns NULL on OOM.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn skev_alloc(size: u64, type_id: u32) -> *mut u8 {
    ensure_init();
    let _ = type_id; // reserved for crate::leak::track_alloc (Step 6)

    let ptr = skev_alloc_raw(size);
    if ptr.is_null() {
        return core::ptr::null_mut();
    }

    // Initialise ref_count = 1 at offset 0. Caller initialises the rest.
    // SAFETY: ptr came from skev_alloc_raw, malloc returns ≥8-byte aligned
    // memory on every supported target, and AtomicU64 is repr(transparent)
    // over u64.
    unsafe {
        let header = &*(ptr as *const AtomicU64);
        header.store(1, Ordering::Relaxed);
    }

    #[cfg(feature = "leak-check")]
    crate::leak::track_alloc(ptr, type_id);
    ptr
}

/// Increment the reference count on a Skev-managed pointer.
/// Defensively no-ops on NULL. Panics on overflow (≥ isize::MAX/2).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn skev_retain(ptr: *mut u8) {
    if ptr.is_null() {
        return;
    }
    // SAFETY: caller upholds that ptr was returned by skev_alloc and
    // has not yet been deallocated.
    let old = unsafe {
        let header = &*(ptr as *const AtomicU64);
        header.fetch_add(1, Ordering::Relaxed)
    };
    if old >= MAX_REFCOUNT {
        skev_runtime_panic(REFCOUNT_OVERFLOW);
    }
}

/// Decrement the reference count. On the final drop (old==1) emit
/// an Acquire fence and deallocate. Defensively no-ops on NULL.
/// Panics on underflow (old==0 — double-release).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn skev_release(ptr: *mut u8) {
    if ptr.is_null() {
        return;
    }
    let old = unsafe {
        let header = &*(ptr as *const AtomicU64);
        header.fetch_sub(1, Ordering::Release)
    };
    match old {
        0 => skev_runtime_panic(REFCOUNT_UNDERFLOW),
        1 => {
            // Final drop — pair with Release subs from every prior
            // releasing thread, ensuring the dealloc sees their writes.
            fence(Ordering::Acquire);
            // SAFETY: ref count just dropped to 0 — no other reference exists.
            unsafe { skev_dealloc(ptr) };
        }
        _ => {}
    }
}

/// Return the allocation to the allocator. Defensively no-ops on NULL.
/// Most callers reach this through `skev_release` rather than calling
/// it directly.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn skev_dealloc(ptr: *mut u8) {
    if ptr.is_null() {
        return;
    }
    #[cfg(feature = "leak-check")]
    crate::leak::track_dealloc(ptr);
    // SAFETY: ptr came from skev_alloc — caller upholds validity.
    unsafe {
        crate::weak::on_dealloc(ptr);
    }
    skev_free_raw(ptr);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alloc_then_release_is_clean() {
        let p = unsafe { skev_alloc(PLAIN_DATA_HEADER_SIZE as u64, 0) };
        assert!(!p.is_null(), "alloc returned NULL");
        // count is 1 → release drops to 0 → dealloc fires
        unsafe { skev_release(p) };
    }

    #[test]
    fn retain_then_two_releases_deallocs_cleanly() {
        let p = unsafe { skev_alloc(PLAIN_DATA_HEADER_SIZE as u64, 0) };
        assert!(!p.is_null());
        unsafe { skev_retain(p) };   // count = 2
        unsafe { skev_release(p) };  // count = 1
        unsafe { skev_release(p) };  // count = 0 → dealloc
    }

    #[test]
    fn null_retain_is_noop() {
        unsafe { skev_retain(core::ptr::null_mut()) };
    }

    #[test]
    fn null_release_is_noop() {
        unsafe { skev_release(core::ptr::null_mut()) };
    }

    #[test]
    fn null_dealloc_is_noop() {
        unsafe { skev_dealloc(core::ptr::null_mut()) };
    }
}
