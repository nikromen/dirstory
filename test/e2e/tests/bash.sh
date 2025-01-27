#!/bin/bash

set -e

echo "Installing dirstory for bash..."
echo "eval \"\$(dirstory init bash)\"" >> "$HOME/.bashrc"

source "$HOME/.bashrc"

echo "Dirstory for bash installed successfully"

source "$TEST_CASES/common.sh"
