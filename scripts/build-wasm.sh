#!/usr/bin/env bash
set -euo pipefail

if ! command -v cargo >/dev/null 2>&1; then
  export PATH="${HOME:-}/.cargo/bin:/mnt/c/Users/${USER:-}/.cargo/bin:${PATH}"
fi

CARGO_BIN="${CARGO:-cargo}"
WASM_BINDGEN_BIN="${WASM_BINDGEN:-wasm-bindgen}"
WASM_BINDGEN_VERSION="${WASM_BINDGEN_VERSION:-0.2.121}"

if ! command -v "${CARGO_BIN}" >/dev/null 2>&1 && command -v cargo.exe >/dev/null 2>&1; then
  CARGO_BIN="cargo.exe"
fi

wasm_bindgen_matches() {
  local candidate="$1"

  if ! command -v "${candidate}" >/dev/null 2>&1; then
    return 1
  fi

  "${candidate}" --version 2>/dev/null | grep -q "${WASM_BINDGEN_VERSION}"
}

ensure_wasm_bindgen() {
  if wasm_bindgen_matches "${WASM_BINDGEN_BIN}"; then
    return
  fi

  if wasm_bindgen_matches wasm-bindgen.exe; then
    WASM_BINDGEN_BIN="wasm-bindgen.exe"
    return
  fi

  echo "Installing wasm-bindgen-cli ${WASM_BINDGEN_VERSION}..."
  "${CARGO_BIN}" install wasm-bindgen-cli --version "${WASM_BINDGEN_VERSION}" --locked --force
  WASM_BINDGEN_BIN="${WASM_BINDGEN:-wasm-bindgen}"

  if ! wasm_bindgen_matches "${WASM_BINDGEN_BIN}"; then
    echo "error: wasm-bindgen-cli ${WASM_BINDGEN_VERSION} was installed but is not first on PATH" >&2
    echo "       set WASM_BINDGEN to the installed binary path or fix PATH to include Cargo's bin directory" >&2
    exit 1
  fi
}

ensure_wasm_bindgen

"${CARGO_BIN}" build -p glyphspace-wasm --target wasm32-unknown-unknown
"${WASM_BINDGEN_BIN}" \
  --target web \
  --out-dir web/src/wasm \
  --out-name glyphspace_wasm \
  target/wasm32-unknown-unknown/debug/glyphspace_wasm.wasm
