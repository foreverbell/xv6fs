all: build

build:
	cargo +nightly build

test:
	RUST_TEST_THREADS=1 cargo +nightly test

.PHONY: build test
