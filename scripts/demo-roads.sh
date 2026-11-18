#!/usr/bin/env bash
# Fetch the road networks every routing demo runs on, with the engine itself,
# and write each to <out>/roads-<cache file>.gz: the release assets the plugin
# installs into its road cache (engine_bin.DEMO_ROADS pins their SHA-256).
# CI runs exactly this script; run it locally to see what a release would ship.
#
#   scripts/demo-roads.sh engine/target/release/solverforge-workbench dist
set -euo pipefail

engine=${1:?usage: demo-roads.sh <engine binary> <out dir>}
out=${2:?usage: demo-roads.sh <engine binary> <out dir>}
cache=$(mktemp -d)
trap 'rm -rf "$cache"' EXIT
mkdir -p "$out"

ask() { printf '%s\n' "$1" | "$engine" | tail -n 1; }

for model in $("$engine" --version | jq -r 'select(.event == "version") | .models[].id'); do
  described=$(ask "{\"command\": \"describe\", \"model\": \"$model\"}")
  [[ $(jq -r '.roads' <<<"$described") == true ]] || continue
  for demo in $(jq -r '.demos[]' <<<"$described"); do
    # OpenStreetMap's servers refuse or time out now and then: try a few
    # times, waiting longer each time, and name each error.
    for attempt in 1 2 3 4 5; do
      echo "measuring $model $demo on the roads (attempt $attempt)"
      answer=$(ask "{\"command\": \"roads\", \"model\": \"$model\", \"demo\": \"$demo\", \"roads\": {\"cache_dir\": \"$cache\"}}") || true
      [[ $(jq -r '.event' <<<"$answer" 2>/dev/null) == roads ]] && break
      [[ -n $answer ]] || answer="the engine gave no answer"
      echo "$answer" >&2
      if (( attempt == 5 )); then
        exit 1
      fi
      sleep $((attempt * 60))
    done
  done
done

shopt -s nullglob
networks=("$cache"/*.json)
(( ${#networks[@]} )) || { echo "no routing demo fetched a network" >&2; exit 1; }
echo "DEMO_ROADS = {"
for network in "${networks[@]}"; do
  name=$(basename "$network")
  gzip -9 -n -c "$network" > "$out/roads-$name.gz"
  echo "    \"$name\": \"$(sha256sum "$out/roads-$name.gz" | cut -d' ' -f1)\","
done
echo "}"
