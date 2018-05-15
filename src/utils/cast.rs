// Creates a new object of type `T` from `block`.
//
// `block` should be a reference.
macro_rules! from_block {
  ($block:expr, $T:ty) => ({
    const SIZE: usize = ::std::mem::size_of::<$T>();
    let mut data: [u8; SIZE] = [0; SIZE];

    data.copy_from_slice(&$block[0..SIZE]);
    unsafe {
      ::std::mem::transmute::<[u8; SIZE], $T>(data)
    }
  });
}

// Creates a new block from `obj` of type `T`. The usused space is
// filled with zero.
//
// `obj` should be a reference.
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
