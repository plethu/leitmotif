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
# Package the runtime, then install it into a clean project like a game would.
version="$(python -c 'from scripts import unity_package; print(unity_package.project_version())')"
previous=0.0.1-previous
package="$output/leitmotif-harbour-$version.tgz"
older="$output/leitmotif-harbour-$previous.tgz"
bash scripts/unity-package.sh "$package"
bash scripts/unity-package.sh "$older" "$previous"
# The test driver stays game-side; the package alone supplies Harbour.dll and its closure.
dotnet restore examples/engine-contracts/tests/UnityManaged/Harbour.Acceptance.csproj --locked-mode
dotnet build examples/engine-contracts/tests/UnityManaged/Harbour.Acceptance.csproj --no-restore -c Release
mkdir -p "$project/Assets/Editor" "$project/Assets/GameOwned" "$project/Assets/Plugins" \
  "$project/Assets/StreamingAssets" "$project/Packages" "$project/ProjectSettings"
cp examples/unity-harbour/Assets/*.cs "$project/Assets/"
cp examples/unity-harbour/Assets/Editor/*.cs "$project/Assets/Editor/"
cp examples/unity-harbour/Packages/manifest.json "$project/Packages/"
cp examples/unity-harbour/ProjectSettings/ProjectVersion.txt "$project/ProjectSettings/"
cp examples/engine-contracts/tests/UnityManaged/bin/Release/netstandard2.1/Harbour.Acceptance.dll \
  "$project/Assets/Plugins/"
printf 'game-owned\n' >"$project/Assets/GameOwned/Marker.txt"
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
unity() { # phase method package-version
  timeout --signal=TERM --kill-after=10s 1200s "$UNITY_EDITOR" \
    -batchmode -nographics -quit -projectPath "$project" \
    -executeMethod "$2" -harbourBackend "$backend" -harbourPackageVersion "$3" \
    -harbourPlayer "$project/Player/Harbour" -logFile "$output/$1.log"
}
check_editor() {
  unity "$1" HarbourBuild.CheckEditor "$2"
  rg -q 'Unity harbour acceptance passed:' "$output/$1.log"
  rg -qF "Unity harbour package: com.plethu.leitmotif.harbour@$2" "$output/$1.log"
}
# A game with an older installation, then the upgrade keeps game-owned assets and meta.
python -m scripts.unity_package install "$older" "$project"
check_editor editor-previous "$previous"
marker_meta="$(<"$project/Assets/GameOwned/Marker.txt.meta")"
python -m scripts.unity_package install "$package" "$project"
check_editor editor-upgrade "$version"
[[ "$(<"$project/Assets/GameOwned/Marker.txt")" == game-owned ]]
[[ "$(<"$project/Assets/GameOwned/Marker.txt.meta")" == "$marker_meta" ]]
# A full reimport from the installed tarball, then the player build.
rm -rf "$project/Library"
check_editor editor "$version"
unity build HarbourBuild.Build "$version"
rg -q "Unity harbour build passed: $backend" "$output/build.log"
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
