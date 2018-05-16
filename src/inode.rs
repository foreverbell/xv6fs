use buffer::BCACHE;
use disk::Block;
use fs::{DiskInode, FileType, IPB, ROOTINO};
use logging::Transaction;
use std::collections::HashMap;
use std::mem::transmute;
use std::sync::{Arc, Mutex};
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
    let mut inode: Option<UnlockedInode>;
    let mut cache = self.cache.lock().unwrap();

    inode = cache.get_mut(&inodeno).map(|inode| {
      UnlockedInode::new(inode.clone(), inode.no)
    });
    if inode.is_none() {
      if cache.len() >= self.capacity {
        let mut free_nos = vec![];

        for (inodeno2, inode2) in cache.iter() {
          if inode2.refcnt() == 0 {
            free_nos.push(*inodeno2);
          }
        }
        if free_nos.is_empty() {
          return None;
        }
        for inodeno2 in free_nos {
          cache.remove(&inodeno2);
        }
      }

      let new_inode = Arc::new(Mutex::new(Inode::new()));
      inode = Some(UnlockedInode::new(new_inode.clone(), inodeno));
      cache.insert(inodeno, UnlockedInode::new(new_inode.clone(), inodeno));
    }
    inode
  }

  pub fn lock<'a, 'b>(
    &self,
    txn: &Transaction<'a>,
    inode: &UnlockedInode,
  ) -> LockedInode<'b> {
    let mut inode = inode.acquire();
    let sb = BCACHE.sb();

    if inode.inode.is_some() {
      return inode;
    }
    let buf = txn.read(sb.iblock(inode.no)).unwrap();
    let inodes: [DiskInode; IPB] = unsafe { transmute::<Block, _>(buf.data) };

    assert!(inodes[inode.no % IPB].file_type != FileType::None);

    inode.inode = Some(inodes[inode.no % IPB]);
    inode
  }
}

impl UnlockedDrop for UnlockedInode {
  fn drop(&mut self) {
    if self.refcnt() == 0 {
      return;
    }
    let inode = self.acquire(); // acquiring lock here is expensive?
    if let Some(inode) = inode.inode {
      if inode.nlink == 0 {
        // truncate file
      }
    }
  }
}
