# Meilisearch Backend

Meilisearch-specific Sink implementation for document ingestion.

## Sink

Writes documents to Meilisearch via `POST /indexes/{index_uid}/documents` with JSON array payloads. Polls `GET /tasks/{taskUid}` until the async task reaches a terminal state ("succeeded" or "failed").

## Config

`MeilisearchSinkConfig` — connection configuration: host URL, Bearer token API key, target index UID.

## Key Concepts

- **JSON Array Payload**: Meilisearch accepts `[doc1,doc2,...]` — no NDJSON, no bulk action lines
- **Async Tasks**: Document POST returns 202 Accepted with a `taskUid`; actual indexing is asynchronous
- **Task Polling**: Sink polls `GET /tasks/{taskUid}` until status is "succeeded" or "failed"
- **Bearer Token Auth**: `Authorization: Bearer {api_key}` header when API key is configured
- **Auto-Create Index**: Meilisearch auto-creates indices on first document POST if they don't exist
- **Pre-computed URLs**: Documents and tasks URLs are built once at construction for zero-alloc hot path

## Knowledge Graph

```
MeilisearchSink → Sink trait → SinkBackend::Meilisearch
MeilisearchSinkConfig → CommonSinkConfig (embedded)
POST /indexes/{uid}/documents ← payloads (JSON arrays)
GET /tasks/{taskUid} → poll until succeeded/failed
JsonArrayManifold → joins entries as [doc1,doc2,...]
NdJsonSplit caster → File→Meilisearch (splits NDJSON lines into entries)
PitToJson caster → ES→Meilisearch (extracts _source from PIT hits)
```
