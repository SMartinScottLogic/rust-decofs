#[macro_use]
extern crate lazy_static;

use std::{fs, thread, time};
use std::process::{Command, Child};  // Run programs
use assert_cmd::prelude::*; // Add methods on commands
use std::sync::Mutex;
use std::path::PathBuf;

struct FuseMounter {
    child: Option<Child>
}

impl FuseMounter {
    fn new() -> FuseMounter {
        FuseMounter { child: None }
    }

    fn source(&self) -> PathBuf {
        PathBuf::from("t2")
    }

    fn target(&self) -> PathBuf {
        PathBuf::from("t")
    }

    fn mount(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let mut cmd = Command::main_binary()?;
        cmd.arg(self.target()).arg(self.source());
        self.child = Some(cmd.spawn()?);
        thread::sleep(time::Duration::from_millis(1000));
        Ok(())
    }

    fn umount(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        Command::new("fusermount").arg("-u").arg(self.target()).spawn()?.wait()?;
        let child = self.child.take();
        child.unwrap().wait()?;
        Ok(())
    }
}

lazy_static! {
    static ref MOUNTER: Mutex<FuseMounter> = Mutex::new(FuseMounter::new());
}

#[test]
fn can_readthru() -> Result<(), Box<dyn std::error::Error>> {
    let mut mounter = MOUNTER.lock()?;
    mounter.mount();
    fs::write(mounter.source().join("hello"), "world")?;
    let actual = fs::read_to_string(mounter.target().join("hello"))?;
    assert_eq!(actual, "world");
    mounter.umount();
    Ok(())
}
