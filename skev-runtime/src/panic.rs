//! Runtime panic path — Phase E Decision 10.
//!
//! Two C-ABI entry points:
//!   skev_runtime_panic(reason)               — internal invariant failures
//!   skev_runtime_panic_msg(reason, msg, len) — user-invoked panics
//!
//! Both fire any registered safety handler (set-once via
//! skev_register_safety_handler), then write to stderr and abort().
//! Debug builds include a backtrace (when RUST_BACKTRACE=1).
//!
//! Reason codes are a STABLE v1.0 ABI guarantee — new codes may be
//! appended but existing values never change.
//!
//! The safety-critical build profile (D10: "no stderr write — may not
//! exist on target") is not implemented in v1.0 of this module — the
//! safety handler hook is the substitute. Step 11 / Chapter 10 work
//! revisits.

use core::sync::atomic::{AtomicBool, AtomicPtr, Ordering};
use std::process::abort;

pub const USER_PANIC: u32 = 0;
pub const REFCOUNT_UNDERFLOW: u32 = 1;
pub const REFCOUNT_OVERFLOW: u32 = 2;
pub const INVARIANT_VIOLATION: u32 = 3;
pub const ALLOCATOR_FAILURE: u32 = 4;

const REASON_STRINGS: [&str; 5] = [
    "user panic",
    "refcount underflow",
    "refcount overflow",
    "invariant violation",
    "allocator failure",
];

fn reason_string(reason: u32) -> &'static str {
    REASON_STRINGS
        .get(reason as usize)
        .copied()
        .unwrap_or("unknown reason")
}

type SafetyHandler = extern "C" fn(reason: u32) -> !;

static SAFETY_HANDLER: AtomicPtr<()> = AtomicPtr::new(core::ptr::null_mut());
static HANDLER_INSTALLED: AtomicBool = AtomicBool::new(false);

/// Register a safety-critical panic handler. Set-once: subsequent
/// calls return `false` without modifying state. The handler MUST
/// diverge (its `-> !` return type is contract, not advice).
#[unsafe(no_mangle)]
pub extern "C" fn skev_register_safety_handler(handler: SafetyHandler) -> bool {
    if HANDLER_INSTALLED
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Relaxed)
        .is_err()
    {
        return false;
    }
    SAFETY_HANDLER.store(handler as *mut (), Ordering::Release);
    true
}

/// If a safety handler is registered, call it. The handler is
/// contractually required to diverge, so on return from this fn
/// no handler was set.
fn fire_safety_handler_if_set(reason: u32) {
    let ptr = SAFETY_HANDLER.load(Ordering::Acquire);
    if !ptr.is_null() {
        // SAFETY: SAFETY_HANDLER is only ever written by
        // skev_register_safety_handler, which only stores a valid
        // SafetyHandler fn pointer cast.
        let h: SafetyHandler = unsafe { core::mem::transmute(ptr) };
        h(reason); // diverges
    }
}

fn write_panic_header(reason: &str, msg: Option<&str>) {
    match msg {
        Some(m) => eprintln!("skev: runtime panic: {reason}: {m}"),
        None => eprintln!("skev: runtime panic: {reason}"),
    }
    #[cfg(debug_assertions)]
    {
        eprintln!("skev: backtrace:\n{}", std::backtrace::Backtrace::capture());
    }
}

/// Internal runtime invariant failure — diverges.
///
/// If a safety handler is registered, it is called first. Otherwise
/// writes a reason line to stderr (with a backtrace in debug builds)
/// and aborts the process.
#[unsafe(no_mangle)]
pub extern "C" fn skev_runtime_panic(reason: u32) -> ! {
    fire_safety_handler_if_set(reason);
    write_panic_header(reason_string(reason), None);
    abort();
}

/// User-invoked panic with a message — diverges. `msg` must point
/// to `len` UTF-8 bytes valid for the duration of this call. Invalid
/// UTF-8 is replaced with a fixed `<invalid utf-8>` marker rather
/// than triggering a second panic.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn skev_runtime_panic_msg(
    reason: u32,
    msg: *const u8,
    len: u64,
) -> ! {
    fire_safety_handler_if_set(reason);
    let s = if msg.is_null() || len == 0 {
        ""
    } else {
        // SAFETY: caller guarantees `msg` points to `len` bytes that
        // are safe to read for the duration of this call.
        unsafe {
            let slice = core::slice::from_raw_parts(msg, len as usize);
            core::str::from_utf8(slice).unwrap_or("<invalid utf-8>")
        }
    };
    write_panic_header(reason_string(reason), Some(s));
    abort();
}

// ---------------------------------------------------------------
// Test support — exposed under cfg(test) (always available to
// in-crate tests) or under the `test-panic-as-result` feature
// (available to external downstream test code).
// ---------------------------------------------------------------

#[cfg(any(test, feature = "test-panic-as-result"))]
pub mod test_support {
    //! Test-only helpers. Lets tests verify the panic path without
    //! aborting the test process.

    use super::*;
    use core::convert::Infallible;

    /// Returns `Err(reason)` instead of aborting. Does NOT invoke any
    /// registered safety handler — handlers diverge, which would
    /// defeat the purpose of an assertable error.
    pub fn panic_as_result(reason: u32) -> Result<Infallible, u32> {
        Err(reason)
    }

    /// Same as panic_as_result but also surfaces the parsed message.
    pub fn panic_msg_as_result(
        reason: u32,
        msg: *const u8,
        len: u64,
    ) -> Result<Infallible, (u32, String)> {
        let s = if msg.is_null() || len == 0 {
            String::new()
        } else {
            // SAFETY: same contract as skev_runtime_panic_msg.
            unsafe {
                let slice = core::slice::from_raw_parts(msg, len as usize);
                core::str::from_utf8(slice)
                    .unwrap_or("<invalid utf-8>")
                    .to_string()
            }
        };
        Err((reason, s))
    }

    /// Look up the canonical human-readable string for a reason code.
    pub fn reason_string(reason: u32) -> &'static str {
        super::reason_string(reason)
    }

    /// True iff a safety handler has been registered this process.
    pub fn safety_handler_is_set() -> bool {
        !SAFETY_HANDLER.load(Ordering::Acquire).is_null()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    // Tests register a safety handler — serialise so the set-once
    // CAS races deterministically.
    static TEST_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn each_reason_code_has_a_string() {
        assert_eq!(reason_string(USER_PANIC), "user panic");
        assert_eq!(reason_string(REFCOUNT_UNDERFLOW), "refcount underflow");
        assert_eq!(reason_string(REFCOUNT_OVERFLOW), "refcount overflow");
        assert_eq!(reason_string(INVARIANT_VIOLATION), "invariant violation");
        assert_eq!(reason_string(ALLOCATOR_FAILURE), "allocator failure");
    }

    #[test]
    fn safety_handler_is_set_after_register() {
        let _g = TEST_LOCK.lock().unwrap();
        extern "C" fn h(_: u32) -> ! {
            std::process::abort()
        }
        // Whether we win the set-once race or not, the handler IS
        // set after a successful registration in any test.
        let _ = skev_register_safety_handler(h);
        assert!(test_support::safety_handler_is_set());
    }

    #[test]
    fn second_register_safety_handler_returns_false() {
        let _g = TEST_LOCK.lock().unwrap();
        extern "C" fn h(_: u32) -> ! {
            std::process::abort()
        }
        let _ = skev_register_safety_handler(h);
        let second = skev_register_safety_handler(h);
        assert!(!second, "second register must return false");
    }

    #[test]
    fn panic_msg_parses_utf8() {
        let msg = "stack corruption at module loader";
        let r = test_support::panic_msg_as_result(
            INVARIANT_VIOLATION,
            msg.as_ptr(),
            msg.len() as u64,
        );
        match r {
            Err((INVARIANT_VIOLATION, s)) => assert_eq!(s, msg),
            Err(other) => panic!("wrong payload: {:?}", other),
            Ok(_) => unreachable!(),
        }
    }
}
