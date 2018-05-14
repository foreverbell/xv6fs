use disk::{BSIZE, Block, DISK};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use utils::locked::LockedItem;

bitflags! {
  struct BufFlags: u32 {
    const VALID = 0b01;
    const DIRTY = 0b10;
  }
}

pub struct Buf {
  blockno: usize,
  flags: BufFlags,
  pub data: Block,
}

pub type LockedBuf<'a> = LockedItem<'a, Buf>;

pub struct Cache {
  capacity: usize,
  cache: Mutex<HashMap<usize, Arc<Mutex<Buf>>>>,
}

lazy_static! {
  pub static ref BCACHE: Cache = Cache::new(256);
}

impl Buf {
  fn new(blockno: usize) -> Self {
    Buf {
      blockno: blockno,
      flags: BufFlags::empty(),
      data: [0; BSIZE],
    }
  }
}

impl Cache {
  fn new(capacity: usize) -> Self {
    Cache {
      capacity: capacity,
      cache: Mutex::new(HashMap::new()),
    }
  }

  pub fn get<'a>(&self, blockno: usize) -> Option<LockedBuf<'a>> {
    let mut buf: Option<Arc<Mutex<Buf>>>;

    {
      let mut cache = self.cache.lock().unwrap();

      buf = cache.get_mut(&blockno).map(|buf| buf.clone());
      if buf.is_none() {
        while cache.len() >= self.capacity {
          let mut unused_blockno = None;

          for (blockno2, buf2) in cache.iter() {
            if Arc::strong_count(&buf2) == 1 {
              if !buf2.lock().unwrap().flags.contains(BufFlags::DIRTY) {
                unused_blockno = Some(*blockno2);
                break;
              }
            }
          }

          match unused_blockno {
            Some(blockno) => cache.remove(&blockno),
            None => return None,
          };
        }

        buf = Some(Arc::new(Mutex::new(Buf::new(blockno))));
        cache.insert(blockno, buf.as_mut().unwrap().clone());
      }
    }

    buf.map(|buf| LockedBuf::new(buf))
  }

  pub fn read<'a>(&self, blockno: usize) -> Option<LockedBuf<'a>> {
    let mut buf = self.get(blockno)?;

    if !buf.flags.contains(BufFlags::VALID) {
      buf.data = DISK.lock().unwrap().read(blockno);
      buf.flags.insert(BufFlags::VALID);
    }
    Some(buf)
  }

  pub fn write<'a>(&self, buf: &mut LockedBuf<'a>) {
    DISK.lock().unwrap().write(buf.blockno, &buf.data);
    buf.flags.remove(BufFlags::DIRTY);
  }
}
