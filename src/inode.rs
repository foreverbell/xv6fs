use fs::DiskInode;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use util::locked::{LockedItem, UnlockedItem};

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
    Inode {
      inode: None,
    }
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
}
