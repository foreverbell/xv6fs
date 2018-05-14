use disk::BSIZE;
use std::mem::size_of;

#[repr(C)]
pub struct SuperBlock {
  size: u32,
  nblocks: u32,
  ninodes: u32,
  nlog: u32,
  log_start: u32,
  inode_start: u32,
  bmap_start: u32,
}

pub const NDIRECT: usize = 12;
pub const NINDIRECT: usize = BSIZE / size_of::<u32>();
pub const MAXFILE: usize = NDIRECT + NINDIRECT;

pub const ROOTINO: usize = 1;

#[repr(u16)]
pub enum FileType {
  Directory,
  File,
}

#[repr(C)]
pub struct DiskInode {
  file_type: FileType,
  unused1: u16,
  unused2: u16,
  nlink: u16,
  size: u32,
  addrs: [u32; NDIRECT + 1],
}

pub const LOGSIZE: usize = 32;

#[repr(C)]
pub struct LogBlock {
  n: u32,
  blocks: [u32; LOGSIZE],
}

pub const DIRSIZE: usize = 14;

#[repr(C)]
pub struct Dirent {
  inum: u16,
  name: [u8; DIRSIZE],
}
