#!/usr/bin/env bash
# Build the installable Unity UPM tarball from the locked dependency closure.
set -euo pipefail
cd "$(dirname "$0")/.."
archive="${1:?usage: unity-package.sh ARCHIVE [VERSION]}"
project=examples/unity-harbour/Package/Dependencies/Harbour.Dependencies.csproj
closure="$(mktemp -d)"
trap 'rm -rf "$closure"' EXIT
dotnet restore "$project" --locked-mode
dotnet build "$project" --no-restore -c Release -o "$closure"
args=(--closure "$closure" --archive "$archive")
[[ -z "${2:-}" ]] || args+=(--version "$2")
python -m scripts.unity_package build "${args[@]}"
python -m scripts.unity_package verify "$archive" >/dev/null
