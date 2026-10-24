set working-directory := '../..'
set shell := ["bash", "-euo", "pipefail", "-c"]
set default-list

# Exact optional engine tools; core-check remains engine-free.
setup:
    mise install dotnet dotnet:coverlet.console dotnet:csharpier github:godotengine/godot

fmt:
    csharpier format examples/engine-contracts
    dotnet restore examples/engine-contracts/Godot/Harbour.Godot.csproj --locked-mode
    dotnet build examples/engine-contracts/Godot/Harbour.Godot.csproj --no-restore
    godot-dotnet --headless --path examples/engine-contracts/Godot res://Format.tscn -- --write

fmt-check:
    csharpier check examples/engine-contracts

# Execute the same harbour fixture through independent console and actual Godot state.
check: fmt-check
    cargo build --locked -p leitmotif --example engine-contracts
    python -m scripts.engine_check --binary "${CARGO_TARGET_DIR:-target}/debug/examples/engine-contracts"
