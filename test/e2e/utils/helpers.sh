safe_cd() {
    if ! cd "$1"; then
        echo "Failed to change directory to $1"
        exit 1
    fi
}

run_test() {
    echo "Running tests for $1..."
    if ! "$TEST_CASES/$1"; then
        echo "Tests for $1 failed"
        exit 1
    else
        echo "Tests for $1 passed"
    fi
}

export -f safe_cd run_test
