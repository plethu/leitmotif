set working-directory := '../..'
set shell := ["bash", "-euo", "pipefail", "-c"]
set default-list

# Exact optional engine tools; core-check remains engine-free.
setup:
    mise install dotnet dotnet:coverlet.console dotnet:csharpier github:godotengine/godot shellcheck

fmt:
    csharpier format examples/engine-contracts examples/unity-harbour
    dotnet restore examples/engine-contracts/Godot/Harbour.Godot.csproj --locked-mode
    dotnet build examples/engine-contracts/Godot/Harbour.Godot.csproj --no-restore
    godot-dotnet --headless --path examples/engine-contracts/Godot res://Format.tscn -- --write

fmt-check:
    csharpier check examples/engine-contracts examples/unity-harbour

# Execute the same harbour fixture through independent console and actual Godot state.
check: fmt-check
    cargo build --locked -p leitmotif --example engine-contracts
    python -m scripts.engine_check --binary "${CARGO_TARGET_DIR:-target}/debug/examples/engine-contracts"

# Optional installed Unity Editor and Linux player, separate from ordinary gates.
unity-check:
    shellcheck scripts/unity-check.sh
    csharpier check examples/unity-harbour
    bash scripts/unity-check.sh
