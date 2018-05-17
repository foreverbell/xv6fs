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

pub trait UnlockedDrop {
  fn drop(&mut self);
}

pub struct UnlockedItem<T: Sized, U: Copy> {
  x: Arc<(Mutex<T>, U)>,
  // U is some constant that does not need a lock.
}

pub struct LockedItem<'a, T: 'a + Sized, U: Copy> {
  x: MutexGuard<'a, T>,
  pub no: U,

  ptr: Option<*const (Mutex<T>, U)>,
}

impl<T: Sized, U: Copy> UnlockedItem<T, U> {
  pub fn new(x: Arc<(Mutex<T>, U)>) -> Self {
    UnlockedItem { x }
  }

  pub fn no(&self) -> U {
    self.x.1
  }

  pub fn acquire<'a>(&self) -> LockedItem<'a, T, U> {
    unsafe {
      let ptr = Arc::into_raw(self.x.clone());
      LockedItem {
        ptr: Some(ptr),
        x: (*ptr).0.lock().unwrap(),
        no: self.x.1,
      }
    }
  }

  // Returns the reference count of this unlocked item.
  // Notice the reference storing in the container is excluded.
  pub fn refcnt(&self) -> usize {
    Arc::strong_count(&self.x) - 1
  }

  pub fn disassemble(self) -> *const (Mutex<T>, U) {
    Arc::into_raw(self.x.clone())
  }

  pub fn assemble(ptr: *const (Mutex<T>, U)) -> Self {
    unsafe { UnlockedItem::new(Arc::from_raw(ptr)) }
  }
}

impl<'a, T: Sized, U: Copy> LockedItem<'a, T, U> {
  pub fn release(mut self) -> UnlockedItem<T, U> {
    let x = unsafe { Arc::from_raw(self.ptr.unwrap()) };
    self.ptr = None;
    drop(self);

    UnlockedItem::new(x)
  }
}

// Workaround for Drop trait cannot be specialized.
// We have a chance to do some clean-ups here before `UnlockedItem`
// is getting dropped.
impl<T: Sized, U: Copy> UnlockedDrop for UnlockedItem<T, U> {
  default fn drop(&mut self) {
    ()
  }
}

impl<T: Sized, U: Copy> Clone for UnlockedItem<T, U> {
  fn clone(&self) -> Self {
    UnlockedItem { x: self.x.clone() }
  }
}

impl<T: Sized, U: Copy> Drop for UnlockedItem<T, U> {
  fn drop(&mut self) {
    UnlockedDrop::drop(self);
  }
}

impl<'a, T: Sized, U: Copy> Deref for LockedItem<'a, T, U> {
  type Target = T;
  fn deref(&self) -> &T {
    &*self.x
  }
}

impl<'a, T: Sized, U: Copy> DerefMut for LockedItem<'a, T, U> {
  fn deref_mut(&mut self) -> &mut T {
    &mut *self.x
  }
}

impl<'a, T: Sized, U: Copy> Drop for LockedItem<'a, T, U> {
  fn drop(&mut self) {
    if let Some(ptr) = self.ptr {
      unsafe {
        let _un = UnlockedItem::new(Arc::from_raw(ptr));
      }
    }
  }
}
