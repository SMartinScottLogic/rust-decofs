#[macro_use]
extern crate log;
extern crate env_logger;

use fuse::{FileType, FileAttr, Filesystem, Request, ReplyData, ReplyEntry, ReplyAttr, ReplyDirectory};
struct DecoFS {
    sourceroot: PathBuf
}

impl DecoFS {
    fn new(sourceroot: &OsStr) -> DecoFS {
      DecoFS { sourceroot: PathBuf::from(sourceroot) }
    }
}

impl Filesystem for DecoFS {
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
