#[macro_use]
extern crate log;
extern crate env_logger;

use std::env;
use std::path::PathBuf;
use std::ffi::{OsStr,OsString};
use libc::{c_int, EROFS};
use fuse::{FileType, FileAttr, Filesystem, Request, Reply, ReplyData, ReplyEntry, ReplyAttr, ReplyDirectory};

macro_rules! fuse_error {
    ($reply:expr, $code: ident) => {
        $reply.error($code)
    }
}

struct DecoFS {
    sourceroot: PathBuf
}

impl DecoFS {
    fn new(sourceroot: &OsStr) -> DecoFS {
      DecoFS { sourceroot: PathBuf::from(sourceroot) }
    }
}

impl Filesystem for DecoFS {

    fn mknod(
    &mut self,
    _req: &Request,
    _parent: u64,
    _name: &OsStr,
    _mode: u32,
    _rdev: u32,
    reply: ReplyEntry
    ) {
        fuse_error!(reply, EROFS);
    }
}

fn main() {
    env_logger::init();

    let mountpoint = env::args_os().nth(1).unwrap();
    let sourceroot = env::args_os().nth(2).unwrap();

    let fs = DecoFS::new(&sourceroot);
    let options = ["-o", "ro", "-o", "fsname=decofs", "-o", "allow_other"]
        .iter()
        .map(|o| o.as_ref())
        .collect::<Vec<&OsStr>>();
    fuse::mount(fs, &mountpoint, &options).unwrap();
}
