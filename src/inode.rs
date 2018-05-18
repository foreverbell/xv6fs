use bitmap::Bitmap;
use buffer::BCACHE;
use disk::BSIZE;
use fs::{DiskInode, FileType, IPB, ROOTINO, NDIRECT, NINDIRECT, MAXFILESIZE,
         Dirent, DIRSIZE};
use logging::{LOGGING, Transaction};
use std::cmp::min;
use std::collections::HashMap;
use std::mem::{transmute, size_of};
use std::ops::{Deref, DerefMut};
use std::sync::{Arc, Mutex};
use util::locked::{LockedItem, UnlockedItem, UnlockedDrop};

pub struct Inode {
  inode: Option<DiskInode>,
  no: usize,
}

impl Deref for Inode {
  type Target = DiskInode;
  fn deref(&self) -> &DiskInode {
    self.inode.as_ref().unwrap()
  }
}

impl DerefMut for Inode {
  fn deref_mut(&mut self) -> &mut DiskInode {
    self.inode.as_mut().unwrap()
  }
}

pub struct Directory<'a> {
  inode: &'a mut Inode,
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
  fn new(no: usize) -> Self {
    Inode { inode: None, no }
  }

  pub fn as_directory<'a>(&'a mut self) -> Directory<'a> {
    assert!(
      self.inode.is_some() &&
        self.inode.as_ref().unwrap().file_type == FileType::Directory
    );
    Directory { inode: self }
  }

  pub fn update<'a>(&self, txn: &Transaction<'a>) {
    assert!(self.inode.is_some());
    let sb = BCACHE.sb();
    let mut buf = txn.read(sb.iblock(self.no)).unwrap();
    let inodes: &mut [DiskInode; IPB] = unsafe { transmute(&mut buf.data) };

    inodes[self.no % IPB] = self.inode.as_ref().unwrap().clone();
    txn.write(&mut buf);
  }

  pub fn nth_block<'a>(
    &mut self,
    txn: &Transaction<'a>,
    n: usize,
  ) -> Option<usize> {
    assert!(self.inode.is_some());
    let inode = self.inode.as_mut().unwrap();

    if n < NDIRECT {
      if inode.addrs[n] == 0 {
        inode.addrs[n] = Bitmap::alloc(txn) as u32;
      }
      return Some(inode.addrs[n] as usize);
    }
    let n = n - NDIRECT;
    if n < NINDIRECT {
      if inode.addrs[NDIRECT] == 0 {
        inode.addrs[NDIRECT] = Bitmap::alloc(txn) as u32;
      }
      let mut buf = txn.read(inode.addrs[NDIRECT] as usize).unwrap();
      let a: &mut [u32; NINDIRECT] = unsafe { transmute(&mut buf.data) };
      if a[n] == 0 {
        a[n] = Bitmap::alloc(txn) as u32;
      }
      txn.write(&mut buf);
    }
    None
  }

  pub fn read<'a>(
    &mut self,
    txn: &Transaction<'a>,
    offset: usize,
    mut n: usize,
  ) -> Option<Vec<u8>> {
    assert!(self.inode.is_some());
    let inode_size = self.inode.as_ref().unwrap().size;

    if offset > inode_size as usize || offset.saturating_add(n) != offset + n ||
      offset + n > MAXFILESIZE
    {
      return None;
    }
    if offset + n > inode_size as usize {
      n = inode_size as usize - offset;
    }

    let mut result = Vec::with_capacity(n);
    let mut cur_offset = offset;
    let mut got = 0;

    while got < n {
      let buf = txn
        .read(self.nth_block(txn, cur_offset / BSIZE).unwrap())
        .unwrap()
        .data;
      let from = cur_offset % BSIZE;
      let m = min(n - got, BSIZE - from);

      for i in from..(from + m) {
        result.push(buf[i]);
      }
      got += m;
      cur_offset += m;
    }
    Some(result)
  }

  pub fn write<'a>(
    &mut self,
    txn: &Transaction<'a>,
    offset: usize,
    data: &[u8],
  ) -> Option<usize> {
    assert!(self.inode.is_some());
    let inode_size = self.inode.as_ref().unwrap().size as usize;
    let n = data.len();

    if offset > inode_size || offset.saturating_add(n) != offset + n ||
      offset + n > MAXFILESIZE
    {
      return None;
    }

    let mut cur_offset = offset;
    let mut written = 0;

    while written < n {
      let mut buf = txn
        .read(self.nth_block(txn, cur_offset / BSIZE).unwrap())
        .unwrap();
      let from = cur_offset % BSIZE;
      let m = min(n - written, BSIZE - from);

      for i in from..(from + m) {
        buf.data[i] = data[i - from + written];
      }
      txn.write(&mut buf);
      written += m;
      cur_offset += m;
    }

    if written > 0 && cur_offset > inode_size as usize {
      self.inode.as_mut().unwrap().size = cur_offset as u32;
      self.update(txn);
    }
    Some(written)
  }
}

impl<'a> Directory<'a> {
  fn inode(&self) -> &DiskInode {
    self.inode.inode.as_ref().unwrap()
  }

  pub fn enumerate<'b>(
    &mut self,
    txn: &Transaction<'b>,
  ) -> Vec<(UnlockedInode, [u8; DIRSIZE])> {
    let nentries = self.inode().size as usize / size_of::<Dirent>();
    let mut result = vec![];
    let mut cur_index = 0;

    while cur_index < nentries {
      let m = min((nentries - cur_index) * size_of::<Dirent>(), BSIZE);
      let buf = self
        .inode
        .read(txn, cur_index * size_of::<Dirent>(), m)
        .unwrap();

      assert!(buf.len() == m);
      assert!(m % size_of::<Dirent>() == 0);

      for i in 0..(m / size_of::<Dirent>()) {
        let ent: &Dirent =
          unsafe { &*(buf.as_slice().as_ptr() as *const Dirent).add(i) };

        if ent.inum != 0 {
          result.push((ICACHE.get(ent.inum as usize).unwrap(), ent.name));
        }
      }
      cur_index += m / size_of::<Dirent>();
    }
    result
  }

  pub fn is_empty<'b>(&mut self, txn: &Transaction<'b>) -> bool {
    self.enumerate(txn).len() == 2
  }

  pub fn lookup<'b>(
    &mut self,
    txn: &Transaction<'b>,
    name: &[u8; DIRSIZE],
  ) -> Option<(UnlockedInode, usize)> {
    let nentries = self.inode().size as usize / size_of::<Dirent>();
    let mut cur_index = 0;

    while cur_index < nentries {
      let m = min((nentries - cur_index) * size_of::<Dirent>(), BSIZE);
      let buf = self.inode.read(txn, cur_index * size_of::<Dirent>(), m)?;

      assert!(buf.len() == m);
      assert!(m % size_of::<Dirent>() == 0);

      for i in 0..(m / size_of::<Dirent>()) {
        let ent: &Dirent =
          unsafe { &*(buf.as_slice().as_ptr() as *const Dirent).add(i) };

        if ent.inum != 0 && ent.name == *name {
          return Some((
            ICACHE.get(ent.inum as usize).unwrap(),
            (cur_index + i) * size_of::<Dirent>(),
          ));
        }
      }
      cur_index += m / size_of::<Dirent>();
    }
    None
  }

  pub fn link<'b>(
    &mut self,
    txn: &Transaction<'b>,
    name: &[u8; DIRSIZE],
    inum: u16,
  ) -> bool {
    assert!(inum > 0);

    if self.lookup(txn, name).is_some() {
      return false;
    }

    let nentries = self.inode().size as usize / size_of::<Dirent>();
    let mut cur_index = 0;

    while cur_index < nentries {
      let m = min((nentries - cur_index) * size_of::<Dirent>(), BSIZE);
      let buf = self
        .inode
        .read(txn, cur_index * size_of::<Dirent>(), m)
        .unwrap();

      assert!(buf.len() == m);
      assert!(m % size_of::<Dirent>() == 0);

      let mut found = false;
      for i in 0..(m / size_of::<Dirent>()) {
        let ent: &Dirent =
          unsafe { &*(buf.as_slice().as_ptr() as *const Dirent).add(i) };

        if ent.inum == 0 {
          cur_index += i;
          found = true;
          break;
        }
      }
      if found {
        break;
      } else {
        cur_index += m / size_of::<Dirent>();
      }
    }

    let ent_bytes: [u8; size_of::<Dirent>()] = unsafe {
      transmute(Dirent {
        name: *name,
        inum: inum,
      })
    };
    self
      .inode
      .write(txn, cur_index * size_of::<Dirent>(), &ent_bytes)
      .unwrap() == ent_bytes.len()
  }
}

impl Cache {
  fn new(capacity: usize) -> Self {
    Cache {
      capacity: capacity,
      cache: Mutex::new(HashMap::with_capacity(capacity)),
    }
  }

  pub fn init(&self) {
    self.cache.lock().unwrap().clear();
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
      let inodes: &mut [DiskInode; IPB] = unsafe { transmute(&mut buf.data) };

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

  pub fn get(&self, inodeno: usize) -> Option<UnlockedInode> {
    let mut inode: Option<UnlockedInode>;
    let mut cache = self.cache.lock().unwrap();

    inode = cache.get_mut(&inodeno).map(|inode| inode.clone());
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

      let new_inode = Arc::new((Mutex::new(Inode::new(inodeno)), inodeno));
      inode = Some(UnlockedInode::new(new_inode.clone()));
      cache.insert(inodeno, UnlockedInode::new(new_inode.clone()));
    }
    inode
  }

  fn put<'a>(&self, txn: &Transaction<'a>, inode: &UnlockedInode) {
    if inode.refcnt() == 0 {
      return;
    }
    let inode = self.lock(txn, inode); // acquiring lock here is expensive?
    if let Some(inode) = inode.inode.as_ref() {
      if inode.nlink == 0 {
        // truncate file
      }
    }
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
    let inodes: &[DiskInode; IPB] = unsafe { transmute(&buf.data) };

    assert!(inodes[inode.no % IPB].file_type != FileType::None);

    inode.inode = Some(inodes[inode.no % IPB].clone());
    inode
  }
}

impl UnlockedDrop for UnlockedInode {
  fn drop(&mut self) {
    let txn = LOGGING.new_nested_txn();
    ICACHE.put(&txn, self);
  }
}
