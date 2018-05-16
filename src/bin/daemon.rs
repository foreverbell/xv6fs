extern crate fuse;
extern crate libc;
extern crate time;
extern crate xv6fs;
extern crate threadpool;

use fuse::{FileType, FileAttr, Filesystem, Request, ReplyData, ReplyEntry,
           ReplyAttr, ReplyDirectory};
use std::ffi::OsStr;
use threadpool::ThreadPool;
use time::Timespec;
use xv6fs::fs::{DIRSIZE, DiskInode};
use xv6fs::fs;
use xv6fs::inode::{ICACHE, Inode};
use xv6fs::logging::LOGGING;

const TTL: Timespec = Timespec { sec: 1, nsec: 0 }; // 1 second

// xv6fs does not support file time stamp, use a dummy one.
const DEFAULT_TIME: Timespec = Timespec { sec: 42, nsec: 42 };

fn str2u8(s: &OsStr) -> Option<[u8; DIRSIZE]> {
  let s_bytes = s.to_str()?.as_bytes();
  if s_bytes.len() > DIRSIZE {
    return None;
  }

  let mut result: [u8; DIRSIZE] = [0; DIRSIZE];
  for i in 0..s_bytes.len() {
    result[i] = s_bytes[i];
  }
  Some(result)
}

fn get_perm(inode: &DiskInode) -> u16 {
  match inode.file_type {
    fs::FileType::None => {
      panic!("invalid file type");
    },
    fs::FileType::Directory => 0o644,
    fs::FileType::File => 0o755,
  }
}

fn get_kind(inode: &DiskInode) -> FileType {
  match inode.file_type {
    fs::FileType::None => {
      panic!("invalid file type");
    },
    fs::FileType::Directory => FileType::Directory,
    fs::FileType::File => FileType::RegularFile,
  }
}

struct Xv6FS {
  pool: ThreadPool,
}

impl Xv6FS {
  fn new(nworkers: usize) -> Self {
    Xv6FS { pool: ThreadPool::new(nworkers) }
  }
}

impl Filesystem for Xv6FS {
  fn lookup(
    &mut self,
    req: &Request,
    parent: u64,
    name: &OsStr,
    reply: ReplyEntry,
  ) {
    let name = str2u8(name).unwrap();
    self.pool.execute(move || {
      let txn = LOGGING.new_txn();
      let mut pinode = ICACHE.get(parent as usize).unwrap().acquire();
      let inode = pinode.dlookup(&txn, &name).unwrap().acquire();
      let inodeno = inode.no;
      let inode = inode.inode.unwrap();

      let attr = FileAttr {
        ino: inodeno as u64,
        size: inode.size as u64,
        blocks: 1,
        atime: DEFAULT_TIME,
        mtime: DEFAULT_TIME,
        ctime: DEFAULT_TIME,
        crtime: DEFAULT_TIME,
        kind: get_kind(&inode),
        perm: get_perm(&inode),
        nlink: inode.nlink as u32,
        uid: 0,
        gid: 0,
        rdev: 0,
        flags: 0,
      };
    });
  }
}

fn main() {
  println!("Hello World!");
}
