#!/bin/bash

set -e

echo "Installing dirstory for bash..."
echo "eval \"\$(dirstory init bash)\"" >> "$HOME/.bashrc"

source "$HOME/.bashrc"

echo "Dirstory for bash installed successfully"

cmp="$UTILS/cmp.sh"
fst_dir=$(pwd)
source "$TEST_CASES/common.sh"
