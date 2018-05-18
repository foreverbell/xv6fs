use std::path::Path;
use std::sync::{Mutex, mpsc};
use std::fs::File;
use std::io::Read;
use std::thread;

// Size of each block.
pub const BSIZE: usize = 512;

pub type Block = [u8; BSIZE];

pub struct Disk {
  blocks: Vec<Block>,
}

enum Request {
  Read {
    reply: mpsc::Sender<Block>,
    blockno: usize,
  },
  Write {
    reply: mpsc::Sender<()>,
    blockno: usize,
    data: Block,
  },
  Exit { reply: mpsc::Sender<Disk> },
}

pub struct DiskService {
  channel: Mutex<Option<mpsc::Sender<Request>>>,
}

lazy_static! {
  pub static ref DISK: DiskService = DiskService {
    channel: Mutex::new(None)
  };
}

impl Disk {
  pub fn new(nblocks: usize) -> Self {
    let mut blocks = Vec::with_capacity(nblocks);

    for _ in 0..nblocks {
      blocks.push([0; BSIZE]);
    }
    Disk { blocks }
  }

  pub fn from(blocks: Vec<Block>) -> Self {
    Disk { blocks }
  }

  pub fn load<P: AsRef<Path>>(path: P) -> Option<Self> {
    let mut f = File::open(path).unwrap();
    let size = f.metadata().unwrap().len() as usize;

    if size % BSIZE != 0 {
      return None;
    }

    let nblocks = size / BSIZE;
    let mut blocks = Vec::with_capacity(nblocks);
    for _ in 0..nblocks {
      let mut buf: [u8; BSIZE] = [0; BSIZE];
      f.read(&mut buf).unwrap();
      blocks.push(buf);
    }

    Some(Disk { blocks })
  }

  pub fn save<P: AsRef<Path>>(_path: P) {
    // TODO: save xv6fs disk image into host's disk.
    unimplemented!();
  }

  fn read(&self, blockno: usize) -> &Block {
    &self.blocks[blockno]
  }

  fn write(&mut self, blockno: usize, data: Block) {
    self.blocks[blockno] = data;
  }
}

impl DiskService {
  pub fn mount(&self, mut disk: Disk) {
    let mut channel = self.channel.lock().unwrap();
    if channel.is_some() {
      drop(channel);
      self.unmount();
      return self.mount(disk);
    }

    let (send, recv) = mpsc::channel();
    *channel = Some(send.clone());
    thread::spawn(move || loop {
      let m = recv.recv();

      if m.is_err() {
        println!("{}", m.err().unwrap());
        break;
      }
      match m.unwrap() {
        Request::Read { reply, blockno } => {
          reply.send(*disk.read(blockno)).unwrap();
        },
        Request::Write {
          reply,
          blockno,
          data,
        } => {
          disk.write(blockno, data);
          reply.send(()).unwrap();
        },
        Request::Exit { reply } => {
          reply.send(disk).unwrap();
          break;
        },
      }
    });
  }

  pub fn unmount(&self) -> Disk {
    let mut channel = self.channel.lock().unwrap();
    assert!(channel.is_some());

    let (send, recv) = mpsc::channel();

    channel
      .as_ref()
      .unwrap()
      .send(Request::Exit { reply: send })
      .unwrap();
    let disk = recv.recv().unwrap();
    *channel = None;
    disk
  }

  pub fn read(&self, blockno: usize) -> Block {
    let channel = self.channel.lock().unwrap();
    assert!(channel.is_some());

    let (send, recv) = mpsc::channel();

    channel
      .as_ref()
      .unwrap()
      .send(Request::Read {
        reply: send,
        blockno: blockno,
      })
      .unwrap();
    recv.recv().unwrap()
  }

  pub fn write(&self, blockno: usize, data: &Block) {
    let channel = self.channel.lock().unwrap();
    assert!(channel.is_some());

    let (send, recv) = mpsc::channel();

    channel
      .as_ref()
      .unwrap()
      .send(Request::Write {
        reply: send,
        blockno: blockno,
        data: *data,
      })
      .unwrap();
    recv.recv().unwrap()
  }
}

#[cfg(test)]
mod test {
  use disk::{Disk, Block, DISK, BSIZE};

  #[test]
  fn test() {
    let disk = Disk::new(2);

    DISK.mount(disk);

    let blk1: Block = [42; BSIZE];
    DISK.write(1, &blk1);

    assert!(DISK.read(0)[0] == 0);
    assert!(DISK.read(1)[0] == 42);
  }
}
