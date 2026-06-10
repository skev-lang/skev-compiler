//! Lazy weak side-table — Phase E Decision 6 (weak section).
//!
//! For each strong-managed object that has ever had a weak ref taken,
//! one `WeakEntry` lives in a sharded hashmap keyed by the target's
//! address. Objects that never get a weak ref pay zero (no entry,
//! no allocation, no hashmap insert).
//!
//! Lifecycle (refcount trick — same shape as Rust `Arc`'s weak count):
//!   entry created with weak_count = 2
//!     (1 = virtual slot held by the strong holder, 1 = the new weak)
//!   weak_alloc on existing entry → fetch_add(1)
//!   weak_release                  → fetch_sub(1); if old==1 → free
//!   on_dealloc (strong dies)      → remove from table,
//!                                   strong_alive=false,
//!                                   fetch_sub(1); if old==1 → free
//!
//! While strong is alive, weak_count ≥ 1 (the virtual slot). Exactly
//! one party brings the count to 0 — no double-free is possible.

use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

use crate::alloc::{skev_alloc_raw, skev_free_raw};

const SHARD_COUNT: usize = 16;

#[repr(C)]
pub struct WeakEntry {
    /// The strong target this entry represents.
    /// Set at creation and never mutated.
    target: *mut u8,
    /// 1 (virtual strong slot) + N (active weak holders).
    weak_count: AtomicU64,
    /// True until on_dealloc fires for the target.
    strong_alive: AtomicBool,
}

#[repr(transparent)]
#[derive(Copy, Clone)]
struct EntryPtr(*mut WeakEntry);
// SAFETY: WeakEntry's interior is atomic-only after `target` is set
// at creation. Sharing a raw pointer across threads is sound.
unsafe impl Send for EntryPtr {}
unsafe impl Sync for EntryPtr {}

struct Shard {
    table: Mutex<HashMap<usize, EntryPtr>>,
}

static WEAK_TABLE: LazyLock<[Shard; SHARD_COUNT]> = LazyLock::new(|| {
    core::array::from_fn(|_| Shard {
        table: Mutex::new(HashMap::new()),
    })
});

fn shard_for(ptr: *mut u8) -> &'static Shard {
    // Discard the bottom-4 bits (libc malloc returns ≥16-byte aligned
    // memory on every v1.0 target), then modulo into a shard.
    let idx = ((ptr as usize) >> 4) % SHARD_COUNT;
    &(*WEAK_TABLE)[idx]
}

unsafe fn alloc_entry(target: *mut u8) -> *mut WeakEntry {
    let size = core::mem::size_of::<WeakEntry>() as u64;
    let raw = skev_alloc_raw(size);
    if raw.is_null() {
        return core::ptr::null_mut();
    }
    let entry = raw as *mut WeakEntry;
    // SAFETY: raw is suitably aligned (malloc returns 16-byte aligned,
    // WeakEntry needs 8-byte for its AtomicU64).
    unsafe {
        entry.write(WeakEntry {
            target,
            weak_count: AtomicU64::new(2),
            strong_alive: AtomicBool::new(true),
        });
    }
    entry
}

unsafe fn free_entry(entry: *mut WeakEntry) {
    // SAFETY: entry was returned by alloc_entry and not yet freed.
    unsafe {
        core::ptr::drop_in_place(entry);
    }
    skev_free_raw(entry as *mut u8);
}

/// Allocate (lazily) a weak entry for `target`. Returns a raw entry
/// pointer the caller stores in its weak slot, or NULL on OOM / null
/// target.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn skev_weak_alloc(target: *mut u8) -> *mut WeakEntry {
    if target.is_null() {
        return core::ptr::null_mut();
    }
    let shard = shard_for(target);
    let mut table = shard.table.lock().unwrap();
    match table.get(&(target as usize)).copied() {
        Some(EntryPtr(entry)) => {
            // SAFETY: entry is in the HashMap → not yet freed.
            unsafe {
                (*entry).weak_count.fetch_add(1, Ordering::AcqRel);
            }
            entry
        }
        None => {
            // SAFETY: caller upholds that target came from skev_alloc.
            let entry = unsafe { alloc_entry(target) };
            if !entry.is_null() {
                table.insert(target as usize, EntryPtr(entry));
            }
            entry
        }
    }
}

/// Upgrade a weak entry to the strong target pointer if the target is
/// still alive. Returns NULL if the target has been deallocated.
///
/// This does NOT bump the strong refcount — codegen is responsible for
/// calling `skev_retain` on the result if it intends to use the value
/// beyond a transient check.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn skev_weak_upgrade(entry: *mut WeakEntry) -> *mut u8 {
    if entry.is_null() {
        return core::ptr::null_mut();
    }
    // SAFETY: caller holds a weak claim, so entry is alive.
    let e = unsafe { &*entry };
    if !e.strong_alive.load(Ordering::Acquire) {
        return core::ptr::null_mut();
    }
    e.target
}

/// Release a weak claim. Frees the entry if this was the last claim.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn skev_weak_release(entry: *mut WeakEntry) {
    if entry.is_null() {
        return;
    }
    // SAFETY: caller holds a weak claim, so entry is alive.
    let e = unsafe { &*entry };
    let old = e.weak_count.fetch_sub(1, Ordering::AcqRel);
    if old == 1 {
        // SAFETY: we were the last claim — no other thread can observe
        // this entry.
        unsafe {
            free_entry(entry);
        }
    }
}

/// Called from arc.rs `skev_dealloc` immediately before the target
/// memory is returned to the allocator. Orphans the weak entry (if
/// any), marks the strong dead so future upgrades return NULL, and
/// frees the entry if no weak holders remain.
pub unsafe fn on_dealloc(ptr: *mut u8) {
    if ptr.is_null() {
        return;
    }
    let shard = shard_for(ptr);
    let mut table = shard.table.lock().unwrap();
    let removed = table.remove(&(ptr as usize));
    drop(table);
    let Some(EntryPtr(entry)) = removed else {
        return; // never had a weak ref
    };
    // SAFETY: entry was in the HashMap and now is not — exclusive
    // ownership in this call path until the fetch_sub below.
    unsafe {
        (*entry).strong_alive.store(false, Ordering::Release);
    }
    let old = unsafe { (*entry).weak_count.fetch_sub(1, Ordering::AcqRel) };
    if old == 1 {
        unsafe {
            free_entry(entry);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::arc::{PLAIN_DATA_HEADER_SIZE, skev_alloc, skev_release};

    fn shard_is_empty_for(ptr: *mut u8) -> bool {
        let shard = shard_for(ptr);
        let table = shard.table.lock().unwrap();
        !table.contains_key(&(ptr as usize))
    }

    #[test]
    fn weak_upgrade_returns_target_while_strong_alive() {
        let p = unsafe { skev_alloc(PLAIN_DATA_HEADER_SIZE as u64, 0) };
        assert!(!p.is_null());
        let entry = unsafe { skev_weak_alloc(p) };
        assert!(!entry.is_null());
        let upgraded = unsafe { skev_weak_upgrade(entry) };
        assert_eq!(upgraded, p);
        unsafe { skev_weak_release(entry) };
        unsafe { skev_release(p) };
    }

    #[test]
    fn weak_upgrade_returns_null_after_strong_release() {
        let p = unsafe { skev_alloc(PLAIN_DATA_HEADER_SIZE as u64, 0) };
        assert!(!p.is_null());
        let entry = unsafe { skev_weak_alloc(p) };
        unsafe { skev_release(p) }; // → on_dealloc fires
        let upgraded = unsafe { skev_weak_upgrade(entry) };
        assert!(upgraded.is_null());
        unsafe { skev_weak_release(entry) }; // frees entry
    }

    #[test]
    fn one_hundred_weak_allocs_then_strong_release_all_null() {
        let p = unsafe { skev_alloc(PLAIN_DATA_HEADER_SIZE as u64, 0) };
        assert!(!p.is_null());
        let entries: Vec<*mut WeakEntry> =
            (0..100).map(|_| unsafe { skev_weak_alloc(p) }).collect();
        for e in &entries {
            assert!(!e.is_null());
        }
        unsafe { skev_release(p) };
        for e in &entries {
            assert!(unsafe { skev_weak_upgrade(*e) }.is_null());
        }
        for e in &entries {
            unsafe { skev_weak_release(*e) };
        }
    }

    #[test]
    fn never_weak_allocd_has_no_entry() {
        let p = unsafe { skev_alloc(PLAIN_DATA_HEADER_SIZE as u64, 0) };
        assert!(!p.is_null());
        assert!(
            shard_is_empty_for(p),
            "shard should be empty before any weak_alloc"
        );
        unsafe { skev_release(p) };
    }

    #[test]
    fn weak_concurrent_stress() {
        let p = unsafe { skev_alloc(PLAIN_DATA_HEADER_SIZE as u64, 0) };
        assert!(!p.is_null());
        let p_addr = p as usize;
        let handles: Vec<_> = (0..4)
            .map(|_| {
                std::thread::spawn(move || {
                    let p = p_addr as *mut u8;
                    for _ in 0..200 {
                        let entry = unsafe { skev_weak_alloc(p) };
                        assert!(!entry.is_null());
                        let upgraded = unsafe { skev_weak_upgrade(entry) };
                        assert_eq!(upgraded, p);
                        unsafe { skev_weak_release(entry) };
                    }
                })
            })
            .collect();
        for h in handles {
            h.join().unwrap();
        }
        unsafe { skev_release(p) }; // triggers on_dealloc → frees entry
    }
}
