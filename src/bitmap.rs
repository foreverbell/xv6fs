use buffer::BCACHE;
use disk::BSIZE;
use fs::BPB;
use logging::Transaction;

struct Bitmap;

impl Bitmap {
  // Zero `blockno`.
  fn zero<'a>(txn: &Transaction<'a>, blockno: usize) {
    let mut block = txn.read(blockno).unwrap();

    block.data = [0; BSIZE];
    txn.write(&mut block);
  }

  pub fn alloc<'a>(txn: &Transaction<'a>) -> Option<usize> {
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
          return Some(i);
        }
      }
    }
    None
  }

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
