#!/usr/bin/env bash
# ============================================================================
#  Kravex Demo — Leg 2: Elasticsearch → OpenSearch
# ============================================================================
#  Migrates a dataset index from Elasticsearch to OpenSearch using kvx-cli.
#  Expects the binary to already be built (demo.sh handles that).
#
#  Usage: ./demo/esdb_to_osdb.sh [dataset]
#    dataset: geonames (default), noaa, pmc
# ============================================================================
set -euo pipefail

DATASET="${1:-geonames}"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
CONFIG="$SCRIPT_DIR/demo_esdb_to_osdb_${DATASET}.toml"

# ── Validate config exists ─────────────────────────────────────────────────
if [[ ! -f "$CONFIG" ]]; then
  echo "💀 Config not found: $CONFIG"
  echo "   Available datasets: geonames, noaa, pmc"
  exit 1
fi

# ── Locate the kvx-cli binary ──────────────────────────────────────────────
BINARY=""
if [[ -x "$PROJECT_ROOT/target/release/kvx-cli" ]]; then
  BINARY="$PROJECT_ROOT/target/release/kvx-cli"
elif [[ -x "$PROJECT_ROOT/target/debug/kvx-cli" ]]; then
  BINARY="$PROJECT_ROOT/target/debug/kvx-cli"
fi

if [[ -z "$BINARY" ]]; then
  echo "💀 kvx-cli binary not found."
  echo "   Build it first by running: ./demo/demo.sh"
  echo "   Or manually: cargo build --release -p kvx-cli"
  exit 1
fi

# ── Launch ──────────────────────────────────────────────────────────────────
echo "🚀 Leg 2: Elasticsearch → OpenSearch ($DATASET)"
echo "   Binary: $BINARY"
echo "   Config: $CONFIG"
echo ""

cd "$PROJECT_ROOT"
RUST_LOG=info exec "$BINARY" "$CONFIG"
