#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DEFAULT_ROM="$ROOT_DIR/roms/Kato-chan & Ken-chan (Japan).pce"

usage() {
    cat <<'EOF'
Usage:
  ./run.sh [--debug] [rom_path]

Examples:
  ./run.sh
  ./run.sh "roms/Kato-chan & Ken-chan (Japan).pce"
  ./run.sh --debug roms/sample_game.pce
EOF
}

release_mode=1
if [[ "${1:-}" == "--debug" ]]; then
    release_mode=0
    shift
elif [[ "${1:-}" == "--release" ]]; then
    release_mode=1
    shift
fi

if [[ "${1:-}" == "-h" || "${1:-}" == "--help" ]]; then
    usage
    exit 0
fi

rom_path="${1:-}"
if [[ -z "$rom_path" ]]; then
    if [[ -f "$DEFAULT_ROM" ]]; then
        rom_path="$DEFAULT_ROM"
    else
        first_rom="$(find "$ROOT_DIR/roms" -maxdepth 1 -type f \( -iname '*.pce' -o -iname '*.bin' \) | sort | head -n 1 || true)"
        if [[ -z "$first_rom" ]]; then
            echo "No ROM found in $ROOT_DIR/roms" >&2
            echo "Please pass a ROM path explicitly." >&2
            exit 1
        fi
        rom_path="$first_rom"
    fi
fi

if [[ ! -f "$rom_path" && -f "$ROOT_DIR/$rom_path" ]]; then
    rom_path="$ROOT_DIR/$rom_path"
fi

if [[ ! -f "$rom_path" ]]; then
    echo "ROM not found: $rom_path" >&2
    usage
    exit 1
fi

cd "$ROOT_DIR"
echo "Launching: $rom_path"
cmd=(cargo run)
if [[ "$release_mode" -eq 1 ]]; then
    cmd+=(--release)
fi
cmd+=(--example pc_engine --features cheat-ui -- "$rom_path")
exec "${cmd[@]}"
