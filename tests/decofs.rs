use std::process::Command;  // Run programs
use assert_cmd::prelude::*; // Add methods on commands
use predicates::prelude::*; // Used for writing assertions

#[test]
fn file_doesnt_exist() -> Result<(), Box<std::error::Error>> {
    let mut cmd = Command::main_binary()?;
    cmd.arg("foobar")
        .arg("test/file/doesnt/exist");
    println!("command: {:?}", cmd);
    cmd.assert()
        .failure()
        .stderr("test")
        .stderr(predicate::str::contains("No such file or directory"));

    Ok(())
}