#[macro_use]
extern crate xv6fs;

use std::env;
use std::fs::File;
use std::io::{Write, Seek, SeekFrom};
use std::mem::{size_of, transmute};
use xv6fs::disk::BSIZE;
use xv6fs::fs::{SuperBlock, DiskInode, FileType, Dirent, IPB, BPB, LOGSIZE,
                NDIRECT, DIRSIZE};

const NBLOCKS: usize = 20000;
const NINODES: usize = 1000;

fn str2u8(s: &str) -> [u8; DIRSIZE] {
  let s_bytes = s.as_bytes();
  let mut result: [u8; DIRSIZE] = [0; DIRSIZE];
  for i in 0..s_bytes.len() {
    result[i] = s_bytes[i];
  }
  result
}

fn main() {
  let mut f = File::create(env::args_os().nth(1).unwrap()).unwrap();

  // Write NBLOCKS zeroed blocks into fs image.
  for _ in 0..NBLOCKS {
    f.write_all(&[0; BSIZE]).unwrap();
  }

  let ninodeblks = (NINODES / IPB + 1) as u32;
  let nbitmapblks = (NBLOCKS / BPB + 1) as u32;
  let nmeta = 2 + LOGSIZE as u32 + ninodeblks + nbitmapblks;

  let sb = SuperBlock {
    nblocks: NBLOCKS as u32,
    unused: 0,
    ninodes: NINODES as u32,
    nlogs: LOGSIZE as u32,
    log_start: 2,
    inode_start: 2 + LOGSIZE as u32,
    bmap_start: 2 + LOGSIZE as u32 + ninodeblks,
  };

  let mut nfree = nmeta;

  // Write the super block.
  f.seek(SeekFrom::Start(BSIZE as u64)).unwrap();
  f.write_all(&to_block!(&sb, SuperBlock)).unwrap();

  // Write the root inode and folder.
  let mut iroot = DiskInode {
    file_type: FileType::Directory,
    unused1: 0,
    unused2: 0,
    nlink: 1,
    size: size_of::<Dirent>() as u32 * 2, /* two files in root folder: `.`
                                           * and `..`. */
    addrs: [0; NDIRECT + 1],
  };
  let inode_blk0 = nfree;
  iroot.addrs[0] = inode_blk0;
  nfree += 1;

  f.seek(SeekFrom::Start(
    (sb.inode_start as usize * BSIZE +
       size_of::<DiskInode>()) as u64,
  )).unwrap();
  f.write_all(unsafe {
    &transmute::<_, [u8; size_of::<DiskInode>()]>(iroot)
  }).unwrap();

  let dirents: [Dirent; 2] = [
    Dirent {
      inum: 1,
      name: str2u8("."),
    },
    Dirent {
      inum: 1,
      name: str2u8(".."),
    },
  ];
  f.seek(SeekFrom::Start(inode_blk0 as u64 * BSIZE as u64))
    .unwrap();
  f.write_all(unsafe {
    &transmute::<_, [u8; size_of::<Dirent>() * 2]>(dirents)
  }).unwrap();

  // Write bitmap.

  // all used blocks should stay within one block in bitmap.
  assert!(nfree <= BPB as u32);

  let mut bitmap: [u8; BSIZE] = [0; BSIZE];
  for i in 0..nfree as usize {
    bitmap[i / 8] |= 1 << (i % 8);
  }
  f.seek(SeekFrom::Start(sb.bmap_start as u64 * BSIZE as u64))
    .unwrap();
  f.write_all(&bitmap).unwrap();

  f.flush().unwrap();
}
