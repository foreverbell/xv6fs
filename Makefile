all:
	cargo build

test:
	RUST_TEST_THREADS=1 cargo test
