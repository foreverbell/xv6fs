use disk::BSIZE;
use std::mem::size_of;

#[repr(C)]
pub struct SuperBlock {
  pub size: u32,
  pub nblocks: u32,
  pub ninodes: u32,
  pub nlog: u32,
  pub log_start: u32,
  pub inode_start: u32,
  pub bmap_start: u32,
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
  pub file_type: FileType,
  pub unused1: u16,
  pub unused2: u16,
  pub nlink: u16,
  pub size: u32,
  pub addrs: [u32; NDIRECT + 1],
}

pub const LOGSIZE: usize = 32;

#[repr(C)]
pub struct LogBlock {
  pub n: u32,
  pub blocks: [u32; LOGSIZE],
}

pub const DIRSIZE: usize = 14;

#[repr(C)]
pub struct Dirent {
  pub inum: u16,
  pub name: [u8; DIRSIZE],
}
