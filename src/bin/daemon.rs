#[macro_use]
extern crate log;
extern crate env_logger;
extern crate fuse;
extern crate libc;
extern crate threadpool;
extern crate time;
extern crate xv6fs;

use fuse::{FileType, FileAttr, Filesystem, Request};
use fuse::{ReplyEmpty, ReplyData, ReplyEntry, ReplyAttr, ReplyDirectory,
           ReplyCreate, ReplyWrite};
use libc::{EEXIST, ENOENT, EIO};
use libc::{O_CREAT, O_EXCL};
use std::env;
use std::ffi::OsStr;
use std::str::from_utf8;
use threadpool::ThreadPool;
use time::Timespec;
use xv6fs::disk::{BSIZE, DISK, Disk};
use xv6fs::fs::{DIRSIZE, DiskInode, ROOTINO};
use xv6fs::fs;
use xv6fs::inode::{ICACHE, UnlockedInode};
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

fn u82str(s_bytes: &[u8; DIRSIZE]) -> &OsStr {
  OsStr::new(from_utf8(s_bytes).unwrap())
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

fn create_attr(
  ino: u64,
  size: u64,
  kind: FileType,
  perm: u16,
  nlink: u32,
) -> FileAttr {
  FileAttr {
    ino: ino,
    size: size,
    blocks: ((size as usize + BSIZE - 1) / BSIZE) as u64,
    atime: DEFAULT_TIME,
    mtime: DEFAULT_TIME,
    ctime: DEFAULT_TIME,
    crtime: DEFAULT_TIME,
    kind: kind,
    perm: perm,
    nlink: nlink,
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
    _req: &Request,
    parent: u64,
    name: &OsStr,
    reply: ReplyEntry,
  ) {
    info!("[lookup] parent={} name={:?}", parent, name);

    let name = str2u8(name);
    if name.is_none() {
      reply.error(ENOENT);
      return;
    }
    self.pool.execute(move || {
      let txn = LOGGING.new_txn();
      let mut pinode = ICACHE.lock(&txn, &get_inode(parent));
      let inode = match pinode.as_directory().lookup(&txn, &name.unwrap()) {
        Some(inode) => inode,
        None => {
          reply.error(ENOENT);
          return;
        },
      };
      let dinode = ICACHE.lock(&txn, &inode).inode.unwrap();
      let attr = create_attr(
        inode.disassemble() as u64,
        dinode.size as u64,
        get_kind(&dinode),
        get_perm(&dinode),
        dinode.nlink as u32,
      );

      reply.entry(&TTL, &attr, 0);
    });
  }

  fn forget(&mut self, _req: &Request, ino: u64, nlookup: u64) {
    info!("[forget] ino={} nlookup={}", ino, nlookup);

    if ino != ROOTINO as u64 {
      for i in 0..nlookup {
        let ino = UnlockedInode::assemble(ino as *const _);

        if i == 0 {
          assert!(ino.refcnt() >= nlookup as usize);
        }
      }
    }
  }

  fn getattr(&mut self, _req: &Request, ino: u64, reply: ReplyAttr) {
    info!("[getattr] ino={}", ino);

    self.pool.execute(move || {
      let txn = LOGGING.new_txn();
      let dinode = ICACHE.lock(&txn, &get_inode(ino)).inode.unwrap();
      let attr = create_attr(
        ino,
        dinode.size as u64,
        get_kind(&dinode),
        get_perm(&dinode),
        dinode.nlink as u32,
      );

      reply.attr(&TTL, &attr);
    });
  }

  // postpone
  fn mkdir(
    &mut self,
    _req: &Request,
    _parent: u64,
    _name: &OsStr,
    _mode: u32,
    _reply: ReplyEntry,
  ) {
    unimplemented!();
  }

  // postpone
  fn unlink(
    &mut self,
    _req: &Request,
    _parent: u64,
    _name: &OsStr,
    _reply: ReplyEmpty,
  ) {
    unimplemented!();
  }

  // postpone
  fn rmdir(
    &mut self,
    _req: &Request,
    _parent: u64,
    _name: &OsStr,
    _reply: ReplyEmpty,
  ) {
    unimplemented!();
  }

  // postpone
  fn rename(
    &mut self,
    _req: &Request,
    _parent: u64,
    _name: &OsStr,
    _newparent: u64,
    _newname: &OsStr,
    _reply: ReplyEmpty,
  ) {
    unimplemented!();
  }

  fn read(
    &mut self,
    _req: &Request,
    ino: u64,
    _fh: u64,
    offset: i64,
    size: u32,
    reply: ReplyData,
  ) {
    info!("[read] ino={} offset={} size={}", ino, offset, size);
    assert!(offset >= 0);

    self.pool.execute(move || {
      let txn = LOGGING.new_txn();
      let mut inode = ICACHE.lock(&txn, &get_inode(ino));

      match inode.read(&txn, offset as usize, size as usize) {
        None => {
          reply.error(EIO);
        },
        Some(data) => {
          reply.data(data.as_slice());
        },
      }
    });
  }

  fn write(
    &mut self,
    _req: &Request,
    ino: u64,
    _fh: u64,
    offset: i64,
    data: &[u8],
    _flags: u32,
    reply: ReplyWrite,
  ) {
    info!("[write] ino={} offset={} size={}", ino, offset, data.len());
    assert!(offset >= 0);

    let data = Vec::from(data);

    self.pool.execute(move || {
      let txn = LOGGING.new_txn();
      let mut inode = ICACHE.lock(&txn, &get_inode(ino));

      match inode.write(&txn, offset as usize, &data) {
        None => reply.error(EIO),
        Some(written) => reply.written(written as u32),
      }
    });
  }

  fn readdir(
    &mut self,
    _req: &Request,
    ino: u64,
    _fh: u64,
    offset: i64,
    mut reply: ReplyDirectory,
  ) {
    info!("[readdir] ino={} offset={}", ino, offset);

    if offset != 0 {
      reply.ok();
      return;
    }
    self.pool.execute(move || {
      let txn = LOGGING.new_txn();
      let ents: Vec<(UnlockedInode, [u8; DIRSIZE])>;
      let mut offset = 0;
      {
        let mut inode = ICACHE.lock(&txn, &get_inode(ino));
        ents = inode.as_directory().enumerate(&txn);
      }

      for (inode, name) in ents {
        let dinode = ICACHE.lock(&txn, &inode).inode.unwrap();
        reply.add(
          inode.disassemble() as u64,
          offset,
          get_kind(&dinode),
          u82str(&name),
        );
        offset += 1;
      }
      reply.ok();
    });
  }

  fn create(
    &mut self,
    _req: &Request,
    parent: u64,
    name: &OsStr,
    _mode: u32,
    flags: u32,
    reply: ReplyCreate,
  ) {
    info!("[create] parent={} name={:?} flags={}", parent, name, flags);

    let name = str2u8(name);
    if name.is_none() {
      reply.error(ENOENT);
      return;
    }
    self.pool.execute(move || {
      let txn = LOGGING.new_txn();
      let mut pinode = ICACHE.lock(&txn, &get_inode(parent));
      let create_flag = flags & O_CREAT as u32 != 0;
      let exist_flag = flags & (O_CREAT | O_EXCL) as u32 != 0;

      match pinode.as_directory().lookup(&txn, &name.unwrap()) {
        Some(inode) => {
          let dinode = ICACHE.lock(&txn, &inode).inode.unwrap();

          if exist_flag || dinode.file_type != fs::FileType::File {
            reply.error(EEXIST);
            return;
          }
          let attr = create_attr(
            inode.disassemble() as u64,
            dinode.size as u64,
            get_kind(&dinode),
            get_perm(&dinode),
            dinode.nlink as u32,
          );
          reply.created(&TTL, &attr, 0, 0, 0);
        },
        None => {
          if !create_flag {
            reply.error(ENOENT);
            return;
          }
          let inode = ICACHE.alloc(&txn, fs::FileType::File).unwrap();
          let mut dinode = ICACHE.lock(&txn, &inode);

          dinode.inode.unwrap().nlink = 1;
          dinode.update(&txn);

          if !pinode.as_directory().link(
            &txn,
            &name.unwrap(),
            inode.no() as u16,
          )
          {
            error!("unable to link inode {} in parent {}", inode.no(), parent);
          }

          let attr = create_attr(
            inode.disassemble() as u64,
            dinode.inode.unwrap().size as u64,
            get_kind(dinode.inode.as_ref().unwrap()),
            get_perm(dinode.inode.as_ref().unwrap()),
            dinode.inode.unwrap().nlink as u32,
          );
          reply.created(&TTL, &attr, 0, 0, 0);
        },
      };
    });
  }
}

fn main() {
  env_logger::init();

  let fsimg = env::args_os().nth(2).unwrap();
  DISK.lock().unwrap().mount(Disk::load(fsimg).unwrap());

  let mountpoint = env::args_os().nth(1).unwrap();
  let xv6fs = Xv6FS::new(10);

  match fuse::mount(xv6fs, &mountpoint, &[]) {
    Ok(_) => (),
    Err(e) => println!("{}", e),
  }
}
