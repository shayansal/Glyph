#!/usr/bin/env bash
set -euo pipefail
if ! command -v cargo >/dev/null 2>&1; then
  export PATH="${HOME:-}/.cargo/bin:/mnt/c/Users/${USER:-}/.cargo/bin:${PATH}"
fi
CARGO_BIN="${CARGO:-cargo}"
if ! command -v "${CARGO_BIN}" >/dev/null 2>&1 && command -v cargo.exe >/dev/null 2>&1; then
  CARGO_BIN="cargo.exe"
fi
export CARGO="${CARGO_BIN}"
"${CARGO_BIN}" fmt --all -- --check
"${CARGO_BIN}" clippy --workspace --all-targets -- -D warnings
"${CARGO_BIN}" test --workspace
"${CARGO_BIN}" build --workspace
bash scripts/build-wasm.sh
cd web && npm install && npm test && npm run build
