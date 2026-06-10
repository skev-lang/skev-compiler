#!/usr/bin/env bash
# Skev Runtime ABI smoke test — Phase E Step 12
# Usage: bash tests/abi_smoke/run.sh
# Requires: llc (LLVM 18), clang, cargo
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$REPO_ROOT"

# ── Toolchain detection (env override > llc-18 > llc) ─────────
LLC="${LLC:-$(command -v llc-18 || command -v llc || true)}"
CLANG="${CLANG:-$(command -v clang-18 || command -v clang || true)}"
if [[ -z "$LLC" || -z "$CLANG" ]]; then
  echo "ERROR: llc and clang (LLVM 18) required. Not found on PATH."
  echo "  Set LLC= and CLANG= env vars to override."
  exit 1
fi

echo "=== Skev Runtime ABI smoke test ==="
echo "  LLC:   $LLC ($("$LLC" --version 2>&1 | grep -i version | head -1))"
echo "  CLANG: $CLANG"

# ── Build runtime if needed ───────────────────────────────────
if [[ ! -f target/release/libskev_runtime.a ]]; then
  echo "=== Building skev-runtime... ==="
  cargo build -p skev-runtime --release
fi

# ── Native libs the Rust staticlib needs (macOS frameworks etc) ─
echo "=== Resolving native-static-libs... ==="
NATIVE_LIBS="$(cargo rustc -q -p skev-runtime --release --crate-type staticlib -- \
  --print native-static-libs 2>&1 \
  | sed -n 's/^note: native-static-libs: //p' | tail -1 || true)"
if [[ -z "$NATIVE_LIBS" ]]; then
  # Args unchanged from a prior run → cargo cached → note not re-emitted.
  # Fall back to the known-good macOS set (clang auto-links libSystem/libc/libm).
  case "$(uname -s)" in
    Darwin) NATIVE_LIBS="-framework CoreFoundation -liconv" ;;
    *)      NATIVE_LIBS="" ;;
  esac
fi
echo "  native-static-libs: ${NATIVE_LIBS:-<none>}"

# ── Compile IR → object ───────────────────────────────────────
echo "=== Compiling IR... ==="
"$LLC" -filetype=obj tests/abi_smoke/main.ll -o /tmp/skev_abi_smoke.o

# ── Link against runtime + native libs ────────────────────────
echo "=== Linking... ==="
# shellcheck disable=SC2086
"$CLANG" /tmp/skev_abi_smoke.o target/release/libskev_runtime.a \
  $NATIVE_LIBS -o /tmp/skev_abi_smoke

# ── Run (capture exit without tripping set -e) ────────────────
echo "=== Running... ==="
if /tmp/skev_abi_smoke; then
  echo ""
  echo "ABI SMOKE: PASSED (exit 0)"
  echo "  Phase D emitter output links cleanly against"
  echo "  libskev_runtime.a — Phase E ABI is confirmed."
else
  EXIT=$?
  echo ""
  echo "ABI SMOKE: FAILED (exit $EXIT)"
  exit "$EXIT"
fi
