container_name := "dirstory_test"
working_dir := "$(pwd)"
bind_path := "/app/bind"

build:
    podman build -t {{container_name}} -f test/Containerfile .

rebuild:
    podman build --no-cache -t {{container_name}} -f test/Containerfile .

shell:
    podman run --rm -it -v {{working_dir}}:{{bind_path}}:Z {{container_name}} /bin/bash

test-unit:
    podman run --rm -ti -v {{working_dir}}:{{bind_path}}:Z {{container_name}} \
        bash -c "cd {{bind_path}} && RUST_BACKTRACE=full cargo test"

test-e2e: rebuild
    podman run --rm -ti {{container_name}} bash -c "./test/e2e/runtest.sh"

rm-image:
    podman image rm {{container_name}}

test: rebuild test-unit test-e2e

clean-local:
    cargo clean
