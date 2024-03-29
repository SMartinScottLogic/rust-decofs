//! Implementation of a pass-through filesystem, to assist with disk decommissioning.
//!
//! Supports file deletion, and read operations, but no creation or renaming.
#![warn(missing_docs, bad_style, unused, unused_extern_crates, unused_import_braces, unused_qualifications, missing_debug_implementations, unused_variables)]
#[macro_use]
extern crate log;
use env_logger;
use time;

use std::env;
use std::{fs,io};
use std::path::{Path, PathBuf};
use std::ffi::{CString, OsStr};
use std::collections::HashMap;
use std::os::linux::fs::MetadataExt;
use libc::{c_int, EPERM, ENOENT, ENOTSUP};
use time::Timespec;
use std::io::prelude::*;
use std::io::SeekFrom;
use std::fs::File;
use std::os::unix::ffi::OsStrExt;

use fuse::{FileType, FileAttr, Filesystem, Request, ReplyData, ReplyEntry, ReplyAttr, ReplyStatfs, ReplyDirectory, ReplyEmpty, ReplyOpen, ReplyWrite, ReplyCreate, ReplyLock, ReplyBmap, ReplyXattr};

const TTL: Timespec = Timespec { sec: 1, nsec: 0}; // 1 second

/// Trait to assign to Reply* types, for commonality of error methods.
trait FuseError {
    /// Reply to a request with the given error code.
    fn fuse_error(self, code: c_int);
}

/// Macro to add FuseError trait
macro_rules! add_fuse_error {
    ($type: ty) => {
        impl FuseError for $type {
            fn fuse_error(self, code: c_int) {
                self.error(code);
            }
        }
    }
}
add_fuse_error!(ReplyEntry);
add_fuse_error!(ReplyEmpty);
add_fuse_error!(ReplyAttr);
add_fuse_error!(ReplyWrite);
add_fuse_error!(ReplyOpen);
add_fuse_error!(ReplyData);
add_fuse_error!(ReplyStatfs);
add_fuse_error!(ReplyCreate);
add_fuse_error!(ReplyXattr);
add_fuse_error!(ReplyDirectory);

struct DecoFS {
    inodes: HashMap<u64, String>
}

impl DecoFS {
    fn new(sourceroot: &OsStr) -> DecoFS {
        let mut inodes = HashMap::new();
        inodes.insert(1, sourceroot.to_str().unwrap().to_string());
        DecoFS { inodes }
    }
    fn stat(&self, path: &PathBuf) -> io::Result<FileAttr> {
      info!("stat {:?}", path);
      let attr = fs::metadata(path)?;

      let file_type = match attr.is_dir() {
        true => FileType::Directory,
        false => FileType::RegularFile
      };
      let file_attr = FileAttr {
        ino: attr.st_ino(),
        size: attr.st_size(),
        blocks: attr.st_blocks(),
        atime: Timespec {sec: attr.st_atime(), nsec: attr.st_atime_nsec() as i32},
        mtime: Timespec {sec: attr.st_mtime(), nsec: attr.st_mtime_nsec() as i32},
        ctime: Timespec {sec: attr.st_ctime(), nsec: attr.st_ctime_nsec() as i32},
        crtime: Timespec {sec: 0, nsec: 0},
        kind: file_type,
        perm: attr.st_mode() as u16,
        nlink: attr.st_nlink() as u32,
        uid: attr.st_uid(),
        gid: attr.st_gid(),
        rdev: attr.st_rdev() as u32,
        flags: 0,
      };
      info!("file_attr {:?}", file_attr);
      Ok(file_attr)
    }

    fn ino_to_path(&self, ino: u64) -> Result<PathBuf, c_int> {
        info!("ino_to_path {}", ino);
        match self.inodes.get(&ino) {
            Some(pathname) => {
                info!("ino_to_path {} -> {}", ino, pathname);
                Ok(PathBuf::from(pathname))
            },
            None => {
                info!("ino_to_path {} ENOENT", ino);
                Err(ENOENT)
            }
        }
    }

    fn get_source_path(&self, parent: u64, name: &OsStr) -> Result<PathBuf, c_int> {
        let root = self.ino_to_path(parent)?;
        Ok(root.join(name))
    }

    fn apply_to_path<T: FuseError, F>(&self, parent: u64, name: &OsStr, reply: T, f: F) where F:Fn(PathBuf, T) {
        match self.get_source_path(parent, name) {
            Ok(path) => f(path, reply),
            Err(e) => reply.fuse_error(e)
        }
    }

    fn apply_to_ino<T: FuseError, F>(&self, ino: u64, reply: T, f: F) where F:Fn(PathBuf, T) {
        match self.ino_to_path(ino) {
            Ok(path) => f(path, reply),
            Err(e) => reply.fuse_error(e)
        }
    }
}

impl Filesystem for DecoFS {
    fn init(&mut self, _req: &Request) -> Result<(), c_int> {
        info!("init");
        Ok(())
    }
    fn destroy(&mut self, _req: &Request) {
        info!("destroy");
    }
    fn lookup(&mut self, _req: &Request, parent: u64, name: &OsStr, reply: ReplyEntry) {
        info!("lookup {} {:?}", parent, name);
        let path = match self.get_source_path(parent, name) {
            Ok(path) => path,
            Err(e) => {reply.fuse_error(e);return;}
        };
        match &self.stat(&path) {
            Ok(stat) => {
                self.inodes.insert(stat.ino, path.as_os_str().to_string_lossy().to_string());
                reply.entry(&TTL, stat, 0);
                },
            Err(e) => reply.fuse_error(e.raw_os_error().unwrap())
        }
    }
    fn forget(&mut self, _req: &Request, ino: u64, _nlookup: u64) {
        info!("forget {}", ino);
    }
    fn getattr(&mut self, _req: &Request, ino: u64, reply: ReplyAttr) {
        info!("getattr {:?}", ino);
        self.apply_to_ino(ino, reply, |path, reply| reply.attr(&TTL, &self.stat(&path).unwrap()))
    }
    fn readlink(&mut self, _req: &Request, ino: u64, reply: ReplyData) {
        info!("readlink {:?}", ino);
        self.apply_to_ino(ino, reply, |path, reply| match fs::read_link(&path) {
            Ok(target) => reply.data(target.as_os_str().to_string_lossy().as_bytes()),
            Err(e) => reply.fuse_error(e.raw_os_error().unwrap())
        })
    }
    fn unlink(&mut self, _req: &Request, parent: u64, name: &OsStr, reply: ReplyEmpty) {
        info!("unlink {:?} {:?}", parent, name);
        self.apply_to_path(parent, name, reply, |path, reply| match fs::remove_file(&path) {
                 Ok(_) => reply.ok(),
                 Err(e) => reply.fuse_error(e.raw_os_error().unwrap())
            })
    }
    fn rmdir(&mut self, _req: &Request, parent: u64, name: &OsStr, reply: ReplyEmpty) {
        info!("rmdir {:?} {:?}", parent, name);
        self.apply_to_path(parent, name, reply, |path, reply| match fs::remove_dir(&path) {
                 Ok(_) => reply.ok(),
                 Err(e) => reply.fuse_error(e.raw_os_error().unwrap())
            })
    }
    fn open(&mut self, _req: &Request, ino: u64, _flags: u32, reply: ReplyOpen) {
        info!("open {:?}", ino);
        self.apply_to_ino(ino, reply, |_path, reply| reply.opened(0, 0))
    }
    fn read(&mut self, _req: &Request, ino: u64, _fh: u64, offset: i64, size: u32, reply: ReplyData) {
        info!("read {:?} {} {}", ino, offset, size);
        self.apply_to_ino(ino, reply, |path, reply| {
            let buffer = || -> io::Result<Vec<u8>> {
                let mut f = File::open(&path)?;
                f.seek(SeekFrom::Start(offset as u64))?;
                let mut handle = f.take(size.into());
                let mut buffer = Vec::new();
                handle.read_to_end(&mut buffer)?;
                Ok(buffer)
            };
            match buffer() {
                Ok(buffer) => reply.data(&buffer),
                Err(e) => reply.fuse_error(e.raw_os_error().unwrap())
            }
        })
    }
    fn flush(&mut self, _req: &Request, ino: u64, _fh: u64, _lock_owner: u64, reply: ReplyEmpty) {
        self.apply_to_ino(ino, reply, |_path, reply| reply.ok());
    }
    fn release(&mut self, _req: &Request, ino: u64, _fh: u64, _flags: u32, _lock_owner: u64, _flush: bool, reply: ReplyEmpty) {
        self.apply_to_ino(ino, reply, |_path, reply| reply.ok());
    }
    fn fsync(&mut self, _req: &Request, ino: u64, _fh: u64, _datasync: bool, reply: ReplyEmpty) {
        self.apply_to_ino(ino, reply, |_path, reply| reply.ok());
    }
    fn opendir(&mut self, _req: &Request, ino: u64, _flags: u32, reply: ReplyOpen) {
        self.apply_to_ino(ino, reply, |_path, reply| reply.opened(0, 0));
    }
    fn readdir(&mut self, _req: &Request, ino: u64, _fh: u64, offset: i64, mut reply: ReplyDirectory) {
        info!("readdir {} {}", ino, offset);
        let root = match self.ino_to_path(ino) {
            Ok(path) => path,
            Err(e) => {reply.fuse_error(e);return;}
        };
        let mut entries = vec![ (1, FileType::Directory, String::from(".")), (1, FileType::Directory, String::from("..")) ];
        for entry in fs::read_dir(&root).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            let attr = fs::metadata(&path).unwrap();
            let file_name = path.file_name().unwrap().to_str().unwrap().to_string();
            let file_type = match attr.is_dir() {
                true => FileType::Directory,
                false => FileType::RegularFile
            };

            entries.push((attr.st_ino(), file_type, file_name.clone()));
            self.inodes.insert(attr.st_ino(), root.join(file_name).to_str().unwrap().to_string());
        }
        info!("entries: {:?}", entries);

        // Offset of 0 means no offset.
        // Non-zero offset means the passed offset has already been seen, and we should start after
        // it.
        let to_skip = if offset == 0 { offset } else { offset + 1 } as usize;
        for (i, entry) in entries.into_iter().enumerate().skip(to_skip) {
            info!("reply {}, {}, {:?}, {}", entry.0, i as i64, entry.1, entry.2);
            let r = reply.add(entry.0, i as i64, entry.1, entry.2);
            info!("r {}", r);
        }
        reply.ok();
    }
    fn releasedir(&mut self, _req: &Request, _ino: u64, _fh: u64, _flags: u32, reply: ReplyEmpty) {
        reply.ok();
    }
    fn fsyncdir(&mut self, _req: &Request, ino: u64, _fh: u64, _datasync: bool, reply: ReplyEmpty) {
        self.apply_to_ino(ino, reply, |_path, reply| reply.ok());
    }
    fn statfs(&mut self, _req: &Request, ino: u64, reply: ReplyStatfs) {
        info!("statfs {:?}", ino);
        self.apply_to_ino(ino, reply, |path, reply| unsafe {
            let stat = || -> io::Result<libc::statfs> {
                let mut stat: libc::statfs = std::mem::uninitialized();
                let cstr = CString::new(path.as_os_str().as_bytes())?;
                libc::statfs(cstr.as_ptr(), &mut stat);
                Ok(stat)
            };
            match stat() {
                Ok(stat) => reply.statfs(stat.f_blocks, stat.f_bfree, stat.f_bavail, stat.f_files, stat.f_ffree, stat.f_bsize as u32, stat.f_namelen as u32, stat.f_frsize as u32),
                Err(e) => reply.fuse_error(e.raw_os_error().unwrap())
            }
        })
    }
    fn getxattr(&mut self, _req: &Request, ino: u64, name: &OsStr, _size: u32, reply: ReplyXattr) {
        info!("getxattr {:?} {:?}", ino, name);
        self.apply_to_ino(ino, reply, |_path, reply| {
            reply.fuse_error(ENOTSUP);
        })
    }
    fn listxattr(&mut self, _req: &Request, ino: u64, size: u32, reply: ReplyXattr) {
        info!("listxattr {:?} {}", ino, size);
        self.apply_to_ino(ino, reply, |path, reply| {
            let xattr = || -> io::Result<String> {
                Ok("".to_string())
            };
            match size {
                0 => unsafe {let mut list:i8 = 0;reply.size(libc::listxattr(CString::new(path.as_os_str().as_bytes()).unwrap().as_ptr(),  &mut list, 0) as u32);},
                // TODO implement
                _ => reply.fuse_error(ENOENT)
            }
        })
    }
    fn access(&mut self, _req: &Request, ino: u64, mask: u32, reply: ReplyEmpty) {
        info!("access {} {}", ino, mask);
        self.apply_to_ino(ino, reply, |_path, reply| reply.ok());
    }
    fn getlk(&mut self, _req: &Request, _ino: u64, _fh: u64, _lock_owner: u64, _start: u64, _end: u64, _typ: u32, _pid: u32, reply: ReplyLock) {
        // TODO implement
    }
    fn bmap(&mut self, _req: &Request, _ino: u64, _blocksize: u32, _idx: u64, reply: ReplyBmap) {
        // TODO implement
    }
    // Disabled functionality
    /// For this deco filesystem, we do not support setting attributes.
    fn setattr(&mut self, _req: &Request, ino: u64, _mode: Option<u32>, _uid: Option<u32>, _gid: Option<u32>, _size: Option<u64>, _atime: Option<Timespec>, _mtime: Option<Timespec>, _fh: Option<u64>, _crtime: Option<Timespec>, _chgtime: Option<Timespec>, _bkuptime: Option<Timespec>, _flags: Option<u32>, reply: ReplyAttr) {
        info!("setattr {}", ino);
        reply.fuse_error(EPERM)
    }
    /// For this deco filesystem, we do not support creating nodes (regular file, character device, block device, fifo or socket).
    fn mknod(&mut self, _req: &Request, parent: u64, name: &OsStr, _mode: u32, _rdev: u32, reply: ReplyEntry) {
        info!("mknod {} {:?}", parent, name);
        println!("mknod {} {:?}", parent, name);
        reply.fuse_error(EPERM)
    }
    /// For this deco filesystem, we do not support creating directories.
    fn mkdir(&mut self, _req: &Request, parent: u64, name: &OsStr, _mode: u32, reply: ReplyEntry) {
        info!("mkdir {} {:?}", parent, name);
        reply.fuse_error(EPERM)
    }
    /// For this deco filesystem, we do not support creating symbolic links.
    fn symlink(&mut self, _req: &Request, parent: u64, name: &OsStr, _link: &Path, reply: ReplyEntry) {
        info!("symlink {} {:?}", parent, name);
        reply.fuse_error(EPERM)
     }
    /// For this deco filesystem, we do not support renaming files.
    fn rename(&mut self, _req: &Request, parent: u64, name: &OsStr, _newparent: u64, _newname: &OsStr, reply: ReplyEmpty) {
        info!("rename {} {:?}", parent, name);
        reply.fuse_error(EPERM)
    }
    /// For this deco filesystem, we do not support creating hard links.
    fn link(&mut self, _req: &Request, ino: u64, newparent: u64, newname: &OsStr, reply: ReplyEntry) {
        info!("link {} {} {:?}", ino, newparent, newname);
        reply.fuse_error(EPERM)
    }
    /// For this deco filesystem, we do not support writing to files.
    fn write(&mut self, _req: &Request, ino: u64, _fh: u64, offset: i64, data: &[u8], _flags: u32, reply: ReplyWrite) {
        info!("write {} {} {}", ino, offset, data.len());
        reply.fuse_error(EPERM)
    }
    /// For this deco filesystem, we do not support writing to extended attributes.
    fn setxattr(&mut self, _req: &Request, ino: u64, name: &OsStr, value: &[u8], _flags: u32, _position: u32, reply: ReplyEmpty) {
        info!("setxattr {} {:?} {:?}", ino, name, value);
        reply.fuse_error(EPERM)
    }
    /// For this deco filesystem, we do not support removing extended attributes.
    fn removexattr(&mut self, _req: &Request, ino: u64, name: &OsStr, reply: ReplyEmpty) {
        info!("removexattr {} {:?}", ino, name);
        reply.fuse_error(EPERM)
    }
    /// For this deco filesystem, we do not support creating files.
    fn create(&mut self, _req: &Request, parent: u64, name: &OsStr, _mode: u32, _flags: u32, reply: ReplyCreate) {
        info!("create {} {:?}", parent, name);
        reply.fuse_error(EPERM)
    }
    /// For this deco filesystem, we do not support file locks.
    fn setlk(&mut self, _req: &Request, ino: u64, _fh: u64, _lock_owner: u64, _start: u64, _end: u64, _typ: u32, _pid: u32, _sleep: bool, reply: ReplyEmpty) {
        info!("setlk {}", ino);
        reply.fuse_error(EPERM)
    }
}

fn main() {
    env_logger::init();

    let mountpoint = env::args_os().nth(1).unwrap();
    let sourceroot = env::args_os().nth(2).unwrap();

    let fs = DecoFS::new(&sourceroot);
    let options = ["-o", "rw", "-o", "fsname=decofs", "-o", "allow_other"]
        .iter()
        .map(|o| o.as_ref())
        .collect::<Vec<&OsStr>>();
    fuse::mount(fs, &mountpoint, &options).unwrap();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ino_to_path_root() {
        let fs = DecoFS::new(OsStr::new("t"));
        match fs.ino_to_path(1) {
            Ok(path) => assert_eq!(path, PathBuf::from("t")),
            _ => assert!(false)
        };
    }

    #[test]
    fn ino_to_path_missing() {
        let fs = DecoFS::new(OsStr::new("t"));
        match fs.ino_to_path(2) {
            Ok(_path) => assert!(false),
            _ => assert!(true)
        };
    }

    #[test]
    fn apply_to_ino_root() {
        struct TestReply { };
        impl FuseError for TestReply {
            fn fuse_error(self, _code: c_int) {
                assert!(false);
            }
        }

        let reply: TestReply = TestReply {};
        let fs = DecoFS::new(OsStr::new("t"));
        fs.apply_to_ino(1, reply, |_path, _reply| assert!(true));
    }

    #[test]
    fn apply_to_ino_missing() {
        struct TestReply { };
        impl FuseError for TestReply {
            fn fuse_error(self, code: c_int) {
                assert_eq!(code, ENOENT);
            }
        }

        let reply: TestReply = TestReply {};
        let fs = DecoFS::new(OsStr::new("t"));
        fs.apply_to_ino(2, reply, |_path, _reply| assert!(false));
    }
}
