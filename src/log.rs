use buffer::BCACHE;
use fs::{ROOTINO, LOGSIZE, LogBlock, SuperBlock};
use std::mem::size_of;
use std::sync::{Mutex, Condvar};
use disk::BSIZE;

struct LogState {
  committing: bool,
  outstanding: usize,
}

struct Log {
  start: usize,
  size: usize,
  state: Mutex<LogState>,
  condvar: Condvar,
  logs: Mutex<LogBlock>,
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
      logs: Mutex::new(LogBlock {
        n: 0,
        blocks: [0; LOGSIZE],
      }),
    }
  }

  pub fn init(&mut self) {
    assert!(size_of::<LogBlock>() <= BSIZE);

    let super_block = read_super_block();

    self.start = super_block.log_start as usize;
    self.size = super_block.nlog as usize;

    self.recover();
  }


  fn read_head(&self, logs: &mut LogBlock) {
    let buf = BCACHE.read(self.start).unwrap();

    *logs = from_block!(&buf.data, LogBlock);
  }

  fn write_head(&self, logs: &LogBlock) {
    let mut buf = BCACHE.read(self.start).unwrap();

    buf.data = to_block!(logs, LogBlock);
    BCACHE.write(&mut buf);
  }

  fn install_txn(&self, logs: &LogBlock) {
    for i in 0..(logs.n as usize) {
      let src_blockno = (self.start as usize) + i + 1;
      let dst_blockno = logs.blocks[i] as usize;

      let src_buf = BCACHE.read(src_blockno).unwrap();
      let mut dst_buf = BCACHE.read(dst_blockno).unwrap();

      dst_buf.data = src_buf.data;
      BCACHE.write(&mut dst_buf);
    }
  }

  fn recover(&self) {
    let logs = &mut *self.logs.lock().unwrap();

    self.read_head(logs);
    self.install_txn(logs);
    logs.n = 0;
    self.write_head(logs);
  }
}
