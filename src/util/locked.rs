/// `LockedItem` is to track a locked item in a container with every
/// item is protected by a individual lock, e.g. `HashMap<usize,
/// LockedItem<T>>`.
///
/// Every `LockedItem` represents an exclusively locked item in this
/// container. `UnlockedItem` is on the opposite.
///
/// Use `LockedItem::release` and `UnlockedItem::acquire` to convert
/// between each other.

use std::ops::{Deref, DerefMut};
use std::sync::{Arc, Mutex, MutexGuard};

pub struct UnlockedItem<T: ?Sized, U> {
  x: Arc<Mutex<T>>,
  pub no: U,  // constant that does need a lock.
}

pub struct LockedItem<'a, T: 'a + ?Sized, U> {
  x: MutexGuard<'a, T>,
  pub no: U,

  ptr: Option<*const Mutex<T>>,
}

impl<T: ?Sized, U> UnlockedItem<T, U> {
  pub fn new(x: Arc<Mutex<T>>, no: U) -> Self {
    UnlockedItem { x, no }
  }

  pub fn acquire<'a>(self) -> LockedItem<'a, T, U> {
    unsafe {
      let ptr = Arc::into_raw(self.x);
      LockedItem {
        ptr: Some(ptr),
        x: (*ptr).lock().unwrap(),
        no: self.no,
      }
    }
  }
}

impl<'a, T: ?Sized, U: Copy> LockedItem<'a, T, U> {
  pub fn release(mut self) -> UnlockedItem<T, U> {
    let x = unsafe { Arc::from_raw(self.ptr.unwrap()) };
    let no = self.no;
    self.ptr = None;
    drop(self);

    UnlockedItem::new(x, no)
  }
}

impl<T: ?Sized, U> Deref for UnlockedItem<T, U> {
  type Target = Arc<Mutex<T>>;
  fn deref(&self) -> &Arc<Mutex<T>> {
    &self.x
  }
}

impl<T: ?Sized, U> DerefMut for UnlockedItem<T, U> {
  fn deref_mut(&mut self) -> &mut Arc<Mutex<T>> {
    &mut self.x
  }
}

impl<'a, T: ?Sized, U> Deref for LockedItem<'a, T, U> {
  type Target = T;
  fn deref(&self) -> &T {
    &*self.x
  }
}

impl<'a, T: ?Sized, U> DerefMut for LockedItem<'a, T, U> {
  fn deref_mut(&mut self) -> &mut T {
    &mut *self.x
  }
}

impl<'a, T: ?Sized, U> Drop for LockedItem<'a, T, U> {
  fn drop(&mut self) {
    if let Some(ptr) = self.ptr {
      unsafe {
        let _rc = Arc::from_raw(ptr);
      }
    }
  }
}
