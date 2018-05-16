use disk::BSIZE;
use std::mem::size_of;

#[repr(C)]
pub struct SuperBlock {
  pub nblocks: u32, // Number of blocks (size of file system image)
  pub unused: u32,
  pub ninodes: u32, // Number of inodes
  pub nlogs: u32, // Number of log blocks
  pub log_start: u32, // Block number of first log block
  pub inode_start: u32, // Block number of first inode block
  pub bmap_start: u32, // Block number of first free map block
}

// Number of bitmap bits per block.
pub const BPB: usize = BSIZE * 8;

// Number of inodes per block.
pub const IPB: usize = BSIZE / size_of::<DiskInode>();

impl SuperBlock {
  // Block of free map containing bit for block `blockno`.
  pub fn bblock(&self, blockno: usize) -> usize {
    self.bmap_start as usize + blockno / BPB
  }

  // Block containing inode `inodeno`.
  pub fn iblock(&self, inodeno: usize) -> usize {
    self.inode_start as usize + inodeno / IPB
  }
}

// Number of direct blocks of an inode.
pub const NDIRECT: usize = 12;

// Number of indirect blocks of an inode.
pub const NINDIRECT: usize = BSIZE / size_of::<u32>();

// Number of blocks of an inode.
pub const NIBLOCKS: usize = NDIRECT + NINDIRECT;

// Inode index of root folder.
pub const ROOTINO: usize = 1;

#[repr(u16)]
#[derive(PartialEq, Eq, Clone, Copy)]
pub enum FileType {
  None,
  Directory,
  File,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct DiskInode {
  pub file_type: FileType,
  pub unused1: u16,
  pub unused2: u16,
  pub nlink: u16,
  pub size: u32,
  pub addrs: [u32; NDIRECT + 1],
}

impl DiskInode {
  pub fn init(&mut self, file_type: FileType) {
    self.file_type = file_type;
    self.unused1 = 0;
    self.unused2 = 0;
    self.nlink = 0;
    self.size = 0;
    for i in 0..(NDIRECT + 1) {
      self.addrs[i] = 0;
    }
  }
}

// Maximum number of log entries.
pub const LOGSIZE: usize = 64;

#[repr(C)]
pub struct LogHeader {
  pub n: u32,
  pub blocks: [u32; LOGSIZE], // blocks[i] <-> sb.log_start + i + 1
}

// Maximum length of directory name.
pub const DIRSIZE: usize = 14;

#[repr(C)]
pub struct Dirent {
  pub inum: u16,
  pub name: [u8; DIRSIZE],
}
