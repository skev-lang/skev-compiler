//! Skev ARC Runtime — v1.0
//!
//! Static library linked into every Skev executable.
//! C-ABI surface: skev_alloc / skev_retain / skev_release / skev_dealloc
//!                skev_init / skev_shutdown
//!                skev_runtime_panic / skev_runtime_panic_msg
//!
//! Module layout (filled in across Steps 2–8 of Phase E):
//!   alloc — fn-pointer indirection over libc malloc/free (D3, Step 2)
//!   arc   — retain / release / dealloc                  (D1, D2, D4 — Step 3)
//!   panic — runtime panic + reason codes                (D10 — Step 4)
//!   weak  — lazy weak side-table                        (D6 — Step 5)
//!   leak  — alloc tracker, feature-gated                (D5 — Step 6)
//!   init  — skev_init / skev_shutdown                   (D9 — Step 7)
//!
//! ABI version constant (D8) — wired in Step 8 alongside the
//! public C-ABI re-exports.

// pub mod alloc;
// pub mod arc;
// pub mod panic;
// pub mod weak;
// pub mod leak;
// pub mod init;
