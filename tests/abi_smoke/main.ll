; Skev Runtime ABI smoke test — Phase E Step 12
; Minimal LLVM IR mirroring the Phase D declare block.
; Exercises the stable D1/D8 symbols; exit 0 = ABI linkable.

; ── Phase D declare block (stable symbols) ───────────────────
declare void @skev_init()
declare void @skev_shutdown()
declare ptr  @skev_alloc(i64, i32)
declare void @skev_retain(ptr)
declare void @skev_release(ptr)
declare void @skev_dealloc(ptr)
declare void @skev_runtime_panic(i32)
declare void @skev_runtime_panic_msg(i32, ptr, i64)

; ── ABI version constant (D8) ────────────────────────────────
@skev_runtime_version = external constant i32

define i32 @main() {
entry:
  ; link-time check: version constant must be reachable
  %ver = load i32, ptr @skev_runtime_version

  ; full lifecycle (mirrors integration test entity_lifecycle)
  call void @skev_init()
  %p = call ptr @skev_alloc(i64 24, i32 1)   ; entity header = 24B, type_id 1
  call void @skev_retain(ptr %p)             ; rc = 2
  call void @skev_release(ptr %p)            ; rc = 1
  call void @skev_release(ptr %p)            ; rc = 0 → auto-dealloc (D4)
  call void @skev_shutdown()
  ret i32 0
}
