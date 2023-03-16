use crate::{
    error::Error,
    item::Item,
};

use std::{
    process::{Command, Stdio},
    io::Write,
};

use serde_json;

pub fn parse(s: &str) -> Result<Vec<Item>, Error> {
    let mut child = Command::new("./parser.exe")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| Error::ParserSpawnError(e))?;

    let p_stdin = child
        .stdin
        .as_mut()
        .unwrap();
    p_stdin.write_all(s.as_ref())
        .map_err(|e| Error::ParserPipeError(e))?;
    drop(p_stdin);

    let output = child
        .wait_with_output()
        .map_err(|e| Error::ParserPipeError(e))?;

    match output.status.success() {
        true => serde_json::from_slice(&output.stdout)
            .map_err(|e| Error::ParserDeserializationError(e)),
        false => Err(Error::ParserRuntimeError(
            String::from_utf8_lossy(&output.stderr).to_string()
        )),
    }
}
