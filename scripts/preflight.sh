#!/usr/bin/env bash
# scripts/preflight.sh
# Pre-flight validation script for GitHub Export.
# Runs all CI checks locally before pushing to GitHub.
# Exit code 0 = all checks passed.  Exit code 1 = one or more checks failed.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
CARGO_ROOT="$PROJECT_ROOT/src-tauri"

ANY_FAILED=0
declare -A RESULTS

# ── Helpers ───────────────────────────────────────────────────────────────────

run_check() {
    local name="$1"
    shift
    echo ""
    echo "==> $name"
    if "$@"; then
        RESULTS["$name"]="PASS"
        echo "    PASS"
    else
        RESULTS["$name"]="FAIL"
        echo "    FAIL"
        ANY_FAILED=1
    fi
}

# ── File Existence Checks ─────────────────────────────────────────────────────

run_check "flake.nix exists" \
    bash -c "test -f '$PROJECT_ROOT/flake.nix' && echo '    Found: $PROJECT_ROOT/flake.nix'"

run_check "src-tauri/Cargo.lock exists" \
    bash -c "test -f '$CARGO_ROOT/Cargo.lock' && echo '    Found: $CARGO_ROOT/Cargo.lock' || \
             (echo '    ERROR: Cargo.lock missing — run cargo generate-lockfile inside src-tauri/ and commit it' && exit 1)"

# ── Rust Checks (run from src-tauri/) ────────────────────────────────────────

run_check "cargo build (debug)" \
    bash -c "cd '$CARGO_ROOT' && cargo build"

run_check "cargo clippy -- -D warnings" \
    bash -c "cd '$CARGO_ROOT' && cargo clippy -- -D warnings"

run_check "cargo test" \
    bash -c "cd '$CARGO_ROOT' && cargo test"

# ── Summary ───────────────────────────────────────────────────────────────────

echo ""
echo "=================================================="
echo "  Preflight Summary"
echo "=================================================="

for key in "${!RESULTS[@]}"; do
    value="${RESULTS[$key]}"
    if [ "$value" = "PASS" ]; then
        echo "  [PASS]  $key"
    else
        echo "  [FAIL]  $key"
    fi
done

echo "=================================================="

if [ "$ANY_FAILED" -ne 0 ]; then
    echo ""
    echo "  PREFLIGHT FAILED — fix the issues above before pushing."
    echo ""
    exit 1
else
    echo ""
    echo "  All checks passed. Code is ready to push to GitHub."
    echo ""
    exit 0
fi
