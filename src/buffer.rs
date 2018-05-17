use disk::{BSIZE, Block, DISK};
use fs::SuperBlock;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use util::locked::{LockedItem, UnlockedItem};

bitflags! {
  struct BufFlags: u32 {
    const VALID = 0b01;
    const DIRTY = 0b10;
  }
}

pub struct Buf {
  pub data: Block,
  flags: BufFlags,
}

pub type LockedBuf<'a> = LockedItem<'a, Buf, usize /* blockno */>;
pub type UnlockedBuf = UnlockedItem<Buf, usize /* blockno */>;

pub struct Cache {
  capacity: usize,
  cache: Mutex<HashMap<usize, UnlockedBuf>>,
}

lazy_static! {
  pub static ref BCACHE: Cache = Cache::new(256);

  // Block 1 is immutable after file system is created, so we can safely
  // store it here.
  static ref SB: SuperBlock = from_block!(
    &DISK.lock().unwrap().read(1), SuperBlock
  );
}

impl Buf {
  fn new() -> Self {
    Buf {
      flags: BufFlags::empty(),
      data: [0; BSIZE],
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

  #[cfg(test)]
  pub fn init(&self) {
    self.cache.lock().unwrap().clear();
  }

  #[cfg(test)]
  pub fn capacity(&self) -> usize {
    self.capacity
  }

  #[cfg(test)]
  pub fn nitems(&self) -> usize {
    self.cache.lock().unwrap().len()
  }

  pub fn sb(&self) -> &SuperBlock {
    &SB
  }

  pub fn get(&self, blockno: usize) -> Option<UnlockedBuf> {
    let mut buf: Option<UnlockedBuf>;
    let mut cache = self.cache.lock().unwrap();

    buf = cache.get_mut(&blockno).map(|buf| buf.clone());
    if buf.is_none() {
      if cache.len() >= self.capacity {
        let mut free_nos = vec![];

        for (blockno2, buf2) in cache.iter() {
          if buf2.refcnt() == 0 {
            if !buf2.acquire().flags.contains(BufFlags::DIRTY) {
              free_nos.push(*blockno2);
              if cache.len() - free_nos.len() < self.capacity {
                break;
              }
            }
          }
        }
        if free_nos.is_empty() {
          return None;
        }
        for blockno2 in free_nos {
          cache.remove(&blockno2);
        }
      }

      let new_buf = Arc::new((Mutex::new(Buf::new()), blockno));
      buf = Some(UnlockedBuf::new(new_buf.clone()));
      cache.insert(blockno, UnlockedBuf::new(new_buf.clone()));
    }
    buf
  }

  pub fn read<'a>(&self, blockno: usize) -> Option<LockedBuf<'a>> {
    let mut buf = self.get(blockno)?.acquire();

    if !buf.flags.contains(BufFlags::VALID) {
      buf.data = DISK.lock().unwrap().read(blockno);
      buf.flags.insert(BufFlags::VALID);
    }
    Some(buf)
  }

  pub fn write<'a>(&self, buf: &mut LockedBuf<'a>) {
    DISK.lock().unwrap().write(buf.no, &buf.data);
    buf.flags.remove(BufFlags::DIRTY);
  }

  // Pins this buf in cache.
  pub fn pin<'a>(&self, buf: &mut LockedBuf<'a>) {
    buf.flags.insert(BufFlags::DIRTY);
  }
}

#[cfg(test)]
mod test {
  use buffer::{BCACHE, BufFlags};
  use disk::{Disk, DISK};

  #[test]
  fn test1() {
    let disk = Disk::new(1024);
    DISK.lock().unwrap().mount(disk);
    BCACHE.init();

    for i in 0..256 {
      assert!(BCACHE.get(i).is_some());
    }
    println!("{}", BCACHE.nitems());
    assert!(BCACHE.nitems() == 256);
    // A stale entry is evicted.
    assert!(BCACHE.get(300).is_some());
    assert!(BCACHE.nitems() == 256);
  }

  #[test]
  fn test2() {
    let disk = Disk::new(1024);
    DISK.lock().unwrap().mount(disk);
    BCACHE.init();

    for i in 0..256 {
      let b = BCACHE.get(i);
      assert!(b.is_some());
      // Mark every newly-inserted cache entry as inevictable.
      b.unwrap().acquire().flags.insert(BufFlags::DIRTY);
    }
    assert!(BCACHE.nitems() == 256);
    // Cache is full, we cannot insert any new entries.
    assert!(BCACHE.get(300).is_none());
  }


  #[test]
  fn test3() {
    let disk = Disk::new(1024);
    DISK.lock().unwrap().mount(disk);
    BCACHE.init();

    let mut vec = vec![];

    for i in 0..256 {
      let b = BCACHE.get(i);
      assert!(b.is_some());
      // Store a reference at somewhere else to prevent this block from
      // evicting.
      vec.push(b.unwrap());
    }
    assert!(BCACHE.nitems() == 256);
    // Cache is full, we cannot insert any new entries.
    assert!(BCACHE.get(300).is_none());
  }

  #[test]
  fn test4() {
    let disk = Disk::new(1024);
    DISK.lock().unwrap().mount(disk);
    BCACHE.init();

    {
      let mut b = BCACHE.read(1000).unwrap();
      assert!(b.data[0] == 0);
      b.data[0] = 42;
      BCACHE.write(&mut b);
    }

    {
      let b = BCACHE.get(1000).unwrap();
      // The data still exists in memory even if we only `get` it.
      assert!(b.acquire().data[0] == 42);
    }

    {
      for i in 0..255 {
        BCACHE.pin(&mut BCACHE.get(i).unwrap().acquire());
      }
      BCACHE.get(255).unwrap();

      let b = BCACHE.get(1000).unwrap();
      // Data is lost because block 1000 is evicted.
      assert!(b.acquire().data[0] == 0);
    }
  }
}
