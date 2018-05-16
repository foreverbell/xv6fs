#![feature(specialization)]

#[macro_use]
extern crate lazy_static;

#[macro_use]
extern crate bitflags;

#[macro_use]
mod util;

#[allow(dead_code)]
#[allow(unused_must_use)]
mod disk;

#[allow(dead_code)]
#[allow(unused_must_use)]
pub mod fs;

#[allow(dead_code)]
#[allow(unused_must_use)]
mod buffer;

#[allow(dead_code)]
#[allow(unused_must_use)]
pub mod logging;

#[allow(dead_code)]
#[allow(unused_must_use)]
mod bitmap;

#[allow(dead_code)]
#[allow(unused_must_use)]
pub mod inode;
