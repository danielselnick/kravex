#!/usr/bin/env python3
"""
Kravex End-to-End Demo — Orchestrator
======================================
Manages infrastructure and polls for kravex completion.
The actual kravex execution lives in standalone shell scripts:
  - file_to_esdb.sh  (Leg 1: geonames.json → Elasticsearch)
  - esdb_to_osdb.sh  (Leg 2: Elasticsearch → OpenSearch)

Usage: ./demo/demo.sh
   or: uv run --project demo demo/demo.py
"""

from __future__ import annotations

import json
import signal
import socket
import subprocess
import sys
import time
from dataclasses import dataclass, field
from datetime import datetime, timezone
from pathlib import Path

import requests

# ============================================================================
#  Constants
# ============================================================================

PROJECT_ROOT = Path(__file__).resolve().parent.parent
DEMO_DIR = PROJECT_ROOT / "demo"
DATASETS_DIR = PROJECT_ROOT / "datasets"
GEONAMES_FILE = DATASETS_DIR / "geonames.json"
GEONAMES_BZ2 = DATASETS_DIR / "geonames.json.bz2"
COMPOSE_FILE = DEMO_DIR / "docker-compose-demo.yml"

ES_URL = "http://localhost:19200"
OS_URL = "http://localhost:19201"
INDEX_NAME = "geonames"

DEMO_PORTS = {19200: "Elasticsearch", 19201: "OpenSearch"}
DEMO_CONTAINER_NAMES = {"kravex-demo-es", "kravex-demo-os"}

EXPECTED_DOC_COUNT = 11_396_503

# Polling config
POLL_INTERVAL_SECS = 5
MAX_WAIT_SECS = 1800  # 30 minutes — geonames is 11.4M docs, not a haiku

# ============================================================================
#  Geonames Index Definition (embedded)
# ============================================================================
# Mappings lifted from benchmark/reset_index.sh. Settings tuned for ingest:
# no replicas (single-node), refresh disabled (bulk throughput over freshness).

GEONAMES_INDEX_BODY = {
    "settings": {
        "number_of_shards": 1,
        "number_of_replicas": 0,
        "refresh_interval": "-1",
    },
    "mappings": {
        "properties": {
            "geonameid": {"type": "integer"},
            "name": {"type": "text"},
            "asciiname": {"type": "text"},
            "alternatenames": {"type": "text"},
            "feature_class": {"type": "keyword"},
            "feature_code": {"type": "keyword"},
            "country_code": {"type": "keyword"},
            "cc2": {"type": "keyword"},
            "admin1_code": {"type": "keyword"},
            "admin2_code": {"type": "keyword"},
            "admin3_code": {"type": "keyword"},
            "admin4_code": {"type": "keyword"},
            "population": {"type": "long"},
            "dem": {"type": "keyword"},
            "timezone": {"type": "keyword"},
            "location": {"type": "geo_point"},
        }
    },
}


# ============================================================================
#  Data Classes
# ============================================================================


@dataclass
class StepResult:
    """Timing and result tracking for a single demo step."""

    name: str
    start: datetime = field(default_factory=lambda: datetime.now(timezone.utc))
    end: datetime | None = None
    duration_secs: float = 0.0
    doc_count: int = 0
    success: bool = False

    def finish(self, doc_count: int = 0):
        self.end = datetime.now(timezone.utc)
        self.duration_secs = (self.end - self.start).total_seconds()
        self.doc_count = doc_count
        self.success = doc_count > 0


# ============================================================================
#  Pre-flight Checks
# ============================================================================


def check_dataset_exists() -> None:
    """Verify geonames.json exists. Offer bz2 decompression if applicable."""
    if GEONAMES_FILE.exists():
        size_gb = GEONAMES_FILE.stat().st_size / (1024**3)
        print(f"  ✅ Dataset found: {GEONAMES_FILE} ({size_gb:.1f} GB)")
        return

    if GEONAMES_BZ2.exists():
        print(f"  ⚠️  Found {GEONAMES_BZ2} but not the decompressed .json")
        answer = input("     Decompress with bunzip2? [Y/n]: ").strip().lower()
        if answer in ("", "y", "yes"):
            print("     Decompressing... (this takes a minute)")
            subprocess.run(["bunzip2", "-k", str(GEONAMES_BZ2)], check=True)
            if GEONAMES_FILE.exists():
                print("  ✅ Decompression complete.")
                return
        bail("Decompression declined or failed.")

    bail(
        f"Dataset not found: {GEONAMES_FILE}\n"
        f"  Download it with:\n"
        f"    python benchmark/scripts/setup.py\n"
        f"  Or manually:\n"
        f"    mkdir -p datasets\n"
        f"    curl -o datasets/geonames.json.bz2 \\\n"
        f"      https://rally-tracks.elastic.co/geonames/documents.json.bz2\n"
        f"    bunzip2 -k datasets/geonames.json.bz2"
    )


def check_docker_available() -> None:
    """Verify Docker daemon is reachable."""
    result = subprocess.run(
        ["docker", "info"],
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        bail(
            "Cannot connect to Docker daemon.\n"
            "  Is Docker Desktop running?\n"
            f"  Error: {result.stderr.strip()[:200]}"
        )
    print("  ✅ Docker daemon is reachable")


# ============================================================================
#  Port Conflict Resolution
# ============================================================================


def is_demo_stack_running() -> bool:
    """Check if the demo docker-compose stack is already up with all containers running."""
    result = subprocess.run(
        ["docker", "ps", "--format", "{{.Names}}"],
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        return False
    running = set(result.stdout.strip().splitlines())
    return DEMO_CONTAINER_NAMES.issubset(running)


def check_port_conflicts() -> None:
    """Check demo ports for conflicts — Docker containers and raw sockets."""
    conflicts: list[str] = []

    # Check Docker containers via `docker ps`
    result = subprocess.run(
        ["docker", "ps", "--format", "{{json .}}"],
        capture_output=True,
        text=True,
    )
    if result.returncode == 0:
        for line in result.stdout.strip().splitlines():
            if not line:
                continue
            try:
                container = json.loads(line)
            except json.JSONDecodeError:
                continue
            ports_str = container.get("Ports", "")
            name = container.get("Names", "unknown")
            for port, service in DEMO_PORTS.items():
                # Ports field looks like "0.0.0.0:19200->9200/tcp"
                if f":{port}->" in ports_str:
                    conflicts.append(
                        f"  Container '{name}' is using port {port} ({service})"
                    )

    # Check raw socket availability (catches non-Docker processes too)
    for port, service in DEMO_PORTS.items():
        if not _port_conflicts_found_in(conflicts, port):
            with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as sock:
                sock.settimeout(1)
                if sock.connect_ex(("localhost", port)) == 0:
                    conflicts.append(
                        f"  Port {port} ({service}) is in use by a non-Docker process"
                    )

    if not conflicts:
        print("  ✅ Demo ports are available")
        return

    print("  ⚠️  Port conflicts detected:")
    for c in conflicts:
        print(c)

    answer = input("\n  Stop conflicting containers and continue? [Y/n]: ").strip().lower()
    if answer not in ("", "y", "yes"):
        bail("Port conflicts unresolved. Exiting.")

    # Stop conflicting Docker containers
    result = subprocess.run(
        ["docker", "ps", "--format", "{{json .}}"],
        capture_output=True,
        text=True,
    )
    if result.returncode == 0:
        for line in result.stdout.strip().splitlines():
            if not line:
                continue
            try:
                container = json.loads(line)
            except json.JSONDecodeError:
                continue
            ports_str = container.get("Ports", "")
            container_id = container.get("ID", "")
            name = container.get("Names", "unknown")
            for port in DEMO_PORTS:
                if f":{port}->" in ports_str and container_id:
                    print(f"  🛑 Stopping container '{name}'...")
                    subprocess.run(
                        ["docker", "stop", container_id],
                        capture_output=True,
                        timeout=15,
                    )
                    break

    # Re-check raw sockets after stopping containers
    time.sleep(2)
    for port, service in DEMO_PORTS.items():
        with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as sock:
            sock.settimeout(1)
            if sock.connect_ex(("localhost", port)) == 0:
                bail(
                    f"Port {port} ({service}) is still in use after stopping containers.\n"
                    f"  A non-Docker process may be bound to it. Check with:\n"
                    f"    lsof -i :{port}"
                )


def _port_conflicts_found_in(conflicts: list[str], port: int) -> bool:
    """Check if a port already appears in the conflict list."""
    return any(str(port) in c for c in conflicts)


# ============================================================================
#  Docker Compose Management
# ============================================================================


def find_compose_command() -> list[str]:
    """Detect docker compose v2 or fall back to docker-compose v1."""
    result = subprocess.run(
        ["docker", "compose", "version"],
        capture_output=True,
        text=True,
    )
    if result.returncode == 0:
        return ["docker", "compose"]

    result = subprocess.run(
        ["docker-compose", "version"],
        capture_output=True,
        text=True,
    )
    if result.returncode == 0:
        return ["docker-compose"]

    bail("Neither 'docker compose' nor 'docker-compose' found. Install Docker Compose.")
    raise  # unreachable


def start_demo_containers() -> None:
    """Launch the demo docker-compose stack."""
    compose_cmd = find_compose_command()
    print("\n🐳 Starting demo containers...")
    subprocess.run(
        [*compose_cmd, "-f", str(COMPOSE_FILE), "up", "-d"],
        cwd=PROJECT_ROOT,
        check=True,
    )
    print("  ✅ Containers launched")


def stop_demo_containers() -> None:
    """Tear down demo containers and volumes."""
    compose_cmd = find_compose_command()
    subprocess.run(
        [*compose_cmd, "-f", str(COMPOSE_FILE), "down", "-v"],
        cwd=PROJECT_ROOT,
        check=True,
        capture_output=True,
    )


# ============================================================================
#  Cluster & Index Operations
# ============================================================================


def wait_for_cluster(url: str, name: str, timeout: int = 120) -> None:
    """Poll cluster health until green or yellow (single-node won't go green without replicas)."""
    print(f"  ⏳ Waiting for {name} at {url}...")
    deadline = time.time() + timeout
    while time.time() < deadline:
        try:
            resp = requests.get(f"{url}/_cluster/health", timeout=5)
            if resp.ok:
                status = resp.json().get("status", "red")
                if status in ("green", "yellow"):
                    print(f"  ✅ {name} is ready (status: {status})")
                    return
        except requests.ConnectionError:
            pass
        time.sleep(2)
    bail(f"{name} did not become ready within {timeout}s. Check Docker logs.")


def delete_index_if_exists(url: str, index: str) -> None:
    """Delete an index if it exists. Silently succeeds on 404."""
    try:
        resp = requests.delete(f"{url}/{index}", timeout=10)
        if resp.status_code in (200, 404):
            return
        print(f"  ⚠️  Unexpected status deleting '{index}' on {url}: {resp.status_code}")
    except requests.RequestException as exc:
        print(f"  ⚠️  Error deleting index '{index}' on {url}: {exc}")


def create_index(url: str, index: str) -> None:
    """Create index with geonames mapping."""
    resp = requests.put(
        f"{url}/{index}",
        json=GEONAMES_INDEX_BODY,
        headers={"Content-Type": "application/json"},
        timeout=30,
    )
    if resp.status_code not in (200, 201):
        bail(f"Failed to create index '{index}' on {url}: {resp.status_code} {resp.text[:300]}")
    print(f"  ✅ Index '{index}' created on {url}")


def get_doc_count(url: str, index: str) -> int:
    """Refresh + count. The eternal dance of eventual consistency."""
    try:
        requests.post(f"{url}/{index}/_refresh", timeout=30)
        resp = requests.get(f"{url}/{index}/_count", timeout=10)
        if resp.ok:
            return resp.json().get("count", 0)
    except requests.RequestException:
        pass
    return 0


# Stall detection: if the count hasn't changed for this many consecutive polls,
# the pipeline is probably done (or dead) and we're just wasting time waiting.
STALL_THRESHOLD_POLLS = 6  # 6 polls × 5s = 30s of no movement before we call it


def poll_until_doc_count(
    url: str,
    index: str,
    expected_count: int,
    timeout: int,
    step_name: str,
) -> int:
    """
    Poll _refresh + _count until doc count >= expected_count.
    Prints progress updates. Returns final count. Bails on timeout or stall.
    """
    print(f"  ⏳ Polling {step_name} for {expected_count:,} docs...")
    start = time.time()
    last_count = 0
    stall_polls = 0

    while (time.time() - start) < timeout:
        current_count = get_doc_count(url, index)

        if current_count != last_count:
            elapsed = time.time() - start
            pct = (current_count / expected_count * 100) if expected_count > 0 else 0
            print(
                f"     📊 {current_count:>12,} / {expected_count:,} docs "
                f"({pct:5.1f}%)  [{elapsed:.0f}s elapsed]",
                end="\r",
            )
            last_count = current_count
            stall_polls = 0
        else:
            stall_polls += 1

        if current_count >= expected_count:
            elapsed = time.time() - start
            print(
                f"\n  ✅ {step_name} complete: {current_count:,} docs "
                f"in {format_duration(elapsed)}"
            )
            return current_count

        # Stall detection: count hasn't budged and we're past initial startup
        if stall_polls >= STALL_THRESHOLD_POLLS and current_count > 0:
            missing = expected_count - current_count
            elapsed = time.time() - start
            print(
                f"\n  ⚠️  {step_name} stalled: {current_count:,} / {expected_count:,} docs "
                f"({missing:,} missing) — no change for {stall_polls * POLL_INTERVAL_SECS}s. "
                f"Pipeline may have finished with rejected documents."
            )
            return current_count

        time.sleep(POLL_INTERVAL_SECS)

    # Timeout
    elapsed = time.time() - start
    print(
        f"\n  💀 {step_name} timed out after {format_duration(elapsed)}. "
        f"Last count: {last_count:,} / {expected_count:,}"
    )
    return last_count


# ============================================================================
#  Summary & Output
# ============================================================================


def print_summary(results: list[StepResult], es_count: int, os_count: int) -> None:
    """Pretty-print the timing summary table."""
    print("\n")
    print("┌──────────────────────────────┬────────────┬──────────────┬─────────┐")
    print("│ Step                         │   Duration │    Doc Count │ Status  │")
    print("├──────────────────────────────┼────────────┼──────────────┼─────────┤")
    for r in results:
        status = "  ✅" if r.success else "  ❌"
        duration = format_duration(r.duration_secs)
        print(
            f"│ {r.name:<28} │ {duration:>10} │ {r.doc_count:>12,} │ {status:<7} │"
        )
    print("├──────────────────────────────┼────────────┼──────────────┼─────────┤")
    total_secs = sum(r.duration_secs for r in results)
    match_str = "  ✅" if es_count == os_count else "  ❌"
    print(
        f"│ {'Total':28} │ {format_duration(total_secs):>10} │ {'':>12} │ {match_str:<7} │"
    )
    print("└──────────────────────────────┴────────────┴──────────────┴─────────┘")
    print(f"\n  🎯 Validation: ES={es_count:,}  OS={os_count:,}  ", end="")
    if es_count == os_count:
        print("Match ✅")
    else:
        print(f"Delta={abs(es_count - os_count):,} ❌")


def format_duration(secs: float) -> str:
    """Format seconds into a human-readable string."""
    if secs < 60:
        return f"{secs:.1f}s"
    minutes = int(secs // 60)
    remaining = secs % 60
    return f"{minutes}m {remaining:.1f}s"


# ============================================================================
#  Signal Handling
# ============================================================================


_cleanup_registered = False


def register_cleanup_handler() -> None:
    """Register Ctrl+C handler that offers to tear down containers."""
    global _cleanup_registered
    if _cleanup_registered:
        return
    _cleanup_registered = True

    def handler(signum, frame):
        print("\n\n  ⚠️  Interrupted!")
        answer = input("  Tear down demo containers? [Y/n]: ").strip().lower()
        if answer in ("", "y", "yes"):
            print("  🛑 Stopping containers...")
            try:
                stop_demo_containers()
                print("  ✅ Containers removed.")
            except Exception as exc:
                print(f"  💀 Cleanup failed: {exc}")
        sys.exit(130)

    signal.signal(signal.SIGINT, handler)


# ============================================================================
#  Utilities
# ============================================================================


def bail(msg: str) -> None:
    """Print error and exit."""
    print(f"\n  💀 {msg}")
    sys.exit(1)


# ============================================================================
#  Main
# ============================================================================


def main() -> None:
    print("=" * 60)
    print("  🚀 Kravex End-to-End Demo")
    print("  Pipeline: geonames.json → Elasticsearch → OpenSearch")
    print("=" * 60)

    register_cleanup_handler()
    results: list[StepResult] = []

    # ── Pre-flight ──────────────────────────────────────────────
    print("\n📋 Pre-flight checks:")
    check_dataset_exists()
    check_docker_available()

    # ── Infrastructure ──────────────────────────────────────────
    if is_demo_stack_running():
        print("\n✅ Demo stack already running — skipping container startup")
    else:
        print("\n🔌 Checking port availability:")
        check_port_conflicts()
        start_demo_containers()
        print("\n⏳ Waiting for clusters to be ready:")
        wait_for_cluster(ES_URL, "Elasticsearch")
        wait_for_cluster(OS_URL, "OpenSearch")

    # ── Leg 1: File → Elasticsearch ─────────────────────────────
    print("\n" + "─" * 60)
    print("  📂 Leg 1: geonames.json → Elasticsearch")
    print("─" * 60)

    delete_index_if_exists(ES_URL, INDEX_NAME)
    create_index(ES_URL, INDEX_NAME)

    print("\n  👉 Run this in another terminal:")
    print("     ./demo/file_to_esdb.sh")
    input("\n  Press Enter when you've started the script...")

    step1 = StepResult(name="File → Elasticsearch")
    count1 = poll_until_doc_count(ES_URL, INDEX_NAME, EXPECTED_DOC_COUNT, MAX_WAIT_SECS, "File → ES")
    step1.finish(count1)
    results.append(step1)

    # ── Leg 2: Elasticsearch → OpenSearch ───────────────────────
    print("\n" + "─" * 60)
    print("  📡 Leg 2: Elasticsearch → OpenSearch")
    print("─" * 60)

    delete_index_if_exists(OS_URL, INDEX_NAME)
    create_index(OS_URL, INDEX_NAME)

    print("\n  👉 Run this in another terminal:")
    print("     ./demo/esdb_to_osdb.sh")
    input("\n  Press Enter when you've started the script...")

    step2 = StepResult(name="Elasticsearch → OpenSearch")
    count2 = poll_until_doc_count(OS_URL, INDEX_NAME, EXPECTED_DOC_COUNT, MAX_WAIT_SECS, "ES → OS")
    step2.finish(count2)
    results.append(step2)

    # ── Validation & Summary ────────────────────────────────────
    es_count = get_doc_count(ES_URL, INDEX_NAME)
    os_count = get_doc_count(OS_URL, INDEX_NAME)
    print_summary(results, es_count, os_count)

    # ── Cleanup ─────────────────────────────────────────────────
    print()
    answer = input("  🗑️  Tear down demo containers? [Y/n]: ").strip().lower()
    if answer in ("", "y", "yes"):
        print("  🛑 Stopping containers...")
        stop_demo_containers()
        print("  ✅ Containers removed.")
    else:
        print(
            "  ℹ️  Containers left running. Stop with:\n"
            "     docker compose -f demo/docker-compose-demo.yml down -v"
        )

    print("\n  🎬 Demo complete. That's a wrap.\n")


if __name__ == "__main__":
    main()
