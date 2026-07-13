#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
catalog="$repo_root/src/providers/catalog.json"
models_url="${MODELS_DEV_API_URL:-https://models.dev/api.json}"

report="$({ curl -fsSL "$models_url"; } | jq --slurpfile catalog "$catalog" '
  def norm: ascii_downcase | gsub("_"; "-");
  def trimurl: rtrimstr("/") | ascii_downcase;

  $catalog[0].providers as $providers |
  [to_entries[] | . as $item |
    ($item.value.api // null) as $api |
    ($providers | map(select(
      ((.id | norm) == ($item.key | norm)) or
      (.aliases | any((. | norm) == ($item.key | norm))) or
      (.agentAliases | any(
        (.agent | norm) == "opencode" and
        ((.alias | norm) == ($item.key | norm))
      ))
    )) | first) as $matched |
    {
      id: $item.key,
      api: $api,
      dynamicApi: ($api != null and ($api | contains("$"))),
      canonical: ($matched.id // null),
      endpointMatched: (
        if $api == null or ($api | contains("$")) then null
        else ([
          ($matched.endpoints[]? | .baseUrl? // empty),
          ($matched.endpoints[]?.baseUrlAliases[]?)
        ] | map(select(type == "string")) | any((. | trimurl) == ($api | trimurl)))
        end
      )
    }
  ] as $rows |
  {
    modelsDevEntries: ($rows | length),
    identityMatches: ($rows | map(select(.canonical != null)) | length),
    staticApiEntries: ($rows | map(select(.api != null and (.dynamicApi | not))) | length),
    staticApiMatches: ($rows | map(select(.endpointMatched == true)) | length),
    dynamicApiEntries: ($rows | map(select(.dynamicApi)) | length),
    missingIdentity: ($rows | map(select(.canonical == null) | .id)),
    missingStaticEndpoint: ($rows | map(select(
      .api != null and (.dynamicApi | not) and .endpointMatched != true
    ) | .id))
  }
')"

printf '%s\n' "$report" | jq .

missing_identity="$(printf '%s\n' "$report" | jq '.missingIdentity | length')"
missing_endpoint="$(printf '%s\n' "$report" | jq '.missingStaticEndpoint | length')"
if [[ "$missing_identity" -ne 0 || "$missing_endpoint" -ne 0 ]]; then
  exit 1
fi
