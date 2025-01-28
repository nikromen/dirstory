#!/bin/fish

echo "Installing dirstory for fish..."
mkdir -p "$HOME/.config/fish"
echo "dirstory init fish | source" >> "$HOME/.config/fish/config.fish"

source "$HOME/.config/fish/config.fish"

echo "Dirstory for fish installed successfully"

set cmp "$UTILS/cmp.sh"
set fst_dir (pwd)

rm -f /tmp/dirstory_test_fish.log
source "$TEST_CASES/common.sh" | tee /tmp/dirstory_test_fish.log

if grep -q "FAIL" /tmp/dirstory_test_fish.log;
    grep "FAIL" /tmp/dirstory_test_fish.log
    exit 1
end
