#[macro_use]
extern crate lazy_static;

use assert_cmd::prelude::*; // Add methods on commands
use std::path::PathBuf;
use std::process::{Child, Command}; // Run programs
use std::{fs, thread, time};

use std::ops::{Deref, DerefMut};
use std::sync::{Mutex, MutexGuard, PoisonError};

struct FuseMounter {
    child: Option<Child>,
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
        Command::new("fusermount")
            .arg("-u")
            .arg(self.target())
            .spawn()?
            .wait()?;
        let child = self.child.take();
        child.unwrap().wait()?;
        Ok(())
    }
}

lazy_static! {
    static ref MOUNTER: MutexMounter = MutexMounter::new();
}

struct MutexMounter {
    mounter: Mutex<FuseMounter>,
}

struct MutexMounterGuard<'a> {
    inner_guard: MutexGuard<'a, FuseMounter>,
}

impl MutexMounter {
    fn new() -> MutexMounter {
        MutexMounter {
            mounter: Mutex::new(FuseMounter::new()),
        }
    }

    fn lock(&self) -> Result<MutexMounterGuard, PoisonError<MutexMounterGuard>> {
        MutexMounterGuard::new(self.mounter.lock().unwrap())
    }
}

impl MutexMounterGuard<'_> {
    fn new(
        mut inner_guard: MutexGuard<FuseMounter>,
    ) -> Result<MutexMounterGuard, PoisonError<MutexMounterGuard>> {
        inner_guard.mount().unwrap();
        Ok(MutexMounterGuard { inner_guard })
    }
}

impl Drop for MutexMounterGuard<'_> {
    fn drop(&mut self) {
        self.inner_guard.umount().unwrap();
    }
}

impl Deref for MutexMounterGuard<'_> {
    type Target = FuseMounter;

    fn deref(&self) -> &FuseMounter {
        self.inner_guard.deref()
    }
}

impl DerefMut for MutexMounterGuard<'_> {
    fn deref_mut(&mut self) -> &mut FuseMounter {
        self.inner_guard.deref_mut()
    }
}

#[test]
fn can_readthru() -> Result<(), Box<dyn std::error::Error>> {
    let actual = {
        let mounter = MOUNTER.lock()?;
        fs::write(mounter.source().join("hello"), "world")?;
        let actual = fs::read_to_string(mounter.target().join("hello"))?;
        actual
    };
    assert_eq!(actual, "world");
    Ok(())
}
