#!/bin/sh

set -e

WORKDIR=$(dirname "$(realpath "$0")")

. "$WORKDIR/config"
. "$UTILS/helpers.sh"

echo "Running tests for sh..."
"$TEST_CASES/sh.sh"

echo "Running tests for bash..."
"$TEST_CASES/bash.sh"

echo "Running tests for zsh..."
"$TEST_CASES/zsh.sh"

echo "Running tests for fish..."
"$TEST_CASES/fish.sh"
