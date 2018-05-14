use buffer::{BCACHE, LockedBuf};
use disk::BSIZE;
use fs::{ROOTINO, LOGSIZE, LogHeader, SuperBlock};
use std::mem::size_of;
use std::sync::{Mutex, Condvar};

// We define LOGSIZE as 64 in fs.rs, thus allow maximum 4 concurrent txns.
const MAXOPBLOCKS: usize = 16;

struct LogState {
  committing: bool,
  outstanding: usize,
}

struct Log {
  start: usize,
  size: usize,
  state: Mutex<LogState>,
  condvar: Condvar,
  lh: Mutex<LogHeader>,
}

struct Transaction<'a> {
  log: &'a Log,
}

fn read_super_block() -> SuperBlock {
  from_block!(&BCACHE.read(ROOTINO).unwrap().data, SuperBlock)
}

impl Log {
  fn new() -> Self {
    Log {
      start: 0,
      size: 0,
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

  pub fn init(&mut self) {
    assert!(size_of::<LogHeader>() <= BSIZE);

    let super_block = read_super_block();

    self.start = super_block.log_start as usize;
    self.size = super_block.nlog as usize;

    assert!(self.size <= LOGSIZE);

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

impl<'a> Transaction<'a> {
  fn new(log: &'a Log) -> Self {
    Transaction { log: log }
  }

  fn begin_txn(&self) {
    let mut state = self.log.state.lock().unwrap();

    loop {
      if state.committing {
        state = self.log.condvar.wait(state).unwrap();
      } else if (state.outstanding + 1) * MAXOPBLOCKS > self.log.size {
        state = self.log.condvar.wait(state).unwrap();
      } else {
        state.outstanding += 1;
        break;
      }
    }
  }

  fn end_txn(&self) {
    let mut state = self.log.state.lock().unwrap();
    let mut do_commit = false;

    assert!(state.outstanding > 0);
    assert!(!state.committing);

    state.outstanding -= 1;

    if state.outstanding == 0 {
      state.committing = true;
      do_commit = true;
    } else {
      self.log.condvar.notify_all();
    }

    drop(state);

    if do_commit {
      self.commit();
      self.log.state.lock().unwrap().committing = false;
      self.log.condvar.notify_all();
    }
  }

  fn commit(&self) {
    let mut lh = self.log.lh.lock().unwrap();

    if lh.n > 0 {
      self.log.write_log(&lh);
      self.log.write_head(&lh); // commit point
      self.log.install_txn(&lh);
      lh.n = 0;
      self.log.write_head(&lh);
    }
  }

  pub fn write<'b>(&self, buf: &mut LockedBuf<'b>) {
    let mut lh = self.log.lh.lock().unwrap();

    if lh.n as usize >= self.log.size - 1 {
      panic!("too big transaction");
    }

    let mut lh_index = None;
    for i in 0..(lh.n as usize) {
      if lh.blocks[i] as usize == buf.blockno {
        lh_index = Some(i);
        break;
      }
    }
    if lh_index.is_none() {
      lh_index = Some(lh.n as usize);
      lh.n += 1;
    }
    lh.blocks[lh_index.unwrap()] = buf.blockno as u32;
    buf.pin();
  }
}

impl<'a> Drop for Transaction<'a> {
  fn drop(&mut self) {
    self.end_txn()
  }
}
