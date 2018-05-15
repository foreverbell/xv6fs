use util::locked::LockedItem;
use fs::DiskInode;

pub struct Inode {
  pub inodeno: usize,
  pub d: Option<DiskInode>,
}
