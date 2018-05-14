use fs::LogBlock;

struct Log {
  start: usize,
  size: usize,
  committing: bool,
  log_block: LogBlock,
}
