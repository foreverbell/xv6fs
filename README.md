# xv6fs

Reimplementation of the file system used by [xv6](https://pdos.csail.mit.edu/6.828/2017/xv6.html)
in Rust and FUSE.

xv6's file system follows the early design of Linux's journaling file system
(namely, extfs2), which supports recovery in presence of crash. For details,
please refer to the [xv6 book](https://pdos.csail.mit.edu/6.828/2017/xv6/book-rev10.pdf).

Normally, a file system should be abstracted into these layers.

* disk
* block cache
* logging (journaling, transaction)
* inode
* directory
* path resolution
* file object / descriptor

We use FUSE's low level API, which manages this last two layers conforming with
Unix. The remaining work is to implement the top five layers and cooperate
them with FUSE interface.

Notice: for convenience, we mock the disk with an array of contiguous 512 bytes
in memory, with an interface providing atomic block read / write. This disk acts
as a service running in a separated thread, and communicate with the file system
in a go routine fashion (which should be similar to IDE interruption).

## Build

A nightly Rust compiler is required (known to compile with `rustc 1.27.0-nightly`).

```bash
$ make run
$ cd mnt
$ touch foobar
```

## License

Conforming with xv6 (see `LICENSE`).
