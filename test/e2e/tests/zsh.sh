#!/bin/zsh

set -e

echo "Installing dirstory for zsh..."
echo "eval \"\$(dirstory init zsh)\"" >> "$HOME/.zshrc"

source "$HOME/.zshrc"

echo "Dirstory for zsh installed successfully"

source "$TEST_CASES/common.sh"
