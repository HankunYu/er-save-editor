#!/usr/bin/env bash
# Install er + er-qs to ~/.local/bin (uses bundled prebuilt binaries if present,
# otherwise builds from source).
set -euo pipefail

DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PREFIX="${PREFIX:-$HOME/.local/bin}"

if [ -x "$DIR/prebuilt/er" ] && [ -x "$DIR/prebuilt/er-qs" ]; then
    echo "Installing prebuilt binaries to $PREFIX ..."
    install -Dm755 "$DIR/prebuilt/er"    "$PREFIX/er"
    install -Dm755 "$DIR/prebuilt/er-qs" "$PREFIX/er-qs"
else
    echo "No prebuilt binaries; building from source (needs cargo) ..."
    cargo build --release --features tui --manifest-path "$DIR/Cargo.toml"
    install -Dm755 "$DIR/target/release/er"    "$PREFIX/er"
    install -Dm755 "$DIR/target/release/er-qs" "$PREFIX/er-qs"
fi

echo "✓ Installed: er, er-qs -> $PREFIX"
if ! command -v er >/dev/null 2>&1; then
    echo "⚠ $PREFIX is not on PATH — add it, e.g.: export PATH=\"$PREFIX:\$PATH\""
fi
echo "Run 'er' for the TUI, or 'er-qs help' for the CLI."
