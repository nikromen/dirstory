CONTAINER_ENGINE ?= $(shell command -v podman 2> /dev/null || echo docker)
CONTAINER_TEST_NAME ?= dirstory_test


build-image:
	$(CONTAINER_ENGINE) build -f test/Containerfile . -t $(CONTAINER_TEST_NAME)


test-in-container: build-image
	$(CONTAINER_ENGINE) run --rm -it $(CONTAINER_TEST_NAME) /bin/sh -c "poetry run pytest -vvv test/"