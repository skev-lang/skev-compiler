//! Allocation leak tracker — Phase E Decision 5.
//!
//! When the `leak-check` Cargo feature is OFF: this module compiles
//! to zero code — `track_alloc`, `track_dealloc`, and `report_leaks`
//! are no-op functions (and the arc.rs call sites are cfg-gated out
//! entirely). Production builds pay nothing.
//!
//! When the feature is ON: every `skev_alloc` inserts an entry keyed
//! by the returned pointer into a 16-shard hashmap. Each entry
//! carries the `type_id` (compile-time-assigned) and a captured
//! backtrace. `skev_dealloc` removes the entry. `report_leaks` walks
//! the tracker and prints any survivors to stderr, returning the count.

#[cfg(feature = "leak-check")]
pub mod inner {
    use std::backtrace::Backtrace;
    use std::collections::HashMap;
    use std::sync::{LazyLock, Mutex};

    pub struct AllocInfo {
        pub type_id: u32,
        pub backtrace: Backtrace,
    }

    pub const SHARD_COUNT: usize = 16;

    pub static TRACKER: LazyLock<[Mutex<HashMap<usize, AllocInfo>>; SHARD_COUNT]> =
        LazyLock::new(|| core::array::from_fn(|_| Mutex::new(HashMap::new())));

    fn shard_for(ptr: *mut u8) -> &'static Mutex<HashMap<usize, AllocInfo>> {
        let idx = ((ptr as usize) >> 4) % SHARD_COUNT;
        &(*TRACKER)[idx]
    }

    pub fn track_alloc(ptr: *mut u8, type_id: u32) {
        if ptr.is_null() {
            return;
        }
        let shard = shard_for(ptr);
        let mut table = shard.lock().unwrap();
        table.insert(
            ptr as usize,
            AllocInfo {
                type_id,
                backtrace: Backtrace::capture(),
            },
        );
    }

    pub fn track_dealloc(ptr: *mut u8) {
        if ptr.is_null() {
            return;
        }
        let shard = shard_for(ptr);
        let mut table = shard.lock().unwrap();
        table.remove(&(ptr as usize));
    }

    /// Walk every shard and emit one stderr line per live allocation,
    /// then return the total count. Called from `skev_shutdown`
    /// (wired in Step 7).
    ///
    /// Output format:
    ///   skev: leak: type_id=<N> at 0x<addr>
    ///   skev: allocated at:
    ///   <backtrace>
    pub fn report_leaks() -> u32 {
        let mut total = 0u32;
        for shard in TRACKER.iter() {
            let table = shard.lock().unwrap();
            for (ptr, info) in table.iter() {
                eprintln!(
                    "skev: leak: type_id={} at 0x{:x}\nskev: allocated at:\n{}",
                    info.type_id, ptr, info.backtrace
                );
                total = total.saturating_add(1);
            }
        }
        total
    }
}

#[cfg(feature = "leak-check")]
pub use inner::{report_leaks, track_alloc, track_dealloc};

#[cfg(not(feature = "leak-check"))]
pub fn track_alloc(_: *mut u8, _: u32) {}

#[cfg(not(feature = "leak-check"))]
pub fn track_dealloc(_: *mut u8) {}

#[cfg(not(feature = "leak-check"))]
pub fn report_leaks() -> u32 {
    0
}

#[cfg(all(feature = "leak-check", test))]
mod tests {
    use super::inner::*;
    use crate::arc::{PLAIN_DATA_HEADER_SIZE, skev_alloc, skev_release};

    /// Test-only: count tracker entries with a specific type_id. Lets
    /// each test use a unique sentinel so concurrent tests don't
    /// pollute counts.
    fn count_for_type_id(tid: u32) -> u32 {
        let mut total = 0u32;
        for shard in TRACKER.iter() {
            let table = shard.lock().unwrap();
            for info in table.values() {
                if info.type_id == tid {
                    total += 1;
                }
            }
        }
        total
    }

    #[test]
    fn five_allocs_three_releases_leaves_two() {
        const TID: u32 = 0xDEAD_0501;
        let ptrs: Vec<*mut u8> = (0..5)
            .map(|_| unsafe { skev_alloc(PLAIN_DATA_HEADER_SIZE as u64, TID) })
            .collect();
        for p in ptrs.iter().take(3) {
            unsafe { skev_release(*p) };
        }
        assert_eq!(count_for_type_id(TID), 2);
        // Cleanup: release the remaining 2 so this test doesn't leak
        // type_id TID into other test runs.
        for p in ptrs.iter().skip(3) {
            unsafe { skev_release(*p) };
        }
        assert_eq!(count_for_type_id(TID), 0);
    }

    #[test]
    fn no_allocs_for_sentinel_reports_zero() {
        const TID: u32 = 0xDEAD_0502;
        assert_eq!(count_for_type_id(TID), 0);
        // Exercise the public report_leaks API once — the return
        // value isn't asserted because parallel tests' in-flight
        // allocations would race with the count.
        let _ = super::report_leaks();
    }

    #[test]
    fn concurrent_alloc_dealloc_balances() {
        const TID: u32 = 0xDEAD_0503;
        let handles: Vec<_> = (0..4)
            .map(|_| {
                std::thread::spawn(move || {
                    for _ in 0..50 {
                        let p = unsafe { skev_alloc(PLAIN_DATA_HEADER_SIZE as u64, TID) };
                        unsafe { skev_release(p) };
                    }
                })
            })
            .collect();
        for h in handles {
            h.join().unwrap();
        }
        assert_eq!(count_for_type_id(TID), 0);
    }
}
