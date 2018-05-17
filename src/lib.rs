#![feature(specialization)]

#[macro_use]
extern crate lazy_static;

#[macro_use]
extern crate bitflags;

#[macro_use]
extern crate log;

#[macro_use]
pub mod util;
pub mod disk;
pub mod fs;
pub mod inode;
pub mod logging;

mod buffer;
#[allow(dead_code)]
mod bitmap;
mod testfs;
