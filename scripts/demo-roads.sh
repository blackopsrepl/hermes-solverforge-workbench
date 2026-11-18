#!/usr/bin/env bash
# Fetch the road network each stock routing demo runs on, with the engine
# itself, and write it to <out>/roads-<cache file>.gz: the release assets the
# plugin downloads into its road cache the first time that demo is opened
# (engine_bin.DEMO_ROADS pins each by "<model>/<demo>" with its SHA-256).
# CI runs exactly this script; run it locally to see what a release would ship.
#
#   scripts/demo-roads.sh engine/target/release/solverforge-workbench dist
set -euo pipefail

engine=${1:?usage: demo-roads.sh <engine binary> <out dir>}
out=${2:?usage: demo-roads.sh <engine binary> <out dir>}
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
mkdir -p "$out"

ask() { printf '%s\n' "$1" | "$engine" | tail -n 1; }

pins=()
for model in $("$engine" --version | jq -r 'select(.event == "version") | .models[].id'); do
  described=$(ask "{\"command\": \"describe\", \"model\": \"$model\"}")
  [[ $(jq -r '.roads' <<<"$described") == true ]] || continue
  for demo in $(jq -r '.demos[]' <<<"$described"); do
    # Each demo measures into a cache of its own, so the one file it leaves
    # is that demo's network.
    cache="$work/$model-$demo"
    mkdir -p "$cache"
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
    shopt -s nullglob
    networks=("$cache"/*.json)
    shopt -u nullglob
    (( ${#networks[@]} == 1 )) || { echo "$model $demo left ${#networks[@]} networks, not one" >&2; exit 1; }
    name=$(basename "${networks[0]}")
    gzip -9 -n -c "${networks[0]}" > "$out/roads-$name.gz"
    pins+=("    \"$model/$demo\": (\"$name\", \"$(sha256sum "$out/roads-$name.gz" | cut -d' ' -f1)\"),")
  done
done

(( ${#pins[@]} )) || { echo "no routing demo fetched a network" >&2; exit 1; }
echo "DEMO_ROADS = {"
printf '%s\n' "${pins[@]}"
echo "}"
