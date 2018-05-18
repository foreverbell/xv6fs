use buffer::BCACHE;
use disk::BSIZE;
use fs::BPB;
use logging::Transaction;

pub struct Bitmap;

impl Bitmap {
  // Zero `blockno`.
  fn zero<'a>(txn: &Transaction<'a>, blockno: usize) {
    let mut block = txn.read(blockno).unwrap();

    block.data = [0; BSIZE];
    txn.write(&mut block);
  }

  // Allocate a new block and mark it used in block bitmap.
  pub fn alloc<'a>(txn: &Transaction<'a>) -> usize {
    let sb = BCACHE.sb();
    let nblocks = sb.nblocks as usize;

    for b in 0..nblocks / BPB {
      let mut block = txn.read(sb.bblock(b * BPB)).unwrap();

      for j in 0..BPB {
        let i = b * BPB + j;
        if i >= nblocks {
          break;
        }
        let mask = 1 << (j % 8);
        if (block.data[j / 8] & mask) == 0 {
          block.data[j / 8] |= mask;
          txn.write(&mut block);
          Bitmap::zero(txn, i);
          return i;
        }
      }
    }
    panic!("no free block");
  }

  // Free a block.
  pub fn free<'a>(txn: &Transaction<'a>, blockno: usize) {
    let sb = BCACHE.sb();
    let mut block = txn.read(sb.bblock(blockno)).unwrap();
    let i = blockno % BPB;
    let mask = 1 << (i % 8);

    assert!(block.data[i / 8] & mask != 0);

    block.data[i / 8] &= !mask;
    txn.write(&mut block);
  }
}

#[cfg(test)]
mod test {
  #[test]
  fn test() {
    use bitmap::Bitmap;
    use buffer::BCACHE;
    use disk::DISK;
    use logging::LOGGING;
    use testfs;

    #[test]
    fn test() {
      let (disk, nfree) = testfs::test::create();
      DISK.mount(disk);
      BCACHE.init();

      let txn = LOGGING.new_txn();
      for i in 0..30 {
        assert!(Bitmap::alloc(&txn) == nfree + i);
      }
      Bitmap::free(&txn, nfree + 10);
      assert!(Bitmap::alloc(&txn) == nfree + 10);
    }
  }
}
