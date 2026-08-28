#!/usr/bin/env bash
# 🔧 Reset an Elasticsearch index — delete and recreate it fresh.
# Usage: ./reset_index.sh [index_name] [es_url]
#   index_name  — defaults to "test-123"
#   es_url      — defaults to "http://localhost:9200"

set -euo pipefail

INDEX="${1:-test-123}"
ES_URL="${2:-http://localhost:9200}"

echo "🗑️  Deleting index: ${INDEX}"
curl -s -X DELETE "${ES_URL}/${INDEX}" | python3 -m json.tool 2>/dev/null || true

echo "🚀 Creating index: ${INDEX}"
curl -s -X PUT "${ES_URL}/${INDEX}" \
  -H 'Content-Type: application/json' \
  -d '{
  "settings": {
    "number_of_replicas": 0,
    "refresh_interval": "-1"
  },
  "mappings": {
    "properties": {
      "geonameid":      { "type": "integer" },
      "name":           { "type": "text" },
      "asciiname":      { "type": "text" },
      "alternatenames": { "type": "text" },
      "feature_class":  { "type": "keyword" },
      "feature_code":   { "type": "keyword" },
      "country_code":   { "type": "keyword" },
      "cc2":            { "type": "keyword" },
      "admin1_code":    { "type": "keyword" },
      "admin2_code":    { "type": "keyword" },
      "admin3_code":    { "type": "keyword" },
      "admin4_code":    { "type": "keyword" },
      "population":     { "type": "integer" },
      "dem":            { "type": "keyword" },
      "timezone":       { "type": "keyword" },
      "location":       { "type": "geo_point" }
    }
  }
}' | python3 -m json.tool

echo "✅ Index '${INDEX}' reset."
