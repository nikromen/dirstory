#!/bin/sh

set -e

WORKDIR=$(dirname "$(realpath "$0")")

. "$WORKDIR/config"
. "$UTILS/helpers.sh"

echo "Running tests for sh..."
run_test sh.sh

echo "Running tests for bash..."
run_test bash.sh

echo "Running tests for zsh..."
run_test zsh.sh

echo "Running tests for fish..."
run_test fish.sh
