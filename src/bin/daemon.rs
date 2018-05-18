extern crate env_logger;
extern crate fuse;
extern crate libc;
#[macro_use]
extern crate log;
extern crate threadpool;
extern crate time;
extern crate xv6fs;

use fuse::{FileType, FileAttr, Filesystem, Request};
use fuse::{ReplyEmpty, ReplyData, ReplyEntry, ReplyAttr, ReplyDirectory,
           ReplyCreate, ReplyWrite};
use libc::{EEXIST, ENOENT, EIO, EISDIR, ENOTDIR, ENOTEMPTY};
use libc::{O_CREAT, O_EXCL};
use std::env;
use std::ffi::OsStr;
use std::mem::{size_of, transmute};
use std::str::from_utf8;
use std::sync::Mutex;
use threadpool::ThreadPool;
use time::Timespec;
use xv6fs::disk::{BSIZE, DISK, Disk};
use xv6fs::fs::{DIRSIZE, ROOTINO, Dirent, DiskInode};
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

macro_rules! convert_name {
  ($name:ident, $reply:ident) => ({
    let name = str2u8($name);
    if name.is_none() {
      $reply.error(ENOENT);
      return;
    }
    name.unwrap()
  });
}

fn u82str(s_bytes: &[u8; DIRSIZE]) -> &OsStr {
  OsStr::new(from_utf8(s_bytes).unwrap())
}

fn get_perm(inode: &DiskInode) -> u16 {
  match inode.file_type {
    fs::FileType::None => panic!("invalid file type"),
    fs::FileType::Directory => 0o755,
    fs::FileType::File => 0o644,
  }
}

fn get_kind(inode: &DiskInode) -> FileType {
  match inode.file_type {
    fs::FileType::None => panic!("invalid file type"),
    fs::FileType::Directory => FileType::Directory,
    fs::FileType::File => FileType::RegularFile,
  }
}

#[derive(Clone, Copy)]
enum FuseInode {
  Ptr(*const (Mutex<Inode>, usize)),
  Inum(usize),
}

impl FuseInode {
  fn new(x: u64) -> Self {
    if x % 2 == 1 {
      FuseInode::Inum((x as usize + 1) / 2)
    } else {
      FuseInode::Ptr(x as *const _)
    }
  }

  fn serialize(self) -> u64 {
    match self {
      FuseInode::Ptr(ptr) => ptr as u64,
      FuseInode::Inum(inum) => inum as u64 * 2 - 1,
    }
  }

  fn get(self) -> UnlockedInode {
    match self {
      FuseInode::Ptr(ptr) => {
        let inode = UnlockedInode::assemble(ptr);
        inode.clone().disassemble(); // disassemble again to retain a reference
        inode
      },
      FuseInode::Inum(inum) => ICACHE.get(inum).unwrap()
    }
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
    uid: 1000,
    gid: 1000,
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

    let name = convert_name!(name, reply);

    self.pool.execute(move || {
      let txn = LOGGING.new_txn();
      let mut pinode = ICACHE.lock(&txn, &FuseInode::new(parent).get());
      let inode = match pinode.as_directory().lookup(&txn, &name) {
        Some((inode, _)) => inode,
        None => {
          reply.error(ENOENT);
          return;
        },
      };
      let dinode = ICACHE.lock(&txn, &inode);
      let attr = create_attr(
        FuseInode::Ptr(inode.disassemble()).serialize(),
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
      // Create an outer txn for txns nested in `UnlockedInode::Drop`.
      let _txn = LOGGING.new_txn();
      for i in 0..nlookup {
        let ino = UnlockedInode::assemble(ino as *const _);

        if i == 0 {
          assert!(ino.refcnt() >= nlookup as usize);
        }
        if i == nlookup - 1 {
          info!("{} refcnt left", ino.refcnt() - 1);
        }
      }
    }
  }

  fn getattr(&mut self, _req: &Request, ino: u64, reply: ReplyAttr) {
    info!("[getattr] ino={}", ino);

    self.pool.execute(move || {
      let txn = LOGGING.new_txn();
      let dinode = ICACHE.lock(&txn, &FuseInode::new(ino).get());
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

  fn setattr(
    &mut self,
    _req: &Request,
    ino: u64,
    _mode: Option<u32>,
    _uid: Option<u32>,
    _gid: Option<u32>,
    _size: Option<u64>,
    _atime: Option<Timespec>,
    _mtime: Option<Timespec>,
    _fh: Option<u64>,
    _crtime: Option<Timespec>,
    _chgtime: Option<Timespec>,
    _bkuptime: Option<Timespec>,
    _flags: Option<u32>,
    reply: ReplyAttr,
  ) {
    info!("[setattr] ino={}", ino);

    self.pool.execute(move || {
      let txn = LOGGING.new_txn();
      let dinode = ICACHE.lock(&txn, &FuseInode::new(ino).get());
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

  fn mkdir(
    &mut self,
    _req: &Request,
    parent: u64,
    name: &OsStr,
    _mode: u32,
    reply: ReplyEntry,
  ) {
    info!("[mkdir] parent={} name={:?}", parent, name);

    let name = convert_name!(name, reply);

    self.pool.execute(move || {
      let txn = LOGGING.new_txn();
      let mut pinode = ICACHE.lock(&txn, &FuseInode::new(parent).get());

      if pinode.as_directory().lookup(&txn, &name).is_some() {
        reply.error(EEXIST);
        return;
      }

      let inode = ICACHE.alloc(&txn, fs::FileType::Directory).unwrap();
      let inodeno = inode.no();
      let mut dinode = ICACHE.lock(&txn, &inode);

      dinode.nlink = 1;
      dinode.update(&txn);

      assert!(dinode.as_directory().link(
        &txn,
        &str2u8(OsStr::new(".")).unwrap(),
        inodeno as u16,
      ));
      assert!(dinode.as_directory().link(
        &txn,
        &str2u8(OsStr::new("..")).unwrap(),
        pinode.no() as u16,
      ));

      assert!(pinode.as_directory().link(&txn, &name, inodeno as u16));

      pinode.nlink += 1; // for `..`
      pinode.update(&txn);

      let attr = create_attr(
        FuseInode::Ptr(inode.disassemble()).serialize(),
        dinode.size as u64,
        get_kind(&dinode),
        get_perm(&dinode),
        dinode.nlink as u32,
      );
      reply.entry(&TTL, &attr, 0);
    });
  }

  fn unlink(
    &mut self,
    _req: &Request,
    parent: u64,
    name: &OsStr,
    reply: ReplyEmpty,
  ) {
    info!("[unlink] parent={} name={:?}", parent, name);

    let name = convert_name!(name, reply);

    self.pool.execute(move || {
      let txn = LOGGING.new_txn();
      let mut pinode = ICACHE.lock(&txn, &FuseInode::new(parent).get());

      match pinode.as_directory().lookup(&txn, &name) {
        Some((inode, offset)) => {
          let mut dinode = ICACHE.lock(&txn, &inode);

          if dinode.file_type != fs::FileType::File {
            reply.error(EISDIR);
            return;
          }
          dinode.nlink -= 1;
          dinode.update(&txn);
          pinode.write(&txn, offset, unsafe {
            &transmute::<_, [u8; size_of::<Dirent>()]>(Dirent {
              inum: 0,
              name: [0; DIRSIZE],
            })
          });

          reply.ok();
        },
        None => {
          reply.error(ENOENT);
        },
      }
    });
  }

  fn rmdir(
    &mut self,
    _req: &Request,
    parent: u64,
    name: &OsStr,
    reply: ReplyEmpty,
  ) {
    info!("[rmdir] parent={} name={:?}", parent, name);

    if name == "." || name == ".." {
      reply.error(ENOENT);
      return;
    }
    let name = convert_name!(name, reply);

    self.pool.execute(move || {
      let txn = LOGGING.new_txn();
      let mut pinode = ICACHE.lock(&txn, &FuseInode::new(parent).get());

      match pinode.as_directory().lookup(&txn, &name) {
        Some((inode, offset)) => {
          let mut dinode = ICACHE.lock(&txn, &inode);

          if dinode.file_type != fs::FileType::Directory {
            reply.error(ENOTDIR);
            return;
          }
          if !dinode.as_directory().is_empty(&txn) {
            reply.error(ENOTEMPTY);
            return;
          }

          dinode.nlink -= 1;
          dinode.update(&txn);

          pinode.nlink -= 1;
          pinode.update(&txn); // for `..`
          pinode.write(&txn, offset, unsafe {
            &transmute::<_, [u8; size_of::<Dirent>()]>(Dirent {
              inum: 0,
              name: [0; DIRSIZE],
            })
          });

          reply.ok();
        },
        None => {
          reply.error(ENOENT);
        },
      }
    });
  }

  fn rename(
    &mut self,
    _req: &Request,
    parent: u64,
    name: &OsStr,
    newparent: u64,
    newname: &OsStr,
    reply: ReplyEmpty,
  ) {
    info!(
      "[rename] parent={} name={:?} newparent={} newname={:?}",
      parent,
      name,
      newparent,
      newname
    );

    let _name = convert_name!(name, reply);
    let _newname = convert_name!(newname, reply);

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
      let mut inode = ICACHE.lock(&txn, &FuseInode::new(ino).get());

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
      let mut inode = ICACHE.lock(&txn, &FuseInode::new(ino).get());

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
        let mut inode = ICACHE.lock(&txn, &FuseInode::new(ino).get());
        ents = inode.as_directory().enumerate(&txn);
      }

      for (inode, name) in ents {
        let dinode = ICACHE.lock(&txn, &inode);
        reply.add(
          FuseInode::Inum(inode.no()).serialize(),
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

    let name = convert_name!(name, reply);

    self.pool.execute(move || {
      let txn = LOGGING.new_txn();
      let mut pinode = ICACHE.lock(&txn, &FuseInode::new(parent).get());
      let create_flag = flags & O_CREAT as u32 != 0;
      let exist_flag = flags & (O_CREAT | O_EXCL) as u32 != 0;

      match pinode.as_directory().lookup(&txn, &name) {
        Some((inode, _)) => {
          let dinode = ICACHE.lock(&txn, &inode);

          if exist_flag || dinode.file_type != fs::FileType::File {
            reply.error(EEXIST);
            return;
          }
          let attr = create_attr(
            FuseInode::Ptr(inode.disassemble()).serialize(),
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

          dinode.nlink = 1;
          dinode.update(&txn);

          assert!(pinode.as_directory().link(&txn, &name, inode.no() as u16));

          let attr = create_attr(
            FuseInode::Ptr(inode.disassemble()).serialize(),
            dinode.size as u64,
            get_kind(&dinode),
            get_perm(&dinode),
            dinode.nlink as u32,
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
  DISK.mount(Disk::load(fsimg).unwrap());

  let mountpoint = env::args_os().nth(1).unwrap();
  let xv6fs = Xv6FS::new(10);

  match fuse::mount(xv6fs, &mountpoint, &[]) {
    Ok(_) => (),
    Err(e) => println!("{}", e),
  }
}
