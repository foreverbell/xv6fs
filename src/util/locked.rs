/// `LockedItem` is to track a locked item in a container with every
/// item is protected by a individual lock, e.g. `HashMap<usize,
/// Arc<Mutex<T>>>`.
///
/// Every `LockedItem` represents an exclusively locked item in this
/// container. `Arc` guarantees this item can outlive the host
/// container.

use std::ops::{Deref, DerefMut};
use std::sync::{Arc, Mutex, MutexGuard};

pub struct LockedItem<'a, T: 'a + ?Sized> {
  x: MutexGuard<'a, T>,
  ptr: *const Mutex<T>,
}

impl<'a, T: ?Sized> LockedItem<'a, T> {
  pub fn new(x: Arc<Mutex<T>>) -> Self {
    unsafe {
      let ptr = Arc::into_raw(x);
      LockedItem {
        ptr: ptr,
        x: (*ptr).lock().unwrap(),
      }
    }
  }
}

impl<'a, T: ?Sized> Deref for LockedItem<'a, T> {
  type Target = T;
  fn deref(&self) -> &T {
    &*self.x
  }
}

impl<'a, T: ?Sized> DerefMut for LockedItem<'a, T> {
  fn deref_mut(&mut self) -> &mut T {
    &mut self.x
  }
}

impl<'a, T: ?Sized> Drop for LockedItem<'a, T> {
  fn drop(&mut self) {
    unsafe {
      let _rc = Arc::from_raw(self.ptr);
    }
  }
}
