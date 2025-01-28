#!/bin/zsh

set -e

echo "Installing dirstory for zsh..."
echo "eval \"\$(dirstory init zsh)\"" >> "$HOME/.zshrc"

source "$HOME/.zshrc"

echo "Dirstory for zsh installed successfully"

# export -f is not available in zsh, so I just copied it

safe_cd() {
    if ! cd "$1"; then
        echo "Failed to change directory to $1"
        exit 1
    fi
}

cmp="$UTILS/cmp.sh"
fst_dir=$(pwd)
source "$TEST_CASES/common.sh"
