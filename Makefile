all: build

build:
	cargo +nightly build

target/debug/daemon:
	cargo +nightly build

target/debug/mkfs:
	cargo +nightly build

fs.img: target/debug/mkfs
	target/debug/mkfs fs.img

run: fs.img target/debug/daemon
	mkdir mnt
	RUST_LOG=info target/debug/daemon mnt fs.img

stop:
	(fusermount -u mnt) &
	(rm -rf mnt) &

test:
	RUST_TEST_THREADS=1 cargo +nightly test

clean: stop
	rm -r target fs.img

.PHONY: build run stop test clean
