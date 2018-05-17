#[cfg(test)]
pub mod test {
  use std::mem::size_of;
  use disk::{BSIZE, Disk, Block};
  use fs::{SuperBlock, DiskInode, FileType, Dirent, IPB, BPB, LOGSIZE,
           NDIRECT, DIRSIZE};

  const NBLOCKS: usize = 200;
  const NINODES: usize = 20;

  fn str2u8(s: &str) -> [u8; DIRSIZE] {
    let s_bytes = s.as_bytes();
    let mut result: [u8; DIRSIZE] = [0; DIRSIZE];
    for i in 0..s_bytes.len() {
      result[i] = s_bytes[i];
    }
    result
  }

  #[allow(unused_unsafe)]
  pub fn create() -> (Disk, usize) {
    let mut b: [u8; NBLOCKS * BSIZE] = [0; NBLOCKS * BSIZE];
    let ptr = &mut b[0] as *mut u8;

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
    unsafe {
      *(ptr.add(BSIZE) as *mut _) = to_block!(&sb, SuperBlock);
    }

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

    unsafe {
      *(ptr.add(sb.inode_start as usize * BSIZE + size_of::<DiskInode>()) as
          *mut _) = iroot;
    }

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

    unsafe {
      *(ptr.add(inode_blk0 as usize * BSIZE) as *mut _) = dirents;
    }

    // Write bitmap.

    // all used blocks should stay within one block in bitmap.
    assert!(nfree <= BPB as u32);

    let mut bitmap: [u8; BSIZE] = [0; BSIZE];
    for i in 0..nfree as usize {
      bitmap[i / 8] |= 1 << (i % 8);
    }

    unsafe {
      *(ptr.add(sb.bmap_start as usize * BSIZE) as *mut _) = bitmap;
    }

    let mut disk: Vec<Block> = Vec::with_capacity(NBLOCKS);
    for i in 0..NBLOCKS {
      let mut buf = [0; BSIZE];
      buf.copy_from_slice(&b[i * BSIZE..(i + 1) * BSIZE]);
      disk.push(buf);
    }

    (Disk::from(disk), nfree as usize)
  }
}
