#[macro_use]
extern crate lazy_static;

use std::{fs, thread, time};
use std::process::{Command, Child};  // Run programs
use assert_cmd::prelude::*; // Add methods on commands

struct FuseMounter {
    child: Option<Child>
}

impl FuseMounter {
    fn new() -> FuseMounter {
        FuseMounter { child: None }
    }

    fn mount(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let mut cmd = Command::main_binary()?;
        cmd.arg("t").arg("t2");
        self.child = Some(cmd.spawn()?);
        thread::sleep(time::Duration::from_millis(1000));
        Ok(())
    }

    fn umount(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        Command::new("fusermount").arg("-u").arg("t").spawn()?.wait()?;
        let child = self.child.take();
        child.unwrap().wait()?;
        Ok(())
    }
}

#[test]
fn can_readthru() -> Result<(), Box<dyn std::error::Error>> {
    let mut mounter = FuseMounter::new();
    mounter.mount();
    fs::write("t2/hello", "world")?;
    let actual = fs::read_to_string("t/hello")?;
    assert_eq!(actual, "world");
    mounter.umount();
    Ok(())
}
