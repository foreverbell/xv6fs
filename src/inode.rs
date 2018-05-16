use buffer::BCACHE;
use disk::Block;
use fs::{DiskInode, FileType, IPB, ROOTINO};
use logging::Transaction;
use std::collections::HashMap;
use std::mem::transmute;
use std::sync::Mutex;
use util::locked::{LockedItem, UnlockedItem, UnlockedDrop};

pub struct Inode {
  pub inode: Option<DiskInode>,
}

pub type LockedInode<'a> = LockedItem<'a, Inode, usize /* inodeno */>;
pub type UnlockedInode = UnlockedItem<Inode, usize /* inodeno */>;

pub struct Cache {
  capacity: usize,
  cache: Mutex<HashMap<usize, UnlockedInode>>,
}

lazy_static! {
  pub static ref ICACHE: Cache = Cache::new(256);
}

impl Inode {
  fn new() -> Self {
    Inode { inode: None }
  }
}

impl Cache {
  fn new(capacity: usize) -> Self {
    Cache {
      capacity: capacity,
      cache: Mutex::new(HashMap::with_capacity(capacity)),
    }
  }

  pub fn capacity(&self) -> usize {
    self.capacity
  }

  pub fn nitems(&self) -> usize {
    self.cache.lock().unwrap().len()
  }

  pub fn alloc<'a>(
    &self,
    txn: &Transaction<'a>,
    file_type: FileType,
  ) -> Option<UnlockedInode> {
    let sb = BCACHE.sb();
    let ninodes = sb.ninodes as usize;

    for b in 0..ninodes / IPB {
      let mut buf = txn.read(sb.iblock(b * IPB)).unwrap();
      let mut inodes: [DiskInode; IPB] =
        unsafe { transmute::<Block, _>(buf.data) };

      for j in 0..IPB {
        let i = b * IPB + j;
        if i <= ROOTINO {
          continue;
        } else if i >= ninodes {
          break;
        }
        if inodes[j].file_type == FileType::None {
          inodes[j].init(file_type);
          txn.write(&mut buf);
          drop(buf);
          return self.get(i);
        }
      }
    }
    None
  }

  pub fn update<'a, 'b>(&self, txn: &Transaction<'a>, inode: &LockedInode<'b>) {
    let sb = BCACHE.sb();
    let mut buf = txn.read(sb.iblock(inode.no)).unwrap();
    let mut inodes: [DiskInode; IPB] =
      unsafe { transmute::<Block, _>(buf.data) };

    inodes[inode.no % IPB] = inode.inode.unwrap();
    txn.write(&mut buf);
  }

  pub fn get(&self, inodeno: usize) -> Option<UnlockedInode> {
    let mut cache = self.cache.lock().unwrap();

    unimplemented!();
  }

  pub fn lock<'a>(&self, inode: &UnlockedInode) -> LockedInode<'a> {
    unimplemented!();
  }
}

impl UnlockedDrop for UnlockedInode {
  fn drop(&mut self) {
    unimplemented!();
  }
}
