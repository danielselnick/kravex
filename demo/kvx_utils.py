#!/usr/bin/env python3

"""
🔧 Kravex Demo Utilities — kvx_utils.py 🔧🚀📦
It was 3am. The demo scripts both needed index ops.
They both needed to find the binary. They both needed cluster health checks.
This module reunites them. Group therapy for Python scripts. 🦆

The singularity will happen before we stop adding functions to this file.
"""

import subprocess
import time
from pathlib import Path
from typing import Optional

import requests


# ============================================================================
#  📡 Cluster Health — "Are you alive? Blink twice if the JVM is eating you."
# ============================================================================

def check_cluster_health(url: str, timeout: int = 120) -> str:
    """
    Wait for a cluster to reach green/yellow health.
    Returns the health status string, or raises if timeout.

    Rationale: Demo needs to wait for clusters post-docker-compose.
    ES/OS clusters can take 30-60s to initialize on cold start,
    hence the generous default timeout.
    """
    # -- 💤 Poll every 2s, up to timeout. Like waiting for your Rust project to compile.
    waited = 0
    while waited < timeout:
        try:
            resp = requests.get(f"{url}/_cluster/health", timeout=5)
            if resp.status_code == 200:
                status = resp.json().get("status", "red")
                if status != "red":
                    return status
        except Exception:
            pass
        time.sleep(2)
        waited += 2

    raise TimeoutError(
        f"💀 Cluster at {url} not healthy after {timeout}s. "
        f"We waited. And waited. Like a dog at the window. But the cluster never came home."
    )


# ============================================================================
#  📦 Index Operations — CRUD for indices, demo-style
# ============================================================================

def create_index(
    url: str,
    name: str,
    mapping: Optional[dict] = None,
    shards: int = 1,
    replicas: int = 0,
    refresh: str = "-1",
) -> bool:
    """
    Create an index with optional mapping and ingest-optimal settings.
    Returns True on success, False on failure.

    Knowledge: shards=1, replicas=0, refresh=-1 maximizes ingest throughput
    for single-node setups. Refresh is disabled because ES refreshes
    every 1s by default, which wastes IOPS during bulk ingest.
    """
    body = mapping or {}
    if "settings" not in body:
        body["settings"] = {}
    body["settings"]["number_of_shards"] = shards
    body["settings"]["number_of_replicas"] = replicas
    body["settings"]["refresh_interval"] = refresh

    # -- 🚀 PUT the index. If it already exists, ES returns 400. We don't judge.
    resp = requests.put(
        f"{url}/{name}",
        json=body,
        headers={"Content-Type": "application/json"},
        timeout=30,
    )
    return resp.status_code in (200, 201)


def delete_index(url: str, name: str) -> bool:
    """
    Delete an index. Returns True if deleted or didn't exist (idempotent).
    The emotional equivalent of 'unsubscribe from all'.
    """
    resp = requests.delete(f"{url}/{name}", timeout=10)
    # -- 🗑️ 200 = deleted, 404 = already gone. Both are fine. Like letting go.
    return resp.status_code in (200, 404)


def get_doc_count(url: str, name: str) -> int:
    """
    Refresh + count. The eternal dance of eventual consistency.

    Tribal knowledge: ES/OS are eventually consistent. Without a refresh,
    recently indexed docs may not appear in _count. We always refresh first
    to get an accurate count, even though it adds ~100ms of latency.
    """
    try:
        requests.post(f"{url}/{name}/_refresh", timeout=10)
        resp = requests.get(f"{url}/{name}/_count", timeout=10)
        if resp.status_code == 200:
            return resp.json().get("count", 0)
    except Exception:
        pass
    return -1


def refresh_index(url: str, name: str) -> bool:
    """
    POST /_refresh — makes newly indexed docs searchable.
    Returns True on success. Like waking documents from their indexing slumber.
    """
    try:
        resp = requests.post(f"{url}/{name}/_refresh", timeout=10)
        return resp.status_code == 200
    except Exception:
        return False


def reset_index(
    url: str,
    name: str,
    shards: int = 1,
    replicas: int = 0,
    refresh: str = "-1",
) -> bool:
    """
    Delete + recreate with ingest-optimal settings.
    The scorched earth + rebuild approach. Therapy would call this 'healthy detachment'.
    """
    delete_index(url, name)
    return create_index(url, name, shards=shards, replicas=replicas, refresh=refresh)


# ============================================================================
#  🔧 Binary Discovery — "Where is kvx-cli? Have you tried looking in target/?"
# ============================================================================

def find_kvx_binary(project_root: Optional[Path] = None) -> Optional[Path]:
    """
    Locate an existing kvx-cli binary. Returns Path or None.

    Search order: release first (faster binary), then debug.
    Does NOT build — use build_kvx_binary() for that.
    """
    root = project_root or _guess_project_root()
    # -- 🔍 Check release first because release builds are what demos deserve
    release_path = root / "target" / "release" / "kvx-cli"
    if release_path.exists():
        return release_path

    debug_path = root / "target" / "debug" / "kvx-cli"
    if debug_path.exists():
        return debug_path

    return None


def build_kvx_binary(project_root: Optional[Path] = None) -> Path:
    """
    Build kvx-cli in release mode. Returns path to binary.
    Raises RuntimeError if build fails.

    'cargo build --release: the sound of the future being compiled'
    """
    root = project_root or _guess_project_root()
    result = subprocess.run(
        ["cargo", "build", "--release", "-p", "kvx-cli"],
        cwd=root,
        capture_output=True,
        text=True,
        timeout=300,
    )
    if result.returncode != 0:
        raise RuntimeError(
            f"💀 Build failed. Cargo said: {result.stderr[:500]}\n"
            f"Against all odds, cargo did NOT succeed. The engineers wept."
        )
    binary = root / "target" / "release" / "kvx-cli"
    if not binary.exists():
        raise RuntimeError(
            f"💀 Build appeared to succeed but binary is missing at {binary}. "
            f"Schrödinger's build."
        )
    return binary


def find_or_build_kvx_binary(project_root: Optional[Path] = None) -> Path:
    """
    Find existing release binary or build one. Debug binaries are forbidden for demos —
    demoing a debug build is like presenting a PowerPoint in Comic Sans.
    """
    root = project_root or _guess_project_root()
    release_path = root / "target" / "release" / "kvx-cli"
    if release_path.exists():
        return release_path
    # -- 🚀 No release binary found — build one. No debug fallback allowed.
    return build_kvx_binary(root)


def _guess_project_root() -> Path:
    """
    Walk up from this file to find the repo root (where Cargo.toml lives).
    demo/kvx_utils.py is at repo_root/demo/, so parent.parent gets us there.
    """
    # -- 🔧 demo/kvx_utils.py → demo/ → repo_root/
    candidate = Path(__file__).resolve().parent.parent
    if (candidate / "Cargo.toml").exists():
        return candidate
    # -- 🐛 Fallback: cwd. Hope for the best, prepare for the worst.
    return Path.cwd()


# -- 🦆 This duck is here for emotional support. It has no other purpose.
