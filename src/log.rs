use buffer::BCACHE;
use fs::{ROOTINO, LogBlock, SuperBlock};

struct Log {
  start: usize,
  size: usize,
  committing: bool,
  log_block: LogBlock,
}

fn read_sb() -> SuperBlock {
  from_block!(&BCACHE.read(ROOTINO).unwrap().data, SuperBlock)
}
