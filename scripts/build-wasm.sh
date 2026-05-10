#!/usr/bin/env bash
set -euo pipefail

if ! command -v cargo >/dev/null 2>&1; then
  export PATH="${HOME:-}/.cargo/bin:/mnt/c/Users/${USER:-}/.cargo/bin:${PATH}"
fi

CARGO_BIN="${CARGO:-cargo}"
WASM_BINDGEN_BIN="${WASM_BINDGEN:-wasm-bindgen}"

if ! command -v "${CARGO_BIN}" >/dev/null 2>&1 && command -v cargo.exe >/dev/null 2>&1; then
  CARGO_BIN="cargo.exe"
fi

if ! command -v "${WASM_BINDGEN_BIN}" >/dev/null 2>&1 && command -v wasm-bindgen.exe >/dev/null 2>&1; then
  WASM_BINDGEN_BIN="wasm-bindgen.exe"
fi

"${CARGO_BIN}" build -p glyphspace-wasm --target wasm32-unknown-unknown
"${WASM_BINDGEN_BIN}" \
  --target web \
  --out-dir web/src/wasm \
  --out-name glyphspace_wasm \
  target/wasm32-unknown-unknown/debug/glyphspace_wasm.wasm
