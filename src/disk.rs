use std::path::Path;
use std::sync::Mutex;
use std::sync::mpsc;
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
  channel: Option<mpsc::Sender<Request>>,
}

lazy_static! {
  // TODO: delegate the mutex to channel.
  pub static ref DISK: Mutex<DiskService> = Mutex::new(DiskService {
    channel: None
  });
}

impl Disk {
  pub fn new(nblocks: usize) -> Self {
    let mut blocks = Vec::with_capacity(nblocks);

    for _ in 0..nblocks {
      blocks.push([0; BSIZE]);
    }
    Disk { blocks }
  }

  pub fn load<P: AsRef<Path>>(path: P) -> Self {
    // TODO: load from path.
    unimplemented!();
  }

  pub fn save<P: AsRef<Path>>(path: P) {
    unimplemented!();
  }

  fn read(&self, blockno: usize) -> &Block {
    &self.blocks[blockno]
  }

  fn write(&mut self, blockno: usize, data: Block) {
    self.blocks[blockno] = data;
  }
}

// TODO: use Result instead of assert.
impl DiskService {
  pub fn mount(&mut self, mut disk: Disk) {
    assert!(self.channel.is_none());

    let (send, recv) = mpsc::channel();
    self.channel = Some(send.clone());
    thread::spawn(move || loop {
      let m = recv.recv();

      if m.is_err() {
        println!("{}", m.err().unwrap());
        break;
      }
      match m.unwrap() {
        Request::Read { reply, blockno } => {
          reply.send(*disk.read(blockno));
        },
        Request::Write {
          reply,
          blockno,
          data,
        } => {
          disk.write(blockno, data);
          reply.send(());
        },
        Request::Exit { reply } => {
          reply.send(disk);
          break;
        },
      }
    });
  }

  pub fn unmount(&mut self) -> Disk {
    assert!(self.channel.is_some());

    let (send, recv) = mpsc::channel();

    self.channel.as_ref().unwrap().send(
      Request::Exit { reply: send },
    );
    let disk = recv.recv().unwrap();
    self.channel = None;
    disk
  }

  pub fn read(&mut self, blockno: usize) -> Block {
    assert!(self.channel.is_some());

    let (send, recv) = mpsc::channel();

    self.channel.as_ref().unwrap().send(Request::Read {
      reply: send,
      blockno: blockno,
    });
    recv.recv().unwrap()
  }

  pub fn write(&mut self, blockno: usize, data: &Block) {
    assert!(self.channel.is_some());

    let (send, recv) = mpsc::channel();

    self.channel.as_ref().unwrap().send(Request::Write {
      reply: send,
      blockno: blockno,
      data: *data,
    });
    recv.recv().unwrap()
  }
}

#[test]
fn test() {
  let disk = Disk::new(2);
  let mut serv = DISK.lock().unwrap();

  serv.mount(disk);

  let blk1: Block = [42; BSIZE];
  serv.write(1, &blk1);

  assert!(serv.read(0)[0] == 0);
  assert!(serv.read(1)[0] == 42);
}
