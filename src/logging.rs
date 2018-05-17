use buffer::{BCACHE, LockedBuf};
use disk::BSIZE;
use fs::{LOGSIZE, LogHeader};
use std::mem::size_of;
use std::sync::{Mutex, Condvar};

// TODO: failpoint testing.
// https://github.com/pingcap/fail-rs

// We define LOGSIZE as 64 in fs.rs, thus allow maximum 4
// concurrent txns.
const MAXOPBLOCKS: usize = 16;

struct LogState {
  committing: bool,
  outstanding: usize,
}

pub struct Logging {
  start: usize,
  size: usize,
  state: Mutex<LogState>,
  condvar: Condvar,
  lh: Mutex<LogHeader>,
}

// TODO: nested transaction.
pub struct Transaction<'a> {
  logging: &'a Logging,
}

lazy_static! {
  pub static ref LOGGING: Logging = Logging::new();
}

impl Logging {
  fn new() -> Self {
    let sb = BCACHE.sb();

    assert!(size_of::<LogHeader>() <= BSIZE);
    assert!(sb.nlogs as usize <= LOGSIZE);

    Logging {
      start: sb.log_start as usize,
      size: sb.nlogs as usize,
      state: Mutex::new(LogState {
        committing: false,
        outstanding: 0,
      }),
      condvar: Condvar::new(),
      lh: Mutex::new(LogHeader {
        n: 0,
        blocks: [0; LOGSIZE],
      }),
    }
  }

  pub fn init(&self) {
    *self.state.lock().unwrap() = LogState {
      committing: false,
      outstanding: 0,
    };
    *self.lh.lock().unwrap() = LogHeader {
      n: 0,
      blocks: [0; LOGSIZE],
    };
    self.recover();
  }

  fn read_head(&self, lh: &mut LogHeader) {
    let buf = BCACHE.read(self.start).unwrap();

    *lh = from_block!(&buf.data, LogHeader);
  }

  fn write_head(&self, lh: &LogHeader) {
    let mut buf = BCACHE.read(self.start).unwrap();

    buf.data = to_block!(lh, LogHeader);
    BCACHE.write(&mut buf);
  }

  fn write_log(&self, lh: &LogHeader) {
    for i in 0..(lh.n as usize) {
      let src_blockno = lh.blocks[i] as usize;
      let dst_blockno = (self.start as usize) + i + 1;

      let src_buf = BCACHE.read(src_blockno).unwrap();
      let mut dst_buf = BCACHE.read(dst_blockno).unwrap();

      dst_buf.data = src_buf.data;
      BCACHE.write(&mut dst_buf);
    }
  }

  fn install_txn(&self, lh: &LogHeader) {
    for i in 0..(lh.n as usize) {
      let src_blockno = (self.start as usize) + i + 1;
      let dst_blockno = lh.blocks[i] as usize;

      let src_buf = BCACHE.read(src_blockno).unwrap();
      let mut dst_buf = BCACHE.read(dst_blockno).unwrap();

      dst_buf.data = src_buf.data;
      BCACHE.write(&mut dst_buf);
    }
  }

  fn recover(&self) {
    let lh = &mut *self.lh.lock().unwrap();

    self.read_head(lh);
    self.install_txn(lh);
    lh.n = 0;
    self.write_head(lh);
  }

  pub fn new_txn<'a>(&'a self) -> Transaction<'a> {
    let txn = Transaction::new(self);
    txn.begin_txn();
    txn
  }
}

// RAII transaction, which acts as a proxy for block cache read and
// write.
impl<'a> Transaction<'a> {
  fn new(logging: &'a Logging) -> Self {
    Transaction { logging }
  }

  fn begin_txn(&self) {
    let mut state = self.logging.state.lock().unwrap();

    loop {
      if state.committing {
        state = self.logging.condvar.wait(state).unwrap();
      } else if (state.outstanding + 1) * MAXOPBLOCKS > self.logging.size {
        state = self.logging.condvar.wait(state).unwrap();
      } else {
        state.outstanding += 1;
        break;
      }
    }
  }

  fn end_txn(&self) {
    let mut state = self.logging.state.lock().unwrap();
    let mut do_commit = false;

    assert!(state.outstanding > 0);
    assert!(!state.committing);

    state.outstanding -= 1;

    if state.outstanding == 0 {
      state.committing = true;
      do_commit = true;
    } else {
      self.logging.condvar.notify_all();
    }

    drop(state);

    if do_commit {
      self.commit();
      self.logging.state.lock().unwrap().committing = false;
      self.logging.condvar.notify_all();
    }
  }

  fn commit(&self) {
    let mut lh = self.logging.lh.lock().unwrap();

    if lh.n > 0 {
      info!("committing {} blocks", lh.n);

      self.logging.write_log(&lh);
      self.logging.write_head(&lh); // commit point
      self.logging.install_txn(&lh);
      lh.n = 0;
      self.logging.write_head(&lh);
    }
  }

  pub fn read<'b>(&self, blockno: usize) -> Option<LockedBuf<'a>> {
    BCACHE.read(blockno)
  }

  pub fn write<'b>(&self, buf: &mut LockedBuf<'b>) {
    let mut lh = self.logging.lh.lock().unwrap();

    if lh.n as usize >= self.logging.size - 1 {
      panic!("too big transaction");
    }

    let mut lh_index = None;
    for i in 0..(lh.n as usize) {
      if lh.blocks[i] as usize == buf.no {
        lh_index = Some(i);
        break;
      }
    }
    if lh_index.is_none() {
      lh_index = Some(lh.n as usize);
      lh.n += 1;
    }
    lh.blocks[lh_index.unwrap()] = buf.no as u32;

    // Pin this buffer in cache to avoid being evicted.
    BCACHE.pin(buf);
  }
}

impl<'a> Drop for Transaction<'a> {
  fn drop(&mut self) {
    self.end_txn()
  }
}

#[cfg(test)]
mod test {
  use buffer::BCACHE;
  use disk::DISK;
  use logging::LOGGING;
  use testfs;

  #[test]
  fn test() {
    let (disk, nfree) = testfs::test::create();
    DISK.lock().unwrap().mount(disk);
    BCACHE.init();

    {
      let txn = LOGGING.new_txn();

      let mut buf1 = txn.read(nfree).unwrap();
      buf1.data[0] = 42;
      txn.write(&mut buf1);

      let mut buf2 = txn.read(nfree + 1).unwrap();
      buf2.data[0] = 100;
      txn.write(&mut buf2);

      assert!(BCACHE.nitems() == 2);
      assert!(LOGGING.state.lock().unwrap().outstanding == 1);
      assert!(LOGGING.lh.lock().unwrap().n == 2);
    }

    BCACHE.init();
    assert!(BCACHE.nitems() == 0);
    assert!(LOGGING.state.lock().unwrap().outstanding == 0);
    assert!(LOGGING.lh.lock().unwrap().n == 0);

    {
      let txn = LOGGING.new_txn();

      let buf1 = txn.read(nfree).unwrap();
      assert!(buf1.data[0] == 42);

      let buf2 = txn.read(nfree + 1).unwrap();
      assert!(buf2.data[0] == 100);
    }
  }
}
