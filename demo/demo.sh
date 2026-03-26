#!/usr/bin/env bash
# ============================================================================
#  Kravex Demo — Entry Point
# ============================================================================
#  Ensures uv is installed, builds kvx-cli in release mode, then delegates
#  to demo.py for orchestration.
#  Usage: ./demo/demo.sh
# ============================================================================
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# ── Check uv ────────────────────────────────────────────────────────────────
if ! command -v uv &>/dev/null; then
  echo "╔══════════════════════════════════════════════════════════════╗"
  echo "║  uv is required but not installed.                          ║"
  echo "║                                                             ║"
  echo "║  Install:                                                   ║"
  echo "║    curl -LsSf https://astral.sh/uv/install.sh | sh         ║"
  echo "║    — or —                                                   ║"
  echo "║    brew install uv                                          ║"
  echo "╚══════════════════════════════════════════════════════════════╝"
  exit 1
fi

# ── Check cargo ─────────────────────────────────────────────────────────────
if ! command -v cargo &>/dev/null; then
  echo "╔══════════════════════════════════════════════════════════════╗"
  echo "║  cargo (Rust toolchain) is required but not installed.      ║"
  echo "║                                                             ║"
  echo "║  Install:                                                   ║"
  echo "║    curl --proto '=https' --tlsv1.2 -sSf                    ║"
  echo "║      https://sh.rustup.rs | sh                              ║"
  echo "╚══════════════════════════════════════════════════════════════╝"
  exit 1
fi

# ── Build kvx-cli (release) ────────────────────────────────────────────────
BINARY="$PROJECT_ROOT/target/release/kvx-cli"

echo "🔨 Building kvx-cli in release mode..."
cd "$PROJECT_ROOT"
cargo build --release -p kvx-cli
echo ""

if [[ ! -x "$BINARY" ]]; then
  echo "💀 Build completed but binary not found at: $BINARY"
  exit 1
fi
echo "✅ kvx-cli built: $BINARY"


# ── Run orchestrator ────────────────────────────────────────────────────────
cd "$PROJECT_ROOT"
exec uv run --project demo demo/demo.py "$@"
