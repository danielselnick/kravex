#!/usr/bin/env bash
# ai
# 🔧 Kravex Benchmark Suite — Setup Script 🚀
# "In a world where benchmarks needed setting up... one shell script dared to try."
#
# This script creates a Python venv, installs deps, then delegates to setup.py
# which handles the real work: dependency checks, dataset downloads, doc count
# verification, and kvx-cli pre-build. 🦆
#
# Usage: Run from anywhere. The script resolves its own location.
#   ./benchmark/scripts/setup.sh
#   cd benchmark/scripts && ./setup.sh
#   bash benchmark/scripts/setup.sh

set -euo pipefail

# -- 🧭 Resolve script directory — works from any CWD, even through symlinks
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
VENV_DIR="${SCRIPT_DIR}/.venv"

# -- 🎨 Logging helpers — emoji-prefixed because the law demands it
info()  { echo "🚀 $*"; }
warn()  { echo "⚠️  $*"; }
die()   { echo "💀 $*" >&2; exit 1; }

# -- 🔧 Preflight: need uv or pip for venv + deps
if command -v uv &>/dev/null; then
    PKG_MGR="uv"
elif command -v python3 &>/dev/null; then
    PKG_MGR="python3"
else
    die "Neither uv nor python3 found. Install one. I believe in you."
fi

info "Using package manager: ${PKG_MGR}"

# -- 🏗️ Create venv if it doesn't exist — idempotent like a good setup script
if [[ -d "${VENV_DIR}" ]]; then
    info "Venv already exists at ${VENV_DIR} — skipping creation ✅"
else
    info "Creating venv at ${VENV_DIR}"
    if [[ "${PKG_MGR}" == "uv" ]]; then
        uv venv "${VENV_DIR}"
    else
        python3 -m venv "${VENV_DIR}"
    fi
    info "Venv created ✅"
fi

# -- 🐍 Use the venv's Python directly — no need to source activate
VENV_PYTHON="${VENV_DIR}/bin/python"

if [[ ! -x "${VENV_PYTHON}" ]]; then
    die "Venv Python not found at ${VENV_PYTHON}. The venv is haunted. Delete ${VENV_DIR} and retry."
fi

# -- 📦 Install Python deps into venv (setup.py needs requests for nothing,
#    but kvx_utils needs subprocess which is stdlib — just ensure pip is there
#    for setup.py's esrally install attempt)
info "Ensuring pip is available in venv"
if [[ "${PKG_MGR}" == "uv" ]]; then
    uv pip install --python "${VENV_PYTHON}" pip --quiet 2>/dev/null || true
else
    "${VENV_PYTHON}" -m ensurepip --upgrade --default-pip 2>/dev/null || true
fi

# -- 🚀 Run setup.py with the venv Python — it handles everything else:
#    dependency checks, dataset downloads, doc count verification, kvx-cli build
info "Running setup.py — 'May the benchmarks be ever in your favor.'"
echo ""

cd "${SCRIPT_DIR}"
"${VENV_PYTHON}" setup.py

# -- ✅ Done. The singularity is one step closer.
echo ""
info "setup.sh complete. Venv at: ${VENV_DIR}"
info "To activate manually: source ${VENV_DIR}/bin/activate"
