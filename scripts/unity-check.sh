#!/usr/bin/env bash
# Optional clean-consumer Unity lane; never runs in core-check or hosted CI.
set -euo pipefail
cd "$(dirname "$0")/.."
: "${UNITY_EDITOR:?Set UNITY_EDITOR to an installed Unity executable}"
backend="${HARBOUR_UNITY_BACKEND:-IL2CPP}"
case "$backend" in
  IL2CPP | Mono2x | CoreCLR) ;;
  *) echo 'HARBOUR_UNITY_BACKEND must be IL2CPP, Mono2x, or CoreCLR' >&2; exit 2 ;;
esac
output="${HARBOUR_UNITY_OUTPUT:-$PWD/target/unity-$backend}"
mkdir -p "$output"
output="$(realpath "$output")"
project="$(mktemp -d "$output/consumer.XXXXXX")"
trap 'rm -rf "$project"' EXIT
# Copy authored fixture inputs only, even if the source was opened in Unity before.
mkdir -p "$project/Assets/Editor" "$project/Assets/Plugins" "$project/Assets/StreamingAssets" \
  "$project/Packages" "$project/ProjectSettings"
cp examples/unity-harbour/Assets/*.cs "$project/Assets/"
cp examples/unity-harbour/Assets/Editor/*.cs "$project/Assets/Editor/"
cp examples/unity-harbour/Packages/manifest.json "$project/Packages/"
cp examples/unity-harbour/ProjectSettings/ProjectVersion.txt "$project/ProjectSettings/"
cargo build --locked -p leitmotif --example engine-contracts
binary="${CARGO_TARGET_DIR:-target}/debug/examples/engine-contracts"
revision="$(git rev-parse HEAD)"
for name in sheltered departed unreachable inconclusive; do
  query="$name"; depth=3; status=0
  case "$name" in
    unreachable) query=storm-passed; status=1 ;;
    inconclusive) query=sheltered; depth=0; status=2 ;;
  esac
  request="$project/Assets/StreamingAssets/$name.request.json"
  report="$project/Assets/StreamingAssets/$name.check.json"
  "$binary" --source-revision "$revision" request --query "$query" --max-depth "$depth" >"$request"
  actual=0
  "$binary" --source-revision "$revision" check "$request" >"$report" || actual=$?
  [[ "$actual" == "$status" ]] || { echo "Unexpected Rust outcome: $name ($actual)" >&2; exit 1; }
done
dotnet restore examples/engine-contracts/tests/UnityManaged/Harbour.Acceptance.csproj --locked-mode
dotnet build examples/engine-contracts/tests/UnityManaged/Harbour.Acceptance.csproj --no-restore -c Release
cp examples/engine-contracts/tests/UnityManaged/bin/Release/netstandard2.1/*.dll "$project/Assets/Plugins/"
# Unity supplies netstandard's platform assemblies, including System.Memory and Tasks.Extensions.
# Only the dependency assets selected by the netstandard project above are distributed.
for phase in editor build; do
  method=HarbourBuild.CheckEditor
  [[ "$phase" != build ]] || method=HarbourBuild.Build
  timeout --signal=TERM --kill-after=10s 1200s "$UNITY_EDITOR" \
    -batchmode -nographics -quit -projectPath "$project" \
    -executeMethod "$method" -harbourBackend "$backend" \
    -harbourPlayer "$project/Player/Harbour" -logFile "$output/$phase.log"
  if [[ "$phase" == editor ]]; then
    rg -q 'Unity harbour acceptance passed:' "$output/editor.log"
  else
    rg -q "Unity harbour build passed: $backend" "$output/build.log"
  fi
done
case "$backend" in
  IL2CPP) runtime="$project/Player/GameAssembly.so" ;;
  Mono2x) runtime="$project/Player/Harbour_Data/MonoBleedingEdge/x86_64/libmonobdwgc-2.0.so" ;;
  CoreCLR) runtime="$project/Player/CoreCLR/native/libcoreclr.so" ;;
esac
[[ -f "$runtime" ]] || { echo "Missing $backend runtime: $runtime" >&2; exit 1; }
timeout --signal=TERM --kill-after=5s 120s "$project/Player/Harbour" \
  -batchmode -nographics -logFile "$output/player.log"
for log in "$output/editor.log" "$output/player.log"; do
  rg -q 'Unity harbour acceptance passed:' "$log"
  rg -q 'Harbour conformance: 30 cases;' "$log"
  rg -q 'Rejected harbour evidence:' "$log"
  rg -q 'host action diverged at crates/leitmotif/examples/support/composed_town/' "$log"
done
printf 'Unity Editor and %s Linux player passed. Logs: %s\n' "$backend" "$output"
