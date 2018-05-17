extern crate env_logger;
extern crate fuse;
extern crate libc;
extern crate threadpool;
extern crate time;
extern crate xv6fs;

use fuse::{FileType, FileAttr, Filesystem, Request};
use fuse::{ReplyEmpty, ReplyData, ReplyEntry, ReplyAttr, ReplyDirectory,
           ReplyCreate, ReplyWrite};
use libc::ENOENT;
use std::ffi::OsStr;
use threadpool::ThreadPool;
use time::Timespec;
use xv6fs::disk::BSIZE;
use xv6fs::fs::{DIRSIZE, DiskInode, ROOTINO};
use xv6fs::fs;
use xv6fs::inode::{ICACHE, Inode, UnlockedInode};
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
    fs::FileType::None => panic!("invalid file type"),
    fs::FileType::Directory => 0o644,
    fs::FileType::File => 0o755,
  }
}

fn get_kind(inode: &DiskInode) -> FileType {
  match inode.file_type {
    fs::FileType::None => panic!("invalid file type"),
    fs::FileType::Directory => FileType::Directory,
    fs::FileType::File => FileType::RegularFile,
  }
}

fn get_inode(inode_ptr: u64) -> UnlockedInode {
  if inode_ptr == ROOTINO as u64 {
    return ICACHE.get(ROOTINO).unwrap();
  } else {
    let inode = UnlockedInode::assemble(inode_ptr as *const _);
    inode.clone().disassemble();
    inode
  }
}

fn create_attr(ino: u64, inode: &DiskInode) -> FileAttr {
  FileAttr {
    ino: ino,
    size: inode.size as u64,
    blocks: ((inode.size as usize + BSIZE - 1) / BSIZE) as u64,
    atime: DEFAULT_TIME,
    mtime: DEFAULT_TIME,
    ctime: DEFAULT_TIME,
    crtime: DEFAULT_TIME,
    kind: get_kind(&inode),
    perm: get_perm(&inode),
    nlink: inode.nlink as u32,
    uid: 501,
    gid: 20,
    rdev: 0,
    flags: 0,
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
    match str2u8(name) {
      None => {
        reply.error(ENOENT);
      },
      Some(name) => {
        self.pool.execute(move || {
          let txn = LOGGING.new_txn();
          let mut pinode = get_inode(parent).acquire();
          let inode = pinode.dlookup(&txn, &name).unwrap();
          let disk_inode = inode.acquire().inode.unwrap();
          let attr = create_attr(inode.disassemble() as u64, &disk_inode);

          reply.entry(&TTL, &attr, 0);
        });
      },
    }
  }

  fn forget(&mut self, _req: &Request, ino: u64, nlookup: u64) {
    if ino != ROOTINO as u64 {
      for _ in 0..nlookup {
        UnlockedInode::assemble(ino as *const _);
      }
    }
  }

  fn getattr(&mut self, _req: &Request, ino: u64, reply: ReplyAttr) {
    self.pool.execute(move || {
      let inode = get_inode(ino).acquire();
      let attr = create_attr(ino, inode.inode.as_ref().unwrap());

      reply.attr(&TTL, &attr);
    });
  }

  fn mkdir(
    &mut self,
    _req: &Request,
    _parent: u64,
    _name: &OsStr,
    _mode: u32,
    reply: ReplyEntry,
  ) {
    unimplemented!();
  }

  fn unlink(
    &mut self,
    _req: &Request,
    _parent: u64,
    _name: &OsStr,
    reply: ReplyEmpty,
  ) {
    unimplemented!();
  }

  fn rmdir(
    &mut self,
    _req: &Request,
    _parent: u64,
    _name: &OsStr,
    reply: ReplyEmpty,
  ) {
    unimplemented!();
  }

  fn rename(
    &mut self,
    _req: &Request,
    _parent: u64,
    _name: &OsStr,
    _newparent: u64,
    _newname: &OsStr,
    reply: ReplyEmpty,
  ) {
    unimplemented!();
  }

  fn read(
    &mut self,
    _req: &Request,
    _ino: u64,
    _fh: u64,
    _offset: i64,
    _size: u32,
    reply: ReplyData,
  ) {
    unimplemented!();
  }

  fn write(
    &mut self,
    _req: &Request,
    _ino: u64,
    _fh: u64,
    _offset: i64,
    _data: &[u8],
    _flags: u32,
    reply: ReplyWrite,
  ) {
    unimplemented!();
  }

  fn readdir(
    &mut self,
    _req: &Request,
    _ino: u64,
    _fh: u64,
    _offset: i64,
    reply: ReplyDirectory,
  ) {
    unimplemented!();
  }

  fn create(
    &mut self,
    _req: &Request,
    _parent: u64,
    _name: &OsStr,
    _mode: u32,
    _flags: u32,
    reply: ReplyCreate,
  ) {
    unimplemented!();
  }
}

fn main() {
  println!("Hello World!");

  env_logger::init();
}
