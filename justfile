# Local commands use host tools; container commands use the same recipes.
set shell := ["bash", "-eu", "-o", "pipefail", "-c"]

image := env("CONTAINER_IMAGE", "localhost/dirstory-test:latest")
target_volume := env("CONTAINER_TARGET_VOLUME", "dirstory-target")
cargo_volume := env("CONTAINER_CARGO_VOLUME", "dirstory-cargo-cache")
pre_commit_volume := env("CONTAINER_PRE_COMMIT_VOLUME", "dirstory-precommit-cache")
workspace := justfile_directory()
container_args := '--rm --pull=never' \
    + ' -v ' + quote(workspace + ':/workspace:z') \
    + ' -v ' + quote(target_volume + ':/workspace/target:z') \
    + ' -v ' + quote(cargo_volume + ':/root/.cargo:z') \
    + ' -v ' + quote(pre_commit_volume + ':/root/.cache/pre-commit:z') \
    + ' -w /workspace' \
    + ' -e CARGO_TARGET_DIR=/workspace/target' \
    + ' -e PRE_COMMIT_HOME=/root/.cache/pre-commit'

export PRE_COMMIT_HOME := env("PRE_COMMIT_HOME", workspace + "/.cache/pre-commit")
export TEST_FILTER := env("TEST_FILTER", "")
export TEST_JOBS := env("TEST_JOBS", "1")
export RUST_BACKTRACE := env("RUST_BACKTRACE", "1")

default:
    @just --list

build:
    cargo build --locked

build-release:
    cargo build --locked --release

fmt:
    cargo fmt --all

fmt-check:
    cargo fmt --all --check

clippy:
    cargo clippy --locked --all-targets --all-features -- -D warnings

# Includes Rust unit tests and CLI integration tests.
test-unit:
    cargo test --locked "$TEST_FILTER" -- --test-threads="$TEST_JOBS" --nocapture

test-e2e: build
    DIRSTORY_BIN="${CARGO_TARGET_DIR:-target}/debug/dirstory" python3 tests/e2e/runtest.py

test:
    just test-unit
    just test-e2e

test-full-backtrace:
    RUST_BACKTRACE=full just test

pre-commit:
    pre-commit run --all-files

# Sequential even if this recipe is invoked with other commands.
test-all:
    just pre-commit
    TEST_FILTER= just test

clean:
    cargo clean

container-build-image:
    podman build -t {{quote(image)}} -f tests/Containerfile .

# Build the image explicitly once; later runs reuse it and the named caches.
container-build:
    podman run {{container_args}} {{quote(image)}} just build

container-test-unit:
    podman run {{container_args}} -e TEST_FILTER -e TEST_JOBS -e RUST_BACKTRACE {{quote(image)}} just test-unit

# Missing shells fail inside the container, rather than silently reducing coverage.
container-test-e2e:
    podman run {{container_args}} -e DIRSTORY_REQUIRE_SHELLS=1 {{quote(image)}} just test-e2e

container-test:
    podman run {{container_args}} -e TEST_FILTER -e TEST_JOBS -e RUST_BACKTRACE -e DIRSTORY_REQUIRE_SHELLS=1 {{quote(image)}} just test

container-test-full-backtrace:
    RUST_BACKTRACE=full just container-test

container-pre-commit:
    podman run {{container_args}} {{quote(image)}} pre-commit run --all-files

container-test-all:
    just container-build-image
    podman run {{container_args}} -e TEST_JOBS -e RUST_BACKTRACE -e DIRSTORY_REQUIRE_SHELLS=1 {{quote(image)}} just test-all

container-shell:
    podman run -it {{container_args}} {{quote(image)}} /bin/bash

# Cleans build output, retaining downloaded dependencies and hook environments.
container-clean:
    podman run {{container_args}} {{quote(image)}} just clean

container-remove-image:
    podman image rm {{quote(image)}}
