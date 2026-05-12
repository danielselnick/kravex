#!/usr/bin/env python3
"""
Kravex End-to-End Demo — Orchestrator
======================================
Manages infrastructure and polls for kravex completion.
The actual kravex execution lives in standalone shell scripts:
  - file_to_esdb.sh  (Leg 1: dataset.json → Elasticsearch)
  - esdb_to_osdb.sh  (Leg 2: Elasticsearch → OpenSearch)

Supports multiple datasets: geonames (11.4M), noaa (33.6M), pmc (574K).

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

from terminal_launcher import detect_terminal, launch_in_terminal

# ============================================================================
#  Constants
# ============================================================================

PROJECT_ROOT = Path(__file__).resolve().parent.parent
DEMO_DIR = PROJECT_ROOT / "demo"
DATASETS_DIR = PROJECT_ROOT / "datasets"
COMPOSE_FILE = DEMO_DIR / "docker-compose-demo.yml"

ES_URL = "http://localhost:19200"
OS_URL = "http://localhost:19201"

DEMO_PORTS = {19200: "Elasticsearch", 19201: "OpenSearch"}
DEMO_CONTAINER_NAMES = {"kravex-demo-es", "kravex-demo-os"}

# Polling config
POLL_INTERVAL_SECS = 5
MAX_WAIT_SECS = 1800  # 30 minutes — noaa is 33.6M docs, give it time

# Content sampling
CONTENT_SAMPLE_SIZE = 20

# Abbreviated mode
ABBREVIATED_FRACTION = 0.25

# ============================================================================
#  Dataset Registry
# ============================================================================

DATASETS = {
    "geonames": {
        "url": "https://rally-tracks.elastic.co/geonames/documents-2.json.bz2",
        "expected_docs": 11_396_503,
        "file": "geonames.json",
        "description": "Geographic features (cities, mountains, lakes)",
    },
    "noaa": {
        "url": "https://rally-tracks.elastic.co/noaa/documents.json.bz2",
        "expected_docs": 33_659_481,
        "file": "noaa.json",
        "description": "NOAA weather station observations",
    },
    "pmc": {
        "url": "https://rally-tracks.elastic.co/pmc/documents.json.bz2",
        "expected_docs": 574_199,
        "file": "pmc.json",
        "description": "PubMed Central journal articles",
    },
}

# ============================================================================
#  Index Mappings — per-dataset, tuned for ingest throughput
# ============================================================================
# Settings: single shard, no replicas, refresh disabled for max ingest speed.

INDEX_MAPPINGS = {
    "geonames": {
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
    },
    "noaa": {
        "settings": {
            "number_of_shards": 1,
            "number_of_replicas": 0,
            "refresh_interval": "-1",
        },
        "mappings": {
            "properties": {
                "date": {"type": "date", "format": "yyyy-MM-dd'T'HH:mm:ss"},
                "TMIN": {"type": "float"},
                "TMAX": {"type": "float"},
                "TAVG": {"type": "float"},
                "TOBS": {"type": "float"},
                "PRCP": {"type": "float"},
                "SNOW": {"type": "keyword"},
                "SNWD": {"type": "keyword"},
                "WESD": {"type": "float"},
                "TRANGE": {"type": "float_range"},
                "station": {
                    "properties": {
                        "name": {"type": "text"},
                        "id": {"type": "keyword"},
                        "state": {"type": "keyword"},
                        "state_code": {"type": "keyword"},
                        "country": {"type": "keyword"},
                        "country_code": {"type": "keyword"},
                        "elevation": {"type": "float"},
                        "location": {"type": "geo_point"},
                    }
                },
            }
        },
    },
    "pmc": {
        "settings": {
            "number_of_shards": 1,
            "number_of_replicas": 0,
            "refresh_interval": "-1",
        },
        "mappings": {
            "properties": {
                "name": {"type": "keyword"},
                "journal": {"type": "text"},
                "date": {"type": "text"},
                "volume": {"type": "keyword"},
                "issue": {"type": "keyword"},
                "accession": {"type": "keyword"},
                "timestamp": {"type": "date", "format": "yyyy-MM-dd HH:mm:ss"},
                "pmid": {"type": "keyword"},
                "body": {"type": "text"},
            }
        },
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
#  Dataset Selection & Download
# ============================================================================


def select_demo_mode() -> bool:
    """Ask user whether to run abbreviated (25%) or full dataset. Returns True for abbreviated."""
    print("\n📦 Demo size:")
    print("  [1] Quick demo — 25% of dataset (default)")
    print("  [2] Full dataset — 100%")
    choice = input("\n  Choice [1]: ").strip()
    if choice in ("", "1"):
        return True
    if choice == "2":
        return False
    bail(f"Invalid selection: '{choice}'. Expected 1 or 2.")
    raise  # unreachable


def create_subset_file(dataset_name: str) -> Path:
    """Create a 25% subset NDJSON file from the full dataset. Cached — skips if already exists."""
    info = DATASETS[dataset_name]
    full_path = DATASETS_DIR / info["file"]
    subset_path = DATASETS_DIR / f"{dataset_name}_25pct.json"

    if subset_path.exists():
        size_mb = subset_path.stat().st_size / (1024**2)
        print(f"  ✅ Subset already exists: {subset_path} ({size_mb:.0f} MB)")
        return subset_path

    total_lines = info["expected_docs"]
    target_lines = int(total_lines * ABBREVIATED_FRACTION)

    print(f"  ✂️  Creating 25% subset ({target_lines:,} lines) from {info['file']}...")
    lines_written = 0
    with open(full_path, "r") as src, open(subset_path, "w") as dst:
        for line in src:
            if lines_written >= target_lines:
                break
            dst.write(line)
            lines_written += 1

    size_mb = subset_path.stat().st_size / (1024**2)
    print(f"  ✅ Subset created: {subset_path} ({lines_written:,} docs, {size_mb:.0f} MB)")
    return subset_path


def select_dataset(abbreviated: bool) -> str:
    """Auto-detect available datasets, prompt user to choose one."""
    print("\n📂 Available datasets:")
    entries = []
    for i, (name, info) in enumerate(DATASETS.items(), 1):
        json_path = DATASETS_DIR / info["file"]
        bz2_path = DATASETS_DIR / f"{info['file']}.bz2"
        subset_path = DATASETS_DIR / f"{name}_25pct.json"

        full_docs = info["expected_docs"]
        display_docs = int(full_docs * ABBREVIATED_FRACTION) if abbreviated else full_docs
        docs_str = f"{display_docs:,}"

        if abbreviated and subset_path.exists():
            size_gb = subset_path.stat().st_size / (1024**3)
            status = f"✅ Ready ({size_gb:.1f} GB)"
        elif json_path.exists():
            size_gb = json_path.stat().st_size / (1024**3)
            if abbreviated:
                size_gb *= ABBREVIATED_FRACTION
            status = f"✅ Ready (~{size_gb:.1f} GB)"
        elif bz2_path.exists():
            status = "📦 Compressed (needs decompress)"
        else:
            status = "⬇️  Download available"

        entries.append(name)
        print(f"  [{i}] {name:<10} ({docs_str:>12} docs) — {status}")
        print(f"      {info['description']}")

    print()
    choice = input("  Select dataset [1]: ").strip()
    if choice == "" or choice == "1":
        return entries[0]
    try:
        idx = int(choice) - 1
        if 0 <= idx < len(entries):
            return entries[idx]
    except ValueError:
        # Maybe they typed the name
        if choice.lower() in DATASETS:
            return choice.lower()

    bail(f"Invalid selection: '{choice}'. Expected 1-{len(entries)} or a dataset name.")
    raise  # unreachable


def ensure_dataset_ready(dataset_name: str) -> Path:
    """Make sure the dataset JSON file exists. Offer download/decompress if not."""
    info = DATASETS[dataset_name]
    json_path = DATASETS_DIR / info["file"]
    bz2_path = DATASETS_DIR / f"{info['file']}.bz2"

    if json_path.exists():
        size_gb = json_path.stat().st_size / (1024**3)
        print(f"  ✅ Dataset found: {json_path} ({size_gb:.1f} GB)")
        return json_path

    if bz2_path.exists():
        print(f"  📦 Found {bz2_path} but not the decompressed .json")
        answer = input("     Decompress with bunzip2? [Y/n]: ").strip().lower()
        if answer in ("", "y", "yes"):
            print("     Decompressing... (this may take a few minutes)")
            subprocess.run(["bunzip2", "-k", str(bz2_path)], check=True)
            if json_path.exists():
                print("  ✅ Decompression complete.")
                return json_path
        bail("Decompression declined or failed.")

    # Neither file exists — offer download
    print(f"  ⬇️  Dataset not found locally: {info['file']}")
    print(f"     Source: {info['url']}")
    answer = input("     Download now? [Y/n]: ").strip().lower()
    if answer not in ("", "y", "yes"):
        bail("Download declined. Cannot proceed without dataset.")

    DATASETS_DIR.mkdir(parents=True, exist_ok=True)
    print(f"     Downloading {dataset_name}... (this may take a while)")
    try:
        subprocess.run(
            ["curl", "-L", "-#", "-o", str(bz2_path), info["url"]],
            check=True,
            timeout=3600,
        )
        print(f"  ✅ Download complete: {bz2_path}")
    except subprocess.CalledProcessError:
        bz2_path.unlink(missing_ok=True)
        bail("Download failed. Check your internet connection.")

    print("     Decompressing...")
    try:
        subprocess.run(["bunzip2", "-k", str(bz2_path)], check=True, timeout=600)
        print(f"  ✅ Ready: {json_path}")
        return json_path
    except subprocess.CalledProcessError:
        json_path.unlink(missing_ok=True)
        bail("Decompression failed.")

    raise  # unreachable


# ============================================================================
#  Pre-flight Checks
# ============================================================================


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

    print("\n  What would you like to do?")
    print("  [1] Leave them running and continue anyway (default)")
    print("  [2] Stop conflicting containers and continue")
    print("  [3] Exit")
    answer = input("\n  Choice [1]: ").strip()
    if answer in ("3",):
        bail("Port conflicts unresolved. Exiting.")
    if answer in ("", "1"):
        print("  ⚠️  Proceeding with conflicting containers — ports may collide")
        return

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
        resp = requests.delete(f"{url}/{index}", timeout=30)
        if resp.status_code in (200, 404):
            return
        print(f"  ⚠️  Unexpected status deleting '{index}' on {url}: {resp.status_code}")
    except requests.RequestException as exc:
        print(f"  ⚠️  Error deleting index '{index}' on {url}: {exc}")


def create_index(url: str, index: str, dataset_name: str) -> None:
    """Create index with dataset-specific mapping."""
    body = INDEX_MAPPINGS.get(dataset_name, {
        "settings": {
            "number_of_shards": 1,
            "number_of_replicas": 0,
            "refresh_interval": "-1",
        }
    })
    max_attempts = 3
    for attempt in range(1, max_attempts + 1):
        try:
            resp = requests.put(
                f"{url}/{index}",
                json=body,
                headers={"Content-Type": "application/json"},
                timeout=60,
            )
            if resp.status_code not in (200, 201):
                bail(f"Failed to create index '{index}' on {url}: {resp.status_code} {resp.text[:300]}")
            print(f"  ✅ Index '{index}' created on {url}")
            return
        except requests.RequestException as exc:
            if attempt < max_attempts:
                print(f"  ⚠️  Index creation timed out (attempt {attempt}/{max_attempts}), retrying...")
                time.sleep(5)
            else:
                bail(f"Failed to create index '{index}' on {url} after {max_attempts} attempts: {exc}")


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
                "\n" + "─" * 60
            )
            print(
                f"  ⚠️  {step_name} STALLED"
            )
            print(
                f"     {current_count:,} / {expected_count:,} docs — "
                f"({missing:,} missing)"
            )
            print(
                f"     No change for {stall_polls * POLL_INTERVAL_SECS}s. "
            )
            print(
                "     Pipeline may have finished or failed. Proceeding with partial count."
            )
            print("─" * 60)
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


def validate_content(
    es_url: str,
    os_url: str,
    index: str,
    sample_size: int,
) -> bool:
    """
    Sample random docs from ES, fetch by _id from OS, compare _source fields.
    Returns True if all sampled docs match, False otherwise.
    """
    print(f"\n  🔬 Sampling {sample_size} docs from {es_url}/{index} for content validation...")

    try:
        resp = requests.get(
            f"{es_url}/{index}/_search",
            params={"size": sample_size, "_source": "true"},
            timeout=30,
        )
        if not resp.ok:
            print(f"  ⚠️  Could not sample from ES: {resp.status_code}")
            return True  # skip, don't fail
        es_hits = resp.json().get("hits", {}).get("hits", [])
    except requests.RequestException as e:
        print(f"  ⚠️  ES content sampling failed: {e}")
        return True

    if not es_hits:
        print("  ⚠️  No docs returned from ES for sampling — skipping content validation")
        return True

    mismatches = 0
    checked = 0

    for hit in es_hits:
        doc_id = hit.get("_id")
        es_source = hit.get("_source", {})
        if not doc_id:
            continue

        # Fetch same doc from OS by _id
        try:
            os_resp = requests.get(
                f"{os_url}/{index}/_doc/{doc_id}",
                timeout=15,
            )
            if not os_resp.ok:
                print(f"  ⚠️  Doc {doc_id} not found in OS (HTTP {os_resp.status_code})")
                mismatches += 1
                continue

            os_doc = os_resp.json()
            os_source = os_doc.get("_source", {})
            checked += 1

            # Compare _source fields — if both are empty that's fine, but
            # if ES has fields that OS doesn't, that's a problem.
            if es_source != os_source:
                # Find the specific fields that differ
                es_keys = set(es_source.keys())
                os_keys = set(os_source.keys())
                missing_in_os = es_keys - os_keys
                extra_in_os = os_keys - es_keys
                common = es_keys & os_keys
                differing_values = {
                    k: (es_source[k], os_source[k])
                    for k in common
                    if es_source[k] != os_source[k]
                }

                print(f"  ❌ Doc {doc_id}: _source mismatch")
                if missing_in_os:
                    print(f"     Fields missing in OS: {missing_in_os}")
                if extra_in_os:
                    print(f"     Extra fields in OS: {extra_in_os}")
                if differing_values:
                    for k, (ev, ov) in differing_values.items():
                        print(f"     Field '{k}': ES={ev}  OS={ov}")
                mismatches += 1

        except requests.RequestException as e:
            print(f"  ⚠️  Could not fetch doc {doc_id} from OS: {e}")
            mismatches += 1

    if mismatches == 0:
        print(f"  ✅ Content validation passed: {checked}/{checked} docs match exactly \n")
        return True
    else:
        print(f"  ❌ Content validation: {mismatches}/{checked + mismatches} docs mismatched or missing\n")
        return False


def print_summary(
    results: list[StepResult],
    dataset_name: str,
    es_count: int,
    os_count: int,
    expected_docs: int,
) -> None:
    """Pretty-print the timing summary table with validation against expected."""
    print("\n")
    print("┌──────────────────────────────┬────────────┬──────────────┬──────────────┐")
    print("│ Step                         │   Duration │    Doc Count │  vs Expected │")
    print("├──────────────────────────────┼────────────┼──────────────┼──────────────┤")
    for r in results:
        status = "  ✅" if r.success else "  ❌"
        duration = format_duration(r.duration_secs)
        if r.doc_count >= expected_docs:
            vs_expected = "✅ 100%"
        else:
            pct = (r.doc_count / expected_docs * 100) if expected_docs > 0 else 0
            vs_expected = f"❌ {pct:.1f}%"
        print(
            f"│ {r.name:<28} │ {duration:>10} │ {r.doc_count:>12,} │ {vs_expected:>12} │"
        )
    print("├──────────────────────────────┼────────────┼──────────────┼──────────────┤")
    total_secs = sum(r.duration_secs for r in results)
    match_str = "  ✅" if es_count == os_count else "  ❌"
    print(
        f"│ {'Total':28} │ {format_duration(total_secs):>10} │ {'':>12} │ {match_str:<12} │"
    )
    print("└──────────────────────────────┴────────────┴──────────────┴──────────────┘")

    print(f"\n  🎯 Expected: {expected_docs:,} docs")
    print(f"     ES count: {es_count:,}  ", end="")
    if es_count >= expected_docs:
        print("✅", end="")
    else:
        print("❌", end="")
    print(f"  OS count: {os_count:,}  ", end="")
    if os_count >= expected_docs:
        print("✅", end="")
    else:
        print("❌", end="")
    print()

    both_full = es_count >= expected_docs and os_count >= expected_docs
    count_match = es_count == os_count

    if both_full and count_match:
        print("  🎉 Full validation PASSED ✅ — all docs present and accounted for")
    elif count_match and not both_full:
        print(f"  ⚠️  Counts match ({es_count:,}) but both are below expected ({expected_docs:,})")
        print(f"  ❌ Partial migration detected — some docs did not make it through")
    else:
        print(f"  ❌ Count mismatch: ES={es_count:,} vs OS={os_count:,} (delta={abs(es_count - os_count):,})")


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
    print("  Pipeline: dataset.json → Elasticsearch → OpenSearch")
    print("=" * 60)

    register_cleanup_handler()
    results: list[StepResult] = []

    # ── Mode & Dataset Selection ───────────────────────────────
    abbreviated = select_demo_mode()
    dataset_name = select_dataset(abbreviated)
    dataset_info = DATASETS[dataset_name]
    full_docs = dataset_info["expected_docs"]
    expected_docs = int(full_docs * ABBREVIATED_FRACTION) if abbreviated else full_docs
    index_name = dataset_name
    # For leg scripts: abbreviated uses geonames_25pct config, full uses geonames
    leg1_dataset_arg = f"{dataset_name}_25pct" if abbreviated else dataset_name

    mode_label = "25%" if abbreviated else "100%"
    print(f"\n  📦 Selected: {dataset_name} ({expected_docs:,} docs, {mode_label})")

    # ── Pre-flight ──────────────────────────────────────────────
    print("\n📋 Pre-flight checks:")
    ensure_dataset_ready(dataset_name)
    if abbreviated:
        create_subset_file(dataset_name)
    check_docker_available()

    # ── Infrastructure ──────────────────────────────────────────
    if is_demo_stack_running():
        print("\n✅ Demo stack already running — skipping container startup")
    else:
        print("\n🔌 Checking port availability:")
        check_port_conflicts()
        start_demo_containers()
        print("\n⏳ Waiting for clusters to be ready:")
        wait_for_cluster(ES_URL, "Elasticsearch", timeout=240)
        wait_for_cluster(OS_URL, "OpenSearch", timeout=240)

    # ── Leg 1: File → Elasticsearch ─────────────────────────────
    print("\n" + "─" * 60)
    print(f"  📂 Leg 1: {dataset_name}.json → Elasticsearch")
    print("─" * 60)

    delete_index_if_exists(ES_URL, index_name)
    create_index(ES_URL, index_name, dataset_name)

    launched = launch_in_terminal(
        script="./demo/file_to_esdb.sh",
        args=[leg1_dataset_arg],
        cwd=PROJECT_ROOT,
    )
    if launched:
        print(f"  ✅ Launched ./demo/file_to_esdb.sh {leg1_dataset_arg} in new terminal window")
        input("\n  Press Enter when the script has finished...")
    else:
        print("\n  👉 Run this in another terminal:")
        print(f"     ./demo/file_to_esdb.sh {leg1_dataset_arg}")
        input("\n  Press Enter when you've started the script...")

    step1 = StepResult(name="File → Elasticsearch")
    count1 = poll_until_doc_count(ES_URL, index_name, expected_docs, MAX_WAIT_SECS, "File → ES")
    step1.finish(count1)
    results.append(step1)

    # ── Validate Leg 1 result ─────────────────────────────────────
    if count1 < expected_docs:
        print()
        print("─" * 60)
        print(f"  ⚠️  Leg 1 incomplete: {count1:,} / {expected_docs:,} docs")
        missing = expected_docs - count1
        if count1 == 0:
            print("  💀 No documents were indexed. This usually means kvx-cli failed.")
            print("     Check the terminal window where file_to_esdb.sh ran.")
            bail("Cannot proceed — Leg 1 produced no data.")
        else:
            print(f"  ⚠️  {missing:,} docs missing — pipeline stalled or some docs rejected.")
            answer = input("  Proceed to Leg 2 anyway? [y/N]: ").strip().lower()
            if answer not in ("y", "yes"):
                bail("Demo aborted — Leg 1 did not complete.")
        print("─" * 60)

    # ── Leg 2: Elasticsearch → OpenSearch ───────────────────────
    print("\n" + "─" * 60)
    print(f"  📡 Leg 2: Elasticsearch → OpenSearch ({dataset_name})")
    print("─" * 60)

    delete_index_if_exists(OS_URL, index_name)
    create_index(OS_URL, index_name, dataset_name)

    launched = launch_in_terminal(
        script="./demo/esdb_to_osdb.sh",
        args=[dataset_name],
        cwd=PROJECT_ROOT,
    )
    if launched:
        print(f"  ✅ Launched ./demo/esdb_to_osdb.sh {dataset_name} in new terminal window")
        input("\n  Press Enter when the script has finished...")
    else:
        print("\n  👉 Run this in another terminal:")
        print(f"     ./demo/esdb_to_osdb.sh {dataset_name}")
        input("\n  Press Enter when you've started the script...")

    step2 = StepResult(name="Elasticsearch → OpenSearch")
    count2 = poll_until_doc_count(OS_URL, index_name, expected_docs, MAX_WAIT_SECS, "ES → OS")
    step2.finish(count2)
    results.append(step2)

    # ── Validate Leg 2 result ─────────────────────────────────────
    if count2 < expected_docs:
        print()
        print("─" * 60)
        print(f"  ⚠️  Leg 2 incomplete: {count2:,} / {expected_docs:,} docs")
        missing = expected_docs - count2
        print(f"     {missing:,} docs missing from OpenSearch. Pipeline may have stalled.")
        if count2 == 0:
            print("  💀 No documents were migrated to OpenSearch. Check the esdb_to_osdb terminal.")
        print("─" * 60)

    # ── Final Counts ────────────────────────────────────────────
    es_count = get_doc_count(ES_URL, index_name)
    os_count = get_doc_count(OS_URL, index_name)

    # ── Content Validation ──────────────────────────────────────
    content_ok = True
    if es_count > 0 and os_count > 0:
        content_ok = validate_content(ES_URL, OS_URL, index_name, CONTENT_SAMPLE_SIZE)

    # ── Summary ─────────────────────────────────────────────────
    print_summary(results, dataset_name, es_count, os_count, expected_docs)

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
