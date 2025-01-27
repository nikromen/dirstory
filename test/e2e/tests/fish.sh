#!/bin/fish

set -e

echo "Installing dirstory for fish..."
mkdir -p "$HOME/.config/fish"
echo "dirstory init fish | source" >> "$HOME/.config/fish/config.fish"

source "$HOME/.config/fish/config.fish"

echo "Dirstory for fish installed successfully"

source "$TEST_CASES/common.sh"
