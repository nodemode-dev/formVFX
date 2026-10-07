#!/usr/bin/env bash
FORMVFX_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
export FORMVFX_ROOT
fx_say() { printf '[formvfx] %s\n' "$*"; }
fx_die() { printf '[formvfx] error: %s\n' "$*" >&2; exit 1; }
fx_os() {
  case "$(uname -s 2>/dev/null)" in
    Darwin) echo macos ;;
    Linux) echo linux ;;
    MINGW*|MSYS*|CYGWIN*) echo windows ;;
    *) echo unknown ;;
  esac
}
fx_python() {
  local c
  for c in python3.14 python3.13 python3.12 python3; do
    if command -v "$c" >/dev/null 2>&1 && "$c" -c 'import sys; raise SystemExit(0 if sys.version_info >= (3, 12) else 1)' 2>/dev/null; then
      echo "$c"
      return 0
    fi
  done
  return 1
}
fx_venv() {
  [ -f "$FORMVFX_ROOT/backend/venv/bin/activate" ] || fx_die "engine venv missing, run ./formvfx install"
  # shellcheck disable=SC1091
  source "$FORMVFX_ROOT/backend/venv/bin/activate"
}
fx_target_triple() {
  if [ -n "${FORMVFX_TARGET:-}" ]; then echo "$FORMVFX_TARGET"; return 0; fi
  command -v rustc >/dev/null 2>&1 || return 1
  rustc -vV | awk '/^host:/ {print $2}'
}
fx_exe() { [ "$(fx_os)" = windows ] && echo ".exe" || echo ""; }
