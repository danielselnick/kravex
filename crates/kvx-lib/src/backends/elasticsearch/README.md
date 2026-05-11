
# Elasticsearch Backend

Elasticsearch-specific Source and Sink implementations.

## Source

Reads documents from Elasticsearch using **PIT (Point In Time) + search_after** pagination.

- Opens a PIT snapshot on the configured index at first `pump()` call
- Issues `_search` requests with `search_after` cursors for deep pagination
- Returns raw `_search` response envelopes as `Barrel` — downstream caster (PitToBulk) extracts hits
- Closes the PIT on exhaustion (best-effort — PIT auto-expires after `keep_alive`)
- Sort order: `_doc` ascending (most efficient for bulk reads)
- Requires ES 7.10+ (PIT API)

### Startup validation
1. HTTP client construction with 10s connect / 30s response timeouts
2. Cluster connectivity ping (GET root URL with auth)
3. Source index existence check (HEAD request)

## Sink

Writes documents to Elasticsearch via the **`_bulk` API**. Pre-computes the bulk URL and auth header at construction time for zero-allocation-per-request hot path.

### Bulk response validation & per-item retry
The `_bulk` API returns HTTP 200 even when individual documents fail (mapping errors, version conflicts, shard failures). The sink parses the response body and checks the top-level `errors` boolean. When `errors: true`, it extracts only the failed NDJSON action/doc pairs and retries them — up to 10 rounds. Each round shrinks the payload to only the rejects. Successful docs are never re-sent.

- **Per-item retry**: Failed docs extracted by index correlation (NDJSON pair `i` ↔ response item `i`), re-assembled into a smaller retry payload
- **No duplicates**: Only failed pairs are retried — critical for File→ES flows with auto-generated `_id`s
- **Fast path**: When `errors: false`, item-level parsing is skipped entirely (zero overhead on happy path)
- **Empty body**: Treated as success (compatibility with proxies/mocks)
- **Malformed body**: Non-JSON 200 responses cause an error (detects reverse-proxy HTML error pages)
- **Safety bail**: If NDJSON line count doesn't match `2 × response items`, retry is skipped (can't safely correlate)

## Config

### ElasticsearchSourceConfig
| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `url` | `String` | yes | Cluster URL (scheme + host + port) |
| `index` | `String` | yes | Source index to read from |
| `username` | `Option<String>` | no | Basic auth username |
| `password` | `Option<String>` | no | Basic auth password |
| `api_key` | `Option<String>` | no | API key auth (takes priority over basic) |
| `common_config` | `CommonSourceConfig` | no | Batch sizing (max_batch_size_docs, max_batch_size_bytes) |

### ElasticsearchSinkConfig
| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `url` | `String` | yes | Cluster URL |
| `index` | `Option<String>` | no | Default target index (per-doc `_index` can override) |
| `username` | `Option<String>` | no | Basic auth username |
| `password` | `Option<String>` | no | Basic auth password |
| `api_key` | `Option<String>` | no | API key auth (priority over basic) |
| `common_config` | `CommonSinkConfig` | no | Request sizing (max_request_size_bytes) |

## Key Concepts

- **PIT**: Consistent snapshot for pagination, avoids deep-pagination overhead
- **search_after**: Cursor-based pagination using sort values from previous response
- **`_bulk` API**: Batch document indexing via NDJSON action/document pairs
- **Bulk response validation**: HTTP 200 ≠ all docs indexed — per-item error checking required
- **Auth hierarchy**: API key > basic auth > anonymous
- **PIT ID rotation**: PIT id may change between responses — always use the latest from the response

## Knowledge Graph

```
ElasticsearchSource → Source trait → SourceBackend::Elasticsearch
ElasticsearchSource.pump() → PIT open → _search loop → PIT close → EOF
ElasticsearchSink → Sink trait → SinkBackend::Elasticsearch
ElasticsearchSink.submit_bulk_request() → send_bulk_post() → parse BulkResponse → extract_failed_pairs() → retry loop (max 10)
send_bulk_post() → HTTP POST + auth + ndjson content-type → Result<String> (response body)
extract_failed_pairs() → correlate NDJSON lines to response items by index → rebuild payload from failed pairs only
BulkResponse → { errors: bool, items: Vec<BulkItemWrapper> } — serde types for _bulk response parsing
BulkItemResult → { status: u16, error: Option<BulkItemError> } — per-document outcome
ElasticsearchSourceConfig → CommonSourceConfig (embedded) → max_batch_size_docs, max_batch_size_bytes
ElasticsearchSinkConfig → CommonSinkConfig (embedded) → max_request_size_bytes
PIT + search_after → Barrel (raw _search envelope) → PitToBulk caster → Draft → Manifold → Sink
_bulk API ← Payload (NDJSON action+doc pairs) ← NdjsonManifold ← Draft ← PitToBulk
```
