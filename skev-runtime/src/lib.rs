//! Skev runtime — v1.0
//!
//! Public C-ABI surface for libskev_runtime.a.
//! All exported functions are extern "C" with #[unsafe(no_mangle)].
//! ABI is stable from v1.0.0 — see Chapter 8 ABI Stability.

// Submodule declarations (added incrementally by Steps 1–7)
mod alloc;
mod arc;
mod weak;
mod panic;
mod leak;
mod init;

// ── Public C-ABI surface (Decisions D1 + D10) ──────────────────
// 12 functions + 1 version constant = 13 stable v1.0 symbols.

// arc.rs — entity / plain-data allocation + refcount (D1, D2, D4)
pub use arc::{skev_alloc, skev_retain, skev_release, skev_dealloc};

// init.rs — runtime lifecycle (D9)
pub use init::{skev_init, skev_shutdown, leak_exit_code};

// panic.rs — runtime panic path + safety handler (D10)
pub use panic::{
    skev_runtime_panic,
    skev_runtime_panic_msg,
    skev_register_safety_handler,
};

// weak.rs — weak reference operations (D6)
pub use weak::{skev_weak_alloc, skev_weak_upgrade, skev_weak_release};

// ── ABI stability symbol (Decision D8) ─────────────────────────
// Linker-side mismatch detection. v1.x = 1. Increments on major bump.
// Type MUST be i32 to match the emitted LLVM IR declaration in Phase D:
//   @skev_runtime_version = external constant i32
#[unsafe(no_mangle)]
pub static skev_runtime_version: i32 = 1;

// ── Internal modules (no public surface) ───────────────────────
// alloc.rs — allocator fn-pointer install (D3)
// leak.rs  — leak tracker (D5, --features leak-check only)
// Reachable via crate:: paths from arc / weak / init only.
