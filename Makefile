all: build

build:
	cargo +nightly build

test:
	RUST_TEST_THREADS=1 cargo +nightly test

clean:
	rm -r target

.PHONY: build test clean
