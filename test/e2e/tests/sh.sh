#!/bin/sh

set -e

echo "Installing dirstory for sh..."
echo "eval \"\$(dirstory init sh)\"" >> "$HOME/.shrc"

. "$HOME/.shrc"

echo "Dirstory for sh installed successfully"

. "$TEST_CASES/common.sh"
