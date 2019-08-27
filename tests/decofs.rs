use std::{fs, thread, time};
use std::process::Command;  // Run programs
use assert_cmd::prelude::*; // Add methods on commands

#[test]
fn can_readthru() -> Result<(), Box<dyn std::error::Error>> {
    fs::write("t2/hello", "world")?;
    let mut cmd = Command::main_binary()?;
    cmd.arg("t").arg("t2");
    let mut child = cmd.spawn()?;
    thread::sleep(time::Duration::from_millis(1000));
    let actual = fs::read_to_string("t/hello");
    Command::new("fusermount").arg("-u").arg("t").spawn()?.wait()?;
    child.wait()?;
    assert_eq!(actual.unwrap(), "world");
    Ok(())
}
