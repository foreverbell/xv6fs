use std::convert::AsMut;

// https://stackoverflow.com/questions/37678698/function-to-build-a-fixed-sized-array-from-slice
pub fn make_array<A, T>(slice: &[T]) -> A
where
  A: Sized + Default + AsMut<[T]>,
  T: Copy,
{
  let mut a = Default::default();
  // The type cannot be inferred!
  // a.as_mut().copy_from_slice(slice);
  <A as AsMut<[T]>>::as_mut(&mut a).copy_from_slice(slice);
  a
}

macro_rules! from_block {
  ($block:expr, $T:ty) => ({
    const SIZE: usize = ::std::mem::size_of::<$T>();
    let data: [u8; SIZE] = $crate::utils::cast::make_array(&$block[0..SIZE]);

    unsafe {
      ::std::mem::transmute::<[u8; SIZE], $T>(data)
    }
  });
}

macro_rules! to_block {
  ($obj:expr, $T:ty) => ({
    let mut block: $crate::disk::Block = [0; $crate::disk::BSIZE];
    let mut pointer: *const u8 = $obj as *const $T as *const u8;
    let size = ::std::cmp::min(
      ::std::mem::size_of::<$T>(),
      $crate::disk::BSIZE
    );

    for i in 0..size {
      unsafe {
        block[i] = *pointer;
        pointer = pointer.add(1);
      }
    }
    block
  });
}

#[cfg(test)]
mod test {
  #[repr(C)]
  #[derive(PartialEq, Eq)]
  struct Foo {
    x: i32,
    y: i64,
  }

  #[test]
  fn test() {
    let bar = Foo { x: 42, y: 64 };
    let block = to_block!(&bar, Foo);
    let bar2 = from_block!(&block, Foo);

    assert!(bar == bar2);
    assert!(bar2.x == 42);
    assert!(bar2.y == 64);
  }
}
