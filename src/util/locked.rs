/// `LockedItem` is to track a locked item in a container with every
/// item is protected by a individual lock, e.g. `HashMap<usize,
/// Arc<Mutex<T>>>`.
///
/// Every `LockedItem` represents an exclusively locked item in this
/// container. `UnlockedItem` is on the opposite.
///
/// Use `LockedItem::release` and `UnlockedItem::acquire` to convert
/// between each other.

use std::ops::{Deref, DerefMut};
use std::sync::{Arc, Mutex, MutexGuard};

pub struct UnlockedItem<T: ?Sized>(Arc<Mutex<T>>);

pub struct LockedItem<'a, T: 'a + ?Sized> {
  x: MutexGuard<'a, T>,
  ptr: Option<*const Mutex<T>>,
}

impl<T: ?Sized> UnlockedItem<T> {
  pub fn new(x: Arc<Mutex<T>>) -> Self {
    UnlockedItem(x)
  }

  pub fn acquire<'a>(self) -> LockedItem<'a, T> {
    unsafe {
      let ptr = Arc::into_raw(self.0);
      LockedItem {
        ptr: Some(ptr),
        x: (*ptr).lock().unwrap(),
      }
    }
  }
}

impl<'a, T: ?Sized> LockedItem<'a, T> {
  pub fn release(mut self) -> UnlockedItem<T> {
    let x = unsafe { Arc::from_raw(self.ptr.unwrap()) };
    self.ptr = None;
    drop(self);
    UnlockedItem(x)
  }
}

impl<T: ?Sized> Deref for UnlockedItem<T> {
  type Target = Arc<Mutex<T>>;
  fn deref(&self) -> &Arc<Mutex<T>> {
    &self.0
  }
}

impl<T: ?Sized> DerefMut for UnlockedItem<T> {
  fn deref_mut(&mut self) -> &mut Arc<Mutex<T>> {
    &mut self.0
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
    &mut *self.x
  }
}

impl<'a, T: ?Sized> Drop for LockedItem<'a, T> {
  fn drop(&mut self) {
    if let Some(ptr) = self.ptr {
      unsafe {
        let _rc = Arc::from_raw(ptr);
      }
    }
  }
}
