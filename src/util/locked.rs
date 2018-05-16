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

pub struct UnlockedItem<T: ?Sized, U: Copy> {
  x: Arc<Mutex<T>>,
  pub no: U, // constant that does need a lock.
}

// Workaround for Drop trait cannot be specialized.
// We have a chance to do some clean-ups here before `UnlockedItem`
// is getting dropped.
impl<T: ?Sized, U: Copy> UnlockedDrop for UnlockedItem<T, U> {
  default fn drop(&mut self) {
    ()
  }
}

pub struct LockedItem<'a, T: 'a + ?Sized, U: Copy> {
  x: MutexGuard<'a, T>,
  pub no: U,

  ptr: Option<*const Mutex<T>>,
}

impl<T: ?Sized, U: Copy> UnlockedItem<T, U> {
  pub fn new(x: Arc<Mutex<T>>, no: U) -> Self {
    UnlockedItem { x, no }
  }

  pub fn acquire<'a>(&self) -> LockedItem<'a, T, U> {
    unsafe {
      let ptr = Arc::into_raw(self.x.clone());
      LockedItem {
        ptr: Some(ptr),
        x: (*ptr).lock().unwrap(),
        no: self.no,
      }
    }
  }

  // Returns the reference count of this unlocked item.
  // Notice the reference storing in the container is excluded.
  pub fn refcnt(&self) -> usize {
    Arc::strong_count(self) - 1
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

impl<T: ?Sized, U: Copy> Deref for UnlockedItem<T, U> {
  type Target = Arc<Mutex<T>>;
  fn deref(&self) -> &Arc<Mutex<T>> {
    &self.x
  }
}

impl<T: ?Sized, U: Copy> DerefMut for UnlockedItem<T, U> {
  fn deref_mut(&mut self) -> &mut Arc<Mutex<T>> {
    &mut self.x
  }
}

impl<T: ?Sized, U: Copy> Drop for UnlockedItem<T, U> {
  fn drop(&mut self) {
    UnlockedDrop::drop(self);
  }
}

impl<'a, T: ?Sized, U: Copy> Deref for LockedItem<'a, T, U> {
  type Target = T;
  fn deref(&self) -> &T {
    &*self.x
  }
}

impl<'a, T: ?Sized, U: Copy> DerefMut for LockedItem<'a, T, U> {
  fn deref_mut(&mut self) -> &mut T {
    &mut *self.x
  }
}

impl<'a, T: ?Sized, U: Copy> Drop for LockedItem<'a, T, U> {
  fn drop(&mut self) {
    if let Some(ptr) = self.ptr {
      unsafe {
        let _un = UnlockedItem::new(Arc::from_raw(ptr), self.no);
      }
    }
  }
}
