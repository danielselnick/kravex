# Meilisearch Backend

Meilisearch-specific Sink implementation using raw `reqwest` + `flate2` gzip compression. Fire-and-forget — no SDK, no task polling.

## Sink

Writes documents via `POST /indexes/{uid}/documents` with gzip-compressed JSON array payloads. Pre-computes the documents URL and Bearer auth header at construction time for zero per-request allocation.

## Config

`MeilisearchSinkConfig` — connection configuration: host URL, Bearer token API key, target index UID.

## Key Concepts

- **Raw Reqwest**: Uses workspace `reqwest 0.13` directly — no SDK dependency conflicts, single reqwest compilation
- **Gzip Compression**: `flate2::GzEncoder` compresses payload before POST — `Content-Encoding: gzip` header
- **Fire-and-Forget**: Meilisearch returns 202 with `taskUid` — we don't poll, don't wait, don't look back
- **Pre-computed URL + Auth**: Documents URL and Bearer header computed once in `new()`, reused on every `send()`
- **Bearer Token Auth**: `Authorization: Bearer {api_key}` injected on all requests (health, index check, document POST)
- **Primary Key**: Optional `primary_key` config field — appends `?primaryKey={field}` to documents URL. Required for datasets without a top-level `*id` field (e.g., NOAA). Omit for datasets with natural `*id` fields (e.g., geonames has `geonameid`)
- **Auto-Create Index**: Meilisearch auto-creates indices on first document POST if they don't exist
- **JSON Array Payload**: Meilisearch accepts `[doc1,doc2,...]` — no NDJSON, no bulk action lines
- **Health + Index Checks**: Constructor pings `/health` and `/indexes/{uid}` to validate connectivity

## Knowledge Graph

```
MeilisearchSink → Sink trait → SinkBackend::Meilisearch
MeilisearchSinkConfig → CommonSinkConfig (embedded)
reqwest::Client → health check, index check, document POST
flate2::GzEncoder → gzip compression before POST
JsonArrayManifold → joins entries as [doc1,doc2,...]
NdJsonSplit caster → File→Meilisearch (splits NDJSON lines into entries)
PitToJson caster → ES→Meilisearch (extracts _source from PIT hits)
```
